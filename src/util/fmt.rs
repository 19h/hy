//! Output formatting helpers.

/// Format a byte count as a human-readable size string.
pub fn format_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    if bytes == 0 {
        return "0 B".into();
    }
    let mut size = bytes as f64;
    let mut idx = 0;
    while size >= 1024.0 && idx < UNITS.len() - 1 {
        size /= 1024.0;
        idx += 1;
    }
    format!("{:.1} {}", size, UNITS[idx])
}

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

/// Format an ISO 8601 date string to `YYYY-MM-DD HH:MM`.
pub fn format_datetime(iso: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(iso)
        .or_else(|_| chrono::DateTime::parse_from_rfc3339(&iso.replace('Z', "+00:00")))
        .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|_| iso.chars().take(16).collect())
}

/// Print a styled success message.
pub fn success(msg: &str) {
    use owo_colors::OwoColorize;
    eprintln!("{}", format!("  {msg}").green());
}

/// Print a styled error message.
pub fn error(msg: &str) {
    use owo_colors::OwoColorize;
    eprintln!("{}", format!("  {msg}").red());
}

/// Print a styled warning message.
pub fn warning(msg: &str) {
    use owo_colors::OwoColorize;
    eprintln!("{}", format!("  {msg}").yellow());
}

/// Print a styled info message.
pub fn info(msg: &str) {
    use owo_colors::OwoColorize;
    eprintln!("{}", format!("  {msg}").blue());
}
