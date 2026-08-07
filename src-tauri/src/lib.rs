mod autostart;
mod config;
mod dev;
mod hotkey;
mod port;
mod process;
mod tray;

use config::detect::{detect_project as detect_project_impl, DetectResult};
use config::store::{
    delete_project, export_all, get_projects, get_settings, import_all, save_project, save_settings,
    Project, Settings,
};
use dev::{DevManager, DevStatusEntry, StartDevResult};
use port::PortEntry;
use process::kill_process_tree;
use tauri::{Manager, State, WindowEvent};
use std::collections::HashMap;

#[tauri::command]
fn list_projects() -> Result<Vec<Project>, String> {
    get_projects()
}

#[tauri::command]
fn upsert_project(project: Project, app: tauri::AppHandle) -> Result<(), String> {
    save_project(project)?;
    tray::refresh(&app);
    Ok(())
}

#[tauri::command]
fn remove_project(id: String, app: tauri::AppHandle) -> Result<(), String> {
    let manager = app.state::<DevManager>();
    let _ = manager.stop(&app, &id);
    delete_project(&id)?;
    tray::refresh(&app);
    Ok(())
}

#[tauri::command]
fn load_settings() -> Result<Settings, String> {
    get_settings()
}

#[tauri::command]
fn update_settings(settings: Settings, app: tauri::AppHandle) -> Result<(), String> {
    let auto_start = settings.auto_start;
    save_settings(settings)?;
    hotkey::apply(&app)?;
    autostart::apply(&app, auto_start)?;
    Ok(())
}

#[tauri::command]
fn detect_project(cwd: String) -> Result<DetectResult, String> {
    detect_project_impl(&cwd)
}

#[tauri::command]
fn export_config() -> Result<String, String> {
    export_all()
}

#[tauri::command]
fn import_config(json: String, app: tauri::AppHandle) -> Result<(), String> {
    import_all(&json)?;
    tray::refresh(&app);
    Ok(())
}

#[tauri::command]
fn list_ports(manager: State<DevManager>) -> Result<Vec<PortEntry>, String> {
    let settings = get_settings()?;
    let owned = manager.owned_pids();
    let projects = get_projects()?;
    let statuses = manager.statuses();

    let session_pids: HashMap<String, u32> = statuses
        .into_iter()
        .filter(|s| s.status == "running" || s.status == "starting")
        .filter_map(|s| s.pid.map(|pid| (s.id, pid)))
        .collect();

    let running: Vec<port::RunningProject> = projects
        .into_iter()
        .filter_map(|p| {
            let root_pid = session_pids.get(&p.id).copied()?;
            Some(port::RunningProject {
                id: p.id,
                name: p.name,
                root_pid,
                port: p.port,
            })
        })
        .collect();

    port::list_listening_ports(&settings.highlight_ports, &owned, &running)
}

#[tauri::command]
fn kill_process(pid: u32) -> Result<(), String> {
    kill_process_tree(pid)
}

#[tauri::command]
fn start_dev(id: String, app: tauri::AppHandle, manager: State<DevManager>) -> Result<StartDevResult, String> {
    let project = get_projects()?
        .into_iter()
        .find(|p| p.id == id)
        .ok_or_else(|| "项目不存在".to_string())?;
    Ok(manager.start(&app, &project))
}

#[tauri::command]
fn stop_dev(id: String, app: tauri::AppHandle, manager: State<DevManager>) -> Result<(), String> {
    manager.stop(&app, &id)
}

#[tauri::command]
fn stop_all_devs(app: tauri::AppHandle, manager: State<DevManager>) -> Result<(), String> {
    manager.stop_all(&app)
}

#[tauri::command]
fn list_dev_status(manager: State<DevManager>) -> Result<Vec<DevStatusEntry>, String> {
    Ok(manager.statuses())
}

#[tauri::command]
fn get_dev_logs(id: String, manager: State<DevManager>) -> Result<Vec<String>, String> {
    Ok(manager.logs(&id))
}

#[tauri::command]
fn hide_to_tray(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .manage(DevManager::new())
        .setup(|app| {
            tray::setup(app.handle())?;
            if let Err(e) = hotkey::setup(app.handle()) {
                eprintln!("[quick-dev] 热键未注册: {e}");
            }
            if let Err(e) = autostart::setup(app.handle()) {
                eprintln!("[quick-dev] 开机自启同步跳过: {e}");
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let hide = get_settings()
                    .map(|s| s.minimize_to_tray)
                    .unwrap_or(true);
                if hide {
                    let _ = window.hide();
                    api.prevent_close();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            list_projects,
            upsert_project,
            remove_project,
            load_settings,
            update_settings,
            list_ports,
            kill_process,
            start_dev,
            stop_dev,
            stop_all_devs,
            list_dev_status,
            get_dev_logs,
            hide_to_tray,
            detect_project,
            export_config,
            import_config,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
