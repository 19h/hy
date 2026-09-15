//! Side-effect-free platform plans, retaining upstream step order and contents.

use std::path::PathBuf;

use serde::Serialize;

use super::detection::{Context, System};

mod linux;
mod macos;
mod windows;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Kind {
    WindowsUserEnv,
    MacosLaunchagent,
    MacosLaunchctlSetenv,
    LinuxEnvironmentD,
    ShellProfile,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Self::WindowsUserEnv => "windows-user-env",
            Self::MacosLaunchagent => "macos-launchagent",
            Self::MacosLaunchctlSetenv => "macos-launchctl-setenv",
            Self::LinuxEnvironmentD => "linux-environment-d",
            Self::ShellProfile => "shell-profile",
        }
    }
}

#[derive(Debug)]
pub(crate) enum Action {
    File {
        path: PathBuf,
        content: String,
    },
    Command {
        program: String,
        args: Vec<String>,
    },
}

#[derive(Debug)]
pub(crate) struct Step {
    pub kind: Kind,
    pub description: String,
    pub action: Action,
    pub needs_logout: bool,
}

impl Step {
    fn file(
        kind: Kind,
        description: String,
        path: PathBuf,
        content: String,
        needs_logout: bool,
    ) -> Self {
        Self {
            kind,
            description,
            action: Action::File {
                path,
                content,
            },
            needs_logout,
        }
    }

    fn command(kind: Kind, description: String, program: &str, args: Vec<String>) -> Self {
        Self {
            kind,
            description,
            action: Action::Command {
                program: program.into(),
                args,
            },
            needs_logout: false,
        }
    }
}

#[derive(Debug)]
pub(crate) struct Plan {
    pub steps: Vec<Step>,
    pub warnings: Vec<String>,
    pub env_var_name: String,
    pub env_var_value: String,
    pub manual_instructions: String,
}

impl Plan {
    pub fn needs_logout(&self) -> bool {
        self.steps.iter().any(|step| step.needs_logout)
    }
}

pub(crate) fn build(context: &Context, name: &str, value: &str) -> Plan {
    match context.system {
        System::Windows => windows::build(name, value),
        System::Macos => macos::build(context, name, value),
        System::Linux => linux::build(context, name, value),
    }
}

fn shell_export(name: &str, value: &str, shell: &str) -> String {
    if shell == "fish" {
        format!("set -gx {name} \"{value}\"")
    } else {
        format!("export {name}=\"{value}\"")
    }
}
