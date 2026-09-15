use serde_json::json;

use super::*;

#[test]
fn listings_use_python_line_boundaries_and_ascii_name_casefolds() {
    for (text, name, expected) in [
        (" • ida-mcp enabled", "ida-mcp", true),
        ("ida-mcp-other enabled", "ida-mcp", false),
        ("◆◆--HexRayßA enabled", "HexRaysSA", true),
        ("◆ HexRayſSA enabled", "HexRaysSA", true),
        ("x\u{85}ida-mcp enabled", "ida-mcp", true),
        ("x\u{2028}ida-mcp enabled", "ida-mcp", true),
        ("x\u{1c}ida-mcp enabled", "ida-mcp", true),
        ("ida-mcp\u{1f}enabled", "ida-mcp", true),
        ("  • - ida-mcp enabled", "ida-mcp", false),
        ("ＩＤＡ-MCP enabled", "ida-mcp", false),
    ] {
        assert_eq!(has_line(text, name), expected, "{text:?}");
    }
    assert!(has_named(
        &json!({"items": [{"pluginId": "IDA-MCP@HexRayẞA"}]}),
        super::super::PLUGIN_ID
    ));
    assert!(!has_named(&json!({"description": super::super::PLUGIN_ID}), super::super::PLUGIN_ID));
    assert!(pi_installed(
        "Uſer packages:\u{85}git:github.com/HexRayßA/ida-mcp@latest",
        Scope::Global
    ));
    assert!(!pi_installed("User packages:\nHexRaysSA/ida-mcp", Scope::Local));
    assert!(pi_installed(
        "User packages:\nother\nLocal packages:\nHexRaysSA/ida-mcp",
        Scope::Local
    ));
}

#[test]
fn ascii_name_folding_matches_every_cpython_ascii_fold_mapping() {
    let expected = [
        (0xdf, "ss"),
        (0x17f, "s"),
        (0x1e9e, "ss"),
        (0x212a, "k"),
        (0xfb00, "ff"),
        (0xfb01, "fi"),
        (0xfb02, "fl"),
        (0xfb03, "ffi"),
        (0xfb04, "ffl"),
        (0xfb05, "st"),
        (0xfb06, "st"),
    ];
    for (code, ascii) in expected {
        assert_eq!(folded(&char::from_u32(code).unwrap().to_string()), ascii);
    }
    if let Some(python) = std::env::var_os("HY_TEST_MCP_ORACLE_PYTHON") {
        let output = std::process::Command::new(python).args(["-I", "-B", "-c", r#"
import json
print(json.dumps([(code, chr(code).casefold()) for code in range(128, 0x110000) if not 0xD800 <= code <= 0xDFFF and chr(code).casefold().isascii()]))
"#]).output().unwrap();
        assert!(output.status.success());
        assert_eq!(serde_json::from_slice::<Value>(&output.stdout).unwrap(), json!(expected));
    }
}
