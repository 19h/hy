//! Platform command construction; dialog text never becomes interpreter source.

use std::process::Command;

#[derive(Clone, Copy)]
pub(super) enum Platform {
    Mac,
    Windows,
    Linux,
}

impl Platform {
    pub fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::Mac
        } else if cfg!(windows) {
            Self::Windows
        } else {
            Self::Linux
        }
    }
}

fn apple_script(source: &str) -> Command {
    let mut command = Command::new("osascript");
    command.args(["-e", source]);
    command
}

fn powershell(source: &str) -> Command {
    let mut command = Command::new("powershell");
    command.args(["-WindowStyle", "Hidden", "-Command", source]);
    command
}

pub(super) fn confirm_commands(platform: Platform, message: &str) -> Vec<Command> {
    let mut command = match platform {
        Platform::Mac => apple_script(concat!(
            "display dialog (system attribute \"KE_DLG_MSG\") with title \"KE\" ",
            "buttons {\"Cancel\", \"Open\"} default button \"Open\" cancel button \"Cancel\""
        )),
        Platform::Windows => powershell(concat!(
            "Add-Type -AssemblyName System.Windows.Forms; ",
            "if ([System.Windows.Forms.MessageBox]::Show(",
            "$env:KE_DLG_MSG, \"KE\", \"YesNo\", \"Warning\") -eq \"Yes\") { exit 0 } else { exit 1 }"
        )),
        Platform::Linux => {
            let escaped = escape_markup(message);
            let mut zenity = Command::new("zenity");
            zenity.args(["--question", "--title=KE", &format!("--text={escaped}")]);
            let mut kdialog = Command::new("kdialog");
            kdialog.args(["--yesno", &escaped, "--title", "KE"]);
            return vec![zenity, kdialog];
        }
    };
    command.env("KE_DLG_MSG", message);
    vec![command]
}

pub(super) fn progress_command(platform: Platform, filename: &str) -> Command {
    let mut command = match platform {
        Platform::Mac => apple_script(concat!(
            "display dialog (\"Downloading \" & (system attribute \"KE_DLG_FILE\") ",
            "& \"\\n\\nPlease wait...\") with title \"KE\" buttons {} giving up after 600"
        )),
        Platform::Windows => powershell(concat!(
            "Add-Type -AssemblyName System.Windows.Forms; ",
            "[System.Windows.Forms.MessageBox]::Show(",
            "\"Downloading \" + $env:KE_DLG_FILE + \"...`n`nPlease wait...\", \"KE\")"
        )),
        Platform::Linux => {
            let mut command = Command::new("notify-send");
            command.args(["-t", "0", "KE", &format!("Downloading {filename}...")]);
            return command;
        }
    };
    command.env("KE_DLG_FILE", filename);
    command
}

pub(super) fn error_command(platform: Platform, message: &str) -> Command {
    let mut command = match platform {
        Platform::Mac => apple_script(concat!(
            "display dialog (system attribute \"KE_DLG_MSG\") with title \"KE\" ",
            "buttons {\"OK\"} default button \"OK\" with icon stop"
        )),
        Platform::Windows => powershell(concat!(
            "Add-Type -AssemblyName System.Windows.Forms; ",
            "[System.Windows.Forms.MessageBox]::Show($env:KE_DLG_MSG, \"KE\", \"OK\", \"Error\")"
        )),
        Platform::Linux => {
            let mut command = Command::new("notify-send");
            command.args(["-u", "critical", "KE", message]);
            return command;
        }
    };
    command.env("KE_DLG_MSG", message);
    command
}

fn escape_markup(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

#[cfg(test)]
mod tests;
