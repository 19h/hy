//! Root-help status from local files only. No authentication or discovery side effects.

use std::path::{Path, PathBuf};

use crate::auth::CredentialsConfig;
use crate::config::{ConfigStore, Env};
use crate::ida;

pub(super) fn summary() -> String {
    let Ok(config) = ConfigStore::read_snapshot() else {
        return String::new();
    };
    let env = Env::global();
    let mut lines = vec!["Status:".into()];
    if env.api_key.is_some() {
        lines.push("  Auth: API key configured (HCLI_API_KEY)".into());
    } else {
        let credentials = config
            .get_value(&format!("{}.credentials", env.config_namespace))
            .and_then(|value| serde_json::from_value::<CredentialsConfig>(value.clone()).ok());
        let email = credentials
            .as_ref()
            .and_then(CredentialsConfig::default_credentials)
            .map(|credential| credential.email.as_str())
            .filter(|email| !email.is_empty());
        lines.push(match email {
            Some(email) => format!("  Auth: {email}"),
            None => format!("  Auth: Not logged in → {} login", env.binary_name),
        });
    }

    let instances = config.get_string_map("ida.instances");
    let default_name = config.get_str("ida.default");
    let default = default_name.and_then(|name| instances.get(name)).map(PathBuf::from);
    if let Some(path) = &default {
        if valid_installation(path) {
            let version = ida::instance_version(path, default_name.unwrap_or_default())
                .map(|version| format!(" {version}"))
                .unwrap_or_default();
            lines.push(format!("  IDA: IDA{version} at {}", path.display()));
        } else {
            lines.push(format!("  IDA: Not found at {}", path.display()));
            lines.push(format!("       → {} ida install -d ida-pro:latest -y", env.binary_name));
        }
    } else if instances.is_empty() {
        lines.push("  IDA: Not installed".into());
        lines.push(format!(
            "       → {} ida install -d ida-pro:latest -l LICENSE_ID -y",
            env.binary_name
        ));
        lines.push(format!("       Find your license ID with: {} license list", env.binary_name));
    } else {
        let valid = instances.values().filter(|path| valid_installation(Path::new(path))).count();
        lines.push(format!(
            "  IDA: {} instance(s), no default set ({valid} valid)",
            instances.len()
        ));
        lines.push(format!("       → {} ida switch", env.binary_name));
    }

    if let Ok(ida_config) = crate::plugin::read_ida_config() {
        let library =
            ida_config["Paths"]["ida-install-dir"].as_str().filter(|path| !path.is_empty());
        if let Some(library) = library {
            let path = ida::normalize_install_dir(Path::new(library));
            if path.exists() && ida::is_idalib_capable(&path) {
                let same = default.as_ref().is_some_and(|default| {
                    matches!((default.canonicalize(), path.canonicalize()), (Ok(left), Ok(right)) if left == right)
                });
                lines.push(if same {
                    "  idalib: active".into()
                } else {
                    format!("  idalib: active ({})", path.display())
                });
            } else {
                lines.push(format!("  idalib: not found at {}", path.display()));
            }
        } else if !instances.is_empty() {
            lines.push("  idalib: not configured (ida-config.json has no ida-install-dir)".into());
        }
    }
    lines.join("\n")
}

fn valid_installation(path: &Path) -> bool {
    path.exists() && ida::ida_binary_path(path).is_some()
}
