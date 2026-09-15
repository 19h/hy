//! Validate manual registrations and commit selected discoveries as one batch.

use std::path::PathBuf;

use serde_json::json;

use super::{AddArgs, Instances, best_default};
use crate::config::ConfigStore;
use crate::error::{Error, Result};
use crate::ida;
use crate::util::{fmt, tui};

pub(super) async fn add(args: AddArgs) -> Result<()> {
    if args.auto {
        return add_discovered().await;
    }
    let (Some(name), Some(path)) = (args.name.filter(|name| !name.is_empty()), args.path) else {
        return Err(Error::Other("Both NAME and PATH are required (or use --auto).".into()));
    };
    let path = crate::util::files::absolute_path(&path)?.canonicalize()?;
    if !path.is_dir() || ida::ida_binary_path(&path).is_none() {
        return Err(Error::Other(format!(
            "Invalid IDA installation directory: {}",
            path.display()
        )));
    }
    let mut instances = super::instances()?;
    if instances.contains_key(&name) {
        fmt::warning(&format!("Instance '{name}' already exists. Use remove first to replace it."));
        return Ok(());
    }
    instances.insert(name.clone(), path.to_string_lossy().into_owned());
    ConfigStore::global().set_value("ida.instances", serde_json::to_value(instances)?)?;
    fmt::success(&format!("Added instance '{name}' -> {}", path.display()));
    Ok(())
}

struct Candidate {
    name: String,
    path: PathBuf,
    registered: bool,
}

impl Candidate {
    fn label(&self) -> String {
        let suffix = if self.registered {
            " (already registered)"
        } else {
            ""
        };
        format!("{}{suffix}", self.path.display())
    }
}

async fn add_discovered() -> Result<()> {
    let mut instances = super::instances()?;
    let candidates: Vec<_> = ida::find_standard_installations()
        .await
        .into_iter()
        .filter(|path| ida::ida_binary_path(path).is_some())
        .map(|path| {
            let name = ida::generate_instance_name(&path);
            Candidate {
                registered: instances.contains_key(&name),
                name,
                path,
            }
        })
        .collect();
    if candidates.is_empty() {
        fmt::info("No valid IDA installations found in standard locations.");
        return Ok(());
    }
    let selected = select_candidates(&candidates)?;
    let mut added = Instances::new();
    for index in selected {
        let candidate = &candidates[index];
        // Two different discovered paths can generate the same name. The first wins.
        if instances.contains_key(&candidate.name) {
            continue;
        }
        let path = candidate.path.canonicalize()?.to_string_lossy().into_owned();
        instances.insert(candidate.name.clone(), path.clone());
        added.insert(candidate.name.clone(), path);
    }
    if added.is_empty() {
        fmt::info("No new installations selected.");
        return Ok(());
    }
    let mut changes = vec![("ida.instances".into(), serde_json::to_value(instances)?)];
    let mut store = ConfigStore::global();
    if store.get_str("ida.default").is_none_or(str::is_empty)
        && let Some(default) = best_default(&added)
    {
        changes.push(("ida.default".into(), json!(default)));
    }
    store.set_values(changes)?;
    fmt::success(&format!("Added {} IDA instance(s).", added.len()));
    Ok(())
}

fn select_candidates(candidates: &[Candidate]) -> Result<Vec<usize>> {
    let labels: Vec<_> = candidates.iter().map(Candidate::label).collect();
    let mut defaults: Vec<_> = candidates.iter().map(|candidate| !candidate.registered).collect();
    loop {
        let selected = dialoguer::MultiSelect::with_theme(&tui::theme())
            .with_prompt("Select installations to register")
            .items(&labels)
            .defaults(&defaults)
            .report(false)
            .interact_opt()
            .map_err(|error| Error::Other(error.to_string()))?;
        match selected {
            None => return Ok(Vec::new()),
            Some(indices) if !indices.is_empty() => return Ok(indices),
            Some(_) => {
                fmt::warning("Please select at least one installation");
                defaults.fill(false);
            }
        }
    }
}
