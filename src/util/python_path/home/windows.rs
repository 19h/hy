//! Windows environment rules for pathlib home expansion.

use super::super::path::windows_splitroot;

pub(crate) fn resolve(user: &str, env: impl Fn(&str) -> Option<String>) -> Option<String> {
    let mut home = if let Some(profile) = env("USERPROFILE") {
        profile
    } else {
        join(&env("HOMEDRIVE").unwrap_or_default(), &env("HOMEPATH")?)
    };
    if !user.is_empty() {
        let current = env("USERNAME");
        if current.as_deref() != Some(user) {
            let (parent, basename) = split(&home);
            if current.as_deref() != Some(basename.as_str()) {
                return None;
            }
            home = join(&parent, user);
        }
    }
    Some(home)
}

fn join(base: &str, value: &str) -> String {
    let base = base.replace('/', "\\");
    let value = value.replace('/', "\\");
    let (mut drive, mut root, mut tail) = windows_splitroot(&base);
    let (value_drive, value_root, value_tail) = windows_splitroot(&value);
    let joined;
    if !value_root.is_empty() {
        if !value_drive.is_empty() || drive.is_empty() {
            drive = value_drive;
        }
        root = value_root;
        tail = value_tail;
    } else if !value_drive.is_empty() && value_drive.to_lowercase() != drive.to_lowercase() {
        drive = value_drive;
        root = value_root;
        tail = value_tail;
    } else {
        if !value_drive.is_empty() {
            drive = value_drive;
        }
        let separator = if tail.is_empty() || tail.ends_with('\\') {
            ""
        } else {
            "\\"
        };
        joined = format!("{tail}{separator}{value_tail}");
        tail = &joined;
    }
    let separator = if !tail.is_empty()
        && root.is_empty()
        && !drive.is_empty()
        && !drive.ends_with([':', '\\'])
    {
        "\\"
    } else {
        ""
    };
    format!("{drive}{root}{separator}{tail}")
}

fn split(value: &str) -> (String, String) {
    let value = value.replace('/', "\\");
    let (drive, root, tail) = windows_splitroot(&value);
    let (head, basename) = tail.rsplit_once('\\').unwrap_or(("", tail));
    (format!("{drive}{root}{}", head.trim_end_matches('\\')), basename.into())
}
