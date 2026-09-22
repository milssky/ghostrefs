use std::fs;
use std::path::Path;

use tree_sitter::Parser;



pub fn parse_file(path: &Path) {
    let source = fs::read_to_string(path).expect("failed to read file");
    let mut parser = Parser::new();

    parser
        .set_language(&tree_sitter_python::LANGUAGE.into())
        .expect("failed to load Python grammar");

    let tree = parser
        .parse(&source, None)
        .expect("failed to parse");

    let root = tree.root_node();
    let mut cursor = root.walk();

    for child in root.children(&mut cursor) {
        let text = child
            .utf8_text(source.as_bytes())
            .expect("node contains invalid UTF-8");

        println!("kind: {}", child.kind());
        println!("range: {:?}", child.range());
        println!("text:\n{text}");
        println!("---");
    }
}