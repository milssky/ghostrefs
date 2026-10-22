#![expect(clippy::print_stderr, reason = "CLI progress is written to stderr")]
#![expect(
    clippy::future_not_send,
    reason = "LSP work runs on a current-thread runtime"
)]

use crate::Result;
use std::{
    path::{Path, PathBuf},
    time::Instant,
};

use crate::{
    cli::{Cli, Verbosity},
    lsp::{LspSession, References, file_uri},
};
use async_lsp::lsp_types::{DocumentSymbol, DocumentSymbolResponse, Position, Url};
use serde_json::Value;
use tokio::runtime::Builder;

use crate::{
    graph,
    language::Language,
    models::{FileAnalysis, SymbolAnalysis},
};

#[derive(Clone, Copy)]
pub(crate) enum ReferenceMode<'source> {
    Graph,
    Report {
        source: &'source Path,
        tests: &'source Path,
    },
}

pub(crate) fn run(
    cli: &Cli,
    files: &[PathBuf],
    settings: &Value,
    mode: ReferenceMode<'_>,
) -> Result<Vec<FileAnalysis>> {
    let runtime = Builder::new_current_thread().enable_all().build()?;
    runtime.block_on(async {
        let mut session = start_session(&cli.root, files, settings).await?;
        let collected = collect_files(&mut session, files, mode, cli.verbosity()).await;
        let shutdown = session.shutdown().await;
        let analyses = collected?;
        shutdown?;
        Ok(analyses)
    })
}

async fn collect_files(
    session: &mut LspSession,
    files: &[PathBuf],
    mode: ReferenceMode<'_>,
    verbosity: Verbosity,
) -> Result<Vec<FileAnalysis>> {
    let started = Instant::now();
    let mut analyses = Vec::new();
    for file in files {
        analyses.push(collect_file(session, file).await?);
    }
    let total: usize = analyses.iter().map(|file| file.symbols.len()).sum();
    if verbosity.is_verbose() {
        eprintln!(
            "Collected {total} symbols in {:.2}s",
            started.elapsed().as_secs_f64()
        );
    }
    collect_references(session, &mut analyses, mode, verbosity).await?;
    Ok(analyses)
}

async fn collect_references(
    session: &mut LspSession,
    analyses: &mut [FileAnalysis],
    mode: ReferenceMode<'_>,
    verbosity: Verbosity,
) -> Result<()> {
    let mut progress = ReferenceProgress::new(analyses, verbosity);
    let mut queried = query_flags(analyses);
    loop {
        let needed = needed_references(analyses, mode);
        let (requests, targets) = reference_requests(analyses, needed, &mut queried)?;
        if requests.is_empty() {
            break;
        }
        progress.requesting(requests.len());
        let responses = collect_reference_batch(session, requests).await?;
        progress.completed = progress.completed.saturating_add(responses.len());
        store_references(analyses, targets, responses)?;
        progress.report_batch();
    }
    mark_unqueried(analyses, queried);
    progress.finish();
    Ok(())
}

fn needed_references(analyses: &[FileAnalysis], mode: ReferenceMode<'_>) -> Vec<Vec<bool>> {
    match mode {
        ReferenceMode::Graph => analyses
            .iter()
            .map(|file| vec![true; file.symbols.len()])
            .collect(),
        ReferenceMode::Report { source, tests } => {
            graph::reference_targets(analyses, source, tests)
        }
    }
}

type SymbolTargets = Vec<(usize, usize)>;
type ReferencePositions = Vec<(Url, Position)>;
type ReferenceRequests = (ReferencePositions, SymbolTargets);

#[expect(
    clippy::too_many_lines,
    reason = "Keep requests and their matching symbol indices together"
)]
fn reference_requests(
    analyses: &[FileAnalysis],
    needed: Vec<Vec<bool>>,
    queried: &mut [Vec<bool>],
) -> Result<ReferenceRequests> {
    let mut requests = Vec::new();
    let mut targets = Vec::new();
    for (file_index, ((file, file_needed), file_queried)) in
        analyses.iter().zip(needed).zip(queried).enumerate()
    {
        let uri = file_uri(&file.path)?;
        for (symbol_index, ((symbol, is_needed), was_queried)) in file
            .symbols
            .iter()
            .zip(file_needed)
            .zip(file_queried)
            .enumerate()
        {
            if is_needed && !*was_queried {
                *was_queried = true;
                requests.push((uri.clone(), symbol.selection_range.start));
                targets.push((file_index, symbol_index));
            }
        }
    }
    Ok((requests, targets))
}

