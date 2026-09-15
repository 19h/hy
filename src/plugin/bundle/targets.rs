//! Wheel-resolution targets and platform tags.

use crate::error::{Error, Result};
use crate::util::{python_repr::string_repr, strings::python_trim};

mod grammar;

#[cfg(test)]
pub(crate) mod reference;
#[cfg(test)]
mod tests;

pub const MINIMUM_PYTHON_VERSION: (u32, u32) = (3, 10);
pub const SUPPORTED_PYTHON_VERSIONS: &[&str] = &["3.10", "3.11", "3.12", "3.13", "3.14"];
pub const ALL_PLATFORMS: &[&str] = &[
    "windows-x86_64",
    "windows-aarch64",
    "linux-x86_64",
    "linux-aarch64",
    "macos-aarch64",
    "macos-x86_64",
];

const PLATFORM_ALIASES: &[(&str, &[&str])] = &[
    ("windows-x86_64", &["windows", "win", "win64"]),
    ("windows-aarch64", &["windows-arm64", "win-arm64"]),
    ("linux-x86_64", &["linux", "linux64"]),
    ("linux-aarch64", &["linux-arm64"]),
    ("macos-aarch64", &["macos-arm64", "macos-arm", "mac-arm64"]),
    ("macos-x86_64", &["macos-intel", "mac-intel", "macos-x64"]),
];

/// Resolve a platform name or alias to the canonical platform string.
pub fn resolve_platform_alias(name: &str) -> Result<String> {
    let lower = python_trim(name).to_lowercase();
    for (canonical, aliases) in PLATFORM_ALIASES {
        if lower == *canonical || aliases.contains(&lower.as_str()) {
            return Ok(canonical.to_string());
        }
    }
    let valid: Vec<String> =
        PLATFORM_ALIASES.iter().map(|(c, a)| format!("  {c} (or: {})", a.join(", "))).collect();
    Err(Error::Other(format!(
        "unknown platform: {}\nvalid platforms:\n{}",
        string_repr(name),
        valid.join("\n")
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
        resolve_platform_alias(ida_platform)?;
        grammar::validate_version(python_version)?;
        Ok(Self {
            // Explicit targets and the current-platform override retain their
            // spelling. Only ordinary --platform aliases are canonicalized.
            ida_platform: ida_platform.into(),
            python_version: python_version.to_string(),
        })
    }

    /// Parse a target ID like `linux-x86_64-cp312`.
    pub fn parse(target_id: &str) -> Result<Self> {
        let (platform, version) = grammar::target_id(target_id)?;
        Self::new(platform, &version)
    }

    /// Target ID, e.g. `linux-x86_64-cp312`.
    pub fn id(&self) -> String {
        format!("{}-cp{}", self.ida_platform, self.python_version.replace('.', ""))
    }

    pub fn abis(&self) -> Vec<String> {
        let ver = self.python_version.replace('.', "");
        vec![format!("cp{ver}"), "abi3".into(), "none".into()]
    }

    /// pip `--platform` tags for this target.
    pub fn pip_platform_tags(&self) -> Result<Vec<String>> {
        Ok(match self.ida_platform.as_str() {
            "windows-x86_64" => vec!["win_amd64".into()],
            "windows-aarch64" => vec!["win_arm64".into()],
            "linux-x86_64" => manylinux_tags((2, 28), "x86_64"),
            "linux-aarch64" => manylinux_tags((2, 28), "aarch64"),
            "macos-aarch64" => mac_tags((11, 0), "arm64"),
            "macos-x86_64" => mac_tags((10, 13), "x86_64"),
            other => {
                let mut available = ALL_PLATFORMS.to_vec();
                available.sort_unstable();
                return Err(Error::Other(format!(
                    "unsupported platform: {other} (available: {})",
                    available.join(", ")
                )));
            }
        })
    }

    /// Arguments for `pip download` targeting this platform/Python pair.
    pub fn pip_download_args(&self) -> Result<Vec<String>> {
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
        for tag in self.pip_platform_tags()? {
            args.push("--platform".into());
            args.push(tag);
        }
        Ok(args)
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
            for fmt in &[arch, "intel", "fat64", "fat32", "universal2", "universal"] {
                if arch == "arm64" && *fmt != "universal2" {
                    continue;
                }
                tags.push(format!("macosx_10_{m}_{fmt}"));
            }
        }
    }
    tags
}
