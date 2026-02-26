//! `hcli logout` command.

use clap::Args;
use dialoguer::{Confirm, Select};

use crate::auth::AuthService;
use crate::error::Result;
use crate::util::fmt;

#[derive(Debug, Args)]
pub struct LogoutArgs {
    /// Name of specific credentials to remove
    #[arg(short, long)]
    pub name: Option<String>,

    /// Remove all credentials
    #[arg(short, long)]
    pub all: bool,
}

pub async fn run(args: LogoutArgs) -> Result<()> {
    let mut auth = AuthService::global();
    auth.init(None);

    let sources = auth.list_credentials().len();

    if sources == 0 {
        fmt::warning("No credentials found.");
        return Ok(());
    }

    if args.all {
        let confirm = Confirm::new()
            .with_prompt(format!("Remove all {sources} credentials?"))
            .default(false)
            .interact()
            .unwrap_or(false);

        if confirm {
            // Collect names first to avoid borrow issues.
            let names: Vec<String> = auth
                .list_credentials()
                .iter()
                .map(|c| c.name.clone())
                .collect();
            let mut removed = 0;
            for name in &names {
                if auth.remove_credentials(name) {
                    removed += 1;
                }
            }
            fmt::success(&format!("Removed {removed} credentials."));
        } else {
            fmt::warning("Logout cancelled.");
        }
        return Ok(());
    }

    if let Some(ref name) = args.name {
        let exists = auth
            .list_credentials()
            .iter()
            .any(|c| c.name == *name);
        if !exists {
            fmt::error(&format!("Credentials '{name}' not found."));
            return Ok(());
        }
        if auth.remove_credentials(name) {
            fmt::success(&format!("Removed credentials '{name}'."));
        }
        return Ok(());
    }

    // Single source: auto-remove.
    if sources == 1 {
        let name = auth
            .list_credentials()
            .first()
            .map(|c| c.name.clone())
            .unwrap();
        if auth.remove_credentials(&name) {
            fmt::success("Logged out.");
        }
        return Ok(());
    }

    // Multiple: interactive selection.
    let items: Vec<String> = auth
        .list_credentials()
        .iter()
        .map(|c| format!("{} - {} ({})", c.name, c.email, c.cred_type))
        .collect();

    let selection = Select::new()
        .with_prompt("Select credentials to remove")
        .items(&items)
        .interact_opt()
        .unwrap_or(None);

    if let Some(idx) = selection {
        let name = auth.list_credentials()[idx].name.clone();
        let confirm = Confirm::new()
            .with_prompt(format!("Remove credentials '{name}'?"))
            .default(false)
            .interact()
            .unwrap_or(false);
        if confirm {
            if auth.remove_credentials(&name) {
                fmt::success(&format!("Removed credentials '{name}'."));
            }
        } else {
            fmt::warning("Logout cancelled.");
        }
    }

    Ok(())
}
