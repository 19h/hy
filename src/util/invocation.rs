//! Follow-up instructions must name the executable selected by this invocation.

use std::path::Path;
use std::sync::OnceLock;

pub fn display() -> &'static str {
    static DISPLAY: OnceLock<String> = OnceLock::new();
    DISPLAY.get_or_init(|| {
        if let Ok(name) = std::env::var("HCLI_BINARY_NAME") {
            return name;
        }
        let Some(argument) = std::env::args_os().next() else {
            return "hy".into();
        };
        let path = Path::new(&argument);
        let name = path.file_name().and_then(|name| name.to_str()).unwrap_or("hy");
        if let Some(found) = crate::ida::python::find_command(name)
            && found.canonicalize().ok()
                == std::env::current_exe().ok().and_then(|path| path.canonicalize().ok())
        {
            return quote(name.trim_end_matches(".exe"));
        }
        let path = if path.components().count() > 1 {
            std::path::absolute(path).ok()
        } else {
            std::env::current_exe().ok()
        };
        quote(&path.map(|path| path.to_string_lossy().into_owned()).unwrap_or_else(|| name.into()))
    })
}

fn quote(value: &str) -> String {
    if cfg!(windows) {
        if !value.contains([' ', '\t', '"']) {
            return value.into();
        }
        let mut output = String::from("\"");
        let mut slashes = 0;
        for character in value.chars() {
            if character == '\\' {
                slashes += 1;
                continue;
            }
            if character == '"' {
                output.push_str(&"\\".repeat(slashes * 2 + 1));
            } else {
                output.push_str(&"\\".repeat(slashes));
            }
            output.push(character);
            slashes = 0;
        }
        output.push_str(&"\\".repeat(slashes * 2));
        output.push('"');
        output
    } else if !value.is_empty()
        && value.bytes().all(|byte| byte.is_ascii_alphanumeric() || b"_@%+=:,./-".contains(&byte))
    {
        value.into()
    } else {
        format!("'{}'", value.replace('\'', "'\"'\"'"))
    }
}
