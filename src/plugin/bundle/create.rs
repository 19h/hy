//! Stage per-target plugin closures and publish a completed bundle atomically.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use sha2::{Digest, Sha256};

use super::{BundleCreatedBy, BundleManifest, BundleTargetPlatformTag, PipTarget};
use crate::error::{Error, Result};
use crate::plugin::PluginMetadata;

pub struct ResolvedCell {
    pub target: PipTarget,
    pub plugins: IndexMap<String, (Vec<u8>, PluginMetadata)>,
    pub roots: Vec<String>,
}

struct Variant {
    name: String,
    version: String,
    bytes: Vec<u8>,
    targets: Vec<PipTarget>,
}

pub async fn create_bundle(
    output: &Path,
    cells: &[ResolvedCell],
    pip: &crate::ida::python::PipOptions,
    status: impl Fn(&str),
) -> Result<()> {
    let staging = tempfile::tempdir()?;
    let plugins_dir = staging.path().join("plugins");
    std::fs::create_dir_all(&plugins_dir)?;
    let deps_dir = staging.path().join("dependencies/python");
    std::fs::create_dir_all(&deps_dir)?;
    let mut variants: IndexMap<(String, [u8; 32]), Variant> = IndexMap::new();
    let mut target_manifests = Vec::new();
    for cell in cells {
        let destination = deps_dir.join(cell.target.id());
        std::fs::create_dir_all(&destination)?;
        status(&format!(
            "downloading wheels for {} Python {}",
            cell.target.ida_platform, cell.target.python_version
        ));
        let accepted = super::cell::prepare(cell, &destination, pip).await?;
        verify_wheelhouse(&destination, &cell.target)?;
        for (name, (bytes, _)) in &cell.plugins {
            if !accepted.contains(&name.to_lowercase()) {
                continue;
            }
            let digest: [u8; 32] = Sha256::digest(bytes).into();
            let mut archive = crate::util::python_zip::Archive::new(std::io::Cursor::new(bytes))?;
            let version =
                crate::plugin::select_archived_plugin(&mut archive, Some(name))?.metadata.version;
            variants
                .entry((name.clone(), digest))
                .or_insert_with(|| Variant {
                    name: name.clone(),
                    version,
                    bytes: bytes.clone(),
                    targets: Vec::new(),
                })
                .targets
                .push(cell.target.clone());
        }
        target_manifests.push(BundleTargetPlatformTag::from_target(&cell.target)?);
    }
    let mut counts = HashMap::new();
    for variant in variants.values() {
        *counts.entry(variant.name.clone()).or_insert(0) += 1;
    }
    let mut filenames = Vec::new();
    for variant in variants.values() {
        let suffix = if counts[&variant.name] > 1 {
            let mut platforms: Vec<_> =
                variant.targets.iter().map(|target| target.ida_platform.clone()).collect();
            platforms.sort();
            platforms.dedup();
            format!("-{}", platforms.join("+"))
        } else {
            String::new()
        };
        filenames.push(format!("{}-{}{suffix}.zip", variant.name, variant.version));
    }
    let mut occurrences = HashMap::new();
    for filename in &filenames {
        *occurrences.entry(filename).or_insert(0) += 1;
    }
    let duplicates: HashSet<_> = occurrences
        .into_iter()
        .filter(|(_, count)| *count > 1)
        .map(|(filename, _)| filename.clone())
        .collect();
    for (variant, mut filename) in variants.into_values().zip(filenames) {
        if duplicates.contains(&filename) {
            let mut targets: Vec<_> = variant.targets.iter().map(PipTarget::id).collect();
            targets.sort();
            filename = format!("{}-{}-{}.zip", variant.name, variant.version, targets.join("+"));
        }
        if filename.contains(['/', '\\', ':']) {
            return Err(Error::PluginInstall("invalid bundle archive filename".into()));
        }
        let path = plugins_dir.join(filename);
        if path.exists() && std::fs::read(&path)? != variant.bytes {
            return Err(Error::PluginInstall(
                "different archives map to one bundle filename".into(),
            ));
        }
        std::fs::write(path, variant.bytes)?;
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
    status("writing bundle archive");
    write_bundle_zip(output, &serde_json::to_vec_pretty(&manifest)?, staging.path())
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
