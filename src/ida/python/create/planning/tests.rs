use super::*;
use serde_json::{Value, json};

fn probe(version: &str, root: &Path) -> Probe {
    Probe {
        frozen: false,
        executable: None,
        prefix: root.display().to_string(),
        base_prefix: root.display().to_string(),
        version: version.into(),
        externally_managed: false,
        virtual_env: None,
        idapython_venv_executable: None,
    }
}

#[test]
fn version_validation_and_probe_precedence_match_upstream() {
    let mut cases = Vec::new();
    for (explicit, expected_valid) in [
        None,
        Some("3.13"),
        Some("3.12"),
        Some("2.7"),
        Some("4.0"),
        Some("03.013"),
        Some("٣.١٣"),
        Some("3.13\n"),
        Some("3.13\n\n"),
        Some("3.13.1"),
        Some(" 3.13"),
        Some(""),
    ]
    .into_iter()
    .zip([true, true, true, true, true, true, true, true, false, false, false, false])
    {
        for probed in [None, Some("3.13")] {
            let valid = explicit.is_none_or(valid_version);
            assert_eq!(valid, expected_valid, "{explicit:?}");
            let actual = if valid {
                choose_version(
                    explicit.map(String::from),
                    probed.map(|version| probe(version, Path::new("/base"))),
                )
                .ok()
                .map(|version| json!([version.value, version.source]))
            } else {
                None
            };
            if probed.is_some() && valid {
                assert_eq!(actual, Some(json!(["3.13", "idat probe"])));
            } else if valid {
                assert_eq!(actual, explicit.map(|value| json!([value, "--python-version"])));
            } else {
                assert_eq!(actual, None);
            }
            cases.push(
                json!({"kind":"version", "explicit":explicit, "probed":probed, "expected":actual}),
            );
        }
    }
    compare(&cases);
}

#[test]
fn creation_plans_preserve_interpreter_priority_and_argument_boundaries() {
    let mut cases = Vec::new();
    for registered in [None, Some(PathBuf::from("/registered python"))] {
        for uv in [None, Some(PathBuf::from("/tools/uv"))] {
            for fallback in [None, Some(PathBuf::from("/fallback/python"))] {
                let target = Path::new("/environment with spaces");
                let actual = plan(target, "3.13", registered.clone(), uv.clone(), fallback.clone()).ok()
                    .map(|plan| json!({"tool":plan.tool, "command":std::iter::once(plan.executable.into_os_string())
                        .chain(plan.arguments).map(|arg| arg.to_string_lossy().into_owned()).collect::<Vec<_>>()}));
                cases.push(
                    json!({"kind":"plan", "registered":registered, "uv":uv, "fallback":fallback,
                    "target":target, "expected":actual}),
                );
            }
        }
    }
    let expected = json!([
        null,
        {"tool":"venv","command":["/fallback/python","-m","venv","/environment with spaces"]},
        {"tool":"uv","command":["/tools/uv","venv","--seed","--python","3.13","/environment with spaces"]},
        {"tool":"uv","command":["/tools/uv","venv","--seed","--python","/fallback/python","/environment with spaces"]},
        {"tool":"venv","command":["/registered python","-m","venv","/environment with spaces"]},
        {"tool":"venv","command":["/registered python","-m","venv","/environment with spaces"]},
        {"tool":"uv","command":["/tools/uv","venv","--seed","--python","/registered python","/environment with spaces"]},
        {"tool":"uv","command":["/tools/uv","venv","--seed","--python","/registered python","/environment with spaces"]}
    ]);
    assert_eq!(json!(cases.iter().map(|case| &case["expected"]).collect::<Vec<_>>()), expected);
    compare(&cases);
}

