//! Register editable src-layout packages in the selected IDA interpreter.

use std::io::Write;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::ida::python;

const PURELIB_QUERY: &str = "import sysconfig; print(sysconfig.get_paths()['purelib'])";

pub struct EditableRegistration {
    change: Option<PathChange>,
}

enum PathChange {
    Write {
        target: PathBuf,
        staged: tempfile::NamedTempFile,
    },
    Remove(PathBuf),
}

fn filename(name: &str) -> String {
    let name: String = name
        .chars()
        .map(|character| {
            if character.is_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect();
    format!("_hcli_editable_{name}.pth")
}

async fn purelib(executable: Option<&Path>) -> Result<PathBuf> {
    let resolved;
    let executable = match executable {
        Some(executable) => executable,
        None => {
            resolved = python::resolve().await?;
            &resolved.exe
        }
    };
    let output =
        python::output(python::command(executable).args(["-c", PURELIB_QUERY]), 30).await?;
    if !output.status.success() {
        return Err(Error::PluginInstall(format!(
            "cannot locate IDA Python site-packages: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let text = std::str::from_utf8(&output.stdout)
        .map_err(|_| Error::PluginInstall("Python site-packages path is not UTF-8".into()))?
        .trim();
    let path = PathBuf::from(text);
    if text.contains(['\r', '\n']) || !path.is_absolute() {
        return Err(Error::PluginInstall(
            "Python site-packages query returned an invalid path".into(),
        ));
    }
    Ok(path)
}

impl EditableRegistration {
    /// Stage a registration, or prepare stale-file cleanup for other layouts.
    pub async fn prepare(
        name: &str,
        source: Option<&Path>,
        executable: Option<&Path>,
    ) -> Result<Self> {
        let src = source.map(|source| source.join("src")).filter(|src| src.is_dir());
        let directory = match purelib(executable).await {
            Ok(directory) => directory,
            Err(error) if src.is_none() => {
                // Upstream skips cleanup when the IDA interpreter cannot be located.
                tracing::debug!(%error, "cannot locate editable registration for cleanup");
                return Ok(Self {
                    change: None,
                });
            }
            Err(error) => return Err(error),
        };
        let target = directory.join(filename(name));
        let change = if let Some(src) = src {
            let src = src.canonicalize()?;
            let text = src
                .to_str()
                .ok_or_else(|| Error::PluginInstall("editable source path is not UTF-8".into()))?;
            if text.contains(['\r', '\n']) {
                return Err(Error::PluginInstall(
                    "editable source path contains a line break".into(),
                ));
            }
            std::fs::create_dir_all(&directory)?;
            let mut staged = tempfile::NamedTempFile::new_in(directory)?;
            writeln!(staged, "{text}")?;
            PathChange::Write {
                target,
                staged,
            }
        } else {
            PathChange::Remove(target)
        };
        Ok(Self {
            change: Some(change),
        })
    }

    /// Publish only after plugin files have been staged and dependency checks passed.
    pub fn publish(self) -> Result<()> {
        match self.change {
            Some(PathChange::Write {
                target,
                staged,
            }) => {
                staged.persist(&target).map_err(|error| {
                    Error::PluginInstall(format!(
                        "cannot publish editable registration {}: {}",
                        target.display(),
                        error.error
                    ))
                })?;
            }
            Some(PathChange::Remove(target)) => match std::fs::remove_file(&target) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(Error::PluginInstall(format!(
                        "cannot remove editable registration {}: {error}",
                        target.display()
                    )));
                }
            },
            None => {}
        }
        Ok(())
    }
}
