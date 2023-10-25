use tree_sitter::TreeCursor;

// This function returns a function that can handle nodes of the given kind
pub fn get_handler(
    kind: &str,
    extension: &str,
) -> Option<fn(&mut tree_sitter::TreeCursor, &str) -> String> {
    match extension {
        "js" | "jsx" => {
            match kind {
                "function_declaration" => Some(handle_js_function),
                // Add more JavaScript-specific handlers here as needed
                _ => None,
            }
        }
        "ts" | "tsx" => {
            match kind {
                "function_declaration" => Some(handle_ts_function),
                "class_declaration" => Some(handle_ts_class),
                // Add more TypeScript-specific handlers here as needed
                _ => None,
            }
        }
        "py" => {
            // Add Python-specific handlers here
            None
        }
        "rs" => {
            match kind {
                "function_item" => Some(handle_function),
                "struct_item" => Some(handle_struct),
                // Add more Rust-specific handlers here as needed
                _ => None,
            }
        }
        _ => None,
    }
}

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
fn handle_ts_class(cursor: &mut tree_sitter::TreeCursor, source_code: &str) -> String {
    let mut class_signature = String::new();
    if cursor.goto_first_child() {
        loop {
            let node = cursor.node();
            if node.kind() == "identifier" {
                let start_byte = node.start_byte();
                let end_byte = node.end_byte();
                let child_name = &source_code[start_byte..end_byte];
                class_signature.push_str(&format!("class {} {{", child_name));
            }
            // You may want to handle fields, methods, etc. here...

            if !cursor.goto_next_sibling() {
                break;
            }
        }
        cursor.goto_parent();
    }
    class_signature.push_str("}");
    class_signature
}

// Handler for JavaScript function_declaration
fn handle_js_function(cursor: &mut tree_sitter::TreeCursor, source_code: &str) -> String {
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

// Handler for function_item
fn handle_function(cursor: &mut tree_sitter::TreeCursor, source_code: &str) -> String {
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

// Handler for struct_item
fn handle_struct(cursor: &mut tree_sitter::TreeCursor, source_code: &str) -> String {
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
    struct_signature
}
