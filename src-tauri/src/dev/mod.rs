use crate::config::detect::resolve_project_root;
use crate::config::store::Project;
use crate::config::store::{get_projects, get_settings};
mod address;
mod group;
use crate::port::{find_listening_port, PortEntry};
use crate::process::job::ProcessJob;
use crate::process::kill_process_tree;
pub use group::{desktop_service, DevStatusEntry};
use serde::Serialize;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Runtime};

const LOG_CAP: usize = 2000;
// Process-only readiness reports a live process, not business readiness.
// A short stability window detects immediate failures without adding four seconds.
const STARTING_TIMEOUT_MS: u64 = 500;

#[derive(Clone, Copy, PartialEq, Eq)]
enum DevPhase {
    Starting,
    Running,
}

#[derive(Clone)]
pub struct DevManager(
    Arc<Mutex<DevState>>,
    Arc<Mutex<()>>,
    Arc<Mutex<group::GroupState>>,
);

struct DevState {
    sessions: HashMap<String, DevSession>,
    crashed_ids: HashSet<String>,
    archived_logs: HashMap<String, VecDeque<String>>,
    stopping: HashSet<String>,
}

struct DevSession {
    pid: u32,
    logs: VecDeque<String>,
    phase: DevPhase,
    job: Arc<ProcessJob>,
    output: Arc<crate::process::drain::RetainedOutput>,
    generation: u64,
    observed_port: Option<u16>,
    observed_url: Option<String>,
    pending_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartDevResult {
    pub ok: bool,
    pub message: Option<String>,
    pub conflict: Option<PortEntry>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DevLogEvent {
    id: String,
    service_id: String,
    session_id: u64,
    timestamp: u64,
    line: String,
}

impl DevManager {
    pub fn new() -> Self {
        Self(
            Arc::new(Mutex::new(DevState {
                sessions: HashMap::new(),
                crashed_ids: HashSet::new(),
                archived_logs: HashMap::new(),
                stopping: HashSet::new(),
            })),
            Arc::new(Mutex::new(())),
            Arc::new(Mutex::new(group::GroupState::default())),
        )
    }

    pub fn owned_pids(&self) -> HashSet<u32> {
        let state = self.0.lock().unwrap();
        state
            .sessions
            .values()
            .filter(|s| s.job.contains_pid(s.pid))
            .map(|s| s.pid)
            .collect()
    }

    pub fn service_owning_pid(&self, pid: u32) -> Option<(String, String)> {
        self.0
            .lock()
            .unwrap()
            .sessions
            .iter()
            .find(|(_, s)| s.job.contains_pid(pid))
            .map(|(key, _)| group::split_key(key))
    }
    #[cfg(test)]
    pub fn has_error(&self) -> bool {
        self.statuses()
            .iter()
            .any(|p| p.services.iter().any(|s| s.message.is_some()))
    }

    fn raw_logs(&self, id: &str) -> Vec<String> {
        let state = self.0.lock().unwrap();
        state
            .sessions
            .get(id)
            .map(|s| s.logs.iter().cloned().collect())
            .or_else(|| {
                state
                    .archived_logs
                    .get(id)
                    .map(|logs| logs.iter().cloned().collect())
            })
            .unwrap_or_default()
    }

    fn start_one<R: Runtime>(&self, app: &AppHandle<R>, project: &Project) -> StartDevResult {
        let _launch_guard = self.1.lock().unwrap();
        let id = project.id.clone();
        if self.is_stopping_all() {
            return StartDevResult {
                ok: false,
                message: Some("正在停止全部项目，请稍后启动".into()),
                conflict: None,
            };
        }

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

        // Only the explicit kill_and_restart action may terminate another process.
        if let Some(port) = project.port {
            match check_port_conflict(port, &self.owned_pids()) {
                Ok(Some(conflict)) => return port_occupied(port, conflict),
                Ok(None) => {}
                Err(e) => {
                    return StartDevResult {
                        ok: false,
                        message: Some(e),
                        conflict: None,
                    }
                }
            }
        }

        let settings = self.runtime_settings();
        let cwd = match resolve_project_root(&project.cwd) {
            Ok(cwd) => cwd,
            Err(e) => {
                return StartDevResult {
                    ok: false,
                    message: Some(e),
                    conflict: None,
                }
            }
        };
        let cwd_str = cwd.to_string_lossy().into_owned();
        let (shell_exe, shell_args) =
            build_shell_command(&settings.shell, &cwd_str, &project.command);

        let (child, job) = match spawn_managed_process(&shell_exe, &shell_args, &cwd) {
            Ok(c) => c,
            Err(e) => {
                return StartDevResult {
                    ok: false,
                    message: Some(e.to_string()),
                    conflict: None,
                };
            }
        };

        let output = match crate::process::drain::RetainedOutput::capture(&child) {
            Ok(output) => Arc::new(output),
            Err(error) => {
                let _ = job.terminate();
                return StartDevResult {
                    ok: false,
                    message: Some(format!("无法保留日志管道：{error}")),
                    conflict: None,
                };
            }
        };
        let pid = child.id();
        let generation = group::next_id();
        {
            let mut state = self.0.lock().unwrap();
            state.crashed_ids.remove(&id);
            state.archived_logs.remove(&id);
            state.sessions.insert(
                id.clone(),
                DevSession {
                    pid,
                    logs: VecDeque::new(),
                    phase: DevPhase::Starting,
                    job,
                    output,
                    generation,
                    observed_port: None,
                    observed_url: None,
                    pending_url: None,
                },
            );
        }

        spawn_log_reader(
            app.clone(),
            self.clone(),
            id.clone(),
            child,
            project.clone(),
            generation,
        );
        self.notify_status(app, &group::split_key(&id).0);
        crate::tray::refresh(app);

        StartDevResult {
            ok: true,
            message: None,
            conflict: None,
        }
    }

    fn stop_one<R: Runtime>(&self, app: &AppHandle<R>, id: &str) -> Result<(), String> {
        let launch_guard = self.1.lock().unwrap();
        let target = {
            let mut state = self.0.lock().unwrap();
            state.crashed_ids.remove(id);
            let target = state
                .sessions
                .get(id)
                .map(|s| (s.pid, s.generation, s.job.clone()));
            if target.is_some() {
                state.stopping.insert(id.to_string());
            }
            target
        };
        drop(launch_guard);
        if let Some((pid, generation, job)) = target {
            self.notify_status(app, &group::split_key(id).0);
            if let Err(e) = stop_job(&job) {
                let mut state = self.0.lock().unwrap();
                state.stopping.remove(id);
                if let Some(session) = state
                    .sessions
                    .get(id)
                    .filter(|s| s.pid == pid && s.generation == generation)
                {
                    let _ = session;
                    drop(state);
                    self.notify_status(app, &group::split_key(id).0);
                    return Err(e);
                }
                return Ok(());
            }
            cleanup_session(self, app, id, pid, generation, "stopped");
        } else {
            crate::tray::refresh(app);
        }
        Ok(())
    }

    pub fn kill_and_restart<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        project_id: &str,
        pid: u32,
        plan_id: Option<&str>,
    ) -> StartDevResult {
        let project = match get_projects()
            .ok()
            .and_then(|ps| ps.into_iter().find(|p| p.id == project_id))
        {
            Some(p) => p,
            None => {
                return StartDevResult {
                    ok: false,
                    message: Some("项目不存在".into()),
                    conflict: None,
                };
            }
        };

        if plan_id.is_some_and(|id| !project.launch_plans.iter().any(|p| p.id==id)) {
            return StartDevResult{ok:false,message:Some("启动方案不存在，未结束占用进程".into()),conflict:None};
        }
        let conflict = project
            .effective_services()
            .iter()
            .filter_map(|s| s.port)
            .find_map(|port| {
                check_port_conflict(port, &self.owned_pids())
                    .ok()
                    .flatten()
                    .filter(|owner| owner.pid == pid)
                    .map(|owner| (port, owner))
            });
        let Some((port, _)) = conflict else {
            return StartDevResult {
                ok: false,
                message: Some("端口占用已变化，请刷新后重试".into()),
                conflict: None,
            };
        };
        let result = if let Some((owner, service)) = self.service_owning_pid(pid) {
            self.stop_service(app, &owner, &service)
        } else {
            kill_process_tree(pid)
        };
        if let Err(message) = result {
            return StartDevResult {
                ok: false,
                message: Some(message),
                conflict: None,
            };
        }
        for _ in 0..12 {
            if matches!(check_port_conflict(port, &self.owned_pids()), Ok(None)) {
                break;
            }
            thread::sleep(Duration::from_millis(200));
        }

        if let Some(plan) = plan_id { self.start_plan(app,&project,plan) } else { self.start(app, &project) }
    }

    #[cfg(test)]
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
        // current_dir already sets the execution directory. Avoid a second cd
        // and preserve cmd's own quoting for paths, commands and batch files.
        (shell_exe, vec!["/D".into(), "/S".into(), "/C".into(), command.into()])
    } else {
        let script = format!(
            "$ErrorActionPreference = 'Stop'; Set-Location -LiteralPath '{}'; {}",
            escape_ps(cwd),
            command
        );
        (
            shell_exe,
            vec![
                "-NoProfile".into(),
                "-NonInteractive".into(),
                "-ExecutionPolicy".into(),
                "Bypass".into(),
                "-WindowStyle".into(),
                "Hidden".into(),
                "-Command".into(),
                script,
            ],
        )
    }
}

