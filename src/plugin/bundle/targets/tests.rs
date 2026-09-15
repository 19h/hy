use serde_json::{Value, json};

use super::{PipTarget, reference, resolve_platform_alias};

#[test]
fn target_grammar_matches_source_unicode_newlines_aliases_and_integer_limits() {
    let platforms = [
        "linux-x86_64",
        "windows",
        "WIN",
        "\u{1c}macos-arm64\u{1f}",
        "",
        "unknown",
        "a'b",
        "a\"b",
        "a\nb",
        "linux-x86_64\n",
    ];
    let mut versions: Vec<String> = [
        "",
        "3",
        "3.9",
        "3.10",
        "03.012",
        "4.0",
        "2.99",
        "3.12\n",
        "3.12\n\n",
        "3.12\r\n",
        " 3.12",
        "3.12 ",
        "+3.12",
        "3.1_2",
        "3.12.0",
        "٣.١٢",
        "３.１２",
        "𝟛.𝟙𝟚",
        "3.²",
        "3.12'",
        "3.12\0",
        "4294967296.0",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    for size in [4299, 4300, 4301] {
        versions.extend([format!("{}.0", "4".repeat(size)), format!("3.{}", "٠".repeat(size))]);
    }
    let mut cases = Vec::new();
    for platform in platforms {
        cases.push(json!({
            "mode": "alias", "platform": platform,
            "expected": reference::outcome(resolve_platform_alias(platform)),
        }));
        for version in &versions {
            cases.push(json!({
                "mode": "new", "platform": platform, "version": version,
                "expected": target_result(PipTarget::new(platform, version)),
            }));
            let id = format!("{platform}-cp{}", version.replace('.', ""));
            cases.push(json!({
                "mode": "parse", "id": id,
                "expected": target_result(PipTarget::parse(&id)),
            }));
        }
    }
    for id in [
        "",
        "-cp312",
        "linux-x86_64-cp3",
        "linux-x86_64-cp",
        "linux-cp312-cp314",
        "linux-x86_64-CP312",
    ] {
        cases.push(
            json!({"mode": "parse", "id": id, "expected": target_result(PipTarget::parse(id))}),
        );
    }
    assert_eq!(cases.len(), 576);
    reference::compare(&cases);
    reference::digest(&cases, "1d6807459362b94b4628c42dbc7e21f03288c6d4768601895f34f6adbf49d20c");
}

fn target_result(result: crate::error::Result<PipTarget>) -> Value {
    reference::outcome(result.map(|target| {
        json!({
            "id": target.id(), "platform": target.ida_platform, "version": target.python_version,
            "abis": target.abis(), "tags": reference::outcome(target.pip_platform_tags()),
        })
    }))
}
