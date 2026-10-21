use std::{
    ops::ControlFlow,
    path::{Path, PathBuf},
    process::Stdio,
};

use async_lsp::{
    LanguageServer, MainLoop, ServerSocket,
    lsp_types::{
        ClientCapabilities, DidChangeConfigurationParams, DidOpenTextDocumentParams,
        DocumentSymbol, DocumentSymbolClientCapabilities, DocumentSymbolParams,
        DocumentSymbolResponse, InitializeParams, InitializedParams, Location, Position,
        ReferenceContext, ReferenceParams, TextDocumentClientCapabilities, TextDocumentIdentifier,
        TextDocumentItem, TextDocumentPositionParams, Url, WorkspaceFolder,
    },
    router::Router,
};
use tokio::{process::Child, process::Command, task::JoinHandle};
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

use crate::language::Language;
use crate::models::{FileAnalysis, SymbolAnalysis};

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
        path: &Path,
        position: Position,
    ) -> Result<Option<Vec<Location>>, Error> {
        let uri = file_uri(&path.canonicalize()?)?;

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

    async fn collect_files(
        &mut self,
        files: &[PathBuf],
        language_id: &str,
        settings: &serde_json::Value,
    ) -> Result<Vec<FileAnalysis>, Error> {
        self.configure(settings.clone())?;
        // Open every document before requesting references across the workspace.
        for file in files {
            self.open_document(file, language_id).await?;
        }
        let mut results = Vec::new();
        for file in files {
            results.push(self.collect_file(file).await?);
        }
        Ok(results)
    }

    async fn collect_file(&mut self, path: &Path) -> Result<FileAnalysis, Error> {
        let declarations = match self.document_symbols(path).await? {
            Some(DocumentSymbolResponse::Nested(declarations)) => declarations,
            Some(DocumentSymbolResponse::Flat(declarations)) if declarations.is_empty() => {
                Vec::new()
            }
            Some(DocumentSymbolResponse::Flat(_)) => {
                return Err(format!(
                    "expected hierarchical document symbols for {}",
                    path.display()
                )
                .into());
            }
            None => {
                return Err(format!(
                    "language server returned no document-symbol result for {}",
                    path.display()
                )
                .into());
            }
        };
        let mut symbols = flatten_symbols(declarations);
        for symbol in &mut symbols {
            symbol.references = self.references(path, symbol.selection_range.start).await?;
        }
        Ok(FileAnalysis {
            path: path.to_path_buf(),
            symbols,
        })
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

pub fn run(
    root: &Path,
    files: &[PathBuf],
    settings: &serde_json::Value,
) -> Result<Vec<FileAnalysis>, Error> {
    let language = Language::detect(files)?;

    let config = language.lsp_config();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    runtime.block_on(async {
        let mut session = LspSession::start(root, config.command, config.args).await?;

        let analysis = session
            .collect_files(files, config.language_id, settings)
            .await;
        let shutdown = session.shutdown().await;

        let results = analysis?;
        shutdown?;

        Ok(results)
    })
}

fn file_uri(path: &Path) -> Result<Url, Error> {
    Url::from_file_path(path).map_err(|_| format!("invalid file path: {}", path.display()).into())
}

/// Preserve declaration order while flattening nested names in depth-first order.
fn flatten_symbols(declarations: Vec<DocumentSymbol>) -> Vec<SymbolAnalysis> {
    let mut pending: Vec<_> = declarations
        .into_iter()
        .rev()
        .map(|symbol| (symbol, String::new()))
        .collect();
    let mut symbols = Vec::new();
    while let Some((symbol, parent)) = pending.pop() {
        let qualname = if parent.is_empty() {
            symbol.name.clone()
        } else {
            format!("{parent}.{}", symbol.name)
        };
        if let Some(children) = symbol.children {
            pending.extend(
                children
                    .into_iter()
                    .rev()
                    .map(|child| (child, qualname.clone())),
            );
        }
        symbols.push(SymbolAnalysis {
            name: symbol.name,
            qualname,
            kind: symbol.kind,
            range: symbol.range,
            selection_range: symbol.selection_range,
            references: None,
        });
    }
    symbols
}
