use serde_json::{Value, json};

fn repository() -> Value {
    json!({
        "defaultBranchRef": {
            "target": {
                "oid": "fixture",
                "zipballUrl": "https://example.test/archive",
                "committedDate": "2026-01-01",
            },
        },
        "releases": {"nodes": []},
        "refs": {"nodes": []},
    })
}

pub(super) fn all() -> Vec<Value> {
    let mut cases = Vec::new();
    for name in ["owner/space name", "o/q\"uote", "o/one\\two", "o/r#hash"] {
        cases.push(json!({
            "repositories": [name],
            "response": {"data": {"repo0": repository()}},
        }));
    }
    for count in [0, 1, 2, 9, 10, 11, 20, 21] {
        let names: Vec<_> =
            (0..count).map(|index| format!("owner-{index}/repo.name_{index}")).collect();
        let data: serde_json::Map<_, _> =
            (0..count).map(|index| (format!("repo{index}"), repository())).collect();
        cases.push(json!({"repositories":names, "response":{"data":data}}));
    }
    let names = ["a/r", "a-b/r", "z/r"];
    let baseline = json!({
        "repo0": repository(),
        "repo1": repository(),
        "repo2": repository(),
        "repo99": {"ignored": "invalid"},
    });
    let mut values = vec![
        Value::Null,
        json!({}),
        json!(false),
        json!(0),
        json!(0.0),
        json!(""),
        json!([]),
        json!(true),
        json!(1),
        json!("invalid"),
        json!([1]),
        repository(),
    ];
    for branch in [
        Value::Null,
        json!({}),
        json!([]),
        json!(""),
        json!(false),
        json!(0),
        json!(true),
        json!({"target":{}}),
    ] {
        values.push(json!({"defaultBranchRef":branch}));
    }
    for alias in ["repo0", "repo1", "repo2"] {
        let mut absent = baseline.clone();
        absent.as_object_mut().unwrap().remove(alias);
        cases.push(json!({"repositories":names, "response":{"data":absent}}));
        for value in &values {
            let mut data = baseline.clone();
            data[alias] = value.clone();
            cases.push(json!({"repositories":names, "response":{"data":data}}));
        }
    }
    for errors in [
        json!([]),
        json!({}),
        json!(""),
        Value::Null,
        json!(false),
        json!(0),
        json!([null]),
        json!([{"type":"NOT_FOUND"}]),
        json!([{"type":"NOT_FOUND", "message":"missing"}, {"type":"NOT_FOUND", "message":null}]),
        json!([{"type":"FORBIDDEN", "message":"denied"}]),
        json!([{"type":"NOT_FOUND"}, {"message":"untyped", "extra":[1,true]}]),
        json!([{"type":"FORBIDDEN"}, {"type":"RATE_LIMITED", "message":"slow down"}]),
    ] {
        for data in
            [baseline.clone(), Value::Null, json!({"repo0":{"defaultBranchRef":{"target":{}}}})]
        {
            cases.push(json!({"repositories":names, "response":{"data":data, "errors":errors}}));
        }
    }
    for response in [
        json!({}),
        Value::Null,
        json!([]),
        json!({"data":[]}),
        json!({"data":"invalid"}),
        json!({"data":{}}),
    ] {
        cases.push(json!({"repositories":names, "response":response}));
    }
    cases
}
