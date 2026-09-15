//! Ordered ZIP names and last-occurrence lookup, as used by Python's `zipfile`.
//!
//! Directory interpretation is separate from decompression: `zip::ZipArchive`
//! discards duplicate directory records before callers can inspect them.
//!
//! Directory/name interpretation follows CPython 3.13.15 `Lib/zipfile/__init__.py`.
//! See `python_zip/LICENSE` for attribution and the adaptation summary.

use std::collections::HashMap;
use std::io::{Read, Seek, Write};

use zip::result::{ZipError, ZipResult};

mod decoder;
mod directory;
mod entry;
mod names;

use entry::Entry;

pub(crate) struct Archive<R> {
    reader: R,
    entries: Vec<Entry>,
    by_name: HashMap<String, usize>,
}

/// Attributes belong to each directory record, even when names are repeated.
pub(crate) struct Member<'a> {
    pub name: &'a str,
    external_attributes: u32,
}

impl Member<'_> {
    pub fn is_dir(&self) -> bool {
        self.name.ends_with('/')
    }

    pub fn is_symlink(&self) -> bool {
        self.external_attributes >> 28 == 0xa
    }
}

impl<R: Read + Seek> Archive<R> {
    pub fn new(mut reader: R) -> ZipResult<Self> {
        let entries = directory::read(&mut reader)?;
        let by_name =
            entries.iter().enumerate().map(|(index, entry)| (entry.name.clone(), index)).collect();
        Ok(Self {
            reader,
            entries,
            by_name,
        })
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn file_names(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|entry| entry.name.as_str())
    }

    pub fn members(&self) -> impl Iterator<Item = Member<'_>> {
        self.entries.iter().map(|entry| Member {
            name: &entry.name,
            external_attributes: entry.external_attributes,
        })
    }

    pub fn name_for_index(&self, index: usize) -> Option<&str> {
        self.entries.get(index).map(|entry| entry.name.as_str())
    }

    /// A repeated name always reads its last directory record, even while iterating.
    pub fn read(&mut self, name: &str) -> ZipResult<Vec<u8>> {
        self.copy_to(name, || Ok(Vec::new()))
    }

    /// Open the destination after validating the local header, before reading data.
    pub fn copy_to<W: Write>(
        &mut self,
        name: &str,
        open_destination: impl FnOnce() -> std::io::Result<W>,
    ) -> ZipResult<W> {
        let index = *self.by_name.get(name).ok_or(ZipError::FileNotFound)?;
        decoder::copy(&mut self.reader, &self.entries[index], open_destination)
    }
}

fn invalid(message: &'static str) -> ZipError {
    ZipError::InvalidArchive(message.into())
}

fn word(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().expect("validated record"))
}

fn dword(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("validated record"))
}

fn qword(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().expect("validated record"))
}

fn read_record<const N: usize>(reader: &mut impl Read) -> ZipResult<[u8; N]> {
    let mut record = [0; N];
    reader.read_exact(&mut record).map_err(|error| {
        if error.kind() == std::io::ErrorKind::UnexpectedEof {
            invalid("Truncated ZIP record")
        } else {
            error.into()
        }
    })?;
    Ok(record)
}

#[cfg(test)]
pub(crate) mod fixtures;
#[cfg(test)]
mod tests;