fn spawn_managed_process(
    shell_exe: &str,
    shell_args: &[String],
    cwd: &std::path::Path,
) -> Result<(std::process::Child, Arc<ProcessJob>), std::io::Error> {
    let job = Arc::new(ProcessJob::new()?);
    let mut command = Command::new(shell_exe);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let lower = shell_exe.to_ascii_lowercase();
        if (lower == "cmd" || lower.ends_with("cmd.exe")) && shell_args.len() == 4 && shell_args[1] == "/S" {
            command.args(&shell_args[..3]).raw_arg(format!("\"{}\"", shell_args[3]));
        } else {
            command.args(shell_args);
        }
    }
    #[cfg(not(windows))]
    command.args(shell_args);
    command
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // Attach to the Job before any project code may spawn descendants.
        command.creation_flags(0x0800_0000 | 0x0000_0004);
        crate::process::python_reload::configure(&mut command)?;
    }
    let mut child = command.spawn()?;
    if let Err(e) = job.attach_and_resume(&child) {
        let _ = child.kill();
        let _ = child.wait();
        return Err(e);
    }
    Ok((child, job))
}

fn stop_job(job: &ProcessJob) -> Result<(), String> {
    job.terminate().map_err(|e| e.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if job.active_count().map_err(|e| e.to_string())? == 0 {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err("进程仍在结束中，请稍后重试停止".into());
        }
        thread::sleep(Duration::from_millis(20));
    }
}

