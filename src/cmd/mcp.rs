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
            source: "https://github.com/HexRaysSA/ida-mcp".into(),
            force: false,
            upgrade: true,
            editable: false,
            no_build_isolation: false,
            config: vec![],
        },
        &crate::plugin::PluginContext::default(),
    )
    .await?;

    let agents = Agent::discover();
    if agents.is_empty() {
        return Err(Error::Other(
            "no supported agent command found on PATH (claude, codex, copilot, pi, or omp)".into(),
        ));
    }
    let choices = agents.iter().map(Agent::label).collect::<Vec<_>>();
    let selection = tui::select("Select an agent:", &choices, 0)
        .ok_or_else(|| Error::Other("agent selection cancelled".into()))?;
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
