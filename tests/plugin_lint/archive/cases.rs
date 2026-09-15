//! Ordered raw ZIP shapes for the lint command's read and reporting policy.

use serde_json::json;

#[path = "../../../src/util/python_zip/fixtures.rs"]
mod fixtures;

use fixtures::{Member, central_offset, local_offset, zip};

fn descriptor(complete: bool) -> Vec<u8> {
    let mut manifest = crate::support::identity_manifest("1", "https://github.com/example/lint");
    if complete {
        for (field, value) in [
            ("description", json!("Fixture")),
            ("categories", json!(["decompilation"])),
            ("keywords", json!(["fixture"])),
            ("license", json!("MIT")),
            ("logoPath", json!("plugin.py")),
            ("authors", json!([{"name": "Fixture", "email": "author@example.test"}])),
        ] {
            manifest["plugin"][field] = value;
        }
    }
    serde_json::to_vec(&manifest).unwrap()
}

pub(super) fn missing_reference_then_invalid_descriptor() -> Vec<u8> {
    zip(&[
        Member::new(b"a/ida-plugin.json", &descriptor(false)),
        Member::new(b"b/ida-plugin.json", b"invalid JSON"),
    ])
}

pub(super) fn archives() -> Vec<Vec<u8>> {
    let mut archives = Vec::new();
    let incomplete = descriptor(false);
    let complete = descriptor(true);
    for prefix in
        ["", "pkg/", "./pkg/", "pkg//./", "/pkg/", "//pkg/", "pkg\\nested/", "pkg/../other/"]
    {
        for leaf in ["ida-plugin.json", "extraida-plugin.json"] {
            for readmes in [
                &[][..],
                &["README.md"],
                &["readme.txt"],
                &["./README.md"],
                &["nested/../README.md"],
                &["README.md/"],
                &["readme-z.txt", "readme-a.txt"],
                &["./readme-a.txt", "README.md"],
            ] {
                let mut members = vec![
                    Member::new(format!("{prefix}{leaf}").as_bytes(), &incomplete),
                    Member::new(format!("{prefix}plugin.py").as_bytes(), b"fixture"),
                ];
                members.extend(
                    readmes
                        .iter()
                        .map(|name| Member::new(format!("{prefix}{name}").as_bytes(), b"fixture")),
                );
                archives.push(zip(&members));
            }
        }
    }
    for first in [&incomplete[..], &complete, b"invalid JSON", b"\xff"] {
        for last in [&incomplete[..], &complete, b"invalid JSON", b"\xff"] {
            for duplicate in [false, true] {
                archives.push(zip(&[
                    Member::new(b"a/ida-plugin.json", first),
                    Member::new(
                        if duplicate {
                            b"a/ida-plugin.json"
                        } else {
                            b"b/ida-plugin.json"
                        },
                        last,
                    ),
                    Member::new(b"a/plugin.py", b"fixture"),
                    Member::new(b"b/plugin.py", b"fixture"),
                ]));
            }
        }
    }
    for index in 0..4 {
        for fault in ["header", "crc", "symlink", "encrypted"] {
            let mut bytes = zip(&[
                Member::new(b"ida-plugin.json", &incomplete),
                Member::new(b"plugin.py", b"fixture"),
                Member::new(b"README.md", b"fixture"),
                Member::new(b"unrelated", b"fixture"),
            ]);
            let central = central_offset(&bytes, index);
            let local = local_offset(&bytes, index);
            match fault {
                "header" => bytes[local] = 0,
                "crc" => bytes[central + 16] ^= 1,
                "symlink" => bytes[central + 38..central + 42]
                    .copy_from_slice(&0xa000_0000_u32.to_le_bytes()),
                "encrypted" => bytes[central + 8] |= 1,
                _ => unreachable!(),
            }
            archives.push(bytes);
        }
    }
    for root in ["café", "日本語"] {
        archives.push(zip(&[
            Member::new(format!("{root}/ida-plugin.json").as_bytes(), &incomplete).utf8(),
            Member::new(format!("{root}/plugin.py").as_bytes(), b"fixture").utf8(),
            Member::new(b"alias", b"fixture").unicode_path(format!("{root}/README.md").as_bytes()),
        ]));
    }
    archives.push(missing_reference_then_invalid_descriptor());
    archives.push(zip(&[]));
    archives.push(b"invalid ZIP".to_vec());
    archives
}
