use super::*;

pub(super) struct Case {
    pub name: String,
    pub prefix: String,
    pub bytes: Vec<u8>,
}

pub(super) fn all() -> Vec<Case> {
    let mut cases = Vec::new();
    for prefix in ["wheels", "wheels/", "./wheels", "", ".", "a//b", "C:cache"] {
        for name in [
            "fixture.whl",
            "nested/fixture.whl",
            "./fixture.whl",
            "dir/",
            ".",
            "../fixture.whl",
            "/fixture.whl",
            "nested/../fixture.whl",
            "a\\b.whl",
            "C:fixture.whl",
        ] {
            let member = format!("{}/{name}", prefix.trim_end_matches('/'));
            let bytes = zip(&[
                Member::new(b"unrelated/../unsafe", b"ignored"),
                Member::new(member.as_bytes(), b"wheel"),
            ]);
            cases.push(Case {
                name: format!("path-{prefix}-{name}"),
                prefix: prefix.into(),
                bytes,
            });
        }
    }
    for selected in [false, true] {
        for fault in ["symlink", "bad-header", "crc", "encrypted", "unsupported", "utf8"] {
            let name = if selected {
                b"wheels/damaged.whl".as_slice()
            } else {
                b"ignored/damaged.whl"
            };
            let mut bytes = zip(&[
                Member::new(b"wheels/first.whl", b"first"),
                Member::new(name, b"damaged"),
                Member::new(b"wheels/last.whl", b"last"),
            ]);
            let central = central_offset(&bytes, 1);
            let local = crate::util::python_zip::fixtures::local_offset(&bytes, 1);
            match fault {
                "symlink" => bytes[central + 38..central + 42]
                    .copy_from_slice(&0xa000_0000_u32.to_le_bytes()),
                "bad-header" => bytes[local] = 0,
                "crc" => bytes[central + 16] ^= 1,
                "encrypted" => bytes[central + 8] |= 1,
                "unsupported" => bytes[central + 10] = 99,
                "utf8" => {
                    bytes[local + 6..local + 8].copy_from_slice(&0x800_u16.to_le_bytes());
                    bytes[local + 30] = 0xff;
                }
                _ => unreachable!(),
            }
            cases.push(Case {
                name: format!("{fault}-{selected}"),
                prefix: "wheels".into(),
                bytes,
            });
        }
    }
    for members in [
        vec![Member::new(b"wheels/a.whl", b"first"), Member::new(b"wheels/a.whl", b"last")],
        vec![Member::new(b"wheels/a.whl", b"first"), Member::new(b"wheels/nested/a.whl", b"last")],
        vec![
            Member::new(b"wheels/a.whl", b"first"),
            Member::new(b"alias", b"last").unicode_path(b"wheels/a.whl"),
        ],
        vec![Member::new(b"wheels/a.whl", b"first"), Member::new(b"wheels/a.whl\0suffix", b"last")],
    ] {
        cases.push(Case {
            name: format!("duplicate-{}", cases.len()),
            prefix: "wheels".into(),
            bytes: zip(&members),
        });
    }
    for length in [0, 1, 65_535, 65_536, 65_537, 131_072, 131_073] {
        let mut bytes = zip(&[Member::new(b"wheels/crc.whl", &vec![b'x'; length])]);
        let central = central_offset(&bytes, 0);
        bytes[central + 16] ^= 1;
        cases.push(Case {
            name: format!("crc-size-{length}"),
            prefix: "wheels".into(),
            bytes,
        });
    }
    cases
}
