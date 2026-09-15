//! Preserve the source's boundary between skipped outer members and terminal errors.

use std::io::{ErrorKind, Read, Seek};

use zip::result::ZipError;

use crate::error::Result;
use crate::util::python_zip::Archive;

pub(super) fn read<R: Read + Seek>(
    archive: &mut Archive<R>,
    name: &str,
) -> Result<Option<Vec<u8>>> {
    match archive.read(name) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if bad_member(&error) => {
            tracing::debug!(%name, %error, "skipping unreadable bundle member");
            Ok(None)
        }
        Err(error) => Err(error.into()),
    }
}

fn bad_member(error: &ZipError) -> bool {
    match error {
        ZipError::InvalidArchive(_) | ZipError::FileNotFound => true,
        ZipError::Io(error) => bad_checksum(error),
        _ => false,
    }
}

fn bad_checksum(error: &std::io::Error) -> bool {
    // zip 8.6.0 exposes CRC failure only as this io::Error pair. Other decoding
    // errors also use InvalidData, so the kind alone would suppress source failures.
    error.kind() == ErrorKind::InvalidData && error.to_string() == "Invalid checksum"
}
