//! `hy login` command.

use clap::Args;
use dialoguer::{Confirm, Input, Select};

use crate::auth::AuthService;
use crate::config::ConfigStore;
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
            // Email OTP
            let last_email = {
                let store = ConfigStore::global();
                store.get_str("login.email").map(String::from)
            };

            let email: String = Input::new()
                .with_prompt("Email address")
                .default(last_email.unwrap_or_default())
                .interact_text()
                .map_err(|_| crate::error::Error::Other("Cancelled".into()))?;

            if email.is_empty() {
                fmt::error("Email address is required.");
                return Ok(());
            }

            fmt::info(&format!("Sending OTP to {email}..."));

            if args.force {
                auth.logout_current();
            }

            auth.send_otp(&email)?;
            fmt::success("OTP sent. Check your email.");

            let otp: String = Input::new()
                .with_prompt("Enter the code received by email")
                .interact_text()
                .map_err(|_| crate::error::Error::Other("Cancelled".into()))?;

            match auth.verify_otp(&email, &otp, args.name.as_deref()) {
                Ok(cred) => {
                    fmt::success(&format!("Logged in as {}", cred.email));
                    let _ = auth.set_default(&cred.name);
                }
                Err(_) => {
                    fmt::error("Login failed. Invalid OTP.");
                    return Ok(());
                }
            }
        }
    }

    auth.show_login_info();
    Ok(())
}
