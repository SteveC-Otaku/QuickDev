<script lang="ts">
  import ActionIcon from "./ActionIcon.svelte";
  import type { PortEntry } from "../types";

  interface Props {
    ports: PortEntry[];
    lastRefresh?: string;
    loading?: boolean;
    onrefresh?: () => void;
    onkill?: (pid: number) => void;
    onstopall?: () => void;
    onopenport?: (port: number) => void;
    onstopproject?: (projectId: string) => void;
  }

  let {
    ports,
    lastRefresh = "—",
    loading = false,
    onrefresh,
    onkill,
    onstopall,
    onopenport,
    onstopproject,
  }: Props = $props();

  let menu = $state<{ x: number; y: number; entry: PortEntry } | null>(null);
  let copiedHint = $state("");

  const projectPorts = $derived.by(() => {
    const seen = new Map<string, PortEntry>();
    for (const p of ports) {
      if (!p.projectName) continue;
      const key = p.projectId ?? p.projectName;
      if (!seen.has(key)) seen.set(key, p);
    }
    return [...seen.values()];
  });
  const otherPorts = $derived(ports.filter((p) => !p.projectName));

  function openMenu(e: MouseEvent, entry: PortEntry) {
    e.preventDefault();
    menu = { x: e.clientX, y: e.clientY, entry };
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

<svelte:window onclick={closeMenu} />

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
      <button class="qd-btn qd-btn-icon qd-btn-stop" title="全部停止" disabled={loading} onclick={() => onstopall?.()}>
        <ActionIcon name="stop" />
        <span>全部停止</span>
      </button>
    </div>
  </div>

  <div class="flex-1 overflow-y-auto px-4 pb-2">
    {#if ports.length === 0}
      <div class="text-center text-qd-dim text-xs py-8">暂无监听端口</div>
    {/if}

    {#if projectPorts.length > 0}
      <div class="qd-project-section-head">
        <span class="qd-project-section-dot"></span>
        <span>运行中的项目</span>
        <span class="qd-project-section-count">{projectPorts.length}</span>
      </div>
      <div class="flex flex-col gap-2 mb-3">
        {#each projectPorts as entry}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            class="qd-project-card cursor-context-menu"
            oncontextmenu={(e) => openMenu(e, entry)}
          >
            <div class="qd-project-card-inner">
              <div class="qd-project-port">:{entry.port}</div>
              <div class="qd-project-name">{entry.projectName}</div>
              <div class="qd-project-status">
                <span class="qd-project-status-dot"></span>
                运行中
              </div>
              <div class="qd-project-actions">
                <button class="qd-btn qd-btn-icon qd-btn-sm qd-project-btn-open" title="浏览器打开" onclick={() => onopenport?.(entry.port)}>
                  <ActionIcon name="open" size={16} />
                </button>
                {#if entry.projectId}
                  <button class="qd-btn qd-btn-icon qd-btn-sm qd-btn-stop" title="停止项目" onclick={() => onstopproject?.(entry.projectId!)}>
                    <ActionIcon name="stop" size={16} />
                  </button>
                {/if}
              </div>
            </div>
          </div>
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
          oncontextmenu={(e) => openMenu(e, entry)}
        >
          <span class="font-mono font-medium text-sm text-qd-brand" class:text-qd-warn={entry.highlight && entry.owned === false}>
            :{entry.port}
          </span>
          <span class="text-qd-body font-medium">{entry.process}</span>
          <span class="font-mono text-[11px] text-qd-muted">{entry.pid}</span>
          <span class="font-mono text-[11px] text-qd-dim truncate" title={entry.command}>{entry.command}</span>
          {#if entry.owned === false}
            <span class="qd-tag qd-tag-danger">外部占用</span>
          {:else}
            <div class="flex gap-1">
              <button class="qd-btn qd-btn-secondary qd-btn-sm qd-btn-icon" title="浏览器打开" onclick={() => onopenport?.(entry.port)}>
                <ActionIcon name="open" />
              </button>
              <button class="qd-btn qd-btn-icon qd-btn-kill qd-btn-sm" title="Kill 进程" onclick={() => onkill?.(entry.pid)}>
                <ActionIcon name="kill" />
              </button>
            </div>
          {/if}
        </div>
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
    <button type="button" onclick={() => { onopenport?.(menu!.entry.port); closeMenu(); }}>打开 localhost:{menu.entry.port}</button>
    {#if menu.entry.projectId}
      <button type="button" onclick={() => { onstopproject?.(menu!.entry.projectId!); closeMenu(); }}>停止项目</button>
    {:else if menu.entry.owned !== true}
      <button type="button" class="danger" onclick={() => { onkill?.(menu!.entry.pid); closeMenu(); }}>
        Kill 进程
      </button>
    {/if}
  </div>
{/if}

<style>
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
