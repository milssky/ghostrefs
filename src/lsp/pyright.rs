//! Pyright process configuration and client capability workarounds.

use async_lsp::lsp_types::{DiagnosticClientCapabilities, TextDocumentClientCapabilities};

use super::server::ServerDefinition;

pub fn definition() -> ServerDefinition {
    ServerDefinition {
        command: "pyright-langserver",
        args: &["--stdio"],
        capabilities: TextDocumentClientCapabilities {
            // Pull diagnostics prevent background type-checking of every opened file.
            diagnostic: Some(DiagnosticClientCapabilities {
                dynamic_registration: Some(true),
                ..DiagnosticClientCapabilities::default()
            }),
            ..TextDocumentClientCapabilities::default()
        },
    }
}
