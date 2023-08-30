use std::env;

use atty::Stream;
use colored::{Color, ColoredString, Colorize};

fn paint(text: &str, true_color: bool) -> ColoredString {
    if true_color {
        text.bold().truecolor(70, 144, 239)
    } else {
        text.bold().color(Color::Blue)
    }
}

/// Prints welcome message
#[rustfmt::skip]
pub fn welcome() {
    if !atty::is(Stream::Stdout) {
        colored::control::set_override(false);
    }

    let mut true_color = true;

    match env::var("COLORTERM") {
        Ok(val) => if val != "24bit" && val != "truecolor" {
            true_color = false;
        },
        Err(_) => true_color = false,
    }

    println!("{}", paint(r#"  _________     _________    ________      _________            "#, true_color));
    println!("{}", paint(r#"  __  ____/___________  /________  _/____________  /________  __"#, true_color));
    println!("{}", paint(r#"  _  /    _  __ \  __  /_  _ \__  / __  __ \  __  /_  _ \_  |/_/"#, true_color));
    println!("{}", paint(r#"  / /___  / /_/ / /_/ / /  __/_/ /  _  / / / /_/ / /  __/_>  <  "#, true_color));
    println!("{}", paint(r#"  \____/  \____/\__,_/  \___//___/  /_/ /_/\__,_/  \___//_/|_|  "#, true_color));
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_welcome() {
        welcome();
    }
}
