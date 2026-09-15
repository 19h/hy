//! Build and inspect portable plugin bundles.

use super::plugin_cmd::{BundleCommands, BundleCreateArgs, BundleInfoArgs};
use crate::error::{Error, Result};
use crate::plugin::{PluginContext, bundle};
use crate::util::{fmt, tui};

pub async fn run(command: BundleCommands, context: &PluginContext) -> Result<()> {
    match command {
        BundleCommands::Info(args) => info(args),
        BundleCommands::Create(args) => create(args, context).await,
    }
}

fn info(args: BundleInfoArgs) -> Result<()> {
    if !bundle::is_plugin_bundle_zip(&args.bundle_path) {
        return Err(Error::Other(format!("{} is not a plugin bundle", args.bundle_path.display())));
    }
    let reader = bundle::BundleReader::open(&args.bundle_path)?;
    let manifest = reader.manifest();
    println!("plugin bundle: {}", args.bundle_path.display());
    println!("  built: {}", manifest.built_at);
    println!("  created by: {} {}", manifest.created_by.tool, manifest.created_by.version);
    println!(
        "  targets: {}",
        manifest
            .target_platform_tags
            .iter()
            .map(|target| target.id.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );
    let plugins = reader.plugins()?;
    if plugins.is_empty() {
        println!("  plugins: (none)");
        return Ok(());
    }
    println!("  plugins: {}", plugins.len());
    for plugin in plugins {
        let mut versions: Vec<_> = plugin.versions.keys().map(String::as_str).collect();
        versions.sort();
        println!("    {}: {}", plugin.name, versions.join(", "));
    }
    Ok(())
}

async fn create(args: BundleCreateArgs, context: &PluginContext) -> Result<()> {
    let targets = super::plugin_bundle_targets::resolve(&args).await?;
    let mut platforms: Vec<_> = targets.iter().map(|target| target.ida_platform.clone()).collect();
    platforms.sort();
    platforms.dedup();
    let mut sources = super::plugin_bundle_sources::Sources::new(context, args.repo.as_deref());
    let mut archives = Vec::new();
    for spec in &args.plugin_specs {
        let spinner = tui::spinner(format!("resolving {spec}"));
        archives.extend(sources.resolve(spec, &platforms).await?);
        spinner.finish_and_clear();
    }
    let spinner = tui::spinner("building bundle");
    bundle::create_bundle(&args.output, &archives, &targets, &context.pip, |message| {
        spinner.set_message(message.to_owned());
    })
    .await?;
    spinner.finish_and_clear();
    fmt::success(&format!("Created plugin bundle: {}", args.output.display()));
    println!("  plugins: {}", args.plugin_specs.len());
    println!("  targets: {}", targets.len());
    for target in targets {
        println!("    {}  Python {}", target.ida_platform, target.python_version);
    }
    Ok(())
}
