<script lang="ts">
  import ActionIcon from "./ActionIcon.svelte";
  import { untrack } from 'svelte';
  import type { PortEntry, Project } from "../types";
  import { projectServices, isDesktopService } from "../services";

  interface Props {
    ports: PortEntry[];
    projects?: Project[];
    lastRefresh?: string;
    loading?: boolean;
    error?: string;
    runningCount?: number;
    stopAllBusy?: boolean;
    stoppingIds?: string[];
    killBusy?: boolean;
    killingPid?: number | null;
    onrefresh?: () => void;
    onkill?: (pid: number) => void;
    onstopall?: () => void;
    onopenport?: (port: number) => void;
    onstopproject?: (projectId: string) => void;
    focusTarget?: {port:number;pid?:number;token:number};
    onclearfocus?: ()=>void;
  }

  let {
    ports,
    projects = [],
    lastRefresh = "—",
    loading = false,
    error = "",
    runningCount = 0,
    stopAllBusy = false,
    stoppingIds = [],
    killBusy = false,
    killingPid = null,
    onrefresh,
    onkill,
    onstopall,
    onopenport,
    onstopproject,
    focusTarget,onclearfocus,
  }: Props = $props();

  let menu = $state<{ x: number; y: number; entry: PortEntry } | null>(null);
  let query = $state("");
  let expanded = $state<string[]>([]);
  let onlyWatched = $state(false);
  let focusToken: number | undefined;
  $effect(()=>{
    if (!focusTarget) return;
    const target=focusTarget;
    if(focusToken!==target.token){focusToken=target.token; query=String(target.port); onlyWatched=false;}
    const matches=ports.filter(p=>p.port===target.port);
    untrack(()=>{expanded=[...new Set([...expanded,...matches.map(p=>`${p.port}/${p.pid}`)])];});
  });
  function focusRow(node: HTMLElement, port:number) {
    $effect(()=>{if(focusTarget?.port===port){node.scrollIntoView({block:'center'});}});
  }
  const shownPorts = $derived(ports.filter((p) => (!onlyWatched || p.highlight || p.projectId) && `${p.port} ${p.pid} ${p.process} ${p.command} ${p.projectName ?? ""} ${p.sourceApp ?? ''} ${p.cwd ?? ''} ${p.executable ?? ''}`.toLowerCase().includes(query.trim().toLowerCase())));
  let copiedHint = $state("");

  const projectPorts = $derived(shownPorts.filter((p) => !!p.projectName));
  const otherPorts = $derived(shownPorts.filter((p) => !p.projectName));
  function desktopEntry(entry: PortEntry) {
    const project = projects.find(p => p.id === entry.projectId);
    const service = project && projectServices(project).find(s => s.id === entry.serviceId);
    return !!service && isDesktopService(service);
  }
  function appReady(entry: PortEntry) {
    return projects.find(p => p.id === entry.projectId)?.serviceStatuses?.some(s => s.id === entry.serviceId && s.status === 'running') ?? false;
  }

  function openMenu(e: MouseEvent, entry: PortEntry) {
    e.preventDefault();
    menu = { x: Math.max(8, Math.min(e.clientX, window.innerWidth - 230)), y: Math.max(8, Math.min(e.clientY, window.innerHeight - 190)), entry };
  }

  function closeMenu() {
    menu = null;
  }

  async function copyText(text: string, label: string) {
    try {
      await navigator.clipboard.writeText(text);
      copiedHint = `已复制 ${label}`;
      setTimeout(() => (copiedHint = ""), 1500);
    } catch {
      copiedHint = "复制失败";
    }
    closeMenu();
  }
</script>

