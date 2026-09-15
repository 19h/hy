use super::{Case, Reply};

pub(super) fn all() -> Vec<Case> {
    let mut cases = Vec::new();
    for status in
        [200, 204, 300, 301, 302, 303, 304, 305, 306, 307, 308, 399, 401, 403, 404, 429, 500]
    {
        for location in [None, Some(""), Some("/next")] {
            for scheme in ["http", "https"] {
                let first = location.map_or_else(
                    || Reply::new(status),
                    |location| Reply::new(status).header("location", location.as_bytes()),
                );
                cases.push(Case {
                    base: format!("{scheme}://api.github.com"),
                    replies: vec![first, Reply::new(200)],
                });
            }
        }
    }
    for count in [0, 1, 19, 20, 21, 22] {
        let mut replies = vec![Reply::new(302).header("location", b"/again"); count];
        replies.push(Reply::new(200));
        cases.push(Case {
            base: "https://api.github.com".into(),
            replies,
        });
    }
    for status in [200, 401, 403, 404, 500] {
        cases.push(Case {
            base: "https://api.github.com".into(),
            replies: vec![
                Reply::new(302).header("location", b"http://mirror.test/final"),
                Reply::new(status),
            ],
        });
    }
    for base in ["http://user:pass@api.github.com", "https://user:pass@api.github.com"] {
        cases.push(Case {
            base: base.into(),
            replies: vec![
                Reply::new(301).header("location", b"https://api.github.com/secure"),
                Reply::new(302).header("location", b"http://mirror.test/plain"),
                Reply::new(303).header("location", b"https://api.github.com/restored"),
                Reply::new(200),
            ],
        });
    }
    for location in [b"#next".as_slice(), b"", b"?q=1", b"../next", b"/caf\xe9", b"/caf\xc3\xa9"] {
        cases.push(Case {
            base: "https://api.github.com".into(),
            replies: vec![
                Reply::new(302).header("location", b"/start#original"),
                Reply::new(307).header("location", location),
                Reply::new(200),
            ],
        });
    }
    cases.push(Case {
        base: "https://api.github.com".into(),
        replies: vec![
            Reply::new(302)
                .header("location", b"/next")
                .header("set-cookie", b"session=fixture; Path=/; Secure"),
            Reply::new(302).header("location", b"https://mirror.test/other"),
            Reply::new(302).header("location", b"https://api.github.com/final"),
            Reply::new(200),
        ],
    });
    for status in [200, 302, 401] {
        let mut reply =
            Reply::new(status).header("content-encoding", b"gzip").header("location", b"/next");
        reply.body = b"invalid gzip".to_vec();
        cases.push(Case {
            base: "https://api.github.com".into(),
            replies: vec![reply, Reply::new(200)],
        });
    }
    for base in ["http://user:pass@api.github.com", "https://user:pass@api.github.com:8443"] {
        for location in
            ["https:/next", "http:///next", "https://:8443/next", "https://new:pass@:8443/next"]
        {
            cases.push(Case {
                base: base.into(),
                replies: vec![
                    Reply::new(302).header("location", location.as_bytes()),
                    Reply::new(200),
                ],
            });
        }
    }
    cases
}
