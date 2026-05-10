//! Stronghold-backed credential persistence — exposes the vault path
//! and a deterministic per-machine "auto password" derived from the
//! salt that the FE submits to `tauri-plugin-stronghold`.

use crate::secrets;

#[tauri::command]
pub async fn vault_paths() -> Result<serde_json::Value, String> {
    let pw = secrets::auto_password_hex()
        .map_err(|e| format!("salt unreadable; vault would derive a wrong key: {e}"))?;
    Ok(serde_json::json!({
        "vault_path": secrets::vault_path().to_string_lossy(),
        "auto_password": pw,
    }))
}
