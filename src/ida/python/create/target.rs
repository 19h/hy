//! Inspect existing targets and validate creation without modifying their contents.

use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::ida::python::checks::{has_pip, version};

pub(super) enum Target {
    Missing,
    Healthy(PathBuf),
    Unusable(String),
}

pub(super) async fn inspect(path: &Path, wanted: &str) -> Result<Target> {
    if !path.exists() {
        return Ok(Target::Missing);
    }
    if !path.is_dir() {
        return Ok(Target::Unusable("exists and is not a directory".into()));
    }
    if !path.join("pyvenv.cfg").is_file() {
        if std::fs::read_dir(path).is_ok_and(|mut entries| entries.next().is_none()) {
            return Ok(Target::Missing);
        }
        return Ok(Target::Unusable(
            "exists and contains files, but is not a virtual environment".into(),
        ));
    }
    let Some(executable) =
        crate::ida::python::layout::candidates(path, None).into_iter().find(|path| path.is_file())
    else {
        return Ok(Target::Unusable(
            "is a virtual environment, but its interpreter is missing".into(),
        ));
    };
    let Some(actual) = version(&executable).await? else {
        return Ok(Target::Unusable(format!(
            "is a virtual environment, but {} does not run",
            executable.display()
        )));
    };
    if actual != wanted {
        return Ok(Target::Unusable(format!(
            "is a Python {actual} virtual environment, but IDA runs Python {wanted}"
        )));
    }
    if !has_pip(&executable).await {
        return Ok(Target::Unusable(
            "is a virtual environment, but pip is not installed in it".into(),
        ));
    }
    Ok(Target::Healthy(executable))
}
