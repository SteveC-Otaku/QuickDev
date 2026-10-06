<script lang="ts">
  import { fly } from "svelte/transition";
  import ActionIcon from "./ActionIcon.svelte";
  import ServicePanel from "./ServicePanel.svelte";
  import LaunchPlans from './LaunchPlans.svelte';
  import { projectServices, isDesktopService } from "../services";
  import LogPanel from "./LogPanel.svelte";
  import type { LogLine, Project } from "../types";

  const UNGROUPED = "未分组";

  interface Props {
    projects: Project[];
    selectedServiceId?: string;
    serviceBusy?: string[];
    onservice?: (id: string, service: string, kind: 'start' | 'stop' | 'restart') => void;
    onopenservice?: (project: Project, service: string) => void;
    onlogservice?: (project: string, service: string) => void;
    onfilterlogs?: (service?: string) => void;
    logs: LogLine[];
    selectedProjectId?: string;
    conflict?: { port: number; pid: number; command: string; projectId?: string } | null;
    killBusy?: boolean;
    startingId?: string | null;
    logHeight?: number;
    stoppingIds?: string[];
    stopAllBusy?: boolean;
    logsLoading?: boolean;
    onstart?: (id: string) => void;
    onstop?: (id: string) => void;
    onopen?: (project: Project) => void;
    onselect?: (id: string) => void;
    onadd?: () => void;
    onedit?: (project: Project) => void;
    ondelete?: (id: string) => void;
    onkillconflict?: (pid: number) => void;
    onlogheight?: (h: number) => void;
    onplanstart?: (id:string,planId:string)=>void;
    onplanstop?: (id:string,planId:string)=>void;
    oninspectconflict?: (port:number,pid:number)=>void;
  }

  let {
    projects, selectedServiceId, serviceBusy = [], onservice, onopenservice, onlogservice, onfilterlogs,
    logs,
    selectedProjectId,
    conflict = null,
    killBusy = false,
    startingId = null,
    logHeight = 160,
    stoppingIds = [],
    stopAllBusy = false,
    logsLoading = false,
    onstart,
    onstop,
    onopen,
    onselect,
    onadd,
    onedit,
    ondelete,
    onkillconflict,
    onlogheight,
    onplanstart,onplanstop,oninspectconflict,
  }: Props = $props();

  let groupFilter = $state<string>("all");

  let query = $state("");
  let expanded = $state<string[]>([]);
  let panelHeight = $state(500);
  const maxLogHeight = $derived(Math.max(100, Math.min(420, panelHeight - 180)));
  const effectiveLogHeight = $derived(Math.min(logHeight, maxLogHeight));
  $effect(() => { if (!groupFilters.includes(groupFilter)) groupFilter = "all"; });

  const statusLabel: Record<string, string> = {
    running: "运行中",
    stopped: "已停止",
    conflict: "端口冲突",
    starting: "启动中",
    stopping: "停止中",
    crashed: "异常退出", partial: "部分运行", failed: "启动失败", rollingBack: "撤回启动中",
  };

  const groupFilters = $derived.by(() => {
    const names = new Set<string>();
    for (const p of projects) {
      names.add(p.group?.trim() || UNGROUPED);
    }
    return ["all", ...[...names].sort((a, b) => {
      if (a === UNGROUPED) return 1;
      if (b === UNGROUPED) return -1;
      return a.localeCompare(b, "zh");
    })];
  });

  const filteredProjects = $derived.by(() => {
    return projects.filter((p) => (groupFilter === "all" || (p.group?.trim() || UNGROUPED) === groupFilter) && `${p.name} ${p.cwd} ${p.command} ${projectServices(p).map(service => service.name).join(' ')}`.toLowerCase().includes(query.trim().toLowerCase()));
  });

  const groupedProjects = $derived.by(() => {
    const map = new Map<string, Project[]>();
    for (const p of filteredProjects) {
      const g = p.group?.trim() || UNGROUPED;
      if (!map.has(g)) map.set(g, []);
      map.get(g)!.push(p);
    }
    return [...map.entries()].sort(([a], [b]) => {
      if (a === UNGROUPED) return 1;
      if (b === UNGROUPED) return -1;
      return a.localeCompare(b, "zh");
    });
  });

  const showGroupHeaders = $derived(groupFilter === "all" && groupedProjects.length > 1);

  function playTitle(status?: string) {
    if (status === "conflict") return "重新检测并启动";
    if (status === "crashed") return "重新启动";
    return "启动";
  }