#[test]
fn registered_interpreters_use_base_prefix_and_skip_store_shims() {
    let root = tempfile::tempdir().unwrap();
    let base = root.path().join("base");
    let paths = candidates(&base, Some("3.13"));
    for path in &paths {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"fixture").unwrap();
    }
    let executable = root.path().join("registered-python");
    std::fs::write(&executable, b"fixture").unwrap();
    let shim = root.path().join("Microsoft/WindowsApps/python.exe");
    std::fs::create_dir_all(shim.parent().unwrap()).unwrap();
    std::fs::write(&shim, b"fixture").unwrap();
    let mut cases = Vec::new();
    let expected = [
        Some(executable.clone()),
        Some(executable.clone()),
        Some(paths[0].clone()),
        Some(paths[0].clone()),
        Some(paths[0].clone()),
        None,
    ];
    for (exe, virtual_env, prefix) in [
        (executable.clone(), None, base.clone()),
        (executable.clone(), Some(""), base.clone()),
        (executable.clone(), Some("venv"), base.clone()),
        (shim, None, base.clone()),
        (root.path().join("idat"), None, base.clone()),
        (executable, None, PathBuf::new()),
    ] {
        let mut info = probe("3.13", &prefix);
        info.executable = Some(exe.display().to_string());
        info.virtual_env = virtual_env.map(String::from);
        let actual = registered_python(Some(&info));
        cases.push(json!({"kind":"registered", "probe":info, "expected":actual}));
    }
    assert_eq!(
        json!(cases.iter().map(|case| &case["expected"]).collect::<Vec<_>>()),
        json!(expected)
    );
    compare(&cases);
}

fn compare(cases: &[Value]) {
    let Some(python) = std::env::var_os("HY_TEST_VENV_ORACLE_PYTHON") else {
        return;
    };
    use std::io::Write;
    use std::process::{Command, Stdio};
    let source = std::env::var_os("HY_TEST_HCLI_SOURCE").map(PathBuf::from).unwrap_or_else(|| {
        Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("ida-hcli")
    });
    let script = r#"
import ast, dataclasses, json, logging, os, platform, re, sys
from pathlib import Path
from types import SimpleNamespace
root = Path(sys.argv[1]) / 'src/hcli/lib'
namespace = dict(globals(), dataclass=dataclasses.dataclass, VERSION_RE=re.compile(r'^\d+\.\d+$'), logger=logging.getLogger('oracle'))
future = ast.ImportFrom(module='__future__', names=[ast.alias(name='annotations')], level=0)
for path, names in [(root / 'venv.py', {'get_python_exe_candidates'}),
    (root / 'ida/python/__init__.py', {'_is_windows_store_shim', '_is_python_executable_name'}),
    (root / 'ida/python/venv_create.py', None)]:
    nodes = [node for node in ast.parse(path.read_text()).body if isinstance(node, (ast.ClassDef, ast.FunctionDef)) and (names is None or node.name in names)]
    exec(compile(ast.fix_missing_locations(ast.Module(body=[future]+nodes, type_ignores=[])), str(path), 'exec'), namespace)
results = []
for case in json.load(sys.stdin):
    try:
        if case['kind'] == 'version':
            def get_probe():
                if case['probed'] is None: raise RuntimeError('no IDA')
                return SimpleNamespace(version_major=3, version_minor=13)
            namespace['probe_current_python_info'] = get_probe
            result = namespace['determine_target_python_version'](case['explicit'])
            result = [result.version, result.source]
        elif case['kind'] == 'plan':
            paths = {name: Path(case[name]) if case[name] else None for name in ['registered','uv','fallback']}
            result = namespace['plan_virtual_environment'](Path(case['target']), '3.13',
                registered_python=paths['registered'], uv_exe=paths['uv'], path_python=paths['fallback'])
            result = dict(tool=result.tool, command=result.build_command())
        else:
            info = SimpleNamespace(**case['probe'], version_major=3, version_minor=13)
            result = namespace['get_registered_python_exe'](info)
            result = str(result) if result else None
    except (ValueError, namespace['VenvCreationError']): result = None
    results.append(result)
print(json.dumps(results))
"#;
    let mut child = Command::new(python)
        .args(["-I", "-B", "-c", script])
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
