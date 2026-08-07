<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import ProjectFormModal from "$lib/components/ProjectFormModal.svelte";
  import PortsPanel from "$lib/components/PortsPanel.svelte";
  import ProjectsPanel from "$lib/components/ProjectsPanel.svelte";
  import SettingsPanel from "$lib/components/SettingsPanel.svelte";
  import {
    exportConfig,
    importConfig,
    getDevLogs,
    killProcess,
    listDevStatus,
    listPorts,
    listProjects,
    loadSettings,
    removeProject,
    startDev,
    stopAllDevs,
    stopDev,
    updateSettings,
    upsertProject,
  } from "$lib/ipc";
  import type { DevStatus, LogLine, PortEntry, Project, Settings, TabId } from "$lib/types";

  let activeTab = $state<TabId>("ports");
  let settings = $state<Settings>({
    highlightPorts: [3000, 5173, 8080, 4173, 2283],
    autoRefresh: true,
    minimizeToTray: true,
    autoStart: false,
    hotkey: "Ctrl+Shift+D",
    writeLogs: false,
    groups: ["工作", "个人"],
    shell: "powershell",
  });
  let ports = $state<PortEntry[]>([]);
  let projects = $state<Project[]>([]);
  let logs = $state<LogLine[]>([]);
  let selectedProjectId = $state<string | undefined>();
  let conflict = $state<{ port: number; pid: number; command: string } | null>(null);
  let portsLoading = $state(false);
  let lastRefresh = $state("—");
  let runningCount = $state(0);
  let hasError = $state(false);
  let formOpen = $state(false);
  let editingProject = $state<Project | null>(null);
  let rawProjectCount = $state(0);

  const tabs = $derived.by(() => {
    const items: { id: TabId; label: string; badge?: string }[] = [
      { id: "ports", label: "端口" },
      { id: "projects", label: "项目" },
    ];
    const conflicts = projects.filter((p) => p.status === "conflict" || p.status === "crashed").length;
    if (conflicts > 0) items[1].badge = `${conflicts} 异常`;
    items.push({ id: "settings", label: "设置" });
    return items;
  });

  function formatRefreshTime() {
    const now = new Date();
    lastRefresh = `${now.getHours().toString().padStart(2, "0")}:${now.getMinutes().toString().padStart(2, "0")}:${now.getSeconds().toString().padStart(2, "0")}`;
  }

  function classifyLog(text: string): LogLine["level"] {
    const lower = text.toLowerCase();
    if (lower.includes("error") || lower.includes("failed")) return "error";
    if (lower.includes("warn")) return "warn";
    if (text.includes("Local:") || text.includes("ready") || text.includes("✓")) return "success";
    if (text.includes("Network:")) return "info";
    return undefined;
  }

  function toLogLines(lines: string[]): LogLine[] {
    return lines.map((text) => ({ time: "", text, level: classifyLog(text) }));
  }

  async function mergeProjectStatus() {
    const [list, statuses] = await Promise.all([listProjects(), listDevStatus()]);
    const statusMap = new Map(statuses.map((s) => [s.id, s.status]));
    rawProjectCount = list.length;
    runningCount = statuses.filter((s) => s.status === "running" || s.status === "starting").length;
    hasError = statuses.some((s) => s.status === "crashed");
    projects = list.map((p) => ({
      ...p,
      status: (statusMap.get(p.id) ?? "stopped") as DevStatus,
    }));
    if (!selectedProjectId && projects.length > 0) {
      selectedProjectId = projects[0].id;
    }
    if (selectedProjectId) {
      const raw = await getDevLogs(selectedProjectId);
      logs = toLogLines(raw);
    }
    conflict = null;
  }

  async function refreshPorts() {
    portsLoading = true;
    try {
      ports = await listPorts();
      formatRefreshTime();
    } catch {
      // preview without Tauri
    } finally {
      portsLoading = false;
    }
  }

  async function refreshAll() {
    await Promise.all([refreshPorts(), mergeProjectStatus()]);
  }

  async function handleKill(pid: number) {
    if (!confirm(`确认 Kill PID ${pid} 及其子进程？`)) return;
    try {
      await killProcess(pid);
      await refreshAll();
    } catch (e) {
      alert(String(e));
    }
  }

  async function handleStart(id: string) {
    conflict = null;
    try {
      const result = await startDev(id);
      if (!result.ok) {
        if (result.conflict) {
          conflict = {
            port: result.conflict.port,
            pid: result.conflict.pid,
            command: result.conflict.command,
          };
          projects = projects.map((p) =>
            p.id === id ? { ...p, status: "conflict" } : p,
          );
        } else if (result.message) {
          alert(result.message);
        }
        return;
      }
      selectedProjectId = id;
      await refreshAll();
    } catch (e) {
      alert(String(e));
    }
  }

  async function handleStop(id: string) {
    try {
      await stopDev(id);
      await refreshAll();
    } catch (e) {
      alert(String(e));
    }
  }

  async function handleOpen(project: Project) {
    const url = project.openUrl ?? (project.port ? `http://localhost:${project.port}` : null);
    if (url) await openUrl(url);
  }

  function openAddForm() {
    editingProject = null;
    formOpen = true;
    activeTab = "projects";
  }

  function openEditForm(project: Project) {
    editingProject = project;
    formOpen = true;
  }

  async function handleSaveProject(project: Project) {
    await upsertProject(project);
    await mergeProjectStatus();
  }

  async function handleDeleteProject(id: string) {
    const target = projects.find((p) => p.id === id);
    if (!target) return;
    if (!confirm(`确认删除项目「${target.name}」？`)) return;
    await removeProject(id);
    if (selectedProjectId === id) selectedProjectId = undefined;
    await mergeProjectStatus();
  }

  onMount(() => {
    let timer: ReturnType<typeof setInterval> | undefined;
    const unsubs: Array<() => void> = [];

    (async () => {
      try {
        settings = await loadSettings();
        await refreshAll();

        timer = setInterval(() => {
          if (settings.autoRefresh) refreshAll();
        }, 5000);

        unsubs.push(
          await listen("dev-log", (e) => {
            const payload = e.payload as { id: string; line: string };
            if (payload.id !== selectedProjectId) return;
            logs = [
              ...logs,
              { time: "", text: payload.line, level: classifyLog(payload.line) },
            ].slice(-2000);
          }),
        );
        unsubs.push(
          await listen("dev-status", () => {
            mergeProjectStatus();
          }),
        );
        unsubs.push(
          await listen("open-project-form", () => {
            openAddForm();
          }),
        );
        unsubs.push(
          await listen("trigger-refresh", () => {
            refreshAll();
          }),
        );
      } catch {
        // browser preview
      }
    })();

    return () => {
      if (timer) clearInterval(timer);
      unsubs.forEach((u) => u());
    };
  });

  async function handleSettingsChange(next: Settings) {
    settings = next;
    try {
      await updateSettings(next);
      await refreshPorts();
    } catch (e) {
      alert(String(e));
    }
  }

  async function handleExportConfig() {
    try {
      const json = await exportConfig();
      const blob = new Blob([json], { type: "application/json" });
      const url = URL.createObjectURL(blob);
      const stamp = new Date().toISOString().slice(0, 10);
      const a = document.createElement("a");
      a.href = url;
      a.download = `quick-dev-config-${stamp}.json`;
      a.click();
      URL.revokeObjectURL(url);
    } catch (e) {
      alert(String(e));
    }
  }

  async function handleImportConfig(file: File) {
    if (!confirm("导入将覆盖当前项目与设置，是否继续？")) return;
    try {
      const json = await file.text();
      await importConfig(json);
      settings = await loadSettings();
      await refreshAll();
      alert("导入成功");
    } catch (e) {
      alert(String(e));
    }
  }
