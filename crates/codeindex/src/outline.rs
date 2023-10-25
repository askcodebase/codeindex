use tree_sitter::Node;

use crate::handlers::get_handler;

pub fn get_outline(node: Node, source_code: &str, extension: Option<&str>) -> Vec<String> {
    let mut signatures = Vec::new();

    if node.kind() == "source_file" {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child_kind = cursor.node().kind();

                // Lookup the handler for this kind of node
                if let Some(handler) = get_handler(child_kind, extension.unwrap_or("")) {
                    let signature = handler(&mut cursor, source_code);
                    signatures.push(signature);
                }

                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    for child in node.children(&mut node.walk()) {
        let mut child_signatures = get_outline(child, source_code, extension);
        signatures.append(&mut child_signatures);
    }

    signatures
}
