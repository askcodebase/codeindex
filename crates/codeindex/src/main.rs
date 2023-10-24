use std::error::Error;
use std::fs;

use ignore::overrides::OverrideBuilder;
use ignore::WalkBuilder;
use tree_sitter::{Node, Parser};
use tree_sitter_rust as ts_rust;

fn main() -> Result<(), Box<dyn Error>> {
    let mut overrides = OverrideBuilder::new(".");
    overrides.add("!.git")?; // ignore .git directory

    for result in WalkBuilder::new("./")
        .overrides(overrides.build()?)
        .hidden(false)
        .build()
    {
        match result {
            Ok(entry) => {
                // Skip the root directory
                if entry.depth() == 0 {
                    continue;
                }

                // Strip the './' prefix and print the path
                let path = entry.path().strip_prefix("./").unwrap_or(entry.path());
                println!("{}", path.display());

                // Only process .rs files
                if path.extension() == Some(std::ffi::OsStr::new("rs")) {
                    let code = fs::read_to_string(path)?;
                    let mut parser = Parser::new();
                    let language = ts_rust::language();
                    parser.set_language(language).unwrap();
                    let tree = parser.parse(&code, None).unwrap();

                    let root_node = tree.root_node();
                    let outline = get_outline(root_node, &code);
                    for signature in outline {
                        println!("  {}", signature);
                    }
                }
            }
            Err(err) => eprintln!("Error: {}", err),
        }
    }
    Ok(())
}

fn get_outline(node: Node, source_code: &str) -> Vec<String> {
    let mut signatures = Vec::new();

    if node.kind() == "source_file" {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child_kind = cursor.node().kind();

                // Lookup the handler for this kind of node
                if let Some(handler) = get_handler(child_kind) {
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
        let mut child_signatures = get_outline(child, source_code);
        signatures.append(&mut child_signatures);
    }

    signatures
}

// This function returns a function that can handle nodes of the given kind
fn get_handler(kind: &str) -> Option<fn(&mut tree_sitter::TreeCursor, &str) -> String> {
    match kind {
        "function_item" => Some(handle_function),
        "struct_item" => Some(handle_struct),
        // Add more cases here as needed
        _ => None,
    }
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