fn port_occupied(port: u16, conflict: PortEntry) -> StartDevResult {
    StartDevResult {
        ok: false,
        message: Some(format!("端口 :{port} 已被占用")),
        conflict: Some(conflict),
    }
}

fn check_port_conflict(port: u16, owned: &HashSet<u32>) -> Result<Option<PortEntry>, String> {
    find_listening_port(port, owned)
}

fn escape_ps(s: &str) -> String {
    s.replace('\'', "''")
}

fn read_output(reader: impl std::io::Read, mut on_line: impl FnMut(&str)) {
    let mut reader = BufReader::new(reader);
    let mut bytes = Vec::new();
    loop {
        bytes.clear();
        match reader.read_until(b'\n', &mut bytes) {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                while bytes.last().is_some_and(|v| matches!(*v, b'\n' | b'\r')) {
                    bytes.pop();
                }
                // Invalid legacy bytes must not stop draining a live service's pipe.
                on_line(&String::from_utf8_lossy(&bytes));
            }
        }
    }
}

fn spawn_log_reader<R: Runtime>(
    app: AppHandle<R>,
    manager: DevManager,
    id: String,
    mut child: std::process::Child,
    project: Project,
    generation: u64,
) {
    thread::spawn(move || {
        let session_pid = child.id();
        let write_logs = manager.runtime_settings().write_logs;

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let app_out = app.clone();
        let mgr_out = manager.clone();
        let id_out = id.clone();
        let proj_out = project.clone();

        let out_handle = stdout.map(|stdout| {
            thread::spawn(move || {
                read_output(stdout, |line| {
                    handle_log_line(
                        &app_out,
                        &mgr_out,
                        &id_out,
                        session_pid,
                        generation,
                        &proj_out,
                        line,
                        false,
                        write_logs,
                    );
                });
            })
        });

        let app_err = app.clone();
        let mgr_err = manager.clone();
        let id_err = id.clone();
        let proj_err = project.clone();

        let err_handle = stderr.map(|stderr| {
            thread::spawn(move || {
                read_output(stderr, |line| {
                    let tagged = format!("[stderr] {line}");
                    handle_log_line(
                        &app_err,
                        &mgr_err,
                        &id_err,
                        session_pid,
                        generation,
                        &proj_err,
                        &tagged,
                        true,
                        write_logs,
                    );
                });
            })
        });

        if let Some(h) = out_handle {
            let _ = h.join();
        }
        if let Some(h) = err_handle {
            let _ = h.join();
        }

        let exit_code = child.wait().ok().and_then(|s| s.code());
        let job = manager
            .0
            .lock()
            .unwrap()
            .sessions
            .get(&id)
            .filter(|s| s.generation == generation)
            .map(|s| s.job.clone());
        if let Some(job) = job {
            while job.active_count().unwrap_or(0) > 0 {
                thread::sleep(Duration::from_millis(100));
            }
        }
        let still_tracking = manager
            .0
            .lock()
            .unwrap()
            .sessions
            .get(&id)
            .is_some_and(|s| s.generation == generation);
        if still_tracking {
            let status = if exit_code == Some(0) {
                "stopped"
            } else {
                "crashed"
            };
            cleanup_session(&manager, &app, &id, session_pid, generation, status);
        }
    });
}

