//! Preserve both pip output streams and upstream's ordered known-error recognition.

use std::path::Path;

use crate::util::strings::python_trim;

pub(super) fn message(executable: &Path, stdout: &[u8], stderr: &[u8], binary: &str) -> String {
    let stdout = String::from_utf8_lossy(stdout);
    let stderr = String::from_utf8_lossy(stderr);
    let text = [python_trim(&stdout), python_trim(&stderr)]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    let exe = executable.display();
    if text.contains("externally-managed-environment") {
        format!(
            "{exe} is an externally managed Python (PEP 668), so pip refuses to install into it. \
             Point IDA to a virtual environment instead of the system or Homebrew Python: \
             run '{binary} ida python create-environment' to create one and configure IDA to use it. \
             Run '{binary} ida python doctor' to inspect the current setup."
        )
    } else if text.contains("no such option: --dry-run") {
        format!(
            "pip does not support --dry-run (requires pip 22.2 or later). \
             Please upgrade pip: {exe} -m pip install --upgrade pip"
        )
    } else {
        text
    }
}
