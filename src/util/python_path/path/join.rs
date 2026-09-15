//! Raw posixpath/ntpath joining before pathlib component normalization.

use super::{Flavor, windows_splitroot};

pub(super) fn raw(base: &str, relative: &str, flavor: Flavor) -> String {
    match flavor {
        Flavor::Posix => {
            if relative.starts_with('/') || base.is_empty() {
                relative.to_owned()
            } else if base.ends_with('/') {
                format!("{base}{relative}")
            } else {
                format!("{base}/{relative}")
            }
        }
        Flavor::Windows => windows(base, relative),
    }
}

fn windows(base: &str, relative: &str) -> String {
    let (mut drive, mut root, mut tail) = split(base);
    let (other_drive, other_root, other_tail) = split(relative);
    let joined_tail;
    if !other_root.is_empty() {
        if !other_drive.is_empty() || drive.is_empty() {
            drive = other_drive;
        }
        root = other_root;
        tail = other_tail;
    } else {
        if !other_drive.is_empty() && other_drive != drive {
            if other_drive.to_lowercase() != drive.to_lowercase() {
                return relative.to_owned();
            }
            drive = other_drive;
        }
        let separator = if tail.is_empty() || tail.ends_with(['/', '\\']) {
            ""
        } else {
            "\\"
        };
        joined_tail = format!("{tail}{separator}{other_tail}");
        tail = &joined_tail;
    }
    // UNC drives need a separator before a relative tail; drive letters do not.
    let separator = if !tail.is_empty()
        && root.is_empty()
        && !drive.is_empty()
        && !drive.ends_with([':', '/', '\\'])
    {
        "\\"
    } else {
        ""
    };
    format!("{drive}{root}{separator}{tail}")
}

fn split(value: &str) -> (&str, &str, &str) {
    let normalized = value.replace('/', "\\");
    let (drive, root, _) = windows_splitroot(&normalized);
    let drive_end = drive.len();
    let root_end = drive_end + root.len();
    // Separators are one byte in both spellings; preserve original drive spelling.
    (&value[..drive_end], &value[drive_end..root_end], &value[root_end..])
}
