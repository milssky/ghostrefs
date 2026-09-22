use std::path::PathBuf;
use clap::Parser;


#[derive(Parser)]
#[command(version, about)]
pub struct Cli {
    #[arg(short, long)]
    pub path: PathBuf,

    #[arg(short, long)]
    pub verbose: bool,
}