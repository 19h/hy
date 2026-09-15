//! Locate the central directory, including concatenated and ZIP64 archives.

use std::io::{Read, Seek, SeekFrom};

use zip::result::ZipResult;

use super::{Entry, dword, invalid, qword, read_record};

struct Directory {
    end: u64,
    size: u64,
    relative_start: u64,
}

pub(super) fn read(reader: &mut (impl Read + Seek)) -> ZipResult<Vec<Entry>> {
    let directory = locate(reader)?;
    let start = directory
        .end
        .checked_sub(directory.size)
        .ok_or_else(|| invalid("Bad offset for central directory"))?;
    let prefix = i128::from(start) - i128::from(directory.relative_start);
    reader.seek(SeekFrom::Start(start))?;
    let mut central = reader.take(directory.size);
    let mut remaining = directory.size;
    let mut entries = Vec::new();
    // Python iterates the declared directory byte size, not the entry count.
    while remaining != 0 {
        let (entry, record_size) = Entry::read(&mut central, prefix)?;
        entries.push(entry);
        remaining = remaining.saturating_sub(record_size);
    }
    let mut order: Vec<_> = (0..entries.len()).collect();
    order.sort_by_key(|&index| entries[index].header_offset);
    let mut end = i128::from(start);
    for index in order.into_iter().rev() {
        entries[index].end_offset = end;
        end = entries[index].header_offset;
    }
    Ok(entries)
}

fn locate(reader: &mut (impl Read + Seek)) -> ZipResult<Directory> {
    let length = reader.seek(SeekFrom::End(0))?;
    if length < 22 {
        return Err(invalid("File is not a zip file"));
    }
    reader.seek(SeekFrom::End(-22))?;
    let last = read_record::<22>(reader)?;
    let (position, record) = if last.starts_with(b"PK\x05\x06") && last[20..] == [0, 0] {
        (length - 22, last)
    } else {
        let start = length.saturating_sub(65_535 + 22);
        reader.seek(SeekFrom::Start(start))?;
        let mut tail = Vec::new();
        reader.take(65_535 + 22).read_to_end(&mut tail)?;
        let offset = tail
            .windows(4)
            .rposition(|bytes| bytes == b"PK\x05\x06")
            .ok_or_else(|| invalid("File is not a zip file"))?;
        let record = tail
            .get(offset..offset + 22)
            .ok_or_else(|| invalid("Truncated end of central directory"))?;
        (start + offset as u64, record.try_into().expect("22-byte record"))
    };
    let directory = Directory {
        end: position,
        size: u64::from(dword(&record, 12)),
        relative_start: u64::from(dword(&record, 16)),
    };
    read_zip64(reader, position, directory)
}

fn read_zip64(
    reader: &mut (impl Read + Seek),
    end_position: u64,
    ordinary: Directory,
) -> ZipResult<Directory> {
    let Some(locator_position) = end_position.checked_sub(20) else {
        return Ok(ordinary);
    };
    reader.seek(SeekFrom::Start(locator_position))?;
    let locator = read_record::<20>(reader)?;
    if !locator.starts_with(b"PK\x06\x07") {
        return Ok(ordinary);
    }
    if dword(&locator, 4) != 0 || dword(&locator, 16) > 1 {
        return Err(invalid("ZIP archives spanning multiple disks are not supported"));
    }
    let candidate =
        locator_position.checked_sub(56).ok_or_else(|| invalid("Corrupt ZIP64 locator"))?;
    let relative = qword(&locator, 8);
    if relative > candidate {
        return Err(invalid("Corrupt ZIP64 locator"));
    }
    reader.seek(SeekFrom::Start(relative))?;
    let mut record = read_record::<56>(reader)?;
    let mut extra_size = candidate - relative;
    if !record.starts_with(b"PK\x06\x06") && relative != candidate {
        reader.seek(SeekFrom::Start(candidate))?;
        record = read_record::<56>(reader)?;
        extra_size = 0;
    }
    if !record.starts_with(b"PK\x06\x06") {
        return Err(invalid("ZIP64 end of central directory record not found"));
    }
    let size = qword(&record, 40);
    let relative_start = qword(&record, 48);
    if relative_start.checked_add(size) != Some(relative)
        || qword(&record, 4).checked_add(12) != 56_u64.checked_add(extra_size)
    {
        return Err(invalid("Corrupt ZIP64 end of central directory record"));
    }
    Ok(Directory {
        end: candidate - extra_size,
        size,
        relative_start,
    })
}
