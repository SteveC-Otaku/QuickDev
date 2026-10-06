<script lang="ts">
  import ActionIcon from './ActionIcon.svelte';
  import type { Project } from '../types';
  import {projectServices,isDesktopService,serviceKindLabels} from '../services';
  let {project,busy=false,onstart,onstop,onopen}: {project:Project;busy?:boolean;onstart?:(id:string)=>void;onstop?:(id:string)=>void;onopen?:(id:string)=>void}=$props();
</script>
<div class="plans" aria-label={`${project.name} 启动方案`}>
  {#each project.launchPlans ?? [] as plan (plan.id)}
    {@const states=plan.serviceIds.map(id=>project.serviceStatuses?.find(s=>s.id===id))}
    {@const running=states.filter(s=>s?.status==='running').length}
    {@const active=project.activePlanIds?.includes(plan.id)}
    {@const ready=running===plan.serviceIds.length}
    {@const failed=states.some(s=>s?.message || s?.status==='failed' || s?.status==='crashed')}
    {@const desktop=projectServices(project).some(s=>s.id===plan.openServiceId && isDesktopService(s))}
    <div class="plan-row">
      <div class="plan-title"><strong>{plan.name}</strong>{#if project.defaultPlanId===plan.id}<span class="default">默认</span>{/if}<small>{plan.serviceIds.map(id=>{const service=projectServices(project).find(s=>s.id===id);return service ? `${service.name}${service.name===serviceKindLabels[service.kind ?? 'unknown'] ? '' : ` · ${serviceKindLabels[service.kind ?? 'unknown']}`}${service.port ? ` :${service.port}` : ''}` : id;}).join(' + ')}</small></div>
      <span class="status" class:ready class:failed>{ready ? (active ? '运行中' : '服务已运行') : failed ? '有异常' : running ? `${running}/${states.length} 运行` : '已停止'}</span>
      {#if active && ready}<button class="qd-btn qd-btn-stop qd-btn-sm" disabled={busy} onclick={()=>onstop?.(plan.id)}><ActionIcon name="stop" size={14}/>停止</button>{:else}<button class="qd-btn qd-btn-play qd-btn-sm" disabled={busy} onclick={()=>onstart?.(plan.id)}><ActionIcon name="play" size={14}/>{ready ? '启用方案' : running ? '补齐启动' : '启动'}</button>{#if active}<button class="qd-btn qd-btn-stop qd-btn-sm" disabled={busy} onclick={()=>onstop?.(plan.id)}>停止</button>{/if}{/if}
      {#if plan.openServiceId && project.serviceStatuses?.some(s=>s.id===plan.openServiceId && s.status==='running')}<button class="qd-btn qd-btn-secondary qd-btn-sm" onclick={()=>onopen?.(plan.openServiceId!)}><ActionIcon name={desktop?'app':'open'} size={14}/>{desktop?'显示 App':'打开网页'}</button>{/if}
    </div>
  {/each}
</div>
<style>
  .plans{display:grid;gap:6px;margin-top:10px;padding-top:10px;border-top:1px solid var(--color-qd-border)}
  .plan-row{display:flex;gap:8px;align-items:center;flex-wrap:wrap;font-size:12px;padding:4px 0}
  .plan-title{display:flex;align-items:center;gap:6px;flex:1;min-width:140px;flex-wrap:wrap}.plan-title strong{color:var(--color-qd-ink)}
  .plan-title small{color:var(--color-qd-muted);font-size:10px;flex-basis:100%;overflow-wrap:anywhere;line-height:1.6}
  .default{color:var(--color-qd-brand);font-size:10px}.status{color:var(--color-qd-muted);font-size:11px}.status.ready{color:var(--color-qd-running)}.status.failed{color:var(--color-qd-danger)}
</style>
