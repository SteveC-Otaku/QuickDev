use crate::config::detect::resolve_project_root;
use crate::config::store::Project;
use crate::config::store::{get_projects, get_settings, save_project};
use crate::port::{list_listening_ports, PortEntry};
use crate::process::kill_process_tree;
use serde::Serialize;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Runtime};

const LOG_CAP: usize = 2000;
const STARTING_TIMEOUT_MS: u64 = 4000;

#[derive(Clone, Copy, PartialEq, Eq)]
enum DevPhase {
    Starting,
    Running,
}

#[derive(Clone)]
pub struct DevManager(Arc<Mutex<DevState>>);

struct DevState {
    sessions: HashMap<String, DevSession>,
    crashed_ids: HashSet<String>,
    error_active: bool,
}

struct DevSession {
    pid: u32,
    logs: VecDeque<String>,
    phase: DevPhase,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DevStatusEntry {
    pub id: String,
    pub status: String,
    pub pid: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartDevResult {
    pub ok: bool,
    pub message: Option<String>,
    pub conflict: Option<PortEntry>,
}

#[derive(Clone, Serialize)]
struct DevLogEvent {
    id: String,
    line: String,
}

#[derive(Clone, Serialize)]
struct DevStatusEvent {
    id: String,
    status: String,
}

impl DevManager {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(DevState {
            sessions: HashMap::new(),
            crashed_ids: HashSet::new(),
            error_active: false,
        })))
    }

    pub fn owned_pids(&self) -> HashSet<u32> {
        let state = self.0.lock().unwrap();
        state.sessions.values().map(|s| s.pid).collect()
    }

    pub fn statuses(&self) -> Vec<DevStatusEntry> {
        let state = self.0.lock().unwrap();
        let mut out: Vec<DevStatusEntry> = state
            .sessions
            .iter()
            .map(|(id, s)| DevStatusEntry {
                id: id.clone(),
                status: match s.phase {
                    DevPhase::Starting => "starting".into(),
                    DevPhase::Running => "running".into(),
                },
                pid: Some(s.pid),
            })
            .collect();
        for id in &state.crashed_ids {
            if !state.sessions.contains_key(id) {
                out.push(DevStatusEntry {
                    id: id.clone(),
                    status: "crashed".into(),
                    pid: None,
                });
            }
        }
        out
    }

    pub fn has_error(&self) -> bool {
        self.0.lock().unwrap().error_active
    }

    pub fn logs(&self, id: &str) -> Vec<String> {
        let state = self.0.lock().unwrap();
        state
            .sessions
            .get(id)
            .map(|s| s.logs.iter().cloned().collect())
            .unwrap_or_default()
    }

    pub fn start<R: Runtime>(&self, app: &AppHandle<R>, project: &Project) -> StartDevResult {
        let id = project.id.clone();

        {
            let state = self.0.lock().unwrap();
            if state.sessions.contains_key(&id) {
                return StartDevResult {
                    ok: false,
                    message: Some("项目已在运行".into()),
                    conflict: None,
                };
            }
        }

        if let Some(port) = project.port {
            if let Some(conflict) = check_port_conflict(port, &self.owned_pids()) {
                return StartDevResult {
                    ok: false,
                    message: Some(format!("端口 :{port} 已被占用")),
                    conflict: Some(conflict),
                };
            }
        }

        let settings = get_settings().unwrap_or_default();
        let cwd = resolve_project_root(&project.cwd).unwrap_or_else(|_| PathBuf::from(&project.cwd));
        let cwd_str = cwd.to_string_lossy().into_owned();
        let (shell_exe, shell_args) = build_shell_command(&settings.shell, &cwd_str, &project.command);

        let _ = app.emit(
            "dev-status",
            DevStatusEvent {
                id: id.clone(),
                status: "starting".into(),
            },
        );

        let child = match spawn_dev_process(&shell_exe, &shell_args) {
            Ok(c) => c,
            Err(e) => {
                let _ = app.emit(
                    "dev-status",
                    DevStatusEvent {
                        id: id.clone(),
                        status: "stopped".into(),
                    },
                );
                return StartDevResult {
                    ok: false,
                    message: Some(e.to_string()),
                    conflict: None,
                };
            }
        };

        let pid = child.id();
        {
            let mut state = self.0.lock().unwrap();
            state.error_active = false;
            state.crashed_ids.remove(&id);
            state.sessions.insert(
                id.clone(),
                DevSession {
                    pid,
                    logs: VecDeque::new(),
                    phase: DevPhase::Starting,
                },
            );
        }

        spawn_log_reader(app.clone(), self.clone(), id.clone(), child, project.clone());
        crate::tray::refresh(app);

        StartDevResult {
            ok: true,
            message: None,
            conflict: None,
        }
    }

    pub fn stop<R: Runtime>(&self, app: &AppHandle<R>, id: &str) -> Result<(), String> {
        let pid = {
            let mut state = self.0.lock().unwrap();
            state.crashed_ids.remove(id);
            state.sessions.remove(id).map(|s| s.pid)
        };
        if let Some(pid) = pid {
            kill_process_tree(pid)?;
            let _ = app.emit(
                "dev-status",
                DevStatusEvent {
                    id: id.to_string(),
                    status: "stopped".into(),
                },
            );
            crate::tray::refresh(app);
        }
        Ok(())
    }

    pub fn stop_all<R: Runtime>(&self, app: &AppHandle<R>) -> Result<(), String> {
        let ids: Vec<String> = {
            let state = self.0.lock().unwrap();
            state.sessions.keys().cloned().collect()
        };
        for id in ids {
            self.stop(app, &id)?;
        }
        {
            let mut state = self.0.lock().unwrap();
            state.error_active = false;
        }
        Ok(())
    }

    pub fn running_count(&self) -> usize {
        self.0.lock().unwrap().sessions.len()
    }
}

