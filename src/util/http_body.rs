//! Decode response bodies using HTTPX's default gzip and deflate rules.

use std::borrow::Cow;

use flate2::{Decompress, FlushDecompress, Status};
use reqwest::header::{CONTENT_ENCODING, HeaderMap};

use crate::error::{Error, Result};

use crate::util::http_headers::{TextDecoder, trim};

const BUFFER_BYTES: usize = 64 * 1024;

pub(crate) struct Decoder {
    layers: Vec<Inflate>,
    remaining: u64,
}

impl Decoder {
    pub(crate) fn new(headers: &HeaderMap, limit: u64) -> Self {
        let mut layers = Vec::new();
        let text_decoder = TextDecoder::new(headers);
        for header in headers.get_all(CONTENT_ENCODING) {
            let text = text_decoder.decode(header);
            for token in text.split(',') {
                match trim(token).to_ascii_lowercase().as_str() {
                    "gzip" => layers.push(Inflate::gzip()),
                    "deflate" => layers.push(Inflate::deflate()),
                    _ => {}
                }
            }
        }
        layers.reverse();
        Self {
            layers,
            remaining: limit,
        }
    }

    pub(crate) fn decode<'a>(&mut self, bytes: &'a [u8]) -> Result<Cow<'a, [u8]>> {
        // HTTPX's raw byte chunker omits empty network chunks. Empty output
        // between decoder layers still reaches the next layer below.
        if bytes.is_empty() {
            return Ok(Cow::Borrowed(bytes));
        }
        let mut decoded = Cow::Borrowed(bytes);
        let count = self.layers.len();
        for (index, layer) in self.layers.iter_mut().enumerate() {
            let limit = if index + 1 == count {
                self.remaining
            } else {
                u64::MAX
            };
            decoded = Cow::Owned(layer.decode(&decoded, limit)?);
        }
        self.account(decoded.len())?;
        Ok(decoded)
    }

    pub(crate) fn finish(&mut self) -> Result<Vec<u8>> {
        let mut decoded = Vec::new();
        let count = self.layers.len();
        for (index, layer) in self.layers.iter_mut().enumerate() {
            let limit = if index + 1 == count {
                self.remaining
            } else {
                u64::MAX
            };
            decoded = layer.decode(&decoded, limit)?;
            let tail = layer.flush(limit - decoded.len() as u64)?;
            decoded.extend_from_slice(&tail);
        }
        self.account(decoded.len())?;
        Ok(decoded)
    }

    fn account(&mut self, count: usize) -> Result<()> {
        self.remaining = self.remaining.checked_sub(count as u64).ok_or_else(size_limit)?;
        Ok(())
    }
}

struct Inflate {
    state: Decompress,
    first_deflate_call: bool,
    finished: bool,
}

impl Inflate {
    fn gzip() -> Self {
        Self {
            state: Decompress::new_gzip(15),
            first_deflate_call: false,
            finished: false,
        }
    }

    fn deflate() -> Self {
        Self {
            state: Decompress::new(true),
            first_deflate_call: true,
            finished: false,
        }
    }

    fn decode(&mut self, bytes: &[u8], limit: u64) -> Result<Vec<u8>> {
        let fallback = std::mem::take(&mut self.first_deflate_call);
        match self.inflate(bytes, limit) {
            Err(InflateError::Stream(_)) if fallback => {
                // Retry the complete first input chunk, discarding its partial
                // output. Later input errors never switch the stream format.
                self.state = Decompress::new(false);
                self.finished = false;
                self.inflate(bytes, limit).map_err(Into::into)
            }
            result => result.map_err(Into::into),
        }
    }

    fn flush(&mut self, limit: u64) -> Result<Vec<u8>> {
        // zlib.decompressobj.flush() does not require a complete trailer.
        // Drain available output without adding an end-of-stream requirement.
        self.inflate(&[], limit).map_err(Into::into)
    }

    fn inflate(
        &mut self,
        mut bytes: &[u8],
        limit: u64,
    ) -> std::result::Result<Vec<u8>, InflateError> {
        let mut decoded = Vec::new();
        let mut buffer = [0; BUFFER_BYTES];
        while !self.finished {
            let input_before = self.state.total_in();
            let output_before = self.state.total_out();
            let status = self
                .state
                .decompress(bytes, &mut buffer, FlushDecompress::None)
                .map_err(InflateError::Stream)?;
            let consumed = (self.state.total_in() - input_before) as usize;
            let produced = (self.state.total_out() - output_before) as usize;
            if decoded.len() as u64 + produced as u64 > limit {
                return Err(InflateError::Limit);
            }
            decoded.extend_from_slice(&buffer[..produced]);
            bytes = &bytes[consumed..];
            if status == Status::StreamEnd {
                // HTTPX uses the first gzip/zlib member and ignores unused data.
                self.finished = true;
            }
            if consumed == 0 && produced == 0 {
                break;
            }
        }
        Ok(decoded)
    }
}

enum InflateError {
    Stream(flate2::DecompressError),
    Limit,
}

impl From<InflateError> for Error {
    fn from(error: InflateError) -> Self {
        match error {
            InflateError::Stream(error) => {
                Self::Other(format!("response decoding failed: {error}"))
            }
            InflateError::Limit => size_limit(),
        }
    }
}

fn size_limit() -> Error {
    Error::Other("response exceeds size limit".into())
}

#[cfg(test)]
mod tests;
