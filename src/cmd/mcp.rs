//! Install the IDA MCP plugin, then configure a selected agent.

use clap::Subcommand;

use crate::error::{Error, Result};
use crate::util::tui;

mod agent;
mod install;
mod listings;

use agent::{Agent, Scope};

#[derive(Debug, Subcommand)]
pub enum McpCommands {
    Install,
}

const PLUGIN_ID: &str = "ida-mcp@HexRaysSA";

pub async fn run(_: McpCommands) -> Result<()> {
    super::plugin_ops::install(
        super::plugin_cmd::PluginInstallArgs {
            source: "ida-mcp@https://github.com/HexRaysSA/ida-mcp".into(),
            force: false,
            upgrade: true,
            editable: false,
            no_build_isolation: false,
            config: vec![],
        },
        &crate::plugin::PluginContext::default(),
    )
    .await?;

    if ["uv", "uvx"].iter().any(|name| crate::ida::python::find_command(name).is_none()) {
        return Err(Error::Other("uv and uvx are required to run the IDA MCP server: https://docs.astral.sh/uv/#installation".into()));
    }

    let agents = Agent::discover();
    if agents.is_empty() {
        crate::util::fmt::warning(
            "No supported agent command found on PATH (claude, codex, copilot, pi, or omp).",
        );
        return print_configuration();
    }
    let mut choices = agents.iter().map(Agent::label).collect::<Vec<_>>();
    choices.push("Other agent (mcp.json)".into());
    let selection = tui::select("Select an agent:", &choices, 0)
        .ok_or_else(|| Error::Other("agent selection cancelled".into()))?;
    if selection == agents.len() {
        return print_configuration();
    }
    let git = crate::ida::python::find_command("git")
        .ok_or_else(|| Error::Other("git is required to install agent integrations".into()))?;
    let mut probe = tokio::process::Command::new(git);
    probe.arg("--version").kill_on_drop(true);
    if !tokio::time::timeout(std::time::Duration::from_secs(10), probe.output())
        .await
        .is_ok_and(|result| result.is_ok_and(|output| output.status.success()))
    {
        return Err(Error::Other(
            "git is required to install agent integrations; verify with git --version".into(),
        ));
    }
    let agent = &agents[selection];
    let scope = if agent.kind.supports_local() {
        match tui::select(
            "Select installation scope:",
            &["Local (current repository)".into(), "Global (current user)".into()],
            1,
        ) {
            Some(0) => Scope::Local,
            Some(_) => Scope::Global,
            None => return Err(Error::Other("scope selection cancelled".into())),
        }
    } else {
        Scope::Global
    };
    install::run(agent, scope).await?;
    let location = match scope {
        Scope::Local => "in this repository",
        Scope::Global => "for the current user",
    };
    println!("Installed IDA MCP for {} {location}.", agent.kind.name());
    Ok(())
}

fn print_configuration() -> Result<()> {
    eprintln!("Add the following to your agent's MCP configuration, then restart it:");
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "mcpServers": {"ida": {"command": "uvx", "args": ["--exclude-newer=1s", "ida-mcp", "stdio"]}}
        }))?
    );
    Ok(())
}