fn handle_log_line<R: Runtime>(
    app: &AppHandle<R>,
    manager: &DevManager,
    id: &str,
    pid: u32,
    generation: u64,
    _project: &Project,
    line: &str,
    is_stderr: bool,
    write_logs: bool,
) {
    if !manager
        .0
        .lock()
        .unwrap()
        .sessions
        .get(id)
        .is_some_and(|s| s.pid == pid && s.generation == generation)
    {
        return;
    }
    let stamped = format!("[{}] {line}", group::now_ms());
    if !push_log(manager, id, generation, &stamped, write_logs) {
        return;
    }
    let _ = is_stderr;
    // Logs can reveal the actual port when a dev server chooses a fallback.
    // A printed URL alone is not readiness evidence: its owner must join this Job.
    if let Some(url) = address::parse_log_url(line) {
        {
            let mut state = manager.0.lock().unwrap();
            if let Some(session) = state.sessions.get_mut(id).filter(|s| s.pid == pid && s.generation == generation) {
                session.pending_url = Some(url);
            }
        }
        manager.notify_status(app, &group::split_key(id).0);
        crate::tray::refresh(app);
    }
    let _ = app.emit(
        "dev-log",
        DevLogEvent {
            id: group::split_key(id).0,
            service_id: group::split_key(id).1,
            session_id: generation,
            timestamp: group::now_ms(),
            line: stamped,
        },
    );
}

