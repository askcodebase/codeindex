use tree_sitter::Node;

use crate::handlers;

pub fn get_outline(node: Node, source_code: &str, extension: Option<&str>) -> Vec<String> {
    let mut cursor = node.walk();
    let mut signatures = Vec::new();

    // Get the handlers for this language
    let handlers = handlers::get_handlers(extension.unwrap_or(""));

    if cursor.goto_first_child() {
        loop {
            let child_node = cursor.node();
            let child_kind = child_node.kind();

            // Lookup the handler for this kind of node
            if let Some(handler) = handlers.get(child_kind) {
                let signature = handler(&mut cursor, source_code);
                signatures.push(signature);
            }

            // If the node has no children or we're done processing the children,
            // we move on to the next sibling.
            if !cursor.goto_first_child() {
                while !cursor.goto_next_sibling() {
                    if !cursor.goto_parent() {
                        return signatures;
                    }
                }
            }
        }
    }
    signatures
}
