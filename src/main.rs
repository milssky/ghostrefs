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
    let tests_tree = scan::parse_dir(&cli.tests);
    let project_tree = scan::parse_dir(&cli.source);
    let _ = dbg!(tests_tree);
    let _ = dbg!(project_tree);

    let _ = lsp::run(std::path::Path::new("fixtures/python"));
}