</script>

<div class="flex flex-col h-full overflow-hidden" bind:clientHeight={panelHeight}>
  {#if conflict}
    <div
      class="qd-conflict-banner mx-4 mt-2 mb-1 px-3 py-2.5 rounded-lg bg-qd-danger-soft border border-[rgba(240,101,101,0.25)] flex items-center gap-2.5 text-xs shrink-0"
      class:qd-conflict-busy={killBusy}
      transition:fly={{ y: -6, duration: typeof window !== "undefined" && window.matchMedia("(prefers-reduced-motion: reduce)").matches ? 0 : 180, opacity: 0 }}
      role="alert"
    >
      <ActionIcon name="alert" class="text-qd-warn" />
      <span class="flex-1 text-qd-body min-w-0">
        <strong class="text-qd-danger font-semibold">端口 :{conflict.port} 已被占用</strong>
        <span class="text-qd-muted"> — PID {conflict.pid} · {conflict.command}</span>
      </span>
      <button
        class="qd-btn qd-btn-secondary qd-btn-sm shrink-0"
        onclick={() => oninspectconflict?.(conflict.port,conflict.pid)}
      >查看占用详情</button>
      <button
        class="qd-btn qd-btn-danger qd-btn-sm qd-btn-icon shrink-0"
        title={killBusy ? "正在结束进程" : "Kill 占用进程"}
        disabled={killBusy}
        onclick={() => onkillconflict?.(conflict.pid)}
      >
        {#if killBusy}
          <span class="qd-spinner qd-spinner-light" aria-hidden="true"></span>
          <span>结束中…</span>
        {:else}
          <ActionIcon name="kill" />
          <span>结束并启动</span>
        {/if}
      </button>
    </div>
  {/if}

  <div class="flex items-center justify-between px-4 py-3 shrink-0">
    <span class="text-[13px] font-semibold text-qd-ink tracking-wide">我的项目</span>
    <button class="qd-btn qd-btn-primary qd-btn-icon" onclick={() => onadd?.()}>
      <ActionIcon name="plus" size={16} /> 添加项目
    </button>
  </div>

  {#if projects.length > 0}<div class="qd-filter-row"><input class="qd-input" aria-label="搜索项目" bind:value={query} placeholder="搜索项目或服务名称、路径或命令…" /></div>{/if}
  {#if projects.length > 0 && groupFilters.length > 2}
    <div class="flex flex-wrap gap-1.5 px-4 pb-2 shrink-0">
      {#each groupFilters as g}
        <button
          type="button"
          class="text-[11px] font-medium px-2.5 py-1 rounded-md border transition-colors"
          class:bg-qd-brand-soft={groupFilter === g}
          class:border-qd-brand={groupFilter === g}
          class:text-qd-brand={groupFilter === g}
          class:border-qd-border={groupFilter !== g}
          class:text-qd-muted={groupFilter !== g}
          onclick={() => (groupFilter = g)}
        >
          {g === "all" ? "全部" : g}
        </button>
      {/each}
    </div>
  {/if}

  <div class="flex-1 overflow-y-auto px-4 pb-2 flex flex-col gap-2 min-h-0">
    {#if projects.length === 0}
      <div class="text-center text-qd-dim text-xs py-8">添加第一个项目，保存路径和命令后即可一键启停。</div>
    {:else if filteredProjects.length === 0}
      <div class="text-center text-qd-dim text-xs py-8">没有匹配的项目，可调整搜索或分组</div>
    {:else}
      {#each groupedProjects as [groupName, items]}
        {#if showGroupHeaders}
          <div class="text-[10px] font-semibold text-qd-muted uppercase tracking-wider px-1 pt-1">{groupName}</div>
        {/if}
        {#each items as project}
          {@const serviceNames = projectServices(project).map(service => service.name).join(' / ')}
          {@const entryId = project.launchPlans?.find(p=>p.id===project.defaultPlanId)?.openServiceId ?? (project.launchPlans?.length===1 ? project.launchPlans[0].openServiceId : undefined)}
          {@const entryService = projectServices(project).find(s=>s.id===entryId) ?? projectServices(project)[0]}
          {@const live = project.status === "running" || project.status === "partial" || project.status === "starting" || project.status === "rollingBack" || project.status === "stopping"}
          <article
            class="rounded-[10px] border border-qd-border bg-qd-surface p-3 transition-colors cursor-pointer"
            class:border-[rgba(123,140,255,0.22)]={live}
            class:bg-qd-danger-soft={project.status === "conflict"}
            class:border-[rgba(240,101,101,0.35)]={project.status === "conflict"}
            class:ring-1={selectedProjectId === project.id}
            class:ring-qd-brand={selectedProjectId === project.id}
          >
            <button type="button" class="flex items-center gap-2.5 mb-2 w-full text-left" aria-label={`查看 ${project.name} · ${serviceNames} 的日志`} aria-pressed={selectedProjectId === project.id} onclick={() => onselect?.(project.id)}>
              <div class="w-8 h-8 rounded-md bg-qd-elevated border border-qd-border flex items-center justify-center text-sm shrink-0">
                {#if project.icon && project.icon !== "📁"}{project.icon}{:else}<ActionIcon name="folder" class="text-qd-brand" size={20} />{/if}
              </div>
              <div class="flex-1 min-w-0">
                <div class="flex items-center gap-2 min-w-0">
                  <div class="text-[13px] font-semibold text-qd-ink truncate" title={`${project.name} · ${serviceNames}`}>{project.name}<span class="text-qd-dim font-normal mx-1.5">·</span><span class="text-qd-brand">{serviceNames}</span></div>
                  {#if project.group}
                    <span class="text-[10px] px-1.5 py-px rounded bg-qd-elevated text-qd-muted border border-qd-border shrink-0">{project.group}</span>
                  {/if}
                </div>
                <div class="text-[11px] text-qd-dim font-mono truncate mt-0.5">
                  {project.cwd} · {project.command}
                </div>
              </div>
              <span
                class="w-[7px] h-[7px] rounded-full shrink-0"
                class:bg-qd-running={project.status === "running"}
                class:shadow-[0_0_6px_var(--color-qd-running)]={project.status === "running"}
                class:bg-qd-warn={project.status === "starting" || project.status === "partial" || project.status === "rollingBack" || project.status === "stopping"}
                class:animate-pulse={project.status === "starting"}
                class:bg-qd-dim={project.status === "stopped"}
                class:bg-qd-danger={project.status === "conflict" || project.status === "crashed"}
              ></span>
              <span
                class="text-[11px] font-medium"
                class:text-qd-running={project.status === "running"}
                class:text-qd-warn={project.status === "starting" || project.status === "partial" || project.status === "rollingBack" || project.status === "stopping"}
                class:text-qd-danger={project.status === "conflict" || project.status === "crashed"}
                class:text-qd-muted={project.status === "stopped"}
              >
                {startingId === project.id ? "正在启动…" : stoppingIds.includes(project.id) || stopAllBusy && live ? "正在停止…" : statusLabel[project.status ?? "stopped"]}
              </span>
            </button>
            <div class="flex items-center gap-1.5">
              {#if project.port}
                <span class="font-mono text-xs font-medium text-qd-brand mr-1">:{project.port}</span>
              {/if}
              {#if project.openUrl && project.openUrl !== `http://localhost:${project.port ?? ""}`}
                <span class="text-[10px] text-qd-dim truncate max-w-[120px]" title={project.openUrl}><ActionIcon name="open" size={12} /> 已识别 URL</span>
              {/if}
              <button class="qd-btn qd-btn-ghost qd-btn-sm qd-btn-icon" title={live ? "停止项目后可编辑" : "编辑"} disabled={live || !!project.operation || serviceBusy.some(key => key.startsWith(`${project.id}/`))} onclick={() => onedit?.(project)}>
                <ActionIcon name="edit" />
              </button>
              {#if !live}
                <button class="qd-btn qd-btn-ghost qd-btn-sm qd-btn-icon text-qd-danger" title="删除" disabled={!!project.operation || serviceBusy.some(key => key.startsWith(`${project.id}/`))} onclick={() => ondelete?.(project.id)}>
                  <ActionIcon name="delete" />
                </button>
              {/if}
              {#if project.status === "partial" && !project.operation}
                <button class="qd-btn qd-btn-play qd-btn-sm" disabled={stopAllBusy || !!startingId} onclick={() => onstart?.(project.id)}><ActionIcon name="play" size={14} /> 补齐启动</button>
              {/if}
              {#if live}
                {#if project.status === "running"}
                  <button class="qd-btn qd-btn-secondary qd-btn-sm qd-btn-icon" title={isDesktopService(entryService) ? "显示 App 窗口" : "浏览器打开"} onclick={() => entryId && onopenservice ? onopenservice(project,entryId) : onopen?.(project)}>
                    <ActionIcon name={isDesktopService(entryService) ? "app" : "open"} />
                  </button>
                {/if}
                <button class="qd-btn qd-btn-secondary qd-btn-sm qd-btn-icon" title="查看日志" onclick={() => onselect?.(project.id)}>
                  <ActionIcon name="log" />
                </button>
                <button class="qd-btn qd-btn-icon qd-btn-stop qd-btn-sm" title={project.status === "starting" ? "取消启动" : "停止项目"} disabled={stopAllBusy || project.status === "stopping" || stoppingIds.includes(project.id)} onclick={() => onstop?.(project.id)}>
                  {#if stopAllBusy || project.status === "stopping" || stoppingIds.includes(project.id)}<span class="qd-spinner" aria-hidden="true"></span><span>停止中…</span>{:else}<ActionIcon name="stop" />{/if}
                </button>
              {:else}
                <button
                  class="qd-btn qd-btn-icon qd-btn-play qd-btn-sm"
                  title={project.defaultPlanId ? `启动默认方案：${project.launchPlans?.find(p=>p.id===project.defaultPlanId)?.name ?? ''}` : playTitle(project.status)}
                  disabled={serviceBusy.some(key => key.startsWith(`${project.id}/`)) || !!startingId || stopAllBusy || project.status === "stopping" || stoppingIds.includes(project.id)}
                  onclick={() => onstart?.(project.id)}
                >
                  {#if startingId === project.id}
                    <span class="qd-spinner" aria-hidden="true"></span><span>启动中…</span>
                  {:else}
                    <ActionIcon name="play" />
                  {/if}
                </button>
              {/if}
            </div>
            {#if (project.launchPlans?.length ?? 0) > 1}<LaunchPlans {project} busy={!!project.operation || !!startingId || stopAllBusy || stoppingIds.includes(project.id) || serviceBusy.some(key=>key.startsWith(`${project.id}/`))} onstart={plan=>onplanstart?.(project.id,plan)} onstop={plan=>onplanstop?.(project.id,plan)} onopen={service=>onopenservice?.(project,service)}/>{/if}
            {#if projectServices(project).length===1 && project.serviceStatuses?.some(s=>s.message)}<p class="text-[11px] text-qd-danger mt-2" role="alert">{project.serviceStatuses?.find(s=>s.message)?.message}</p>{/if}
            {#if projectServices(project).length > 1}
              <button class="qd-btn qd-btn-ghost qd-btn-sm mt-2" aria-expanded={expanded.includes(project.id)} onclick={() => expanded = expanded.includes(project.id) ? expanded.filter(id => id !== project.id) : [...expanded,project.id]}>
                <ActionIcon name={expanded.includes(project.id) ? 'up' : 'down'} size={14} /> {expanded.includes(project.id) ? '收起' : '展开'} {projectServices(project).length} 个服务 · {project.serviceStatuses?.filter(s => s.status === 'running').length ?? 0} 运行{#if project.serviceStatuses?.some(s => s.message)} · 有异常{/if}
              </button>
              {#if expanded.includes(project.id)}<ServicePanel {project} busy={serviceBusy} onaction={onservice} onopen={onopenservice} onlogs={onlogservice} />{/if}
            {/if}
          </article>
        {/each}
      {/each}
    {/if}
  </div>

  {#if selectedProjectId}
    {@const selected = projects.find(p => p.id === selectedProjectId)}
    {#if selected && projectServices(selected).length > 1}
      <div class="flex flex-wrap gap-1 px-4 py-1.5 border-t border-qd-border shrink-0" aria-label="日志服务筛选">
        <button class="qd-btn qd-btn-sm" class:qd-btn-primary={!selectedServiceId} aria-pressed={!selectedServiceId} onclick={() => onfilterlogs?.()}>全部服务</button>
        {#each projectServices(selected) as service}
          <button class="qd-btn qd-btn-sm" class:qd-btn-primary={selectedServiceId === service.id} aria-pressed={selectedServiceId === service.id} onclick={() => onfilterlogs?.(service.id)}>{service.name}</button>
        {/each}
      </div>
    {/if}
  {/if}
  <LogPanel
    {logs}
    loading={logsLoading}
    projectId={selectedProjectId}
    title={`${(projects.find((p) => p.id === selectedProjectId)?.name) ?? "日志"} — 日志`}
    height={effectiveLogHeight}
    maxHeight={maxLogHeight}
    onheight={onlogheight}
  />
</div>