{#snippet sourceDetails(entry:PortEntry)}
  <div class="qd-port-source">
    <strong>{entry.sourceApp ? `${entry.sourceApp} 相关进程` : '监听进程来源'}</strong>
    <dl>
      <dt>进程</dt><dd>{entry.process} · PID {entry.pid}</dd>
      <dt>程序路径</dt><dd>{entry.executable || '权限不足或进程已退出，无法读取'}</dd>
      <dt>工作目录</dt><dd>{entry.cwd || '未能读取'}</dd>
      <dt>来源依据</dt><dd>{entry.sourceEvidence || '未识别上游软件；不能仅凭 node.exe 判断来源'}</dd>
      <dt>父进程链</dt><dd>{entry.parentChain?.join(' ← ') || '父进程已退出或无法读取'}</dd>
      <dt>启动命令</dt><dd>{entry.command}</dd>
    </dl>
  </div>
{/snippet}

{#snippet killBtn(entry: PortEntry)}
  <button
    class="qd-btn qd-btn-icon qd-btn-kill qd-btn-sm"
    title={killBusy && killingPid === entry.pid ? "正在结束…" : "Kill 进程"}
    disabled={killBusy}
    onclick={() => onkill?.(entry.pid)}
  >
    {#if killBusy && killingPid === entry.pid}
      <span class="qd-spinner qd-spinner-light" aria-hidden="true"></span>
    {:else}
      <ActionIcon name="kill" />
    {/if}
  </button>
{/snippet}

<svelte:window onclick={closeMenu} onkeydown={(e) => { if (e.key === "Escape") closeMenu(); }} />

<div class="flex flex-col h-full overflow-hidden">
  <div class="flex items-center justify-between px-4 py-3 shrink-0">
    <span class="text-[13px] font-semibold text-qd-ink tracking-wide">监听端口</span>
    <div class="flex items-center gap-2">
      {#if copiedHint}
        <span class="text-[11px] text-qd-running">{copiedHint}</span>
      {/if}
      <span class="text-[11px] text-qd-dim">{ports.length} 个 · {lastRefresh}</span>
      <button class="qd-btn qd-btn-secondary qd-btn-icon" title="刷新" disabled={loading} onclick={() => onrefresh?.()}>
        <ActionIcon name="refresh" />
        <span>{loading ? "刷新中…" : "刷新"}</span>
      </button>
      <button class="qd-btn qd-btn-icon qd-btn-stop" title="只停止即栈管理的项目" disabled={stopAllBusy || runningCount === 0 || stoppingIds.length > 0} onclick={() => onstopall?.()}>
        <ActionIcon name="stop" />
        {#if stopAllBusy}<span class="qd-spinner" aria-hidden="true"></span>{/if}<span>{stopAllBusy ? "停止中…" : "全部停止项目"}</span>
      </button>
    </div>
  </div>

  <div class="qd-filter-row">
    <input class="qd-input" aria-label="搜索端口、PID 或命令" bind:value={query} placeholder="搜索端口、进程、PID 或命令…" />
    <button class="qd-btn qd-btn-secondary" aria-pressed={onlyWatched} onclick={() => onlyWatched = !onlyWatched}>{onlyWatched ? "显示全部端口" : "只看项目与高亮"}</button>
  </div>
  {#if error}<div class="qd-inline-message qd-inline-error" role="alert">{error} · {ports.length > 0 ? "保留上次扫描结果" : "请点击刷新重试"}</div>{/if}
  <div class="flex-1 overflow-y-auto px-4 pb-2" aria-busy={loading}>
    {#if loading && ports.length === 0}
      <div class="qd-skeleton-list" aria-busy="true">
        {#each [1, 2, 3] as _}
          <div class="qd-skeleton-row"></div>
        {/each}
        <div class="text-center text-qd-dim text-xs py-3 flex items-center justify-center gap-2">
          <span class="qd-spinner"></span>
          正在扫描端口…
        </div>
      </div>
    {:else if !error && ports.length === 0}
      <div class="text-center text-qd-dim text-xs py-8">暂无监听端口，启动项目后可在这里查看</div>
    {/if}

    {#if focusTarget}<div class="qd-empty" role="status">正在查看端口 :{focusTarget.port}{#if focusTarget.pid} · 冲突时 PID {focusTarget.pid}{/if}<button class="qd-btn qd-btn-ghost qd-btn-sm" onclick={()=>{onclearfocus?.();query='';}}>显示全部端口</button>{#if !loading && !ports.some(p=>p.port===focusTarget!.port)}<p>该端口当前没有监听进程，可能已释放。</p>{/if}</div>{/if}
    {#if ports.length > 0 && shownPorts.length === 0}<div class="qd-empty">没有匹配的端口，试试其他关键词或显示全部端口。</div>{/if}
    {#if projectPorts.length > 0}
      <div class="qd-project-section-head">
        <span class="qd-project-section-dot"></span>
        <span>项目监听端口</span>
        <span class="qd-project-section-count">{projectPorts.length}</span>
      </div>
      <div class="flex flex-col gap-2 mb-3">
        {#each projectPorts as entry}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            class="qd-project-card cursor-context-menu"
            class:qd-port-focus={focusTarget?.port===entry.port}
            use:focusRow={entry.port}
            oncontextmenu={(e) => openMenu(e, entry)}
          >
            <div class="qd-project-card-inner">
              <div class="qd-project-port">:{entry.port}</div>
              <div class="qd-project-name">{entry.projectName}</div>
              <div class="qd-project-status">
                <span class="qd-project-status-dot"></span>
                {desktopEntry(entry) && !appReady(entry) ? 'App 启动中' : '运行中'}
              </div>
              <div class="qd-project-actions">
                <button class="qd-btn qd-btn-ghost qd-btn-sm" aria-expanded={expanded.includes(`${entry.port}/${entry.pid}`)} aria-label={`查看端口 ${entry.port} 的来源`} onclick={()=>{const key=`${entry.port}/${entry.pid}`;expanded=expanded.includes(key)?expanded.filter(k=>k!==key):[...expanded,key];}}>来源</button>
                <button class="qd-btn qd-btn-icon qd-btn-sm qd-project-btn-open" title={desktopEntry(entry) ? '显示 App 窗口' : '浏览器打开'} disabled={desktopEntry(entry) && !appReady(entry)} onclick={() => onopenport?.(entry.port)}>
                  <ActionIcon name={desktopEntry(entry) ? 'app' : 'open'} size={16} />
                </button>
                {#if entry.projectId}
                  <button class="qd-btn qd-btn-icon qd-btn-sm qd-btn-stop" title="停止项目" disabled={stopAllBusy || stoppingIds.includes(entry.projectId!)} onclick={() => onstopproject?.(entry.projectId!)}>
                    {#if stopAllBusy || stoppingIds.includes(entry.projectId!)}<span class="qd-spinner" aria-hidden="true"></span>{:else}<ActionIcon name="stop" size={16} />{/if}
                  </button>
                {/if}
              </div>
            </div>
          </div>
          {#if expanded.includes(`${entry.port}/${entry.pid}`)}{@render sourceDetails(entry)}{/if}
        {/each}
      </div>
    {/if}

    {#if otherPorts.length > 0}
      {#if projectPorts.length > 0}
        <div class="text-[10px] font-semibold text-qd-muted uppercase tracking-wider px-1 py-2 mt-1">其他端口</div>
      {/if}
      {#each otherPorts as entry}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="grid grid-cols-[72px_80px_64px_1fr_auto] gap-2 items-center px-2.5 py-2 rounded-md text-xs border border-transparent hover:bg-qd-surface transition-colors cursor-context-menu"
          class:qd-port-highlight={entry.highlight}
          class:qd-port-focus={focusTarget?.port===entry.port}
          use:focusRow={entry.port}
          oncontextmenu={(e) => openMenu(e, entry)}
        >
          <span class="font-mono font-medium text-sm text-qd-brand" class:text-qd-warn={entry.owned === false}>
            :{entry.port}
          </span>
          <span class="text-qd-body font-medium truncate" title={entry.process}>{entry.sourceApp ?? entry.process}</span>
          <span class="font-mono text-[11px] text-qd-muted">{entry.pid}</span>
          <span class="font-mono text-[11px] text-qd-dim truncate" title={entry.command}>{entry.command}</span>
          <div class="flex items-center gap-1.5 justify-end">
            <button class="qd-btn qd-btn-ghost qd-btn-sm" aria-expanded={expanded.includes(`${entry.port}/${entry.pid}`)} aria-label={`查看端口 ${entry.port} 的来源`} onclick={() => { const key = `${entry.port}/${entry.pid}`; expanded = expanded.includes(key) ? expanded.filter(k => k !== key) : [...expanded,key]; }}>来源</button>
            {#if entry.owned === false}
              <span class="qd-tag qd-tag-danger">外部占用</span>
            {:else}
              <button class="qd-btn qd-btn-secondary qd-btn-sm qd-btn-icon" title="浏览器打开" onclick={() => onopenport?.(entry.port)}>
                <ActionIcon name="open" />
              </button>
            {/if}
            {@render killBtn(entry)}
          </div>
        </div>
        {#if expanded.includes(`${entry.port}/${entry.pid}`)}
          {@render sourceDetails(entry)}
        {/if}
      {/each}
    {/if}
  </div>
</div>

{#if menu}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="ctx-menu"
    style:left="{menu.x}px"
    style:top="{menu.y}px"
    onclick={(e) => e.stopPropagation()}
  >
    <button type="button" onclick={() => copyText(String(menu!.entry.port), "端口")}>复制端口 :{menu.entry.port}</button>
    <button type="button" onclick={() => copyText(String(menu!.entry.pid), "PID")}>复制 PID {menu.entry.pid}</button>
    <button type="button" disabled={desktopEntry(menu.entry) && !appReady(menu.entry)} onclick={() => { onopenport?.(menu!.entry.port); closeMenu(); }}>{desktopEntry(menu.entry) ? '显示 App 窗口' : `打开 localhost:${menu.entry.port}`}</button>
    {#if menu.entry.projectId}
      <button type="button" disabled={stopAllBusy || stoppingIds.includes(menu.entry.projectId!)} onclick={() => { onstopproject?.(menu!.entry.projectId!); closeMenu(); }}>停止项目</button>
    {:else if menu.entry.owned !== true}
      <button
        type="button"
        class="danger"
        disabled={killBusy}
        onclick={() => {
          if (killBusy) return;
          onkill?.(menu!.entry.pid);
          closeMenu();
        }}
      >
        {killBusy && killingPid === menu.entry.pid ? "正在结束…" : "Kill 进程"}
      </button>
    {/if}
  </div>
{/if}


<style>
  .qd-port-focus {outline:2px solid var(--color-qd-brand);outline-offset:2px;}
  .qd-port-source { margin:0 8px 8px; padding:12px; border:1px solid var(--color-qd-border); border-radius:8px; font-size:11px; background:var(--color-qd-surface); }
  .qd-port-source dl { display:grid; grid-template-columns:72px minmax(0,1fr); gap:7px; margin-top:8px; }
  .qd-port-source dt { color:var(--color-qd-muted); }
  .qd-port-source dd { margin:0; overflow-wrap:anywhere; font-family:monospace; }
  .ctx-menu {
    position: fixed;
    z-index: 200;
    min-width: 160px;
    padding: 4px;
    background: var(--color-qd-surface);
    border: 1px solid var(--color-qd-border);
    border-radius: 8px;
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.45);
  }
  .ctx-menu button {
    display: block;
    width: 100%;
    text-align: left;
    padding: 7px 10px;
    font-size: 12px;
    color: var(--color-qd-body);
    background: transparent;
    border: none;
    border-radius: 6px;
    cursor: pointer;
  }
  .ctx-menu button:hover {
    background: var(--color-qd-elevated);
    color: var(--color-qd-ink);
  }
  .ctx-menu button.danger {
    color: var(--color-qd-danger);
  }
</style>
