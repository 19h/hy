//! `hcli whoami` command.

use crate::auth::AuthService;
use crate::error::Result;

pub async fn run() -> Result<()> {
    let mut auth = AuthService::global();
    auth.init(None);
    auth.show_login_info();
    Ok(())
}
