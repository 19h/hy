//! Execute native IDA installers and verify their output.

use std::path::{Path, PathBuf};

use tokio::process::Command;

use crate::error::{Error, Result};
use crate::util::io::check_free_space;

pub async fn install_ida(installer: &Path, destination: &Path) -> Result<PathBuf> {
    if !installer.is_file() {
        return Err(Error::FileNotFound(installer.into()));
    }
    let installer = installer.canonicalize()?;
    if std::fs::symlink_metadata(destination).is_ok() {
        return Err(Error::IdaInstallFailed(format!(
            "installation directory already exists: {}",
            destination.display()
        )));
    }
    let expected_extension = if cfg!(target_os = "macos") {
        "zip"
    } else if cfg!(windows) {
        "exe"
    } else {
        "run"
    };
    if installer.extension().and_then(|extension| extension.to_str()) != Some(expected_extension) {
        return Err(Error::IdaInstallFailed(format!(
            "expected a .{expected_extension} installer on this platform"
        )));
    }
    let required = std::fs::metadata(&installer)?.len().checked_mul(3).ok_or_else(|| {
        Error::IdaInstallFailed("installer size exceeds supported disk-space calculation".into())
    })?;
    check_free_space(destination, required)?;
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::create_dir(destination)?;
    let destination = destination.canonicalize()?;
    if cfg!(target_os = "macos") {
        install_mac(&installer, &destination).await?;
    } else {
        install_native(&installer, &destination).await?;
    }
    verify_installation(&destination)?;
    Ok(destination)
}

async fn run(command: &mut Command, operation: &str) -> Result<()> {
    command.kill_on_drop(true);
    let output = tokio::time::timeout(std::time::Duration::from_secs(600), command.output())
        .await
        .map_err(|_| {
            Error::IdaInstallFailed(format!("{operation} timed out after 600 seconds"))
        })??;
    if !output.status.success() {
        return Err(Error::IdaInstallFailed(format!(
            "{operation} failed ({}): {}{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        )));
    }
    Ok(())
}

fn installer_command(installer: &Path, destination: &Path, debug_log: &Path) -> Command {
    let mut command = Command::new(installer);
    command.args(["--mode", "unattended", "--debugtrace"]).arg(debug_log);
    if cfg!(windows) {
        command.args(["--install_python", "0"]);
    }
    command.arg("--prefix").arg(destination);
    command
}

async fn install_native(installer: &Path, destination: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = std::fs::metadata(installer)?.permissions();
        permissions.set_mode(permissions.mode() | 0o100);
        std::fs::set_permissions(installer, permissions)?;
        let home =
            dirs::home_dir().ok_or_else(|| Error::Other("home directory is unavailable".into()))?;
        std::fs::create_dir_all(home.join(".local/share/applications"))?;
    }
    let debug_log = destination.join("installer-debug.log");
    run(&mut installer_command(installer, destination, &debug_log), "installer execution").await
}

async fn install_mac(installer: &Path, destination: &Path) -> Result<()> {
    let unpack = tempfile::tempdir()?;
    let output = tempfile::tempdir()?;
    // Validate member paths before handing extraction to the system utility.
    let mut archive = zip::ZipArchive::new(std::fs::File::open(installer)?)?;
    for index in 0..archive.len() {
        if archive.by_index(index)?.enclosed_name().is_none() {
            return Err(Error::IdaInstallFailed(
                "installer archive contains an unsafe path".into(),
            ));
        }
    }
    run(
        Command::new("unzip").arg("-qq").arg(installer).arg("-d").arg(unpack.path()),
        "installer extraction",
    )
    .await?;
    let roots = std::fs::read_dir(unpack.path())?.collect::<std::io::Result<Vec<_>>>()?;
    let [root] = roots.as_slice() else {
        return Err(Error::IdaInstallFailed(
            "installer archive must contain exactly one application".into(),
        ));
    };
    let executable = ["osx-arm64", "osx-x86_64"]
        .iter()
        .map(|name| root.path().join("Contents/MacOS").join(name))
        .find(|path| path.is_file())
        .ok_or_else(|| {
            Error::IdaInstallFailed("installer executable not found in application".into())
        })?;
    let debug_log = unpack.path().join("installer-debug.log");
    run(&mut installer_command(&executable, output.path(), &debug_log), "installer execution")
        .await?;
    let products = std::fs::read_dir(output.path())?.collect::<std::io::Result<Vec<_>>>()?;
    let [product] = products.as_slice() else {
        return Err(Error::IdaInstallFailed(
            "installer must produce exactly one installation directory".into(),
        ));
    };
    if !product.path().is_dir() {
        return Err(Error::IdaInstallFailed("installer output is not a directory".into()));
    }
    run(Command::new("ditto").arg(product.path()).arg(destination), "installation copy").await
}

fn verify_installation(destination: &Path) -> Result<()> {
    let mut pending = vec![destination.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(directory)? {
            let entry = entry?;
            let kind = entry.file_type()?;
            if kind.is_file() && entry.file_name() == "ida.hlp" {
                return Ok(());
            }
            if kind.is_dir() {
                pending.push(entry.path());
            }
        }
    }
    Err(Error::IdaInstallFailed("installation failed: ida.hlp was not created".into()))
}

pub fn is_idalib_capable(installation: &Path) -> bool {
    let directory = super::executable_dir(installation);
    let filename = if cfg!(windows) {
        "idalib.dll"
    } else if cfg!(target_os = "macos") {
        "libidalib.dylib"
    } else {
        "libidalib.so"
    };
    directory.join(filename).is_file()
}

/// Use the supported idapro/ida_registry API with this installation explicitly selected.
pub async fn accept_eula(installation: &Path) -> Result<()> {
    let interpreter = crate::config::Env::global()
        .current_ida_python_exe
        .as_ref()
        .map(PathBuf::from)
        .or_else(|| super::python::find_command("python3"))
        .or_else(|| super::python::find_command("python"))
        .ok_or_else(|| Error::Other("Python with idapro is unavailable".into()))?;
    let mut command = super::python::command(&interpreter);
    command.arg("-c").arg("import idapro\nimport ida_registry\nfor version in range(90, 95):\n    ida_registry.reg_write_int('EULA %d' % version, 1)\n")
        .env("IDADIR", super::executable_dir(installation));
    run(&mut command, "EULA acceptance").await
}
