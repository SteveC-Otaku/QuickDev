use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt;

pub fn setup(app: &AppHandle) -> Result<(), String> {
    let enabled = crate::config::store::get_settings()
        .map(|s| s.auto_start)
        .unwrap_or(false);
    apply(app, enabled)
}

pub fn apply(app: &AppHandle, enabled: bool) -> Result<(), String> {
    let mgr = app.autolaunch();
    if enabled {
        mgr.enable().map_err(|e| e.to_string())
    } else {
        match mgr.disable() {
            Ok(()) => Ok(()),
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("找不到") || msg.contains("not found") || msg.contains("No such")
                {
                    Ok(())
                } else {
                    Err(msg)
                }
            }
        }
    }
}
