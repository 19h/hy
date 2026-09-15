use serde_json::{Value, json};

use super::*;

#[test]
fn release_selection_matches_upstream_tags_drafts_and_stable_ties() {
    let cases = [
        (json!([{"tag_name": "2.0.0", "draft": true}]), ">1.0.0", false, Some("2.0.0")),
        (
            json!([{"tag_name": "2.0.0+first"}, {"tag_name": "v2.0.0+last"}]),
            ">1.0.0",
            false,
            Some("v2.0.0+last"),
        ),
        (
            json!([{"tag_name": "v2.0.0+last"}, {"tag_name": "2.0.0+first"}]),
            ">1.0.0",
            false,
            Some("2.0.0+first"),
        ),
        (
            json!([{"tag_name": "3.0.0-rc.1"}, {"tag_name": "2.0.0"}]),
            ">1.0.0",
            false,
            Some("2.0.0"),
        ),
        (
            json!([{"tag_name": "3.0.0-rc.1"}, {"tag_name": "2.0.0"}]),
            ">1.0.0",
            true,
            Some("3.0.0-rc.1"),
        ),
        (json!([{"tag_name": "2.0.0+dev"}]), ">1.0.0", false, None),
        (json!([{"tag_name": "2.0.0+dev"}]), ">1.0.0", true, Some("2.0.0+dev")),
        (json!([{"tag_name": "2.0.0-test"}]), ">1.0.0", false, Some("2.0.0-test")),
        (json!([{"tag_name": "2.0.0"}]), ">2.0.0", false, None),
        (json!([{"tag_name": "2.0.0"}]), ">=2.0.0", false, Some("2.0.0")),
        (
            json!([{"tag_name": "V2.0.0"}, {"tag_name": "2.0"}, {}, {"tag_name": null}]),
            ">1.0.0",
            false,
            None,
        ),
        (json!([{"tag_name": "vv2.0.0"}]), ">1.0.0", false, Some("vv2.0.0")),
        (json!([{"tag_name": "v\u{1c}2.0.0\u{1f}"}]), ">1.0.0", false, Some("v\u{1c}2.0.0\u{1f}")),
        (json!([{"tag_name": " v2.0.0"}]), ">1.0.0", false, None),
    ];
    for (releases, requirement, include_dev, expected) in &cases {
        let mut latest = None;
        for release in serde_json::from_value::<Vec<Release>>(releases.clone()).unwrap() {
            consider_release(&mut latest, release, requirement, *include_dev);
        }
        assert_eq!(latest.as_ref().map(|release| release.tag.as_str()), *expected, "{releases}");
    }
    if let Some(python) = std::env::var_os("HY_TEST_UPDATE_ORACLE_PYTHON") {
        use std::io::Write;
        use std::process::{Command, Stdio};
        let mut child = Command::new(python).args(["-I", "-B", "-c", r#"
import json, sys
from types import SimpleNamespace
from unittest.mock import patch
from semantic_version import SimpleSpec
from hcli.lib.update.release import GitHubRepo, get_compatible_version
results = []
for releases, requirement, include_dev, _ in json.load(sys.stdin):
    with patch('httpx.get', return_value=SimpleNamespace(text=json.dumps(releases))):
        latest = get_compatible_version(GitHubRepo('fixture', 'repo'), SimpleSpec(requirement), include_dev)
        results.append(latest._origin_tag_name if latest else None)
print(json.dumps(results))
"#]).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
        child.stdin.take().unwrap().write_all(&serde_json::to_vec(&cases).unwrap()).unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success());
        assert_eq!(
            serde_json::from_slice::<Value>(&output.stdout).unwrap(),
            json!(cases.iter().map(|(_, _, _, expected)| expected).collect::<Vec<_>>())
        );
    }
}
