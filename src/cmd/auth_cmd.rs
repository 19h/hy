//! `hy auth` command group: list, switch, default, key management.

use clap::{Args, Subcommand};
use dialoguer::{Confirm, Input, Password, Select};

use crate::auth::AuthService;
use crate::error::Result;
use crate::util::fmt;

#[derive(Debug, Subcommand)]
pub enum AuthCommands {
    /// List all stored credentials
    List,
    /// Switch the active credentials
    Switch(SwitchArgs),
    /// Set the default credentials
    Default(DefaultArgs),
    /// Manage API keys
    Key {
        #[command(subcommand)]
        command: KeyCommands,
    },
}

#[derive(Debug, Args)]
pub struct SwitchArgs {
    /// Name of the credentials to switch to (interactive if omitted)
    pub name: Option<String>,
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
    Revoke,
    /// Install an API key as credentials
    Install(KeyInstallArgs),
}

#[derive(Debug, Args)]
pub struct KeyCreateArgs {
    /// Name for the new API key
    #[arg(short, long)]
    pub name: Option<String>,
}

#[derive(Debug, Args)]
pub struct KeyInstallArgs {
    /// The API key token
    #[arg(short, long)]
    pub key: Option<String>,
    /// Name for the credentials
    #[arg(short, long)]
    pub name: Option<String>,
    /// Set as default credentials
    #[arg(long)]
    pub set_default: bool,
}

pub async fn run(cmd: AuthCommands) -> Result<()> {
    match cmd {
        AuthCommands::List => run_list().await,
        AuthCommands::Switch(args) => run_switch(args).await,
        AuthCommands::Default(args) => run_default(args).await,
        AuthCommands::Key { command } => run_key(command).await,
    }
}

// ── auth list ───────────────────────────────────────────────────────────

async fn run_list() -> Result<()> {
    let mut auth = AuthService::global();
    auth.init(None);

    let creds = auth.list_credentials();
    if creds.is_empty() {
        fmt::warning("No credentials found.");
        eprintln!("Use 'hy login' or 'hy auth key install' to add credentials.");
        return Ok(());
    }

    let default_name = auth.default_name().map(String::from);
    let current_name = auth
        .current_credentials()
        .map(|c| c.name.clone());

    eprintln!("Credentials ({}):\n", creds.len());

    // Sort: default first, then by name.
    let mut sorted: Vec<_> = creds.into_iter().collect();
    sorted.sort_by(|a, b| {
        let a_default = default_name.as_deref() == Some(a.name.as_str());
        let b_default = default_name.as_deref() == Some(b.name.as_str());
        b_default.cmp(&a_default).then(a.name.cmp(&b.name))
    });

    let mut table = crate::util::tui::Table::new(&["Label", "Type", "Status", "Created", "Last Used"]);
    for c in &sorted {
        let mut status_parts = Vec::new();
        if current_name.as_deref() == Some(c.name.as_str()) {
            status_parts.push("Active");
        }
        if default_name.as_deref() == Some(c.name.as_str()) {
            status_parts.push("Default");
        }
        let status = if status_parts.is_empty() {
            "-".to_string()
        } else {
            use owo_colors::OwoColorize;
            status_parts.join(", ").green().to_string()
        };

        table.add_row(vec![
            c.label(),
            format!("{}", c.cred_type),
            status,
            fmt::format_datetime(&c.created_at.to_rfc3339()),
            fmt::format_datetime(&c.last_used.to_rfc3339()),
        ]);
    }
    table.print();

    eprintln!();
    if let Some(ref name) = current_name {
        let email = sorted
            .iter()
            .find(|c| c.name == *name)
            .map(|c| c.email.as_str())
            .unwrap_or("?");
        fmt::success(&format!("Current: {name} ({email})"));
    } else {
        fmt::error("No active credentials");
    }
    if let Some(ref name) = default_name {
        fmt::info(&format!("Default: {name}"));
    } else {
        fmt::warning("No default credentials set");
    }

    Ok(())
}

