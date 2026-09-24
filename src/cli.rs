use std::path::PathBuf;
use clap::Parser;


#[derive(Parser)]
#[command(version, about)]
pub struct Cli {
    /// Set path to tests
    #[arg(short, long, value_parser = parse_directory)]
    pub path: PathBuf,

    /// Verbose level
    #[arg(short, long)]
    pub verbose: bool,
}


fn parse_directory(path_value: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(path_value);

    if !path.exists() {
        return Err(format!("path does not exist: {}", path.display()));
    }
    if !path.is_dir() {
        return Err(format!("path is not a directory: {}", path.display()));
    }

    Ok(path)
}