//! `ida://` URL protocol handler registration / removal.

use crate::error::Result;

/// Register the `ida://` protocol handler for the current platform.
pub fn register_protocol_handler(binary_path: &str) -> Result<()> {
    if cfg!(target_os = "macos") {
        register_macos(binary_path)
    } else if cfg!(target_os = "windows") {
        register_windows(binary_path)
    } else {
        register_linux(binary_path)
    }
}

/// Unregister the `ida://` protocol handler.
pub fn unregister_protocol_handler() -> Result<()> {
    if cfg!(target_os = "macos") {
        unregister_macos()
    } else if cfg!(target_os = "windows") {
        unregister_windows()
    } else {
        unregister_linux()
    }
}

// ── macOS ───────────────────────────────────────────────────────────────

fn register_macos(binary_path: &str) -> Result<()> {
    let app_dir = dirs::home_dir()
        .unwrap_or_default()
        .join("Applications")
        .join("HCLIHandler.app")
        .join("Contents");

    std::fs::create_dir_all(app_dir.join("MacOS"))?;

    // AppleScript-based handler.
    let script = format!(
        r#"on open location theURL
    do shell script "{binary_path} open " & quoted form of theURL
end open location"#
    );

    let scpt_path = app_dir.join("MacOS").join("HCLIHandler");
    std::fs::write(&scpt_path, script)?;

    // Info.plist with URL scheme.
    let plist = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleURLTypes</key>
    <array>
        <dict>
            <key>CFBundleURLName</key>
            <string>IDA Protocol Handler</string>
            <key>CFBundleURLSchemes</key>
            <array>
                <string>ida</string>
            </array>
        </dict>
    </array>
</dict>
</plist>"#;
    std::fs::write(app_dir.join("Info.plist"), plist)?;
    Ok(())
}

fn unregister_macos() -> Result<()> {
    let app_dir = dirs::home_dir()
        .unwrap_or_default()
        .join("Applications")
        .join("HCLIHandler.app");
    if app_dir.exists() {
        std::fs::remove_dir_all(app_dir)?;
    }
    Ok(())
}

// ── Windows ─────────────────────────────────────────────────────────────

fn register_windows(binary_path: &str) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        use winreg::enums::*;
        use winreg::RegKey;

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let (key, _) = hkcu.create_subkey(r"SOFTWARE\Classes\ida")?;
        key.set_value("", &"URL:IDA Protocol")?;
        key.set_value("URL Protocol", &"")?;

        let (cmd_key, _) = key.create_subkey(r"shell\open\command")?;
        cmd_key.set_value("", &format!("\"{binary_path}\" open \"%1\""))?;
    }
    let _ = binary_path;
    Ok(())
}

fn unregister_windows() -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        use winreg::enums::*;
        use winreg::RegKey;

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let _ = hkcu.delete_subkey_all(r"SOFTWARE\Classes\ida");
    }
    Ok(())
}

// ── Linux ───────────────────────────────────────────────────────────────

fn register_linux(binary_path: &str) -> Result<()> {
    let desktop_dir = dirs::data_dir()
        .unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_default()
                .join(".local")
                .join("share")
        })
        .join("applications");
    std::fs::create_dir_all(&desktop_dir)?;

    let desktop_entry = format!(
        r#"[Desktop Entry]
Name=HCLI Handler
Exec={binary_path} open %u
Type=Application
NoDisplay=true
MimeType=x-scheme-handler/ida;
"#
    );

    let path = desktop_dir.join("hcli-handler.desktop");
    std::fs::write(&path, desktop_entry)?;

    // Register with xdg-mime.
    let _ = std::process::Command::new("xdg-mime")
        .args(["default", "hcli-handler.desktop", "x-scheme-handler/ida"])
        .status();

    Ok(())
}

fn unregister_linux() -> Result<()> {
    let path = dirs::data_dir()
        .unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_default()
                .join(".local")
                .join("share")
        })
        .join("applications")
        .join("hcli-handler.desktop");
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}
