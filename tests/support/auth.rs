//! Synthetic credentials and local service configuration for authentication tests.

use super::{Sandbox, http::Server};
use serde_json::{Value, json};
use std::fs;

pub const EMAIL: &str = "account@example.test";

pub fn stored(kind: &str, token: &str) -> Value {
    json!({"hcli.credentials": {"default": "account", "credentials": {
        "account": {"name": "account", "type": kind, "token": token, "email": EMAIL,
            "created_at": "2026-09-15T10:00:00Z", "last_used": "2026-09-15T10:00:00Z"}
    }}})
}

pub fn write_config(sandbox: &Sandbox, value: &Value) {
    let path = sandbox.config_path();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
}

pub fn command(sandbox: &Sandbox, server: &Server, args: &[&str]) -> std::process::Command {
    let mut command = sandbox.command(args);
    command
        .env("HCLI_API_URL", &server.url)
        .env("HCLI_SUPABASE_URL", &server.url)
        .env("HCLI_SUPABASE_ANON_KEY", "fixture-anon");
    command
}
