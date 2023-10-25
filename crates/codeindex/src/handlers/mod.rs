mod javascript;
mod python;
mod rust;
mod typescript;

use std::collections::HashMap;

use tree_sitter::TreeCursor;

pub fn get_handlers(extension: &str) -> HashMap<&'static str, fn(&mut TreeCursor, &str) -> String> {
    match extension {
        "js" | "jsx" => javascript::get_handlers(),
        "ts" | "tsx" => typescript::get_handlers(),
        "rs" => rust::get_handlers(),
        "py" => python::get_handlers(),
        _ => HashMap::new(),
    }
}
