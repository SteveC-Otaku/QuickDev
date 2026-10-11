<script lang="ts">
  import ActionIcon from "./ActionIcon.svelte";
  import { app } from "../store.svelte";
  import { dialogFocus } from "../focus";
  import { updater } from "../update.svelte";

  const pct = $derived(updater.progress?.total ? Math.round(updater.progress.done / updater.progress.total * 100) : 0);
</script>

<svelte:window onkeydown={(e) => {
  if (e.key !== "Escape") return;
  if (app.confirmation) app.resolveConfirmation(false);
  else if (updater.dialogOpen && !updater.progress) updater.dialogOpen = false;
}} />

{#if app.notice}
  <aside class="qd-notice" class:qd-notice-error={app.notice.kind === "error"} role={app.notice.kind === "error" ? "alert" : "status"}>
    <span>{app.notice.text}</span>
    {#if app.notice.configuration}<button class="qd-btn qd-btn-secondary qd-btn-sm" onclick={()=>{const target=app.notice?.configuration;if(target)void app.locateServiceConfiguration(target.projectId,target.serviceId);}}>定位配置</button>{/if}
    <button class="qd-btn-ghost" aria-label="关闭提示" onclick={() => app.notice = null}><ActionIcon name="close" size={16} /></button>
  </aside>
{/if}

{#if app.confirmation}
  <div class="qd-dialog-backdrop">
    <div class="qd-dialog" role="dialog" aria-modal="true" aria-labelledby="confirm-title" aria-describedby="confirm-body" tabindex="-1" use:dialogFocus>
      <h2 id="confirm-title">{app.confirmation.title}</h2>
      <p id="confirm-body">{app.confirmation.text}</p>
      <div class="qd-dialog-actions">
        <button class="qd-btn qd-btn-secondary" onclick={() => app.resolveConfirmation(false)}>取消</button>
        <button class="qd-btn qd-btn-danger" onclick={() => app.resolveConfirmation(true)}>{app.confirmation.action}</button>
      </div>
    </div>
  </div>
{/if}

{#if updater.dialogOpen && updater.available && !app.confirmation}
  <div class="qd-dialog-backdrop">
    <div class="qd-dialog qd-update" role="dialog" aria-modal="true" aria-labelledby="update-title" tabindex="-1" use:dialogFocus>
      <h2 id="update-title">发现新版本 v{updater.available.version}</h2>
      <div class="qd-update-sub">当前 v{updater.current}{updater.available.date ? ` · 发布于 ${updater.available.date.slice(0, 10)}` : ""}</div>
      {#if updater.available.body}
        <ul class="qd-update-notes">
          {#each updater.available.body.split("\n").map(l => l.replace(/^[-*•]\s*/, "").trim()).filter(Boolean) as line}<li>{line}</li>{/each}
        </ul>
      {/if}
      {#if updater.progress}
        <div class="qd-update-bar" role="progressbar" aria-label="下载进度" aria-valuemin={0} aria-valuemax={100} aria-valuenow={pct}><span style:width="{updater.progress.total ? pct : 30}%"></span></div>
        <p class="qd-update-hint">{updater.progress.total && pct < 100 ? `正在下载 ${pct}%` : "正在安装，即栈会自动重启…"}</p>
      {:else}
        <p class="qd-update-hint">{app.runningCount > 0 ? `更新时会停止正在运行的 ${app.runningCount} 个项目，` : ""}安装完成后自动重启。</p>
      {/if}
      {#if updater.error}<p class="qd-update-hint qd-update-error" role="alert">{updater.error}</p>{/if}
      <div class="qd-dialog-actions">
        <button class="qd-btn qd-btn-secondary" disabled={!!updater.progress} onclick={() => updater.dialogOpen = false}>稍后</button>
        <button class="qd-btn qd-btn-primary" disabled={!!updater.progress} onclick={() => updater.install()}>立即更新</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .qd-update { width: min(100%, 460px); }
  .qd-update-sub { margin-top: 4px; font-size: 12px; color: var(--color-qd-dim); }
  .qd-update-notes { margin: 14px 0 0; padding: 12px 14px 12px 28px; max-height: 220px; overflow-y: auto; list-style: disc; border: 1px solid var(--color-qd-border); border-radius: 8px; background: var(--color-qd-elevated); font-size: 12.5px; line-height: 1.7; color: var(--color-qd-body); }
  .qd-update-bar { margin-top: 16px; height: 6px; border-radius: 3px; background: var(--color-qd-elevated); overflow: hidden; }
  .qd-update-bar span { display: block; height: 100%; background: var(--color-qd-brand); transition: width 0.2s; }
  .qd-dialog p.qd-update-hint { margin: 12px 0 16px; font-size: 12px; line-height: 1.6; color: var(--color-qd-muted); }
  .qd-dialog p.qd-update-error { color: var(--color-qd-danger); }
</style>
