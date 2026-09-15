//! Restore the previous terminal mode when a prompt returns or fails.

pub(crate) struct RawMode {
    already_enabled: bool,
}

impl RawMode {
    pub(crate) fn enter() -> std::io::Result<Self> {
        let already_enabled = crossterm::terminal::is_raw_mode_enabled()?;
        if !already_enabled {
            crossterm::terminal::enable_raw_mode()?;
        }
        Ok(Self {
            already_enabled,
        })
    }
}

impl Drop for RawMode {
    fn drop(&mut self) {
        if !self.already_enabled {
            let _ = crossterm::terminal::disable_raw_mode();
        }
    }
}
