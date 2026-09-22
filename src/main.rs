use clap::Parser;

mod cli;
mod project;


fn main() {
    let cli = cli::Cli::parse();
    println!("path: {}", cli.path.display());

    if cli.verbose {
        println!("verbose enabled"); // TODO
    }
    project::parse_file(&cli.path);
}