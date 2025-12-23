//! SchemaRefly Language Server
//!
//! This is the main entry point for the SchemaRefly LSP server.
//! It starts the server and handles stdin/stdout communication with the editor.

use schemarefly_lsp::Backend;
use tower_lsp::{LspService, Server};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[tokio::main]
async fn main() {
    // Initialize tracing for logging
    tracing_subscriber::registry()
        .with(fmt::layer().with_writer(std::io::stderr))
        .with(EnvFilter::from_default_env())
        .init();

    tracing::info!("Starting SchemaRefly LSP server");

    // Create LSP service
    let (stdin, stdout) = (tokio::io::stdin(), tokio::io::stdout());
    let (service, socket) = LspService::new(|client| Backend::new(client));

    // Start the server
    Server::new(stdin, stdout, socket).serve(service).await;

    tracing::info!("SchemaRefly LSP server stopped");
}
