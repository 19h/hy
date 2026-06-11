//! Plugin bundles for offline installation.
//!
//! A bundle is a zip archive with the layout:
//!
//! ```text
//! plugin-bundle.json                      # manifest
//! plugins/<name>-<version>[.platforms].zip
//! dependencies/python/<target-id>/*.whl   # wheelhouse per pip target
//! ```
//!
//! The format is interoperable with bundles produced by the Python hcli
//! (`kind: "hcli-plugin-bundle"`, manifest version 1).

use std::io::Read;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

pub const MINIMUM_PYTHON_VERSION: (u32, u32) = (3, 10);
pub const SUPPORTED_PYTHON_VERSIONS: &[&str] = &["3.10", "3.11", "3.12", "3.13", "3.14"];
pub const ALL_PLATFORMS: &[&str] = &[
    "windows-x86_64",
    "linux-x86_64",
    "macos-aarch64",
    "macos-x86_64",
];

const PLATFORM_ALIASES: &[(&str, &[&str])] = &[
    ("windows-x86_64", &["windows", "win", "win64"]),
    ("linux-x86_64", &["linux", "linux64"]),
    ("macos-aarch64", &["macos-arm64", "macos-arm", "mac-arm64"]),
    ("macos-x86_64", &["macos-intel", "mac-intel", "macos-x64"]),
];

/// Resolve a platform name or alias to the canonical platform string.
pub fn resolve_platform_alias(name: &str) -> Result<String> {
    let lower = name.trim().to_lowercase();
    for (canonical, aliases) in PLATFORM_ALIASES {
        if lower == *canonical || aliases.contains(&lower.as_str()) {
            return Ok(canonical.to_string());
        }
    }
    let valid: Vec<String> = PLATFORM_ALIASES
        .iter()
        .map(|(c, a)| format!("  {c} (or: {})", a.join(", ")))
        .collect();
    Err(Error::Other(format!(
        "unknown platform: '{name}'\nvalid platforms:\n{}",
        valid.join("\n")
    )))
}

/// The bundle platform identifier for the machine we are running on.
pub fn current_bundle_platform() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows-x86_64"
    } else if cfg!(target_os = "macos") {
        if cfg!(target_arch = "aarch64") {
            "macos-aarch64"
        } else {
            "macos-x86_64"
        }
    } else {
        "linux-x86_64"
    }
}

fn parse_python_version(version: &str) -> Result<(u32, u32)> {
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() == 2
        && let (Ok(major), Ok(minor)) = (parts[0].parse(), parts[1].parse()) {
            return Ok((major, minor));
        }
    Err(Error::Other(format!(
        "invalid python version: '{version}' (expected 'major.minor')"
    )))
}

// ── pip targets ─────────────────────────────────────────────────────────

/// A (platform, Python version) pair that wheels are resolved for.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PipTarget {
    pub ida_platform: String,
    pub python_version: String,
}

impl PipTarget {
    pub fn new(ida_platform: &str, python_version: &str) -> Result<Self> {
        let platform = resolve_platform_alias(ida_platform)?;
        let ver = parse_python_version(python_version)?;
        if ver < MINIMUM_PYTHON_VERSION {
            return Err(Error::Other(format!(
                "python {python_version} is below minimum {}.{}",
                MINIMUM_PYTHON_VERSION.0, MINIMUM_PYTHON_VERSION.1
            )));
        }
        Ok(Self {
            ida_platform: platform,
            python_version: python_version.to_string(),
        })
    }

    /// Parse a target ID like `linux-x86_64-cp312`.
    pub fn parse(target_id: &str) -> Result<Self> {
        let Some(pos) = target_id.rfind("-cp") else {
            return Err(Error::Other(format!(
                "invalid target ID: '{target_id}'\nexpected format: <platform>-cp<ver> (e.g. 'linux-x86_64-cp312')"
            )));
        };
        let (platform, cp) = target_id.split_at(pos);
        let digits = &cp[3..];
        if digits.len() < 2 || !digits.chars().all(|c| c.is_ascii_digit()) {
            return Err(Error::Other(format!("invalid target ID: '{target_id}'")));
        }
        let (major, minor) = digits.split_at(1);
        Self::new(platform, &format!("{major}.{minor}"))
    }

