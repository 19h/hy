//! Plan a command's dependency graph, then apply it in dependency order.

use std::collections::{HashMap, HashSet};

use super::plugin_cmd::PluginInstallArgs;
use super::plugin_ops::{
    self, InstallAction,
    source::{self, Source},
};
use crate::error::{Error, Result};
use crate::plugin::repositories::Repositories;
use crate::plugin::requirements::{self, Requirement};
use crate::plugin::resolve::{self, Cell, Resolution};
use crate::plugin::{self, InstallationSource, PluginContext, PluginMetadata, index};

pub(super) async fn cell(
    snapshot: &index::Snapshot,
    roots: &[Requirement],
    fixed: &[PluginMetadata],
) -> Result<Cell> {
    let mut cell = Cell {
        platform: crate::ida::current_ida_platform()?,
        ida_version: plugin_ops::ida_version(),
        python_version: None,
        check_python: false,
    };
    if resolve::needs_python(snapshot, roots, fixed, &cell) {
        cell.python_version = crate::ida::python::full_version().await?;
    }
    cell.check_python = true;
    Ok(cell)
}

pub(super) fn installed(exclude: &[String]) -> Result<Vec<PluginMetadata>> {
    Ok(plugin::installed_plugins()?
        .into_iter()
        .map(|record| record.metadata)
        .filter(|metadata| !exclude.iter().any(|name| name.eq_ignore_ascii_case(&metadata.name)))
        .collect())
}

pub(super) async fn install(args: PluginInstallArgs, context: &PluginContext) -> Result<()> {
    for argument in &args.config {
        if !argument.contains('=') {
            return Err(Error::PluginInstall(format!(
                "invalid config format: {argument}, expected key=value"
            )));
        }
    }
    match source::classify(&args.source, args.editable)? {
        Source::Repository => {
            let reference = index::parse_reference(&args.source)?;
            let mut scopes = HashMap::new();
            if context.repository.is_none() {
                let (_, default) = index::repositories()?;
                scopes.insert(
                    reference.name.to_lowercase(),
                    reference.repo.clone().unwrap_or(default),
                );
            } else if reference.repo.is_some() {
                return Err(Error::PluginInstall(
                    "repository prefixes cannot be combined with --repo".into(),
                ));
            }
            let pool = Repositories::load(context, &scopes, None).await?;
            let roots = [Requirement::parse(&args.source)?];
            let cell = cell(&pool.snapshot, &roots, &[]).await?;
            let plan = resolve::resolve(
                &pool.snapshot,
                &roots,
                &cell,
                &[],
                &installed(std::slice::from_ref(&reference.name))?,
            )?;
            apply(
                &pool,
                plan,
                &roots.iter().map(|root| root.name.clone()).collect::<Vec<_>>(),
                &args,
                context,
            )
            .await
        }
        Source::Directory(path) => {
            install_local(InstallationSource::directory(&path, args.editable)?, &args, context)
                .await
        }
        source => {
            let bytes = match source {
                Source::Archive(path) => std::fs::read(path)?,
                Source::Download => index::fetch(&args.source).await?,
                Source::GitHub => index::github_archive(&args.source).await?,
                _ => unreachable!("handled local and repository sources"),
            };
            let temporary = tempfile::Builder::new().suffix(".zip").tempfile()?;
            std::fs::write(temporary.path(), &bytes)?;
            if plugin::bundle::is_plugin_bundle_zip(temporary.path()) {
                let mut pool = Repositories::empty();
                pool.add(index::load(&temporary.path().to_string_lossy(), true).await?, |_| true)?;
                let roots: Vec<_> = pool
                    .snapshot
                    .plugins
                    .iter()
                    .map(|plugin| Requirement {
                        name: plugin.name.clone(),
                        spec: String::new(),
                        host: Some(plugin.host.clone()),
                        required: true,
                    })
                    .collect();
                let cell = cell(&pool.snapshot, &roots, &[]).await?;
                let names = roots.iter().map(|root| root.name.clone()).collect::<Vec<_>>();
                let plan =
                    resolve::resolve(&pool.snapshot, &roots, &cell, &[], &installed(&names)?)?;
                apply(&pool, plan, &names, &args, context).await
            } else {
                install_local(InstallationSource::archive(bytes)?, &args, context).await
            }
        }
    }
}

