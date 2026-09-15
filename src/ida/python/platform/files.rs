//! Python text-file and profile-update semantics, including unchanged-file skips.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use super::plan::Kind;
use crate::error::Result;

pub(super) enum Change {
    Unchanged,
    Updated(String),
    Added(String),
}

pub(super) fn profile_change(existing: &str, line: &str) -> Change {
    let lines: Vec<_> = existing.split_inclusive(line_break).collect();
    if lines.iter().any(|part| without_ending(part) == line) {
        return Change::Unchanged;
    }
    // The pinned helper searches '=' before space. For ordinary fish exports
    // this deliberately yields "set ", including unrelated set assignments.
    let prefix = line.find('=').or_else(|| line.find(' ')).map_or(line, |index| &line[..=index]);
    let mut stale = false;
    let mut updated = String::new();
    for part in lines {
        if part.trim_end_matches('\n').starts_with(prefix) {
            if !stale {
                updated.push_str(line);
                updated.push('\n');
            }
            stale = true;
        } else {
            updated.push_str(part);
        }
    }
    if stale {
        return Change::Updated(updated);
    }
    Change::Added(format!(
        "{}{line}\n",
        if existing.is_empty() || existing.ends_with('\n') {
            ""
        } else {
            "\n"
        }
    ))
}

fn line_break(character: char) -> bool {
    matches!(
        character,
        '\n' | '\r' | '\x0b' | '\x0c' | '\x1c'..='\x1e' | '\u{85}' | '\u{2028}' | '\u{2029}'
    )
}

fn without_ending(line: &str) -> &str {
    let Some(last) = line.chars().last().filter(|&character| line_break(character)) else {
        return line;
    };
    &line[..line.len() - last.len_utf8()]
}

fn read(path: &Path) -> std::io::Result<String> {
    // TextIOWrapper's default newline mode translates CRLF and CR on reading.
    Ok(String::from_utf8_lossy(&fs::read(path)?).replace("\r\n", "\n").replace('\r', "\n"))
}

fn encoded(text: &str) -> Vec<u8> {
    if cfg!(windows) {
        text.replace('\n', "\r\n").into_bytes()
    } else {
        text.as_bytes().to_vec()
    }
}

pub(super) fn apply(kind: Kind, path: &Path, content: &str) -> Result<(bool, String)> {
    let existing = if path.is_file() {
        read(path)?
    } else {
        String::new()
    };
    if kind == Kind::ShellProfile {
        let change = profile_change(&existing, content);
        let verb = match change {
            Change::Unchanged => {
                return Ok((true, format!("{} already contains this line", path.display())));
            }
            Change::Updated(text) => {
                fs::create_dir_all(path.parent().unwrap())?;
                fs::write(path, encoded(&text))?;
                "Updated"
            }
            Change::Added(text) => {
                fs::create_dir_all(path.parent().unwrap())?;
                OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)?
                    .write_all(&encoded(&text))?;
                "Added to"
            }
        };
        return Ok((false, format!("{verb} {}", path.display())));
    }
    if path.is_file()
        && (existing.contains(content)
            || existing
                .split_inclusive(line_break)
                .any(|line| without_ending(line) == content.trim_end_matches('\n')))
    {
        return Ok((true, format!("{} already configured", path.display())));
    }
    fs::create_dir_all(path.parent().unwrap())?;
    fs::write(path, encoded(content))?;
    Ok((false, format!("Created {}", path.display())))
}
