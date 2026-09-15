use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

#[test]
fn subprocess_text_matches_cpython_utf8_errors_and_universal_newlines() {
    let mut inputs = vec![vec![]];
    for first in 0..=255u8 {
        inputs.push(vec![first]);
        for second in 0..=255u8 {
            inputs.push(vec![first, second]);
        }
    }
    for first in [0xe0, 0xed, 0xf0, 0xf4] {
        for second in 0..=255u8 {
            for third in [0, 0x80, 0xa0, 0xbf, 0xc0] {
                for tail in [None, Some(0), Some(0x80), Some(0xbf)] {
                    let mut bytes = vec![b'x', first, second, third];
                    bytes.extend(tail);
                    inputs.push(bytes);
                }
            }
        }
    }
    for text in
        ["3.12\r\n", "3\r12\n", "\u{feff}3.12", "日本語𝟛.١٢", "\u{1c}\r\n\u{85}", "a\r\r\nb"]
    {
        inputs.push(text.as_bytes().to_vec());
    }
    let cases: Vec<_> = inputs
        .into_iter()
        .map(|bytes| {
            json!({
                "mode": "text", "expected": outcome(super::text::decode(&bytes)), "bytes": bytes,
            })
        })
        .collect();
    assert_eq!(cases.len(), 86279);
    compare(&cases);
    digest(&cases, "379b57f3f5d1a4ec5a594b0c16c3d4916a86ac0a107a1993c2ba022dc58de91f");
}

#[test]
fn version_outcomes_decode_both_streams_before_checking_status() {
    let streams = [
        b"".as_slice(),
        b" \r\n\t",
        b"3.12\n",
        b"3\r12\n",
        b"\xef\xbb\xbf3.12",
        b"\xff",
        b"x\xe2\x82",
        b"ok\0tail",
        "\u{1c}٣.١٢\u{1f}".as_bytes(),
        b"\xed\xa0\x80",
        b"ordinary stderr",
    ];
    let mut cases = Vec::new();
    for success in [false, true] {
        for stdout in streams {
            for stderr in streams {
                cases.push(json!({
                    "mode": "version", "success": success, "stdout": stdout, "stderr": stderr,
                    "expected": outcome(super::version_output(success, stdout, stderr)),
                }));
            }
        }
    }
    assert_eq!(cases.len(), 242);
    compare(&cases);
    digest(&cases, "526175975c4adb4c561fa65c5da2d046258b63491a2efd0cfe0ccaa1f1009efa");
    let command = super::version_command(Path::new("fixture-python"));
    let command = command.as_std();
    assert_eq!(command.get_envs().count(), 0);
    assert!(command.get_current_dir().is_none());
    assert_eq!(
        command.get_args().collect::<Vec<_>>(),
        ["-c", "import sys; print(f'{sys.version_info.major}.{sys.version_info.minor}')",]
    );
}

#[cfg(unix)]
#[tokio::test]
async fn pip_availability_matches_source_status_policy_without_decoding_output() {
    use std::os::unix::fs::PermissionsExt;

    let directory = tempfile::tempdir().unwrap();
    let executable = directory.path().join("python");
    let streams = [b"".as_slice(), b"ordinary\n", b"\xff\xed\xa0\x80", b"\0\r\n"];
    let mut cases = Vec::new();
    for status in [0, 1, 7, -15] {
        for stdout in streams {
            for stderr in streams {
                let termination = if status < 0 {
                    "kill -TERM $$".into()
                } else {
                    format!("exit {status}")
                };
                let script = format!(
                    "#!/bin/sh\nprintf '{}'\nprintf '{}' >&2\n{termination}\n",
                    octal(stdout),
                    octal(stderr)
                );
                std::fs::write(&executable, script).unwrap();
                std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755))
                    .unwrap();
                let available = super::has_pip(&executable).await;
                assert_eq!(available, status == 0);
                cases.push(json!({
                    "mode": "pip", "status": status, "stdout": stdout, "stderr": stderr,
                    "expected": {"value": available},
                }));
            }
        }
    }
    assert_eq!(cases.len(), 64);
    compare(&cases);
    digest(&cases, "79caf3f49ab82a9fcafdf2382fcf34b0d3b4d7016cf501aaa0476e11a35fe887");
    let command = super::pip_command(Path::new("fixture-python"));
    assert_eq!(command.as_std().get_envs().count(), 0);
    assert!(command.as_std().get_current_dir().is_none());
    assert_eq!(command.as_std().get_args().collect::<Vec<_>>(), ["-c", "import pip"]);
}

