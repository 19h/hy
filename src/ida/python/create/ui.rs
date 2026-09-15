//! Creation progress and Rich-compatible yes/no input grammar.

use std::io::{self, Write};

use crate::error::{Error, Result};

pub(super) struct Ui {
    pub interactive: bool,
    pub quiet: bool,
}

impl Ui {
    pub fn print(&self, message: &str) {
        if self.quiet {
            eprintln!("{message}");
        } else {
            println!("{message}");
        }
    }

    pub fn confirm(&self, prompt: &str) -> Result<bool> {
        if !self.interactive {
            return Ok(true);
        }
        loop {
            print!("{prompt} [y/n] (y): ");
            io::stdout().flush()?;
            let mut input = String::new();
            if io::stdin().read_line(&mut input)? == 0 {
                return Err(Error::Other("EOF when reading a line".into()));
            }
            match crate::util::strings::python_trim(&input).to_lowercase().as_str() {
                "" | "y" => return Ok(true),
                "n" => return Ok(false),
                _ => self.print("Please enter Y or N"),
            }
        }
    }
}
