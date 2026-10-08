//! App info and window helpers.

use tauri::{AppHandle, State};

use crate::dto::AppInfo;
use crate::error::{AppError, AppResult};
use crate::platform;
use crate::state::AppState;

#[tauri::command]
#[specta::specta]
pub fn app_info(app: AppHandle, state: State<'_, AppState>) -> AppInfo {
    AppInfo { version: app.package_info().version.to_string(), platform: platform::platform_name().into(), mica: state.mica }
}

/// Opens the Windows 11 Snap Layouts flyout (hovering the custom maximize button, WIN-01).
#[tauri::command]
#[specta::specta]
pub fn window_snap_overlay() {
    platform::window::snap_overlay();
}

/// Writes text to a path the user chose in the native save dialog (recovery code "Save as Text").
#[tauri::command]
#[specta::specta]
pub fn save_text_file(path: String, contents: String) -> AppResult<()> {
    let path = std::path::PathBuf::from(path);
    if !path.is_absolute() || path.is_dir() {
        return Err(AppError::invalid("path", "choose a file location"));
    }
    std::fs::write(&path, contents)?;
    Ok(())
}

/// Feeds the idle auto-lock timer (SEC-02).
#[tauri::command]
#[specta::specta]
pub fn activity_ping(state: State<'_, AppState>) {
    state.touch_activity();
}