struct Root {
    source: InstallationSource,
    metadata: PluginMetadata,
    location: Option<index::Location>,
}

fn prepare_root(
    mut source: InstallationSource,
    selected_name: Option<&str>,
    location: Option<index::Location>,
    args: &PluginInstallArgs,
) -> Result<Option<Root>> {
    let metadata = source.metadata(selected_name)?;
    check_existing(&metadata, args)?;
    if let InstallAction::Keep(installed) = plugin_ops::installation_action(&metadata, args)? {
        crate::util::fmt::success(&format!(
            "Already installed {}=={}",
            installed.name, installed.version
        ));
        return Ok(None);
    }
    Ok(Some(Root {
        source,
        metadata,
        location,
    }))
}

async fn install_local(
    source: InstallationSource,
    args: &PluginInstallArgs,
    context: &PluginContext,
) -> Result<()> {
    let Some(root) = prepare_root(source, None, None, args)? else {
        return Ok(());
    };
    let pool = if requirements::collect(&root.metadata).is_empty() {
        Repositories::empty()
    } else {
        Repositories::load(context, &HashMap::new(), None).await?
    };
    apply_fixed(&pool, vec![root], args, context).await
}

async fn apply_fixed(
    pool: &Repositories,
    roots: Vec<Root>,
    args: &PluginInstallArgs,
    context: &PluginContext,
) -> Result<()> {
    if roots.is_empty() {
        return Ok(());
    }
    let fixed: Vec<_> = roots.iter().map(|root| root.metadata.clone()).collect();
    let names: Vec<_> = fixed.iter().map(|metadata| metadata.name.clone()).collect();
    let cell = cell(&pool.snapshot, &[], &fixed).await?;
    for metadata in &fixed {
        if let Some(reason) = cell.incompatibility(metadata) {
            return Err(Error::PluginInstall(format!(
                "{} {} {reason}",
                metadata.name, metadata.version
            )));
        }
    }
    let plan = resolve::resolve(&pool.snapshot, &[], &cell, &fixed, &installed(&names)?)?;
    let required = required_names(&fixed, &plan);
    apply_dependencies(pool, &plan, &required, args, context).await?;
    let targets = configuration_targets(&plan, &fixed);
    for root in roots {
        let bundle = root.location.as_ref().map(|location| pool.source(location)).transpose()?;
        plugin_ops::install_local(
            root.source,
            &scoped_arguments(args, &root.metadata, true, &targets),
            bundle.and_then(index::LoadedRepository::bundle_reader),
            context,
            Some(&root.metadata.name),
        )
        .await?;
    }
    Ok(())
}

fn check_existing(metadata: &PluginMetadata, args: &PluginInstallArgs) -> Result<()> {
    if !args.force
        && !args.upgrade
        && !args.editable
        && plugin::read_installed_metadata(&metadata.name).is_ok()
    {
        return Err(Error::PluginAlreadyInstalled(metadata.name.clone()));
    }
    plugin::components::validate_collisions(metadata, args.force || args.upgrade || args.editable)
}

pub(super) async fn apply(
    pool: &Repositories,
    plan: Resolution,
    roots: &[String],
    args: &PluginInstallArgs,
    context: &PluginContext,
) -> Result<()> {
    let mut prepared = Vec::new();
    for name in &plan.order {
        if !roots.iter().any(|root| root.eq_ignore_ascii_case(name)) {
            continue;
        }
        let location = &plan.selected[name];
        let source = InstallationSource::archive(pool.fetch(location).await?)?;
        if let Some(root) = prepare_root(
            source,
            Some(&location.descriptor.metadata.name),
            Some(location.clone()),
            args,
        )? {
            prepared.push(root);
        }
    }
    apply_fixed(pool, prepared, args, context).await
}

