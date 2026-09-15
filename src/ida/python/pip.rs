//! Explicit pip configuration shared by plugin install, upgrade, and migration.

use std::ffi::OsString;

mod links;

pub(crate) use links::normalize_find_links;

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct PipOptions {
    pub index_url: Option<String>,
    pub extra_index_urls: Vec<String>,
    pub find_links: Vec<String>,
    pub no_index: bool,
    pub isolated: bool,
    pub no_cache_dir: bool,
    pub disable_pip_version_check: bool,
    pub no_build_isolation: bool,
    pub skip_environment_check: bool,
}

impl PipOptions {
    pub fn has_custom_sources(&self) -> bool {
        self.index_url.is_some() || !self.extra_index_urls.is_empty() || !self.find_links.is_empty()
    }

    pub fn arguments(&self) -> Vec<OsString> {
        let mut arguments = Vec::new();
        for (enabled, flag) in [
            (self.isolated, "--isolated"),
            (self.disable_pip_version_check, "--disable-pip-version-check"),
            (self.no_cache_dir, "--no-cache-dir"),
            (self.no_index, "--no-index"),
        ] {
            if enabled {
                arguments.push(flag.into());
            }
        }
        if let Some(url) = self.index_url.as_ref().filter(|value| !value.is_empty()) {
            arguments.extend([OsString::from("--index-url"), url.into()]);
        }
        for url in &self.extra_index_urls {
            arguments.extend([OsString::from("--extra-index-url"), url.into()]);
        }
        for path in &self.find_links {
            arguments.extend([OsString::from("--find-links"), path.into()]);
        }
        if self.no_build_isolation {
            arguments.push("--no-build-isolation".into());
        }
        arguments
    }

    pub fn with_wheelhouse(&self, path: &std::path::Path) -> Self {
        let mut options = self.clone();
        options.isolated = true;
        options.no_cache_dir = true;
        options.disable_pip_version_check = true;
        options.find_links.insert(0, path.to_string_lossy().into_owned());
        options
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_repeated_sources_and_paths_as_argv_tokens() {
        let options = PipOptions {
            extra_index_urls: vec![
                "https://one.example/simple".into(),
                "https://two.example/simple".into(),
            ],
            find_links: vec!["/tmp/wheels with spaces".into()],
            no_index: true,
            no_build_isolation: true,
            ..Default::default()
        };
        let expected: Vec<OsString> = [
            "--no-index",
            "--extra-index-url",
            "https://one.example/simple",
            "--extra-index-url",
            "https://two.example/simple",
            "--find-links",
            "/tmp/wheels with spaces",
            "--no-build-isolation",
        ]
        .into_iter()
        .map(OsString::from)
        .collect();
        assert_eq!(options.arguments(), expected);
    }
}
