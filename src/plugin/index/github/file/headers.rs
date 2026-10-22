//! Response-header failures occur after stat and before host validation or open.

use crate::error::Result;

use super::Request;

mod timestamp;

pub(super) fn validate(request: &Request, modified: f64) -> Result<()> {
    timestamp::datetime(modified)?;
    request.validate_mime_selector()
}

#[cfg(test)]
mod tests;
