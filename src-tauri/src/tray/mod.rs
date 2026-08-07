use crate::config::store::{get_projects, Project};
use crate::dev::DevManager;
use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, Runtime,
};

const TRAY_ID: &str = "quick-dev-tray";
const ICON_SIZE: u32 = 32;

#[derive(Clone, Copy, PartialEq, Eq)]
enum TrayVisual {
    Idle,
    Running,
    Error,
}

pub fn setup<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let menu = build_menu(app)?;
    let icon = icon_for(TrayVisual::Idle);

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .tooltip("即栈 Quick Dev")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(handle_menu_event)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

pub fn refresh<R: Runtime>(app: &AppHandle<R>) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        if let Ok(menu) = build_menu(app) {
            let _ = tray.set_menu(Some(menu));
        }

        let manager = app.state::<DevManager>();
        let n = manager.running_count();
        let visual = if manager.has_error() {
            TrayVisual::Error
        } else if n > 0 {
            TrayVisual::Running
        } else {
            TrayVisual::Idle
        };

        let _ = tray.set_icon(Some(icon_for(visual)));
        let tip = match visual {
            TrayVisual::Error => "即栈 · 有项目异常退出".into(),
            TrayVisual::Running => format!("即栈 · {n} 个项目运行中"),
            TrayVisual::Idle => "即栈 Quick Dev".into(),
        };
        let _ = tray.set_tooltip(Some(&tip));
    }
}

// 托盘图标：仪表盘 PNG（灰=空闲，绿=运行，黄=异常）
fn icon_for(state: TrayVisual) -> Image<'static> {
    let file = match state {
        TrayVisual::Idle => "tray-idle.png",
        TrayVisual::Running => "tray-running.png",
        TrayVisual::Error => "tray-warning.png",
    };
    load_tray_png(file)
}

fn load_tray_png(name: &str) -> Image<'static> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("icons")
        .join(name);
    let img = image::open(&path).unwrap_or_else(|e| panic!("tray icon {name}: {e}"));
    let resized = img.resize_exact(ICON_SIZE, ICON_SIZE, image::imageops::FilterType::Lanczos3);
    let rgba = resized.to_rgba8().into_raw();
    Image::new_owned(rgba, ICON_SIZE, ICON_SIZE)
}

fn build_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let manager = app.state::<DevManager>();
    let running: std::collections::HashSet<String> =
        manager.statuses().into_iter().filter(|s| s.status == "running").map(|s| s.id).collect();

    let menu = Menu::new(app)?;
    let show = MenuItem::with_id(app, "show", "打开面板", true, None::<&str>)?;
    let add = MenuItem::with_id(app, "add-project", "添加项目", true, None::<&str>)?;
    menu.append(&show)?;
    menu.append(&add)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;

    let projects = get_projects().unwrap_or_default();
    if projects.is_empty() {
        let empty = MenuItem::with_id(app, "no-project", "（暂无项目）", false, None::<&str>)?;
        menu.append(&empty)?;
    } else {
        for p in &projects {
            if running.contains(&p.id) {
                let port = p.port.map(|n| format!(" :{n}")).unwrap_or_default();
                let stop = MenuItem::with_id(
                    app,
                    format!("stop-{id}", id = p.id),
                    &format!("■ 停止 · {}{}", p.name, port),
                    true,
                    None::<&str>,
                )?;
                menu.append(&stop)?;
                if open_url_for(p).is_some() {
                    let open = MenuItem::with_id(
                        app,
                        format!("open-{id}", id = p.id),
                        &format!("↗ 浏览器打开 · {}", p.name),
                        true,
                        None::<&str>,
                    )?;
                    menu.append(&open)?;
                }
            } else {
                let start = MenuItem::with_id(
                    app,
                    format!("start-{id}", id = p.id),
                    &format!("▶ 启动 · {}", p.name),
                    true,
                    None::<&str>,
                )?;
                menu.append(&start)?;
            }
        }
    }

    menu.append(&PredefinedMenuItem::separator(app)?)?;
    let stop_all = MenuItem::with_id(app, "stop-all", "全部停止", true, None::<&str>)?;
    let refresh_ports = MenuItem::with_id(app, "refresh-ports", "刷新端口", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    menu.append(&stop_all)?;
    menu.append(&refresh_ports)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&quit)?;
    Ok(menu)
}

fn open_url_for(p: &Project) -> Option<String> {
    p.open_url
        .clone()
        .or_else(|| p.port.map(|port| format!("http://localhost:{port}")))
}

fn handle_menu_event<R: Runtime>(app: &AppHandle<R>, event: tauri::menu::MenuEvent) {
    let id = event.id().as_ref();
    let manager = app.state::<DevManager>();

    match id {
        "show" => show_main_window(app),
        "add-project" => {
            show_main_window(app);
            let _ = app.emit("open-project-form", ());
        }
        "stop-all" => {
            let _ = manager.stop_all(app);
            let _ = app.emit("trigger-refresh", ());
        }
        "refresh-ports" => {
            let _ = app.emit("trigger-refresh", ());
        }
        "quit" => app.exit(0),
        _ if id.starts_with("start-") => {
            let project_id = id.trim_start_matches("start-");
            if let Ok(projects) = get_projects() {
                if let Some(project) = projects.into_iter().find(|p| p.id == project_id) {
                    let _ = manager.start(app, &project);
                    let _ = app.emit("trigger-refresh", ());
                }
            }
        }
        _ if id.starts_with("stop-") => {
            let project_id = id.trim_start_matches("stop-");
            let _ = manager.stop(app, project_id);
            let _ = app.emit("trigger-refresh", ());
        }
        _ if id.starts_with("open-") => {
            let project_id = id.trim_start_matches("open-");
            if let Ok(projects) = get_projects() {
                if let Some(project) = projects.into_iter().find(|p| p.id == project_id) {
                    if let Some(url) = open_url_for(&project) {
                        use tauri_plugin_opener::OpenerExt;
                        let _ = app.opener().open_url(&url, None::<&str>);
                    }
                }
            }
        }
        _ => {}
    }
}

pub fn show_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}
