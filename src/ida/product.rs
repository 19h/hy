//! Installer product names and edition-specific destination directories.

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

#[derive(Debug, PartialEq, Eq)]
pub struct IdaProduct {
    pub name: String,
    pub major: u8,
    pub minor: u8,
    pub suffix: String,
}

impl IdaProduct {
    pub fn from_installer(path: &Path) -> Result<Self> {
        let filename = path.file_name().and_then(|name| name.to_str()).unwrap_or_default();
        let pattern =
            regex::Regex::new(r"^ida-([^_]+)_([0-9]{2})(sp[0-9]+)?_").expect("constant expression");
        let captures = pattern.captures(filename).ok_or_else(|| {
            Error::IdaInstallFailed(format!("unrecognized installer filename: {filename}"))
        })?;
        let edition = &captures[1];
        let name = match edition {
            "pro" => "IDA Professional".into(),
            "classroom" | "classroom-free" => "IDA Classroom".into(),
            "essential" => "IDA Essential".into(),
            "free" | "free-pc" => "IDA Free".into(),
            "home-arm" => "IDA Home (ARM)".into(),
            "home-mips" => "IDA Home (MIPS)".into(),
            "home-pc" => "IDA Home (PC)".into(),
            "home-ppc" => "IDA Home (PPC)".into(),
            "home-riscv" => "IDA Home (RISC-V)".into(),
            other => format!(
                "IDA {}",
                other
                    .split('-')
                    .map(|word| {
                        let mut characters = word.chars();
                        characters
                            .next()
                            .map(|first| first.to_uppercase().to_string() + characters.as_str())
                            .unwrap_or_default()
                    })
                    .collect::<Vec<_>>()
                    .join("-")
            ),
        };
        Ok(Self {
            name,
            major: captures[2].as_bytes()[0] - b'0',
            minor: captures[2].as_bytes()[1] - b'0',
            suffix: captures.get(3).map(|value| value.as_str()).unwrap_or_default().into(),
        })
    }

    pub fn default_directory(&self) -> Result<PathBuf> {
        let name = format!("{} {}.{}{}", self.name, self.major, self.minor, self.suffix);
        if cfg!(target_os = "macos") {
            Ok(Path::new("/Applications").join(format!("{name}.app")))
        } else if cfg!(windows) {
            Ok(PathBuf::from(
                std::env::var_os("ProgramFiles").unwrap_or_else(|| r"C:\Program Files".into()),
            )
            .join(name))
        } else {
            let home = dirs::home_dir()
                .ok_or_else(|| Error::Other("home directory is unavailable".into()))?;
            let name = if self.major == 9 && self.minor == 2 {
                name.replace(' ', "-")
            } else {
                name
            };
            Ok(home.join(".local/share/applications").join(name))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_editions_service_packs_and_legacy_asset_names() {
        for (filename, name, suffix) in [
            ("ida-pro_92sp1_x64linux.run", "IDA Professional", "sp1"),
            ("ida-home-riscv_92_x64win.exe", "IDA Home (RISC-V)", ""),
            ("ida-free-pc_92_armmac.app.zip", "IDA Free", ""),
            ("ida-classroom-free_92_x64win.exe", "IDA Classroom", ""),
        ] {
            let product = IdaProduct::from_installer(Path::new(filename)).unwrap();
            assert_eq!(product.name, name);
            assert_eq!((product.major, product.minor), (9, 2));
            assert_eq!(product.suffix, suffix);
        }
        for invalid in ["installer.zip", "ida-pro_9_x64linux.run", "ida-pro_999_x64linux.run"] {
            assert!(IdaProduct::from_installer(Path::new(invalid)).is_err());
        }
    }
}