    /// Target ID, e.g. `linux-x86_64-cp312`.
    pub fn id(&self) -> String {
        format!(
            "{}-cp{}",
            self.ida_platform,
            self.python_version.replace('.', "")
        )
    }

    pub fn abis(&self) -> Vec<String> {
        let ver = self.python_version.replace('.', "");
        vec![format!("cp{ver}"), "abi3".into(), "none".into()]
    }

    /// pip `--platform` tags for this target.
    pub fn pip_platform_tags(&self) -> Vec<String> {
        match self.ida_platform.as_str() {
            "windows-x86_64" => vec!["win_amd64".into()],
            "linux-x86_64" => manylinux_tags((2, 28), "x86_64"),
            "macos-aarch64" => mac_tags((11, 0), "arm64"),
            "macos-x86_64" => mac_tags((10, 13), "x86_64"),
            other => vec![other.to_string()],
        }
    }

    /// Arguments for `pip download` targeting this platform/Python pair.
    pub fn pip_download_args(&self) -> Vec<String> {
        let mut args = vec![
            "--only-binary=:all:".to_string(),
            "--implementation".into(),
            "cp".into(),
            "--python-version".into(),
            self.python_version.clone(),
        ];
        for abi in self.abis() {
            args.push("--abi".into());
            args.push(abi);
        }
        for tag in self.pip_platform_tags() {
            args.push("--platform".into());
            args.push(tag);
        }
        args
    }
}

fn manylinux_tags(max_glibc: (u32, u32), arch: &str) -> Vec<String> {
    let (major, max_minor) = max_glibc;
    let mut tags = Vec::new();
    for minor in (5..=max_minor).rev() {
        tags.push(format!("manylinux_{major}_{minor}_{arch}"));
    }
    if max_glibc >= (2, 17) {
        tags.push(format!("manylinux2014_{arch}"));
    }
    if max_glibc >= (2, 12) {
        tags.push(format!("manylinux2010_{arch}"));
    }
    if max_glibc >= (2, 5) {
        tags.push(format!("manylinux1_{arch}"));
    }
    tags
}

fn mac_tags(min_version: (u32, u32), arch: &str) -> Vec<String> {
    let mut tags = Vec::new();
    let (major, minor) = min_version;
    if major >= 11 {
        tags.push(format!("macosx_{major}_0_{arch}"));
        tags.push(format!("macosx_{major}_0_universal2"));
        // 10.x compatibility range (universal2 fat wheels).
        for m in (4..=16).rev() {
            tags.push(format!("macosx_10_{m}_universal2"));
            if arch == "x86_64" {
                tags.push(format!("macosx_10_{m}_{arch}"));
            }
        }
    } else {
        for m in (4..=minor).rev() {
            for fmt in &[arch, "intel", "fat64", "universal2", "universal"] {
                if arch == "arm64" && *fmt != "universal2" {
                    continue;
                }
                tags.push(format!("macosx_10_{m}_{fmt}"));
            }
        }
    }
    tags
}

