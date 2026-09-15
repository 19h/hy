//! Output formatting helpers.

/// Format a duration in seconds to human-readable form.
#[allow(dead_code)]
pub fn format_duration(seconds: f64) -> String {
    if seconds < 60.0 {
        format!("{seconds:.1}s")
    } else if seconds < 3600.0 {
        format!("{:.1}m", seconds / 60.0)
    } else if seconds < 86400.0 {
        format!("{:.1}h", seconds / 3600.0)
    } else {
        format!("{:.1}d", seconds / 86400.0)
    }
}

/// Print a styled success message.
pub fn success(msg: &str) {
    use owo_colors::OwoColorize;
    eprintln!("  {} {msg}", "✓".green().bold());
}

/// Print a styled error message.
pub fn error(msg: &str) {
    use owo_colors::OwoColorize;
    eprintln!("  {} {}", "✗".red().bold(), msg.red());
}

/// Print a styled warning message.
pub fn warning(msg: &str) {
    use owo_colors::OwoColorize;
    eprintln!("  {} {}", "⚠".yellow().bold(), msg.yellow());
}

/// Print a styled info message.
pub fn info(msg: &str) {
    use owo_colors::OwoColorize;
    eprintln!("  {} {msg}", "•".blue().bold());
}
