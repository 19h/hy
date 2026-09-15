//! Decode complete HTTP bodies before the caller applies its response policy.

use crate::error::Result;
use crate::util::http_body::Decoder;

pub(super) async fn decode(mut response: reqwest::Response) -> Result<Vec<u8>> {
    let mut decoder = Decoder::new(response.headers(), u64::MAX);
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        body.extend_from_slice(&decoder.decode(&chunk)?);
    }
    body.extend(decoder.finish()?);
    Ok(body)
}
