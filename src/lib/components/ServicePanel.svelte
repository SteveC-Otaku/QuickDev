<script lang="ts">
  import ActionIcon from "./ActionIcon.svelte";
  import type { Project } from '../types';
  import { projectServices, isDesktopService } from '../services';
  let { project, busy = [], onaction, onopen, onlogs }: {
    project: Project; busy?: string[];
    onaction?: (id: string, service: string, kind: 'start' | 'stop' | 'restart') => void;
    onopen?: (project: Project, service: string) => void;
    onlogs?: (project: string, service: string) => void;
  } = $props();
  const labels: Record<string,string> = { stopped:'已停止', starting:'等待就绪', running:'已就绪', stopping:'停止中', crashed:'异常退出', failed:'启动失败' };
</script>
<div class="services">
  {#each projectServices(project) as service (service.id)}
    {@const state = project.serviceStatuses?.find(s => s.id === service.id)}
    {@const status = state?.status ?? 'stopped'}
    {@const pending = !!project.operation || busy.includes(`${project.id}/${service.id}`)}
    <div class="service">
      <div class="heading"><strong>{service.name}</strong><span class:ready={status === 'running'} class:error={!!state?.message}>{status === 'running' && state?.readiness === 'process' ? '进程运行中' : labels[status] ?? status}{#if service.port} · :{service.port}{/if}</span></div>
      <div class="command" title={`${service.cwd} · ${service.command}`}>{service.cwd} · {service.command}</div>
      {#if state?.message}<p role="status" class="error">{state.message}</p>{/if}
      <div class="actions">
        {#if status === 'running'}
          <span class="readiness">{state?.readiness === 'window' ? 'App 窗口已出现' : state?.readiness === 'http' ? '健康检查通过' : state?.readiness === 'port' ? '端口已就绪' : '进程运行中'}</span>
          <button class="qd-btn qd-btn-secondary qd-btn-sm" disabled={pending} onclick={() => onaction?.(project.id,service.id,'stop')}><ActionIcon name="stop" size={14} /> 停止</button>
          <button class="qd-btn qd-btn-secondary qd-btn-sm" disabled={pending} onclick={() => onaction?.(project.id,service.id,'restart')}><ActionIcon name="refresh" size={14} /> 重启</button>
          {#if isDesktopService(service) || state?.openUrl || service.openUrl || service.port}<button class="qd-btn qd-btn-secondary qd-btn-sm" onclick={() => onopen?.(project,service.id)}><ActionIcon name={isDesktopService(service) ? 'app' : 'open'} size={14} /> {isDesktopService(service) ? '显示 App 窗口' : '打开网页'}</button>{/if}
        {:else}
          <button class="qd-btn qd-btn-play qd-btn-sm" disabled={pending || status === 'starting' || status === 'stopping'} onclick={() => onaction?.(project.id,service.id,'start')}><ActionIcon name={state?.message ? 'refresh' : 'play'} size={14} /> {pending ? '处理中…' : state?.message ? '重试' : '启动'}</button>
        {/if}
        <button class="qd-btn qd-btn-ghost qd-btn-sm" onclick={() => onlogs?.(project.id,service.id)}><ActionIcon name="log" size={14} /> 日志</button>
      </div>
    </div>
  {/each}
</div>
<style>
  .services { display:grid; gap:8px; margin-top:10px; }
  .service { padding:9px; border:1px solid var(--color-qd-border); border-radius:8px; background:var(--color-qd-window); }
  .heading,.actions { display:flex; align-items:center; gap:8px; flex-wrap:wrap; font-size:11px; }
  .heading strong { color:var(--color-qd-ink); font-size:12px; }
  .heading span { margin-left:auto; color:var(--color-qd-muted); }
  .command { margin:5px 0 8px; font:10px monospace; color:var(--color-qd-dim); overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .readiness { color:var(--color-qd-muted); margin-right:auto; }
  .heading .ready { color:var(--color-qd-running); }
  .heading .error,.error { color:var(--color-qd-danger); }
  p { font-size:11px; overflow-wrap:anywhere; margin:6px 0; }
</style>
