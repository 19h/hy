use super::*;

mod lzma;
mod zip64;

pub(super) fn all() -> Vec<(String, Vec<u8>)> {
    let mut cases = Vec::new();
    let members = [
        Member::new(b"a", b"first"),
        Member::new(b"a", b"last"),
        Member::new(b"\xc3\xa9", b"CP437"),
        Member::new(b"\xc3\xa9", b"UTF8").utf8(),
    ];
    for length in 0..=4 {
        for mut seed in 0..4_usize.pow(length) {
            let name = format!("order-{length}-{seed}");
            let selected: Vec<_> = (0..length)
                .map(|_| {
                    let member = members[seed % 4].clone();
                    seed /= 4;
                    member
                })
                .collect();
            cases.push((name, zip(&selected)));
        }
    }
    for byte in 0..=255 {
        let name = [b'x', byte];
        cases.push((format!("cp437-{byte}"), zip(&[Member::new(&name, &[byte])])));
    }
    for name in [b"\xff".as_slice(), b"\xc3", b"\xed\xa0\x80", b"\0suffix", b"a\\b"] {
        cases.push((format!("utf8-{name:?}"), zip(&[Member::new(name, b"payload").utf8()])));
    }
    for name in [b"renamed".as_slice(), b"", b"same\0suffix", b"\xff", b"a\\b"] {
        let unicode = Member::new(b"original", b"unicode").unicode_path(name);
        cases.push((format!("unicode-{name:?}"), zip(std::slice::from_ref(&unicode))));
        for index in [4, 5] {
            let mut invalid = unicode.clone();
            invalid.extra[index] ^= 2;
            cases.push((format!("ignored-unicode-{name:?}-{index}"), zip(&[invalid])));
        }
    }
    cases.push((
        "unicode-collision".into(),
        zip(&[
            Member::new(b"a", b"first").unicode_path(b"same"),
            Member::new(b"b", b"second").unicode_path(b"same"),
            Member::new(b"a", b"third").unicode_path(b"different"),
        ]),
    ));
    cases.push((
        "nul-collision".into(),
        zip(&[Member::new(b"same\0first", b"first"), Member::new(b"same\0last", b"last")]),
    ));
    for length in 0..=8 {
        let mut member = Member::new(b"extra", b"payload").unicode_path(b"renamed");
        member.extra.truncate(length);
        cases.push((format!("truncated-extra-{length}"), zip(&[member])));
    }
    for flags in [0, 1, 8, 0x20, 0x40, 0x61, 0x800] {
        for damage in ["none", "signature", "name", "local-utf8", "crc", "overlap"] {
            let mut member = Member::new(b"member", b"payload");
            member.flags = flags;
            if damage == "name" {
                member.local_name = b"other".to_vec();
            }
            if damage == "local-utf8" {
                member.local_name = b"\xff".to_vec();
                member.local_flags = 0x800;
            }
            let mut bytes = zip(&[member]);
            let central = central_offset(&bytes, 0);
            match damage {
                "signature" => bytes[0] = 0,
                "crc" => bytes[central + 16] ^= 1,
                "overlap" => {
                    bytes[central + 20..central + 24].copy_from_slice(&100_u32.to_le_bytes())
                }
                _ => (),
            }
            cases.push((format!("flags-{flags}-{damage}"), bytes));
        }
    }
    for version in [0, 10, 20, 45, 63, 64, 255, 256, 276] {
        let mut bytes = zip(&[Member::new(b"member", b"payload")]);
        let central = central_offset(&bytes, 0);
        bytes[central + 6..central + 8]
            .copy_from_slice(&u16::try_from(version).unwrap().to_le_bytes());
        cases.push((format!("version-{version}"), bytes));
    }
    let base = zip(&[Member::new(b"member", b"payload")]);
    for count in [0_u16, 1, 2, u16::MAX] {
        let mut bytes = base.clone();
        let end = bytes.len() - 22;
        bytes[end + 8..end + 10].copy_from_slice(&count.to_le_bytes());
        bytes[end + 10..end + 12].copy_from_slice(&count.to_le_bytes());
        cases.push((format!("count-{count}"), bytes));
    }
    for prefix in [b"".as_slice(), b"executable prefix", b"PK\x05\x06"] {
        for suffix in [b"".as_slice(), b"comment", b"PK\x05\x06", b"trailing data"] {
            let mut bytes = prefix.to_vec();
            bytes.extend_from_slice(&base);
            bytes.extend_from_slice(suffix);
            cases.push((format!("prefix-{prefix:?}-suffix-{suffix:?}"), bytes));
        }
    }
    for length in 0..base.len() {
        cases.push((format!("truncated-{length}"), base[..length].to_vec()));
    }
    for method in [
        zip::CompressionMethod::Stored,
        zip::CompressionMethod::Deflated,
        zip::CompressionMethod::Bzip2,
    ] {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .start_file(
                "compressed",
                zip::write::SimpleFileOptions::default()
                    .compression_method(method)
                    .last_modified_time(zip::DateTime::default()),
            )
            .unwrap();
        writer.write_all(b"repeated payload repeated payload repeated payload").unwrap();
        cases.push((format!("codec-{method:?}"), writer.finish().unwrap().into_inner()));
    }
    for field in [28, 30, 32] {
        for length in [0_u16, 1, 6, 7, 100, u16::MAX] {
            let mut bytes = base.clone();
            let central = central_offset(&bytes, 0);
            bytes[central + field..central + field + 2].copy_from_slice(&length.to_le_bytes());
            cases.push((format!("central-field-{field}-{length}"), bytes));
        }
    }
    for offset in [0_u32, 1, 40, 100, u32::MAX] {
        let mut bytes = base.clone();
        let end = bytes.len() - 22;
        bytes[end + 16..end + 20].copy_from_slice(&offset.to_le_bytes());
        cases.push((format!("directory-offset-{offset}"), bytes));
    }
    cases.extend(zip64::cases());
    cases.extend(lzma::cases());
    for size in [0_u32, 1, 6, 7, 8, 100] {
        let mut bytes = base.clone();
        let central = central_offset(&bytes, 0);
        bytes[central + 24..central + 28].copy_from_slice(&size.to_le_bytes());
        cases.push((format!("uncompressed-size-{size}"), bytes));
    }
    for method in 0..=100_u16 {
        if matches!(method, 0 | 8 | 12 | 14) {
            continue;
        }
        let mut bytes = base.clone();
        let central = central_offset(&bytes, 0);
        bytes[central + 10..central + 12].copy_from_slice(&method.to_le_bytes());
        cases.push((format!("unsupported-method-{method}"), bytes));
    }
    cases
}
