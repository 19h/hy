//! Lexical pathlib components: preserve parents and do not access files.

mod join;

#[derive(Clone, Copy)]
pub(crate) enum Flavor {
    Posix,
    Windows,
}

impl Flavor {
    fn separator(self) -> char {
        match self {
            Self::Posix => '/',
            Self::Windows => '\\',
        }
    }
}

pub(crate) struct ParsedPath {
    pub drive: String,
    pub root: String,
    pub tail: Vec<String>,
}

impl ParsedPath {
    /// Join raw spellings before parsing, as pathlib does for incomplete UNC paths.
    pub fn joined(base: &str, relative: &str, flavor: Flavor) -> Self {
        Self::parse(&join::raw(base, relative, flavor), flavor)
    }

    pub fn parse(value: &str, flavor: Flavor) -> Self {
        let value = match flavor {
            Flavor::Posix => value.to_owned(),
            Flavor::Windows => value.replace('/', "\\"),
        };
        let (drive, mut root, tail) = match flavor {
            Flavor::Posix => {
                let root = if value.starts_with("//") && !value.starts_with("///") {
                    "//"
                } else if value.starts_with('/') {
                    "/"
                } else {
                    ""
                };
                ("", root, value.trim_start_matches('/'))
            }
            Flavor::Windows => windows_splitroot(&value),
        };
        if root.is_empty() && drive.starts_with('\\') && !drive.ends_with('\\') {
            let parts: Vec<_> = drive.split('\\').collect();
            if (parts.len() == 4 && !"?.".contains(parts[2])) || parts.len() == 6 {
                root = "\\";
            }
        }
        Self {
            drive: drive.into(),
            root: root.into(),
            tail: tail
                .split(flavor.separator())
                .filter(|part| !part.is_empty() && *part != ".")
                .map(str::to_owned)
                .collect(),
        }
    }

    pub fn render(&self, flavor: Flavor) -> String {
        let separator = flavor.separator().to_string();
        let tail = self.tail.join(&separator);
        if !self.drive.is_empty() || !self.root.is_empty() {
            return format!("{}{}{tail}", self.drive, self.root);
        }
        // Removing a leading '.' must not turn a relative component into a drive.
        if matches!(flavor, Flavor::Windows)
            && self.tail.first().is_some_and(|first| !windows_splitroot(first).0.is_empty())
        {
            return format!(".{separator}{tail}");
        }
        if tail.is_empty() {
            ".".into()
        } else {
            tail
        }
    }
}

/// Split an already separator-normalized Windows path, including UNC/device drives.
pub(super) fn windows_splitroot(value: &str) -> (&str, &str, &str) {
    if let Some(tail) = value.strip_prefix('\\') {
        if tail.starts_with('\\') {
            let start =
                if value.get(..8).is_some_and(|head| head.eq_ignore_ascii_case("\\\\?\\UNC\\")) {
                    8
                } else {
                    2
                };
            let Some(server_end) = value[start..].find('\\').map(|offset| start + offset) else {
                return (value, "", "");
            };
            let Some(share_end) =
                value[server_end + 1..].find('\\').map(|offset| server_end + 1 + offset)
            else {
                return (value, "", "");
            };
            return (&value[..share_end], "\\", &value[share_end + 1..]);
        }
        return ("", "\\", tail);
    }
    if let Some((colon, ':')) = value.char_indices().nth(1) {
        let end = colon + 1;
        return match value[end..].strip_prefix('\\') {
            Some(tail) => (&value[..end], "\\", tail),
            None => (&value[..end], "", &value[end..]),
        };
    }
    ("", "", value)
}
