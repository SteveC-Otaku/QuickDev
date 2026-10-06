import { isTauri } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  bootstrap,
  exportConfig,
  getDevLogs,
  importConfig,
  loadSettings,
  killAndRestart,
  killProcess,
  listDevStatus,
  listPorts,
  listProjects,
  removeProject,
  startDev,
  startService,
  focusService,
  stopService,
  stopAllDevs,
  stopDev,
  updateSettings,
  upsertProject,
} from "./ipc";
import { isDesktopService, projectServices } from "./services";
import type {
  ConflictInfo,
  DevStatus,
  LogLine,
  PortEntry,
  Project,
  Settings,
  TabId,
  ProjectStatus,
} from "./types";

// Yield one browser paint so feedback is visible before dispatching native work.
async function paintFeedback() {
  if (typeof requestAnimationFrame !== "undefined") await new Promise<void>(resolve => requestAnimationFrame(() => setTimeout(resolve, 0)));
}

const LOG_HEIGHT_KEY = "qd-log-height";

function defaultSettings(): Settings {
  return {
    highlightPorts: [3000, 5173, 8080, 4173, 2283],
    autoRefresh: true,
    minimizeToTray: true,
    autoStart: false,
    hotkey: "Ctrl+Shift+D",
    writeLogs: false,
    groups: ["工作", "个人"],
    shell: "powershell",
    trayAnimation: true,
    refreshIntervalSecs: 5,
    startPage: 'ports',
  };
}

function classifyLog(text: string): LogLine["level"] {
  const lower = text.toLowerCase();
  if (lower.includes("error") || lower.includes("failed")) return "error";
  if (lower.includes("warn")) return "warn";
  if (text.includes("Local:") || text.includes("ready") || text.includes("✓")) return "success";
  if (text.includes("Network:")) return "info";
  return undefined;
}

function readableLog(text: string) {
  return text.replace(/\[(\d{13})\]/, (_match, stamp) => {
    const date = new Date(Number(stamp));
    return `[${date.toLocaleTimeString('zh-CN',{hour12:false})}.${String(date.getMilliseconds()).padStart(3,'0')}]`;
  });
}
function toLogLines(lines: string[]): LogLine[] {
  return lines.map((text) => ({ text: readableLog(text), level: classifyLog(text) }));
}

function formatTime(d = new Date()) {
  return `${d.getHours().toString().padStart(2, "0")}:${d.getMinutes().toString().padStart(2, "0")}:${d.getSeconds().toString().padStart(2, "0")}`;
}

// Chapter: app store — ports / projects / settings 状态收口
class AppStore {
  settings = $state<Settings>(defaultSettings());
  ports = $state<PortEntry[]>([]);
  projects = $state<Project[]>([]);
  logs = $state<LogLine[]>([]);
  selectedProjectId = $state<string | undefined>();
  selectedServiceId = $state<string | undefined>();
  portFocus = $state<{port:number;pid?:number;token:number} | undefined>();
  serviceBusy = $state<string[]>([]);
  localOperationStartedAt = $state<number | undefined>();
  #revisions = new Map<string, number>();
  #logSessions = new Map<string, number>();
  conflict = $state<ConflictInfo | null>(null);
  killBusy = $state(false);
  killingPid = $state<number | null>(null);
  startingId = $state<string | null>(null);
  portsLoading = $state(false);
  lastRefresh = $state("—");
  runningCount = $state(0);
  hasError = $state(false);
  activeTab = $state<TabId>("ports");
  formOpen = $state(false);
  editingProject = $state<Project | null>(null);
  notice = $state<{ text: string; kind: "error" | "success" } | null>(null);
  confirmation = $state<{ title: string; text: string; action: string } | null>(null);
  stoppingIds = $state<string[]>([]);
  stopAllBusy = $state(false);
  settingsBusy = $state(false);
  configBusy = $state(false);
  logsLoading = $state(false);
  bootError = $state("");
  portsError = $state("");
  preview = $state(false);
  exitMessage = $state("");
  get operationText() {
    if (this.exitMessage) return this.exitMessage;
    const operation = this.projects.find(p => p.operation)?.operation;
    if (operation) return `${this.projects.find(p => p.operation?.id === operation.id)?.name ?? "项目"} · ${operation.message}`;
    if (this.serviceBusy.length) return "正在处理服务，请稍候…";
    if (this.stopAllBusy) return "正在停止全部项目…";
    if (this.killBusy) return this.conflict ? "正在释放端口并重新启动…" : "正在结束占用进程…";
    const stopping = this.projects.filter(p => p.status === "stopping" || this.stoppingIds.includes(p.id));
    if (stopping.length || this.stoppingIds.length) return `正在停止 ${stopping[0]?.name ?? "项目"}${stopping.length > 1 ? ` 等 ${stopping.length} 个项目` : ""}…`;
    const starting = this.projects.filter(p => p.status === "starting" || p.id === this.startingId);
    if (starting.length || this.startingId) return `正在启动 ${starting[0]?.name ?? "项目"}，等待服务响应…`;
    return "";
  }

