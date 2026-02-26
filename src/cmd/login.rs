//! `hcli login` command.

use clap::Args;
use dialoguer::{Confirm, Select};

use crate::auth::AuthService;
use crate::error::Result;
use crate::util::fmt;

#[derive(Debug, Args)]
pub struct LoginArgs {
    /// Force account selection
    #[arg(short, long)]
    pub force: bool,

    /// Custom name for the credentials
    #[arg(short, long)]
    pub name: Option<String>,
}

pub async fn run(args: LoginArgs) -> Result<()> {
    let mut auth = AuthService::global();
    auth.init(None);

    // Already logged in?
    if auth.is_logged_in() && !args.force {
        let cred = auth.current_credentials();
        if let Some(c) = cred {
            fmt::success(&format!("You are already logged in as {}.", c.email));
        }

        let add_another = Confirm::new()
            .with_prompt("Would you like to login as another user?")
            .default(false)
            .interact()
            .unwrap_or(false);

        if !add_another {
            return Ok(());
        }
    }

    // Choose method.
    let methods = &["Google OAuth", "Email (OTP)"];
    let selection = Select::new()
        .with_prompt("Choose login method")
        .items(methods)
        .default(0)
        .interact()
        .unwrap_or(0);

    match selection {
        0 => {
            // Google OAuth
            fmt::info("Starting OAuth login...");
            let cred = auth.login_interactive_blocking(args.name.as_deref())?;
            fmt::success(&format!("Logged in as {}", cred.email));
            let _ = auth.set_default(&cred.name);
        }
        _ => {
            // OTP (placeholder — requires Supabase OTP integration).
            fmt::warning("Email OTP login is not yet implemented in the Rust version.");
            fmt::info("Please use Google OAuth instead.");
        }
    }

    auth.show_login_info();
    Ok(())
}
