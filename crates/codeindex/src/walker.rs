use std::error::Error;
use std::fs;
use std::io::ErrorKind::InvalidData;

use ignore::overrides::OverrideBuilder;
use ignore::WalkBuilder;
use tree_sitter::Parser;

use crate::languages::get_language;
use crate::outline::get_outline;

pub fn process_entries() -> Result<(), Box<dyn Error>> {
    let mut overrides = OverrideBuilder::new(".");
    overrides.add("!.git")?; // ignore .git directory

    for result in WalkBuilder::new("./")
        .overrides(overrides.build()?)
        .hidden(false)
        .build()
    {
        match result {
            Ok(entry) => process_entry(entry)?,
            Err(err) => eprintln!("Error: {}", err),
        }
    }
    Ok(())
}

fn process_entry(entry: ignore::DirEntry) -> Result<(), Box<dyn Error>> {
    // Skip the root directory
    if entry.depth() == 0 {
        return Ok(());
    }

    // Strip the './' prefix and print the path
    let path = entry.path().strip_prefix("./").unwrap_or(entry.path());
    println!("{}", path.display());

    // Check if path is a file
    if path.is_file() {
        match fs::read_to_string(path) {
            Ok(code) => {
                let mut parser = Parser::new();
                let language = get_language(path.extension().and_then(std::ffi::OsStr::to_str));

                if let Some(language) = language {
                    parser.set_language(language).unwrap();
                } else {
                    return Ok(()); // Ignore other file types
                }

                let extension = path.extension().and_then(std::ffi::OsStr::to_str);
                let tree = parser.parse(&code, None).unwrap();
                let root_node = tree.root_node();
                let outline = get_outline(root_node, &code, extension);
                for signature in outline {
                    println!("  {}", signature);
                }
            }
            Err(e) if e.kind() == InvalidData => {
                // Skip binary files
            }
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}
