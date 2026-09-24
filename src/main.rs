use clap::Parser;

mod cli;
mod project;
mod tree_parser;

fn main() {
    let cli = cli::Cli::parse();
    println!("path: {}", cli.path.display());

    if cli.verbose {
        println!("verbose enabled"); // TODO
    }
    let _ = project::parse_dir(&cli.path);
}