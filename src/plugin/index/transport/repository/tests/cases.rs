use serde::Serialize;

#[derive(Clone, Serialize)]
pub(super) struct Reply {
    pub status: u16,
    pub headers: Vec<(String, Vec<u8>)>,
    pub body: Vec<u8>,
}

impl Reply {
    pub fn new(status: u16) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: b"payload".to_vec(),
        }
    }

    pub fn header(mut self, name: &str, value: &[u8]) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }
}

#[derive(Serialize)]
pub(super) struct Case {
    pub url: String,
    pub authenticated: bool,
    pub replies: Vec<Reply>,
    pub repository: Option<String>,
}

pub(super) fn all() -> Vec<Case> {
    let mut cases = Vec::new();
    for status in
        [200, 204, 300, 301, 302, 303, 304, 305, 306, 307, 308, 399, 401, 403, 404, 429, 500]
    {
        for location in [None, Some(""), Some("/next")] {
            for host in ["mirror.test", "plugins.hex-rays.com"] {
                for authenticated in [false, true] {
                    let first = location.map_or_else(
                        || Reply::new(status),
                        |location| Reply::new(status).header("location", location.as_bytes()),
                    );
                    cases.push(Case {
                        url: format!("https://{host}/start"),
                        authenticated,
                        replies: vec![first, Reply::new(200)],
                        repository: None,
                    });
                }
            }
        }
    }
    for authenticated in [false, true] {
        for status in [200, 401, 403, 404] {
            cases.push(Case {
                url: "https://mirror.test/start".into(),
                authenticated,
                replies: vec![
                    Reply::new(302).header("location", b"https://plugins.hex-rays.com/first"),
                    Reply::new(307).header("location", b"https://mirror.test/second"),
                    Reply::new(308).header("location", b"https://sub.plugins.hex-rays.com/final"),
                    Reply::new(status),
                ],
                repository: Some("private".into()),
            });
        }
    }
    for count in [0, 1, 9, 10, 11, 12] {
        let mut replies = vec![Reply::new(302).header("location", b"/again"); count];
        replies.push(Reply::new(200));
        cases.push(Case {
            url: "https://plugins.hex-rays.com/start".into(),
            authenticated: true,
            replies,
            repository: None,
        });
    }
    for scheme in ["http", "https"] {
        cases.push(Case {
            url: format!("{scheme}://mirror.test/start"),
            authenticated: true,
            replies: vec![
                Reply::new(301).header("location", b"https://plugins.hex-rays.com/secure"),
                Reply::new(302).header("location", b"http://mirror.test/plain"),
                Reply::new(200),
            ],
            repository: None,
        });
    }
    for location in ["", "#next", "?q=1", "../next", "//mirror.test/next"] {
        cases.push(Case {
            url: "https://plugins.hex-rays.com/path/start#original".into(),
            authenticated: false,
            replies: vec![Reply::new(302).header("location", location.as_bytes()), Reply::new(200)],
            repository: None,
        });
    }
    cases.push(Case {
        url: "https://plugins.hex-rays.com/start".into(),
        authenticated: false,
        replies: vec![
            Reply::new(302)
                .header("location", b"/next")
                .header("set-cookie", b"session=fixture; Path=/; Secure"),
            Reply::new(302).header("location", b"https://mirror.test/other"),
            Reply::new(302).header("location", b"https://plugins.hex-rays.com/final"),
            Reply::new(200),
        ],
        repository: None,
    });
    for value in [b"/caf\xe9".as_slice(), b"/caf\xc3\xa9"] {
        cases.push(Case {
            url: "https://mirror.test/start".into(),
            authenticated: false,
            replies: vec![Reply::new(302).header("location", value), Reply::new(200)],
            repository: None,
        });
    }
    for status in [200, 302, 401] {
        let mut reply =
            Reply::new(status).header("content-encoding", b"gzip").header("location", b"/next");
        reply.body = b"invalid gzip".to_vec();
        cases.push(Case {
            url: "https://plugins.hex-rays.com/start".into(),
            authenticated: true,
            replies: vec![reply, Reply::new(200)],
            repository: None,
        });
    }
    cases
}
