//! Default selection commits locally before synchronous status lookup.

use super::DefaultArgs;
use crate::auth::AuthService;
use crate::error::{Error, Result};
use crate::util::fmt;

pub(super) async fn run(args: DefaultArgs) -> Result<()> {
    let show_status =
        tokio::task::spawn_blocking(move || apply(args)).await.map_err(|error| {
            Error::Authentication(format!("authentication worker failed: {error}"))
        })??;
    if show_status {
        crate::auth::show_resolved_login_info().await?;
    }
    Ok(())
}

fn apply(args: DefaultArgs) -> Result<bool> {
    let mut auth = AuthService::global();
    auth.init(None)?;

    if let Some(name) = args.name {
        // Set new default.
        let creds = auth.list_credentials();
        if creds.is_empty() {
            fmt::warning("No credentials found.");
            eprintln!("Use 'hy login' or 'hy auth key install' to add credentials.");
            return Ok(false);
        }

        let names: Vec<String> = creds.iter().map(|c| c.name.clone()).collect();
        if !names.contains(&name) {
            fmt::error(&format!("Credentials '{name}' not found."));
            eprintln!("Available credentials: {}", names.join(", "));
            return Ok(false);
        }

        // Check if already default.
        if auth.default_name() == Some(name.as_str()) {
            fmt::warning(&format!("'{name}' is already the default credentials."));
            return Ok(false);
        }

        if auth.set_default(&name)? {
            fmt::success(&format!("Set '{name}' as the default credentials."));
            eprintln!();
            return Ok(true);
        } else {
            fmt::error(&format!("Failed to set '{name}' as default credentials."));
        }
    } else {
        // Show current default.
        if let Some(name) = auth.default_name() {
            if let Some(c) = auth.current_credentials() {
                fmt::success(&format!("Default credentials: {name}"));
                eprintln!("Email: {}", c.email);
                eprintln!("Type: {}", c.cred_type);
            } else {
                fmt::warning(&format!("Default set to '{name}' but source not found."));
            }
        } else {
            fmt::warning("No default credentials set.");
        }

        // Show available sources.
        let creds = auth.list_credentials();
        if !creds.is_empty() {
            let names: Vec<String> = creds.iter().map(|c| c.name.clone()).collect();
            eprintln!("\nAvailable sources: {}", names.join(", "));
        }
    }

    Ok(false)
}
