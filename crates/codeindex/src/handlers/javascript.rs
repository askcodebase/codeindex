use std::collections::HashMap;

use tree_sitter::TreeCursor;

// Handler for JavaScript function_declaration
fn handle_js_function(cursor: &mut TreeCursor, source_code: &str) -> String {
    let mut function_signature = String::new();
    if cursor.goto_first_child() {
        loop {
            let node = cursor.node();
            match node.kind() {
                "identifier" => {
                    let start_byte = node.start_byte();
                    let end_byte = node.end_byte();
                    let child_name = &source_code[start_byte..end_byte];
                    function_signature.push_str(&format!("function {}", child_name));
                }
                "formal_parameters" => {
                    let start_byte = node.start_byte();
                    let end_byte = node.end_byte();
                    let parameters = &source_code[start_byte..end_byte];
                    function_signature.push_str(&format!("{}", parameters));
                }
                // JavaScript doesn't have explicit return types, so we don't handle that here
                _ => {}
            }

            if !cursor.goto_next_sibling() {
                break;
            }
        }
        cursor.goto_parent();
    }
    function_signature
}

pub fn get_handlers() -> HashMap<&'static str, fn(&mut TreeCursor, &str) -> String> {
    let mut handlers: HashMap<&str, fn(&mut TreeCursor, &str) -> String> = HashMap::new();
    handlers.insert("function_declaration", handle_js_function);
    // Insert more handlers as needed
    handlers
}
