mod gauge;
mod menu_icons;
use crate::config::store::{get_projects, get_settings, Project};
use crate::dev::{DevManager, DevStatusEntry};
use gauge::Visual;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc, Mutex,
};
use std::time::{Duration, Instant};
use tauri::{
    image::Image,
    menu::{Menu, IconMenuItem, PredefinedMenuItem, Submenu},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, Runtime,
};
const TRAY_ID: &str = "quick-dev-tray";
#[derive(Clone, Copy, PartialEq, Eq)]
struct Motion {
    visual: Visual,
    size: u32,
    enabled: bool,
}
struct Controller {
    tx: mpsc::Sender<Option<Motion>>,
    exiting: AtomicBool,
    signature: Mutex<(u64, String)>,
}

pub fn setup<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(gauge::icon(icon_size(), Visual::Idle, 0))
        .tooltip("即栈 · 已停止")
        .show_menu_on_left_click(false)
        .on_menu_event(handle_menu_event)
        .on_tray_icon_event(|tray, event| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
            ) {
                show_main_window(tray.app_handle());
            }
        })
        .build(app)?;
    let (tx, rx) = mpsc::channel();
    app.manage(Controller {
        tx,
        exiting: AtomicBool::new(false),
        signature: Mutex::new((0, String::new())),
    });
    let app_handle = app.clone();
    std::thread::spawn(move || {
        let mut state = Motion {
            visual: Visual::Idle,
            size: icon_size(),
            enabled: false,
        };
        let mut frame = 0;
        let mut last_frame = Instant::now();
        // At most one main-thread icon update is queued. Slow UI work cannot
        // accumulate hundreds of stale animation callbacks.
        let pending: Arc<Mutex<(bool, Option<Image<'static>>)>> =
            Arc::new(Mutex::new((false, None)));
        loop {
            let moving = state.enabled
                && !(state.visual == Visual::Stopping && frame >= 16)
                && system_animation()
                && matches!(
                    state.visual,
                    Visual::Running | Visual::Starting | Visual::Stopping
                );
            let incoming = if moving {
                rx.recv_timeout(Duration::from_millis(75).saturating_sub(last_frame.elapsed()))
            } else {
                rx.recv().map_err(|_| mpsc::RecvTimeoutError::Disconnected)
            };
            match incoming {
                Ok(Some(next)) => {
                    if next == state {
                        continue;
                    }
                    state = next;
                    frame = 0;
                }
                Ok(None) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    frame = if state.visual == Visual::Stopping {
                        (frame + 1).min(16)
                    } else {
                        (frame + 1) % 32
                    }
                }
                Err(_) => break,
            }
            let animated = state.enabled && system_animation();
            let mut slot = pending.lock().unwrap();
            slot.1 = Some(gauge::icon(
                state.size,
                state.visual,
                if animated { frame } else { 0 },
            ));
            last_frame = Instant::now();
            if slot.0 {
                continue;
            }
            slot.0 = true;
            drop(slot);
            let pending = pending.clone();
            let handle = app_handle.clone();
            if app_handle
                .run_on_main_thread(move || {
                    let mut slot = pending.lock().unwrap();
                    let image = slot.1.take();
                    slot.0 = false;
                    drop(slot);
                    if let Some(tray) = handle.tray_by_id(TRAY_ID) {
                        let _ = tray.set_icon(image);
                    }
                })
                .is_err()
            {
                break;
            }
        }
    });
    refresh(app);
    Ok(())
}

