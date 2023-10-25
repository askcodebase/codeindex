use tree_sitter::Language;
use {
    tree_sitter_javascript as ts_js, tree_sitter_python as ts_python, tree_sitter_rust as ts_rust,
    tree_sitter_typescript as ts_ts,
};

pub fn get_language(extension: Option<&str>) -> Option<Language> {
    match extension {
        Some("rs") => Some(ts_rust::language()),
        Some("js") | Some("jsx") => Some(ts_js::language()),
        Some("ts") => Some(ts_ts::language_typescript()),
        Some("tsx") => Some(ts_ts::language_tsx()),
        Some("py") => Some(ts_python::language()),
        _ => None,
    }
}
