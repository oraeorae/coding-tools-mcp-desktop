use crate::error::AppResult;

#[tauri::command]
pub fn get_startup_restore_enabled() -> AppResult<bool> {
    crate::startup_restore::get_enabled()
}

#[tauri::command]
pub fn set_startup_restore_enabled(enabled: bool) -> AppResult<()> {
    crate::startup_restore::set_enabled(enabled)
}
