//! Shared TUI primitives: themed prompts, width-aware tables, and
//! consistent progress indicators.
//!
//! All interactive elements should go through this module so the CLI keeps
//! a single visual language.

use std::time::Duration;

use console::Term;
use dialoguer::theme::ColorfulTheme;
use indicatif::{ProgressBar, ProgressStyle};
use owo_colors::OwoColorize;

/// The shared prompt theme.
pub fn theme() -> ColorfulTheme {
    ColorfulTheme::default()
}

/// Themed yes/no prompt. Returns `default` when the prompt is cancelled
/// or the terminal is not interactive.
pub fn confirm(prompt: &str, default: bool) -> bool {
    dialoguer::Confirm::with_theme(&theme())
        .with_prompt(prompt)
        .default(default)
        .interact()
        .unwrap_or(default)
}

/// Themed single-choice select. Returns `None` when cancelled.
pub fn select(prompt: &str, items: &[String], default: usize) -> Option<usize> {
    dialoguer::Select::with_theme(&theme())
        .with_prompt(prompt)
        .items(items)
        .default(default)
        .interact_opt()
        .ok()
        .flatten()
}

/// Themed multi-select. Returns `None` when cancelled.
pub fn multi_select(prompt: &str, items: &[String]) -> Option<Vec<usize>> {
    dialoguer::MultiSelect::with_theme(&theme())
        .with_prompt(prompt)
        .items(items)
        .interact_opt()
        .ok()
        .flatten()
}

/// Themed free-text input with a default value.
pub fn input(prompt: &str, default: &str) -> String {
    dialoguer::Input::with_theme(&theme())
        .with_prompt(prompt)
        .default(default.to_string())
        .allow_empty(true)
        .interact_text()
        .unwrap_or_else(|_| default.to_string())
}

/// A byte-transfer progress bar with the shared style.
pub fn byte_progress(total: u64, msg: impl Into<String>) -> ProgressBar {
    let pb = ProgressBar::new(total);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("  {msg:.bold} {bar:32.cyan/blue} {bytes}/{total_bytes} {bytes_per_sec:.dim} eta {eta:.dim}")
            .unwrap()
            .progress_chars("━╸─"),
    );
    pb.set_message(msg.into());
    pb
}

/// A spinner for long-running operations; ticks on its own.
pub fn spinner(msg: impl Into<String>) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("  {spinner:.cyan} {msg}")
            .unwrap()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏", " "]),
    );
    pb.set_message(msg.into());
    pb.enable_steady_tick(Duration::from_millis(80));
    pb
}

// ── width-aware tables ──────────────────────────────────────────────────

/// A simple table that sizes its columns to the content and reflows to
/// the terminal width. Cells may contain ANSI colour codes; widths are
/// measured with the escapes stripped.
pub struct Table {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
}

const INDENT: &str = "  ";
const GUTTER: &str = "  ";

impl Table {
    pub fn new(headers: &[&str]) -> Self {
        Self {
            headers: headers.iter().map(|s| s.to_string()).collect(),
            rows: Vec::new(),
        }
    }

    pub fn add_row(&mut self, cells: Vec<String>) {
        self.rows.push(cells);
    }

    /// Render the table to stderr.
    pub fn print(&self) {
        let cols = self.headers.len();

        // Natural column widths (ANSI-aware).
        let mut widths: Vec<usize> = self
            .headers
            .iter()
            .map(|h| console::measure_text_width(h))
            .collect();
        for row in &self.rows {
            for (i, cell) in row.iter().enumerate().take(cols) {
                widths[i] = widths[i].max(console::measure_text_width(cell));
            }
        }

        // Shrink to terminal width by trimming the widest columns.
        let term_width = Term::stderr().size_checked().map(|(_, w)| w as usize).unwrap_or(100);
        let term_width = term_width.max(60);
        let chrome = INDENT.len() + GUTTER.len() * (cols.saturating_sub(1));
        let mut total: usize = widths.iter().sum::<usize>() + chrome;
        while total > term_width {
            // Clamp the widest column; stop once everything is small.
            let (idx, _) = widths
                .iter()
                .enumerate()
                .max_by_key(|(_, w)| **w)
                .unwrap();
            if widths[idx] <= 8 {
                break;
            }
            let excess = total - term_width;
            widths[idx] = widths[idx].saturating_sub(excess).max(8);
            total = widths.iter().sum::<usize>() + chrome;
        }

        // Header.
        let mut line = String::from(INDENT);
        for (i, h) in self.headers.iter().enumerate() {
            line.push_str(&format!("{}", fit_cell(h, widths[i]).bold()));
            if i + 1 < cols {
                line.push_str(GUTTER);
            }
        }
        eprintln!("{line}");

        // Separator.
        let sep_len: usize = widths.iter().sum::<usize>() + GUTTER.len() * (cols - 1);
        eprintln!("{INDENT}{}", "─".repeat(sep_len).dimmed());

        // Rows.
        for row in &self.rows {
            let mut line = String::from(INDENT);
            for (i, width) in widths.iter().enumerate() {
                let cell = row.get(i).map(String::as_str).unwrap_or("");
                line.push_str(&fit_cell(cell, *width));
                if i + 1 < cols {
                    line.push_str(GUTTER);
                }
            }
            eprintln!("{line}");
        }
    }
}

/// Pad a cell to `width` display columns, truncating with an ellipsis only
/// when it genuinely overflows. (`console::pad_str` truncates exact-fit
/// strings, so we pad ourselves.)
fn fit_cell(s: &str, width: usize) -> String {
    let w = console::measure_text_width(s);
    if w > width {
        console::truncate_str(s, width.saturating_sub(1), "…").to_string()
    } else {
        format!("{s}{}", " ".repeat(width - w))
    }
}