// ── auth switch ─────────────────────────────────────────────────────────

async fn run_switch(args: SwitchArgs) -> Result<()> {
    let mut auth = AuthService::global();
    auth.init(None);

    let creds = auth.list_credentials();
    if creds.is_empty() {
        fmt::warning("No credentials found.");
        eprintln!("Use 'hy login' or 'hy auth key install' to add credentials.");
        return Ok(());
    }

    if creds.len() < 2 {
        let c = creds[0];
        fmt::warning("Only one credentials available. No switching needed.");
        eprintln!("Current source: {} ({})", c.name, c.label());
        return Ok(());
    }

    let default_name = auth.default_name().map(String::from);

    // Non-interactive path: name argument given.
    if let Some(ref name) = args.name {
        let names: Vec<String> = creds.iter().map(|c| c.name.clone()).collect();
        if !names.iter().any(|n| n == name) {
            fmt::error(&format!("Credentials '{name}' not found."));
            eprintln!("Available credentials: {}", names.join(", "));
            return Ok(());
        }
        if default_name.as_deref() == Some(name.as_str()) {
            fmt::warning(&format!("'{name}' is already the default credentials."));
            return Ok(());
        }
        if auth.set_default(name) {
            fmt::success(&format!("Switched default credentials to '{name}'."));
            auth.show_login_info();
        } else {
            fmt::error(&format!("Failed to switch to credentials '{name}'."));
        }
        return Ok(());
    }

    // Interactive selection.
    let items: Vec<String> = creds
        .iter()
        .map(|c| format!("{} ({})", c.label(), c.cred_type))
        .collect();

    let default_idx = creds
        .iter()
        .position(|c| default_name.as_deref() == Some(c.name.as_str()))
        .unwrap_or(0);

    let selection = Select::new()
        .with_prompt("Select new default credentials")
        .items(&items)
        .default(default_idx)
        .interact_opt()
        .unwrap_or(None);

    let Some(idx) = selection else {
        fmt::warning("Switch cancelled.");
        return Ok(());
    };

    let name = creds[idx].name.clone();
    if auth.set_default(&name) {
        fmt::success(&format!("Switched default credentials to '{name}'."));
        eprintln!();
        auth.show_login_info();
    } else {
        fmt::error(&format!("Failed to switch to credentials '{name}'."));
    }

    Ok(())
}

// ── auth default ────────────────────────────────────────────────────────

