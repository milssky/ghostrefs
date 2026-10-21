use std::{ops::ControlFlow, path::Path, process::Stdio};

use async_lsp::{
    LanguageServer, MainLoop, ServerSocket,
    lsp_types::{
        ClientCapabilities, DiagnosticClientCapabilities, DidChangeConfigurationParams,
        DidOpenTextDocumentParams, DocumentSymbolClientCapabilities, DocumentSymbolParams,
        DocumentSymbolResponse, InitializeParams, InitializedParams, Location, Position,
        ReferenceContext, ReferenceParams, TextDocumentClientCapabilities, TextDocumentIdentifier,
        TextDocumentItem, TextDocumentPositionParams, Url, WorkspaceFolder,
        request::{RegisterCapability, UnregisterCapability, WorkspaceDiagnosticRefresh},
    },
    router::Router,
};
use tokio::{process::Child, process::Command, task::JoinHandle};
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

type Error = Box<dyn std::error::Error>;

pub struct LspSession {
    server: ServerSocket,
    child: Child,
    io_task: JoinHandle<Result<(), async_lsp::Error>>,
}

impl LspSession {
    pub async fn start(root: &Path, command: &str, args: &[&str]) -> Result<Self, Error> {
        let root = tokio::fs::canonicalize(root).await?;
        let mut child = Command::new(command)
            .args(args)
            .current_dir(&root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;

        let (mainloop, mut server) = MainLoop::new_client(|_| {
            let mut router = Router::new(());
            router.request::<RegisterCapability, _>(|_, _| async { Ok(()) });
            router.request::<UnregisterCapability, _>(|_, _| async { Ok(()) });
            router.request::<WorkspaceDiagnosticRefresh, _>(|_, _| async { Ok(()) });
            router.unhandled_notification(|_, _| ControlFlow::Continue(()));
            router
        });

        let stdout = child.stdout.take().unwrap();
        let stdin = child.stdin.take().unwrap();
        let io_task = tokio::spawn(async move {
            mainloop
                .run_buffered(stdout.compat(), stdin.compat_write())
                .await
        });

        server
            .initialize(InitializeParams {
                process_id: Some(std::process::id()),
                capabilities: ClientCapabilities {
                    text_document: Some(TextDocumentClientCapabilities {
                        // We only need symbols and references. Pull diagnostics keep
                        // Pyright from type-checking every opened file in the background.
                        diagnostic: Some(DiagnosticClientCapabilities {
                            dynamic_registration: Some(true),
                            ..Default::default()
                        }),
                        document_symbol: Some(DocumentSymbolClientCapabilities {
                            hierarchical_document_symbol_support: Some(true),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                workspace_folders: Some(vec![WorkspaceFolder {
                    uri: Url::from_directory_path(&root).unwrap(),
                    name: "workspace".into(),
                }]),
                ..Default::default()
            })
            .await?;
        server.initialized(InitializedParams {})?;

        Ok(Self {
            server,
            child,
            io_task,
        })
    }

    pub fn configure(&mut self, settings: serde_json::Value) -> Result<(), Error> {
        self.server
            .did_change_configuration(DidChangeConfigurationParams { settings })?;
        Ok(())
    }

    pub async fn open_document(&mut self, path: &Path, language_id: &str) -> Result<(), Error> {
        let path = path.canonicalize()?;
        let uri = file_uri(&path)?;

        let text = tokio::fs::read_to_string(&path).await?;

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

    pub async fn document_symbols(
        &mut self,
        path: &Path,
    ) -> Result<Option<DocumentSymbolResponse>, Error> {
        let uri = file_uri(&path.canonicalize()?)?;

        let symbols = self
            .server
            .document_symbol(DocumentSymbolParams {
                text_document: TextDocumentIdentifier { uri },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            })
            .await?;

        Ok(symbols)
    }

    pub async fn references(
        &mut self,
        uri: Url,
        position: Position,
    ) -> Result<Option<Vec<Location>>, Error> {
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
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            })
            .await?;
        Ok(references)
    }

    pub async fn shutdown(mut self) -> Result<(), Error> {
        self.server.shutdown(()).await?;
        self.server.exit(())?;

        let status = self.child.wait().await?;

        if !status.success() {
            return Err(format!("language server exited with {status}").into());
        }

        match self.io_task.await? {
            Ok(()) | Err(async_lsp::Error::Eof) => {}
            Err(error) => return Err(error.into()),
        }

        Ok(())
    }
}

pub fn file_uri(path: &Path) -> Result<Url, Error> {
    Url::from_file_path(path).map_err(|_| format!("invalid file path: {}", path.display()).into())
}
