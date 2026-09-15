use sha2::{Digest, Sha256};

use super::*;

fn manifest(name: &str, version: &str) -> Value {
    let mut manifest = identity_manifest(version, "https://github.com/Example/Original/");
    manifest["plugin"]["name"] = json!(name);
    manifest
}

fn package(manifest: &Value) -> Vec<u8> {
    zip(&[("ida-plugin.json", &serde_json::to_vec(manifest).unwrap()), ("plugin.py", b"# plugin")])
}

#[test]
fn case_variants_share_identity_and_use_the_last_ordered_display_name() {
    let sandbox = Sandbox::new();
    let path = sandbox.path().join("bundle.zip");
    for (left, right, display) in [
        ("1", "2", "EXAMPLE"),
        ("2", "1", "example"),
        ("1", "1.0", "EXAMPLE"),
        ("1", "1", "EXAMPLE"),
        ("10", "2", "example"),
    ] {
        let first = package(&manifest("example", left));
        let last = package(&manifest("EXAMPLE", right));
        for reverse in [false, true] {
            let mut members =
                vec![("plugins/a.zip", first.as_slice()), ("plugins/z.zip", last.as_slice())];
            if reverse {
                members.reverse();
            }
            fs::write(&path, bundle(&members)).unwrap();
            let result = inspect(&sandbox, &path, true);
            assert_eq!(result["success"], true);
            let display = if reverse && left == "1" && right == "1.0" {
                "example"
            } else {
                display
            };
            let mut versions = vec![left, right];
            versions.sort();
            versions.dedup();
            assert_eq!(
                result["report"],
                report(&path, &format!("  plugins: 1\n    {display}: {}\n", versions.join(", "),))
            );
        }
    }
}

#[test]
fn incomparable_compatibility_sets_preserve_source_display_selection() {
    let sandbox = Sandbox::new();
    let path = sandbox.path().join("bundle.zip");
    let mut archives = Vec::new();
    for (name, ida, platforms) in [
        ("EXAMPLE", vec!["9.0"], vec!["linux-x86_64"]),
        ("Example", vec!["9.1"], vec!["windows-x86_64"]),
        ("eXample", vec!["9.0", "9.1"], vec!["linux-x86_64"]),
        ("examplE", vec!["9.0"], vec!["linux-x86_64", "windows-x86_64"]),
    ] {
        let mut manifest = manifest(name, "1");
        manifest["plugin"]["idaVersions"] = json!(ida);
        manifest["plugin"]["platforms"] = json!(platforms);
        archives.push(package(&manifest));
    }
    let names = ["plugins/a.zip", "plugins/b.zip", "plugins/c.zip", "plugins/d.zip"];
    let mut reports = Vec::new();
    for a in 0..4 {
        for b in 0..4 {
            if b == a {
                continue;
            }
            for c in 0..4 {
                if c == a || c == b {
                    continue;
                }
                let d = (0..4).find(|d| *d != a && *d != b && *d != c).unwrap();
                let members: Vec<_> = [a, b, c, d]
                    .into_iter()
                    .map(|index| (names[index], archives[index].as_slice()))
                    .collect();
                fs::write(&path, bundle(&members)).unwrap();
                let result = inspect(&sandbox, &path, true);
                assert_eq!(result["success"], true);
                let text = result["report"].as_str().unwrap();
                assert!(text.contains("  plugins: 1\n"));
                reports.push(text.lines().last().unwrap().to_owned());
            }
        }
    }
    assert_eq!(reports.len(), 24);
    assert_eq!(
        format!("{:x}", Sha256::digest(serde_json::to_vec(&reports).unwrap())),
        "15f66cd82eb0a1f974e402ba42d17d95c4f0d74e2e0c0a57a6c5d1ebd5d71828",
    );
}

#[test]
fn duplicate_location_ties_compare_descriptor_equality_before_reporting() {
    let sandbox = Sandbox::new();
    let path = sandbox.path().join("bundle.zip");
    for name in ["example", "EXAMPLE"] {
        compare_duplicates(
            &sandbox,
            &path,
            &manifest("example", "1"),
            &manifest(name, "1"),
            name == "example",
        );
    }
    let first = manifest("example", "1");
    let mut second = first.clone();
    second["$schema"] = json!("schema:fixture");
    compare_duplicates(&sandbox, &path, &first, &second, false);

    for (left, right, equal) in [
        ("1", "true", true),
        ("9007199254740993", "9007199254740993.0", false),
        ("18446744073709551616", "18446744073709551616.0", true),
    ] {
        let mut first = manifest("example", "1");
        first["plugin"]["fixture"] =
            json!({"nested": [serde_json::from_str::<Value>(left).unwrap()]});
        let mut second = manifest("example", "1");
        second["plugin"]["fixture"] =
            json!({"nested": [serde_json::from_str::<Value>(right).unwrap()]});
        compare_duplicates(&sandbox, &path, &first, &second, equal);
    }
}

fn compare_duplicates(
    sandbox: &Sandbox,
    path: &Path,
    first: &Value,
    second: &Value,
    success: bool,
) {
    let first = serde_json::to_vec(first).unwrap();
    let second = serde_json::to_vec(second).unwrap();
    let archive = zip(&[
        ("first/ida-plugin.json", &first),
        ("first/plugin.py", b"# first"),
        ("second/ida-plugin.json", &second),
        ("second/plugin.py", b"# second"),
    ]);
    fs::write(path, bundle(&[("plugins/example.zip", &archive)])).unwrap();
    let result = inspect(sandbox, path, true);
    assert_eq!(result["success"], success);
    assert_eq!(
        result["report"],
        report(
            path,
            if success {
                "  plugins: 1\n    example: 1\n"
            } else {
                ""
            }
        )
    );
}
