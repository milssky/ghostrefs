
use std::{ops::ControlFlow, path::Path, process::Stdio};

use async_lsp::{
    LanguageServer, MainLoop, lsp_types::{
        InitializeParams, InitializeResult, InitializedParams, Url, WorkspaceFolder,
        DidOpenTextDocumentParams, DocumentSymbolParams, TextDocumentIdentifier, TextDocumentItem,
    }, router::Router,
};
use tokio::{process::Command};
use tokio_util::compat::{
    TokioAsyncReadCompatExt,
    TokioAsyncWriteCompatExt,
};

pub fn run(
    root: &Path
) -> Result<InitializeResult, Box<dyn std::error::Error>> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    runtime.block_on(async {
        let root = root.canonicalize()?;
        let mut child = Command::new("pyright-langserver")
            .arg("--stdio")
            .current_dir(&root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()?;

        let stdout = child.stdout.take().unwrap();
        let stdin = child.stdin.take().unwrap();

        let (mainloop, mut server) = MainLoop::new_client(|_| {
            let mut router = Router::new(());
            router.unhandled_notification(|_,_| {
                ControlFlow::Continue(())
            });
            router
        });

        let io_task = tokio::spawn(async move {
            mainloop
                .run_buffered(stdout.compat(), stdin.compat_write())
                .await
        });

        let result = server
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
        // --------------
        let file = root.join("project/main.py");
        let uri = Url::from_file_path(&file).unwrap();
        let text = std::fs::read_to_string(&file)?;

        server.did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "python".into(),
                version: 1,
                text,
            },
        })?;

        let symbols = server
            .document_symbol(DocumentSymbolParams {
                text_document: TextDocumentIdentifier { uri },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            })
            .await?;

        dbg!(&symbols);
        // --------------
        server.shutdown(()).await?;
        server.exit(())?;

        let status = child.wait().await?;
        if !status.success() {
            return Err(format!("Server exit status is {status}").into());
        }
        match io_task.await? {
            Ok(()) | Err(async_lsp::Error::Eof) => {}
            Err(error) => return Err(error.into()),
        }

        Ok(result)
    })
}