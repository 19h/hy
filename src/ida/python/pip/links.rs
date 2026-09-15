//! Normalize local pip find-links paths while preserving URL arguments.

use crate::error::Result;
use crate::util::python_path;

pub(crate) fn normalize_find_links(links: Vec<String>) -> Result<Vec<String>> {
    links.iter().map(|link| python_path::expand_user_unless_url(link)).collect()
}
