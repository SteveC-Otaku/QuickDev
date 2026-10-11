<script lang="ts">
  import ActionIcon from "$lib/components/ActionIcon.svelte";
  import { onMount } from "svelte";
  import ProjectFormModal from "$lib/components/ProjectFormModal.svelte";
  import PortsPanel from "$lib/components/PortsPanel.svelte";
  import ProjectsPanel from "$lib/components/ProjectsPanel.svelte";
  import SettingsPanel from "$lib/components/SettingsPanel.svelte";
  import Feedback from "$lib/components/Feedback.svelte";
  import OperationStatus from "$lib/components/OperationStatus.svelte";
  import { app } from "$lib/store.svelte";
  import { openDataDirectory } from "$lib/ipc";
  import { updater } from "$lib/update.svelte";

  onMount(() => {
    void app.boot();
    void updater.init();
    return () => app.dispose();
  });
</script>

<div class="h-dvh flex flex-col bg-qd-window text-qd-body select-none relative">
  {#if app.booting}
    <div class="qd-boot-overlay" role="status" aria-live="polite">
      <div class="qd-boot-card">
        <img src="/icon/logo.png" alt="" class="qd-boot-logo" />
        <div class="qd-boot-title">即栈</div>
        <div class="qd-boot-spinner" aria-hidden="true"></div>
        <div class="qd-boot-hint">{app.bootHint || "加载中…"}</div>
      </div>
    </div>
  {/if}

  <header class="h-10 flex items-center justify-center px-4 border-b border-qd-border bg-qd-window shrink-0 relative">
    <img src="/icon/logo.png" alt="即栈" class="absolute left-4 h-6 w-auto" />
    <span class="text-xs font-medium text-qd-muted tracking-wide">即栈 Quick Dev</span>
  </header>

  {#if app.preview}
    <div class="qd-inline-message" role="status">浏览器预览 · 端口扫描、进程管理和配置保存需在桌面应用中使用</div>
  {:else if app.bootError}
    <div class="qd-inline-message qd-inline-error" role="alert"><span>{app.bootError}</span><button class="qd-btn qd-btn-secondary" onclick={() => app.boot()}>重试</button></div>
  {/if}
  <nav aria-label="主导航" class="flex gap-0.5 px-3.5 pt-2 border-b border-qd-border shrink-0">
    {#each app.tabs as tab}
      <button
        class="tab-btn text-[13px] font-medium px-3.5 py-2 rounded-t-md relative transition-colors"
        class:active={app.activeTab === tab.id}
        aria-current={app.activeTab === tab.id ? "page" : undefined}
        onclick={() => (app.activeTab = tab.id)}
      >
        <ActionIcon name={tab.id === "ports" ? "ports" : tab.id === "projects" ? "folder" : "settings"} size={16} /> <span>{tab.label}</span>
        {#if tab.badge}
          <span class="ml-1 text-[10px] font-semibold px-1 py-px rounded bg-qd-danger-soft text-qd-danger align-middle">
            {tab.badge}
          </span>
        {/if}
      </button>
    {/each}
  </nav>

  <main class="flex-1 overflow-hidden min-h-0" inert={!!app.exitMessage || app.booting || !!app.bootError || app.preview || !!app.confirmation || app.formOpen || updater.dialogOpen}>
    {#if app.activeTab === "ports"}
      <PortsPanel
        focusTarget={app.portFocus}
        onclearfocus={()=>app.portFocus=undefined}
        ports={app.ports}
        projects={app.projects}
        lastRefresh={app.lastRefresh}
        loading={app.portsLoading}
        error={app.portsError}
        runningCount={app.runningCount}
        stopAllBusy={app.stopAllBusy}
        stoppingIds={[...new Set([...app.stoppingIds, ...app.projects.filter(p => p.status === "stopping").map(p => p.id)])]}
        killBusy={app.killBusy}
        killingPid={app.killingPid}
        onrefresh={() => app.refreshPorts()}
        onkill={(pid) => app.handleKill(pid)}
        onstopall={() => app.handleStopAll()}
        onopenport={(port) => app.handleOpenPort(port)}
        onstopproject={(id) => app.handleStop(id)}
      />
    {:else if app.activeTab === "projects"}
      <ProjectsPanel
        projects={app.projects}
        logs={app.logs}
        selectedProjectId={app.selectedProjectId}
        selectedServiceId={app.selectedServiceId}
        serviceBusy={app.serviceBusy}
        onservice={(id,service,kind) => app.handleService(id,service,kind)}
        onopenservice={(p,service) => app.openService(p.id,service)}
        onlogservice={(id,service) => app.openServiceLogs(id,service)}
        onfilterlogs={(service) => app.selectLogService(service)}
        conflict={app.conflict}
        killBusy={app.killBusy}
        startingId={app.startingId}
        stoppingIds={[...new Set([...app.stoppingIds, ...app.projects.filter(p => p.status === "stopping").map(p => p.id)])]}
        stopAllBusy={app.stopAllBusy}
        logsLoading={app.logsLoading}
        logHeight={app.logHeight}
        onstart={(id) => app.handleStart(id)}
        onstop={(id) => app.handleStop(id)}
        onopen={(p) => app.handleOpen(p)}
        onselect={(id) => app.selectProject(id)}
        onadd={() => app.openAddForm()}
        onedit={(p) => app.openEditForm(p)}
        ondelete={(id) => app.handleDeleteProject(id)}
        onkillconflict={(pid) => app.handleKill(pid)}
        onlogheight={(h) => app.setLogHeight(h)}
        onplanstart={(id,plan)=>app.handleStart(id,plan)}
        onplanstop={(id,plan)=>app.handleStop(id,plan)}
        oninspectconflict={(port,pid)=>app.inspectPort(port,pid)}
      />
    {:else}
      <SettingsPanel
        settings={app.settings}
        busy={app.settingsBusy}
        configBusy={app.configBusy}
        onchange={(s) => app.handleSettingsChange(s)}
        onexport={() => app.handleExportConfig()}
        onimport={(f) => app.handleImportConfig(f)}
        onopendirectory={(kind) => { void openDataDirectory(kind).catch(e => app.notify(`打开目录失败：${String(e)}`)); }}
      />
    {/if}
  </main>

  <footer class="h-7 flex items-center gap-2 px-4 text-[11px] text-qd-dim border-t border-qd-border shrink-0">
    {#if app.operationText}
      <OperationStatus text={app.operationText} startedAt={app.operationStartedAt} operationId={app.operationId} />
    {:else}
    <span
      class="w-1.5 h-1.5 rounded-full"
      class:bg-qd-danger={app.hasError}
      class:bg-qd-running={!app.hasError && app.runningCount > 0}
      class:bg-qd-dim={!app.hasError && app.runningCount === 0}
    ></span>
    {#if app.portsLoading && app.activeTab === "ports"}
      <span class="qd-spinner" aria-hidden="true"></span>
      <span>刷新端口…</span>
    {:else if app.hasError}
      有服务异常，请查看项目详情
    {:else}
      监听 {app.ports.length} 个端口 · {app.runningCount} 个项目 · {app.runningServiceCount} 个服务运行 · {app.readyServiceCount} 个就绪
    {/if}
    {/if}
  </footer>

  <Feedback />

  <ProjectFormModal
    open={app.formOpen}
    editing={app.editingProject}
    focusServiceId={app.editingServiceId}
    projectCount={app.projects.length}
    groups={app.settings.groups ?? []}
    onclose={() => (app.formOpen = false)}
    onsave={(p) => app.handleSaveProject(p)}
  />
</div>

<style>
  .tab-btn {
    display: inline-flex; align-items: center; gap: 6px;
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
