//! Ticket coercion and side effects are checked independently of ACL prompts.

use super::*;

#[test]
fn upload_ticket_validation_preserves_transfer_and_confirmation_order() {
    let mut cases = Vec::new();
    let base = json!({"key": "///nested/file", "code": "fixture-code", "version": 7});
    for field in ["key", "code", "version"] {
        for value in [
            None,
            Some(json!(null)),
            Some(json!(true)),
            Some(json!(false)),
            Some(json!(42)),
            Some(json!("")),
            Some(json!("1.0")),
            Some(json!("184467440737095516160")),
            Some(json!([])),
            Some(json!({})),
        ] {
            for transfer in [false, true] {
                let mut ticket = base.clone();
                if let Some(value) = &value {
                    ticket[field] = value.clone();
                } else {
                    ticket.as_object_mut().unwrap().remove(field);
                }
                if transfer {
                    ticket["url"] = json!("https://fixture.test/signed-put");
                }
                cases.push(ticket);
            }
        }
    }
    for url in [
        json!(null),
        json!(false),
        json!(true),
        json!(0),
        json!(0.0),
        json!(42),
        json!(""),
        json!([]),
        json!({}),
        json!([1]),
        json!({"url":"x"}),
    ] {
        let mut ticket = base.clone();
        ticket["url"] = url;
        cases.push(ticket);
    }
    cases.extend([json!(null), json!([]), json!("ticket"), json!(42)]);
    assert_eq!(cases.len(), 75);

    let sandbox = Sandbox::new();
    write_config(&sandbox, &stored("key", "fixture-key"));
    let file = sandbox.path().join("payload.json");
    fs::write(&file, b"fixture").unwrap();
    let mut observed = Vec::new();
    for ticket in &cases {
        let ticket = ticket.clone();
        let server = Server::start(move |request, base| {
            if request.path == "/api/assets/shared" {
                let mut ticket = ticket.clone();
                if ticket.get("url").and_then(Value::as_str)
                    == Some("https://fixture.test/signed-put")
                {
                    ticket["url"] = json!(format!("{base}/signed-put"));
                }
                return Response::json(ticket);
            }
            Response::json(json!({}))
        });
        let output = command(
            &sandbox,
            &server,
            &["share", "put", file.to_str().unwrap(), "--acl", "private"],
        )
        .output()
        .unwrap();
        let events: Vec<_> = server
            .requests()
            .into_iter()
            .map(|request| json!([request.method, request.path]))
            .collect();
        observed.push(json!({"success": output.status.success(), "events": events}));
    }
    reference::compare(&file, &cases, &observed);
    assert_eq!(
        format!("{:x}", Sha256::digest(serde_json::to_vec(&observed).unwrap())),
        "2be63a8dede6d49a8f97691a012493d1b7defd793b17fc8c678f08a73dec8f3d"
    );
    // Invalid keys fail after PUT; invalid codes/versions fail after confirmation.
    assert_eq!(observed[1]["events"].as_array().unwrap().len(), 2);
    assert_eq!(observed[21]["events"].as_array().unwrap().len(), 3);
    assert_eq!(observed[41]["events"].as_array().unwrap().len(), 3);
    assert_eq!(observed[44]["success"], true); // Boolean version, no transfer.
}
