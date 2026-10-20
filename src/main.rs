use clap::Parser;

mod cli;
mod language;
mod lsp;
mod scan;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = cli::Cli::parse().resolve_paths()?;
    let mut files = scan::scan_dir(&cli.source)?;
    files.extend(scan::scan_dir(&cli.tests)?);
    files.sort();
    files.dedup();

    if cli.verbose {
        eprintln!("Collecting LSP symbols from {} files", files.len());
    }

    for file in lsp::run(&cli.root, &files)? {
        println!("{}: {:#?}", file.path.display(), file.symbols);
    }

    Ok(())
}
