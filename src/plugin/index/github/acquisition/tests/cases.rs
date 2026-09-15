use serde_json::{Value, json};

fn commit(name: &str, url: &str, date: &str) -> Value {
    json!({"oid":name, "zipballUrl":url, "committedDate":date})
}

fn release(date: &str, tag: &str, source: Value, assets: Vec<Value>) -> Value {
    json!({"name":tag, "tagName":tag, "createdAt":date, "publishedAt":date,
        "isPrerelease":false, "isDraft":false, "url":"https://github.com/o/r/release",
        "tag":{"target":source}, "releaseAssets":{"nodes":assets}})
}

fn repository(name: &str, releases: Vec<Value>, tags: Vec<Value>) -> Value {
    json!({"name":name, "metadata":{
        "defaultBranchRef":{"target":commit("default", "https://example.test/default", "2026-01-01")},
        "releases":{"nodes":releases}, "refs":{"nodes":tags},
    }})
}

pub(super) fn all() -> Vec<Value> {
    let mut cases = Vec::new();
    for date in ["", "2025-08-31", "2025-09-01", "2025-09-01T00:00:00Z", "2026-01-01"] {
        for content_type in
            ["application/zip", "application/x-zip-compressed", "raw", "RAW", "text/plain", ""]
        {
            for name in ["a.zip", "B.ZIP", "c.zip?", "d", "e.ZİP", "f.zip "] {
                for size in [0, 104_857_600, 104_857_601] {
                    let asset = json!({"name":name, "contentType":content_type, "size":size, "downloadUrl":"https://example.test/asset"});
                    let release = release(
                        date,
                        "v1",
                        commit("one", "https://example.test/source", date),
                        vec![asset],
                    );
                    cases.push(json!([repository("o/r", vec![release], vec![])]));
                }
            }
        }
    }
    for duplicate_url in [false, true] {
        for duplicate_commit in [false, true] {
            for reverse in [false, true] {
                let mut releases = Vec::new();
                let mut tags = Vec::new();
                for index in 0..3 {
                    let url = format!(
                        "https://example.test/source{}",
                        if duplicate_url {
                            0
                        } else {
                            index
                        }
                    );
                    let oid = format!(
                        "commit{}",
                        if duplicate_commit {
                            0
                        } else {
                            index
                        }
                    );
                    let commit = commit(&oid, &url, "2026-01-01");
                    let asset = json!({"name":format!("{index}.zip"), "contentType":"raw", "size":1,
                        "downloadUrl":"https://example.test/shared"});
                    releases.push(release(
                        "2026-01-01",
                        &format!("v{index}"),
                        commit.clone(),
                        vec![asset.clone(), asset],
                    ));
                    tags.push(json!({"name":format!("v{index}"), "target":commit}));
                }
                tags.push(json!({"name":"vtag-only", "target":commit("tag", "https://example.test/tag", "2026-01-01")}));
                tags.push(tags.last().unwrap().clone());
                tags.push(json!({"name":"Vuppercase", "target":commit("upper", "https://example.test/upper", "2026-01-01")}));
                tags.push(json!({"name":"vold", "target":commit("old", "https://example.test/old", "2025-08-31")}));
                if reverse {
                    releases.reverse();
                    tags.reverse();
                }
                cases.push(json!([repository("o/r", releases, tags)]));
            }
        }
    }
    let repositories: Vec<_> = ["a-b/repo", "a/repo", "a/repo-other"].into_iter().map(|name| {
        repository(name, vec![release("2026-01-01", "v1", commit("same", "https://example.test/same-source", "2026-01-01"),
            vec![json!({"name":"same.zip", "contentType":"raw", "size":1, "downloadUrl":"https://example.test/same-asset"})])], vec![])
    }).collect();
    cases.push(json!(repositories));
    cases.push(json!([]));
    cases
}
