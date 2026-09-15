//! Per-user protocol registry entries for the native executable.

use winreg::RegKey;
use winreg::enums::HKEY_CURRENT_USER;

use crate::error::Result;

const KEY: &str = r"SOFTWARE\Classes\ida";

pub(super) fn register(binary_path: &str) -> Result<()> {
    let current_user = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = current_user.create_subkey(KEY)?;
    key.set_value("", &"URL:IDA Protocol")?;
    key.set_value("URL Protocol", &"")?;
    let (icon, _) = key.create_subkey("DefaultIcon")?;
    icon.set_value("", &format!("\"{binary_path}\",1"))?;
    let (command, _) = key.create_subkey(r"shell\open\command")?;
    command.set_value("", &format!("\"{binary_path}\" ida open -- \"%1\""))?;
    Ok(())
}

pub(super) fn unregister() -> Result<()> {
    let current_user = RegKey::predef(HKEY_CURRENT_USER);
    for suffix in [r"\shell\open\command", r"\shell\open", r"\shell", r"\DefaultIcon", ""] {
        match current_user.delete_subkey(format!("{KEY}{suffix}")) {
            Ok(()) => (),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}
