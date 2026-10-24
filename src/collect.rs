#![expect(clippy::print_stderr, reason = "CLI progress is written to stderr")]
#![expect(
    clippy::future_not_send,
    reason = "LSP work runs on a current-thread runtime"
)]

use std::{
    path::{Path, PathBuf},
    time::Instant,
};

use serde_json::Value;
use tokio::runtime::Builder;

use crate::{
    Result, graph,
    language::{self, Language},
    lsp::{LspSession, file_uri},
    models::FileAnalysis,
};

#[derive(Clone, Copy)]
pub enum ReferenceMode<'source> {
    Graph,
    Report {
        source: &'source Path,
        tests: &'source Path,
    },
}

pub fn run(
    root: &Path,
    files: &[PathBuf],
    lsp_settings: &Value,
    mode: ReferenceMode<'_>,
) -> Result<Vec<FileAnalysis>> {
    let runtime = Builder::new_current_thread().enable_all().build()?;
    runtime.block_on(async {
        let language = language::detect(files)?;
        let mut session = start_session(root, files, language, lsp_settings).await?;
        let collected = collect_files(&mut session, files, language, mode).await;
        let shutdown = session.shutdown().await;
        let analyses = collected?;
        shutdown?;
        Ok(analyses)
    })
}

async fn collect_files(
    session: &mut LspSession,
    files: &[PathBuf],
    language: &dyn Language,
    mode: ReferenceMode<'_>,
) -> Result<Vec<FileAnalysis>> {
    let started = Instant::now();
    let mut analyses = Vec::new();
    for file in files {
        analyses.push(collect_file(session, file, language).await?);
    }
    let total: usize = analyses.iter().map(|file| file.symbols().len()).sum();
    eprintln!(
        "Collected {total} symbols in {:.2}s",
        started.elapsed().as_secs_f64()
    );
    collect_references(session, &mut analyses, mode).await?;
    Ok(analyses)
}

async fn collect_references(
    session: &mut LspSession,
    analyses: &mut [FileAnalysis],
    mode: ReferenceMode<'_>,
) -> Result<()> {
    let mut progress = ReferenceProgress::new(analyses);
    let mut queried = query_flags(analyses);
    loop {
        let needed = needed_references(analyses, mode);
        let pending = pending_references(&needed, &queried);
        if pending == 0 {
            break;
        }
        progress.requesting(pending);
        collect_reference_round(session, analyses, needed, &mut queried).await?;
        progress.completed = progress.completed.saturating_add(pending);
        progress.report_round();
    }
    mark_unqueried(analyses, queried);
    progress.finish();
    Ok(())
}

fn needed_references(analyses: &[FileAnalysis], mode: ReferenceMode<'_>) -> Vec<Vec<bool>> {
    match mode {
        ReferenceMode::Graph => analyses
            .iter()
            .map(|file| vec![true; file.symbols().len()])
            .collect(),
        ReferenceMode::Report { source, tests } => {
            graph::reference_targets(analyses, source, tests)
        }
    }
}

fn pending_references(needed: &[Vec<bool>], queried: &[Vec<bool>]) -> usize {
    needed
        .iter()
        .flatten()
        .zip(queried.iter().flatten())
        .filter(|(is_needed, was_queried)| **is_needed && !**was_queried)
        .count()
}

async fn collect_reference_round(
    session: &mut LspSession,
    analyses: &mut [FileAnalysis],
    needed: Vec<Vec<bool>>,
    queried: &mut [Vec<bool>],
) -> Result<()> {
    for ((file, file_needed), file_queried) in analyses.iter_mut().zip(needed).zip(queried) {
        collect_file_references(session, file, file_needed, file_queried).await?;
    }
    Ok(())
}

async fn collect_file_references(
    session: &mut LspSession,
    file: &mut FileAnalysis,
    needed: Vec<bool>,
    queried: &mut [bool],
) -> Result<()> {
    let uri = file_uri(file.path())?;
    for (((position, references), is_needed), was_queried) in
        file.reference_slots_mut().zip(needed).zip(queried)
    {
        if is_needed && !*was_queried {
            *references = session.references(uri.clone(), position).await?;
            *was_queried = true;
        }
    }
    Ok(())
}

fn mark_unqueried(analyses: &mut [FileAnalysis], queried: Vec<Vec<bool>>) {
    for (file, file_queried) in analyses.iter_mut().zip(queried) {
        for ((_, references), was_queried) in file.reference_slots_mut().zip(file_queried) {
            if !was_queried {
                *references = Some(Vec::new());
            }
        }
    }
}

struct ReferenceProgress {
    total: usize,
    completed: usize,
    started: Instant,
}

impl ReferenceProgress {
    fn new(analyses: &[FileAnalysis]) -> Self {
        Self {
            total: analyses.iter().map(|file| file.symbols().len()).sum(),
            completed: 0,
            started: Instant::now(),
        }
    }

    fn requesting(&self, count: usize) {
        eprintln!(
            "Requesting {count} references ({} completed, {} total symbols)",
            self.completed, self.total
        );
    }

    fn report_round(&self) {
        eprintln!(
            "References: {}/{} in {:.2}s",
            self.completed,
            self.total,
            self.started.elapsed().as_secs_f64()
        );
    }

    fn finish(&self) {
        eprintln!(
            "Collected {}/{} reference results in {:.2}s",
            self.completed,
            self.total,
            self.started.elapsed().as_secs_f64()
        );
    }
}

async fn collect_file(
    session: &mut LspSession,
    path: &Path,
    language: &dyn Language,
) -> Result<FileAnalysis> {
    language.normalize_symbols(session.document_symbols(path).await?, path)
}

fn query_flags(analyses: &[FileAnalysis]) -> Vec<Vec<bool>> {
    analyses
        .iter()
        .map(|file| vec![false; file.symbols().len()])
        .collect()
}

async fn start_session(
    root: &Path,
    files: &[PathBuf],
    language: &dyn Language,
    lsp_settings: &Value,
) -> Result<LspSession> {
    let server = language.default_server();
    let mut session = LspSession::start(root, server).await?;
    session.configure(lsp_settings.clone())?;
    for file in files {
        session.open_document(file, language.language_id()).await?;
    }
    Ok(session)
}
