//! Identity lookup, fallback, and persistence boundaries for status commands.

mod support;

use std::fs;

use serde_json::json;
use support::auth::{EMAIL, command, stored, write_config};
use support::http::{Response, Server};
use support::{Sandbox, assert_success};

const ENVIRONMENT_KEY: &str = "status-environment-secret";

fn output_text(output: &std::process::Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn environment_identity_uses_the_api_and_falls_back_without_changing_credentials() {
    let mut cases = vec![
        (Response::json(json!({"email": EMAIL})), EMAIL),
        (Response::json(json!({"email": ""})), ""),
        (Response::json(json!({})), "api-key-user"),
        (Response::json(json!({"email": null})), "api-key-user"),
        (Response::json(json!({"email": 123})), "api-key-user"),
        (Response::json(json!([])), "api-key-user"),
        (
            Response {
                status: 200,
                content_type: "application/json",
                body: b"invalid JSON".to_vec(),
            },
            "api-key-user",
        ),
    ];
    for status in [401, 403, 404, 429, 500] {
        cases.push((
            Response {
                status,
                ..Response::json(json!({"message": ENVIRONMENT_KEY}))
            },
            "api-key-user",
        ));
    }
    for (response, email) in cases {
        let sandbox = Sandbox::new();
        write_config(&sandbox, &stored("interactive", "must-not-validate-stored-token"));
        let before = fs::read(sandbox.config_path()).unwrap();
        let server = Server::start(move |request, _| {
            assert_eq!(request.method, "GET");
            assert_eq!(request.path, "/api/whoami");
            let headers = request.headers.to_lowercase();
            assert!(headers.contains(&format!("x-api-key: {ENVIRONMENT_KEY}\r\n")));
            assert!(headers.contains("accept: application/json\r\n"));
            assert!(headers.contains("content-type: application/json\r\n"));
            assert!(!headers.contains("authorization:"));
            response.clone()
        });
        let output = command(&sandbox, &server, &["whoami"])
            .env("HCLI_API_KEY", ENVIRONMENT_KEY)
            .output()
            .unwrap();
        assert_success(&output);
        let text = output_text(&output);
        assert!(
            text.contains(&format!("logged in as {email} using an API key from HCLI_API_KEY")),
            "{text}"
        );
        assert!(!text.contains(ENVIRONMENT_KEY));
        assert_eq!(server.requests().len(), 1);
        assert_eq!(fs::read(sandbox.config_path()).unwrap(), before);
    }
}

#[test]
fn identity_redirects_do_not_forward_api_keys() {
    for body in [json!({"email": "redirect-body@example.test"}), json!({})] {
        let sandbox = Sandbox::new();
        let destination = Server::start(|_, _| panic!("identity lookup followed a redirect"));
        let location = format!("{}/other", destination.url);
        let email = body["email"].as_str().unwrap_or("api-key-user").to_owned();
        let server = Server::start_with_headers(move |_, _| {
            (
                Response {
                    status: 302,
                    ..Response::json(body.clone())
                },
                vec![("Location".into(), location.clone())],
            )
        });
        let output = command(&sandbox, &server, &["whoami"])
            .env("HCLI_API_KEY", ENVIRONMENT_KEY)
            .output()
            .unwrap();
        assert_success(&output);
        assert!(output_text(&output).contains(&format!("logged in as {email} using")));
        assert_eq!(server.requests().len(), 1);
        assert!(destination.requests().is_empty());
        assert!(!sandbox.config_path().exists());
    }
}

#[test]
fn managed_status_validates_interactive_tokens_without_touching_last_used() {
    for kind in ["key", "interactive"] {
        let sandbox = Sandbox::new();
        write_config(&sandbox, &stored(kind, "stored-status-token"));
        let before = fs::read(sandbox.config_path()).unwrap();
        let server = Server::start(|request, _| {
            assert_eq!(request.path, "/auth/v1/user");
            Response::json(json!({"email": EMAIL}))
        });
        let output = command(&sandbox, &server, &["whoami"]).output().unwrap();
        assert_success(&output);
        assert!(output_text(&output).contains(&format!("logged in as {EMAIL}")));
        assert_eq!(server.requests().len(), usize::from(kind == "interactive"));
        assert_eq!(fs::read(sandbox.config_path()).unwrap(), before);
    }
}

#[test]
fn setting_a_default_resolves_environment_identity_after_committing_the_selection() {
    let sandbox = Sandbox::new();
    let mut config = stored("key", "old-managed-key");
    let mut second = config["hcli.credentials"]["credentials"]["account"].clone();
    second["name"] = "second".into();
    config["hcli.credentials"]["credentials"]["second"] = second;
    write_config(&sandbox, &config);
    let path = sandbox.config_path();
    let server = Server::start(move |request, _| {
        assert_eq!(request.path, "/api/whoami");
        let saved: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(saved["hcli.credentials"]["default"], "second");
        Response::json(json!({"email": EMAIL}))
    });
    let output = command(&sandbox, &server, &["auth", "default", "second"])
        .env("HCLI_API_KEY", ENVIRONMENT_KEY)
        .output()
        .unwrap();
    assert_success(&output);
    assert!(
        output_text(&output)
            .contains(&format!("logged in as {EMAIL} using an API key from HCLI_API_KEY"))
    );
    assert_eq!(server.requests().len(), 1);
    let saved: serde_json::Value =
        serde_json::from_slice(&fs::read(sandbox.config_path()).unwrap()).unwrap();
    assert_eq!(saved["hcli.credentials"]["credentials"], config["hcli.credentials"]["credentials"]);
}

#[test]
fn async_key_installation_status_retains_the_environment_placeholder() {
    let sandbox = Sandbox::new();
    let server = Server::start(|request, _| {
        assert_eq!(request.path, "/api/whoami");
        assert!(request.headers.to_lowercase().contains("x-api-key: installed-key\r\n"));
        Response::json(json!({"email": EMAIL}))
    });
    let output = command(
        &sandbox,
        &server,
        &[
            "auth",
            "key",
            "install",
            "--key",
            "installed-key",
            "--key-name",
            "installed",
            "--set-default",
        ],
    )
    .env("HCLI_API_KEY", ENVIRONMENT_KEY)
    .output()
    .unwrap();
    assert_success(&output);
    assert!(
        output_text(&output)
            .contains("logged in as api-key-user using an API key from HCLI_API_KEY")
    );
    assert_eq!(server.requests().len(), 1);
}

#[test]
fn an_invalid_environment_key_header_falls_back_before_network_access() {
    let sandbox = Sandbox::new();
    let server = Server::start(|_, _| panic!("invalid API key reached the network"));
    let output = command(&sandbox, &server, &["whoami"])
        .env("HCLI_API_KEY", "invalid\nheader")
        .output()
        .unwrap();
    assert_success(&output);
    assert!(output_text(&output).contains("logged in as api-key-user"));
    assert!(!output_text(&output).contains("invalid\nheader"));
    assert!(server.requests().is_empty());
}
