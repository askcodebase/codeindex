use std::collections::HashMap;

use tree_sitter::TreeCursor;

// Handler for Python function_definition
fn handle_py_function(cursor: &mut TreeCursor, source_code: &str) -> String {
    let mut function_signature = String::new();
    if cursor.goto_first_child() {
        loop {
            let node = cursor.node();
            match node.kind() {
                "identifier" => {
                    let start_byte = node.start_byte();
                    let end_byte = node.end_byte();
                    let child_name = &source_code[start_byte..end_byte];
                    function_signature.push_str(&format!("def {}", child_name));
                }
                "parameters" => {
                    let start_byte = node.start_byte();
                    let end_byte = node.end_byte();
                    let parameters = &source_code[start_byte..end_byte];
                    function_signature.push_str(&format!("{}", parameters));
                }
                // Python doesn't have explicit return types, so we don't handle that here
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

// Handler for Python class_definition
fn handle_py_class(cursor: &mut TreeCursor, source_code: &str) -> String {
    let mut class_signature = String::new();
    if cursor.goto_first_child() {
        loop {
            let node = cursor.node();
            if node.kind() == "identifier" {
                let start_byte = node.start_byte();
                let end_byte = node.end_byte();
                let child_name = &source_code[start_byte..end_byte];
                class_signature.push_str(&format!("class {}:", child_name));
            }
            // You may want to handle fields, methods, etc. here...

            if !cursor.goto_next_sibling() {
                break;
            }
        }
        cursor.goto_parent();
    }
    class_signature
}

pub fn get_handlers() -> HashMap<&'static str, fn(&mut TreeCursor, &str) -> String> {
    let mut handlers: HashMap<&str, fn(&mut TreeCursor, &str) -> String> = HashMap::new();
    handlers.insert("function_definition", handle_py_function);
    handlers.insert("class_definition", handle_py_class);
    // Insert more handlers as needed
    handlers
}
