//! `hcli auth` command group: list, switch, default, key management.

use clap::{Args, Subcommand};
use dialoguer::Select;

use crate::auth::AuthService;
use crate::error::Result;
use crate::util::fmt;

#[derive(Debug, Subcommand)]
pub enum AuthCommands {
    /// List all stored credentials
    List,
    /// Switch the active credentials
    Switch,
    /// Set the default credentials
    Default(DefaultArgs),
    /// Manage API keys
    Key {
        #[command(subcommand)]
        command: KeyCommands,
    },
}

#[derive(Debug, Args)]
pub struct DefaultArgs {
    /// Name of the credentials to set as default
    pub name: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum KeyCommands {
    /// List API keys
    List,
    /// Create a new API key
    Create(KeyCreateArgs),
    /// Revoke an API key
    Revoke(KeyRevokeArgs),
    /// Install an API key as credentials
    Install(KeyInstallArgs),
}

#[derive(Debug, Args)]
pub struct KeyCreateArgs {
    /// Name for the new API key
    pub name: String,
}

#[derive(Debug, Args)]
pub struct KeyRevokeArgs {
    /// Name of the API key to revoke
    pub name: String,
}

#[derive(Debug, Args)]
pub struct KeyInstallArgs {
    /// Name for the credentials
    pub name: String,
    /// The API key token
    pub token: String,
}

pub async fn run(cmd: AuthCommands) -> Result<()> {
    match cmd {
        AuthCommands::List => run_list().await,
        AuthCommands::Switch => run_switch().await,
        AuthCommands::Default(args) => run_default(args).await,
        AuthCommands::Key { command } => run_key(command).await,
    }
}

async fn run_list() -> Result<()> {
    let mut auth = AuthService::global();
    auth.init(None);

    let creds = auth.list_credentials();
    if creds.is_empty() {
        fmt::warning("No credentials stored.");
        return Ok(());
    }

    let default_name = auth.default_name().map(String::from);

    eprintln!(
        "{:<20} {:<30} {:<12} {:<10}",
        "Name", "Email", "Type", "Status"
    );
    eprintln!("{}", "-".repeat(75));

    for c in &creds {
        let status = if default_name.as_deref() == Some(c.name.as_str()) {
            "Default"
        } else {
            "-"
        };
        eprintln!(
            "{:<20} {:<30} {:<12} {:<10}",
            c.name, c.email, c.cred_type, status
        );
    }

    Ok(())
}

async fn run_switch() -> Result<()> {
    let mut auth = AuthService::global();
    auth.init(None);

    let creds = auth.list_credentials();
    if creds.len() < 2 {
        fmt::warning("Need at least 2 credentials to switch.");
        return Ok(());
    }

    let items: Vec<String> = creds
        .iter()
        .map(|c| format!("{} - {} ({})", c.name, c.email, c.cred_type))
        .collect();

    let selection = Select::new()
        .with_prompt("Switch to")
        .items(&items)
        .interact_opt()
        .unwrap_or(None);

    if let Some(idx) = selection {
        let name = creds[idx].name.clone();
        if auth.set_default(&name) {
            fmt::success(&format!("Switched to '{name}'."));
        }
    }

    Ok(())
}

async fn run_default(args: DefaultArgs) -> Result<()> {
    let mut auth = AuthService::global();
    auth.init(None);

    if let Some(name) = args.name {
        if auth.set_default(&name) {
            fmt::success(&format!("Default set to '{name}'."));
        } else {
            fmt::error(&format!("Credentials '{name}' not found."));
        }
    } else if let Some(name) = auth.default_name() {
        eprintln!("Current default: {name}");
    } else {
        eprintln!("No default credentials set.");
    }

    Ok(())
}

async fn run_key(cmd: KeyCommands) -> Result<()> {
    let client = crate::api::ApiClient::new()?;

    match cmd {
        KeyCommands::List => {
            let keys: Vec<crate::api::ApiKey> = client.get_json("/api/keys").await?;
            if keys.is_empty() {
                fmt::warning("No API keys found.");
                return Ok(());
            }
            eprintln!(
                "{:<20} {:<25} {:<25} {}",
                "Name", "Created", "Last Used", "Requests"
            );
            eprintln!("{}", "-".repeat(80));
            for k in &keys {
                eprintln!(
                    "{:<20} {:<25} {:<25} {}",
                    k.name,
                    fmt::format_datetime(&k.created_at),
                    k.last_used_at
                        .as_deref()
                        .map(fmt::format_datetime)
                        .unwrap_or_else(|| "Never".into()),
                    k.request_count
                );
            }
        }
        KeyCommands::Create(args) => {
            let body = serde_json::json!({ "name": args.name });
            let token: crate::api::ApiKeyToken = client.post_json("/api/keys", &body).await?;
            fmt::success(&format!("API key created: {}", token.key));
            fmt::warning("Save this key — it will not be shown again.");
        }
        KeyCommands::Revoke(args) => {
            let empty = serde_json::json!({});
            let _: serde_json::Value = client
                .post_json(&format!("/api/keys/revoke/{}", args.name), &empty)
                .await?;
            fmt::success(&format!("API key '{}' revoked.", args.name));
        }
        KeyCommands::Install(args) => {
            let mut auth = AuthService::global();
            auth.init(None);
            // Validate the token by calling whoami.
            // (In a full implementation we'd temporarily use this key to call /api/whoami.)
            let cred = auth.add_api_key_credential(&args.name, &args.token, "api-key-user");
            fmt::success(&format!(
                "Credentials '{}' installed.",
                cred.name
            ));
        }
    }

    Ok(())
}
