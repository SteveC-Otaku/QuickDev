mod autostart;
mod config;
mod dev;
mod hotkey;
mod port;
mod process;
mod tray;

use config::detect::{detect_project as detect_project_impl, DetectResult};
use config::store::{
    delete_project, export_all, get_projects, get_settings, import_all, save_project,
    save_settings, Project, Settings,
};
use dev::{DevManager, DevStatusEntry, StartDevResult};
use port::{PortEntry, PortSuggest};
use process::kill_process_tree;
use serde::Serialize;
use std::collections::{HashSet};
use tauri::{Manager, State, WindowEvent};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BootstrapPayload {
    settings: Settings,
    projects: Vec<Project>,
    statuses: Vec<DevStatusEntry>,
}

#[tauri::command]
fn bootstrap(manager: State<'_, DevManager>) -> Result<BootstrapPayload, String> {
    let projects = get_projects()?;
    manager.register_projects(&projects);
    Ok(BootstrapPayload {
        settings: get_settings()?,
        projects,
        statuses: manager.statuses(),
    })
}

#[tauri::command]
fn list_projects() -> Result<Vec<Project>, String> {
    get_projects()
}

#[tauri::command]
fn upsert_project(project: Project, app: tauri::AppHandle) -> Result<(), String> {
    let manager = app.state::<DevManager>();
    if manager.project_busy(&project.id) { return Err("请先停止项目，再修改配置".into()); }
    save_project(project)?;
    manager.register_projects(&get_projects()?);
    tray::refresh(&app);
    Ok(())
}