fn build_shell_command(shell: &str, cwd: &str, command: &str) -> (String, Vec<String>) {
    let shell = shell.trim();
    let shell_exe = if shell.is_empty() {
        "powershell".to_string()
    } else {
        shell.to_string()
    };
    let lower = shell_exe.to_lowercase();

    if lower.ends_with("cmd.exe") || lower == "cmd" {
        let script = format!("cd /d \"{}\" && {}", cwd.replace('"', ""), command);
        (shell_exe, vec!["/C".into(), script])
    } else {
        let script = format!(
            "Set-Location -LiteralPath '{}'; {}",
            escape_ps(cwd),
            command
        );
        (
            shell_exe,
            vec![
                "-NoProfile".into(),
                "-NonInteractive".into(),
                "-WindowStyle".into(),
                "Hidden".into(),
                "-Command".into(),
                script,
            ],
        )
    }
}

fn spawn_dev_process(
    shell_exe: &str,
    shell_args: &[String],
) -> Result<std::process::Child, std::io::Error> {
    let mut cmd = Command::new(shell_exe);
    cmd.args(shell_args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd.spawn()
}

fn check_port_conflict(port: u16, owned: &HashSet<u32>) -> Option<PortEntry> {
    let entries = list_listening_ports(&[], owned, &[]).ok()?;
    entries
        .into_iter()
        .find(|e| e.port == port && e.owned != Some(true))
}

fn escape_ps(s: &str) -> String {
    s.replace('\'', "''")
}

fn spawn_log_reader<R: Runtime>(
    app: AppHandle<R>,
    manager: DevManager,
    id: String,
    mut child: std::process::Child,
    project: Project,
) {
    thread::spawn(move || {
        let write_logs = get_settings().map(|s| s.write_logs).unwrap_or(false);

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let app_out = app.clone();
        let mgr_out = manager.clone();
        let id_out = id.clone();
        let proj_out = project.clone();

        let out_handle = stdout.map(|stdout| {
            thread::spawn(move || {
                for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                    handle_log_line(
                        &app_out,
                        &mgr_out,
                        &id_out,
                        &proj_out,
                        &line,
                        false,
                        write_logs,
                    );
                }
            })
        });

        let app_err = app.clone();
        let mgr_err = manager.clone();
        let id_err = id.clone();
        let proj_err = project.clone();

        let err_handle = stderr.map(|stderr| {
            thread::spawn(move || {
                for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                    let tagged = format!("[stderr] {line}");
                    handle_log_line(
                        &app_err,
                        &mgr_err,
                        &id_err,
                        &proj_err,
                        &tagged,
                        true,
                        write_logs,
                    );
                }
            })
        });

        let app_timeout = app.clone();
        let mgr_timeout = manager.clone();
        let id_timeout = id.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(STARTING_TIMEOUT_MS));
            promote_to_running(&app_timeout, &mgr_timeout, &id_timeout);
        });

        if let Some(h) = out_handle {
            let _ = h.join();
        }
        if let Some(h) = err_handle {
            let _ = h.join();
        }

        let exit_code = child.wait().ok().and_then(|s| s.code());
        let still_tracking = manager.0.lock().unwrap().sessions.contains_key(&id);
        if still_tracking {
            let status = if exit_code == Some(0) { "stopped" } else { "crashed" };
            cleanup_session(&manager, &app, &id, status);
        }
    });
}