pub fn shutdown<R: Runtime>(app: &AppHandle<R>) {
    if let Some(controller) = app.try_state::<Controller>() {
        let _ = controller.tx.send(None);
    }
}
pub fn refresh<R: Runtime>(app: &AppHandle<R>) {
    let Some(controller) = app.try_state::<Controller>() else {
        return;
    };
    let (revision, stopping_all, retaining_all, statuses) =
        app.state::<DevManager>().status_snapshot();
    let projects = get_projects().unwrap_or_default();
    let visual = if (stopping_all || retaining_all) && visual_for(&statuses) != Visual::Warning {
        Visual::Stopping
    } else {
        visual_for(&statuses)
    };
    let animation = get_settings().map(|s| s.tray_animation).unwrap_or(true);
    // Revisions and timestamps change on every event. Only menu-visible data
    // belongs in this signature; animation frames never rebuild menus.
    let signature=serde_json::to_string(&(stopping_all,retaining_all,projects.clone(),statuses.iter().map(|p|serde_json::json!({"id":p.id,"status":p.status,"services":p.services,"activePlanIds":p.active_plan_ids,"operation":p.operation.as_ref().map(|op|(&op.kind,&op.message))})).collect::<Vec<_>>())).unwrap_or_default();
    let mut old = controller.signature.lock().unwrap();
    if revision < old.0 {
        return;
    }
    let _ = controller.tx.send(Some(Motion {
        visual,
        size: icon_size(),
        enabled: animation,
    }));
    if old.1 == signature {
        old.0 = revision;
        return;
    }
    *old = (revision, signature.clone());
    drop(old);
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        if handle.state::<Controller>().signature.lock().unwrap().1 != signature {
            return;
        }
        if let Some(tray) = handle.tray_by_id(TRAY_ID) {
            if let Ok(menu) = build_menu(&handle, &projects, &statuses, stopping_all, retaining_all)
            {
                let _ = tray.set_menu(Some(menu));
            }
            let _ = tray.set_tooltip(Some(summary(&statuses)));
        }
    });
}
fn visual_for(statuses: &[DevStatusEntry]) -> Visual {
    if statuses.iter().any(|p| {
        p.services
            .iter()
            .any(|s| s.message.is_some() || matches!(s.status.as_str(), "failed" | "crashed"))
    }) {
        Visual::Warning
    } else if statuses.iter().any(|p| {
        p.operation
            .as_ref()
            .is_some_and(|op| matches!(op.kind.as_str(), "stop" | "rollback"))
            || p.status == "stopping"
    }) {
        Visual::Stopping
    } else if statuses
        .iter()
        .any(|p| p.operation.is_some() || p.status == "starting")
    {
        Visual::Starting
    } else if statuses
        .iter()
        .any(|p| p.services.iter().any(|s| s.status == "running"))
    {
        Visual::Running
    } else {
        Visual::Idle
    }
}
fn summary(statuses: &[DevStatusEntry]) -> String {
    let ready = statuses
        .iter()
        .flat_map(|p| &p.services)
        .filter(|s| s.status == "running" && s.readiness != "process")
        .count();
    let running = statuses
        .iter()
        .flat_map(|p| &p.services)
        .filter(|s| s.status == "running")
        .count();
    let pending = statuses.iter().filter(|p| p.operation.is_some()).count();
    let errors = statuses
        .iter()
        .flat_map(|p| &p.services)
        .filter(|s| s.message.is_some())
        .count();
    format!("即栈 · {} 个项目 · {running} 个服务运行 / {ready} 就绪 · {pending} 项处理中 · {errors} 个异常",statuses.len())
}
fn item<R: Runtime>(
    app: &AppHandle<R>,
    id: impl Into<String>,
    text: &str,
    enabled: bool,
) -> tauri::Result<IconMenuItem<R>> {
    let id = id.into();
    let image = menu_icons::icon(&id, text, icon_size());
    IconMenuItem::with_id(app, id, text.replace('&', "&&"), enabled, image, None::<&str>)
}
fn build_menu<R: Runtime>(
    app: &AppHandle<R>,
    projects: &[Project],
    statuses: &[DevStatusEntry],
    stopping_all: bool,
    retaining_all: bool,
) -> tauri::Result<Menu<R>> {
    let menu = Menu::new(app)?;
    let running = statuses.iter().flat_map(|p| &p.services).filter(|s| s.status == "running").count();
    let pending = statuses.iter().filter(|p| p.operation.is_some()).count();
    menu.append(&item(app, "summary", &format!("即栈 · {running} 个服务运行 · {pending} 项处理中"), false)?)?;
    menu.append(&item(app, "show", "打开面板", true)?)?;
    menu.append(&item(app, "add-project", "添加项目", true)?)?;
    menu.append(&item(app, "refresh-ports", "查看 / 刷新端口", true)?)?;
    menu.append(&item(app, "settings", "设置", true)?)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    if projects.is_empty() {
        menu.append(&item(app, "empty", "暂无项目，点击上方添加", false)?)?;
    }
    for project in projects {
        let status = statuses.iter().find(|s| s.id == project.id);
        let busy = status.and_then(|s| s.operation.as_ref());
        let live = status.is_some_and(|s| s.services.iter().any(|s| s.pid.is_some()));
        let sub = Submenu::new(
            app,
            format!(
                "{} · {}",
                menu_name(&project.name),
                status.map(|s| label(&s.status)).unwrap_or("已停止")
            ),
            true,
        )?;
        let services = project.effective_services();
        for service in &services {
            let state = status.and_then(|p| p.services.iter().find(|s| s.id == service.id));
            let desktop = desktop_service(service);
            if desktop || service.open_url.is_some() || service.port.is_some() {
                sub.append(&item(app, format!("open|{}|{}",project.id,service.id),
                    &format!("{} · {}",if desktop { "显示 App 窗口" } else { "打开网页" }, service.name),
                    state.is_some_and(|s| s.status == "running") && !retaining_all)?)?;
            }
        }
        sub.append(&item(app,format!("logs|{}",project.id),"查看项目日志",true)?)?;
        if let Some(operation) = busy {
            sub.append(&item(app,format!("progress|{}",project.id),&operation.message,false)?)?;
        }
        sub.append(&PredefinedMenuItem::separator(app)?)?;
        sub.append(&item(
            app,
            format!("start|{}", project.id),
            if project.default_plan_id.is_some() {
                "启动默认方案"
            } else if live {
                "补齐启动"
            } else {
                "启动全部服务"
            },
            busy.is_none() && !stopping_all && !retaining_all
                && services.iter().any(|service| !status.is_some_and(|p| p.services.iter().any(|s| s.id == service.id && s.status == "running"))),
        )?)?;
        for plan in &project.launch_plans {
            sub.append(&item(app,format!("plan_start|{}|{}",project.id,plan.id), &format!("启动 · {}",plan.name),busy.is_none() && !stopping_all && !retaining_all)?)?;
            if status.is_some_and(|s| s.active_plan_ids.contains(&plan.id)) {
                sub.append(&item(app,format!("plan_stop|{}|{}",project.id,plan.id), &format!("停止 · {}",plan.name),busy.is_none() && !stopping_all && !retaining_all)?)?;
            }
        }
        let stopping = busy.is_some_and(|op| matches!(op.kind.as_str(), "stop" | "rollback"));
        sub.append(&item(
            app,
            format!("stop|{}", project.id),
            if stopping {
                "停止中…"
            } else if busy.is_some() {
                "取消启动"
            } else {
                "停止项目"
            },
            (live || busy.is_some()) && !stopping && !stopping_all && !retaining_all,
        )?)?;
        let details = Submenu::new(app,"服务详情",true)?;
        for service in project.effective_services() {
            let state = status.and_then(|p| p.services.iter().find(|s| s.id == service.id));
            let ready = state.is_some_and(|s| s.status == "running");
            let svc_live = state.is_some_and(|s| s.pid.is_some());
            let svc = Submenu::new(
                app,
                format!(
                    "{} · {}",
                    menu_name(&service.name),
                    state.map(|s| label(&s.status)).unwrap_or("已停止")
                ),
                true,
            )?;
            if let Some(message) = state.and_then(|s| s.message.as_ref()) {
                svc.append(&item(
                    app,
                    format!("error|{}|{}", project.id, service.id),
                    message,
                    false,
                )?)?;
            }
            svc.append(&item(
                app,
                format!("start|{}|{}", project.id, service.id),
                if state.is_some_and(|s| s.message.is_some()) {
                    "重试启动"
                } else {
                    "启动"
                },
                !svc_live && busy.is_none() && !stopping_all && !retaining_all,
            )?)?;
            svc.append(&item(
                app,
                format!("stop|{}|{}", project.id, service.id),
                "停止",
                svc_live && busy.is_none() && !stopping_all && !retaining_all,
            )?)?;
            let desktop = desktop_service(&service);
            if desktop || service.open_url.is_some() || service.port.is_some() {
                svc.append(&item(
                    app,
                    format!("open|{}|{}", project.id, service.id),
                    if desktop { "显示 App 窗口" } else { "浏览器打开" },
                    ready && !retaining_all,
                )?)?;
            }
            svc.append(&item(
                app,
                format!("logs|{}|{}", project.id, service.id),
                "查看日志",
                true,
            )?)?;
            details.append(&svc)?;
        }
        sub.append(&PredefinedMenuItem::separator(app)?)?;
        sub.append(&details)?;
        menu.append(&sub)?;
    }
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&item(
        app,
        "stop-all",
        if stopping_all {
            "正在停止全部…"
        } else {
            "停止全部项目"
        },
        !stopping_all
            && !retaining_all
            && statuses
                .iter()
                .any(|p| p.pid.is_some() || p.operation.is_some()),
    )?)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&item(
        app,
        "quit-retain",
        if retaining_all {
            "正在退出并保留服务…"
        } else {
            "退出即栈（服务继续运行）"
        },
        !app.state::<DevManager>().has_operations(),
    )?)?;
    menu.append(&item(
        app,
        "quit-stop",
        "停止所有项目并退出",
        !stopping_all && !retaining_all,
    )?)?;
    Ok(menu)
}
fn menu_name(name: &str) -> String {
    let short: String = name.chars().take(48).collect();
    format!("{}{}",short.replace('&',"&&"),if name.chars().count() > 48 { "…" } else { "" })
}
fn desktop_service(service: &crate::config::store::ProjectService) -> bool {
    use crate::config::store::Readiness;
    service.readiness == Readiness::Window || (service.readiness == Readiness::Auto
        && crate::process::window::is_desktop_command(&service.command))
}
fn label(status: &str) -> &str {
    match status {
        "running" => "运行中",
        "starting" => "启动中…",
        "stopping" => "停止中…",
        "rollingBack" => "撤回启动中…",
        "partial" => "部分运行",
        "crashed" | "failed" => "异常",
        "conflict" => "端口冲突",
        _ => "已停止",
    }
}
fn handle_menu_event<R: Runtime>(app: &AppHandle<R>, event: tauri::menu::MenuEvent) {
    let id = event.id().as_ref();
    match id {
        "show" => show_main_window(app),
        "add-project" => {
            show_main_window(app);
            let _ = app.emit("open-project-form", ());
        }
        "refresh-ports" => {
            show_main_window(app);
            let _ = app.emit("navigate-tab", "ports");
            let _ = app.emit("trigger-refresh", ());
        }
        "settings" => {
            show_main_window(app);
            let _ = app.emit("navigate-tab", "settings");
        }
        "quit-retain" => quit_preserving_services(app),
        "stop-all" | "quit-stop" => {
            let exit = id == "quit-stop";
            let handle = app.clone();
            let manager = app.state::<DevManager>().inner().clone();
            tauri::async_runtime::spawn_blocking(move || {
                match manager.stop_all(&handle) {
                    Ok(()) if exit => handle.exit(0),
                    Ok(()) => {}
                    Err(error) => {
                        show_main_window(&handle);
                        let _ = handle.emit("operation-error", error);
                    }
                }
                let _ = handle.emit("trigger-refresh", ());
            });
        }
        _ => {
            let parts: Vec<_> = id.split('|').collect();
            if parts.len() < 2 {
                return;
            }
            let action = parts[0].to_owned();
            let project_id = parts[1].to_owned();
            let service_id = parts.get(2).map(|s| s.to_string());
            if action == "logs" {
                show_main_window(app);
                let _ = app.emit(
                    "open-service-logs",
                    serde_json::json!({"id":project_id,"serviceId":service_id}),
                );
                return;
            }
            if action == "open" {
                let desktop = app.state::<DevManager>().statuses().iter().any(|p| p.id == project_id && p.services.iter().any(|s| Some(&s.id) == service_id.as_ref() && s.readiness == "window"));
                if desktop {
                    if let Some(service) = service_id {
                        if let Err(error) = app.state::<DevManager>().focus_service(&project_id, &service) { let _ = app.emit("operation-error", error); }
                    }
                    return;
                }
                let ready = app.state::<DevManager>().statuses().iter().any(|p| {
                    p.id == project_id
                        && p.services
                            .iter()
                            .any(|s| Some(&s.id) == service_id.as_ref() && s.status == "running")
                });
                if ready {
                    if let Ok(projects) = get_projects() {
                        if let Some(service) =
                            projects.iter().find(|p| p.id == project_id).and_then(|p| {
                                p.effective_services()
                                    .into_iter()
                                    .find(|s| Some(&s.id) == service_id.as_ref())
                            })
                        {
                            if let Some(url) = service.open_url.or_else(|| {
                                service.port.map(|port| format!("http://localhost:{port}"))
                            }) {
                                use tauri_plugin_opener::OpenerExt;
                                let _ = app.opener().open_url(url, None::<&str>);
                            }
                        }
                    }
                }
                return;
            }
            let handle = app.clone();
            let manager = app.state::<DevManager>().inner().clone();
            tauri::async_runtime::spawn_blocking(move || {
                if action == "start" || action == "plan_start" {
                    match get_projects().and_then(|ps| {
                        ps.into_iter()
                            .find(|p| p.id == project_id)
                            .ok_or_else(|| "项目不存在".into())
                    }) {
                        Ok(project) => {
                            let result = if action == "plan_start" {
                                manager.start_plan(&handle,&project,service_id.as_deref().unwrap_or(""))
                            } else if let Some(service) = &service_id {
                                manager.start_service(&handle, &project, &service)
                            } else {
                                manager.start(&handle, &project)
                            };
                            if !result.ok {
                                show_main_window(&handle);
                                let _ = handle.emit(
                                    "dev-start-failed",
                                    serde_json::json!({"id":project.id,"result":result}),
                                );
                            } else if action == "plan_start" || service_id.is_none() {
                                let plan_id=if action=="plan_start" {service_id.as_deref()} else {project.default_plan_id.as_deref()};
                                if let Some(service)=project.launch_plans.iter().find(|p|Some(p.id.as_str())==plan_id).and_then(|p|p.open_service_id.as_ref()).and_then(|id|project.effective_services().into_iter().find(|s|&s.id==id)) {
                                    if service.readiness==crate::config::store::Readiness::Window || service.readiness==crate::config::store::Readiness::Auto && (service.kind.as_deref()==Some("desktop") || crate::process::window::is_desktop_command(&service.command)) { if let Err(error)=manager.focus_service(&project.id,&service.id) {let _=handle.emit("operation-error",error);} }
                                    else if let Some(url)=service.open_url.or_else(||service.port.map(|p|format!("http://localhost:{p}"))) { use tauri_plugin_opener::OpenerExt; let _=handle.opener().open_url(url,None::<&str>); }
                                }
                            }
                        }
                        Err(error) => {
                            let _ = handle.emit("operation-error", error);
                        }
                    }
                } else if action == "stop" || action == "plan_stop" {
                    let result = if action == "plan_stop" {
                        manager.stop_plan(&handle,&project_id,service_id.as_deref().unwrap_or(""))
                    } else if let Some(service) = service_id {
                        manager.stop_service(&handle, &project_id, &service)
                    } else {
                        manager.stop(&handle, &project_id)
                    };
                    if let Err(error) = result {
                        let _ = handle.emit("operation-error", error);
                    }
                }
                let _ = handle.emit("trigger-refresh", ());
            });
        }
    }
}
pub fn quit_preserving_services<R: Runtime>(app: &AppHandle<R>) {
    let manager = app.state::<DevManager>().inner().clone();
    if manager.has_operations() {
        show_main_window(app);
        let _ = app.emit(
            "operation-error",
            "项目仍在启停，请完成后保留服务退出；也可选择停止全部并退出",
        );
        return;
    }
    if app
        .state::<Controller>()
        .exiting
        .swap(true, Ordering::AcqRel)
    {
        return;
    }
    let _ = app.emit("exit-state", "正在退出并保留服务…");
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || match manager.preserve_outputs(&handle) {
        Ok(()) => handle.exit(0),
        Err(error) => {
            handle
                .state::<Controller>()
                .exiting
                .store(false, Ordering::Release);
            let _ = handle.emit("exit-state", "");
            show_main_window(&handle);
            let _ = handle.emit("operation-error", error);
        }
    });
}
pub fn show_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}
#[cfg(windows)]
fn icon_size() -> u32 {
    use windows_sys::Win32::UI::{HiDpi::GetDpiForWindow, WindowsAndMessaging::FindWindowW};
    let class: Vec<u16> = "Shell_TrayWnd\0".encode_utf16().collect();
    let dpi = unsafe { GetDpiForWindow(FindWindowW(class.as_ptr(), std::ptr::null())) };
    let target = 16 * dpi.max(96) / 96;
    gauge::SIZES
        .into_iter()
        .min_by_key(|size| size.abs_diff(target))
        .unwrap_or(16)
}
#[cfg(not(windows))]
fn icon_size() -> u32 {
    32
}
#[cfg(windows)]
fn system_animation() -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SystemParametersInfoW, SPI_GETCLIENTAREAANIMATION,
    };
    let mut enabled = 1i32;
    unsafe {
        SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            (&mut enabled as *mut i32).cast(),
            0,
        )
    };
    enabled != 0
}
#[cfg(not(windows))]
fn system_animation() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn warning_wins_over_busy_and_process_running_is_not_business_ready() {
        let manager = DevManager::new();
        let project: Project = serde_json::from_str(
            r#"{"id":"test","name":"测试","cwd":"D:/test","command":"python server.py"}"#,
        )
        .unwrap();
        manager.register_projects(&[project]);
        let mut statuses = manager.statuses();
        assert_eq!(visual_for(&statuses), Visual::Idle);
        statuses[0].services[0].status = "running".into();
        assert_eq!(visual_for(&statuses), Visual::Running);
        assert!(summary(&statuses).contains("1 个服务运行 / 0 就绪"));
        statuses[0].status = "starting".into();
        assert_eq!(visual_for(&statuses), Visual::Starting);
        statuses[0].services[0].message = Some("失败".into());
        assert_eq!(visual_for(&statuses), Visual::Warning);
        statuses[0].status = "stopping".into();
        assert_eq!(visual_for(&statuses), Visual::Warning);
    }
}
