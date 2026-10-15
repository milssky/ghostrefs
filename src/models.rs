use std::path::PathBuf;


#[derive(Debug, PartialEq, Eq)]
pub enum SymbolKind {
    Function,
    Class,
    Method, //?
}


#[derive(Debug, PartialEq, Eq)]
pub enum ReferenceKind {
    Import,
    Call,
    Name,
}


#[derive(Debug, PartialEq, Eq)]
pub struct Position {
    pub row: usize,
    pub column: usize,
}


#[derive(Debug, PartialEq, Eq)]
pub struct SourceSpan {
    pub start_byte: usize,
    pub end_byte: usize,
    pub start: Position,
    pub end: Position,
}


#[derive(Debug, PartialEq, Eq)]
pub struct Reference {
    pub name: String,
    pub kind: ReferenceKind,
    pub span: SourceSpan,
}


#[derive(Debug, PartialEq, Eq)]
pub struct Symbol {
    pub name: String,
    pub qualname: String,
    pub kind: SymbolKind,
    pub span: SourceSpan, 
}


#[derive(Debug, PartialEq, Eq)]
pub struct ParsedFile {
    pub path: PathBuf,
    pub symbols: Vec<Symbol>,
    pub references: Vec<Reference>,}


#[derive(Debug, PartialEq, Eq)]
pub struct ParsedDir {
    pub path: PathBuf,
    pub files: Vec<ParsedFile>
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ScopeKind {
    Class,
    Function,
}


pub struct Scope {
    pub name: String,
    pub kind: ScopeKind,
}