async fn run_default(args: DefaultArgs) -> Result<()> {
    let mut auth = AuthService::global();
    auth.init(None);

    if let Some(name) = args.name {
        // Set new default.
        let creds = auth.list_credentials();
        if creds.is_empty() {
            fmt::warning("No credentials found.");
            eprintln!("Use 'hy login' or 'hy auth key install' to add credentials.");
            return Ok(());
        }

        let names: Vec<String> = creds.iter().map(|c| c.name.clone()).collect();
        if !names.contains(&name) {
            fmt::error(&format!("Credentials '{name}' not found."));
            eprintln!("Available credentials: {}", names.join(", "));
            return Ok(());
        }

        // Check if already default.
        if auth.default_name() == Some(name.as_str()) {
            fmt::warning(&format!("'{name}' is already the default credentials."));
            return Ok(());
        }

        if auth.set_default(&name) {
            fmt::success(&format!("Set '{name}' as the default credentials."));
            eprintln!();
            auth.show_login_info();
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
                fmt::warning(&format!(
                    "Default set to '{name}' but source not found."
                ));
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

    Ok(())
}

// ── key subcommands ─────────────────────────────────────────────────────

async fn run_key(cmd: KeyCommands) -> Result<()> {
    match cmd {
        KeyCommands::List => run_key_list().await,
        KeyCommands::Create(args) => run_key_create(args).await,
        KeyCommands::Revoke => run_key_revoke().await,
        KeyCommands::Install(args) => run_key_install(args).await,
    }
}

async fn run_key_list() -> Result<()> {
    let client = crate::api::ApiClient::new()?;
    let keys: Vec<crate::api::ApiKey> = client.get_json("/api/keys").await?;
    if keys.is_empty() {
        fmt::warning("No API keys found.");
        return Ok(());
    }
    let mut table = crate::util::tui::Table::new(&["Name", "Created", "Last Used", "Requests"]);
    for k in &keys {
        table.add_row(vec![
            k.name.clone(),
            fmt::format_datetime(&k.created_at),
            k.last_used_at
                .as_deref()
                .map(fmt::format_datetime)
                .unwrap_or_else(|| "Never".into()),
            k.request_count.to_string(),
        ]);
    }
    table.print();
    Ok(())
}

async fn run_key_create(args: KeyCreateArgs) -> Result<()> {
    let client = crate::api::ApiClient::new()?;

    fmt::warning("The key will be displayed only once, so make sure to save it in a secure place.");

    // Get name from arg or prompt.
    let key_name = if let Some(name) = args.name {
        name
    } else {
        Input::new()
            .with_prompt("Enter the name for this key")
            .default("hy".into())
            .interact_text()
            .map_err(|_| crate::error::Error::Other("Cancelled".into()))?
    };

    if key_name.is_empty() {
        fmt::error("Key name is required.");
        return Ok(());
    }

    // Check for duplicate name.
    let existing_keys: Vec<crate::api::ApiKey> = client.get_json("/api/keys").await?;
    if existing_keys.iter().any(|k| k.name == key_name) {
        fmt::error(&format!(
            "An API key with name '{key_name}' already exists."
        ));
        return Ok(());
    }

    // Confirm creation.
    let confirmed = Confirm::new()
        .with_prompt(format!("Do you want to create a new API key '{key_name}'?"))
        .default(true)
        .interact()
        .unwrap_or(false);

    if !confirmed {
        fmt::warning("Key creation cancelled.");
        return Ok(());
    }

    // Create the key.
    fmt::info(&format!("Creating API key '{key_name}'..."));
    let body = serde_json::json!({ "name": key_name });
    let token: crate::api::ApiKeyToken = client.post_json("/api/keys", &body).await?;
    fmt::success(&format!("API key created: {}", token.key));

    // Offer to install as credentials.
    let install = Confirm::new()
        .with_prompt("Do you want to use this key for hy?")
        .default(true)
        .interact()
        .unwrap_or(false);

    if install {
        fmt::info("Installing API key as credentials...");
        let email =
            match crate::api::ApiClient::validate_api_key(&token.key).await {
                Ok(email) => email,
                Err(e) => {
                    fmt::error(&format!("Failed to validate API key: {e}"));
                    return Ok(());
                }
            };

        let mut auth = AuthService::global();
        auth.init(None);
        let cred = auth.add_api_key_credential(&key_name, &token.key, &email);

        let creds_count = auth.list_credentials().len();
        if creds_count <= 1 {
            // First credential — auto set default.
            fmt::success(&format!(
                "API key credentials installed for {}",
                cred.email
            ));
        } else {
            fmt::success(&format!(
                "API key credentials '{}' created!",
                cred.name
            ));
            eprintln!("Email: {}", cred.email);

            // Ask about default.
            let set_default = Confirm::new()
                .with_prompt(format!(
                    "Set '{}' as the default credentials?",
                    cred.name
                ))
                .default(true)
                .interact()
                .unwrap_or(false);

            if set_default {
                auth.set_default(&cred.name);
                fmt::success(&format!(
                    "'{}' set as default credentials.",
                    cred.name
                ));
            }
        }
    } else {
        fmt::warning("Save this key -- it will not be shown again.");
    }

    Ok(())
}

async fn run_key_revoke() -> Result<()> {
    let client = crate::api::ApiClient::new()?;

    // Fetch all keys.
    let keys: Vec<crate::api::ApiKey> = client.get_json("/api/keys").await?;
    if keys.is_empty() {
        fmt::warning("No API keys found to revoke.");
        return Ok(());
    }

    // Build picker items.
    let items: Vec<String> = keys
        .iter()
        .map(|k| {
            let created = &k.created_at[..10.min(k.created_at.len())];
            format!(
                "{} (Created: {}, Requests: {})",
                k.name, created, k.request_count
            )
        })
        .collect();

    let selection = Select::new()
        .with_prompt("Select API key to revoke")
        .items(&items)
        .interact_opt()
        .unwrap_or(None);

    let Some(idx) = selection else {
        fmt::warning("Operation cancelled.");
        return Ok(());
    };

    let selected = &keys[idx];

    // Confirm revocation.
    let confirmed = Confirm::new()
        .with_prompt(format!(
            "Do you want to revoke the key named '{}'?",
            selected.name
        ))
        .default(false)
        .interact()
        .unwrap_or(false);

    if !confirmed {
        fmt::warning("Revocation cancelled.");
        return Ok(());
    }

    fmt::info(&format!("Revoking API key '{}'...", selected.name));
    let empty = serde_json::json!({});
    let _: serde_json::Value = client
        .post_json(&format!("/api/keys/revoke/{}", selected.name), &empty)
        .await?;
    fmt::success(&format!("API key '{}' has been revoked.", selected.name));

    Ok(())
}

async fn run_key_install(args: KeyInstallArgs) -> Result<()> {
    let mut auth = AuthService::global();
    auth.init(None);

    // Show current status.
    if let Some(c) = auth.current_credentials() {
        eprintln!("Current credentials: {} ({})", c.name, c.email);
    }

    // Get key from arg or prompt.
    let key = if let Some(k) = args.key {
        k
    } else {
        Password::new()
            .with_prompt("Enter API key")
            .interact()
            .map_err(|_| crate::error::Error::Other("Cancelled".into()))?
    };

    if key.is_empty() {
        fmt::error("No API key provided.");
        return Ok(());
    }

    // Validate the key by calling /api/whoami.
    fmt::info("Validating API key...");
    let email = match crate::api::ApiClient::validate_api_key(&key).await {
        Ok(email) => email,
        Err(e) => {
            fmt::error(&format!(
                "Failed to validate API key or get user information: {e}"
            ));
            return Ok(());
        }
    };

    // Get name from arg or prompt.
    let name = if let Some(n) = args.name {
        n
    } else {
        Input::new()
            .with_prompt("API Key name")
            .default("hy".into())
            .interact_text()
            .map_err(|_| crate::error::Error::Other("Cancelled".into()))?
    };

    // Install the credential.
    let cred = auth.add_api_key_credential(&name, &key, &email);
    fmt::success(&format!("API key '{}' created!", cred.name));
    eprintln!("Email: {}", cred.email);
    eprintln!("Type: {}", cred.cred_type);

    // Handle default setting.
    let creds = auth.list_credentials();
    if creds.len() <= 1 {
        // First source — automatically default.
        fmt::success("Set as default credentials.");
    } else if args.set_default {
        auth.set_default(&cred.name);
        fmt::success(&format!(
            "'{}' set as default credentials.",
            cred.label()
        ));
    } else {
        let set_default = Confirm::new()
            .with_prompt(format!(
                "Use '{}' as the default credentials?",
                cred.label()
            ))
            .default(true)
            .interact()
            .unwrap_or(false);

        if set_default {
            auth.set_default(&cred.name);
            fmt::success(&format!(
                "'{}' set as default credentials.",
                cred.label()
            ));
        } else if let Some(default) = auth.default_name() {
            eprintln!("Default credentials remains: {default}");
        }
    }

    eprintln!();
    auth.show_login_info();

    Ok(())
}