// ── manifest models ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleCreatedBy {
    pub tool: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleTargetPlatformTag {
    pub id: String,
    pub ida_platform: String,
    pub python_version: String,
    pub implementation: String,
    pub abis: Vec<String>,
    pub pip_platform_tags: Vec<String>,
    pub wheelhouse: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleManifest {
    pub version: u32,
    pub kind: String,
    pub built_at: String,
    pub created_by: BundleCreatedBy,
    pub target_platform_tags: Vec<BundleTargetPlatformTag>,
}

impl BundleTargetPlatformTag {
    pub fn from_target(target: &PipTarget) -> Self {
        Self {
            id: target.id(),
            ida_platform: target.ida_platform.clone(),
            python_version: target.python_version.clone(),
            implementation: "cp".into(),
            abis: target.abis(),
            pip_platform_tags: target.pip_platform_tags(),
            wheelhouse: format!("dependencies/python/{}", target.id()),
        }
    }
}

// ── bundle reading ──────────────────────────────────────────────────────

/// True if `path` is a zip file containing a `plugin-bundle.json` manifest.
pub fn is_plugin_bundle_zip(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    let Ok(file) = std::fs::File::open(path) else {
        return false;
    };
    let Ok(mut archive) = zip::ZipArchive::new(file) else {
        return false;
    };
    archive.by_name("plugin-bundle.json").is_ok()
}

/// Read and parse the bundle manifest from a bundle zip.
pub fn read_bundle_manifest(path: &Path) -> Result<BundleManifest> {
    let file = std::fs::File::open(path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    let mut entry = archive
        .by_name("plugin-bundle.json")
        .map_err(|_| Error::Other(format!("{} is not a plugin bundle", path.display())))?;
    let mut text = String::new();
    entry.read_to_string(&mut text)?;
    let manifest: BundleManifest = serde_json::from_str(&text)
        .map_err(|e| Error::Other(format!("invalid plugin-bundle.json: {e}")))?;
    if manifest.kind != "hcli-plugin-bundle" || manifest.version != 1 {
        return Err(Error::Other(format!(
            "unsupported bundle (kind: {}, version: {})",
            manifest.kind, manifest.version
        )));
    }
    Ok(manifest)
}

/// A plugin archive found inside a bundle.
#[derive(Debug, Clone)]
pub struct BundledPlugin {
    /// Entry name inside the bundle zip (e.g. `plugins/foo-1.0.0.zip`).
    pub entry_name: String,
    pub metadata: crate::plugin::PluginMetadata,
}

/// List the plugin archives contained in a bundle.
pub fn list_bundle_plugins(path: &Path) -> Result<Vec<BundledPlugin>> {
    let file = std::fs::File::open(path)?;
    let mut archive = zip::ZipArchive::new(file)?;

    let names: Vec<String> = archive
        .file_names()
        .filter(|n| n.starts_with("plugins/") && n.ends_with(".zip"))
        .map(String::from)
        .collect();

    let mut plugins = Vec::new();
    for name in names {
        let mut entry = archive.by_name(&name)?;
        let mut buf = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut buf)?;
        drop(entry);

        let metadata = read_metadata_from_zip_bytes(&buf)?;
        plugins.push(BundledPlugin {
            entry_name: name,
            metadata,
        });
    }
    plugins.sort_by(|a, b| a.metadata.name.cmp(&b.metadata.name));
    Ok(plugins)
}

/// Extract a plugin archive from a bundle to a temporary file and return
/// its path (caller cleans up the parent directory).
pub fn extract_bundled_plugin(bundle_path: &Path, entry_name: &str) -> Result<PathBuf> {
    let file = std::fs::File::open(bundle_path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    let mut entry = archive
        .by_name(entry_name)
        .map_err(|_| Error::Other(format!("entry not found in bundle: {entry_name}")))?;

    let dir = tempfile::tempdir()?.keep();
    let filename = entry_name.rsplit('/').next().unwrap_or("plugin.zip");
    let out_path = dir.join(filename);
    let mut out = std::fs::File::create(&out_path)?;
    std::io::copy(&mut entry, &mut out)?;
    Ok(out_path)
}

fn read_metadata_from_zip_bytes(buf: &[u8]) -> Result<crate::plugin::PluginMetadata> {
    let cursor = std::io::Cursor::new(buf);
    let mut archive = zip::ZipArchive::new(cursor)?;
    for i in 0..archive.len() {
        let entry = archive.by_index(i)?;
        let name = entry.name().to_owned();
        if (name == "ida-plugin.json" || name.ends_with("/ida-plugin.json"))
            && name.matches('/').count() <= 1
        {
            let manifest: crate::plugin::PluginManifest = serde_json::from_reader(entry)?;
            return Ok(manifest.metadata);
        }
    }
    Err(Error::PluginInstall(
        "ida-plugin.json not found in archive".into(),
    ))
}

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
/// `python3 -m pip download` when any plugin declares Python dependencies.
pub fn create_bundle(
    output: &Path,
    archives: &[ResolvedPluginArchive],
    targets: &[PipTarget],
    status: impl Fn(&str),
) -> Result<()> {
    let staging = tempfile::tempdir()?;
    let plugins_dir = staging.path().join("plugins");
    std::fs::create_dir_all(&plugins_dir)?;
    let deps_dir = staging.path().join("dependencies").join("python");
    std::fs::create_dir_all(&deps_dir)?;

    let mut all_python_deps: Vec<String> = Vec::new();

    for archive in archives {
        let suffix = if archive.platforms.is_empty() {
            String::new()
        } else {
            format!("-{}", archive.platforms.join("+"))
        };
        let filename = format!("{}-{}{}.zip", archive.name, archive.version, suffix);
        let dest = plugins_dir.join(&filename);
        if !dest.exists() {
            std::fs::write(&dest, &archive.bytes)?;
        }

        let metadata = read_metadata_from_zip_bytes(&archive.bytes)?;
        if let Some(deps) = metadata.python_dependencies {
            all_python_deps.extend(deps);
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
            download_wheelhouse(&all_python_deps, target, &wh_dir)?;
            verify_wheelhouse(&wh_dir, target)?;
        }
        target_manifests.push(BundleTargetPlatformTag::from_target(target));
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

fn find_python() -> Option<PathBuf> {
    for name in &["python3", "python"] {
        if let Ok(output) = std::process::Command::new(name).arg("--version").output()
            && output.status.success() {
                return Some(PathBuf::from(name));
            }
    }
    None
}

fn download_wheelhouse(deps: &[String], target: &PipTarget, dest: &Path) -> Result<()> {
    let python = find_python().ok_or_else(|| {
        Error::Other(
            "bundling Python dependencies requires a `python3` with pip on PATH".into(),
        )
    })?;

    let mut cmd = std::process::Command::new(python);
    cmd.arg("-m").arg("pip").arg("download");
    cmd.args(target.pip_download_args());
    cmd.arg("--dest").arg(dest);
    cmd.args(deps);

    let output = cmd.output()?;
    if !output.status.success() {
        return Err(Error::Other(format!(
            "pip download failed for target {}:\n{}\n{}",
            target.id(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(())
}

fn verify_wheelhouse(wh_dir: &Path, target: &PipTarget) -> Result<()> {
    for entry in std::fs::read_dir(wh_dir)?.flatten() {
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

    let tmp_output = output.with_extension("tmp.zip");
    let result = (|| -> Result<()> {
        let file = std::fs::File::create(&tmp_output)?;
        let mut zf = zip::ZipWriter::new(file);
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

        zf.start_file("plugin-bundle.json", options)?;
        zf.write_all(manifest_bytes)?;

        let mut files = Vec::new();
        collect_files(staging, &mut files)?;
        files.sort();

        for file_path in files {
            let arcname = file_path
                .strip_prefix(staging)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            zf.start_file(&arcname, options)?;
            let mut f = std::fs::File::open(&file_path)?;
            std::io::copy(&mut f, &mut zf)?;
        }
        zf.finish()?;
        Ok(())
    })();

    match result {
        Ok(()) => {
            std::fs::rename(&tmp_output, output)?;
            Ok(())
        }
        Err(e) => {
            let _ = std::fs::remove_file(&tmp_output);
            Err(e)
        }
    }
}

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir)?.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, out)?;
        } else if path.is_file() {
            out.push(path);
        }
    }
    Ok(())
}
