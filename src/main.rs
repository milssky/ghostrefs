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
    let _ = project::parse_dir(&cli.tests);
}