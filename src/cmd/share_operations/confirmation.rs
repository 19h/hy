//! Click overwrite and Rich deletion confirmations have different input grammars.

use std::io::{self, Write};

use crate::api::Asset;
use crate::error::{Error, Result};
use crate::util::strings::python_trim;

pub(super) fn overwrite() -> Result<bool> {
    ask("Overwrite existing file? [y/N]: ", Grammar::Overwrite)
}

pub(super) fn delete(asset: &Asset) -> Result<bool> {
    ask(
        &format!(
            "\nDelete file {} [{}]? [y/n]: ",
            asset.filename,
            asset.code.as_deref().unwrap_or("None")
        ),
        Grammar::Delete,
    )
}

enum Grammar {
    Overwrite,
    Delete,
}

impl Grammar {
    fn parse(&self, input: &str) -> Option<bool> {
        match (self, python_trim(input).to_lowercase().as_str()) {
            (_, "y") | (Self::Overwrite, "yes") => Some(true),
            (_, "n") | (Self::Overwrite, "no" | "") => Some(false),
            _ => None,
        }
    }

    fn invalid_message(&self) -> &'static str {
        match self {
            Self::Overwrite => "Error: invalid input",
            Self::Delete => "Please enter Y or N",
        }
    }
}

fn ask(prompt: &str, grammar: Grammar) -> Result<bool> {
    loop {
        print!("{prompt}");
        io::stdout().flush()?;
        let mut input = String::new();
        if io::stdin().read_line(&mut input)? == 0 {
            return Err(Error::Other(match grammar {
                Grammar::Overwrite => String::new(),
                Grammar::Delete => "EOF when reading a line".into(),
            }));
        }
        if let Some(answer) = grammar.parse(&input) {
            return Ok(answer);
        }
        println!("{}", grammar.invalid_message());
    }
}

#[cfg(test)]
mod tests;
