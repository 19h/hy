//! Read-only HTTPcore probe: a fake backend records TLS identity arguments.

use std::process::Command;

pub(super) fn assert_tls_identities() {
    let Some(python) = std::env::var_os("HY_TEST_HTTPX_ORACLE_PYTHON") else {
        return;
    };
    let output = Command::new(python).args(["-I", "-B", "-c", r#"
import httpcore, json
class Stream:
    def __init__(self, names): self.names, self.reply = names, b''
    def read(self, max_bytes, timeout=None):
        data, self.reply = self.reply[:max_bytes], self.reply[max_bytes:]
        return data
    def write(self, data, timeout=None):
        self.reply = b'HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n'
    def close(self): pass
    def start_tls(self, ssl_context, server_hostname=None, timeout=None):
        self.names.append(server_hostname)
        return self
    def get_extra_info(self, info): return None
class Backend:
    def __init__(self, names): self.names = names
    def connect_tcp(self, host, port, **kwargs): return Stream(self.names)
    def sleep(self, seconds): pass
results=[]
for pinned in [False, True]:
    for proxy_scheme in ['http', 'https']:
        for target_scheme in ['http', 'https']:
            names=[]
            host='203.0.113.1' if pinned else 'original.test'
            extensions={'sni_hostname':'original.test'} if pinned else {}
            with httpcore.HTTPProxy(proxy_url=f'{proxy_scheme}://proxy.test:8080', network_backend=Backend(names)) as proxy:
                response=proxy.handle_request(httpcore.Request('GET', f'{target_scheme}://{host}/path', headers=[(b'Host',b'original.test')], extensions=extensions))
                response.close()
            results.append(names)
print(json.dumps(results))
"#]).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let names: Vec<Vec<String>> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        names,
        vec![
            Vec::<String>::new(),
            vec!["original.test".into()],
            vec!["proxy.test".into()],
            vec!["proxy.test".into(), "original.test".into()],
            vec![],
            vec!["203.0.113.1".into()],
            vec!["original.test".into()],
            vec!["original.test".into(), "203.0.113.1".into()],
        ]
    );
    eprintln!("HTTPcore TLS identity oracle matched 8 proxy modes");
}
