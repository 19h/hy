//! Agent-specific command order and installation scope.

use super::PLUGIN_ID;
use super::agent::{Agent, Kind, Scope};
use super::listings::{has_line, has_named, pi_installed, same_name};
use crate::error::Result;

pub(super) async fn run(agent: &Agent, scope: Scope) -> Result<()> {
    match agent.kind {
        Kind::Claude => claude(agent, scope).await,
        Kind::Codex => codex(agent).await,
        Kind::Copilot => copilot(agent).await,
        Kind::Pi => pi(agent, scope).await,
        Kind::Omp => omp(agent, scope).await,
    }
}

async fn claude(agent: &Agent, scope: Scope) -> Result<()> {
    let scope = scope.cli();
    let installed = agent.query_json(&["plugin", "list", "--json"]).await?;
    if installed.as_array().into_iter().flatten().any(|item| {
        item.get("id")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|id| same_name(id, PLUGIN_ID))
            && item.get("scope").and_then(serde_json::Value::as_str) == Some(scope)
    }) {
        return agent.checked(&["plugin", "update", PLUGIN_ID, "--scope", scope, "--yes"]).await;
    }
    let marketplaces = agent.query_json(&["plugin", "marketplace", "list", "--json"]).await?;
    if !has_named(&marketplaces, "HexRaysSA") {
        agent
            .checked(&[
                "plugin",
                "marketplace",
                "add",
                "HexRaysSA/claude-marketplace",
                "--scope",
                scope,
            ])
            .await?;
    }
    agent.checked(&["plugin", "install", PLUGIN_ID, "--scope", scope, "--yes"]).await
}

async fn codex(agent: &Agent) -> Result<()> {
    let installed = agent.query_json(&["plugin", "list", "--json"]).await?;
    if has_named(&installed, PLUGIN_ID) {
        return agent.checked(&["plugin", "marketplace", "upgrade", "HexRaysSA"]).await;
    }
    let marketplaces = agent.query_json(&["plugin", "marketplace", "list", "--json"]).await?;
    if has_named(&marketplaces, "HexRaysSA") {
        agent.checked(&["plugin", "marketplace", "upgrade", "HexRaysSA"]).await?;
    } else {
        agent.checked(&["plugin", "marketplace", "add", "HexRaysSA/codex-marketplace"]).await?;
    }
    agent.checked(&["plugin", "add", PLUGIN_ID]).await
}

async fn copilot(agent: &Agent) -> Result<()> {
    let installed =
        agent.query(&["plugin", "list"]).await?.is_some_and(|text| has_line(&text, "ida-mcp"));
    if installed {
        agent.checked(&["plugin", "marketplace", "update", "HexRaysSA"]).await?;
        return agent.checked(&["plugin", "update", "ida-mcp"]).await;
    }
    let marketplace = agent.query(&["plugin", "marketplace", "list"]).await?;
    if marketplace.is_some_and(|text| has_line(&text, "HexRaysSA")) {
        agent.checked(&["plugin", "marketplace", "update", "HexRaysSA"]).await?;
    } else {
        agent.checked(&["plugin", "marketplace", "add", "HexRaysSA/copilot-marketplace"]).await?;
    }
    agent.checked(&["plugin", "install", PLUGIN_ID]).await
}

async fn pi(agent: &Agent, scope: Scope) -> Result<()> {
    const SOURCE: &str = "git:github.com/HexRaysSA/ida-mcp@latest";
    let listing = agent.query(&["list", "--no-approve"]).await?;
    if listing.is_some_and(|text| pi_installed(&text, scope)) {
        agent.checked(&["update", "--extension", SOURCE]).await
    } else if matches!(scope, Scope::Local) {
        agent.checked(&["install", SOURCE, "--local"]).await
    } else {
        agent.checked(&["install", SOURCE]).await
    }
}

async fn omp(agent: &Agent, scope: Scope) -> Result<()> {
    let scope = scope.cli();
    let installed = agent.query_json(&["plugin", "list", "--json", "--scope", scope]).await?;
    if has_named(&installed, "ida-mcp") {
        agent.checked(&["plugin", "upgrade", "ida-mcp", "--scope", scope]).await
    } else {
        agent
            .checked(&["plugin", "install", "github:HexRaysSA/ida-mcp#latest", "--scope", scope])
            .await
    }
}

#[cfg(all(test, unix))]
mod tests;
