//! License query and download behavior against isolated customer and asset APIs.

mod support;

use serde_json::{Value, json};
use std::fs;
use support::{
    auth::*,
    http::{Response, Server},
    *,
};

fn license(id: &str, key: &str, plan: &str, product: &str, date: Option<&str>) -> Value {
    json!({"pubhash": id, "license_key": key, "product_catalog": plan, "product_code": product,
        "status": "active", "end_date": date, "asset_types": ["hexlic"], "license_type": "named"})
}

fn server(licenses: Vec<Value>) -> Server {
    Server::start(move |request, base| match request.path.as_str() {
        "/api/customers" => Response::json(json!([{"id": 17, "email": EMAIL}])),
        "/api/licenses/17?page=1&limit=100" => {
            Response::json(json!({"total": licenses.len(), "items": licenses}))
        }
        path if path.starts_with("/api/licenses/17/download/hexlic/") => Response::json(json!(
            format!("{base}/signed/{}.hexlic", path.rsplit('/').next().unwrap())
        )),
        path if path.starts_with("/signed/") => Response {
            status: 200,
            content_type: "application/octet-stream",
            body: b"license fixture".to_vec(),
        },
        _ => Response::missing(),
    })
}

#[test]
fn exact_license_id_plan_and_product_filters_use_license_keys_for_download() {
    for (options, expected) in [
        (vec!["--all"], vec!["key-one", "key-two", "key-three"]),
        (vec!["--all", "--id", "public-one"], vec!["key-one"]),
        (vec!["--all", "--id", "public"], vec![]),
        (vec!["--all", "--plan", "legacy"], vec!["key-two"]),
        (vec!["--all", "--type", "IDAHOME"], vec!["key-three"]),
        (vec!["--all", "--plan", "subscription", "--type", "IDAPRO"], vec!["key-one"]),
        (vec!["--all", "--type", "idapro"], vec![]),
    ] {
        let sandbox = Sandbox::new();
        write_config(&sandbox, &stored("key", "fixture-api-key"));
        let mut expired = license("public-expired", "key-expired", "subscription", "IDAPRO", None);
        expired["status"] = json!("expired");
        let server = server(vec![
            license("public-one", "key-one", "subscription", "IDAPRO", None),
            license("public-two", "key-two", "legacy", "IDAPRO", None),
            license("public-three", "key-three", "subscription", "IDAHOME", None),
            expired,
        ]);
        let mut args = vec!["license", "get", "--output-dir", "~/downloads"];
        args.extend(options);
        assert_success(&command(&sandbox, &server, &args).output().unwrap());
        let requests = server.requests();
        let keys: Vec<_> = requests
            .iter()
            .filter(|request| request.path.contains("/download/"))
            .map(|request| request.path.rsplit('/').next().unwrap())
            .collect();
        assert_eq!(keys, expected);
        for request in &requests {
            if request.path.starts_with("/signed/") {
                assert!(!request.headers.contains("fixture-api-key"));
            } else {
                assert!(request.headers.contains("fixture-api-key"));
            }
        }
        for key in expected {
            assert_eq!(
                fs::read(sandbox.path().join(format!("downloads/{key}.hexlic"))).unwrap(),
                b"license fixture"
            );
        }
    }
}

#[test]
fn download_order_matches_null_first_then_descending_lexical_end_dates() {
    let sandbox = Sandbox::new();
    write_config(&sandbox, &stored("key", "fixture-api-key"));
    let server = server(vec![
        license("a", "old", "legacy", "IDAPRO", Some("2020-01-01")),
        license("b", "null-first", "legacy", "IDAPRO", None),
        license("c", "new", "legacy", "IDAPRO", Some("2040-01-01")),
        license("d", "null-second", "legacy", "IDAPRO", None),
    ]);
    assert_success(
        &command(
            &sandbox,
            &server,
            &["license", "get", "--all", "--output-dir", sandbox.path().to_str().unwrap()],
        )
        .output()
        .unwrap(),
    );
    let keys: Vec<_> = server
        .requests()
        .into_iter()
        .filter(|request| request.path.contains("/download/"))
        .map(|request| request.path.rsplit('/').next().unwrap().to_owned())
        .collect();
    assert_eq!(keys, ["null-first", "null-second", "new", "old"]);
}

