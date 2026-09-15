use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum System {
    Windows,
    Macos,
    Linux,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Session {
    Wayland,
    X11,
    Tty,
    Unknown,
}

pub(crate) struct Context {
    pub system: System,
    pub home: PathBuf,
    pub shell: Option<String>,
    pub systemd: bool,
    pub session: Session,
}

impl Context {
    pub fn detect() -> Result<Self> {
        let system = if cfg!(windows) {
            System::Windows
        } else if cfg!(target_os = "macos") {
            System::Macos
        } else {
            System::Linux
        };
        Ok(Self {
            system,
            home: dirs::home_dir()
                .ok_or_else(|| Error::Other("home directory unavailable".into()))?,
            shell: std::env::var("SHELL").ok(),
            systemd: cfg!(target_os = "linux") && Path::new("/run/systemd/system").is_dir(),
            session: session(
                std::env::var("WAYLAND_DISPLAY").ok().as_deref(),
                std::env::var("DISPLAY").ok().as_deref(),
                std::env::var("TERM").ok().as_deref(),
            ),
        })
    }

    pub fn shell_kind(&self) -> &str {
        match self
            .shell
            .as_deref()
            .and_then(|shell| Path::new(shell).file_name())
            .and_then(|name| name.to_str())
        {
            Some(name @ ("bash" | "zsh" | "fish" | "sh")) => name,
            _ => "unknown",
        }
    }

    pub fn profile(&self) -> Option<PathBuf> {
        let suffix = match self.shell_kind() {
            "bash" => ".bash_profile",
            "zsh" => ".zprofile",
            "fish" => ".config/fish/config.fish",
            "sh" => ".profile",
            _ => return None,
        };
        Some(self.home.join(suffix))
    }

    pub fn shell_name(&self) -> &str {
        self.shell.as_deref().unwrap_or("your shell")
    }
}

pub(super) fn session(wayland: Option<&str>, display: Option<&str>, term: Option<&str>) -> Session {
    if wayland.is_some_and(|value| !value.is_empty()) {
        Session::Wayland
    } else if display.is_some_and(|value| !value.is_empty()) {
        Session::X11
    } else if term.is_some_and(|value| !value.is_empty()) {
        Session::Tty
    } else {
        Session::Unknown
    }
}
