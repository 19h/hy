//! Populate an empty instance registry after protocol-handler registration.

use std::path::PathBuf;

use serde_json::json;

use super::{Instances, best_default};
use crate::config::{ConfigStore, Env};
use crate::error::{Error, Result};
use crate::ida;
use crate::util::fmt;

pub(in crate::cmd) async fn ensure_registered() -> Result<()> {
    if report_existing(&ConfigStore::global())? {
        return Ok(());
    }
    let discovered = ida::find_standard_installations().await;
    match register_discovered(&mut ConfigStore::global(), discovered) {
        Err(Error::Io(error)) => {
            // The handler is already registered; upstream reports discovery and
            // configuration I/O failures as advisory at this stage.
            fmt::warning(&format!("Could not auto-discover IDA installations: {error}"));
            setup_instructions();
            Ok(())
        }
        result => result,
    }
}

fn report_existing(store: &ConfigStore) -> Result<bool> {
    let instances: Instances = store
        .get_value("ida.instances")
        .filter(|value| !value.is_null())
        .cloned()
        .map(serde_json::from_value)
        .transpose()?
        .unwrap_or_default();
    if instances.is_empty() {
        return Ok(false);
    }
    fmt::info(&format!("Found {} registered IDA instance(s).", instances.len()));
    match store.get_str("ida.default").filter(|name| !name.is_empty()) {
        Some(name) => fmt::info(&format!("Default IDA instance: {name}")),
        None => fmt::warning(&format!(
            "No default IDA instance set. Use `{} ida switch` to set one.",
            Env::global().binary_name
        )),
    }
    Ok(true)
}

fn register_discovered(store: &mut ConfigStore, paths: Vec<PathBuf>) -> Result<()> {
    let mut instances = Instances::new();
    for path in paths {
        if !path.is_dir() || ida::ida_binary_path(&path).is_none() {
            continue;
        }
        let name = ida::generate_instance_name(&path);
        if !instances.contains_key(&name) {
            instances.insert(name, path.canonicalize()?.to_string_lossy().into_owned());
        }
    }
    let Some(default) = best_default(&instances) else {
        fmt::warning("No IDA installations found.");
        setup_instructions();
        return Ok(());
    };
    store.set_values([
        ("ida.instances".into(), serde_json::to_value(&instances)?),
        ("ida.default".into(), json!(default)),
    ])?;
    fmt::success(&format!("Automatically registered {} IDA instance(s).", instances.len()));
    fmt::success(&format!("Default IDA instance set to: {default}"));
    Ok(())
}

fn setup_instructions() {
    let binary = &Env::global().binary_name;
    fmt::info(&format!(
        "Use `{binary} ida add --auto` or `{binary} ida add <name> <path>`, then `{binary} ida switch`."
    ));
}

#[cfg(test)]
mod tests;
