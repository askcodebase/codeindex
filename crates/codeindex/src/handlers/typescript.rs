use std::collections::HashMap;

use tree_sitter::TreeCursor;

// Handler for TypeScript function_declaration
fn handle_ts_function(cursor: &mut tree_sitter::TreeCursor, source_code: &str) -> String {
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
                "type_annotation" => {
                    let start_byte = node.start_byte();
                    let end_byte = node.end_byte();
                    let return_type = &source_code[start_byte..end_byte];
                    function_signature.push_str(&format!(": {}", return_type));
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

// Handler for TypeScript class_declaration
fn handle_ts_class(cursor: &mut TreeCursor, source_code: &str) -> String {
    let mut class_signature = String::new();
    if cursor.goto_first_child() {
        loop {
            let node = cursor.node();
            match node.kind() {
                "class" => {
                    let start_byte = node.start_byte();
                    let end_byte = node.end_byte();
                    let keyword = &source_code[start_byte..end_byte];
                    class_signature.push_str(keyword);
                    class_signature.push(' ');
                }
                "type_identifier" => {
                    let start_byte = node.start_byte();
                    let end_byte = node.end_byte();
                    let child_name = &source_code[start_byte..end_byte];
                    class_signature.push_str(child_name);
                }
                _ => {}
            }

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
    handlers.insert("function_declaration", handle_ts_function);
    handlers.insert("class_declaration", handle_ts_class);
    // Insert more handlers as needed
    handlers
}
