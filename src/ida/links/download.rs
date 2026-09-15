//! Bounded KE transfers with address pinning and content verification.

use std::io::Write;
use std::path::Path;

use futures_util::StreamExt;
use sha2::{Digest, Sha256};

use crate::error::{Error, Result};

use super::transport::Transport;
use crate::util::http_body::Decoder;

const METADATA_MAX_BYTES: u64 = 16 * 1024 * 1024;

pub(super) enum Payload<'a> {
    Content {
        sha256: &'a str,
        limit: u64,
    },
    Metadata,
}

impl Payload<'_> {
    fn limit(&self) -> u64 {
        match self {
            Self::Content {
                limit,
                ..
            } => *limit,
            Self::Metadata => METADATA_MAX_BYTES,
        }
    }

    fn validate(&self, temporary: &tempfile::NamedTempFile, hash: Sha256) -> Result<()> {
        match self {
            Self::Content {
                sha256,
                ..
            } => {
                if !format!("{:x}", hash.finalize()).eq_ignore_ascii_case(sha256) {
                    return Err(Error::Other("KE content SHA-256 mismatch".into()));
                }
            }
            Self::Metadata => {
                super::metadata::validate(&std::fs::read(temporary.path())?)?;
            }
        }
        Ok(())
    }
}

pub(super) async fn download(
    url: &url::Url,
    destination: &Path,
    payload: Payload<'_>,
    client: &Transport,
) -> Result<()> {
    let response = client.response(url).await?;
    let limit = payload.limit();
    // Content-Length describes the encoded body. Upstream limits and hashes
    // decoded bytes, so a wire-length precheck would reject valid downloads.
    let mut decoder = Decoder::new(response.headers(), limit);
    let directory = destination
        .parent()
        .ok_or_else(|| Error::Other("KE download destination has no parent directory".into()))?;
    std::fs::create_dir_all(directory)?;
    let mut temporary = tempfile::NamedTempFile::new_in(directory)?;
    let mut stream = response.bytes_stream();
    let mut hash = Sha256::new();
    while let Some(chunk) = stream.next().await {
        let raw = chunk?;
        let chunk = decoder.decode(&raw)?;
        write_chunk(directory, &mut temporary, &mut hash, &chunk)?;
    }
    let tail = decoder.finish()?;
    write_chunk(directory, &mut temporary, &mut hash, &tail)?;
    payload.validate(&temporary, hash)?;
    temporary.persist(destination).map_err(|e| e.error)?;
    Ok(())
}

fn write_chunk(
    directory: &Path,
    temporary: &mut tempfile::NamedTempFile,
    hash: &mut Sha256,
    chunk: &[u8],
) -> Result<()> {
    if chunk.is_empty() {
        return Ok(());
    }
    if fs2::available_space(directory)? < 512 * 1024 * 1024 + chunk.len() as u64 {
        return Err(Error::Other("KE download requires 512 MiB free disk reserve".into()));
    }
    hash.update(chunk);
    temporary.write_all(chunk)?;
    Ok(())
}
