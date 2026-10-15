use clap::Parser;

mod cli;
mod project;
mod tree_parser;
mod models;

fn main() {
    let cli = cli::Cli::parse();
    println!("path: {}", cli.tests.display());

    if cli.verbose {
        println!("verbose enabled"); // TODO
    }
    let tests_tree = project::parse_dir(&cli.tests);
    let project_tree = project::parse_dir(&cli.source);
    let _ = dbg!(tests_tree);
    let _ = dbg!(project_tree);
}