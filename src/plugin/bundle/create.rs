//! Stage plugin archives and wheelhouses, then publish the completed bundle.

use std::path::{Path, PathBuf};

use super::{BundleCreatedBy, BundleManifest, BundleTargetPlatformTag, PipTarget};
use crate::error::{Error, Result};

// ── bundle creation ─────────────────────────────────────────────────────

/// A resolved plugin archive ready to be placed in a bundle.
pub struct ResolvedPluginArchive {
    pub name: String,
    pub version: String,
    pub bytes: Vec<u8>,
    /// Platforms this exact archive serves (used for filename suffixing
    /// when a plugin ships per-platform builds). Empty = all platforms.
    pub platforms: Vec<String>,
}

/// Create a plugin bundle archive at `output`.
///
/// `archives` are the resolved plugin zips; `targets` the pip targets to
/// build wheelhouses for. Wheel downloading shells out to
/// the selected interpreter when a plugin declares an explicit requirement list.
pub async fn create_bundle(
    output: &Path,
    archives: &[ResolvedPluginArchive],
    targets: &[PipTarget],
    pip: &crate::ida::python::PipOptions,
    status: impl Fn(&str),
) -> Result<()> {
    let staging = tempfile::tempdir()?;
    let plugins_dir = staging.path().join("plugins");
    std::fs::create_dir_all(&plugins_dir)?;
    let deps_dir = staging.path().join("dependencies").join("python");
    std::fs::create_dir_all(&deps_dir)?;

    let mut all_python_deps: Vec<String> = Vec::new();

    for archive in archives {
        crate::plugin::validate_name(&archive.name)?;
        let suffix = if archive.platforms.is_empty() {
            String::new()
        } else {
            format!("-{}", archive.platforms.join("+"))
        };
        let filename = format!("{}-{}{}.zip", archive.name, archive.version, suffix);
        if filename.contains(['/', '\\']) {
            return Err(Error::PluginInstall(
                "plugin version cannot be used in a bundle filename".into(),
            ));
        }
        let dest = plugins_dir.join(&filename);
        if !dest.exists() {
            std::fs::write(&dest, &archive.bytes)?;
        }

        for metadata in super::metadata::descriptors(&archive.bytes)? {
            // Bundle creation collects only explicit lists in the source CLI.
            // Inline script metadata is resolved by installation, not this stage.
            if let crate::plugin::PythonDependencies::Requirements(requirements) =
                metadata.python_dependencies
            {
                all_python_deps.extend(requirements);
            }
        }
    }

    let mut target_manifests = Vec::new();
    for target in targets {
        let wh_dir = deps_dir.join(target.id());
        std::fs::create_dir_all(&wh_dir)?;

        if !all_python_deps.is_empty() {
            status(&format!(
                "downloading wheels for {} Python {}",
                target.ida_platform, target.python_version
            ));
            super::download::wheelhouse(&all_python_deps, target, &wh_dir, pip).await?;
            verify_wheelhouse(&wh_dir, target)?;
        }
        target_manifests.push(BundleTargetPlatformTag::from_target(target)?);
    }

    let manifest = BundleManifest {
        version: 1,
        kind: "hcli-plugin-bundle".into(),
        built_at: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        created_by: BundleCreatedBy {
            tool: "hy".into(),
            version: crate::config::Env::global().version.clone(),
        },
        target_platform_tags: target_manifests,
    };
    let manifest_bytes = serde_json::to_vec_pretty(&manifest)?;

    status("writing bundle archive");
    write_bundle_zip(output, &manifest_bytes, staging.path())?;
    Ok(())
}

fn verify_wheelhouse(wh_dir: &Path, target: &PipTarget) -> Result<()> {
    for entry in std::fs::read_dir(wh_dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name.ends_with(".whl") {
            continue;
        }
        if name.ends_with(".tar.gz") || name.ends_with(".tar.bz2") || name.ends_with(".zip") {
            return Err(Error::Other(format!(
                "sdist found in wheelhouse for {}: {name}",
                target.id()
            )));
        }
    }
    Ok(())
}

fn write_bundle_zip(output: &Path, manifest_bytes: &[u8], staging: &Path) -> Result<()> {
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    let output = std::path::absolute(output)?;
    let parent = output
        .parent()
        .ok_or_else(|| Error::Other("bundle output has no parent directory".into()))?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    {
        let mut zf = zip::ZipWriter::new(temporary.as_file_mut());
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

        zf.start_file("plugin-bundle.json", options)?;
        zf.write_all(manifest_bytes)?;

        let mut files = Vec::new();
        collect_files(staging, &mut files)?;
        files.sort();

        for file_path in files {
            let arcname =
                file_path.strip_prefix(staging).unwrap().to_string_lossy().replace('\\', "/");
            zf.start_file(&arcname, options)?;
            let mut f = std::fs::File::open(&file_path)?;
            std::io::copy(&mut f, &mut zf)?;
        }
        zf.finish()?;
    }
    temporary.persist(output).map_err(|error| Error::Io(error.error))?;
    Ok(())
}

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, out)?;
        } else if path.is_file() {
            out.push(path);
        }
    }
    Ok(())
}
