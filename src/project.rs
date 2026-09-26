use std::fs;
use std::io;
use std::path::Path;

use crate::tree_parser::parse_file;


// TODO: move to other crate?
pub fn parse_dir(path: &Path) -> io::Result<()> {
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            parse_dir(&path)?;
        } else {
            println!("{}", path.display());
            let file = parse_file(&path);
            println!("{:#?}", file);
        }
    }
    Ok(())
}