<script lang="ts">
  import ActionIcon from "./ActionIcon.svelte";
  import type { LogLine, Project } from "../types";

  const UNGROUPED = "未分组";

  interface Props {
    projects: Project[];
    logs: LogLine[];
    selectedProjectId?: string;
    conflict?: { port: number; pid: number; command: string } | null;
    onstart?: (id: string) => void;
    onstop?: (id: string) => void;
    onopen?: (project: Project) => void;
    onselect?: (id: string) => void;
    onadd?: () => void;
    onedit?: (project: Project) => void;
    ondelete?: (id: string) => void;
    onkillconflict?: (pid: number) => void;
  }

  let {
    projects,
    logs,
    selectedProjectId,
    conflict = null,
    onstart,
    onstop,
    onopen,
    onselect,
    onadd,
    onedit,
    ondelete,
    onkillconflict,
  }: Props = $props();

  let groupFilter = $state<string>("all");

  const statusLabel: Record<string, string> = {
    running: "运行中",
    stopped: "已停止",
    conflict: "端口冲突",
    starting: "启动中",
    crashed: "异常退出",
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
    if (groupFilter === "all") return projects;
    return projects.filter((p) => (p.group?.trim() || UNGROUPED) === groupFilter);
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
</script>

<div class="flex flex-col h-full overflow-hidden">
  {#if conflict}
    <div class="mx-4 mt-2 mb-1 px-3 py-2.5 rounded-lg bg-qd-danger-soft border border-[rgba(240,101,101,0.25)] flex items-center gap-2.5 text-xs shrink-0">
      <span class="text-base">⚠</span>
      <span class="flex-1 text-qd-body">
        <strong class="text-qd-danger font-semibold">端口 :{conflict.port} 已被占用</strong>
        — PID {conflict.pid} · {conflict.command}
      </span>
      <button class="qd-btn qd-btn-danger qd-btn-sm qd-btn-icon" title="Kill 占用进程" onclick={() => onkillconflict?.(conflict.pid)}>
        <ActionIcon name="kill" />
        <span>Kill 占用者</span>
      </button>
    </div>
  {/if}

  <div class="flex items-center justify-between px-4 py-3 shrink-0">
    <span class="text-[13px] font-semibold text-qd-ink tracking-wide">我的项目</span>
    <button class="qd-btn qd-btn-primary" onclick={() => onadd?.()}>
      + 添加项目
    </button>
  </div>

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
      <div class="text-center text-qd-dim text-xs py-8">暂无项目，点击右上角添加</div>
    {:else if filteredProjects.length === 0}
      <div class="text-center text-qd-dim text-xs py-8">该分组下暂无项目</div>
    {:else}
      {#each groupedProjects as [groupName, items]}
        {#if showGroupHeaders}
          <div class="text-[10px] font-semibold text-qd-muted uppercase tracking-wider px-1 pt-1">{groupName}</div>
        {/if}
        {#each items as project}
          <div
            role="button"
            tabindex="0"
            class="rounded-[10px] border border-qd-border bg-qd-surface p-3 transition-colors cursor-pointer"
            class:border-[rgba(123,140,255,0.22)]={project.status === "running" || project.status === "starting"}
            class:bg-qd-danger-soft={project.status === "conflict"}
            class:border-[rgba(240,101,101,0.35)]={project.status === "conflict"}
            class:ring-1={selectedProjectId === project.id}
            class:ring-qd-brand={selectedProjectId === project.id}
            onclick={() => onselect?.(project.id)}
            onkeydown={(e) => e.key === "Enter" && onselect?.(project.id)}
          >
            <div class="flex items-center gap-2.5 mb-2">
              <div class="w-8 h-8 rounded-md bg-qd-elevated border border-qd-border flex items-center justify-center text-sm shrink-0">
                {project.icon ?? "📁"}
              </div>
              <div class="flex-1 min-w-0">
                <div class="flex items-center gap-2">
                  <div class="text-[13px] font-semibold text-qd-ink">{project.name}</div>
                  {#if project.group}
                    <span class="text-[10px] px-1.5 py-px rounded bg-qd-elevated text-qd-muted border border-qd-border">{project.group}</span>
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
                class:bg-qd-warn={project.status === "starting"}
                class:animate-pulse={project.status === "starting"}
                class:bg-qd-dim={project.status === "stopped"}
                class:bg-qd-danger={project.status === "conflict" || project.status === "crashed"}
              ></span>
              <span
                class="text-[11px] font-medium"
                class:text-qd-running={project.status === "running"}
                class:text-qd-warn={project.status === "starting"}
                class:text-qd-danger={project.status === "conflict" || project.status === "crashed"}
                class:text-qd-muted={project.status === "stopped"}
              >
                {statusLabel[project.status ?? "stopped"]}
              </span>
            </div>
            <div class="flex items-center gap-1.5" role="presentation" onclick={(e) => e.stopPropagation()}>
              {#if project.port}
                <span class="font-mono text-xs font-medium text-qd-brand mr-1">:{project.port}</span>
              {/if}
              {#if project.openUrl && project.openUrl !== `http://localhost:${project.port ?? ""}`}
                <span class="text-[10px] text-qd-dim truncate max-w-[120px]" title={project.openUrl}>↗ 已识别 URL</span>
              {/if}
              <button class="qd-btn qd-btn-ghost qd-btn-sm qd-btn-icon" title="编辑" onclick={() => onedit?.(project)}>
                <ActionIcon name="edit" />
              </button>
              {#if project.status !== "running"}
                <button class="qd-btn qd-btn-ghost qd-btn-sm qd-btn-icon text-qd-danger" title="删除" onclick={() => ondelete?.(project.id)}>
                  <ActionIcon name="delete" />
                </button>
              {/if}
              {#if project.status === "running"}
                <button class="qd-btn qd-btn-secondary qd-btn-sm qd-btn-icon" title="浏览器打开" onclick={() => onopen?.(project)}>
                  <ActionIcon name="open" />
                </button>
                <button class="qd-btn qd-btn-secondary qd-btn-sm qd-btn-icon" title="查看日志" onclick={() => onselect?.(project.id)}>
                  <ActionIcon name="log" />
                </button>
                <button class="qd-btn qd-btn-icon qd-btn-stop qd-btn-sm" title="停止" onclick={() => onstop?.(project.id)}>
                  <ActionIcon name="stop" />
                </button>
              {:else if project.status === "starting"}
                <button class="qd-btn qd-btn-secondary qd-btn-sm qd-btn-icon" title="查看日志" onclick={() => onselect?.(project.id)}>
                  <ActionIcon name="log" />
                </button>
                <button class="qd-btn qd-btn-icon qd-btn-stop qd-btn-sm" title="停止" onclick={() => onstop?.(project.id)}>
                  <ActionIcon name="stop" />
                </button>
              {:else if project.status === "conflict"}
                <button class="qd-btn qd-btn-icon qd-btn-play qd-btn-sm" title="强制启动" onclick={() => onstart?.(project.id)}>
                  <ActionIcon name="play" />
                </button>
              {:else if project.status === "crashed"}
                <button class="qd-btn qd-btn-icon qd-btn-play qd-btn-sm" title="重新启动" onclick={() => onstart?.(project.id)}>
                  <ActionIcon name="play" />
                </button>
              {:else}
                <button class="qd-btn qd-btn-icon qd-btn-play qd-btn-sm" title="启动" onclick={() => onstart?.(project.id)}>
                  <ActionIcon name="play" />
                </button>
              {/if}
            </div>
          </div>
        {/each}
      {/each}
    {/if}
  </div>

  <div class="h-[140px] shrink-0 border-t border-qd-border flex flex-col">
    <div class="flex items-center justify-between px-4 py-1.5 bg-qd-surface border-b border-qd-border/60 shrink-0">
      <span class="text-[11px] font-semibold text-qd-muted uppercase tracking-wider">
        {(projects.find((p) => p.id === selectedProjectId)?.name) ?? "日志"} — 日志
      </span>
    </div>
    <div class="flex-1 overflow-y-auto px-4 py-2 bg-qd-log-bg font-mono text-[11px] leading-relaxed">
      {#if logs.length === 0}
        <div class="text-qd-dim">选择项目或启动 dev 后显示日志</div>
      {/if}
      {#each logs as line}
        <div
          class="text-qd-dim"
          class:text-qd-info={line.level === "info"}
          class:text-qd-running={line.level === "success"}
          class:text-qd-warn={line.level === "warn"}
          class:text-qd-danger={line.level === "error"}
        >
          {#if line.time}<span class="opacity-60 mr-1.5">{line.time}</span>{/if}{line.text}
        </div>
      {/each}
    </div>
  </div>
</div>
