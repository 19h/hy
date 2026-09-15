use std::io::Write;
use std::process::{Command, Stdio};

use sha2::{Digest, Sha256};

use super::split;

type Cookies = Vec<Vec<(String, Option<String>)>>;

fn parse(text: &str) -> Cookies {
    split(text)
        .into_iter()
        .map(|attributes| {
            attributes.into_iter().map(|attribute| (attribute.key, attribute.value)).collect()
        })
        .collect()
}

#[test]
fn quoted_values_and_header_separators_preserve_cookie_boundaries() {
    assert_eq!(
        parse(r#"foo="bar"; port="80,81"; discard, bar=baz"#),
        vec![
            vec![
                ("foo".into(), Some("bar".into())),
                ("port".into(), Some("80,81".into())),
                ("discard".into(), None)
            ],
            vec![("bar".into(), Some("baz".into()))],
        ]
    );
    assert_eq!(
        parse(r#"Basic realm="\"foo\bar\"""#),
        vec![vec![("Basic".into(), None), ("realm".into(), Some("\"foobar\"".into()))],]
    );
    assert_eq!(parse("==; , ;"), Vec::<Vec<(String, Option<String>)>>::new());
}

fn corpus() -> Vec<String> {
    let mut cases = Vec::new();
    let alphabet = ['a', '=', ' ', ';', ',', '"', '\\', '\n', 'é'];
    for length in 0..=4 {
        for mut index in 0..alphabet.len().pow(length) {
            let mut text = String::new();
            for _ in 0..length {
                text.push(alphabet[index % alphabet.len()]);
                index /= alphabet.len();
            }
            cases.push(text);
        }
    }
    for text in [
        "a=\"x,y;z\"; Version=0,b=two",
        "a=\"x\\\ny\";Version=0",
        "a=\"x\ny\";Version=0",
        "a=\"x\\\ry\";Version=0",
        "a=\"x\\éy\";Version=0",
        "a=\"unterminated,b=two",
        "a=\"\"b=two",
        "a=\"\"\";b=two",
        "a=one b=two",
        "a=one;\u{1c}Version=0",
        "a=one\u{a0}Version=0",
        "a=one\u{200b}Version=0",
        "a=one\u{3000}Version=0",
    ] {
        cases.push(text.into());
    }
    cases
}

#[test]
fn header_word_corpus_matches_cpython() {
    let cases = corpus();
    let actual: Vec<_> = cases.iter().map(|text| parse(text)).collect();
    let digest = format!("{:x}", Sha256::digest(serde_json::to_vec(&actual).unwrap()));
    assert_eq!(digest, "21a67ebcfe39a070ee0ea6a870d179e9fbae4ff4bcf31f4fe962122baa98a5c2");
    eprintln!("header word corpus: {} cases, SHA-256 {digest}", cases.len());
    let Some(python) = std::env::var_os("HY_TEST_HTTPX_ORACLE_PYTHON") else {
        return;
    };
    let mut child = Command::new(python)
        .args(["-I", "-B", "-c", "import http.cookiejar,json,sys; json.dump([http.cookiejar.split_header_words([text]) for text in json.load(sys.stdin)],sys.stdout)"])
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .spawn().unwrap();
    child.stdin.take().unwrap().write_all(&serde_json::to_vec(&cases).unwrap()).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let expected: Vec<Cookies> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(actual.len(), expected.len());
    for ((text, actual), expected) in cases.iter().zip(actual).zip(expected) {
        assert_eq!(actual, expected, "{text:?}");
    }
}
