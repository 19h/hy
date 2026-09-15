use super::*;

pub(super) fn cases() -> Vec<(String, Vec<u8>)> {
    let ordinary = zip(&[Member::new(b"member", b"payload")]);
    let mut cases = Vec::new();
    for mask in 0..8_u8 {
        let bytes = large_member(&ordinary, mask);
        cases.push((format!("zip64-member-{mask}"), bytes.clone()));
        for extension in [0, 1, 16] {
            let bytes = large_directory(&bytes, extension);
            for prefix in [b"".as_slice(), b"executable prefix"] {
                let mut prefixed = prefix.to_vec();
                prefixed.extend_from_slice(&bytes);
                cases.push((format!("zip64-{mask}-{extension}-{prefix:?}"), prefixed));
            }
        }
    }
    let valid = large_directory(&ordinary, 0);
    let locator = valid.len() - 42;
    let record = locator - 56;
    for offset in [
        record,
        record + 4,
        record + 40,
        record + 48,
        locator,
        locator + 4,
        locator + 8,
        locator + 16,
    ] {
        for value in [0_u8, 1, 0xff] {
            let mut bytes = valid.clone();
            bytes[offset] = value;
            cases.push((format!("zip64-damage-{offset}-{value}"), bytes));
        }
    }
    cases
}

fn large_member(ordinary: &[u8], mask: u8) -> Vec<u8> {
    let central = central_offset(ordinary, 0);
    let end = ordinary.len() - 22;
    let mut bytes = ordinary[..end].to_vec();
    let mut data = Vec::new();
    for (bit, offset) in [(1, 24), (2, 20), (4, 42)] {
        if mask & bit != 0 {
            data.extend_from_slice(&u64::from(dword(&bytes, central + offset)).to_le_bytes());
            bytes[central + offset..central + offset + 4].fill(0xff);
        }
    }
    let length = u16::try_from(data.len()).unwrap();
    bytes[central + 30..central + 32].copy_from_slice(&(length + 4).to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&length.to_le_bytes());
    bytes.extend_from_slice(&data);
    let size = u32::try_from(bytes.len() - central).unwrap();
    let mut footer = ordinary[end..].to_vec();
    footer[12..16].copy_from_slice(&size.to_le_bytes());
    bytes.extend_from_slice(&footer);
    bytes
}

fn large_directory(ordinary: &[u8], extension: usize) -> Vec<u8> {
    let end = ordinary.len() - 22;
    let central = central_offset(ordinary, 0);
    let mut bytes = ordinary[..end].to_vec();
    let mut record = vec![0; 56 + extension];
    record[..4].copy_from_slice(b"PK\x06\x06");
    record[4..12].copy_from_slice(&(44 + extension as u64).to_le_bytes());
    record[12..14].copy_from_slice(&45_u16.to_le_bytes());
    record[14..16].copy_from_slice(&45_u16.to_le_bytes());
    record[24..32].copy_from_slice(&1_u64.to_le_bytes());
    record[32..40].copy_from_slice(&1_u64.to_le_bytes());
    record[40..48].copy_from_slice(&((end - central) as u64).to_le_bytes());
    record[48..56].copy_from_slice(&(central as u64).to_le_bytes());
    bytes.extend_from_slice(&record);
    let mut locator = [0; 20];
    locator[..4].copy_from_slice(b"PK\x06\x07");
    locator[8..16].copy_from_slice(&(end as u64).to_le_bytes());
    locator[16..20].copy_from_slice(&1_u32.to_le_bytes());
    bytes.extend_from_slice(&locator);
    let mut footer = ordinary[end..].to_vec();
    footer[8..20].fill(0xff);
    bytes.extend_from_slice(&footer);
    bytes
}
