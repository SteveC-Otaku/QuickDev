<script lang="ts">
  import ActionIcon from "./ActionIcon.svelte";
  import { onDestroy } from "svelte";
  import type { LogLine } from "../types";

  interface Props {
    logs: LogLine[];
    title: string;
    height: number;
    maxHeight?: number;
    loading?: boolean;
    projectId?: string;
    onheight?: (h: number) => void;
  }

  let { logs, title, height, maxHeight = 420, loading = false, projectId, onheight }: Props = $props();

  let collapsed = $state(false);
  let cleanupResize = () => {};
  onDestroy(() => cleanupResize());
  $effect(() => { void projectId; scrollTop = 0; stickBottom = true; });
  const LINE = 18;
  const OVERSCAN = 8;

  let scrollEl: HTMLDivElement | undefined = $state();
  let scrollTop = $state(0);
  let viewH = $state(120);
  let stickBottom = $state(true);

  const totalH = $derived(Math.max(logs.length, 1) * LINE);
  const start = $derived(Math.max(0, Math.floor(scrollTop / LINE) - OVERSCAN));
  const visible = $derived(Math.ceil(viewH / LINE) + OVERSCAN * 2);
  const end = $derived(Math.min(logs.length, start + visible));
  const slice = $derived(logs.slice(start, end));
  const padTop = $derived(start * LINE);

  $effect(() => {
    // auto-scroll when new logs arrive and user is pinned to bottom
    void logs;
    void height;
    if (!stickBottom || !scrollEl) return;
    requestAnimationFrame(() => {
      if (scrollEl) {
        scrollEl.scrollTop = scrollEl.scrollHeight;
        scrollTop = scrollEl.scrollTop;
      }
    });
  });

  function onScroll() {
    if (!scrollEl) return;
    scrollTop = scrollEl.scrollTop;
    viewH = scrollEl.clientHeight;
    const dist = scrollEl.scrollHeight - scrollEl.scrollTop - scrollEl.clientHeight;
    stickBottom = dist < 24;
  }

  function startResize(e: PointerEvent) {
    if (e.button !== 0) return;
    cleanupResize();
    e.preventDefault();
    const handle = e.currentTarget as HTMLButtonElement;
    handle.setPointerCapture(e.pointerId);
    let lastY = e.clientY;
    let currentH = height;
    const move = (ev: PointerEvent) => {
      if (ev.pointerId !== e.pointerId) return;
      // Increment from the last position so reversing at either limit is immediate.
      currentH = Math.min(maxHeight, Math.max(100, currentH + lastY - ev.clientY));
      lastY = ev.clientY;
      onheight?.(currentH);
    };
    const up = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      window.removeEventListener("pointercancel", up);
      handle.removeEventListener("lostpointercapture", up);
      if (handle.hasPointerCapture(e.pointerId)) handle.releasePointerCapture(e.pointerId);
    };
    cleanupResize = up;
    window.addEventListener("pointercancel", up);
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
    handle.addEventListener("lostpointercapture", up);
  }
</script>

<div class="shrink-0 min-h-[100px] border-t border-qd-border flex flex-col" style:height="{collapsed ? 32 : height}px" style:min-height="{collapsed ? 32 : 100}px">
  {#if !collapsed}<button
    type="button"
    class="qd-log-resize"
    aria-label="拖动调整日志高度"
    onpointerdown={startResize}
    onkeydown={(e) => { if (e.key === "ArrowUp" || e.key === "ArrowDown") { e.preventDefault(); onheight?.(Math.min(maxHeight, Math.max(100, height + (e.key === "ArrowUp" ? 20 : -20)))); } }}
  ></button>{/if}
  <div class="flex items-center justify-between px-4 py-1.5 bg-qd-surface border-b border-qd-border/60 shrink-0">
    <span class="text-[11px] font-semibold text-qd-muted uppercase tracking-wider">{title}</span>
    <div class="flex items-center gap-2"><button class="qd-btn qd-btn-ghost qd-btn-sm" aria-expanded={!collapsed} onclick={() => collapsed = !collapsed}><ActionIcon name={collapsed ? "down" : "up"} size={14} /> {collapsed ? "展开日志" : "收起日志"}</button>{#if !stickBottom}<button class="qd-btn-ghost" onclick={() => { stickBottom = true; if (scrollEl) scrollEl.scrollTop = scrollEl.scrollHeight; }}>回到最新</button>{/if}<span class="text-[10px] text-qd-dim">{logs.length} 行</span></div>
  </div>
  {#if !collapsed}<div
    bind:this={scrollEl}
    class="flex-1 overflow-auto px-4 py-2 bg-qd-log-bg font-mono text-[11px] leading-[18px] min-h-0"
    bind:clientHeight={viewH}
    aria-busy={loading}
    onscroll={onScroll}
  >
    {#if loading}<div class="flex items-center gap-2 text-qd-muted" role="status"><span class="qd-spinner" aria-hidden="true"></span>读取日志…</div>
    {:else if logs.length === 0}
      <div class="text-qd-dim">该项目暂无日志，启动后会实时显示输出</div>
    {:else}
      <div style:height="{totalH}px" class="relative">
        <div style:transform="translateY({padTop}px)">
          {#each slice as line}
            <div
              class="text-qd-body whitespace-pre h-[18px]"
              class:text-qd-info={line.level === "info"}
              class:text-qd-running={line.level === "success"}
              class:text-qd-warn={line.level === "warn"}
              class:text-qd-danger={line.level === "error"}
            >
              {line.text}
            </div>
          {/each}
        </div>
      </div>
    {/if}
  </div>{/if}
</div>
