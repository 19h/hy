//! Verify launcher files and ordered desktop-tool calls without changing MIME state.

use std::ffi::OsString;
use std::fs;

use super::*;

#[derive(Debug, PartialEq, Eq)]
struct Invocation {
    program: OsString,
    arguments: Vec<OsString>,
    required: bool,
}

impl Invocation {
    fn capture(command: &Command, required: bool) -> Self {
        Self {
            program: command.get_program().into(),
            arguments: command.get_args().map(OsString::from).collect(),
            required,
        }
    }
}

#[test]
fn desktop_launcher_quotes_paths_and_preserves_the_url_placeholder() {
    assert_eq!(desktop_quote("/opt/IDA Tools/100%/hy"), "\"/opt/IDA Tools/100%%/hy\"");
    assert_eq!(desktop_quote("/opt/a\"b\\c$d`e"), "\"/opt/a\\\\\"b\\\\\\\\c\\\\$d\\\\`e\"");
    let entry = desktop_entry("/opt/IDA Tools/hy");
    assert!(
        entry.contains(&format!(
            "Exec={PYTHON_ENVIRONMENT_RESET} \"/opt/IDA Tools/hy\" ida open -- %u\n"
        )),
        "{entry}"
    );
}

#[test]
fn registration_and_removal_update_the_canonical_file_and_mime_association() {
    let temporary = tempfile::tempdir().unwrap();
    let directory = temporary.path().join("applications");
    let mut calls = Vec::new();
    let mut run = |command: &mut Command, required| {
        calls.push(Invocation::capture(command, required));
        Ok(())
    };
    register_at("/opt/hy", &directory, &mut run).unwrap();
    let path = directory.join("hcli-idb-handler.desktop");
    assert_eq!(fs::read_to_string(&path).unwrap(), desktop_entry("/opt/hy"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o755);
    }
    unregister_at(&directory, &mut run).unwrap();
    assert!(!path.exists());
    unregister_at(&directory, &mut run).unwrap();
    assert_eq!(
        calls,
        vec![
            Invocation {
                program: "xdg-mime".into(),
                arguments: vec![
                    "default".into(),
                    "hcli-idb-handler.desktop".into(),
                    "x-scheme-handler/ida".into()
                ],
                required: true
            },
            Invocation {
                program: "update-desktop-database".into(),
                arguments: vec![directory.as_os_str().into()],
                required: false
            },
            Invocation {
                program: "xdg-mime".into(),
                arguments: vec!["default".into(), "".into(), "x-scheme-handler/ida".into()],
                required: false
            },
            Invocation {
                program: "update-desktop-database".into(),
                arguments: vec![directory.as_os_str().into()],
                required: false
            },
        ]
    );
}

#[test]
fn failed_mime_registration_stops_before_database_update() {
    let temporary = tempfile::tempdir().unwrap();
    let mut calls = 0;
    let error = register_at("/opt/hy", temporary.path(), &mut |command, required| {
        calls += 1;
        assert_eq!(command.get_program(), "xdg-mime");
        assert!(required);
        Err(crate::error::Error::Other("fixture MIME failure".into()))
    })
    .unwrap_err();
    assert_eq!(calls, 1);
    assert!(error.to_string().contains("fixture MIME failure"));
    // Upstream leaves the desktop file after a failed xdg-mime command.
    assert!(temporary.path().join(DESKTOP_FILE).is_file());
}