async fn apply_dependencies(
    pool: &Repositories,
    plan: &Resolution,
    required: &HashSet<String>,
    args: &PluginInstallArgs,
    context: &PluginContext,
) -> Result<()> {
    for warning in plan.warnings.iter().chain(&plan.skipped) {
        crate::util::fmt::warning(warning);
    }
    let targets = configuration_targets(plan, &[]);
    let mut failed = HashSet::new();
    for name in &plan.order {
        let location = &plan.selected[name];
        let metadata = &location.descriptor.metadata;
        if requirements::collect(metadata).iter().any(|requirement| {
            requirement.required && failed.contains(&requirement.name.to_lowercase())
        }) {
            failed.insert(name.to_lowercase());
            crate::util::fmt::warning(&format!(
                "skipping optional plugin {name}: a required dependency failed to install"
            ));
            continue;
        }
        let mut arguments = scoped_arguments(args, metadata, false, &targets);
        arguments.upgrade = true;
        arguments.force = false;
        arguments.editable = false;
        let result = async {
            let source = InstallationSource::archive(pool.fetch(location).await?)?;
            plugin_ops::install_local(
                source,
                &arguments,
                pool.source(location)?.bundle_reader(),
                context,
                Some(&metadata.name),
            )
            .await
        }
        .await;
        if let Err(error) = result {
            if required.contains(&name.to_lowercase()) {
                return Err(error);
            }
            crate::util::fmt::warning(&format!("skipping optional plugin {name}: {error}"));
            failed.insert(name.to_lowercase());
        }
    }
    Ok(())
}

fn required_names(fixed: &[PluginMetadata], plan: &Resolution) -> HashSet<String> {
    let mut required = HashSet::new();
    let mut pending = Vec::new();
    pending.extend(
        fixed
            .iter()
            .flat_map(requirements::collect)
            .filter(|entry| entry.required)
            .map(|entry| entry.name),
    );
    while let Some(name) = pending.pop() {
        if !required.insert(name.to_lowercase()) {
            continue;
        }
        if let Some((_, location)) =
            plan.selected.iter().find(|(selected, _)| selected.eq_ignore_ascii_case(&name))
        {
            pending.extend(
                requirements::collect(&location.descriptor.metadata)
                    .into_iter()
                    .filter(|entry| entry.required)
                    .map(|entry| entry.name),
            );
        }
    }
    required
}

fn scoped_arguments(
    args: &PluginInstallArgs,
    metadata: &PluginMetadata,
    root: bool,
    targets: &HashSet<String>,
) -> PluginInstallArgs {
    let names: Vec<_> = plugin::components::metadata_tree(metadata)
        .into_iter()
        .map(|metadata| metadata.name.as_str())
        .collect();
    let config = args
        .config
        .iter()
        .filter_map(|argument| {
            let (key, _) = argument.split_once('=')?;
            match key.split_once('.') {
                Some((name, _))
                    if names.iter().any(|candidate| candidate.eq_ignore_ascii_case(name)) =>
                {
                    Some(argument.clone())
                }
                None if root => Some(argument.clone()),
                Some((name, _)) if root && !targets.contains(&name.to_lowercase()) => {
                    Some(argument.clone())
                }
                _ => None,
            }
        })
        .collect();
    PluginInstallArgs {
        source: args.source.clone(),
        force: args.force,
        upgrade: args.upgrade,
        editable: args.editable,
        no_build_isolation: args.no_build_isolation,
        config,
    }
}

fn configuration_targets(plan: &Resolution, fixed: &[PluginMetadata]) -> HashSet<String> {
    fixed
        .iter()
        .chain(plan.selected.values().map(|location| &location.descriptor.metadata))
        .flat_map(plugin::components::metadata_tree)
        .map(|metadata| metadata.name.to_lowercase())
        .collect()
}
