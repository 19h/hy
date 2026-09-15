use super::{Kind, Plan, Step};

pub(super) fn build(name: &str, value: &str) -> Plan {
    let command =
        format!("[Environment]::SetEnvironmentVariable(\"{name}\", \"{value}\", \"User\")");
    let step = Step::command(
        Kind::WindowsUserEnv,
        format!("Set {name} as a user environment variable via PowerShell"),
        "powershell",
        vec!["-NoProfile".into(), "-Command".into(), command.clone()],
    );
    let manual =
        format!(include_str!("windows-manual.txt"), name = name, value = value, command = command,)
            .trim_end_matches('\n')
            .to_owned();
    Plan {
        steps: vec![step],
        warnings: Vec::new(),
        env_var_name: name.into(),
        env_var_value: value.into(),
        manual_instructions: manual,
    }
}
