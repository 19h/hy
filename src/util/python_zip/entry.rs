//! Central member records; retain original names for local-header validation.

use std::io::Read;

use zip::result::{ZipError, ZipResult};

use super::{dword, invalid, names, qword, read_record, word};

pub(super) struct Entry {
    pub name: String,
    pub original_name: String,
    pub header_offset: i128,
    pub end_offset: i128,
    pub flags: u16,
    pub method: u16,
    pub crc: u32,
    pub compressed_size: u64,
    pub size: u64,
    pub external_attributes: u32,
}

impl Entry {
    pub fn read(reader: &mut impl Read, prefix: i128) -> ZipResult<(Self, u64)> {
        let header = read_record::<46>(reader)?;
        if !header.starts_with(b"PK\x01\x02") {
            return Err(invalid("Bad magic number for central directory"));
        }
        let filename = read_bytes(reader, word(&header, 28))?;
        let flags = word(&header, 8);
        let original_name = names::decode(&filename, flags)?;
        let extra = read_bytes(reader, word(&header, 30))?;
        let _comment = read_bytes(reader, word(&header, 32))?;
        if header[6] > 63 {
            return Err(ZipError::UnsupportedArchive("ZIP extraction version exceeds 6.3"));
        }
        let mut entry = Self {
            name: names::sanitize(&original_name),
            original_name,
            header_offset: i128::from(dword(&header, 42)),
            end_offset: 0,
            flags,
            method: word(&header, 10),
            crc: dword(&header, 16),
            compressed_size: u64::from(dword(&header, 20)),
            size: u64::from(dword(&header, 24)),
            external_attributes: dword(&header, 38),
        };
        entry.decode_extra(&extra, crc32fast::hash(&filename))?;
        entry.header_offset += prefix;
        let record_size = 46
            + u64::from(word(&header, 28))
            + u64::from(word(&header, 30))
            + u64::from(word(&header, 32));
        Ok((entry, record_size))
    }

    fn decode_extra(&mut self, mut bytes: &[u8], filename_crc: u32) -> ZipResult<()> {
        while bytes.len() >= 4 {
            let tag = word(bytes, 0);
            let length = usize::from(word(bytes, 2));
            let data = bytes.get(4..4 + length).ok_or_else(|| invalid("Corrupt extra field"))?;
            match tag {
                1 => self.decode_zip64(data)?,
                0x7075 => self.decode_unicode_path(data, filename_crc)?,
                _ => (),
            }
            bytes = &bytes[4 + length..];
        }
        Ok(())
    }

    fn decode_zip64(&mut self, mut data: &[u8]) -> ZipResult<()> {
        if self.size == u64::from(u32::MAX) || self.size == u64::MAX {
            self.size = take_zip64(&mut data)?;
        }
        if self.compressed_size == u64::from(u32::MAX) {
            self.compressed_size = take_zip64(&mut data)?;
        }
        if self.header_offset == i128::from(u32::MAX) {
            self.header_offset = i128::from(take_zip64(&mut data)?);
        }
        Ok(())
    }

    fn decode_unicode_path(&mut self, data: &[u8], filename_crc: u32) -> ZipResult<()> {
        if data.len() < 5 {
            return Err(invalid("Corrupt Unicode path extra field"));
        }
        if data[0] == 1 && dword(data, 1) == filename_crc {
            let name = std::str::from_utf8(&data[5..])
                .map_err(|_| invalid("Invalid UTF-8 in Unicode path extra field"))?;
            if !name.is_empty() {
                self.name = names::sanitize(name);
            }
        }
        Ok(())
    }
}

fn read_bytes(reader: &mut impl Read, length: u16) -> ZipResult<Vec<u8>> {
    let mut bytes = Vec::new();
    reader.take(u64::from(length)).read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn take_zip64(data: &mut &[u8]) -> ZipResult<u64> {
    if data.len() < 8 {
        return Err(invalid("Corrupt ZIP64 extra field"));
    }
    let value = qword(data, 0);
    *data = &data[8..];
    Ok(value)
}
