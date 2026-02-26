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
        let sources = auth.list_credentials();
        let current = auth.current_credentials();

        if current.is_none() {
            fmt::error("No valid credentials found.");
            return Ok(());
        }
        let current = current.unwrap();

        if sources.len() <= 1 {
            // Single credential — simplified prompt.
            fmt::success(&format!(
                "You are already logged in as {}.",
                current.email
            ));
            let add_another = Confirm::new()
                .with_prompt("Would you like to login as another user?")
                .default(false)
                .interact()
                .unwrap_or(false);
            if !add_another {
                return Ok(());
            }
        } else {
            // Multiple credentials — more detailed.
            fmt::success("You are already logged in.");
            eprintln!(
                "Current source: {} ({})",
                current.name, current.email
            );
            let add_another = Confirm::new()
                .with_prompt("Would you like to add another credentials?")
                .default(false)
                .interact()
                .unwrap_or(false);
            if !add_another {
                return Ok(());
            }
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

    // Remember credential count before login for default-setting logic.
    let creds_before = auth.list_credentials().len();

    let cred = match selection {
        0 => {
            // Google OAuth
            fmt::info("Starting OAuth login...");
            let cred = auth.login_interactive_blocking(args.name.as_deref())?;
            fmt::success(&format!("Logged in as {}", cred.email));
            Some(cred)
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
                    Some(cred)
                }
                Err(_) => {
                    fmt::error("Login failed. Invalid OTP.");
                    return Ok(());
                }
            }
        }
    };

    // Handle default credential setting.
    if let Some(ref cred) = cred {
        if creds_before == 0 {
            // First login — automatically set as default.
            let _ = auth.set_default(&cred.name);
        } else {
            // Additional credentials — ask before overwriting default.
            fmt::success(&format!(
                "Credentials '{}' created!",
                cred.label()
            ));
            eprintln!("Email: {}", cred.email);
            eprintln!("Type: {}", cred.cred_type);

            let set_default = Confirm::new()
                .with_prompt(format!(
                    "Set '{}' as the default credentials?",
                    cred.name
                ))
                .default(true)
                .interact()
                .unwrap_or(false);

            if set_default {
                let _ = auth.set_default(&cred.name);
                fmt::success(&format!(
                    "'{}' set as default credentials.",
                    cred.name
                ));
            }

            eprintln!();
            auth.show_login_info();
        }
    } else {
        fmt::error("Login failed.");
    }

    Ok(())
}
