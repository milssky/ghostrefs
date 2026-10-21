use std::{
    path::{Path, PathBuf},
    time::Instant,
};

use async_lsp::lsp_types::{DocumentSymbol, DocumentSymbolResponse};

use crate::{
    graph,
    language::Language,
    lsp::LspSession,
    models::{FileAnalysis, SymbolAnalysis},
};

type Error = Box<dyn std::error::Error>;
#[derive(Clone, Copy)]
pub enum ReferenceMode<'a> {
    Graph,
    Report { source: &'a Path, tests: &'a Path },
}

pub fn run(
    root: &Path,
    files: &[PathBuf],
    settings: &serde_json::Value,
    mode: ReferenceMode<'_>,
    verbose: bool,
) -> Result<Vec<FileAnalysis>, Error> {
    let language = Language::detect(files)?;

    let config = language.lsp_config();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    runtime.block_on(async {
        let mut session = LspSession::start(root, config.command, config.args).await?;

        let analysis = collect_files(
            &mut session,
            files,
            config.language_id,
            settings,
            mode,
            verbose,
        )
        .await;
        let shutdown = session.shutdown().await;

        let results = analysis?;
        shutdown?;

        Ok(results)
    })
}

async fn collect_files(
    session: &mut LspSession,
    files: &[PathBuf],
    language_id: &str,
    settings: &serde_json::Value,
    mode: ReferenceMode<'_>,
    verbose: bool,
) -> Result<Vec<FileAnalysis>, Error> {
    let started = Instant::now();
    session.configure(settings.clone())?;
    // Open every document before requesting references across the workspace.
    for file in files {
        session.open_document(file, language_id).await?;
    }
    let mut results = Vec::new();
    for file in files {
        results.push(collect_file(session, file).await?);
    }
    let total: usize = results.iter().map(|file| file.symbols.len()).sum();
    if verbose {
        eprintln!(
            "Collected {total} symbols in {:.2}s",
            started.elapsed().as_secs_f64()
        );
    }
    collect_references(session, &mut results, mode, verbose).await?;
    Ok(results)
}

async fn collect_references(
    session: &mut LspSession,
    results: &mut [FileAnalysis],
    mode: ReferenceMode<'_>,
    verbose: bool,
) -> Result<(), Error> {
    let total: usize = results.iter().map(|file| file.symbols.len()).sum();
    let references_started = Instant::now();
    let mut completed = 0;
    let mut queried: Vec<Vec<bool>> = results
        .iter()
        .map(|file| vec![false; file.symbols.len()])
        .collect();
    loop {
        let needed = match mode {
            ReferenceMode::Graph => results
                .iter()
                .map(|file| vec![true; file.symbols.len()])
                .collect(),
            ReferenceMode::Report { source, tests } => {
                graph::reference_targets(results, source, tests)
            }
        };
        let count = needed
            .iter()
            .zip(&queried)
            .flat_map(|(needed, queried)| needed.iter().zip(queried))
            .filter(|(needed, queried)| **needed && !**queried)
            .count();
        if count == 0 {
            break;
        }
        let requested = completed + count;
        if verbose {
            eprintln!(
                "Requesting {count} references ({completed} completed, {total} total symbols)"
            );
        }
        for ((file, needed), queried) in results.iter_mut().zip(needed).zip(&mut queried) {
            let uri = crate::lsp::file_uri(&file.path)?;
            for ((symbol, needed), queried) in file.symbols.iter_mut().zip(needed).zip(queried) {
                if !needed || *queried {
                    continue;
                }
                symbol.references = session
                    .references(uri.clone(), symbol.selection_range.start)
                    .await?;
                *queried = true;
                completed += 1;
                if verbose && (completed.is_multiple_of(500) || completed == requested) {
                    eprintln!(
                        "References: {completed}/{requested} in {:.2}s",
                        references_started.elapsed().as_secs_f64()
                    );
                }
            }
        }
    }
    for (file, queried) in results.iter_mut().zip(queried) {
        for (symbol, queried) in file.symbols.iter_mut().zip(queried) {
            if !queried {
                symbol.references = Some(Vec::new());
            }
        }
    }
    if verbose {
        eprintln!(
            "Collected {completed}/{total} reference results in {:.2}s",
            references_started.elapsed().as_secs_f64()
        );
    }
    Ok(())
}

async fn collect_file(session: &mut LspSession, path: &Path) -> Result<FileAnalysis, Error> {
    let declarations = match session.document_symbols(path).await? {
        Some(DocumentSymbolResponse::Nested(declarations)) => declarations,
        Some(DocumentSymbolResponse::Flat(declarations)) if declarations.is_empty() => Vec::new(),
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
    let symbols = flatten_symbols(declarations);
    Ok(FileAnalysis {
        path: path.to_path_buf(),
        symbols,
    })
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
