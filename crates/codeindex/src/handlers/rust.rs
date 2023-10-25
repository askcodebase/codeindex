use std::collections::HashMap;

use tree_sitter::TreeCursor;

// Handler for Rust function_item
fn handle_rs_function(cursor: &mut TreeCursor, source_code: &str) -> String {
    let mut function_signature = String::new();
    if cursor.goto_first_child() {
        loop {
            let node = cursor.node();
            match node.kind() {
                "identifier" => {
                    let start_byte = node.start_byte();
                    let end_byte = node.end_byte();
                    let child_name = &source_code[start_byte..end_byte];
                    function_signature.push_str(&format!("fn {}", child_name));
                }
                "parameters" => {
                    let start_byte = node.start_byte();
                    let end_byte = node.end_byte();
                    let parameters = &source_code[start_byte..end_byte];
                    function_signature.push_str(&format!("{}", parameters));
                }
                "type_identifier" => {
                    let start_byte = node.start_byte();
                    let end_byte = node.end_byte();
                    let return_type = &source_code[start_byte..end_byte];
                    function_signature.push_str(&format!(" -> {}", return_type));
                }
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

// Handler for Rust struct_item
fn handle_rs_struct(cursor: &mut TreeCursor, source_code: &str) -> String {
    let mut struct_signature = String::new();
    if cursor.goto_first_child() {
        loop {
            let node = cursor.node();
            if node.kind() == "identifier" {
                let start_byte = node.start_byte();
                let end_byte = node.end_byte();
                let child_name = &source_code[start_byte..end_byte];
                struct_signature.push_str(&format!("struct {} {{", child_name));
            }
            // You may want to handle fields here...

            if !cursor.goto_next_sibling() {
                break;
            }
        }
        cursor.goto_parent();
    }
    struct_signature.push_str("}");
    struct_signature
}

pub fn get_handlers() -> HashMap<&'static str, fn(&mut TreeCursor, &str) -> String> {
    let mut handlers: HashMap<&str, fn(&mut TreeCursor, &str) -> String> = HashMap::new();
    handlers.insert("function_item", handle_rs_function);
    handlers.insert("struct_item", handle_rs_struct);
    // Insert more handlers as needed
    handlers
}
