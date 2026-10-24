//! Server selection and launch configuration, separate from language semantics.

use async_lsp::lsp_types::TextDocumentClientCapabilities;

use super::pyright;

#[derive(Debug, Clone, Copy)]
pub enum Server {
    Pyright,
}

pub struct ServerDefinition {
    pub command: &'static str,
    pub args: &'static [&'static str],
    pub capabilities: TextDocumentClientCapabilities,
}

impl Server {
    pub fn definition(self) -> ServerDefinition {
        match self {
            Self::Pyright => pyright::definition(),
        }
    }
}
