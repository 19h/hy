//! Read installation versions without executing IDA or importing its Python modules.

use std::path::Path;

pub fn detect_ida_version(directory: &Path) -> Option<String> {
    crate::config::Env::global()
        .current_ida_version
        .clone()
        .or_else(|| instance_version(directory, ""))
}

/// Prefer Windows installer metadata, then SDK, binary, directory and instance names.
pub fn instance_version(directory: &Path, name: &str) -> Option<String> {
    installation_version(directory).map(|(version, _)| version).or_else(|| version_in_name(name))
}

pub(super) fn installation_version(directory: &Path) -> Option<(String, &'static str)> {
    #[cfg(windows)]
    if let Some(installation) = super::discovery::windows::for_directory(directory) {
        let pattern =
            regex::Regex::new(r"[0-9]+\.[0-9]+").expect("static registry version pattern");
        for value in
            [Some(installation.display_name), installation.display_version].into_iter().flatten()
        {
            if let Some(found) = pattern.find(&value) {
                return Some((found.as_str().into(), "Windows registry"));
            }
        }
    }
    if let Some(version) = sdk_version(directory) {
        return Some((version, "python/ida_pro.py SDK docstring"));
    }
    if let Some(version) =
        super::ida_binary_path(directory).as_deref().and_then(version_from_binary)
    {
        return Some((version, "IDA binary version metadata"));
    }
    version_in_name(directory.file_name().and_then(|name| name.to_str()).unwrap_or(""))
        .map(|version| (version, "installation directory name"))
}

pub(super) fn sdk_version(directory: &Path) -> Option<String> {
    let sdk = super::executable_dir(directory).join("python/ida_pro.py");
    let pattern = regex::Regex::new(r"IDA SDK v(\d+\.\d+)").expect("static SDK version pattern");
    let source = std::fs::read_to_string(sdk).ok()?;
    Some(pattern.captures(&source)?[1].into())
}

pub(super) fn version_in_name(name: &str) -> Option<String> {
    let pattern = regex::Regex::new(r"\d+\.\d+").expect("static directory version pattern");
    Some(pattern.find(name)?.as_str().into())
}

fn version_from_binary(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    let pattern = regex::bytes::Regex::new(r"\d+\.\d+\.\d{6}(?:\.[a-z0-9]+)?").ok()?;
    let version = pattern.find(&bytes)?;

    Some(
        String::from_utf8_lossy(version.as_bytes())
            .split('.')
            .take(2)
            .collect::<Vec<_>>()
            .join("."),
    )
}
