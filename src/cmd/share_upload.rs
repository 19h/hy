//! Shared-file upload orchestration and visibility-derived permissions.

use super::share::{AccessControl, PutArgs, expanded_path};
use crate::api::{ApiClient, UploadOptions};
use crate::auth::AuthService;
use crate::error::{Error, Result};

pub(super) async fn run(args: PutArgs) -> Result<()> {
    let path = crate::util::realpath::resolve(&expanded_path(&args.path)?)?;
    if !path.exists() {
        return Err(abort(&format!("Error: File not found: {}", path.display())));
    }
    if !path.is_file() {
        return Err(abort(&format!("Error: Path is not a file: {}", path.display())));
    }
    let email = AuthService::global().get_user_email()?.ok_or(Error::NotLoggedIn)?;
    let domain = email.split_once('@').map(|(_, domain)| domain.to_lowercase()).unwrap_or_default();
    let acl = match args.acl {
        Some(acl) => acl,
        None => {
            let choices = vec![
                "[private] Just for me".into(),
                format!("[domain] Anyone from my domain (@{domain})"),
                "[authenticated] Anyone authenticated with the link".into(),
            ];
            let selected = super::share_prompt::select("Pick a visibility 🔎", &choices, 2)?;
            [AccessControl::Private, AccessControl::Domain, AccessControl::Authenticated][selected]
        }
    };
    if args.force && args.code.as_ref().is_some_and(|code| !code.is_empty()) {
        return Err(abort("Error: --force and --code cannot be used together"));
    }
    let (segments, emails) = match acl {
        AccessControl::Private => (vec!["authenticated".into()], Some(vec![email])),
        AccessControl::Domain => (vec!["authenticated".into(), format!("@{domain}")], None),
        AccessControl::Authenticated => (vec!["authenticated".into()], None),
    };
    let options = UploadOptions {
        force: args.force,
        code: args.code,
        allowed_segments: Some(segments),
        allowed_emails: emails,
        metadata: Some(serde_json::Map::from_iter([("acl_type".into(), serde_json::json!(acl))])),
        ..Default::default()
    };
    let uploaded = ApiClient::new()?.upload_asset("shared", &path, options).await?;
    let env = crate::config::Env::global();
    println!("✓ File uploaded successfully!");
    println!("Share Code: {}", uploaded.code);
    println!("Share URL: {}/share/{}", env.portal_url, uploaded.code);
    println!("Download URL: {}/api/assets/s/{}", env.api_url, uploaded.code);
    Ok(())
}

fn abort(message: &str) -> Error {
    println!("{message}");
    eprintln!("Aborted!");
    Error::ChildExit(1)
}
