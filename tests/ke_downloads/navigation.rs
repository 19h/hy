//! KE dispatch against owned IPC instances and isolated launcher programs.

use std::fs;

use serde_json::json;

use super::fixture::*;
use super::support::{assert_success, ida_ipc::IpcFixture};

#[test]
fn repeated_download_parameters_use_the_final_value_and_reject_a_final_blank() {
    let fixture = Fixture::new(response(200, b"{}"), response(200, CONTENT));
    let uri = format!("ida://ke/{FILENAME}?url=invalid&{}", fixture.uri().query().unwrap());
    assert_success(&fixture.command_for_uri(&uri, true).output().unwrap());
    fixture.assert_request_order();
    assert_eq!(fs::read(fixture.path()).unwrap(), CONTENT);

    let fixture = Fixture::new(response(200, b"{}"), response(200, CONTENT));
    let uri = format!("{}&url=", fixture.uri());
    let output = fixture.command_for_uri(&uri, true).output().unwrap();
    assert_error(&output, "missing the 'url'");
    assert!(fixture.dialogs.text("error").contains("missing the 'url'"));
    assert!(fixture.server.requests().is_empty());
    assert!(!fixture.sandbox.path().join("downloads").exists());
}

#[test]
fn open_only_links_reuse_the_matching_database_without_sending_navigation() {
    let fixture = Fixture::new(response(200, b"{}"), response(200, CONTENT));
    let ipc = IpcFixture::for_database(fixture.path(), true);
    let uri = format!("{}&other=1&EA=0", fixture.uri());
    let output =
        fixture.command_for_uri(&uri, false).env("HCLI_KE_SKIP_CONFIRM", "1").output().unwrap();
    assert_success(&output);
    assert!(ipc.navigations().is_empty());
    assert_eq!(ipc.analysis_query_count(), 0);
    assert!(String::from_utf8_lossy(&output.stdout).contains("Opening "));
    assert!(!fixture.dialogs.directory.join("error").exists());
    assert!(!fixture.sandbox.config_path().exists());
}

#[test]
fn open_only_launches_still_wait_for_the_database_and_auto_analysis() {
    let fixture = Fixture::new(response(200, b"{}"), response(200, CONTENT));
    let installation = fixture.installation();
    fs::write(installation.join("python/ida_pro.py"), "# IDA SDK v9.4\n").unwrap();
    let ipc =
        IpcFixture::for_database_after_launch(fixture.path(), installation.join("ida.database"));
    let output = fixture
        .command_for(false)
        .args(["--timeout", "2"])
        .env("HCLI_KE_SKIP_CONFIRM", "1")
        .env("HCLI_CURRENT_IDA_INSTALL_DIR", installation)
        .output()
        .unwrap();
    assert_success(&output);
    assert!(ipc.navigations().is_empty());
    assert_eq!(ipc.analysis_query_count(), 1);
    assert!(!fixture.dialogs.directory.join("error").exists());
}

#[test]
fn navigation_preserves_raw_query_fields_authority_spelling_and_fragments() {
    let fixture = Fixture::new(response(200, b"{}"), response(200, CONTENT));
    let ipc = IpcFixture::for_database(fixture.path(), false);
    let uri = format!(
        "IDA://Ke:invalid/{FILENAME}?{}&&%72va=&name=a%20b&view=raw+view&flag&#section",
        fixture.uri().query().unwrap()
    );
    let output =
        fixture.command_for_uri(&uri, false).env("HCLI_KE_SKIP_CONFIRM", "1").output().unwrap();
    assert_success(&output);
    assert_eq!(
        ipc.navigations(),
        vec![json!({
            "cmd": "open_ida_link",
            "uri": format!("ida://Ke:invalid/{FILENAME}?%72va=&name=a%20b&view=raw+view&flag#section"),
        })]
    );
    assert_eq!(ipc.analysis_query_count(), 0);
}

#[test]
fn launch_and_startup_failures_use_the_native_error_dialog() {
    for (version, expected) in [("9.3", "Failed to launch IDA"), ("9.4", "startup timed out")] {
        let fixture = Fixture::new(response(200, b"{}"), response(200, CONTENT));
        let installation = fixture.installation();
        fs::write(installation.join("python/ida_pro.py"), format!("# IDA SDK v{version}\n"))
            .unwrap();
        if version == "9.3" {
            fs::write(installation.join("ida"), "#!/nonexistent/hy-fixture-interpreter\n").unwrap();
        }
        let output = fixture
            .command_for(false)
            .args(["--timeout", "0"])
            .env("HCLI_KE_SKIP_CONFIRM", "1")
            .env("HCLI_CURRENT_IDA_INSTALL_DIR", &installation)
            .output()
            .unwrap();
        assert_error(&output, expected);
        assert!(fixture.dialogs.text("error").contains(expected));
        fixture.assert_dialog_dismissed();
        assert_eq!(fs::read(fixture.path()).unwrap(), CONTENT);
        if version == "9.4" {
            super::support::assert_file_eventually(
                &installation.join("ida.database"),
                fixture.path().as_os_str().as_encoded_bytes(),
            );
        }
    }
}

#[test]
fn an_ipc_navigation_rejection_is_not_reported_as_a_launch_failure() {
    let fixture = Fixture::new(response(200, b"{}"), response(200, CONTENT));
    let ipc = IpcFixture::for_database(fixture.path(), true);
    let uri = format!("{}&rva=1", fixture.uri());
    let output =
        fixture.command_for_uri(&uri, false).env("HCLI_KE_SKIP_CONFIRM", "1").output().unwrap();
    assert_error(&output, "fixture navigation rejected");
    assert_eq!(ipc.navigations().len(), 1);
    assert!(!fixture.dialogs.directory.join("error").exists());
}