fn store_references(
    analyses: &mut [FileAnalysis],
    targets: SymbolTargets,
    responses: Vec<References>,
) -> Result<()> {
    for ((file, symbol), references) in targets.into_iter().zip(responses) {
        analyses
            .get_mut(file)
            .and_then(|analysis| analysis.symbols.get_mut(symbol))
            .ok_or("reference target is missing from collected symbols")?
            .references = references;
    }
    Ok(())
}

fn mark_unqueried(analyses: &mut [FileAnalysis], queried: Vec<Vec<bool>>) {
    for (file, file_queried) in analyses.iter_mut().zip(queried) {
        for (symbol, was_queried) in file.symbols.iter_mut().zip(file_queried) {
            if !was_queried {
                symbol.references = Some(Vec::new());
            }
        }
    }
}

struct ReferenceProgress {
    total: usize,
    completed: usize,
    started: Instant,
    verbosity: Verbosity,
}

impl ReferenceProgress {
    fn new(analyses: &[FileAnalysis], verbosity: Verbosity) -> Self {
        Self {
            total: analyses.iter().map(|file| file.symbols.len()).sum(),
            completed: 0,
            started: Instant::now(),
            verbosity,
        }
    }

    fn requesting(&self, count: usize) {
        if self.verbosity.is_verbose() {
            eprintln!(
                "Requesting {count} references ({} completed, {} total symbols)",
                self.completed, self.total
            );
        }
    }

    fn report_batch(&self) {
        if self.verbosity.is_verbose() {
            eprintln!(
                "References: {}/{} in {:.2}s",
                self.completed,
                self.total,
                self.started.elapsed().as_secs_f64()
            );
        }
    }

    fn finish(&self) {
        if self.verbosity.is_verbose() {
            eprintln!(
                "Collected {}/{} reference results in {:.2}s",
                self.completed,
                self.total,
                self.started.elapsed().as_secs_f64()
            );
        }
    }
}

async fn collect_file(session: &mut LspSession, path: &Path) -> Result<FileAnalysis> {
    let declarations = nested_declarations(session.document_symbols(path).await?, path)?;
    Ok(FileAnalysis {
        path: path.to_path_buf(),
        symbols: flatten_symbols(declarations),
    })
}

fn nested_declarations(
    response: Option<DocumentSymbolResponse>,
    path: &Path,
) -> Result<Vec<DocumentSymbol>> {
    match response {
        Some(DocumentSymbolResponse::Nested(declarations)) => Ok(declarations),
        Some(DocumentSymbolResponse::Flat(declarations)) if declarations.is_empty() => {
            Ok(Vec::new())
        }
        Some(DocumentSymbolResponse::Flat(_)) => Err(format!(
            "expected hierarchical document symbols for {}",
            path.display()
        )
        .into()),
        None => Err(format!(
            "language server returned no document-symbol result for {}",
            path.display()
        )
        .into()),
    }
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
        let flattened = flatten_symbol(&symbol, &parent);
        if let Some(children) = symbol.children {
            pending.extend(
                children
                    .into_iter()
                    .rev()
                    .map(|child| (child, flattened.qualname.clone())),
            );
        }
        symbols.push(flattened);
    }
    symbols
}

fn flatten_symbol(symbol: &DocumentSymbol, parent: &str) -> SymbolAnalysis {
    let qualname = if parent.is_empty() {
        symbol.name.clone()
    } else {
        format!("{parent}.{}", symbol.name)
    };
    SymbolAnalysis {
        name: symbol.name.clone(),
        qualname,
        kind: symbol.kind,
        range: symbol.range,
        selection_range: symbol.selection_range,
        references: None,
    }
}

fn query_flags(analyses: &[FileAnalysis]) -> Vec<Vec<bool>> {
    analyses
        .iter()
        .map(|file| vec![false; file.symbols.len()])
        .collect()
}

async fn start_session(root: &Path, files: &[PathBuf], settings: &Value) -> Result<LspSession> {
    let config = Language::detect(files)?.lsp_config();
    let mut session = LspSession::start(root, config.command, config.args).await?;
    session.configure(settings.clone())?;
    for file in files {
        session.open_document(file, config.language_id).await?;
    }
    Ok(session)
}

async fn collect_reference_batch(
    session: &mut LspSession,
    requests: ReferencePositions,
) -> Result<Vec<References>> {
    let mut responses = Vec::with_capacity(requests.len());
    for (uri, position) in requests {
        responses.push(session.references(uri, position).await?);
    }
    Ok(responses)
}
