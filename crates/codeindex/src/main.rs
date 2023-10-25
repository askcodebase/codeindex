mod walker;  
mod handlers;  
mod languages;  
mod outline;  
  
use std::error::Error;  
use walker::process_entries;  
  
fn main() -> Result<(), Box<dyn Error>> {  
    process_entries()  
}  
