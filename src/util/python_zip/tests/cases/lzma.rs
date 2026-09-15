use super::*;

pub(super) fn cases() -> Vec<(String, Vec<u8>)> {
    let payload = b"repeated payload repeated payload repeated payload";
    // `xz --format=lzma --stdout`: five property bytes, eight size bytes,
    // then the stream. ZIP replaces the size bytes with its version/property header.
    let compressed = [
        9, 4, 5, 0, 0x5d, 0, 0, 0x80, 0, 0x00, 0x39, 0x19, 0x4a, 0x66, 0xee, 0x71, 0xe6, 0xd3,
        0x75, 0xf6, 0xd0, 0xa3, 0x71, 0xc0, 0xf5, 0xca, 0x2b, 0x62, 0xf9, 0xeb, 0x90, 0xf8, 0x47,
        0xff, 0xff, 0xc8, 0xa4, 0x00, 0x00,
    ];
    let mut cases = Vec::new();
    for flags in [0, 2, 8, 10] {
        let mut bytes = zip(&[Member::new(b"lzma", &compressed)]);
        let central = central_offset(&bytes, 0);
        for (offset, value) in [(6, flags), (8, 14), (central + 8, flags), (central + 10, 14)] {
            bytes[offset..offset + 2].copy_from_slice(&u16::try_from(value).unwrap().to_le_bytes());
        }
        for offset in [14, central + 16] {
            bytes[offset..offset + 4].copy_from_slice(&crc32fast::hash(payload).to_le_bytes());
        }
        for offset in [22, central + 24] {
            bytes[offset..offset + 4].copy_from_slice(&(payload.len() as u32).to_le_bytes());
        }
        cases.push((format!("lzma-flags-{flags}"), bytes));
    }
    cases
}