#[tauri::command]
async fn remove_project(id: String, app: tauri::AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let manager = app.state::<DevManager>();
        manager.stop(&app, &id)?;
        delete_project(&id)?;
        manager.register_projects(&get_projects()?);
        tray::refresh(&app);
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn load_settings() -> Result<Settings, String> {
    get_settings()
}

#[tauri::command]
fn open_data_directory(kind: String, app: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let root = config::store::data_dir()?;
    let path = match kind.as_str() {
        "config" => root,
        "logs" => root.join("logs"),
        _ => return Err("目录类型无效".into()),
    };
    std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    app.opener().open_path(path.to_string_lossy(), None::<&str>).map_err(|e| e.to_string())
}

#[tauri::command]
fn update_settings(settings: Settings, app: tauri::AppHandle) -> Result<(), String> {
    let previous = get_settings()?;
    let auto_start = settings.auto_start;
    save_settings(settings)?;
    if let Err(e) = hotkey::apply(&app).and_then(|_| autostart::apply(&app, auto_start)) {
        save_settings(previous.clone())?;
        let _ = hotkey::apply(&app);
        let _ = autostart::apply(&app, previous.auto_start);
        return Err(e);
    }
    tray::refresh(&app);
    Ok(())
}

#[tauri::command]
async fn detect_project(cwd: String) -> Result<DetectResult, String> {
    tauri::async_runtime::spawn_blocking(move || detect_project_impl(&cwd))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn suggest_projects() -> Result<Vec<PortSuggest>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let settings = get_settings().unwrap_or_default();
        let known: HashSet<String> = get_projects()?
            .into_iter()
            .map(|p| {
                p.cwd
                    .trim_end_matches(['\\', '/'])
                    .replace('/', "\\")
                    .to_lowercase()
            })
            .collect();
        port::suggest_from_ports(&settings.highlight_ports, &known)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn export_config() -> Result<String, String> {
    export_all()
}

fn manager_has_sessions(app: &tauri::AppHandle) -> bool {
    app.state::<DevManager>().has_sessions_or_operations()
}

#[tauri::command]
fn import_config(json: String, app: tauri::AppHandle) -> Result<(), String> {
    if manager_has_sessions(&app) {
        return Err("请先停止全部项目，再导入配置".into());
    }
    let previous = export_all()?;
    import_all(&json)?;
    let settings = get_settings()?;
    if let Err(e) = hotkey::apply(&app).and_then(|_| autostart::apply(&app, settings.auto_start)) {
        import_all(&previous)?;
        let _ = hotkey::apply(&app);
        let _ = autostart::apply(&app, get_settings()?.auto_start);
        return Err(e);
    }
    app.state::<DevManager>().register_projects(&get_projects()?);
    tray::refresh(&app);
    Ok(())
}

#[tauri::command]
async fn list_ports(manager: State<'_, DevManager>) -> Result<Vec<PortEntry>, String> {
    let manager = manager.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let settings = get_settings()?;
        let owned = manager.owned_pids();
        let running = manager.running_roots();

        port::list_listening_ports(&settings.highlight_ports, &owned, &running)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn kill_process(pid: u32, app: tauri::AppHandle, manager: State<'_, DevManager>) -> Result<(), String> {
    let manager=manager.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        if let Some((project,service))=manager.service_owning_pid(pid) {manager.stop_service(&app,&project,&service)} else {kill_process_tree(pid)}
    })
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn kill_and_restart(
    pid: u32,
    project_id: String,
    plan_id: Option<String>,
    app: tauri::AppHandle,
    manager: State<'_, DevManager>,
) -> Result<StartDevResult, String> {
    let manager = manager.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        Ok(manager.kill_and_restart(&app, &project_id, pid, plan_id.as_deref()))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn start_dev(
    id: String,
    plan_id: Option<String>,
    app: tauri::AppHandle,
    manager: State<'_, DevManager>,
) -> Result<StartDevResult, String> {
    let manager = manager.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let project = get_projects()?
            .into_iter()
            .find(|p| p.id == id)
            .ok_or_else(|| "项目不存在".to_string())?;
        Ok(if let Some(plan) = plan_id { manager.start_plan(&app, &project, &plan) } else { manager.start(&app, &project) })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn start_service(id: String, service_id: String, app: tauri::AppHandle, manager: State<'_, DevManager>) -> Result<StartDevResult, String> {
    let manager = manager.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let project = get_projects()?.into_iter().find(|p| p.id == id).ok_or("项目不存在")?;
        Ok(manager.start_service(&app, &project, &service_id))
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
async fn stop_service(id: String, service_id: String, app: tauri::AppHandle, manager: State<'_, DevManager>) -> Result<(), String> {
    let manager = manager.inner().clone();
    tauri::async_runtime::spawn_blocking(move || manager.stop_service(&app, &id, &service_id)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
fn focus_service(id: String, service_id: String, manager: State<'_, DevManager>) -> Result<(), String> {
    manager.focus_service(&id, &service_id)
}

#[tauri::command]
async fn stop_dev(
    id: String,
    plan_id: Option<String>,
    app: tauri::AppHandle,
    manager: State<'_, DevManager>,
) -> Result<(), String> {
    let manager = manager.inner().clone();
    tauri::async_runtime::spawn_blocking(move || if let Some(plan) = plan_id { manager.stop_plan(&app, &id, &plan) } else { manager.stop(&app, &id) })
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn stop_all_devs(
    app: tauri::AppHandle,
    manager: State<'_, DevManager>,
) -> Result<(), String> {
    let manager = manager.inner().clone();
    tauri::async_runtime::spawn_blocking(move || manager.stop_all(&app))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
fn list_dev_status(manager: State<'_, DevManager>) -> Result<Vec<DevStatusEntry>, String> {
    Ok(manager.statuses())
}

#[tauri::command]
fn get_dev_logs(id: String, service_id: Option<String>, manager: State<'_, DevManager>) -> Result<Vec<String>, String> {
    Ok(manager.logs(&id, service_id.as_deref()))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .manage(DevManager::new())
        .setup(|app| {
            app.state::<DevManager>().register_projects(&get_projects()?);
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
            if matches!(event, WindowEvent::ScaleFactorChanged { .. } | WindowEvent::ThemeChanged(_) | WindowEvent::Focused(true)) {
                tray::refresh(window.app_handle());
            }
            if let WindowEvent::CloseRequested { api, .. } = event {
                let hide = get_settings().map(|s| s.minimize_to_tray).unwrap_or(true);
                if hide {
                    let _ = window.hide();
                    api.prevent_close();
                } else {
                    api.prevent_close();
                    tray::quit_preserving_services(window.app_handle());
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            bootstrap,
            list_projects,
            upsert_project,
            remove_project,
            load_settings,
            open_data_directory,
            update_settings,
            list_ports,
            kill_process,
            kill_and_restart,
            start_dev,
            start_service,
            focus_service,
            stop_service,
            stop_dev,
            stop_all_devs,
            list_dev_status,
            get_dev_logs,
            detect_project,
            suggest_projects,
            export_config,
            import_config,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app,event| {if matches!(event,tauri::RunEvent::Exit) {tray::shutdown(app);}});
}

pub fn drain_preserved_logs() { process::drain::drain(); }
