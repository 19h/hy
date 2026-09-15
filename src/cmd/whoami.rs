//! `hcli whoami` command.

use crate::error::Result;

pub async fn run() -> Result<()> {
    crate::auth::show_resolved_login_info().await
}
