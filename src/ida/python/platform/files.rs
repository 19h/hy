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
    let assignment = assignment_pattern(line);
    let mut stale = false;
    let mut updated = String::new();
    for part in lines {
        if assignment.as_ref().is_some_and(|pattern| pattern.is_match(part)) {
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

fn assignment_pattern(line: &str) -> Option<regex::Regex> {
    let declaration = regex::Regex::new(
        r"^(?:export\s+)?([A-Za-z_][A-Za-z0-9_]*)=|^set\s+(?:-\S+\s+)*([A-Za-z_][A-Za-z0-9_]*)",
    )
    .expect("constant assignment expression");
    let variable =
        declaration.captures(line).and_then(|captures| captures.get(1).or_else(|| captures.get(2)));
    variable.map(|variable| {
        regex::Regex::new(&format!(
            r"^(?:(?:export\s+)?{}=|set\s+(?:-\S+\s+)*{}(?:\s|$))",
            regex::escape(variable.as_str()),
            regex::escape(variable.as_str()),
        ))
        .expect("escaped assignment name")
    })
}

pub(crate) fn preview_profile(path: &Path, line: &str) -> Result<(Vec<String>, Vec<String>)> {
    let existing = if path.is_file() {
        read(path)?
    } else {
        String::new()
    };
    let lines: Vec<_> = existing.split(line_break).collect();
    if lines.contains(&line) {
        return Ok((Vec::new(), Vec::new()));
    }
    let assignment = assignment_pattern(line);
    let removed = lines
        .into_iter()
        .filter(|line| assignment.as_ref().is_some_and(|pattern| pattern.is_match(line)))
        .map(str::to_owned)
        .collect();
    Ok((removed, vec![line.into()]))
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
    if path.is_file() && existing.trim_end_matches('\n') == content.trim_end_matches('\n') {
        return Ok((true, format!("{} already configured", path.display())));
    }
    fs::create_dir_all(path.parent().unwrap())?;
    fs::write(path, encoded(content))?;
    Ok((false, format!("Created {}", path.display())))
}
