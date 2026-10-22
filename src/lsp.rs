use crate::Result;
use std::{future::ready, ops::ControlFlow, path::Path, process::Stdio};

use async_lsp::{
    Error as LspError, LanguageServer as _, MainLoop, ServerSocket,
    lsp_types::{
        ClientCapabilities, DiagnosticClientCapabilities, DidChangeConfigurationParams,
        DidOpenTextDocumentParams, DocumentSymbolClientCapabilities, DocumentSymbolParams,
        DocumentSymbolResponse, InitializeParams, InitializedParams, Location, PartialResultParams,
        Position, ReferenceContext, ReferenceParams, TextDocumentClientCapabilities,
        TextDocumentIdentifier, TextDocumentItem, TextDocumentPositionParams, Url,
        WorkDoneProgressParams, WorkspaceFolder,
        request::{RegisterCapability, UnregisterCapability, WorkspaceDiagnosticRefresh},
    },
    router::Router,
};
use std::process;
use tokio::{
    fs::{canonicalize, read_to_string},
    process::Child,
    process::Command,
    task::JoinHandle,
};

pub(crate) type References = Option<Vec<Location>>;
type IoTask = JoinHandle<Result<(), LspError>>;
use tokio_util::compat::{TokioAsyncReadCompatExt as _, TokioAsyncWriteCompatExt as _};

pub(crate) struct LspSession {
    server: ServerSocket,
    child: Child,
    io_task: IoTask,
}

impl LspSession {
    #[expect(
        clippy::future_not_send,
        reason = "LSP initialization runs on a current-thread runtime"
    )]
    pub(crate) async fn start(root: &Path, command: &str, arguments: &[&str]) -> Result<Self> {
        let workspace_root = canonicalize(root).await?;
        let mut child = spawn_server(&workspace_root, command, arguments)?;
        let (mainloop, mut server) = MainLoop::new_client(client_router);
        let io_task = start_io(mainloop, &mut child)?;
        let _ = server
            .initialize(initialize_params(&workspace_root)?)
            .await?;
        server.initialized(InitializedParams {})?;

        Ok(Self {
            server,
            child,
            io_task,
        })
    }

    pub(crate) fn configure(&mut self, settings: serde_json::Value) -> Result<()> {
        self.server
            .did_change_configuration(DidChangeConfigurationParams { settings })?;
        Ok(())
    }

    pub(crate) async fn open_document(&mut self, path: &Path, language_id: &str) -> Result<()> {
        let canonical_path = path.canonicalize()?;
        let uri = file_uri(&canonical_path)?;

        let text = read_to_string(&canonical_path).await?;

        self.server.did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri,
                language_id: language_id.into(),
                version: 1,
                text,
            },
        })?;

        Ok(())
    }

    pub(crate) async fn document_symbols(
        &mut self,
        path: &Path,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = file_uri(&path.canonicalize()?)?;

        let symbols = self
            .server
            .document_symbol(DocumentSymbolParams {
                text_document: TextDocumentIdentifier { uri },
                work_done_progress_params: WorkDoneProgressParams::default(),
                partial_result_params: PartialResultParams::default(),
            })
            .await?;

        Ok(symbols)
    }

    pub(crate) async fn references(&mut self, uri: Url, position: Position) -> Result<References> {
        let references = self
            .server
            .references(ReferenceParams {
                text_document_position: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier { uri },
                    position,
                },
                context: ReferenceContext {
                    include_declaration: false,
                },
                work_done_progress_params: WorkDoneProgressParams::default(),
                partial_result_params: PartialResultParams::default(),
            })
            .await?;
        Ok(references)
    }

    pub(crate) async fn shutdown(mut self) -> Result<()> {
        self.server.shutdown(()).await?;
        self.server.exit(())?;

        let status = self.child.wait().await?;

        if !status.success() {
            return Err(format!("language server exited with {status}").into());
        }

        match self.io_task.await? {
            Ok(()) | Err(LspError::Eof) => {}
            Err(error) => return Err(error.into()),
        }

        Ok(())
    }
}

pub(crate) fn file_uri(path: &Path) -> Result<Url> {
    Url::from_file_path(path).map_err(|()| format!("invalid file path: {}", path.display()).into())
}

fn spawn_server(root: &Path, command: &str, arguments: &[&str]) -> Result<Child> {
    Ok(Command::new(command)
        .args(arguments)
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .kill_on_drop(true)
        .spawn()?)
}

fn client_router(_: ServerSocket) -> Router<()> {
    let mut router = Router::new(());
    let _ = router.request::<RegisterCapability, _>(|(), _| ready(Ok(())));
    let _ = router.request::<UnregisterCapability, _>(|(), _| ready(Ok(())));
    let _ = router.request::<WorkspaceDiagnosticRefresh, _>(|(), ()| ready(Ok(())));
    let _ = router.unhandled_notification(|(), _| ControlFlow::Continue(()));
    router
}

fn initialize_params(root: &Path) -> Result<InitializeParams> {
    Ok(InitializeParams {
        process_id: Some(process::id()),
        capabilities: ClientCapabilities {
            text_document: Some(document_capabilities()),
            ..ClientCapabilities::default()
        },
        workspace_folders: Some(vec![WorkspaceFolder {
            uri: Url::from_directory_path(root).map_err(|()| "invalid workspace directory")?,
            name: "workspace".into(),
        }]),
        ..InitializeParams::default()
    })
}

fn document_capabilities() -> TextDocumentClientCapabilities {
    // Pull diagnostics prevent background type-checking of every opened file.
    TextDocumentClientCapabilities {
        diagnostic: Some(DiagnosticClientCapabilities {
            dynamic_registration: Some(true),
            ..DiagnosticClientCapabilities::default()
        }),
        document_symbol: Some(DocumentSymbolClientCapabilities {
            hierarchical_document_symbol_support: Some(true),
            ..DocumentSymbolClientCapabilities::default()
        }),
        ..TextDocumentClientCapabilities::default()
    }
}

fn start_io(mainloop: MainLoop<Router<()>>, child: &mut Child) -> Result<IoTask> {
    let stdout = child
        .stdout
        .take()
        .ok_or("language server stdout was not piped")?;
    let stdin = child
        .stdin
        .take()
        .ok_or("language server stdin was not piped")?;
    Ok(tokio::spawn(async move {
        mainloop
            .run_buffered(stdout.compat(), stdin.compat_write())
            .await
    }))
}