fn handle_log_line<R: Runtime>(
    app: &AppHandle<R>,
    manager: &DevManager,
    id: &str,
    project: &Project,
    line: &str,
    is_stderr: bool,
    write_logs: bool,
) {
    push_log(manager, id, line, write_logs);
    if !is_stderr {
        maybe_update_from_log(app, project, line);
    }
    promote_to_running(app, manager, id);
    let _ = app.emit(
        "dev-log",
        DevLogEvent {
            id: id.to_string(),
            line: line.to_string(),
        },
    );
}

fn promote_to_running<R: Runtime>(app: &AppHandle<R>, manager: &DevManager, id: &str) {
    let should_emit = {
        let mut state = manager.0.lock().unwrap();
        if let Some(session) = state.sessions.get_mut(id) {
            if session.phase == DevPhase::Starting {
                session.phase = DevPhase::Running;
                true
            } else {
                false
            }
        } else {
            false
        }
    };
    if should_emit {
        let _ = app.emit(
            "dev-status",
            DevStatusEvent {
                id: id.to_string(),
                status: "running".into(),
            },
        );
        crate::tray::refresh(app);
    }
}

fn push_log(manager: &DevManager, id: &str, line: &str, write_logs: bool) {
    let mut state = manager.0.lock().unwrap();
    if let Some(session) = state.sessions.get_mut(id) {
        session.logs.push_back(line.to_string());
        while session.logs.len() > LOG_CAP {
            session.logs.pop_front();
        }
    }
    drop(state);
    if write_logs {
        let _ = append_log_file(id, line);
    }
}

fn logs_dir() -> Result<PathBuf, String> {
    let base = std::env::var("LOCALAPPDATA").map_err(|e| e.to_string())?;
    let dir = PathBuf::from(base).join("QuickDev").join("logs");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn append_log_file(id: &str, line: &str) -> Result<(), String> {
    let path = logs_dir()?.join(format!("{id}.log"));
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| e.to_string())?;
    writeln!(file, "{line}").map_err(|e| e.to_string())
}

fn maybe_update_from_log<R: Runtime>(app: &AppHandle<R>, project: &Project, line: &str) {
    let Some(url) = parse_local_url(line) else {
        return;
    };
    let Ok(mut projects) = get_projects() else {
        return;
    };
    let Some(p) = projects.iter_mut().find(|p| p.id == project.id) else {
        return;
    };
    if p.open_url.as_deref() == Some(url.as_str()) {
        return;
    }
    p.open_url = Some(url.clone());
    if let Some(port) = port_from_url(&url) {
        p.port = Some(port);
    }
    let updated = p.clone();
    if save_project(updated).is_ok() {
        let _ = app.emit("trigger-refresh", ());
    }
}

fn parse_local_url(line: &str) -> Option<String> {
    let lower = line.to_lowercase();
    if !lower.contains("local:") {
        return None;
    }
    let start = lower.find("http://").or_else(|| lower.find("https://"))?;
    let raw: String = line[start..]
        .chars()
        .take_while(|c| !c.is_whitespace())
        .collect();
    let url = raw
        .trim_end_matches(|c: char| c == ')' || c == ']' || c == ',')
        .to_string();
    if url.starts_with("http://") || url.starts_with("https://") {
        Some(url)
    } else {
        None
    }
}

fn port_from_url(url: &str) -> Option<u16> {
    let after_scheme = url.split("://").nth(1)?;
    let host_port = after_scheme.split('/').next()?;
    host_port.rsplit(':').next()?.parse().ok()
}

fn cleanup_session<R: Runtime>(manager: &DevManager, app: &AppHandle<R>, id: &str, status: &str) {
    {
        let mut state = manager.0.lock().unwrap();
        state.sessions.remove(id);
        if status == "crashed" {
            state.crashed_ids.insert(id.to_string());
            state.error_active = true;
        }
    }
    let _ = app.emit(
        "dev-status",
        DevStatusEvent {
            id: id.to_string(),
            status: status.into(),
        },
    );
    crate::tray::refresh(app);
}
