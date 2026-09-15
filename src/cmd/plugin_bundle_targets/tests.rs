use std::cell::RefCell;

use serde_json::json;

use super::BundleCreateArgs;

use crate::plugin::bundle::target_reference as reference;

#[tokio::test]
async fn target_selection_matches_source_precedence_observations_and_deduplication() {
    let platform_lists = [
        vec![],
        vec!["all"],
        vec!["current"],
        vec!["current", "current"],
        vec!["windows", "WIN"],
        vec!["linux", "all", "linux"],
        vec!["\u{1c}ALL\u{1f}"],
        vec!["bad", "current"],
        vec!["current", "bad"],
    ];
    let python_lists = [
        vec![],
        vec!["all"],
        vec!["current"],
        vec!["current", "current"],
        vec!["3.12", "03.12", "3.12"],
        vec!["٣.١٢"],
        vec!["3.9"],
        vec!["\u{1c}ALL\u{1f}"],
        vec!["bad", "current"],
        vec!["current", "bad"],
    ];
    let target_lists =
        [vec![], vec!["linux-x86_64-cp312"], vec!["windows-cp312", "windows-cp312"], vec!["bad"]];
    let mut cases = Vec::new();
    for platforms in &platform_lists {
        for pythons in &python_lists {
            for targets in &target_lists {
                for (platform, python) in [
                    (Some("windows-x86_64"), Some("3.12")),
                    (Some("windows"), Some("3.12")),
                    (Some("bad"), Some("3.9")),
                    (None, Some("3.12")),
                    (Some("windows-x86_64"), None),
                ] {
                    let args = BundleCreateArgs {
                        output: "unused.zip".into(),
                        repo: None,
                        plugin_specs: vec!["fixture".into()],
                        platforms: strings(platforms),
                        pythons: strings(pythons),
                        targets: strings(targets),
                    };
                    let calls = RefCell::new(Vec::new());
                    let result = super::resolve_with(
                        &args,
                        || {
                            calls.borrow_mut().push("platform");
                            observed(platform, "platform failed")
                        },
                        || {
                            calls.borrow_mut().push("python");
                            std::future::ready(observed(python, "python failed"))
                        },
                    )
                    .await;
                    let value = reference::outcome(result.map(|targets| {
                        targets
                            .into_iter()
                            .map(|target| {
                                json!({
                                    "id": target.id(), "platform": target.ida_platform,
                                    "version": target.python_version,
                                })
                            })
                            .collect::<Vec<_>>()
                    }));
                    cases.push(json!({
                        "mode": "resolve", "platforms": platforms,
                        "pythons": pythons, "targets": targets,
                        "platform": platform, "version": python,
                        "expected": {"result": value, "calls": calls.into_inner()},
                    }));
                }
            }
        }
    }
    assert_eq!(cases.len(), 1800);
    reference::compare(&cases);
    reference::digest(&cases, "438838f8755cb0c91d55a3871d35c083552c486cd7a5b3168884ea9ec9c1f6a2");
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).into()).collect()
}

fn observed(value: Option<&str>, message: &str) -> crate::error::Result<String> {
    value.map(str::to_owned).ok_or_else(|| crate::error::Error::Other(message.into()))
}