#[test]
fn per_asset_failures_and_missing_keys_do_not_abort_later_downloads() {
    let sandbox = Sandbox::new();
    write_config(&sandbox, &stored("key", "fixture-api-key"));
    let server = Server::start(|request, base| match request.path.as_str() {
        "/api/customers" => Response::json(json!([{"id": 17}])),
        "/api/licenses/17?page=1&limit=100" => {
            let mut missing = license("missing", "", "legacy", "IDAPRO", None);
            missing["license_key"] = Value::Null;
            let mut present = license("present", "actual-key", "legacy", "IDAPRO", None);
            present["asset_types"] =
                json!(["http-error", "null", "empty", "bad-transfer", "valid"]);
            Response::json(json!({"total": 2, "items": [missing, present]}))
        }
        "/api/licenses/17/download/null/actual-key" => Response::json(Value::Null),
        "/api/licenses/17/download/empty/actual-key" => Response::json(json!("")),
        "/api/licenses/17/download/bad-transfer/actual-key" => {
            Response::json(json!(format!("{base}/missing.hexlic")))
        }
        "/api/licenses/17/download/valid/actual-key" => {
            Response::json(json!(format!("{base}/valid.hexlic")))
        }
        "/valid.hexlic" => Response {
            status: 200,
            content_type: "application/octet-stream",
            body: b"valid".to_vec(),
        },
        _ => Response::missing(),
    });
    let output = command(
        &sandbox,
        &server,
        &["license", "get", "--all", "--output-dir", sandbox.path().to_str().unwrap()],
    )
    .output()
    .unwrap();
    assert_success(&output);
    assert_eq!(fs::read(sandbox.path().join("valid.hexlic")).unwrap(), b"valid");
    let text = String::from_utf8_lossy(&output.stderr);
    assert!(text.contains("License has no key"));
    assert!(text.contains("Error downloading license"));
    assert!(server.requests().iter().all(|request| !request.path.contains("/hexlic/missing")));
}

#[test]
fn absent_customer_ids_stop_before_license_requests_and_empty_accounts_fail() {
    for customers in [json!([]), json!([{}]), json!([{"id": 0}])] {
        let sandbox = Sandbox::new();
        write_config(&sandbox, &stored("key", "fixture-api-key"));
        let empty = customers.as_array().unwrap().is_empty();
        let server = Server::start(move |_, _| Response::json(customers.clone()));
        for operation in ["list", "get"] {
            let output = command(&sandbox, &server, &["license", operation]).output().unwrap();
            assert_eq!(output.status.success(), !empty);
        }
        assert!(server.requests().iter().all(|request| request.path == "/api/customers"));
    }
}

#[cfg(unix)]
#[test]
fn interactive_license_selection_starts_empty_and_groups_legacy_first() {
    use support::terminal::Terminal;
    for select in [false, true] {
        let sandbox = Sandbox::new();
        write_config(&sandbox, &stored("key", "fixture-api-key"));
        let server = server(vec![
            license("subscription", "subscription-key", "subscription", "IDAPRO", None),
            license("legacy", "legacy-key", "legacy", "IDAPRO", None),
        ]);
        let mut terminal = Terminal::start(command(
            &sandbox,
            &server,
            &["license", "get", "--output-dir", sandbox.path().to_str().unwrap()],
        ));
        terminal.wait_for("Select licenses");
        terminal.send(if select {
            " \r"
        } else {
            "\r"
        });
        let (status, output) = terminal.finish();
        assert!(status.success(), "{output}");
        let requests = server.requests();
        assert_eq!(
            requests.len(),
            if select {
                4
            } else {
                2
            }
        );
        if select {
            assert_eq!(requests[2].path, "/api/licenses/17/download/hexlic/legacy-key");
        }
    }
}
