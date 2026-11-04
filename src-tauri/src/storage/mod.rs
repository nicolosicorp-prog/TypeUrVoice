pub mod settings;
pub mod history;
pub mod dictionary;
pub mod api_keys;
pub mod download;

use anyhow::Result;
use std::path::Path;

/// Restrict file permissions to owner-only read/write (0600 on Unix, no-op on
/// Windows). Used for `settings.json`, `.api_key`, `history.db` — anything
/// containing user data or secrets.
#[cfg(unix)]
pub(crate) fn restrict_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
pub(crate) fn restrict_permissions(_path: &Path) -> Result<()> {
    Ok(())
}
