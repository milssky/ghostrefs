use clap::Parser;

mod cli;
mod language;
mod lsp;
mod models;
mod scan;

fn main() {
    let cli = cli::Cli::parse()
        .resolve_paths()
        .expect("Invalid project paths");
    println!("path: {}", cli.tests.display());

    if cli.verbose {
        println!("verbose enabled"); // TODO
    }
    let tests_tree = scan::parse_dir(&cli.tests).expect("failed to parse tests");

    let project_tree = scan::parse_dir(&cli.source).expect("failed to parse source");

    let mut files: Vec<_> = project_tree
        .files
        .iter()
        .chain(tests_tree.files.iter())
        .map(|file| {
            file.path
                .canonicalize()
                .expect("failed to resolve file path")
        })
        .collect();

    files.sort();
    files.dedup();

    let lsp_result = lsp::run(&cli.root, &files);
    let _ = dbg!(lsp_result);
}