</script>

<div class="h-screen flex flex-col bg-qd-window text-qd-body select-none">
  <header class="h-10 flex items-center justify-center px-4 border-b border-qd-border bg-qd-window shrink-0 relative">
    <img src="/icon/logo.png" alt="即栈" class="absolute left-4 h-6 w-auto" />
    <span class="text-xs font-medium text-qd-muted tracking-wide">即栈 Quick Dev</span>
  </header>

  <nav class="flex gap-0.5 px-3.5 pt-2 border-b border-qd-border shrink-0">
    {#each tabs as tab}
      <button
        class="tab-btn text-[13px] font-medium px-3.5 py-2 rounded-t-md relative transition-colors"
        class:active={activeTab === tab.id}
        onclick={() => (activeTab = tab.id)}
      >
        {tab.label}
        {#if tab.badge}
          <span class="ml-1 text-[10px] font-semibold px-1 py-px rounded bg-qd-danger-soft text-qd-danger align-middle">
            {tab.badge}
          </span>
        {/if}
      </button>
    {/each}
  </nav>

  <main class="flex-1 overflow-hidden min-h-0">
    {#if activeTab === "ports"}
      <PortsPanel
        {ports}
        {lastRefresh}
        loading={portsLoading}
        onrefresh={refreshPorts}
        onkill={handleKill}
        onstopall={async () => { await stopAllDevs(); await refreshAll(); }}
        onopenport={async (port) => { await openUrl(`http://localhost:${port}`); }}
        onstopproject={handleStop}
      />
    {:else if activeTab === "projects"}
      <ProjectsPanel
        {projects}
        {logs}
        {selectedProjectId}
        {conflict}
        onstart={handleStart}
        onstop={handleStop}
        onopen={handleOpen}
        onselect={async (id) => {
          selectedProjectId = id;
          logs = toLogLines(await getDevLogs(id));
        }}
        onadd={openAddForm}
        onedit={openEditForm}
        ondelete={handleDeleteProject}
        onkillconflict={handleKill}
      />
    {:else}
      <SettingsPanel
        {settings}
        onchange={handleSettingsChange}
        onexport={handleExportConfig}
        onimport={handleImportConfig}
      />
    {/if}
  </main>

  <footer class="h-7 flex items-center gap-2 px-4 text-[11px] text-qd-dim border-t border-qd-border shrink-0">
    <span
      class="w-1.5 h-1.5 rounded-full"
      class:bg-qd-danger={hasError}
      class:bg-qd-running={!hasError && runningCount > 0}
      class:animate-pulse={!hasError && runningCount > 0}
      class:bg-qd-dim={!hasError && runningCount === 0}
    ></span>
    {#if hasError}
      有项目异常退出
    {:else}
      监听 {ports.length} 个端口 · {runningCount} 个项目运行中
    {/if}
  </footer>

  <ProjectFormModal
    open={formOpen}
    editing={editingProject}
    projectCount={rawProjectCount}
    groups={settings.groups ?? []}
    onclose={() => (formOpen = false)}
    onsave={handleSaveProject}
  />
</div>

<style>
  .tab-btn {
    color: var(--color-qd-muted);
    background: transparent;
    border: none;
    cursor: pointer;
  }
  .tab-btn:hover { color: var(--color-qd-body); }
  .tab-btn.active {
    color: var(--color-qd-ink);
    background: var(--color-qd-surface);
  }
  .tab-btn.active::after {
    content: "";
    position: absolute;
    bottom: -1px;
    left: 0;
    right: 0;
    height: 2px;
    background: var(--color-qd-brand);
    border-radius: 2px 2px 0 0;
  }
</style>
