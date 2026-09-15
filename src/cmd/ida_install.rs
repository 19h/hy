//! Plan installation, acquire artifacts, and apply post-install configuration.

use std::path::{Path, PathBuf};

use serde_json::json;

use super::ida_cmd::IdaInstallArgs;
use crate::error::{Error, Result};
use crate::ida::{self, IdaProduct};
use crate::util::fmt;

pub async fn run(args: IdaInstallArgs) -> Result<()> {
    if args.download_id.is_some() || args.license_id.is_some() {
        crate::auth::request_headers(true).await?;
    }
    let temporary = tempfile::tempdir()?;
    let installer = match (&args.download_id, &args.installer) {
        (Some(reference), _) => {
            super::download::download_installer(reference, temporary.path()).await?
        }
        (None, Some(path)) => path.canonicalize()?,
        (None, None) => {
            return Err(Error::Other("provide an installer path or --download-id".into()));
        }
    };
    let product = IdaProduct::from_installer(&installer)?;
    let destination = match &args.prefix {
        Some(path) => absolute_destination(path)?,
        None => product.default_directory()?,
    };
    if std::fs::symlink_metadata(&destination).is_ok() {
        fmt::info(&format!(
            "Directory already exists: {}. Choose another --install-dir or remove it first.",
            destination.display()
        ));
        return Ok(());
    }
    let set_default = args.set_default && !args.no_set_default;
    let accept_eula = args.accept_eula && !args.no_accept_eula;
    println!("Installer: {}\nDestination: {}", installer.display(), destination.display());
    if let Some(license) = &args.license_id {
        println!("Install license: {license}");
    }
    println!("Set as default: {set_default}\nAccept EULA: {accept_eula}");
    if args.create_python_environment {
        println!("Create Python environment: {}", ida::ida_user_dir().join("venv").display());
    }
    if args.dry_run {
        println!("Dry run: installer execution and post-install configuration skipped.");
        return Ok(());
    }
    if cfg!(target_os = "linux")
        && product.major == 9
        && product.minor == 2
        && destination.to_string_lossy().contains(' ')
    {
        fmt::warning("IDA 9.2 idat cannot start from Linux installation paths containing spaces.");
        if !args.yes && !confirm("Continue with this installation path?")? {
            return Ok(());
        }
    }
    if !args.yes && !confirm("Proceed with installation?")? {
        return Ok(());
    }
    let installed = ida::install_ida(&installer, &destination).await?;
    if let Some(license) = &args.license_id {
        install_license(license, temporary.path(), &installed).await?;
    }
    if set_default {
        register_default(&installed)?;
    }
    if accept_eula {
        if ida::is_idalib_capable(&installed) {
            if let Err(error) = ida::accept_eula(&installed).await {
                fmt::warning(&format!("EULA acceptance skipped: {error}"));
            }
        } else {
            fmt::info("EULA acceptance skipped: this installation does not include idalib.");
        }
    }
    if args.create_python_environment {
        use std::io::IsTerminal;
        ida::python::create_environment(ida::python::CreateOptions {
            ida_installation: Some(installed.clone()),
            path: None,
            python_version: None,
            configure: true,
            reinstall_plugins: true,
            interactive: !args.yes && std::io::stdin().is_terminal(),
            quiet: false,
        })
        .await?;
    }
    fmt::success(&format!("IDA installed at {}", installed.display()));
    Ok(())
}

fn absolute_destination(path: &Path) -> Result<PathBuf> {
    let path = if let Ok(rest) = path.strip_prefix("~") {
        dirs::home_dir()
            .ok_or_else(|| Error::Other("home directory is unavailable".into()))?
            .join(rest)
    } else {
        path.into()
    };
    Ok(std::path::absolute(path)?)
}

fn confirm(prompt: &str) -> Result<bool> {
    if !crate::util::tui::is_interactive() {
        return Err(Error::Other(
            "installation requires confirmation; pass --yes in a noninteractive terminal".into(),
        ));
    }
    dialoguer::Confirm::with_theme(&crate::util::tui::theme())
        .with_prompt(prompt)
        .default(false)
        .interact()
        .map_err(|error| Error::Other(error.to_string()))
}

async fn install_license(identifier: &str, temporary: &Path, installation: &Path) -> Result<()> {
    super::license::run(super::license::LicenseCommands::Get(super::license::LicenseGetArgs {
        customer_id: None,
        id: Some(identifier.into()),
        plan: None,
        product_type: None,
        all: false,
        output_dir: temporary.into(),
    }))
    .await?;
    let suffix = format!("{identifier}.hexlic");
    let mut copied = 0;
    for entry in std::fs::read_dir(temporary)? {
        let entry = entry?;
        if entry.file_type()?.is_file() && entry.file_name().to_string_lossy().ends_with(&suffix) {
            std::fs::copy(entry.path(), ida::executable_dir(installation).join(entry.file_name()))?;
            copied += 1;
        }
    }
    if copied == 0 {
        return Err(Error::NotFound(format!("downloaded license *{suffix}")));
    }
    Ok(())
}

fn register_default(installation: &Path) -> Result<()> {
    let mut store = crate::config::ConfigStore::global();
    let mut instances = store.get_string_map("ida.instances");
    let base = installation
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("ida")
        .trim_end_matches(".app")
        .to_lowercase()
        .replace(' ', "-")
        .replace("ida-professional", "ida-pro");
    let mut name = base.clone();
    let mut suffix = 2;
    while instances.get(&name).is_some_and(|existing| Path::new(existing) != installation) {
        name = format!("{base}-{suffix}");
        suffix += 1;
    }
    instances.insert(name.clone(), installation.to_string_lossy().into_owned());
    store.set_values([
        ("ida.instances".into(), json!(instances)),
        ("ida.default".into(), json!(name)),
    ])?;
    drop(store);
    if ida::is_idalib_capable(installation) {
        crate::plugin::set_ida_installation_directory(installation)?;
    }
    Ok(())
}
