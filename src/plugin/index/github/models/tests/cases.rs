use serde_json::{Value, json};

fn commit() -> Value {
    json!({"oid": "commit", "zipballUrl": "https://example.test/source", "committedDate": "2026-01-01"})
}

pub(super) fn graphql() -> Value {
    json!({
        "defaultBranchRef": {"target": commit()},
        "releases": {"nodes": [{
            "name": "release",
            "tagName": "v1",
            "createdAt": "2026-01-01",
            "publishedAt": "2026-01-02",
            "isPrerelease": false,
            "isDraft": false,
            "url": "https://example.test/release",
            "tag": {"target": commit()},
            "releaseAssets": {"nodes": [{
                "name": "plugin.zip",
                "contentType": "raw",
                "size": 1,
                "downloadUrl": "https://example.test/asset",
            }]},
        }]},
        "refs": {"nodes": [{"name": "v1", "target": commit()}]},
    })
}

pub(super) fn cached() -> Value {
    json!({
        "default_branch": {
            "commit_hash": "commit",
            "zipball_url": "https://example.test/source",
            "committed_date": "2026-01-01",
        },
        "releases": [{
            "name": "release",
            "tag_name": "v1",
            "commit_hash": "commit",
            "created_at": "2026-01-01",
            "published_at": "2026-01-02",
            "is_prerelease": false,
            "is_draft": false,
            "url": "https://example.test/release",
            "zipball_url": "https://example.test/source",
            "assets": [{
                "name": "plugin.zip",
                "content_type": "raw",
                "size": 1,
                "download_url": "https://example.test/asset",
            }],
        }],
        "tags": [{
            "tag_name": "v1",
            "commit_hash": "commit",
            "zipball_url": "https://example.test/source",
            "committed_date": "2026-01-01",
        }],
    })
}

fn mutations(cases: &mut Vec<Value>, kind: &str, baseline: &Value, paths: &[&str]) {
    let values = [
        Value::Null,
        json!(false),
        json!(true),
        json!(0),
        json!(1),
        json!(-1),
        json!(1.0),
        json!(1.5),
        json!(""),
        json!("text"),
        json!([]),
        json!({}),
        json!([null]),
        json!({"unexpected": true}),
    ];
    for path in paths {
        for replacement in &values {
            let mut value = baseline.clone();
            *value.pointer_mut(path).unwrap() = replacement.clone();
            cases.push(json!({"kind": kind, "value": value}));
        }
        let mut value = baseline.clone();
        let (parent, field) = path.rsplit_once('/').unwrap();
        value.pointer_mut(parent).unwrap().as_object_mut().unwrap().remove(field);
        cases.push(json!({"kind": kind, "value": value}));
    }
}

pub(super) fn all() -> Vec<Value> {
    let wire = graphql();
    let cache = cached();
    let mut cases =
        vec![json!({"kind": "graphql", "value": wire}), json!({"kind": "cache", "value": cache})];
    mutations(
        &mut cases,
        "graphql",
        &wire,
        &[
            "/defaultBranchRef",
            "/defaultBranchRef/target",
            "/defaultBranchRef/target/oid",
            "/defaultBranchRef/target/zipballUrl",
            "/defaultBranchRef/target/committedDate",
            "/releases",
            "/releases/nodes",
            "/releases/nodes/0/name",
            "/releases/nodes/0/tagName",
            "/releases/nodes/0/createdAt",
            "/releases/nodes/0/publishedAt",
            "/releases/nodes/0/isPrerelease",
            "/releases/nodes/0/isDraft",
            "/releases/nodes/0/url",
            "/releases/nodes/0/tag",
            "/releases/nodes/0/tag/target",
            "/releases/nodes/0/tag/target/oid",
            "/releases/nodes/0/tag/target/zipballUrl",
            "/releases/nodes/0/tag/target/committedDate",
            "/releases/nodes/0/releaseAssets",
            "/releases/nodes/0/releaseAssets/nodes",
            "/releases/nodes/0/releaseAssets/nodes/0/name",
            "/releases/nodes/0/releaseAssets/nodes/0/contentType",
            "/releases/nodes/0/releaseAssets/nodes/0/downloadUrl",
            "/releases/nodes/0/releaseAssets/nodes/0/size",
            "/refs",
            "/refs/nodes",
            "/refs/nodes/0/name",
            "/refs/nodes/0/target",
            "/refs/nodes/0/target/oid",
            "/refs/nodes/0/target/zipballUrl",
            "/refs/nodes/0/target/committedDate",
        ],
    );
    mutations(
        &mut cases,
        "cache",
        &cache,
        &[
            "/default_branch",
            "/default_branch/commit_hash",
            "/default_branch/zipball_url",
            "/default_branch/committed_date",
            "/releases",
            "/releases/0/name",
            "/releases/0/tag_name",
            "/releases/0/commit_hash",
            "/releases/0/created_at",
            "/releases/0/published_at",
            "/releases/0/is_prerelease",
            "/releases/0/is_draft",
            "/releases/0/url",
            "/releases/0/zipball_url",
            "/releases/0/assets",
            "/releases/0/assets/0/name",
            "/releases/0/assets/0/content_type",
            "/releases/0/assets/0/download_url",
            "/releases/0/assets/0/size",
            "/tags",
            "/tags/0/tag_name",
            "/tags/0/commit_hash",
            "/tags/0/zipball_url",
            "/tags/0/committed_date",
        ],
    );
    for (kind, baseline, asset_path, boolean_path) in [
        ("graphql", &wire, "/releases/nodes/0/releaseAssets/nodes/0", "/releases/nodes/0/isDraft"),
        ("cache", &cache, "/releases/0/assets/0", "/releases/0/is_draft"),
    ] {
        for size in [
            json!(-104_857_601_i64),
            json!(104_857_600),
            json!(104_857_601),
            serde_json::from_str("184467440737095516160").unwrap(),
            json!("184467440737095516160"),
            json!("-184467440737095516160"),
            json!("1.0"),
            json!("1_000"),
            json!("00-1"),
            json!("0__1"),
            json!("1e3"),
            json!("١"),
            json!("  +01.00  "),
            json!(1e20),
        ] {
            let mut value = baseline.clone();
            value.pointer_mut(asset_path).unwrap()["size"] = size;
            cases.push(json!({"kind": kind, "value": value}));
        }
        for flag in ["on", "OFF", "true", "False", "Y", "n", "1", "0", " yes ", ""] {
            let mut value = baseline.clone();
            *value.pointer_mut(boolean_path).unwrap() = json!(flag);
            cases.push(json!({"kind": kind, "value": value}));
        }
        for alias in ["contentType", "downloadUrl"] {
            for replacement in [Value::Null, json!("alias"), json!(1)] {
                let mut value = baseline.clone();
                let asset = value.pointer_mut(asset_path).unwrap();
                asset["content_type"] = json!("field");
                asset["download_url"] = json!("field");
                asset[alias] = replacement;
                cases.push(json!({"kind": kind, "value": value}));
            }
        }
    }
    for path in ["/releases/nodes/0/tag/target", "/refs/nodes/0/target"] {
        for nested in [
            json!({"target": commit()}),
            json!({"target": {"target": commit()}}),
            json!({"oid": "outer", "zipballUrl": "outer", "committedDate": "outer", "target": commit()}),
            json!({"oid": "outer", "zipballUrl": "outer", "committedDate": "outer", "target": null}),
        ] {
            let mut value = wire.clone();
            *value.pointer_mut(path).unwrap() = nested;
            cases.push(json!({"kind": "graphql", "value": value}));
        }
    }
    cases
}
