//! Deterministic raw ZIP fixtures, including names rejected by ZIP writers.

#[derive(Clone)]
pub(crate) struct Member {
    pub name: Vec<u8>,
    pub local_name: Vec<u8>,
    pub flags: u16,
    pub local_flags: u16,
    pub extra: Vec<u8>,
    pub body: Vec<u8>,
}

impl Member {
    pub fn new(name: &[u8], body: &[u8]) -> Self {
        Self {
            name: name.to_vec(),
            local_name: name.to_vec(),
            flags: 0,
            local_flags: 0,
            extra: Vec::new(),
            body: body.to_vec(),
        }
    }

    pub fn utf8(mut self) -> Self {
        self.flags |= 0x800;
        self.local_flags |= 0x800;
        self
    }

    pub fn unicode_path(mut self, name: &[u8]) -> Self {
        self.extra.extend_from_slice(&0x7075_u16.to_le_bytes());
        self.extra.extend_from_slice(&u16::try_from(name.len() + 5).unwrap().to_le_bytes());
        self.extra.push(1);
        self.extra.extend_from_slice(&crc32fast::hash(&self.name).to_le_bytes());
        self.extra.extend_from_slice(name);
        self
    }
}

pub(crate) fn zip(members: &[Member]) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut central = Vec::new();
    for member in members {
        let offset = u32::try_from(bytes.len()).unwrap();
        let size = u32::try_from(member.body.len()).unwrap();
        let crc = crc32fast::hash(&member.body);
        let mut local = [0; 30];
        local[..4].copy_from_slice(b"PK\x03\x04");
        local[4..6].copy_from_slice(&20_u16.to_le_bytes());
        local[6..8].copy_from_slice(&member.local_flags.to_le_bytes());
        local[12..14].copy_from_slice(&33_u16.to_le_bytes());
        local[14..18].copy_from_slice(&crc.to_le_bytes());
        local[18..22].copy_from_slice(&size.to_le_bytes());
        local[22..26].copy_from_slice(&size.to_le_bytes());
        local[26..28]
            .copy_from_slice(&u16::try_from(member.local_name.len()).unwrap().to_le_bytes());
        bytes.extend_from_slice(&local);
        bytes.extend_from_slice(&member.local_name);
        bytes.extend_from_slice(&member.body);

        let mut record = [0; 46];
        record[..4].copy_from_slice(b"PK\x01\x02");
        record[4..6].copy_from_slice(&20_u16.to_le_bytes());
        record[6..28].copy_from_slice(&local[4..26]);
        record[8..10].copy_from_slice(&member.flags.to_le_bytes());
        record[28..30].copy_from_slice(&u16::try_from(member.name.len()).unwrap().to_le_bytes());
        record[30..32].copy_from_slice(&u16::try_from(member.extra.len()).unwrap().to_le_bytes());
        record[42..46].copy_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(&record);
        central.extend_from_slice(&member.name);
        central.extend_from_slice(&member.extra);
    }
    let mut end = [0; 22];
    end[..4].copy_from_slice(b"PK\x05\x06");
    let count = u16::try_from(members.len()).unwrap();
    end[8..10].copy_from_slice(&count.to_le_bytes());
    end[10..12].copy_from_slice(&count.to_le_bytes());
    end[12..16].copy_from_slice(&u32::try_from(central.len()).unwrap().to_le_bytes());
    end[16..20].copy_from_slice(&u32::try_from(bytes.len()).unwrap().to_le_bytes());
    bytes.extend_from_slice(&central);
    bytes.extend_from_slice(&end);
    bytes
}

pub(crate) fn central_offset(bytes: &[u8], index: usize) -> usize {
    let mut offset = dword(bytes, bytes.len() - 6) as usize;
    for _ in 0..index {
        offset += 46
            + usize::from(word(bytes, offset + 28))
            + usize::from(word(bytes, offset + 30))
            + usize::from(word(bytes, offset + 32));
    }
    offset
}

pub(crate) fn local_offset(bytes: &[u8], index: usize) -> usize {
    dword(bytes, central_offset(bytes, index) + 42) as usize
}

fn word(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
}

fn dword(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}
