//! Validate the selected local header, then delegate decompression to `zip`.

use std::io::{Cursor, Read, Seek, SeekFrom, Write};

use zip::result::{ZipError, ZipResult};

use super::{Entry, invalid, names, read_record, word};

pub(super) fn copy<W: Write>(
    reader: &mut (impl Read + Seek),
    entry: &Entry,
    open_destination: impl FnOnce() -> std::io::Result<W>,
) -> ZipResult<W> {
    seek_data(reader, entry)?;
    if entry.flags & 1 != 0 {
        return Err(ZipError::UnsupportedArchive(ZipError::PASSWORD_REQUIRED));
    }
    if !matches!(entry.method, 0 | 8 | 12 | 14) {
        return Err(ZipError::CompressionMethodNotSupported(entry.method));
    }

    // The stream decoder accepts a local header. Supply the authoritative central
    // sizes/CRC, so data descriptors and duplicate names never affect selection.
    // This small in-memory header is chained to the existing compressed bytes;
    // the archive is neither copied nor rewritten.
    let header = decoder_header(entry);
    let mut stream = Cursor::new(header).chain(reader.take(entry.compressed_size));
    let mut file = zip::read::read_zipfile_from_stream(&mut stream)?
        .ok_or_else(|| invalid("Missing decoder header"))?;
    let mut destination = open_destination()?;
    let mut checksum = crc32fast::Hasher::new();
    let mut remaining = entry.size;
    // shutil.copyfileobj uses 64 KiB on Unix and 1 MiB on Windows. Validate the
    // final chunk before publishing it, including an empty file's CRC.
    let chunk_size = if cfg!(windows) {
        1_048_576
    } else {
        65_536
    };
    let mut bytes = Vec::with_capacity(chunk_size as usize);
    loop {
        bytes.clear();
        let requested = remaining.min(chunk_size);
        (&mut file).take(requested).read_to_end(&mut bytes)?;
        checksum.update(&bytes);
        remaining -= bytes.len() as u64;
        let done = remaining == 0 || (bytes.len() as u64) < requested;
        if done && checksum.clone().finalize() != entry.crc {
            return Err(invalid("Bad CRC-32 for file"));
        }
        destination.write_all(&bytes)?;
        if done {
            return Ok(destination);
        }
    }
}

fn seek_data(reader: &mut (impl Read + Seek), entry: &Entry) -> ZipResult<()> {
    let offset = entry.header_offset.try_into().map_err(|_| {
        ZipError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Invalid local header offset",
        ))
    })?;
    reader.seek(SeekFrom::Start(offset))?;
    let header = read_record::<30>(reader)?;
    if !header.starts_with(b"PK\x03\x04") {
        return Err(invalid("Bad magic number for file header"));
    }
    let mut filename = Vec::new();
    reader.take(u64::from(word(&header, 26))).read_to_end(&mut filename)?;
    reader.seek(SeekFrom::Current(i64::from(word(&header, 28))))?;
    if entry.flags & 0x20 != 0 {
        return Err(ZipError::UnsupportedArchive("compressed patched data (flag bit 5)"));
    }
    if entry.flags & 0x40 != 0 {
        return Err(ZipError::UnsupportedArchive("strong encryption (flag bit 6)"));
    }
    if names::decode(&filename, word(&header, 6))? != entry.original_name {
        return Err(invalid("File name in directory and header differ"));
    }
    let end = i128::from(reader.stream_position()?) + i128::from(entry.compressed_size);
    if end > entry.end_offset && entry.end_offset != entry.header_offset {
        return Err(invalid("Overlapped ZIP entries"));
    }
    Ok(())
}

fn decoder_header(entry: &Entry) -> Vec<u8> {
    let mut header = vec![0; 30];
    header[..4].copy_from_slice(b"PK\x03\x04");
    header[4..6].copy_from_slice(&45_u16.to_le_bytes());
    header[6..8].copy_from_slice(&(entry.flags & !8).to_le_bytes());
    header[8..10].copy_from_slice(&entry.method.to_le_bytes());
    header[14..18].copy_from_slice(&entry.crc.to_le_bytes());
    header[18..26].fill(0xff);
    header[26..28].copy_from_slice(&1_u16.to_le_bytes());
    header[28..30].copy_from_slice(&20_u16.to_le_bytes());
    header.push(b'_');
    header.extend_from_slice(&1_u16.to_le_bytes());
    header.extend_from_slice(&16_u16.to_le_bytes());
    header.extend_from_slice(&entry.size.to_le_bytes());
    header.extend_from_slice(&entry.compressed_size.to_le_bytes());
    header
}
