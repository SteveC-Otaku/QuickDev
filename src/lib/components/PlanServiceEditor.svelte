<script lang="ts">
  import ActionIcon from './ActionIcon.svelte';
  import ServiceEditor from './ServiceEditor.svelte';
  import {serviceKindLabels,candidateKind} from '../services';
  import type {ProjectService} from '../types';
  let {service=$bindable(),index=0,sharedCount=1,invalid=false,onremove}:{service:ProjectService;index?:number;sharedCount?:number;invalid?:boolean;onremove?:()=>void}=$props();
  let advanced=$state(false);
  $effect(()=>{if(invalid&&service.command.trim())advanced=true;});
  function portInput(e:Event){const value=(e.currentTarget as HTMLInputElement).value.trim();service.port=value?Number(value):undefined;}
  function classify(){if(!service.kind||service.kind==='unknown'){const kind=candidateKind({command:service.command,cwd:service.cwd,label:'',reason:''});if(kind!=='unknown'){service.kind=kind;if(kind==='desktop'&&(service.startupTimeoutSecs??60)===60)service.startupTimeoutSecs=600;}}}
</script>
<section class="command-editor" class:invalid aria-label={`${service.name || '自定义任务'} 命令设置`}>
  <div class="command-row">
    <span class="index">{String(index+1).padStart(2,'0')}</span>
    <div class="identity"><strong>{service.name || (service.kind==='unknown'||!service.kind?'自定义任务':serviceKindLabels[service.kind])}</strong><span class="kind" class:desktop={service.kind==='desktop'}>{serviceKindLabels[service.kind ?? 'unknown']}</span>{#if sharedCount>1}<small>共用 · {sharedCount} 个方案</small>{/if}</div>
    <label class="command-field">启动命令<input class="qd-input font-mono" data-command-id={service.id} bind:value={service.command} onchange={classify} placeholder="例如 npm run preview" /></label>
    <label class="port-field">端口<input class="qd-input font-mono" value={service.port??''} oninput={portInput} placeholder="可选" inputmode="numeric" /></label>
    <button type="button" class="qd-btn qd-btn-ghost qd-btn-sm" aria-label={`从当前方案移除 ${service.name || '任务'}`} onclick={onremove}><ActionIcon name="close" size={14}/></button>
  </div>
  <details bind:open={advanced}>
    <summary><ActionIcon name={advanced?'up':'down'} size={14}/>服务参数 · 目录、名称、类型与等待判断</summary>
    {#if advanced}
      <p class="hint">这里调整上面这条命令的执行参数。{#if sharedCount>1}此服务由多个方案共用，修改后在这些方案中同步生效。{/if}</p>
      <div class="advanced-fields"><label>执行目录<input class="qd-input font-mono" bind:value={service.cwd} placeholder="D:\Projects\my-app\web"/></label><label>网页地址（选填）<input class="qd-input font-mono" bind:value={service.openUrl} placeholder="留空根据端口自动打开"/></label></div>
      <ServiceEditor bind:service optionsOnly/>
    {/if}
  </details>
</section>
<style>
.command-editor{border:1px solid var(--color-qd-border);border-radius:8px;padding:10px;background:var(--color-qd-surface)}.command-editor.invalid{border-color:var(--color-qd-danger)}
.command-row{display:grid;grid-template-columns:20px minmax(100px,1fr) minmax(180px,2fr) 68px 24px;gap:9px;align-items:center}.index{font-size:10px;color:var(--color-qd-muted);font-family:var(--font-mono)}.identity{display:grid;gap:4px;min-width:0}.identity strong{font-size:12px;color:var(--color-qd-ink);overflow-wrap:anywhere}.identity small{color:var(--color-qd-warn);font-size:10px}.kind{font-size:10px;color:var(--color-qd-brand);background:var(--color-qd-brand-soft);border-radius:4px;padding:2px 5px;width:fit-content}.kind.desktop{color:var(--color-qd-running)}
label{display:grid;gap:5px;font-size:11px;color:var(--color-qd-muted);min-width:0}.command-row .qd-input{padding:7px 8px;font-size:11px}.advanced-fields{display:grid;gap:10px;margin:10px 0}.hint{font-size:11px;color:var(--color-qd-muted);line-height:1.7;margin-top:8px}details{margin-top:10px;border-top:1px solid var(--color-qd-border);padding-top:8px}summary{display:flex;align-items:center;gap:6px;font-size:10px;color:var(--color-qd-muted);cursor:pointer;list-style:none}summary::-webkit-details-marker{display:none}summary:focus-visible{outline:2px solid var(--color-qd-brand);outline-offset:3px}
@media(max-width:750px){.command-row{grid-template-columns:20px minmax(110px,1fr) 68px 24px}.command-field{grid-column:2/-1;grid-row:2}}
</style>
