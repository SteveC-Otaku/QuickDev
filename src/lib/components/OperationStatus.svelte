<script lang="ts">
  let { text, startedAt, operationId }: { text: string; startedAt?: number; operationId?: number | string } = $props();
  let seconds = $state(0);
  $effect(() => {
    if (!text) return;
    void operationId;
    const started = startedAt ?? Date.now();
    seconds = Math.max(0, Math.floor((Date.now() - started) / 1000));
    const timer = setInterval(() => seconds = Math.floor((Date.now() - started) / 1000), 1000);
    return () => clearInterval(timer);
  });
</script>
{#if text}
  <div class="operation">
    <span class="qd-spinner" aria-hidden="true"></span>
    <span class="message" role="status" title={text}>{text}</span>
    <span class="elapsed" aria-hidden="true">{seconds}s</span>
    {#if seconds >= 8}<small>仍在等待，可切换页面或查看日志</small>{/if}
  </div>
{/if}
<style>
  .operation { display: flex; align-items: center; gap: 8px; width: 100%; min-width: 0; height: 100%; overflow: hidden; color: var(--color-qd-body); font-size: 11px; }
  .message { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .elapsed { flex-shrink: 0; min-width: 4ch; text-align: right; font-variant-numeric: tabular-nums; color: var(--color-qd-muted); }
  small { flex-shrink: 0; font-size: 11px; color: var(--color-qd-muted); white-space: nowrap; }
  @media (max-width: 620px) { small { display: none; } }
</style>
