//! Project transactions coordinate independent Jobs. Only Jobs created by this
//! transaction are rolled back; pre-existing services are never rollback targets.
use super::*;
use crate::config::store::{validate_project, ProjectService, Readiness, StartMode};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

static SEQUENCE: AtomicU64 = AtomicU64::new(1);
pub fn desktop_service(service: &ProjectService) -> bool {
    service.readiness == Readiness::Window || (service.readiness == Readiness::Auto && (service.kind.as_deref()==Some("desktop") || crate::process::window::is_desktop_command(&service.command)))
}
pub(super) fn next_id() -> u64 {
    SEQUENCE.fetch_add(1, Ordering::Relaxed)
}
pub(super) fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub fn service_key(project: &str, service: &str) -> String {
    format!("{}-{project}-{service}", project.len())
}
pub(super) fn split_key(key: &str) -> (String, String) {
    if let Some((length, rest)) = key.split_once('-') {
        if let Ok(length) = length.parse::<usize>() {
            if rest.is_char_boundary(length) && rest.as_bytes().get(length) == Some(&b'-') {
                return (rest[..length].into(), rest[length + 1..].into());
            }
        }
    }
    (key.into(), "default".into())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationSnapshot {
    pub id: u64,
    pub kind: String,
    pub started_at: u64,
    pub message: String,
}
#[derive(Clone)]
struct Operation {
    snapshot: OperationSnapshot,
    cancel: Arc<AtomicBool>,
}

#[derive(Default)]
pub(super) struct GroupState {
    #[cfg(test)]
    settings_override: Option<crate::config::store::Settings>,
    projects: HashMap<String, Project>,
    operations: HashMap<String, Operation>,
    failures: HashMap<String, String>,
    revision: u64,
    stopping_all: bool,
    retaining_all: bool,
    active_plans: HashMap<String, HashSet<String>>,
    manual_services: HashMap<String, u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceStatusEntry {
    pub id: String,
    pub name: String,
    pub status: String,
    pub pid: Option<u32>,
    pub session_id: Option<u64>,
    pub message: Option<String>,
    pub open_url: Option<String>,
    pub readiness: String,
}

#[derive(Debug, PartialEq)]
enum ServiceEntry { Window, Url(String) }
fn entry_from_status(service: &ServiceStatusEntry) -> Result<ServiceEntry, String> {
    if service.status != "running" { return Err(format!("{}尚未就绪，请查看日志或等待启动完成", service.name)); }
    if service.readiness == "window" { return Ok(ServiceEntry::Window); }
    service.open_url.clone().filter(|url| !url.trim().is_empty()).map(ServiceEntry::Url)
        .ok_or_else(|| format!("{}尚未配置网页地址，请填写端口或网页地址", service.name))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DevStatusEntry {
    pub id: String,
    pub status: String,
    pub pid: Option<u32>,
    pub services: Vec<ServiceStatusEntry>,
    pub operation: Option<OperationSnapshot>,
    pub revision: u64,
    pub active_plan_ids: Vec<String>,
}

pub fn aggregate_status(
    services: &[ServiceStatusEntry],
    operation: Option<&OperationSnapshot>,
) -> String {
    if let Some(op) = operation {
        return match op.kind.as_str() {
            "rollback" => "rollingBack",
            "stop" => "stopping",
            _ => "starting",
        }
        .into();
    }
    let running = services.iter().filter(|s| s.status == "running").count();
    let failed = services
        .iter()
        .any(|s| matches!(s.status.as_str(), "failed" | "crashed"));
    if running > 0 && (running < services.len() || failed) {
        "partial"
    } else if failed {
        "crashed"
    } else if services.iter().any(|s| s.status == "stopping") {
        "stopping"
    } else if services.iter().any(|s| s.status == "starting") {
        "starting"
    } else if running > 0 {
        "running"
    } else {
        "stopped"
    }
    .into()
}

impl DevManager {
    pub(super) fn runtime_settings(&self) -> crate::config::store::Settings {
        #[cfg(test)]
        if let Some(settings) = self.2.lock().unwrap().settings_override.clone() {
            return settings;
        }
        get_settings().unwrap_or_default()
    }
    pub fn has_operations(&self) -> bool {
        let groups = self.2.lock().unwrap();
        !groups.operations.is_empty() || groups.stopping_all || groups.retaining_all
    }
    pub fn preserve_outputs<R: Runtime>(&self, app: &AppHandle<R>) -> Result<(), String> {
        let outputs = {
            let mut groups = self.2.lock().unwrap();
            if groups.retaining_all || groups.stopping_all || !groups.operations.is_empty() {
                return Err("项目仍在启停，请完成后保留服务退出；也可选择停止全部并退出".into());
            }
            groups.retaining_all = true;
            groups.revision = next_id();
            self.0
                .lock()
                .unwrap()
                .sessions
                .values()
                .map(|s| s.output.clone())
                .collect::<Vec<_>>()
        };
        crate::tray::refresh(app);
        let mut helpers = Vec::new();
        for output in outputs {
            match output.preserve() {
                Ok(helper) => helpers.push(helper),
                Err(error) => {
                    for mut helper in helpers {
                        let _ = helper.kill();
                        let _ = helper.wait();
                    }
                    {
                        let mut groups = self.2.lock().unwrap();
                        groups.retaining_all = false;
                        groups.revision = next_id();
                    }
                    crate::tray::refresh(app);
                    return Err(format!("无法保留服务日志输出，应用未退出：{error}"));
                }
            }
        }
        Ok(())
    }
    pub fn is_stopping_all(&self) -> bool {
        self.2.lock().unwrap().stopping_all
    }
    pub fn has_sessions_or_operations(&self) -> bool {
        let groups = self.2.lock().unwrap();
        !groups.operations.is_empty() || !self.0.lock().unwrap().sessions.is_empty()
    }
    pub fn project_busy(&self, project: &str) -> bool {
        let groups = self.2.lock().unwrap();
        groups.operations.contains_key(project)
            || self
                .0
                .lock()
                .unwrap()
                .sessions
                .keys()
                .any(|key| split_key(key).0 == project)
    }
    pub fn register_projects(&self, projects: &[Project]) {
        let mut groups = self.2.lock().unwrap();
        groups.projects = projects
            .iter()
            .map(|p| (p.id.clone(), p.clone().normalize()))
            .collect();
        groups.revision = next_id();
    }

    // Keep a printed URL pending until its listener belongs to this session's Job.
    // OS queries run without either manager lock; generation checks reject stale results.
    fn refresh_observed_addresses(&self) {
        let pending: Vec<_> = self.0.lock().unwrap().sessions.iter().filter_map(|(key, session)| {
            session.pending_url.as_ref().map(|url| (key.clone(), session.generation, session.job.clone(), url.clone()))
        }).collect();
        let mut changed = false;
        let mut listeners = HashMap::new();
        for (key, generation, job, url) in pending {
            let Some(port) = address::port(&url) else { continue; };
            let pids = listeners.entry(port).or_insert_with(|| crate::port::listening_pids(port).unwrap_or_default());
            if !pids.iter().any(|pid| job.contains_pid(*pid)) { continue; }
            let mut state = self.0.lock().unwrap();
            if let Some(session) = state.sessions.get_mut(&key).filter(|s| s.generation == generation && s.pending_url.as_ref() == Some(&url)) {
                changed |= session.observed_url.as_ref() != Some(&url);
                session.observed_port = Some(port);
                session.observed_url = Some(url);
                session.pending_url = None;
            }
        }
        if changed { self.2.lock().unwrap().revision = next_id(); }
    }

    fn service_entry(&self, project: &str, service: &str) -> Result<ServiceEntry, String> {
        let status = self.statuses().into_iter().find(|p| p.id == project).ok_or("项目不存在，请刷新项目列表")?;
        let status = status.services.iter().find(|s| s.id == service).ok_or("服务不存在，请检查启动方案配置")?;
        entry_from_status(status)
    }

    pub fn open_service<R: Runtime>(&self, app: &AppHandle<R>, project: &str, service: &str) -> Result<(), String> {
        match self.service_entry(project, service)? {
            ServiceEntry::Window => self.focus_service(project, service),
            ServiceEntry::Url(url) => {
                use tauri_plugin_opener::OpenerExt;
                app.opener().open_url(url, None::<&str>).map_err(|error| format!("打开地址失败：{error}"))
            }
        }
    }

    pub fn open_plan_entry<R: Runtime>(&self, app: &AppHandle<R>, project: &str, plan: Option<&str>) -> Result<(), String> {
        let service = {
            let groups = self.2.lock().unwrap();
            let config = groups.projects.get(project).ok_or("项目不存在，请刷新项目列表")?;
            let plan = if let Some(id) = plan.or(config.default_plan_id.as_deref()) {
                config.launch_plans.iter().find(|p| p.id == id).ok_or("启动方案不存在")?
            } else if let Some(first) = config.launch_plans.first() { first } else { return Ok(()); };
            plan.open_service_id.clone()
        };
        if let Some(service) = service { self.open_service(app, project, &service)?; }
        Ok(())
    }

    pub fn statuses(&self) -> Vec<DevStatusEntry> {
        self.status_snapshot().3
    }
    pub fn status_snapshot(&self) -> (u64, bool, bool, Vec<DevStatusEntry>) {
        self.refresh_observed_addresses();
        let groups = self.2.lock().unwrap();
        let state = self.0.lock().unwrap();
        let mut out: Vec<_> = groups
            .projects
            .values()
            .map(|project| {
                let services: Vec<_> = project
                    .effective_services()
                    .iter()
                    .map(|service| {
                        let key = service_key(&project.id, &service.id);
                        let session = state.sessions.get(&key);
                        let message = groups.failures.get(&key).cloned().or_else(|| {
                            state
                                .crashed_ids
                                .contains(&key)
                                .then(|| "进程异常退出，请查看日志".into())
                        });
                        let status = if state.stopping.contains(&key) {
                            "stopping"
                        } else if let Some(session) = session {
                            if session.phase == DevPhase::Running {
                                "running"
                            } else {
                                "starting"
                            }
                        } else if message.is_some() {
                            "failed"
                        } else {
                            "stopped"
                        };
                        ServiceStatusEntry {
                            id: service.id.clone(),
                            name: service.name.clone(),
                            status: status.into(),
                            pid: session.map(|s| s.pid),
                            session_id: session.map(|s| s.generation),
                            message,
                            open_url: address::resolve(service.open_url.as_deref(), service.port, session.and_then(|s| s.observed_url.as_deref())),
                            readiness: if desktop_service(service) {
                                "window"
                            } else if service.readiness == Readiness::Http {
                                "http"
                            } else if (service.port.is_some()
                                || session.is_some_and(|s| s.observed_port.is_some()))
                                && service.readiness != Readiness::Process
                            {
                                "port"
                            } else {
                                "process"
                            }
                            .into(),
                        }
                    })
                    .collect();
                let operation = groups
                    .operations
                    .get(&project.id)
                    .map(|o| o.snapshot.clone());
                DevStatusEntry {
                    id: project.id.clone(),
                    status: aggregate_status(&services, operation.as_ref()),
                    pid: services.iter().find_map(|s| s.pid),
                    services,
                    operation,
                    revision: groups.revision,
                    active_plan_ids: groups.active_plans.get(&project.id).map(|ids| { let mut ids: Vec<_> = ids.iter().cloned().collect(); ids.sort(); ids }).unwrap_or_default(),
                }
            })
            .collect();
        out.sort_by(|a, b| a.id.cmp(&b.id));
        (
            groups.revision,
            groups.stopping_all,
            groups.retaining_all,
            out,
        )
    }

    pub(super) fn notify_status<R: Runtime>(&self, app: &AppHandle<R>, project: &str) {
        self.2.lock().unwrap().revision = next_id();
        if let Some(status) = self.statuses().into_iter().find(|s| s.id == project) {
            let _ = app.emit("dev-status", status);
        }
    }

    fn stage<R: Runtime>(&self, app: &AppHandle<R>, project: &str, kind: &str, message: &str) {
        if let Some(operation) = self.2.lock().unwrap().operations.get_mut(project) {
            operation.snapshot.kind = kind.into();
            operation.snapshot.message = message.into();
        }
        self.notify_status(app, project);
        crate::tray::refresh(app);
    }

    fn begin(&self, project: &Project) -> Result<Arc<AtomicBool>, String> {
        let mut groups = self.2.lock().unwrap();
        if groups.retaining_all {
            return Err("正在退出并保留服务，请稍候".into());
        }
        if groups.stopping_all {
            return Err("正在停止全部项目，请稍后启动".into());
        }
        if groups.operations.contains_key(&project.id) {
            return Err("项目正在操作，请稍后重试".into());
        }
        let cancel = Arc::new(AtomicBool::new(false));
        groups
            .projects
            .insert(project.id.clone(), project.clone().normalize());
        groups.operations.insert(
            project.id.clone(),
            Operation {
                snapshot: OperationSnapshot {
                    id: next_id(),
                    kind: "start".into(),
                    started_at: now_ms(),
                    message: "检查启动环境…".into(),
                },
                cancel: cancel.clone(),
            },
        );
        Ok(cancel)
    }

    pub fn start<R: Runtime>(&self, app: &AppHandle<R>, project: &Project) -> StartDevResult {
        if let Some(id) = project.default_plan_id.as_ref().or_else(||project.launch_plans.first().map(|p|&p.id)) { return self.start_plan(app, project, id); }
        self.start_selected(app, project, None, project.start_mode, None)
    }

    pub fn start_plan<R: Runtime>(&self, app: &AppHandle<R>, project: &Project, plan_id: &str) -> StartDevResult {
        let Some(plan) = project.launch_plans.iter().find(|p| p.id == plan_id) else { return failure("启动方案不存在".into(), None); };
        self.start_selected(app, project, Some(&plan.service_ids), plan.start_mode, Some(plan_id))
    }

    pub fn start_service<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        project: &Project,
        service_id: &str,
    ) -> StartDevResult {
        self.start_selected(app, project, Some(&[service_id.to_owned()]), project.start_mode, None)
    }

    fn start_selected<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        project: &Project,
        selected: Option<&[String]>,
        mode: StartMode,
        plan_id: Option<&str>,
    ) -> StartDevResult {
        if let Err(error) = validate_project(project) {
            return failure(error, None);
        }
        let all = project.effective_services();
        let services: Vec<_> = if let Some(ids) = selected {
            let mut services = Vec::new();
            for id in ids { let Some(service) = all.iter().find(|s| &s.id == id) else { return failure("服务不存在".into(), None); }; services.push(service.clone()); }
            services
        } else { all };
        if services.is_empty() {
            return failure("服务不存在".into(), None);
        }
        let cancel = match self.begin(project) {
            Ok(cancel) => cancel,
            Err(e) => return failure(e, None),
        };
        self.notify_status(app, &project.id);
        crate::tray::refresh(app);
        let keys: Vec<_> = services
            .iter()
            .map(|s| service_key(&project.id, &s.id))
            .collect();
        let preexisting: HashSet<_> = self.0.lock().unwrap().sessions.keys().cloned().collect();
        // A generation prevents even a replacement of the same service from
        // accidentally becoming a rollback target.
        let created = Mutex::new(Vec::<(String, u64)>::new());
        let failed = Mutex::new(None::<StartDevResult>);
        for key in &keys {
            self.2.lock().unwrap().failures.remove(key);
        }
        let launch = |service: &ProjectService| -> Result<(), StartDevResult> {
            let key = service_key(&project.id, &service.id);
            if preexisting.contains(&key) {
                let generation = self
                    .0
                    .lock()
                    .unwrap()
                    .sessions
                    .get(&key)
                    .map(|s| s.generation)
                    .ok_or_else(|| failure("原有服务已退出".into(), None))?;
                return self
                    .await_ready(app, &key, service, generation, &cancel)
                    .map_err(|e| failure(format!("{}：{e}", service.name), None));
            }
            if cancel.load(Ordering::Acquire) {
                return Err(failure("启动已取消".into(), None));
            }
            if let Err(e) = preflight(service) {
                return Err(failure(format!("{}：{e}", service.name), None));
            }
            let single = service_project(project, service);
            let result = self.start_one(app, &single);
            if !result.ok {
                return Err(failure(
                    format!(
                        "{}：{}",
                        service.name,
                        result.message.as_deref().unwrap_or("启动失败")
                    ),
                    result.conflict,
                ));
            }
            let generation = self
                .0
                .lock()
                .unwrap()
                .sessions
                .get(&key)
                .map(|s| s.generation);
            let Some(generation) = generation else {
                return Err(failure(
                    format!("{}：进程启动后立即退出，请查看日志", service.name),
                    None,
                ));
            };
            created.lock().unwrap().push((key.clone(), generation));
            self.await_ready(app, &key, service, generation, &cancel)
                .map_err(|e| failure(format!("{}：{e}", service.name), None))
        };
        let record = |service: &ProjectService, result: Result<(), StartDevResult>| {
            if let Err(error) = result {
                let mut first = failed.lock().unwrap();
                if first.is_none() {
                    self.2.lock().unwrap().failures.insert(
                        service_key(&project.id, &service.id),
                        error.message.clone().unwrap_or_default(),
                    );
                    *first = Some(error);
                }
                cancel.store(true, Ordering::Release);
            }
        };
        self.stage(app, &project.id, "start", "启动服务，等待就绪…");
        if mode == StartMode::Sequential {
            for service in &services {
                self.stage(app, &project.id, "start", &format!("正在启动 {}，等待就绪…", service.name));
                record(service, launch(service));
                if cancel.load(Ordering::Acquire) {
                    break;
                }
            }
        } else {
            thread::scope(|scope| {
                for service in &services {
                    let launch = &launch;
                    let record = &record;
                    scope.spawn(move || record(service, launch(service)));
                }
            });
        }
        let error = failed.into_inner().unwrap();
        let cancelled = cancel.load(Ordering::Acquire);
        let mut rollback_errors = Vec::new();
        if error.is_some() || cancelled {
            self.stage(app, &project.id, "rollback", "启动未完成，撤回本次启动…");
            let created = created.into_inner().unwrap();
            for (key, generation) in created {
                if self
                    .0
                    .lock()
                    .unwrap()
                    .sessions
                    .get(&key)
                    .is_some_and(|s| s.generation == generation)
                {
                    if let Err(e) = self.stop_one(app, &key) {
                        let message = format!("回退未完成：{e}");
                        self.2.lock().unwrap().failures.insert(key, message.clone());
                        rollback_errors.push(message);
                    }
                }
            }
        }
        {
            let mut groups = self.2.lock().unwrap();
            if error.is_none() && !cancelled {
                if let Some(id) = plan_id { groups.active_plans.entry(project.id.clone()).or_default().insert(id.into()); }
                else {
                    let state=self.0.lock().unwrap();
                    for key in &keys { if let Some(session)=state.sessions.get(key) {groups.manual_services.insert(key.clone(),session.generation);} }
                }
            }
            groups.operations.remove(&project.id);
        }
        self.notify_status(app, &project.id);
        crate::tray::refresh(app);
        if !rollback_errors.is_empty() {
            return failure(
                format!(
                    "{}；{}",
                    error
                        .as_ref()
                        .and_then(|e| e.message.as_deref())
                        .unwrap_or("启动已取消"),
                    rollback_errors.join("；")
                ),
                error.and_then(|e| e.conflict),
            );
        }
        error.unwrap_or_else(|| {
            if cancelled {
                failure("启动已取消，本次新启动的服务已停止".into(), None)
            } else {
                StartDevResult {
                    ok: true,
                    message: None,
                    conflict: None,
                }
            }
        })
    }

    fn await_ready<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        key: &str,
        service: &ProjectService,
        generation: u64,
        cancel: &AtomicBool,
    ) -> Result<(), String> {
        let started = Instant::now();
        let client = if service.readiness == Readiness::Http {
            Some(
                reqwest::blocking::Client::builder()
                    .no_proxy()
                    .redirect(reqwest::redirect::Policy::none())
                    .timeout(Duration::from_millis(750))
                    .build()
                    .map_err(|e| e.to_string())?,
            )
        } else {
            None
        };
        loop {
            if cancel.load(Ordering::Acquire) {
                return Err("启动已取消".into());
            }
            self.refresh_observed_addresses();
            let (pid, job, observed_port, phase) = {
                let state = self.0.lock().unwrap();
                let Some(session) = state
                    .sessions
                    .get(key)
                    .filter(|s| s.generation == generation)
                else {
                    return Err("进程已退出，请查看日志".into());
                };
                (
                    session.pid,
                    session.job.clone(),
                    session.observed_port,
                    session.phase,
                )
            };
            if phase == DevPhase::Running {
                return Ok(());
            }
            if job.active_count().map_err(|e| e.to_string())? == 0 {
                return Err("进程已退出，请查看日志".into());
            }
            let port = if service.readiness == Readiness::Http {
                service
                    .health_url
                    .as_deref()
                    .and_then(|url| url.parse::<tauri::Url>().ok())
                    .and_then(|url| url.port_or_known_default())
            } else if service.readiness == Readiness::Process || desktop_service(service) {
                None
            } else {
                service.port.or(observed_port)
            };
            let owned_listener = port.is_some_and(|port| {
                crate::port::listening_pids(port)
                    .ok()
                    .is_some_and(|pids| pids.into_iter().any(|pid| job.contains_pid(pid)))
            });
            let ready = if desktop_service(service) {
                crate::process::window::visible_window(&job).is_some()
            } else if let Some(client) = &client {
                owned_listener
                    && service.health_url.as_ref().is_some_and(|url| {
                        client
                            .get(url)
                            .send()
                            .is_ok_and(|r| r.status().is_success())
                    })
            } else if port.is_some() {
                owned_listener
            } else {
                started.elapsed() >= Duration::from_millis(STARTING_TIMEOUT_MS)
            };
            if ready {
                promote_to_running(app, self, key, pid, generation);
                return Ok(());
            }
            if started.elapsed() >= Duration::from_secs(service.startup_timeout_secs) {
                return Err(format!(
                    "等待就绪超时（{} 秒），请检查日志和就绪设置",
                    service.startup_timeout_secs
                ));
            }
            thread::sleep(Duration::from_millis(150));
        }
    }

    pub fn stop<R: Runtime>(&self, app: &AppHandle<R>, project: &str) -> Result<(), String> {
        self.stop_selected(app, project, None, None)
    }

    pub fn stop_plan<R: Runtime>(&self, app: &AppHandle<R>, project: &str, plan: &str) -> Result<(), String> {
        self.stop_selected(app, project, None, Some(plan))
    }

    pub fn focus_service(&self, project: &str, service: &str) -> Result<(), String> {
        let job = self.0.lock().unwrap().sessions.get(&service_key(project, service))
            .map(|s| s.job.clone()).ok_or("服务未运行")?;
        crate::process::window::focus(&job)
    }
    pub fn stop_service<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        project: &str,
        service: &str,
    ) -> Result<(), String> {
        self.stop_selected(app, project, Some(service), None)
    }

    fn stop_selected<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        project: &str,
        selected: Option<&str>,
        plan_id: Option<&str>,
    ) -> Result<(), String> {
        // Claim the stop operation atomically after startup cancellation. A
        // second caller cannot replace the first stop's operation token.
        let (config, plan_services) = loop {
            {
                let mut groups = self.2.lock().unwrap();
                if groups.retaining_all {
                    return Err("正在退出并保留服务，请稍候".into());
                }
                if let Some(operation) = groups.operations.get(project) {
                    if operation.snapshot.kind == "stop" {
                        return Err("项目正在停止，请稍后重试".into());
                    }
                    operation.cancel.store(true, Ordering::Release);
                } else {
                    let config = groups.projects.get(project).cloned().ok_or("项目不存在")?;
                    {
                        let state=self.0.lock().unwrap();
                        groups.manual_services.retain(|key,generation|state.sessions.get(key).is_some_and(|session|session.generation==*generation));
                    }
                    let plan_services = if let Some(id) = plan_id {
                        let plan = config.launch_plans.iter().find(|p| p.id == id).ok_or("启动方案不存在")?;
                        let active = groups.active_plans.get(project);
                        let keep: HashSet<_> = config.launch_plans.iter().filter(|p| p.id != id && active.is_some_and(|ids| ids.contains(&p.id))).flat_map(|p| p.service_ids.iter().cloned()).collect();
                        Some(plan.service_ids.iter().filter(|service| !keep.contains(*service) && !groups.manual_services.contains_key(&service_key(project, service))).cloned().collect::<Vec<_>>())
                    } else { None };
                    if selected
                        .is_some_and(|id| !config.effective_services().iter().any(|s| s.id == id))
                    {
                        return Err("服务不存在".into());
                    }
                    groups.operations.insert(
                        project.into(),
                        Operation {
                            snapshot: OperationSnapshot {
                                id: next_id(),
                                kind: "stop".into(),
                                started_at: now_ms(),
                                message: "停止服务…".into(),
                            },
                            cancel: Arc::new(AtomicBool::new(false)),
                        },
                    );
                    break (config, plan_services);
                }
            }
            thread::sleep(Duration::from_millis(25));
        };
        self.notify_status(app, project);
        crate::tray::refresh(app);
        let services: Vec<_> = config
            .effective_services()
            .into_iter()
            .filter(|s| selected.is_none_or(|id| s.id == id))
            .filter(|s| plan_services.as_ref().is_none_or(|ids| ids.contains(&s.id)))
            .collect();
        let errors = Mutex::new(Vec::<String>::new());
        thread::scope(|scope| {
            for service in &services {
                let errors = &errors;
                scope.spawn(move || {
                    let key = service_key(project, &service.id);
                    match self.stop_one(app, &key) {
                        Ok(()) => {
                            self.2.lock().unwrap().failures.remove(&key);
                        }
                        Err(e) => {
                            self.2.lock().unwrap().failures.insert(key, e.clone());
                            errors
                                .lock()
                                .unwrap()
                                .push(format!("{}：{e}", service.name));
                        }
                    }
                });
            }
        });
        {
            let mut groups = self.2.lock().unwrap();
            if errors.lock().unwrap().is_empty() {
                if let Some(id) = plan_id { if let Some(ids) = groups.active_plans.get_mut(project) { ids.remove(id); } }
                else {
                    for service in &services { groups.manual_services.remove(&service_key(project, &service.id)); }
                    if selected.is_none() { groups.active_plans.remove(project); }
                }
            }
            groups.operations.remove(project);
        }
        self.notify_status(app, project);
        crate::tray::refresh(app);
        let errors = errors.into_inner().unwrap();
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("；"))
        }
    }

    pub fn stop_all<R: Runtime>(&self, app: &AppHandle<R>) -> Result<(), String> {
        let ids = {
            let mut groups = self.2.lock().unwrap();
            if groups.retaining_all {
                return Err("正在退出并保留服务，请稍候".into());
            }
            if groups.stopping_all {
                return Err("正在停止全部项目".into());
            }
            groups.stopping_all = true;
            groups.revision = next_id();
            for operation in groups.operations.values() {
                operation.cancel.store(true, Ordering::Release);
            }
            groups.projects.keys().cloned().collect::<Vec<_>>()
        };
        crate::tray::refresh(app);
        let errors = Mutex::new(Vec::new());
        for batch in ids.chunks(8) {
            thread::scope(|scope| {
                for id in batch {
                    let errors = &errors;
                    scope.spawn(move || {
                        while self
                            .2
                            .lock()
                            .unwrap()
                            .operations
                            .get(id)
                            .is_some_and(|op| op.snapshot.kind == "stop")
                        {
                            thread::sleep(Duration::from_millis(25));
                        }
                        if let Err(e) = self.stop(app, id) {
                            errors.lock().unwrap().push(e);
                        }
                    });
                }
            });
        }
        {
            let mut groups = self.2.lock().unwrap();
            groups.stopping_all = false;
            groups.revision = next_id();
        }
        crate::tray::refresh(app);
        let errors = errors.into_inner().unwrap();
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("；"))
        }
    }

    pub fn logs(&self, project: &str, service: Option<&str>) -> Vec<String> {
        let config = self.2.lock().unwrap().projects.get(project).cloned();
        let Some(config) = config else {
            return Vec::new();
        };
        let mut lines = config
            .effective_services()
            .iter()
            .filter(|s| service.is_none_or(|id| s.id == id))
            .flat_map(|s| {
                self.raw_logs(&service_key(project, &s.id))
                    .into_iter()
                    .map(|line| {
                        let stamp = line
                            .strip_prefix('[')
                            .and_then(|v| v.split_once(']'))
                            .and_then(|(v, _)| v.parse::<u64>().ok())
                            .unwrap_or(0);
                        let text = if service.is_none() {
                            format!("[{}] {line}", s.name)
                        } else {
                            line
                        };
                        (stamp, text)
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        lines.sort_by_key(|(stamp, _)| *stamp);
        lines
            .into_iter()
            .rev()
            .take(LOG_CAP)
            .map(|(_, line)| line)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    }

    pub fn running_roots(&self) -> Vec<crate::port::RunningProject> {
        let groups = self.2.lock().unwrap();
        let state = self.0.lock().unwrap();
        state
            .sessions
            .iter()
            .filter_map(|(key, s)| {
                let (id, service_id) = split_key(key);
                let project = groups.projects.get(&id)?;
                let service = project.services.iter().find(|s| s.id == service_id)?;
                Some(crate::port::RunningProject {
                    id,
                    name: format!("{} · {}", project.name, service.name),
                    root_pid: s.pid,
                    job: s.job.clone(),
                    service_id,
                })
            })
            .collect()
    }
}

fn failure(message: String, conflict: Option<PortEntry>) -> StartDevResult {
    StartDevResult {
        ok: false,
        message: Some(message),
        conflict,
    }
}
fn service_project(project: &Project, service: &ProjectService) -> Project {
    Project {
        id: service_key(&project.id, &service.id),
        name: format!("{} · {}", project.name, service.name),
        cwd: service.cwd.clone(),
        command: service.command.clone(),
        port: service.port,
        open_url: service.open_url.clone(),
        tags: vec![],
        group: project.group.clone(),
        services: vec![],
        start_mode: StartMode::Parallel, launch_plans: vec![], default_plan_id: None,
    }
}

fn tool_exists(tool: &str, cwd: &std::path::Path) -> bool {
    let tool = tool.trim_matches(['\'', '"']);
    let extensions = std::env::var("PATHEXT").unwrap_or_else(|_| ".EXE;.CMD;.BAT;.COM".into());
    let extensions: Vec<_> = std::iter::once("").chain(extensions.split(';')).collect();
    let path = std::path::Path::new(tool);
    if path.components().count() > 1 || path.is_absolute() {
        return cwd.join(path).is_file();
    }
    std::iter::once(cwd.to_path_buf())
        .chain(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        ))
        .any(|dir| {
            extensions
                .iter()
                .any(|ext| dir.join(format!("{tool}{ext}")).is_file())
        })
}

fn preflight(service: &ProjectService) -> Result<(), String> {
    let cwd = std::path::Path::new(&service.cwd);
    if !cwd.is_dir() {
        return Err("目录不存在或不是文件夹，请检查项目路径".into());
    }
    let first = service
        .command
        .split_whitespace()
        .next()
        .unwrap_or_default();
    let tool = first.trim_matches(['\'', '"']);
    let known = [
        "node", "npm", "pnpm", "yarn", "bun", "python", "python3", "py", "cargo", "dotnet",
        "docker",
    ];
    if known.contains(&tool) && !tool_exists(tool, cwd) {
        return Err(format!("找不到 {tool}，请安装或将其加入 PATH 后重启即栈"));
    }
    if ["npm", "pnpm", "yarn"].contains(&tool) && !tool_exists("node", cwd) {
        return Err("找不到 Node.js，请安装后重启即栈".into());
    }
    if tool.contains("python")
        && (tool.contains('\\') || tool.contains('/'))
        && !tool_exists(tool, cwd)
    {
        return Err("Python 虚拟环境不存在，请重新创建环境或修改命令".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn opening_requires_ready_target_and_reports_missing_address() {
        let mut status = ServiceStatusEntry { id:"web".into(), name:"网页".into(), status:"running".into(), pid:None, session_id:None, message:None, open_url:None, readiness:"process".into() };
        assert!(entry_from_status(&status).unwrap_err().contains("尚未配置网页地址"));
        status.open_url=Some("http://localhost:5174/admin".into());
        assert_eq!(entry_from_status(&status).unwrap(), ServiceEntry::Url("http://localhost:5174/admin".into()));
        status.readiness="window".into();
        assert_eq!(entry_from_status(&status).unwrap(), ServiceEntry::Window);
        status.status="starting".into();
        assert!(entry_from_status(&status).unwrap_err().contains("尚未就绪"));
    }
    #[test]
    fn keys_round_trip_without_collisions() {
        for (project, service) in [("项目-前端", "api"), ("a-b", "c"), ("a", "b-c")] {
            assert_eq!(
                split_key(&service_key(project, service)),
                (project.into(), service.into())
            );
        }
        assert_ne!(service_key("a-b", "c"), service_key("a", "b-c"));
    }

    #[cfg(windows)]
    struct Fixture {
        root: PathBuf,
        manager: DevManager,
        app: tauri::App<tauri::test::MockRuntime>,
        project: Project,
    }
    #[cfg(windows)]
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "qd-services-中文 space-{}-{}",
                std::process::id(),
                next_id()
            ));
            std::fs::create_dir_all(&root).unwrap();
            std::fs::write(root.join("server.py"), "import http.server,sys,time\nready=time.monotonic()+float(sys.argv[2])\nclass Handler(http.server.BaseHTTPRequestHandler):\n def do_GET(self):\n  self.send_response(200 if time.monotonic()>=ready else 503);self.end_headers();self.wfile.write(b'ok')\n def log_message(self,*args):pass\nhttp.server.HTTPServer(('127.0.0.1',int(sys.argv[1])),Handler).serve_forever()\n").unwrap();
            let manager = DevManager::new();
            manager.2.lock().unwrap().settings_override = Some(crate::config::store::Settings {
                shell: "powershell.exe".into(),
                write_logs: false,
                ..Default::default()
            });
            let project = Project {
                id: format!("test-{}", next_id()),
                name: "测试前后端".into(),
                cwd: root.to_string_lossy().into_owned(),
                command: String::new(),
                port: None,
                open_url: None,
                tags: vec![],
                group: None,
                services: vec![],
                start_mode: StartMode::Parallel, launch_plans: vec![], default_plan_id: None,
            };
            Self {
                root,
                manager,
                app: tauri::test::mock_app(),
                project,
            }
        }
        fn server(&self, id: &str, delay: f32) -> ProjectService {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let port = listener.local_addr().unwrap().port();
            ProjectService {
                kind: None,
                id: id.into(),
                name: id.into(),
                cwd: self.root.to_string_lossy().into_owned(),
                command: format!("python -u server.py {port} {delay}"),
                port: Some(port),
                open_url: Some(format!("http://localhost:{port}")),
                readiness: Readiness::Http,
                health_url: Some(format!("http://127.0.0.1:{port}/health")),
                startup_timeout_secs: 15,
            }
        }
        fn failing(&self) -> ProjectService {
            let unused = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            ProjectService {
                kind: None,
                id: "bad".into(),
                name: "失败服务".into(),
                cwd: self.root.to_string_lossy().into_owned(),
                command: "python -c \"import time;time.sleep(0.6);raise SystemExit(7)\"".into(),
                port: Some(unused.local_addr().unwrap().port()),
                open_url: None,
                // This fixture must never become ready while PowerShell is still
                // loading. Process-only mode deliberately makes no such promise.
                readiness: Readiness::Port,
                health_url: None,
                startup_timeout_secs: 15,
            }
        }
    }
    #[cfg(windows)]
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = self.manager.stop_all(self.app.handle());
            // Only this fixture's unique, resolved directory is removed.
            if self.root.starts_with(std::env::temp_dir()) {
                let _ = std::fs::remove_dir_all(&self.root);
            }
        }
    }

    #[cfg(windows)]
    #[test]
    fn runtime_log_addresses_keep_parallel_configuration_and_reject_foreign_sessions() {
        let mut f=Fixture::new();
        let mut manual=f.server("manual",0.0);
        manual.open_url=Some(format!("http://127.0.0.1:{}/admin?mode=dev",manual.port.unwrap()));
        let mut discovered=f.server("discovered",0.0);
        let discovered_url=format!("http://127.0.0.1:{}/",discovered.port.unwrap());
        discovered.port=None;discovered.open_url=None;discovered.readiness=Readiness::Process;discovered.health_url=None;
        f.project.services=vec![manual.clone(),discovered];
        let configured=serde_json::to_value(&f.project.services).unwrap();
        assert!(f.manager.start(f.app.handle(),&f.project).ok);
        let records:Vec<_>={
            let state=f.manager.0.lock().unwrap();
            f.project.services.iter().map(|service| {
                let key=service_key(&f.project.id,&service.id);let session=&state.sessions[&key];
                let url=if service.id=="manual" {format!("http://127.0.0.1:{}/",manual.port.unwrap())} else {discovered_url.clone()};
                (key,session.pid,session.generation,url)
            }).collect()
        };
        std::thread::scope(|scope| {
            for (key,pid,generation,url) in &records {
                let manager=f.manager.clone();let app=f.app.handle().clone();let project=f.project.clone();
                scope.spawn(move || super::super::handle_log_line(&app,&manager,key,*pid,*generation,&project,&format!("Local: {url}"),false,false));
            }
        });
        let deadline=Instant::now()+Duration::from_secs(15);
        loop {
            f.manager.statuses();
            if f.manager.0.lock().unwrap().sessions.values().all(|session|session.observed_url.is_some()) {break;}
            assert!(Instant::now()<deadline,"owned listener never validated");
            thread::sleep(Duration::from_millis(100));
        }
        assert_eq!(f.manager.service_entry(&f.project.id,"manual").unwrap(),ServiceEntry::Url(manual.open_url.clone().unwrap()));
        assert_eq!(f.manager.service_entry(&f.project.id,"discovered").unwrap(),ServiceEntry::Url(discovered_url.clone()));
        assert_eq!(serde_json::to_value(&f.manager.2.lock().unwrap().projects[&f.project.id].services).unwrap(),configured);
        let (key,pid,generation,_) = &records[1];
        super::super::handle_log_line(f.app.handle(),&f.manager,key,*pid,generation+1,&f.project,"Local: http://localhost:65534/",false,false);
        assert_eq!(f.manager.service_entry(&f.project.id,"discovered").unwrap(),ServiceEntry::Url(discovered_url.clone()));
        let external=std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        super::super::handle_log_line(f.app.handle(),&f.manager,key,*pid,*generation,&f.project,&format!("Local: http://127.0.0.1:{}/",external.local_addr().unwrap().port()),false,false);
        assert_eq!(f.manager.service_entry(&f.project.id,"discovered").unwrap(),ServiceEntry::Url(discovered_url));
        assert_eq!(serde_json::to_value(&f.manager.2.lock().unwrap().projects[&f.project.id].services).unwrap(),configured);
        f.manager.stop(f.app.handle(),&f.project.id).unwrap();
        assert!(f.manager.statuses()[0].services.iter().find(|s|s.id=="discovered").unwrap().open_url.is_none(),"runtime discovery must disappear when session stops");
    }

    #[cfg(windows)]
    #[test]
    fn launch_plans_reuse_shared_jobs_and_stop_only_unreferenced_services() {
        use crate::config::store::LaunchPlan;
        let mut f=Fixture::new();
        f.project.services=vec![f.server("api",0.0),f.server("web",0.0),f.server("app",0.0)];
        f.project.launch_plans=vec![
            LaunchPlan{id:"website".into(),name:"官网".into(),service_ids:vec!["api".into(),"web".into()],start_mode:StartMode::Sequential,open_service_id:Some("web".into())},
            LaunchPlan{id:"app".into(),name:"App".into(),service_ids:vec!["api".into(),"app".into()],start_mode:StartMode::Parallel,open_service_id:Some("app".into())},
        ];
        f.project.default_plan_id=Some("website".into());
        assert!(f.manager.start(f.app.handle(),&f.project).ok);
        let api_key=service_key(&f.project.id,"api");
        let generation=f.manager.0.lock().unwrap().sessions[&api_key].generation;
        assert!(!f.manager.0.lock().unwrap().sessions.contains_key(&service_key(&f.project.id,"app")));
        assert!(f.manager.start_plan(f.app.handle(),&f.project,"app").ok);
        assert_eq!(f.manager.0.lock().unwrap().sessions[&api_key].generation,generation);
        assert_eq!(f.manager.statuses()[0].active_plan_ids.len(),2);
        f.manager.stop_plan(f.app.handle(),&f.project.id,"website").unwrap();
        assert!(!f.manager.0.lock().unwrap().sessions.contains_key(&service_key(&f.project.id,"web")));
        assert_eq!(f.manager.0.lock().unwrap().sessions[&api_key].generation,generation);
        f.manager.stop_plan(f.app.handle(),&f.project.id,"app").unwrap();
        assert!(f.manager.0.lock().unwrap().sessions.is_empty());
        assert!(f.manager.statuses()[0].active_plan_ids.is_empty());
        // An independently started backend is not owned by the later website plan.
        assert!(f.manager.start_service(f.app.handle(),&f.project,"api").ok);
        assert!(f.manager.start(f.app.handle(),&f.project).ok);
        f.manager.stop_plan(f.app.handle(),&f.project.id,"website").unwrap();
        assert!(f.manager.0.lock().unwrap().sessions.contains_key(&api_key));
        f.manager.stop(f.app.handle(),&f.project.id).unwrap();
        assert!(f.manager.0.lock().unwrap().sessions.is_empty());
        assert!(!f.manager.start_plan(f.app.handle(),&f.project,"missing").ok);
        // A stale independent claim cannot preserve a replacement generation.
        assert!(f.manager.start_service(f.app.handle(),&f.project,"api").ok);
        f.manager.stop_one(f.app.handle(),&api_key).unwrap();
        assert!(f.manager.start(f.app.handle(),&f.project).ok);
        f.manager.stop_plan(f.app.handle(),&f.project.id,"website").unwrap();
        assert!(f.manager.0.lock().unwrap().sessions.is_empty());
    }

    #[cfg(windows)]
    #[test]
    fn failed_launch_plan_rolls_back_new_jobs_and_preserves_another_plan() {
        use crate::config::store::LaunchPlan;
        let mut f=Fixture::new();
        f.project.services=vec![f.server("api",0.0),f.server("web",0.0),f.failing()];
        f.project.launch_plans=vec![
            LaunchPlan{id:"api".into(),name:"已有后端".into(),service_ids:vec!["api".into()],start_mode:StartMode::Parallel,open_service_id:None},
            LaunchPlan{id:"bad".into(),name:"失败方案".into(),service_ids:vec!["api".into(),"web".into(),"bad".into()],start_mode:StartMode::Sequential,open_service_id:None},
        ];
        assert!(f.manager.start_plan(f.app.handle(),&f.project,"api").ok);
        assert!(!f.manager.start_plan(f.app.handle(),&f.project,"bad").ok);
        let status=&f.manager.statuses()[0];
        assert_eq!(status.active_plan_ids,vec!["api"]);
        assert_eq!(f.manager.0.lock().unwrap().sessions.len(),1);
        assert!(f.manager.0.lock().unwrap().sessions.contains_key(&service_key(&f.project.id,"api")));
    }

    #[cfg(windows)]
    #[test]
    fn process_only_start_avoids_the_old_four_second_floor() {
        let mut f = Fixture::new();
        f.project.services = vec![ProjectService {
            kind: None,
            id:"process".into(), name:"进程检查".into(), cwd:f.root.to_string_lossy().into_owned(),
            command:"python -c \"import time;time.sleep(30)\"".into(), port:None, open_url:None,
            readiness:Readiness::Process, health_url:None, startup_timeout_secs:10,
        }];
        let began = Instant::now();
        let result = f.manager.start(f.app.handle(),&f.project);
        let elapsed = began.elapsed();
        assert!(result.ok, "{:?}",result);
        assert!(elapsed < Duration::from_secs(3), "process-only start took {:?}",elapsed);
        assert_eq!(f.manager.statuses()[0].services[0].readiness,"process");
        println!("PROCESS_ONLY_START_MS={}",elapsed.as_millis());
        f.manager.stop_all(f.app.handle()).unwrap();
        assert!(!f.manager.has_sessions_or_operations());
    }

    #[cfg(windows)]
    #[test]
    fn cmd_starts_from_a_chinese_space_directory_with_literal_quotes() {
        let mut f = Fixture::new();
        f.manager.2.lock().unwrap().settings_override.as_mut().unwrap().shell = "cmd.exe".into();
        std::fs::write(f.root.join("quoted script.py"),"import pathlib,time\npathlib.Path('started.txt').write_text('ok')\ntime.sleep(30)\n").unwrap();
        f.project.services = vec![ProjectService {
            kind: None,
            id:"cmd".into(),name:"CMD".into(),cwd:f.root.to_string_lossy().into_owned(),
            command:"python \"quoted script.py\"".into(),port:None,open_url:None,
            readiness:Readiness::Process,health_url:None,startup_timeout_secs:10,
        }];
        let result = f.manager.start(f.app.handle(),&f.project);
        assert!(result.ok,"{:?}",result);
        let deadline = Instant::now()+Duration::from_secs(5);
        while !f.root.join("started.txt").exists() && Instant::now()<deadline { thread::sleep(Duration::from_millis(25)); }
        assert!(f.root.join("started.txt").exists(),"quoted command did not execute in the configured directory");
        f.manager.stop_all(f.app.handle()).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn joint_failure_rolls_back_only_new_jobs_and_preserves_existing_service() {
        let mut f = Fixture::new();
        f.project.services = vec![f.server("api", 0.0), f.failing()];
        let api = f.manager.start_service(f.app.handle(), &f.project, "api");
        assert!(api.ok, "{:?}", api);
        let before = f.manager.owned_pids();
        assert_eq!(before.len(), 1);
        let ports =
            crate::port::list_listening_ports(&[], &before, &f.manager.running_roots()).unwrap();
        let entry = ports
            .iter()
            .find(|p| Some(p.port) == f.project.services[0].port)
            .unwrap();
        assert_eq!(entry.service_id.as_deref(), Some("api"));
        assert_eq!(entry.owned, Some(true));
        assert_eq!(
            f.manager.service_owning_pid(entry.pid),
            Some((f.project.id.clone(), "api".into()))
        );
        let result = f.manager.start(f.app.handle(), &f.project);
        assert!(!result.ok);
        assert_eq!(f.manager.owned_pids(), before);
        let status = f.manager.statuses().pop().unwrap();
        assert_eq!(status.status, "partial");
        assert!(f.manager.has_error());
        f.manager.stop(f.app.handle(), &f.project.id).unwrap();
        assert_eq!(f.manager.running_count(), 0);
        // Job termination is synchronous for the request, but Windows can
        // finish closing a killed child's listening socket asynchronously.
        let address = ("127.0.0.1", f.project.services[0].port.unwrap());
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if std::net::TcpListener::bind(address).is_ok() { break; }
            assert!(Instant::now() < deadline, "Stopped API port was not released");
            thread::sleep(Duration::from_millis(25));
        }
        let result = f.manager.start(f.app.handle(), &f.project);
        assert!(!result.ok);
        assert_eq!(f.manager.running_count(), 0);
        assert!(
            f.manager
                .logs(&f.project.id, Some("bad"))
                .iter()
                .any(|line| line.contains("python") || line.starts_with('['))
                || f.manager.statuses()[0].services.iter().find(|s| s.id == "bad").unwrap().message.is_some(),
            "Missing failing service feedback: {:?}; {:?}", result, f.manager.statuses()
        );
    }

    #[cfg(windows)]
    #[test]
    fn sequential_start_waits_for_http_2xx_and_stop_cancels_pending_start() {
        let mut f = Fixture::new();
        f.project.services = vec![f.server("api", 1.0), f.server("web", 0.0)];
        f.project.start_mode = StartMode::Sequential;
        let app = f.app.handle().clone();
        thread::scope(|scope| {
            let task = scope.spawn(|| f.manager.start(&app, &f.project));
            let deadline = Instant::now() + Duration::from_secs(15);
            loop {
                let status = f.manager.statuses();
                if status.first().is_some_and(|p| p.services[0].pid.is_some()) {
                    assert!(
                        status[0].services[1].pid.is_none(),
                        "second service must wait for first readiness"
                    );
                    break;
                }
                assert!(Instant::now() < deadline);
                thread::sleep(Duration::from_millis(20));
            }
            let result = task.join().unwrap();
            assert!(result.ok, "{:?}", result);
        });
        assert_eq!(f.manager.statuses()[0].status, "running");
        f.manager.stop(f.app.handle(), &f.project.id).unwrap();
        f.project.services = vec![f.server("slow", 30.0)];
        thread::scope(|scope| {
            let task = scope.spawn(|| f.manager.start(&app, &f.project));
            let deadline = Instant::now() + Duration::from_secs(15);
            while f.manager.owned_pids().is_empty() {
                assert!(Instant::now() < deadline);
                thread::sleep(Duration::from_millis(20));
            }
            f.manager.stop(f.app.handle(), &f.project.id).unwrap();
            assert!(!task.join().unwrap().ok);
        });
        assert_eq!(f.manager.running_count(), 0);
        assert!(!f.manager.has_sessions_or_operations());
        assert_eq!(f.manager.statuses()[0].status, "stopped");
    }

    #[cfg(windows)]
    #[test]
    fn parallel_start_reports_both_services_and_stop_all_cleans_every_job() {
        let mut f = Fixture::new();
        f.project.services = vec![f.server("api", 0.0), f.server("web", 0.0)];
        let result = f.manager.start(f.app.handle(), &f.project);
        assert!(result.ok, "{:?}", result);
        let status = f.manager.statuses().pop().unwrap();
        assert_eq!(status.status, "running");
        assert!(status
            .services
            .iter()
            .all(|s| s.pid.is_some() && s.status == "running"));
        assert_ne!(status.services[0].session_id, status.services[1].session_id);
        f.manager.stop_all(f.app.handle()).unwrap();
        assert!(!f.manager.has_sessions_or_operations());
        assert_eq!(f.manager.statuses()[0].status, "stopped");
    }
    #[cfg(windows)]
    #[test]
    fn timeout_leaves_no_job_and_retains_actionable_error() {
        let mut f = Fixture::new();
        let mut service = f.server("slow", 30.0);
        service.startup_timeout_secs = 1;
        f.project.services = vec![service];
        let result = f.manager.start(f.app.handle(), &f.project);
        assert!(!result.ok);
        assert!(result.message.unwrap().contains("超时"));
        assert_eq!(f.manager.running_count(), 0);
        assert!(f.manager.statuses()[0].services[0].message.is_some());
    }
}
