use std::{
    ops::ControlFlow,
    path::{Path, PathBuf},
    process::Stdio,
};

use async_lsp::{
    LanguageServer, MainLoop, ServerSocket,
    lsp_types::{
        DidChangeConfigurationParams, DidOpenTextDocumentParams, DocumentSymbolParams,
        DocumentSymbolResponse, InitializeParams, InitializedParams, TextDocumentIdentifier,
        TextDocumentItem, Url, WorkspaceFolder,
    },
    router::Router,
};
use tokio::{process::Child, process::Command, task::JoinHandle};
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

use crate::language::Language;

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
        let uri = Url::from_file_path(&path)
            .map_err(|_| format!("invalid file path: {}", path.display()))?;

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
        let path = path.canonicalize()?;
        let uri = Url::from_file_path(&path)
            .map_err(|_| format!("invalid file path: {}", path.display()))?;

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

#[derive(Debug)]
#[expect(dead_code, reason = "We need it now. Temp.")]
pub struct FileSymbols {
    pub path: PathBuf,
    pub symbols: Option<DocumentSymbolResponse>,
}

pub fn run(root: &Path, files: &[PathBuf]) -> Result<Vec<FileSymbols>, Error> {
    let language = Language::detect(files)?;

    let config = language.lsp_config()?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    runtime.block_on(async {
        let mut session = LspSession::start(root, config.command, config.args).await?;

        let analysis: Result<Vec<FileSymbols>, Error> = async {
            session.configure(config.settings)?;

            for file in files {
                session.open_document(file, config.language_id).await?;
            }

            let mut results = Vec::new();

            for file in files {
                let symbols = session.document_symbols(file).await?;

                results.push(FileSymbols {
                    path: file.clone(),
                    symbols,
                });
            }

            Ok(results)
        }
        .await;

        // Пытаемся завершить сервер и при ошибке анализа.
        let shutdown = session.shutdown().await;

        let results = analysis?;
        shutdown?;

        Ok(results)
    })
}