#[cfg(unix)]
fn octal(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("\\{byte:03o}")).collect()
}

#[cfg(unix)]
#[tokio::test]
async fn pip_launch_failures_and_deadline_return_false() {
    use std::os::unix::fs::PermissionsExt;
    use std::time::{Duration, Instant};

    let directory = tempfile::tempdir().unwrap();
    let executable = directory.path().join("python");
    assert!(!super::has_pip(&executable).await);
    assert!(!super::has_pip(directory.path()).await);
    std::fs::write(&executable, "#!/bin/sh\nexit 0\n").unwrap();
    assert!(!super::has_pip(&executable).await);
    std::fs::write(&executable, "#!/bin/sh\nprintf '%s' \"$$\" > \"$0.pid\"\nexec /bin/sleep 30\n")
        .unwrap();
    std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755)).unwrap();
    let started = Instant::now();
    assert!(!super::has_pip(&executable).await);
    assert!(started.elapsed() >= Duration::from_secs(10));
    assert!(started.elapsed() < Duration::from_secs(20));
    assert_terminated(&executable.with_extension("pid")).await;
}

#[cfg(unix)]
#[tokio::test]
async fn version_launch_failures_and_deadline_return_no_observation() {
    use std::os::unix::fs::PermissionsExt;
    use std::time::{Duration, Instant};

    let directory = tempfile::tempdir().unwrap();
    assert!(super::version(&directory.path().join("missing")).await.unwrap().is_none());
    assert!(super::version(directory.path()).await.unwrap().is_none());
    let executable = directory.path().join("python");
    std::fs::write(&executable, "#!/bin/sh\nexit 0\n").unwrap();
    assert!(super::version(&executable).await.unwrap().is_none());
    let pid_file = executable.with_extension("pid");
    std::fs::write(&executable, "#!/bin/sh\nprintf '%s' \"$$\" > \"$0.pid\"\nexec /bin/sleep 30\n")
        .unwrap();
    std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755)).unwrap();
    let started = Instant::now();
    assert!(super::version(&executable).await.unwrap().is_none());
    assert!(started.elapsed() >= Duration::from_secs(10));
    assert!(started.elapsed() < Duration::from_secs(20));
    assert_terminated(&pid_file).await;
}

#[cfg(unix)]
async fn assert_terminated(pid_file: &Path) {
    use std::time::{Duration, Instant};

    let pid: i32 = std::fs::read_to_string(pid_file).unwrap().parse().unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        // SAFETY: signal zero only observes the owned fixture process.
        if unsafe { libc::kill(pid, 0) } == -1 {
            assert_eq!(std::io::Error::last_os_error().raw_os_error(), Some(libc::ESRCH));
            break;
        }
        assert!(Instant::now() < deadline, "timed-out interpreter remains alive");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

fn outcome<T: serde::Serialize>(result: crate::error::Result<T>) -> Value {
    match result {
        Ok(value) => json!({"value": value}),
        Err(error) => json!({"error": error.to_string()}),
    }
}

fn digest(cases: &[Value], expected: &str) {
    let values: Vec<_> = cases.iter().map(|case| &case["expected"]).collect();
    let actual = format!("{:x}", Sha256::digest(serde_json::to_vec(&values).unwrap()));
    assert_eq!(actual, expected);
}

fn compare(cases: &[Value]) {
    let Some(python) = std::env::var_os("HY_TEST_VENV_ORACLE_PYTHON") else {
        return;
    };
    let source =
        std::env::var_os("HY_TEST_HCLI_SOURCE").map(std::path::PathBuf::from).unwrap_or_else(
            || Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("ida-hcli"),
        );
    let mut child = Command::new(python)
        .args(["-I", "-B", "-c", include_str!("reference.py")])
        .arg(source)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&serde_json::to_vec(cases).unwrap()).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let actual: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(actual.len(), cases.len());
    for (actual, case) in actual.iter().zip(cases) {
        assert_eq!(*actual, case["expected"], "{case}");
    }
}
