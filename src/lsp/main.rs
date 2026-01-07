use tower_lsp::{LspService, Server};

mod backend;
mod completion;
mod diagnostics;
mod formatting;
mod tailwind;
mod workspace;

use backend::TopoBackend;

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| TopoBackend::new(client));
    Server::new(stdin, stdout, socket).serve(service).await;
}
