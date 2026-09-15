//! Questionary select bindings with safe_ask_async cancellation semantics.

use console::Term;
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};

use crate::error::{Error, Result};
use crate::util::tui::RawMode;

pub(super) fn select(message: &str, choices: &[String], initial: usize) -> Result<usize> {
    assert!(initial < choices.len());
    let term = Term::stderr();
    if !term.is_term() {
        return Err(Error::Other("selection requires an interactive terminal".into()));
    }
    let raw_mode = RawMode::enter()?;
    let mut cursor = initial;
    loop {
        render(&term, message, choices, cursor)?;
        let key = loop {
            if let Event::Key(key) = crossterm::event::read()?
                && key.kind != KeyEventKind::Release
            {
                break key;
            }
        };
        term.clear_last_lines(choices.len() + 1)?;
        match (key.code, key.modifiers.contains(KeyModifiers::CONTROL)) {
            (KeyCode::Enter, _) => return Ok(cursor),
            (KeyCode::Char('c' | 'q'), true) => {
                drop(raw_mode);
                println!("\nOperation cancelled.");
                return Err(Error::ChildExit(0));
            }
            (KeyCode::Down, _) | (KeyCode::Char('j'), false) | (KeyCode::Char('n'), true) => {
                cursor = (cursor + 1) % choices.len();
            }
            (KeyCode::Up, _) | (KeyCode::Char('k'), false) | (KeyCode::Char('p'), true) => {
                cursor = (cursor + choices.len() - 1) % choices.len();
            }
            _ => (),
        }
    }
}

fn render(term: &Term, message: &str, choices: &[String], cursor: usize) -> std::io::Result<()> {
    let width = usize::from(term.size().1).saturating_sub(1);
    term.write_str(&console::truncate_str(&format!("? {message}"), width, "…"))?;
    term.write_str("\r\n")?;
    for (index, choice) in choices.iter().enumerate() {
        let marker = if index == cursor {
            ">"
        } else {
            " "
        };
        term.write_str(&console::truncate_str(&format!("{marker} {choice}"), width, "…"))?;
        term.write_str("\r\n")?;
    }
    Ok(())
}
