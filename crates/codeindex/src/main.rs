mod welcome;

use qdrant::start_qdrant;
use welcome::welcome;

/// CodeIndex is a local-first high performance codebase index engine designed for AI.
/// It helps your LLM understand the structure and semantics of a codebase and grab code
/// context when needed.
///
/// This CLI starts a CodeIndex peer/server.
fn main() {
    welcome();

    let _ = start_qdrant();
    log::info!("CodeIndex server stopped");
}
