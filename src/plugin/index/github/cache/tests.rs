use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::UNIX_EPOCH;

use serde_json::{Value, json};

use super::*;

const NOW: u64 = 2_000_000_000;
const DAY: Duration = Duration::from_secs(86_400);
const CONTENT: &[u8] = b"\"fixture\"";

#[test]
fn cache_boundary_uses_the_clock_float_before_subtracting_stat_time() {
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("entry");
    fs::write(&path, CONTENT).unwrap();
    fs::File::open(&path)
        .unwrap()
        .set_modified(UNIX_EPOCH + Duration::new(1_700_000_000, 20))
        .unwrap();
    let now = UNIX_EPOCH + Duration::new(1_700_086_400, 120);
    assert_eq!(read_path(&path, Some(DAY), || now).unwrap().as_deref(), Some(CONTENT));
    assert!(path.is_file());
}

fn fixture(root: &Path, name: &str) -> PathBuf {
    fs::create_dir_all(root).unwrap();
    let path = root.join("entry");
    match name {
        "missing" => return path,
        "missing-parent" => return root.join("absent/entry"),
        "file-parent" => {
            fs::write(&path, CONTENT).unwrap();
            return path.join("entry");
        }
        "directory" | "expired-directory" => fs::create_dir(&path).unwrap(),
        #[cfg(unix)]
        "dangling-link" => {
            std::os::unix::fs::symlink(root.join("absent"), &path).unwrap();
            return path;
        }
        #[cfg(unix)]
        "loop" => {
            std::os::unix::fs::symlink(&path, &path).unwrap();
            return path;
        }
        _ => fs::write(&path, CONTENT).unwrap(),
    }
    let modified = match name {
        "future" => NOW + 86_400,
        "boundary" => NOW - 86_400,
        "expired" | "expired-directory" => NOW - 86_401,
        _ => NOW,
    };
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // FILE_WRITE_ATTRIBUTES and FILE_FLAG_BACKUP_SEMANTICS also permit directories.
        options.access_mode(0x100).custom_flags(0x0200_0000);
    }
    options.open(&path).unwrap().set_modified(UNIX_EPOCH + Duration::from_secs(modified)).unwrap();
    path
}

#[test]
fn cache_reads_match_source_expiry_and_filesystem_error_boundaries() {
    let root = tempfile::tempdir().unwrap();
    let mut names = vec![
        "missing",
        "missing-parent",
        "file-parent",
        "fresh",
        "future",
        "boundary",
        "expired",
        "directory",
        "expired-directory",
    ];
    if cfg!(unix) {
        names.extend(["dangling-link", "loop"]);
    }
    let mut cases = Vec::new();
    let mut expected = Vec::new();
    for kind in ["candidates", "asset", "source"] {
        for name in &names {
            let directory = root.path().join(kind).join(name);
            let source = fixture(&directory.join("source"), name);
            let native = fixture(&directory.join("native"), name);
            let lifetime = (kind == "candidates").then_some(DAY);
            let mut clock_reads = 0;
            let result = read_path(&native, lifetime, || {
                clock_reads += 1;
                UNIX_EPOCH + Duration::from_secs(NOW)
            });
            let state = match result {
                Ok(Some(bytes)) => {
                    assert_eq!(bytes, CONTENT);
                    "hit"
                }
                Ok(None) => "miss",
                Err(_) => "error",
            };
            let unlinked = kind == "candidates" && *name == "expired";
            let expected_state = match *name {
                "missing" | "missing-parent" | "file-parent" | "dangling-link" | "loop" => "miss",
                "directory" | "expired-directory" => "error",
                "expired" if kind == "candidates" => "miss",
                _ => "hit",
            };
            assert_eq!(state, expected_state, "{kind}/{name}");
            if unlinked {
                assert!(!native.exists());
            }
            cases.push(json!({"kind": kind, "path": source, "now": NOW}));
            expected.push(json!({
                "state": state,
                "unlinked": unlinked,
                "clock_reads": clock_reads,
            }));
        }
    }
    compare_source(&cases, &expected);
}

fn compare_source(cases: &[Value], expected: &[Value]) {
    let Some(python) = std::env::var_os("HY_TEST_BUNDLE_ORACLE_PYTHON") else {
        return;
    };
    let upstream =
        std::env::var_os("HY_TEST_HCLI_SOURCE").map(PathBuf::from).unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("ida-hcli")
        });
    let mut child = Command::new(python)
        .args(["-I", "-B", "-c", include_str!("reference.py")])
        .arg(upstream)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&serde_json::to_vec(&cases).unwrap()).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let actual: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(actual.len(), expected.len());
    for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        assert_eq!(actual, expected, "case {index}: {}", cases[index]);
    }
    eprintln!("matched {} upstream catalogue cache reads", cases.len());
}

#[test]
fn fractional_cache_expiry_matches_source_clock_and_native_stat_floats() {
    const FRACTIONS: [u32; 12] =
        [0, 1, 20, 119, 120, 121, 238, 239, 499_999_999, 500_000_000, 999_999_880, 999_999_999];
    let root = tempfile::tempdir().unwrap();
    let native = root.path().join("native");
    let mut cases = Vec::new();
    let mut expected = Vec::new();
    for seconds in [1_000_000, 1_700_000_000, 2_000_000_000, 4_294_967_295_u64] {
        for fraction in FRACTIONS {
            let modified = UNIX_EPOCH + Duration::new(seconds, fraction);
            let source = root.path().join(format!("{seconds}-{fraction}"));
            fs::write(&source, CONTENT).unwrap();
            fs::File::open(&source).unwrap().set_modified(modified).unwrap();
            for age in [-86_400, 86_400, 86_401_i64] {
                for fraction in FRACTIONS {
                    let now = Duration::new((seconds as i64 + age) as u64, fraction);
                    fs::write(&native, CONTENT).unwrap();
                    fs::File::open(&native).unwrap().set_modified(modified).unwrap();
                    let result = read_path(&native, Some(DAY), || UNIX_EPOCH + now).unwrap();
                    let hit = result.is_some();
                    assert_eq!(native.exists(), hit);
                    if let Some(bytes) = result {
                        assert_eq!(bytes, CONTENT);
                    }
                    cases.push(json!({
                        "kind": "candidates", "path": source,
                        "now_ns": u64::try_from(now.as_nanos()).unwrap(),
                    }));
                    expected.push(json!({
                        "state": if hit { "hit" } else { "miss" },
                        "unlinked": !hit, "clock_reads": 1,
                    }));
                }
            }
        }
    }
    assert_eq!(cases.len(), 1_728);
    compare_source(&cases, &expected);
}