  get operationStartedAt() { return this.projects.find(p => p.operation)?.operation?.startedAt ?? this.localOperationStartedAt; }
  get operationId() { return this.projects.find(p => p.operation)?.operation?.id ?? this.localOperationStartedAt; }

  get runningServiceCount() { return this.projects.flatMap(p => p.serviceStatuses ?? []).filter(s => s.status === 'running').length; }
  get readyServiceCount() { return this.projects.flatMap(p => p.serviceStatuses ?? []).filter(s => s.status === 'running' && s.readiness !== 'process').length; }
  recount() {
    this.runningCount = this.projects.filter(p => ["running", "partial", "starting", "stopping", "rollingBack"].includes(p.status ?? "")).length;
    this.hasError = this.projects.some(p => ["crashed", "failed", "conflict"].includes(p.status ?? "") || p.serviceStatuses?.some(s => s.message || s.status === "failed"));
  }

  applyStatusEvent(status: ProjectStatus) {
    if (status.revision !== undefined && status.revision < (this.#revisions.get(status.id) ?? 0)) return;
    if (status.revision !== undefined) this.#revisions.set(status.id, status.revision);
    this.projects = this.projects.map(p => p.id === status.id ? { ...p, status: status.status, serviceStatuses: status.services, operation: status.operation, activePlanIds:status.activePlanIds } : p);
    this.recount();
  }

  #confirmResolve?: (value: boolean) => void;
  #noticeTimer?: ReturnType<typeof setTimeout>;
  #heightTimer?: ReturnType<typeof setTimeout>;
  #portsRequest?: Promise<void>;
  #statusRequest?: Promise<void>;
  #logVersion = 0;
  #logEvents = 0;
  #disposed = false;
  booting = $state(true);
  bootHint = $state("正在启动…");
  logHeight = $state(160);

  #timer: ReturnType<typeof setInterval> | undefined;
  #unsubs: UnlistenFn[] = [];

  constructor() {
    if (typeof localStorage !== "undefined") {
      try {
        const h = Number(localStorage.getItem(LOG_HEIGHT_KEY));
        if (h >= 100 && h <= 420) this.logHeight = h;
      } catch { /* storage unavailable */ }
    }
  }

  get tabs() {
    const items: { id: TabId; label: string; badge?: string }[] = [
      { id: "ports", label: "端口" },
      { id: "projects", label: "项目" },
    ];
    const n = this.projects.filter((p) => p.status === "conflict" || p.status === "crashed" || p.serviceStatuses?.some(s => s.message)).length;
    if (n > 0) items[1].badge = `${n} 异常`;
    items.push({ id: "settings", label: "设置" });
    return items;
  }

  notify(text: string, kind: "error" | "success" = "error") {
    if (this.#noticeTimer) clearTimeout(this.#noticeTimer);
    this.notice = { text, kind };
    if (kind === "success") this.#noticeTimer = setTimeout(() => this.notice = null, 4000);
  }

  ask(title: string, text: string, action: string) {
    if (this.confirmation) return Promise.resolve(false);
    this.confirmation = { title, text, action };
    return new Promise<boolean>((resolve) => this.#confirmResolve = resolve);
  }

  resolveConfirmation(value: boolean) {
    this.#confirmResolve?.(value);
    this.#confirmResolve = undefined;
    this.confirmation = null;
  }

  async boot() {
    this.dispose();
    this.#disposed = false;
    this.bootError = "";
    this.preview = !isTauri();
    if (this.preview) { this.booting = false; return; }
    this.booting = true;
    this.bootHint = "读取配置…";
    try {
      const payload = await bootstrap();
      this.settings = payload.settings;
      this.activeTab = payload.settings.startPage ?? 'ports';
      this.applyStatuses(payload.projects, payload.statuses);
      this.bootHint = "扫描端口…";
      await Promise.all([this.refreshPorts(), this.selectedProjectId ? this.loadLogs(this.selectedProjectId, true) : Promise.resolve()]);
    } catch (e) {
      this.bootError = `读取配置失败：${String(e)}`;
    } finally {
      this.booting = false;
      this.bootHint = "";
    }

    if (this.#disposed || this.bootError) return;
    let lastRefreshAt = Date.now();
    this.#timer = setInterval(() => {
      if (!this.settings.autoRefresh) return;
      const interval = (this.settings.refreshIntervalSecs ?? 5) * 1000;
      if (Date.now() - lastRefreshAt < interval) return;
      lastRefreshAt = Date.now();
      void this.mergeProjectStatus();
      if (this.activeTab === "ports") void this.refreshPorts();
    }, 1000);

    try {
      this.#unsubs.push(
        await listen("dev-log", (e) => {
          const payload = e.payload as { id: string; serviceId?: string; sessionId?: number; line: string };
          if (payload.id !== this.selectedProjectId || this.selectedServiceId && payload.serviceId !== this.selectedServiceId) return;
          if (payload.sessionId !== undefined && payload.serviceId) {
            const key = `${payload.id}/${payload.serviceId}`;
            const current = this.projects.find(p => p.id === payload.id)?.serviceStatuses?.find(s => s.id === payload.serviceId)?.sessionId ?? 0;
            if (payload.sessionId < Math.max(current, this.#logSessions.get(key) ?? 0)) return;
            this.#logSessions.set(key, payload.sessionId);
          }
          this.#logEvents++;
          this.logs = [...this.logs, { text: readableLog(this.selectedServiceId ? payload.line : `[${this.projects.find(p => p.id === payload.id)?.serviceStatuses?.find(s => s.id === payload.serviceId)?.name ?? "服务"}] ${payload.line}`), level: classifyLog(payload.line) }].slice(-2000);
        }),
      );
      this.#unsubs.push(await listen<ProjectStatus>("dev-status", (e) => {
        this.applyStatusEvent(e.payload);
        void this.mergeProjectStatus();
      }));
      this.#unsubs.push(await listen<string>("exit-state", e => { this.exitMessage=e.payload; if (e.payload) this.localOperationStartedAt=Date.now(); }));
      this.#unsubs.push(await listen("open-project-form", () => { this.openAddForm(); }));
      this.#unsubs.push(await listen<{ id: string; serviceId?: string }>("open-service-logs", (e) => {
        void this.openServiceLogs(e.payload.id,e.payload.serviceId);
      }));
      this.#unsubs.push(await listen<string>("operation-error", (e) => this.notify(e.payload)));
      this.#unsubs.push(await listen<string>("navigate-tab", (e) => {
        if (e.payload === "ports" || e.payload === "projects" || e.payload === "settings") this.activeTab = e.payload;
      }));
      this.#unsubs.push(await listen<{ id: string; result: { message?: string; conflict?: PortEntry } }>("dev-start-failed", (e) => {
        const { id, result } = e.payload;
        this.activeTab = "projects";
        if (result.conflict) {
          this.conflict = { port: result.conflict.port, pid: result.conflict.pid, command: result.conflict.command, projectId: id };
          void this.mergeProjectStatus();
        } else { this.notify(result.message ?? "启动失败，请查看项目配置"); }
      }));
      this.#unsubs.push(
        await listen("trigger-refresh", () => {
          void this.mergeProjectStatus();
          void this.refreshPorts();
        }),
      );
    } catch {
      this.notify("实时事件连接失败，请使用手动刷新");
    }
    if (this.#disposed) this.dispose();
  }

  dispose() {
    this.#disposed = true;
    this.#logVersion++;
    this.resolveConfirmation(false);
    if (this.#noticeTimer) clearTimeout(this.#noticeTimer);
    if (this.#timer) clearInterval(this.#timer);
    if (this.#heightTimer) { clearTimeout(this.#heightTimer); this.persistLogHeight(); }
    this.#unsubs.forEach((u) => u());
    this.#unsubs = [];
  }

  applyStatuses(list: Project[], statuses: ProjectStatus[]) {
    const statusMap = new Map(statuses.map(s => [s.id, s]));
    this.projects = list.map(p => {
      const state = statusMap.get(p.id);
      const previous = this.projects.find(old => old.id === p.id);
      if (state?.revision !== undefined && state.revision < (this.#revisions.get(p.id) ?? 0)) return { ...p, status: previous?.status, serviceStatuses: previous?.serviceStatuses, operation: previous?.operation, activePlanIds:previous?.activePlanIds };
      if (state?.revision !== undefined) this.#revisions.set(p.id, state.revision);
      return { ...p, status: state?.status ?? (this.conflict?.projectId === p.id ? "conflict" : "stopped"), serviceStatuses: state?.services, operation: state?.operation, activePlanIds:state?.activePlanIds };
    });
    if (!this.projects.some(p => p.id === this.selectedProjectId)) {
      this.selectedProjectId = this.projects[0]?.id; this.selectedServiceId = undefined; this.logs = []; this.#logVersion++;
    }
    this.recount();
  }

  async mergeProjectStatus() {
    if (this.preview) return;
    if (this.#statusRequest) return this.#statusRequest;
    this.#statusRequest = (async () => {
      try {
        const [list, statuses] = await Promise.all([listProjects(), listDevStatus()]);
        if (this.#disposed) return;
        this.applyStatuses(list, statuses);
        if (this.selectedProjectId) await this.loadLogs(this.selectedProjectId, false);
      } catch (e) {
        this.notify(`刷新项目失败：${String(e)}`);
      }
    })();
    try { await this.#statusRequest; } finally { this.#statusRequest = undefined; }
  }

  async refreshPorts() {
    if (this.preview) return;
    if (this.#portsRequest) return this.#portsRequest;
    this.portsLoading = true;
    this.portsError = "";
    this.#portsRequest = (async () => {
      try {
        const ports = await listPorts();
        if (this.#disposed) return;
        this.ports = ports;
        this.lastRefresh = formatTime();
      } catch (e) {
        this.portsError = `端口扫描失败：${String(e)}`;
      }
    })();
    try { await this.#portsRequest; }
    finally { this.#portsRequest = undefined; this.portsLoading = false; }
  }

  async refreshAll() {
    await Promise.all([this.refreshPorts(), this.mergeProjectStatus()]);
  }

  async handleKill(pid: number) {
    if (this.killBusy) return;
    const restartId = this.conflict?.pid === pid ? this.conflict.projectId : undefined;
    const restartPlanId = this.conflict?.pid === pid ? this.conflict.planId : undefined;
    const msg = restartId
      ? `确认结束占用进程 PID ${pid}，并重新启动项目？`
      : `确认结束进程 PID ${pid}？`;
    if (!await this.ask("结束进程", `${msg} 该进程的子进程也会结束，请先保存相关工作。`, restartId ? "结束并启动" : "结束进程")) return;

    this.killBusy = true;
    this.killingPid = pid;
    await paintFeedback();
    try {
      if (restartId) {
        const result = await killAndRestart(pid, restartId, restartPlanId);
        this.conflict = null;
        if (!result.ok && result.conflict) {
          this.conflict = {
            port: result.conflict.port,
            pid: result.conflict.pid,
            command: result.conflict.command,
            projectId: restartId,
            planId:restartPlanId,
          };
          this.projects = this.projects.map((p) =>
            p.id === restartId ? { ...p, status: "conflict" } : p,
          );
        } else if (!result.ok && result.message) {
          this.notify(result.message);
        } else {
          this.selectedProjectId = restartId;
        }
        await this.refreshAll();
        if (result.ok) await this.openPlanEntry(restartId,restartPlanId);
      } else {
        await killProcess(pid);
        await new Promise((r) => setTimeout(r, 400));
        await this.refreshAll();
      }
    } catch (e) {
      this.notify(String(e));
    } finally {
      this.killBusy = false;
      this.killingPid = null;
    }
  }

  async handleStart(id: string, planId?: string) {
    if (this.startingId || this.stopAllBusy || this.stoppingIds.includes(id)) return;
    this.conflict = null;
    this.startingId = id;
    this.localOperationStartedAt = Date.now();
    await paintFeedback();
    try {
      const result = await startDev(id, planId);
      if (!result.ok) {
        if (result.conflict) {
          this.conflict = {
            port: result.conflict.port,
            pid: result.conflict.pid,
            command: result.conflict.command,
            projectId: id,
            planId,
          };
          this.projects = this.projects.map((p) =>
            p.id === id ? { ...p, status: "conflict" } : p,
          );
        } else if (result.message) {
          this.notify(result.message);
        }
        return;
      }
      this.selectedProjectId = id;
      await this.mergeProjectStatus();
      await this.openPlanEntry(id,planId);
      void this.refreshPorts();
    } catch (e) {
      this.notify(String(e));
    } finally {
      this.startingId = null;
      this.localOperationStartedAt = undefined;
    }
  }

  async handleStop(id: string, planId?: string) {
    if (this.stopAllBusy || this.stoppingIds.includes(id) || this.projects.some(p => p.id === id && p.status === "stopping")) return;
    this.stoppingIds = [...this.stoppingIds, id];
    this.localOperationStartedAt = Date.now();
    await paintFeedback();
    try {
      await stopDev(id, planId);
      await this.mergeProjectStatus();
      void this.refreshPorts();
      this.notify(planId ? "方案已停止；其他方案或独立服务需要的进程会保留" : "项目已停止", "success");
    } catch (e) { this.notify(`停止失败：${String(e)}`); }
    finally { this.stoppingIds = this.stoppingIds.filter((item) => item !== id); this.localOperationStartedAt = undefined; }
  }

  async handleStopAll() {
    if (this.stopAllBusy || this.startingId || this.stoppingIds.length) return;
    if (!await this.ask("停止全部项目", `将停止即栈管理的 ${this.runningCount} 个项目，不会结束其他外部服务。`, "全部停止")) return;
    this.stopAllBusy = true;
    this.localOperationStartedAt = Date.now();
    await paintFeedback();
    try {
      await stopAllDevs();
      await this.mergeProjectStatus();
      void this.refreshPorts();
      this.notify("全部项目已停止", "success");
    } catch (e) { this.notify(`停止失败：${String(e)}`); }
    finally { this.stopAllBusy = false; this.localOperationStartedAt = undefined; }
  }

  async inspectPort(port:number,pid?:number) {
    this.portFocus={port,pid,token:Date.now()}; this.activeTab='ports';
    await this.refreshPorts();
    if (!this.ports.some(p=>p.port===port)) this.notify(`端口 :${port} 当前已释放，或扫描未能读取`);
  }

  async openServiceLogs(projectId: string, serviceId?: string) {
    this.activeTab = "projects";
    await this.selectProject(projectId);
    if (this.selectedProjectId === projectId) await this.selectLogService(serviceId);
  }
  async selectLogService(serviceId?: string) {
    this.selectedServiceId = serviceId; this.logs = [];
    if (this.selectedProjectId) await this.loadLogs(this.selectedProjectId, true);
  }

  async handleService(projectId: string, serviceId: string, kind: "start" | "stop" | "restart") {
    const key = `${projectId}/${serviceId}`;
    if (this.serviceBusy.includes(key) || this.stopAllBusy || this.projects.find(p => p.id === projectId)?.operation) return;
    this.serviceBusy = [...this.serviceBusy, key]; this.localOperationStartedAt = Date.now();
    await paintFeedback();
    try {
      if (kind !== "start") await stopService(projectId, serviceId);
      if (kind !== "stop") {
        const result = await startService(projectId, serviceId);
        if (!result.ok) { this.notify(result.message ?? "服务启动失败"); if (result.conflict) this.conflict = {projectId, port: result.conflict.port, pid: result.conflict.pid, command: result.conflict.command}; }
      }
      await this.mergeProjectStatus(); void this.refreshPorts();
    } catch (e) { this.notify(String(e)); }
    finally { this.serviceBusy = this.serviceBusy.filter(item => item !== key); this.localOperationStartedAt = undefined; }
  }

  async openService(projectId: string, serviceId: string) {
    const project = this.projects.find(p => p.id === projectId);
    if (project?.serviceStatuses?.find(s => s.id === serviceId)?.status !== "running") return;
    const service = projectServices(project).find(s => s.id === serviceId);
    if (service && isDesktopService(service)) {
      try { await focusService(projectId, serviceId); } catch (e) { this.notify(String(e)); }
      return;
    }
    const url = project.serviceStatuses?.find(s => s.id === serviceId)?.openUrl ?? service?.openUrl ?? (service?.port ? `http://localhost:${service.port}` : undefined);
    if (url) await this.openAddress(url); else this.notify("此服务尚未设置打开地址");
  }

  async openPlanEntry(projectId:string,planId?:string) {
    const project=this.projects.find(p=>p.id===projectId);
    const plan=project?.launchPlans?.find(p=>p.id===(planId??project.defaultPlanId));
    if(plan?.openServiceId) await this.openService(projectId,plan.openServiceId);
  }

  async openAddress(url: string) {
    try { await openUrl(url); }
    catch (e) { this.notify(`打开地址失败：${String(e)}`); }
  }

  async handleOpen(project: Project) {
    const service = projectServices(project)[0];
    if (isDesktopService(service)) { await this.openService(project.id, service.id); return; }
    const url = project.openUrl ?? (project.port ? `http://localhost:${project.port}` : null);
    if (url) await this.openAddress(url);
    else this.notify("请先为项目设置打开地址或端口");
  }

  async handleOpenPort(port: number) {
    const entry = this.ports.find(p => p.port === port && p.projectId);
    const project = this.projects.find(p => p.id === entry?.projectId);
    const service = project && projectServices(project).find(s => s.id === entry?.serviceId);
    if (project && service && isDesktopService(service)) {
      if (project.serviceStatuses?.find(s => s.id === service.id)?.status !== 'running') { this.notify('App 窗口尚未就绪，请查看编译日志'); return; }
      await this.openService(project.id, service.id); return;
    }
    await this.openAddress(`http://localhost:${port}`);
  }

  openAddForm() {
    this.editingProject = null;
    this.formOpen = true;
    this.activeTab = "projects";
  }

  openEditForm(project: Project) {
    this.editingProject = project;
    this.formOpen = true;
  }

  async handleSaveProject(project: Project) {
    await upsertProject(project);
    await this.mergeProjectStatus();
    this.notify("项目已保存", "success");
  }

  async handleDeleteProject(id: string) {
    const target = this.projects.find((p) => p.id === id);
    if (!target) return;
    if (!await this.ask("删除项目", `确认删除「${target.name}」的配置？项目文件会保留。`, "删除配置")) return;
    try {
    await removeProject(id);
    if (this.selectedProjectId === id) this.selectedProjectId = undefined;
    await this.mergeProjectStatus();
    this.notify("项目配置已删除", "success");
    } catch (e) { this.notify(`删除失败：${String(e)}`); }
  }

  async selectProject(id: string) {
    this.selectedProjectId = id;
    this.selectedServiceId = undefined;
    this.logs = [];
    await this.loadLogs(id, true);
  }

  async loadLogs(id: string, showLoading: boolean) {
    if (!showLoading && this.logsLoading) return;
    const version = ++this.#logVersion;
    const events = this.#logEvents;
    const serviceId = this.selectedServiceId;
    if (showLoading) this.logsLoading = true;
    try {
      const lines = await getDevLogs(id, serviceId);
      if (!this.#disposed && this.selectedProjectId === id && this.selectedServiceId === serviceId && version === this.#logVersion && events === this.#logEvents) this.logs = toLogLines(lines);
    } catch (e) { this.notify(`读取日志失败：${String(e)}`); }
    finally { if (version === this.#logVersion) this.logsLoading = false; }
  }

  async handleSettingsChange(next: Settings) {
    if (this.settingsBusy) return;
    const highlightChanged = next.highlightPorts.join(',') !== this.settings.highlightPorts.join(',');
    this.settingsBusy = true;
    try {
      await updateSettings(next);
      this.settings = next;
      this.notify("设置已保存", "success");
      if (highlightChanged) await this.refreshPorts();
    } catch (e) {
      try { this.settings = await loadSettings(); } catch { /* keep last known settings */ }
      this.notify(`设置保存失败：${String(e)}`);
    } finally { this.settingsBusy = false; }
  }

  async handleExportConfig() {
    if (this.configBusy) return;
    this.configBusy = true;
    try {
      const json = await exportConfig();
      const blob = new Blob([json], { type: "application/json" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `quick-dev-config-${new Date().toISOString().slice(0, 10)}.json`;
      a.click();
      setTimeout(() => URL.revokeObjectURL(url), 1000);
      this.notify("配置已导出", "success");
    } catch (e) {
      this.notify(String(e));
    } finally { this.configBusy = false; }
  }

  async handleImportConfig(file: File) {
    if (this.configBusy) return;
    if (this.runningCount > 0) { this.notify("请先停止全部项目，再导入配置"); return; }
    if (!await this.ask("导入配置", "导入将覆盖当前项目与设置，建议先导出备份。", "覆盖并导入")) return;
    this.configBusy = true;
    try {
      await importConfig(await file.text());
      const payload = await bootstrap();
      this.settings = payload.settings;
      this.applyStatuses(payload.projects, payload.statuses);
      await this.refreshPorts();
      this.notify("配置导入成功", "success");
    } catch (e) {
      this.notify(String(e));
    } finally { this.configBusy = false; }
  }

  setLogHeight(h: number) {
    this.logHeight = Math.min(420, Math.max(100, h));
    if (this.#heightTimer) clearTimeout(this.#heightTimer);
    this.#heightTimer = setTimeout(() => { this.#heightTimer = undefined; this.persistLogHeight(); }, 200);
  }

  private persistLogHeight() {
    try { localStorage.setItem(LOG_HEIGHT_KEY, String(this.logHeight)); } catch { /* storage unavailable */ }
  }
}

export const app = new AppStore();
