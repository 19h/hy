use super::{Context, Kind, Plan, Step, shell_export};
use crate::ida::python::platform::detection::Session;

pub(super) fn build(context: &Context, name: &str, value: &str) -> Plan {
    let path = context.home.join(".config/environment.d/50-hexrays-idapython-venv-executable.conf");
    let profile = context.profile();
    let mut steps = Vec::new();
    if context.systemd {
        steps.push(Step::file(
            Kind::LinuxEnvironmentD,
            format!("Create {} for graphical desktop sessions", path.display()),
            path.clone(),
            format!("{name}={value}"),
            true,
        ));
    }
    if let Some(profile) = &profile {
        steps.push(Step::file(
            Kind::ShellProfile,
            format!("Add export to {} for terminal/SSH sessions", profile.display()),
            profile.clone(),
            shell_export(name, value, context.shell_kind()),
            false,
        ));
    }
    let warnings = match (context.systemd, profile.is_some(), context.session) {
        (false, true, Session::Wayland) => vec![
            "This system does not use systemd, so environment.d is not available. Under Wayland, \
             there is no reliable mechanism to set per-user environment variables for graphical apps. \
             The shell profile may not reach IDA launched from the desktop. If IDA does not see \
             the variable, configure it in your Wayland compositor's environment settings \
             (e.g., sway: `exec`, Hyprland: `env =`, labwc: environment config).".into()
        ],
        (false, true, _) => vec![
            "This system does not use systemd, so environment.d is not available. Whether the shell \
             profile reaches graphical sessions depends on your display manager. If IDA launched \
             from the desktop does not see the variable, add it to ~/.xprofile (for X11) or \
             configure it in your desktop environment's session settings.".into()
        ],
        (false, false, _) => vec![format!(
            "This system does not use systemd, and HCLI could not detect your shell. \
             HCLI cannot automatically configure {name}. Set it in your shell's login profile \
             and in your desktop environment's session configuration (e.g., ~/.xprofile for X11, \
             or your Wayland compositor's environment settings)."
        )],
        (true, false, _) => vec![format!(
            "HCLI does not know how to configure {}. IDA launched from the desktop will work \
             (via environment.d), but terminal and SSH sessions will not see {name} until \
             you add the export to your shell's login profile manually.", context.shell_name()
        )],
        _ => Vec::new(),
    };
    let mut manual = vec![format!("Configure {name} for your system:\n")];
    if context.systemd {
        manual.push(format!(
            "  For graphical sessions (GNOME, KDE, etc.):\n    \
             Create {} with:\n      {name}={value}\n",
            path.display()
        ));
    }
    manual.push(if let Some(profile) = profile {
        format!(
            "  For terminal/SSH sessions:\n    Add to {}:\n      {}\n",
            profile.display(),
            shell_export(name, value, context.shell_kind())
        )
    } else {
        format!(
            "  For terminal/SSH sessions:\n    \
             Add to your shell's login profile (e.g. ~/.profile):\n      {}\n",
            shell_export(name, value, "sh")
        )
    });
    manual.push("Log out and back in for the changes to take effect.".into());
    Plan {
        steps,
        warnings,
        env_var_name: name.into(),
        env_var_value: value.into(),
        manual_instructions: manual.join("\n"),
    }
}
