use super::*;

#[test]
fn location_decoding_uses_the_complete_response_header_encoding() {
    let current = url::Url::parse("http://example.test/start#keep").unwrap();
    let cases: Vec<Vec<(&str, Vec<u8>)>> = vec![
        vec![("location", "/café".as_bytes().to_vec())],
        vec![("location", b"/caf\xe9".to_vec())],
        vec![("location", "/café".as_bytes().to_vec()), ("x-other", vec![0xff])],
    ];
    let mut expected = Vec::new();
    for (headers, path) in cases.iter().zip(["/caf%C3%A9", "/caf%C3%A9", "/caf%C3%83%C2%A9"]) {
        let mut response = hyper::Response::builder().status(302);
        for (name, value) in headers {
            response = response.header(*name, header::HeaderValue::from_bytes(value).unwrap());
        }
        let response: reqwest::Response = response.body(Vec::<u8>::new()).unwrap().into();
        let target = target(&current, &Response::new(response)).unwrap().unwrap();
        assert_eq!(target.path(), path);
        assert_eq!(target.fragment(), Some("keep"));
        expected.push(target.to_string());
    }
    let Some(python) = std::env::var_os("HY_TEST_HTTPX_ORACLE_PYTHON") else {
        return;
    };
    use std::io::Write;
    use std::process::{Command, Stdio};
    let script = r#"
import json, sys, httpx
assert httpx.__version__ == '0.28.1'
cases = json.load(sys.stdin)
request = httpx.Request('GET', 'http://example.test/start#keep')
with httpx.Client() as client:
    results = [str(client._redirect_url(request, httpx.Response(302,
        headers=[(name.encode('ascii'), bytes(value)) for name, value in headers])))
        for headers in cases]
print(json.dumps(results))
"#;
    let mut child = Command::new(python)
        .args(["-I", "-B", "-c", script])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&serde_json::to_vec(&cases).unwrap()).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let actual: Vec<String> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(actual, expected);
}
