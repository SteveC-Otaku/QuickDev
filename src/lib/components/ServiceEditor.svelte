<script lang="ts">
  import ActionIcon from "./ActionIcon.svelte";
  import { serviceKindLabels } from '../services';
  import type { ProjectService } from "../types";
  let { service = $bindable(), optionsOnly = false, onremove }: { service: ProjectService; optionsOnly?: boolean; onremove?: () => void } = $props();
  let advancedOpen = $state(false);
  const modes = [
    {id:'auto',label:'自动判断（推荐）',hint:'桌面项目等到 App 窗口出现；网页项目等到端口可用。没有端口的任务只检查进程是否在运行。'},
    {id:'window',label:'等 App 窗口出现',hint:'适合 Tauri / Electron 桌面应用；启动命令需要真正打开应用窗口。'},
    {id:'process',label:'命令保持运行即可',hint:'适合后台脚本。只确认进程存活，不保证网页已经能打开。'},
    {id:'port',label:'等网页端口可用',hint:'适合前端和后端服务。请在项目中填写端口号，例如 5173 或 8000。'},
    {id:'http',label:'等接口检查通过',hint:'适合需要确认业务就绪的后端。填写本机检查地址，返回成功响应后才算启动成功。'},
  ] as const;
  const selectedMode = $derived(modes.find(mode => mode.id === (service.readiness ?? 'auto')) ?? modes[0]);
  $effect(() => { if (service.readiness && service.readiness !== 'auto') advancedOpen = true; });
  function portInput(event: Event) {
    const value = (event.currentTarget as HTMLInputElement).value.trim();
    service.port = value ? Number(value) : undefined;
  }
</script>

<section class="service-editor" aria-label={`${service.name || "新服务"} 设置`}>
  <div class="service-heading"><label>任务名称（选填）<input class="qd-input" bind:value={service.name} placeholder="留空自动命名，也可填前端 / 后端" /></label>{#if onremove}<button type="button" class="qd-btn qd-btn-ghost qd-btn-sm" onclick={onremove}><ActionIcon name="close" size={14} /> 移除服务</button>{/if}</div>
  <div class="service-row" role="group" aria-label="服务类型">{#each Object.entries(serviceKindLabels) as [kind,label]}<button type="button" class="qd-btn qd-btn-secondary qd-btn-sm" aria-pressed={(service.kind ?? 'unknown') === kind} onclick={() => service.kind = kind as ProjectService['kind']}>{label}</button>{/each}</div>
  {#if !optionsOnly}
    <label>执行目录<input class="qd-input font-mono" bind:value={service.cwd} required placeholder="D:\Projects\app\backend" /></label>
    <label>启动命令<input class="qd-input font-mono" bind:value={service.command} required placeholder="python manage.py runserver" /></label>
    <div class="service-row"><label>预期端口<input class="qd-input" value={service.port ?? ""} oninput={portInput} inputmode="numeric" placeholder="可选" /></label><label>打开地址<input class="qd-input" bind:value={service.openUrl} placeholder="http://localhost:8000" /></label></div>
  {/if}
  <div class="readiness-summary"><strong>什么时候算启动成功？</strong><span>{selectedMode.label}</span></div>
  <p class="service-hint">{selectedMode.hint}</p>
  <details bind:open={advancedOpen}>
    <summary><ActionIcon name={advancedOpen ? 'up' : 'down'} size={14} /> 调整启动判断与等待时间</summary>
    {#if advancedOpen}
    <div class="advanced-options">
      <p class="service-hint">通常保持自动即可；启动一直在等待时，再按项目类型调整。</p>
      <div class="checks" role="group" aria-label="启动成功的判断方式">
        {#each modes as mode}
          <button type="button" class="qd-btn qd-btn-secondary qd-btn-sm" class:chosen={selectedMode.id === mode.id} aria-pressed={selectedMode.id === mode.id} onclick={() => service.readiness = mode.id}>{mode.label}</button>
        {/each}
      </div>
      {#if service.readiness === 'http'}<label>检查地址<input class="qd-input font-mono" bind:value={service.healthUrl} required placeholder="http://localhost:8000/health" /></label>{/if}
      <label>最多等多久（秒）<input class="qd-input" type="number" min="1" max="3600" bind:value={service.startupTimeoutSecs} required /></label>
      <p class="service-hint">通常用 60 秒；桌面应用首次编译可用 600 秒。提前就绪会立即完成，不会等满这个时间。</p>
    </div>
    {/if}
  </details>
</section>

<style>
  .service-row button[aria-pressed="true"] {border-color:var(--color-qd-brand);color:var(--color-qd-brand);background:var(--color-qd-brand-soft);}
  .service-editor { flex-shrink:0; display:flex; flex-direction:column; gap:8px; border:1px solid var(--color-qd-border); background:var(--color-qd-elevated); border-radius:8px; padding:10px; }
  label { display:flex; flex-direction:column; gap:4px; color:var(--color-qd-muted); font-size:11px; min-width:0; flex:1; }
  .service-row,.service-heading { display:flex; gap:8px; align-items:center; }
  input[type="number"] { appearance:textfield; }
  input[type="number"]::-webkit-inner-spin-button,input[type="number"]::-webkit-outer-spin-button { appearance:none; margin:0; }
  .checks { display:flex; flex-wrap:wrap; gap:5px; }
  .chosen { border-color:var(--color-qd-brand); background:var(--color-qd-brand-soft); color:var(--color-qd-ink); }
  .service-hint { font-size:11px; line-height:1.6; color:var(--color-qd-muted); }
  .readiness-summary { display:flex; align-items:center; justify-content:space-between; gap:8px; flex-wrap:wrap; font-size:11px; }
  .readiness-summary strong { color:var(--color-qd-body); font-weight:500; }
  .readiness-summary span { color:var(--color-qd-brand); }
  details { border-top:1px solid var(--color-qd-border); padding-top:8px; }
  summary { display:flex; align-items:center; gap:6px; cursor:pointer; color:var(--color-qd-muted); font-size:11px; list-style:none; }
  summary::-webkit-details-marker { display:none; }
  summary:focus-visible { outline:2px solid var(--color-qd-brand); outline-offset:3px; border-radius:4px; }
  .advanced-options { display:grid; gap:8px; padding-top:10px; }
</style>