fn promote_to_running<R: Runtime>(
    app: &AppHandle<R>,
    manager: &DevManager,
    id: &str,
    pid: u32,
    generation: u64,
) {
    let should_emit = {
        let mut state = manager.0.lock().unwrap();
        if state.stopping.contains(id) {
            return;
        }
        if let Some(session) = state.sessions.get_mut(id) {
            if session.pid == pid
                && session.generation == generation
                && session.phase == DevPhase::Starting
            {
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
        manager.notify_status(app, &group::split_key(id).0);
        crate::tray::refresh(app);
    }
}

fn push_log(manager: &DevManager, id: &str, generation: u64, line: &str, write_logs: bool) -> bool {
    let mut state = manager.0.lock().unwrap();
    if let Some(session) = state
        .sessions
        .get_mut(id)
        .filter(|s| s.generation == generation)
    {
        session.logs.push_back(line.to_string());
        while session.logs.len() > LOG_CAP {
            session.logs.pop_front();
        }
    } else {
        return false;
    }
    drop(state);
    if write_logs {
        let _ = append_log_file(id, line);
    }
    true
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

fn cleanup_session<R: Runtime>(
    manager: &DevManager,
    app: &AppHandle<R>,
    id: &str,
    pid: u32,
    generation: u64,
    status: &str,
) {
    {
        let mut state = manager.0.lock().unwrap();
        if !state
            .sessions
            .get(id)
            .is_some_and(|s| s.pid == pid && s.generation == generation)
        {
            return;
        }
        if let Some(session) = state.sessions.remove(id) {
            state.archived_logs.insert(id.to_string(), session.logs);
        }
        let stopping = state.stopping.remove(id);
        if status == "crashed" && !stopping {
            state.crashed_ids.insert(id.to_string());
        }
    }
    manager.notify_status(app, &group::split_key(id).0);
    crate::tray::refresh(app);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn native_job_stops_owned_shell_and_descendant_without_taskkill() {
        struct Guard {
            child: std::process::Child,
            job: Arc<ProcessJob>,
        }
        impl Drop for Guard {
            fn drop(&mut self) {
                let _ = self.job.terminate();
                let _ = self.child.kill();
                let _ = self.child.wait();
            }
        }
        for (shell, args) in [
            (
                "cmd.exe",
                vec![
                    "/D".into(),
                    "/C".into(),
                    "ping -n 31 127.0.0.1 > nul".into(),
                ],
            ),
            (
                "powershell.exe",
                vec![
                    "-NoProfile".into(),
                    "-NonInteractive".into(),
                    "-WindowStyle".into(),
                    "Hidden".into(),
                    "-Command".into(),
                    "ping.exe -n 31 127.0.0.1 | Out-Null".into(),
                ],
            ),
        ] {
            let (child, job) = spawn_managed_process(shell, &args, &std::env::temp_dir()).unwrap();
            let mut owned = Guard { child, job };
            let deadline = Instant::now() + Duration::from_secs(5);
            while owned.job.active_count().unwrap() < 2 && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(20));
            }
            assert!(
                owned.job.active_count().unwrap() >= 2,
                "shell and child must both join Job"
            );
            assert!(owned.job.contains_pid(owned.child.id()));
            assert!(!owned.job.contains_pid(std::process::id()));
            let external = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let owner = find_listening_port(external.local_addr().unwrap().port(), &HashSet::new())
                .unwrap()
                .unwrap();
            assert!(
                !owned.job.contains_pid(owner.pid),
                "external listener must not make a managed project ready"
            );
            let started = Instant::now();
            stop_job(&owned.job).unwrap();
            eprintln!("{shell} owned shell + child stop: {:?}", started.elapsed());
            assert_eq!(owned.job.active_count().unwrap(), 0);
            owned.child.wait().unwrap();
        }
    }

    #[test]
    fn detects_unhighlighted_occupied_port_without_killing_it() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        assert!(check_port_conflict(port, &HashSet::new())
            .unwrap()
            .is_some());
        assert!(std::net::TcpStream::connect(("127.0.0.1", port)).is_ok());
    }

    #[test]
    fn legacy_log_bytes_do_not_close_the_reader_or_drop_following_lines() {
        let input = b"\xff legacy\r\nnext line\n";
        let mut lines = Vec::new();
        read_output(&input[..], |line| lines.push(line.to_owned()));
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("legacy"));
        assert_eq!(lines[1], "next line");
    }

    #[test]
    fn shell_changes_directory_with_fail_fast_and_literal_quoting() {
        let (_, args) = build_shell_command("powershell", "D:\u{5c}O'Brien", "npm run dev");
        let script = args.last().unwrap();
        assert!(script.contains("$ErrorActionPreference = 'Stop'"));
        assert!(script.contains("O''Brien"));
        assert!(script.contains("-LiteralPath"));
    }

    #[test]
    fn logs_remain_available_after_session_exit() {
        let manager = DevManager::new();
        manager
            .0
            .lock()
            .unwrap()
            .archived_logs
            .insert("test".into(), VecDeque::from(["last error".into()]));
        assert_eq!(manager.raw_logs("test"), vec!["last error"]);
    }
}
