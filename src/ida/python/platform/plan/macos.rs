use super::{Context, Kind, Plan, Step, shell_export};

pub(super) fn build(context: &Context, name: &str, value: &str) -> Plan {
    let path =
        context.home.join("Library/LaunchAgents/com.hex-rays.idapython-venv-executable.plist");
    // Interpolation is literal, matching the pinned source template.
    let content = format!(include_str!("launchagent.plist"), name = name, value = value);
    let mut steps = vec![
        Step::file(
            Kind::MacosLaunchagent,
            format!("Create LaunchAgent so IDA launched from Finder/Dock inherits {name}"),
            path.clone(),
            content,
            true,
        ),
        Step::command(
            Kind::MacosLaunchctlSetenv,
            format!("Apply {name} to the current session (immediate, no logout needed)"),
            "launchctl",
            vec!["setenv".into(), name.into(), value.into()],
        ),
    ];
    let mut warnings = Vec::new();
    let profile = context.profile();
    if let Some(profile) = &profile {
        steps.push(Step::file(
            Kind::ShellProfile,
            format!("Add export to {} for terminal sessions", profile.display()),
            profile.clone(),
            shell_export(name, value, context.shell_kind()),
            false,
        ));
    } else {
        warnings.push(format!(
            "HCLI does not know how to configure {}. IDA launched from Finder/Dock \
             will work (via the LaunchAgent), but terminal sessions will not see \
             {name} until you add the export to your shell's login profile manually.",
            context.shell_name()
        ));
    }
    let mut manual = vec![
        format!("Configure {name} for your system:\n"),
        format!(
            "  For Finder/Dock (LaunchAgent):\n    \
             Create {} with the contents shown above,\n    \
             then run: launchctl setenv {name} {value}\n",
            path.display()
        ),
    ];
    manual.push(if let Some(profile) = profile {
        format!(
            "  For terminal sessions:\n    Add to {}:\n      {}\n",
            profile.display(),
            shell_export(name, value, context.shell_kind())
        )
    } else {
        format!(
            "  For terminal sessions:\n    Add to your shell's login profile:\n      {}\n",
            shell_export(name, value, "sh")
        )
    });
    manual.push(
        "Log out and back in for the LaunchAgent to take effect.\n\
         New terminal windows pick up the shell profile change immediately."
            .into(),
    );
    Plan {
        steps,
        warnings,
        env_var_name: name.into(),
        env_var_value: value.into(),
        manual_instructions: manual.join("\n"),
    }
}
