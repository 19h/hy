//! Searchable shared-file selection and cancellation-aware action prompts.

use console::Term;
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};

use crate::error::{Error, Result};
use crate::util::tui::RawMode;

mod selection;

use selection::{Edit, Selection};

pub(super) fn files(choices: &[String], values: &[crate::api::Asset]) -> Result<Vec<usize>> {
    let term = Term::stderr();
    if !term.is_term() {
        return Err(Error::Other("file selection requires an interactive terminal".into()));
    }
    let raw_mode = RawMode::enter()?;
    let mut selection = Selection::new(choices, values);
    loop {
        let lines = render(&term, choices, &selection)?;
        let key = loop {
            if let Event::Key(key) = crossterm::event::read()?
                && key.kind != KeyEventKind::Release
            {
                break key;
            }
        };
        term.clear_last_lines(lines)?;
        let edit = match (key.code, key.modifiers.contains(KeyModifiers::CONTROL)) {
            (KeyCode::Enter, _) => return Ok(selection.selected()),
            (KeyCode::Char('c' | 'q'), true) => {
                drop(raw_mode);
                return Err(cancelled());
            }
            (KeyCode::Up, _) | (KeyCode::Char('p'), true) => Edit::Previous,
            (KeyCode::Down, _) | (KeyCode::Char('n'), true) => Edit::Next,
            (KeyCode::Char(' '), false) => Edit::Toggle,
            (KeyCode::Char('a'), true) => Edit::ToggleAll,
            (KeyCode::Tab, _) => Edit::Invert,
            (KeyCode::Backspace, _) => Edit::Erase,
            (KeyCode::Char(character), false)
                if character.is_ascii_graphic() && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                Edit::Type(character)
            }
            _ => continue,
        };
        selection.apply(edit);
    }
}

fn render(term: &Term, choices: &[String], selection: &Selection) -> std::io::Result<usize> {
    let (height, width) = term.size();
    let page_size = usize::from(height).saturating_sub(4).max(1);
    let start = selection.cursor / page_size * page_size;
    let page = &selection.visible[start..selection.visible.len().min(start + page_size)];
    let title = format!("? Select files to manage: {}", selection.query);
    write_line(term, &title, width)?;
    for (offset, &index) in page.iter().enumerate() {
        let cursor = if start + offset == selection.cursor {
            ">"
        } else {
            " "
        };
        let mark = if selection.is_selected(index) {
            "x"
        } else {
            " "
        };
        let row = format!("{cursor} [{mark}] {}", choices[index]);
        write_line(term, &row, width)?;
    }
    let hint = if selection.found_matches {
        "Type to filter; space selects; Ctrl-A toggles all; Tab inverts; Enter confirms"
    } else {
        "No matches; showing all files. Backspace edits the filter; Ctrl-C cancels"
    };
    write_line(term, hint, width)?;
    Ok(page.len() + 2)
}

fn write_line(term: &Term, text: &str, width: u16) -> std::io::Result<()> {
    term.write_str(&console::truncate_str(text, usize::from(width).saturating_sub(1), "…"))?;
    term.write_str("\r\n")
}

pub(super) fn action(choices: &[String]) -> Result<usize> {
    crate::cmd::share_prompt::select("What would you like to do?", choices, 0)
}

pub(super) fn output_directory() -> Result<String> {
    ask(inquire::Text::new("Output directory").with_initial_value("./").prompt())
}

fn ask<T>(result: std::result::Result<T, inquire::InquireError>) -> Result<T> {
    result.map_err(|error| match error {
        inquire::InquireError::OperationCanceled | inquire::InquireError::OperationInterrupted => {
            cancelled()
        }
        error => Error::Other(format!("shared-file prompt failed: {error}")),
    })
}

fn cancelled() -> Error {
    println!("Operation cancelled.");
    Error::ChildExit(0)
}
