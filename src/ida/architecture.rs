//! Detect IDA's architecture from executable headers.

use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use crate::error::{Error, Result};

/// Plugin and wheel compatibility follows the selected IDA executable, not hy's CPU.
pub fn current_ida_platform() -> Result<String> {
    if let Some(platform) = &crate::config::Env::global().current_ida_platform {
        return Ok(platform.clone());
    }
    let installation = super::resolve_install_dir()?;
    let executable = super::ida_binary_path(&installation.path).ok_or(Error::IdaNotFound)?;
    let architecture = binary_architecture(&executable)?.ok_or_else(|| {
        Error::Other(format!("unrecognized IDA executable architecture: {}", executable.display()))
    })?;
    let os = match std::env::consts::OS {
        os @ ("windows" | "linux" | "macos") => os,
        os => return Err(Error::Other(format!("unsupported IDA operating system: {os}"))),
    };
    Ok(format!("{os}-{architecture}"))
}

pub fn binary_architecture(path: &Path) -> Result<Option<&'static str>> {
    let mut file = std::fs::File::open(path)?;
    let mut header = [0_u8; 64];
    if file.read(&mut header)? < 20 {
        return Ok(None);
    }
    let architecture = if header.starts_with(b"\x7fELF") {
        let machine = match header[5] {
            1 => u16::from_le_bytes([header[18], header[19]]),
            2 => u16::from_be_bytes([header[18], header[19]]),
            _ => return Ok(None),
        };
        match machine {
            0x3e => Some("x86_64"),
            0xb7 => Some("aarch64"),
            _ => None,
        }
    } else if header.starts_with(b"MZ") {
        let offset = u32::from_le_bytes(header[60..64].try_into().expect("fixed header field"));
        file.seek(SeekFrom::Start(u64::from(offset)))?;
        let mut pe = [0_u8; 6];
        if file.read_exact(&mut pe).is_err() || !pe.starts_with(b"PE\0\0") {
            return Ok(None);
        }
        match u16::from_le_bytes([pe[4], pe[5]]) {
            0x8664 => Some("x86_64"),
            0xaa64 => Some("aarch64"),
            _ => None,
        }
    } else {
        let cpu = match &header[..4] {
            b"\xcf\xfa\xed\xfe" => {
                u32::from_le_bytes(header[4..8].try_into().expect("fixed header field"))
            }
            b"\xfe\xed\xfa\xcf" => {
                u32::from_be_bytes(header[4..8].try_into().expect("fixed header field"))
            }
            _ => return Ok(None),
        };
        match cpu {
            0x0100_0007 => Some("x86_64"),
            0x0100_000c => Some("aarch64"),
            _ => None,
        }
    };
    Ok(architecture)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_architecture_from_elf_pe_and_macho_headers() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("binary");
        for (magic, machine, offset, expected) in [
            (&b"\x7fELF"[..], &b"\xb7\0"[..], 18, "aarch64"),
            (&b"\xcf\xfa\xed\xfe"[..], &b"\x07\0\0\x01"[..], 4, "x86_64"),
            (&b"\xfe\xed\xfa\xcf"[..], &b"\x01\0\0\x0c"[..], 4, "aarch64"),
        ] {
            let mut header = vec![0; 64];
            header[..4].copy_from_slice(magic);
            if magic == b"\x7fELF" {
                header[5] = 1;
            }
            header[offset..offset + machine.len()].copy_from_slice(machine);
            std::fs::write(&path, header).unwrap();
            assert_eq!(binary_architecture(&path).unwrap(), Some(expected));
        }
        let mut pe = vec![0; 70];
        pe[..2].copy_from_slice(b"MZ");
        pe[60..64].copy_from_slice(&64_u32.to_le_bytes());
        pe[64..70].copy_from_slice(b"PE\0\0\x64\x86");
        std::fs::write(&path, pe).unwrap();
        assert_eq!(binary_architecture(&path).unwrap(), Some("x86_64"));
        std::fs::write(&path, b"truncated").unwrap();
        assert_eq!(binary_architecture(&path).unwrap(), None);
    }
}
