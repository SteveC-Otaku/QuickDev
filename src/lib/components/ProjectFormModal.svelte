<script lang="ts">
  import {untrack,tick} from 'svelte';
  import ActionIcon from './ActionIcon.svelte';
  import PlanServiceEditor from './PlanServiceEditor.svelte';
  import {dialogFocus} from '../focus';
  import {open as openDialog} from '@tauri-apps/plugin-dialog';
  import {detectProject,suggestProjects} from '../ipc';
   import {projectServices,validateServices,validateLaunchPlans,generateLaunchPlans,resolvedOpenService,isDesktopService,candidateKind,serviceKindLabels,projectPathName,commonProjectDirectory,mergeDetectedServices,sameLaunchPlan} from '../services';
  import type {Project,ProjectService,LaunchPlan,CommandCandidate,PortSuggest} from '../types';
   interface Props{open:boolean;editing?:Project|null;focusServiceId?:string;projectCount:number;groups?:string[];onclose?:()=>void;onsave?:(project:Project)=>void|Promise<void>;ondetect?:typeof detectProject;}
   let {open,editing=null,focusServiceId,projectCount,groups=[],onclose,onsave,ondetect=detectProject}:Props=$props();
  let name=$state(''),rootCwd=$state(''),group=$state('');
  let services=$state<ProjectService[]>([]),plans=$state<LaunchPlan[]>([]);
  let defaultPlanId=$state<string|undefined>(),selectedId=$state('');
  let saving=$state(false),detecting=$state(false),suggesting=$state(false);
  let error=$state(''),notice=$state(''),invalidServiceId=$state('');
  let candidates=$state<CommandCandidate[]>([]),suggests=$state<PortSuggest[]>([]);
  let pickerOpen=$state(false);
   let detectedDuplicates=$state<string[]>([]);
  let workspace=$state<HTMLElement>(),nameInput=$state<HTMLInputElement>();
  let initializedOpen=false,detectVersion=0,sessionVersion=0,sequence=0,detectedCwd='';
  let starter=true;
  const selected=$derived(plans.find(p=>p.id===selectedId));
  const title=$derived(editing?'编辑项目':'添加项目');
  const groupChoices=$derived([...new Set(['工作','个人',...groups.map(g=>g.trim()).filter(Boolean),...(group.trim()?[group.trim()]:[])])]);
  type Draft={name:string;rootCwd:string;group:string;services:ProjectService[];plans:LaunchPlan[];defaultPlanId?:string;selectedId:string;starter:boolean};
  let draft=$state<Draft|null>(null);
  const uid=(prefix:string)=>`${prefix}-${Date.now().toString(36)}-${++sequence}`;
  const blankService=(id=uid('service')):ProjectService=>({id,name:'',cwd:rootCwd,command:'',kind:'unknown',readiness:'auto',startupTimeoutSecs:60});
  const snapshot=():Draft=>({name,rootCwd,group,services:structuredClone($state.snapshot(services)),plans:structuredClone($state.snapshot(plans)),defaultPlanId,selectedId,starter});
  function requestClose(){detectVersion++;sessionVersion++;detecting=false;if(!editing&&(name.trim()||rootCwd.trim()||services.some(s=>s.command.trim())))draft=snapshot();onclose?.();}
  $effect(()=>{
    if(!open){initializedOpen=false;return;}if(initializedOpen)return;initializedOpen=true;
    untrack(()=>{
       sessionVersion++;detectVersion++;saving=false;detecting=false;error='';notice='';invalidServiceId='';pickerOpen=false;candidates=[];detectedCwd='';suggests=[];detectedDuplicates=[];
      if(editing){
        name=editing.name;services=structuredClone($state.snapshot(projectServices(editing))).map(s=>({...s,kind:s.kind??candidateKind({command:s.command,cwd:s.cwd,label:s.name,reason:''})}));rootCwd=commonProjectDirectory(services)||editing.cwd;group=editing.group??'';
        plans=structuredClone($state.snapshot(editing.launchPlans??[]));
        if(!plans.length)plans=[{id:'default-plan',name:'启动方案',serviceIds:services.map(s=>s.id),startMode:editing.startMode??'parallel'}];
        defaultPlanId=plans.some(p=>p.id===editing?.defaultPlanId)?editing.defaultPlanId:plans[0]?.id;selectedId=defaultPlanId??plans[0]?.id??'';starter=false;
      }else if(draft){const saved=structuredClone($state.snapshot(draft));({name,rootCwd,group,services,plans,defaultPlanId,selectedId,starter}=saved);}
      else{name='';rootCwd='';group='';const service=blankService('default');services=[service];plans=[{id:'default-plan',name:'启动方案',serviceIds:[service.id],startMode:'parallel'}];defaultPlanId=selectedId=plans[0].id;starter=true;}
      if(!editing)void loadSuggests(sessionVersion);
       if(editing&&focusServiceId)void focusError(focusServiceId);
    });
  });
  async function loadSuggests(session:number){suggesting=true;try{const result=await suggestProjects();if(session===sessionVersion&&open)suggests=result;}catch{if(session===sessionVersion)suggests=[];}finally{if(session===sessionVersion)suggesting=false;}}
  function setRoot(value:string){const previous=rootCwd;rootCwd=value;for(const service of services)if(!service.cwd.trim()||service.cwd===previous)service.cwd=value;}
  function applySuggest(s:PortSuggest){setRoot(s.cwd);if(!name.trim())name=projectPathName(s.cwd);services=[{...blankService('default'),name:'服务',cwd:s.cwd,command:s.command,port:s.port,openUrl:`http://localhost:${s.port}`}];plans=[{id:'default-plan',name:'启动方案',serviceIds:['default'],startMode:'parallel'}];defaultPlanId=selectedId=plans[0].id;starter=false;suggests=[];notice='已从运行中的进程填写配置，请检查启动命令。';}
  async function browse(){try{const path=await openDialog({directory:true,multiple:false,title:'选择项目目录'});if(typeof path==='string'&&open){setRoot(path);await runDetect(true);}}catch(e){error=String(e);}}
  function sameDirectory(a:string,b:string){const normalize=(path:string)=>path.split('\\').join('/').replace(/\/+$/,'').toLowerCase();return normalize(a)===normalize(b);}
  function candidateService(candidate:CommandCandidate){return services.find(s=>sameDirectory(s.cwd,candidate.cwd)&&s.command.trim()===candidate.command.trim());}
   function addCandidate(candidate:CommandCandidate){const existing=candidateService(candidate);if(existing)return existing;const kind=candidateKind(candidate);const service:ProjectService={id:uid('service'),name:serviceKindLabels[kind],kind,cwd:candidate.cwd,command:candidate.command,port:kind==='desktop'?undefined:candidate.port,openUrl:kind==='desktop'?undefined:candidate.openUrl,readiness:'auto',startupTimeoutSecs:kind==='desktop'?600:60};services=[...services,service];return service;}
  async function runDetect(automatic=false){
    if(saving||detecting)return;const path=rootCwd.trim();if(!path){if(!automatic)error='请先填写项目目录';return;}if(automatic&&detectedCwd===path)return;
    if(!name.trim())name=projectPathName(path);const version=++detectVersion;const before=JSON.stringify({services,plans});error='';notice='';detecting=true;
    try{
      const result=await ondetect(path);if(version!==detectVersion||!open||rootCwd.trim()!==path)return;
      candidates=result.candidates??[];
      if(!candidates.length&&result.command)candidates=[{label:'识别到的启动命令',cwd:result.cwd??path,command:result.command,port:result.port,reason:'项目配置'}];
      detectedCwd=path;
      if(!candidates.length){notice='没有识别到启动命令，可以在当前方案直接填写。';return;}
      if(JSON.stringify({services,plans})!==before){notice='识别完成，保留了你刚修改的配置；其他命令可从备用入口添加。';return;}
      const replaceStarter=starter&&services.every(s=>!s.command.trim());if(replaceStarter){services=[];plans=[];defaultPlanId=undefined;}
       const merged=mergeDetectedServices(services,candidates,()=>uid('service'));services=merged.services;const recommended=merged.recommended;
      const generated=generateLaunchPlans(recommended);let count=0;
       for(const plan of generated)if(!plans.some(p=>p.id===plan.id||sameLaunchPlan(p,plan))){plans=[...plans,plan];count++;}
       detectedDuplicates=plans.filter((plan,index)=>plans.slice(0,index).some(other=>sameLaunchPlan(other,plan))).map(p=>p.id);
      defaultPlanId??=plans[0]?.id;if(!plans.some(p=>p.id===selectedId))selectedId=defaultPlanId??plans[0]?.id??'';
       starter=false;const warnings=[...new Set([...(result.warnings??[]),...candidates.map(c=>c.warning).filter(Boolean)])];notice=(count?`识别完成 · 已整理 ${plans.length} 个方案，请选择左侧方案检查命令。`:'识别完成 · 已补齐可识别的空值，保留已有命令、手动参数和自定义方案。')+(warnings.length?` ${warnings.join('；')}`:'');
    }catch(e){if(version===detectVersion&&open)error=String(e);}finally{if(version===detectVersion)detecting=false;}
  }
  function planSummary(plan:LaunchPlan){return plan.serviceIds.map(id=>{const service=services.find(s=>s.id===id);return service?.name.trim()||serviceKindLabels[service?.kind??'unknown'];}).join(' + ')||'尚未配置命令';}
   function entryHint(plan:LaunchPlan){const id=resolvedOpenService(plan,services);const service=services.find(s=>s.id===id);return !service?'后台运行，不打开窗口':isDesktopService(service)?'就绪后自动显示 App 窗口':!service.openUrl?.trim()&&!service.port?'尚未识别网页地址，请填写前端端口或网页地址':'启动完成后尝试打开网页；进程运行不代表网页已就绪';}
   function removeDuplicate(id:string){const plan=plans.find(p=>p.id===id);if(!plan||!plans.some(p=>p.id!==id&&sameLaunchPlan(p,plan)))return;plans=plans.filter(p=>p.id!==id);detectedDuplicates=detectedDuplicates.filter(value=>value!==id);if(defaultPlanId===id)defaultPlanId=plans.find(p=>sameLaunchPlan(p,plan))?.id;if(selectedId===id)selectedId=defaultPlanId??plans[0]?.id??'';notice='已从编辑草稿移除重复方案，共用服务保留；点击保存后生效。';}
  async function selectPlan(id:string,focus=false){selectedId=id;pickerOpen=false;await tick();workspace?.scrollTo({top:0,behavior:'instant'});if(focus){nameInput?.focus();nameInput?.select();}}
  async function addPlan(){starter=false;const service=blankService();services=[...services,service];const plan:LaunchPlan={id:uid('plan'),name:`自定义方案 ${plans.filter(p=>p.id.startsWith('plan-')).length+1}`,serviceIds:[service.id],startMode:'parallel'};plans=[...plans,plan];defaultPlanId??=plan.id;notice='新方案已创建，可以直接填写下面的启动命令。';await selectPlan(plan.id,true);}
  async function addCommand(){if(!selected)return;starter=false;const service=blankService();services=[...services,service];selected.serviceIds=[...selected.serviceIds,service.id];await tick();const input=workspace?.querySelector<HTMLInputElement>(`[data-command-id="${service.id}"]`);input?.scrollIntoView({block:'center',behavior:'smooth'});input?.focus({preventScroll:true});notice='已新增命令，执行目录默认使用项目目录。';}
  function removeMember(id:string){if(!selected)return;selected.serviceIds=selected.serviceIds.filter(service=>service!==id);if(!plans.some(p=>p.serviceIds.includes(id))){services=services.filter(s=>s.id!==id);}notice='已从当前方案移除；其他方案仍在使用的服务会保留。';}
  async function deletePlan(){if(!selected)return;const old=selected;plans=plans.filter(p=>p.id!==old.id);services=services.filter(s=>!old.serviceIds.includes(s.id)||plans.some(p=>p.serviceIds.includes(s.id)));if(defaultPlanId===old.id)defaultPlanId=plans[0]?.id;await selectPlan(defaultPlanId??plans[0]?.id??'');notice='方案已移除，共用服务已保留。';}
  function joinService(id:string){if(!selected||selected.serviceIds.includes(id))return;selected.serviceIds=[...selected.serviceIds,id];pickerOpen=false;notice='已复用已有服务；修改命令会同步影响使用它的方案。';}
  function joinCandidate(candidate:CommandCandidate){if(!selected)return;const service=addCandidate(candidate);joinService(service.id);}
   async function focusError(serviceId?:string,planId?:string){invalidServiceId=serviceId??'';const owner=planId??plans.find(p=>p.serviceIds.includes(serviceId??''))?.id;if(owner)await selectPlan(owner);await tick();if(serviceId){const target=workspace?.querySelector<HTMLElement>(`[${serviceId===focusServiceId?'data-address-id':'data-command-id'}="${serviceId}"]`);target?.scrollIntoView({block:'center'});target?.focus();}}
  async function submit(e:Event){
    e.preventDefault();if(saving||detecting)return;error='';invalidServiceId='';name=name.trim()||projectPathName(rootCwd);if(!rootCwd.trim()||!name){error='请填写项目目录';return;}
    const cleaned=services.map(s=>({...s,name:s.name.trim(),cwd:s.cwd.trim(),command:s.command.trim(),openUrl:s.openUrl?.trim()||undefined,healthUrl:s.healthUrl?.trim()||undefined}));
    const savedPlans=plans.map(p=>({...p,name:p.name.trim(),openServiceId:resolvedOpenService(p,cleaned)}));
    const serviceError=validateServices(cleaned,savedPlans);if(serviceError){const bad=cleaned.find(s=>validateServices([s]));error=bad?`${bad.name||'任务'}：${!bad.command?'请填写启动命令':!bad.cwd?'请填写执行目录':validateServices([bad])}`:serviceError;await focusError(bad?.id);return;}
    const planError=validateLaunchPlans(savedPlans,cleaned,defaultPlanId);if(planError){error=planError;const bad=savedPlans.find(p=>validateLaunchPlans([p],cleaned));await focusError(undefined,bad?.id);return;}
    if(!savedPlans.length||!cleaned.length){error='至少添加一个启动方案和命令';return;}
    const first=cleaned[0];const project:Project={...editing,id:editing?.id??uid('project'),name,cwd:first.cwd,command:first.command,port:first.port,openUrl:first.openUrl||((first.kind!=='desktop'&&first.port)?`http://localhost:${first.port}`:undefined),group:group.trim()||undefined,services:cleaned,launchPlans:savedPlans,defaultPlanId:defaultPlanId??savedPlans[0].id,startMode:savedPlans.find(p=>p.id===defaultPlanId)?.startMode??'parallel'};
    const session=sessionVersion;saving=true;try{await onsave?.(project);if(session===sessionVersion){draft=null;onclose?.();}}catch(e){if(session===sessionVersion&&open)error=String(e);}finally{if(session===sessionVersion)saving=false;}
  }
</script>
<svelte:window onkeydown={e=>{if(open&&e.key==='Escape')requestClose();}}/>
{#if open}
<div class="overlay" role="presentation"><div class="modal" role="dialog" tabindex="-1" aria-modal="true" aria-labelledby="project-form-title" use:dialogFocus>
  <header class="modal-header"><h2 id="project-form-title">{title}<span>选择方案，直接编辑它要执行的命令</span></h2><button type="button" class="qd-btn qd-btn-ghost" aria-label="关闭项目表单" onclick={requestClose}><ActionIcon name="close" size={16}/></button></header>
  <form onsubmit={submit} novalidate aria-busy={saving}>
    <div class="split-layout">
      <aside class="plan-sidebar" aria-label="启动方案列表">
        <p class="sidebar-title">启动方案 · {plans.length} 个</p>
        {#each plans as plan (plan.id)}<button type="button" class="plan-nav" aria-pressed={selectedId===plan.id} disabled={saving} onclick={()=>selectPlan(plan.id)}><strong>{plan.name||'未命名方案'}{#if defaultPlanId===plan.id}<span>默认</span>{/if}</strong><small>{planSummary(plan)}</small></button>{/each}
        <button type="button" class="qd-btn qd-btn-secondary add-plan" disabled={saving||detecting} onclick={addPlan}><ActionIcon name="plus" size={16}/>新增方案</button>
        <p class="sidebar-hint">一个方案 = 点击一次启动的一组命令。可以共用后端，也可以使用独立命令。</p>
      </aside>
      <div class="workspace" bind:this={workspace}>
        <fieldset disabled={saving}>
          <div class="basics"><label>项目名称（选填）<input class="qd-input" bind:value={name} placeholder="留空使用目录名称"/></label><label class="directory">项目目录<div class="directory-row"><input class="qd-input font-mono" value={rootCwd} oninput={e=>setRoot(e.currentTarget.value)} onblur={()=>{if(!name.trim())name=projectPathName(rootCwd);if(!editing)void runDetect(true);}} placeholder="D:\Projects\my-app"/><button type="button" class="qd-btn qd-btn-secondary qd-btn-sm" onclick={browse}>浏览…</button></div></label><button type="button" class="qd-btn qd-btn-secondary detect" disabled={detecting} onclick={()=>runDetect()}>{#if detecting}<span class="qd-spinner"></span>{:else}<ActionIcon name="refresh" size={14}/>{/if}{detecting?'识别中…':'自动识别'}</button></div>
          <section class="group-settings" aria-label="项目分组">
            <div class="group-label"><strong>项目分组</strong><span>用于项目列表筛选 · 选填</span></div>
            <div class="group-row"><div class="group-choices" role="group" aria-label="选择分组"><button type="button" class="group-option" aria-pressed={!group.trim()} onclick={()=>group=''}>未分组</button>{#each groupChoices as option}<button type="button" class="group-option" aria-pressed={group.trim()===option} onclick={()=>group=option}>{option}</button>{/each}</div><label class="custom-group"><span>自定义分组</span><input class="qd-input" bind:value={group} placeholder="输入名称，或选择左侧分组"/></label></div>
          </section>
          <div class="recognition-status" role="status">{#if detecting}<span class="qd-spinner"></span>正在读取项目配置并整理方案…{:else if notice}<ActionIcon name="check" size={14}/>{notice}{:else}在当前方案直接填写命令，或自动识别项目入口。{/if}</div>
           {#if detectedDuplicates.length}<section class="service-picker" aria-label="重复方案预览"><p>以下方案的服务顺序、启动方式和打开入口与其他方案相同。只在点击移除并保存后删除，共用服务会保留。</p>{#each detectedDuplicates as id}<button type="button" class="qd-btn qd-btn-secondary qd-btn-sm" onclick={()=>removeDuplicate(id)}>移除重复方案：{plans.find(p=>p.id===id)?.name}</button>{/each}</section>{/if}
          {#if selected}
          <section class="plan-content" aria-label={`${selected.name || '未命名'} 方案编辑`}>
            <div class="plan-heading"><input class="qd-input plan-name" bind:this={nameInput} aria-label="当前方案名称" bind:value={selected.name} placeholder="官网 / 桌面 App / 自定义"/>{#if defaultPlanId===selected.id}<span class="default-tag">默认方案</span>{:else}<button type="button" class="qd-btn qd-btn-secondary qd-btn-sm" onclick={()=>{defaultPlanId=selected?.id;notice='已设为默认方案，项目主启动按钮会运行它。';}}>设为默认</button>{/if}<button type="button" class="qd-btn qd-btn-ghost qd-btn-sm" aria-label={`移除方案 ${selected.name}`} onclick={deletePlan}><ActionIcon name="close" size={14}/></button></div>
            <p class="entry-hint"><ActionIcon name="check" size={14}/>{entryHint(selected)}</p>
            <div class="commands">
              {#each selected.serviceIds as id,index (id)}
                {@const serviceIndex=services.findIndex(s=>s.id===id)}
                {#if serviceIndex>=0}<PlanServiceEditor bind:service={services[serviceIndex]} {index} sharedCount={plans.filter(p=>p.serviceIds.includes(id)).length} invalid={invalidServiceId===id} onremove={()=>removeMember(id)}/>{/if}
              {/each}
              {#if !selected.serviceIds.length}<p class="empty">还没有命令。点击“新增命令”直接填写，或复用已有服务。</p>{/if}
            </div>
            <div class="plan-tools"><button type="button" class="qd-btn qd-btn-secondary qd-btn-sm" onclick={addCommand}><ActionIcon name="plus" size={14}/>新增命令</button><button type="button" class="qd-btn qd-btn-secondary qd-btn-sm" aria-expanded={pickerOpen} onclick={()=>pickerOpen=!pickerOpen}>复用已有服务</button><div class="start-mode" role="group" aria-label="当前方案启动方式"><span>启动方式：</span><button type="button" class="qd-btn qd-btn-secondary qd-btn-sm" aria-pressed={selected.startMode==='parallel'} onclick={()=>{if(selected)selected.startMode='parallel';}}>同时</button><button type="button" class="qd-btn qd-btn-secondary qd-btn-sm" aria-pressed={selected.startMode==='sequential'} onclick={()=>{if(selected)selected.startMode='sequential';}}>按顺序</button></div></div>
            {#if selected.startMode==='sequential'&&selected.serviceIds.length>1}<p class="hint">按上方命令顺序启动，前一个就绪后启动下一个；可移出后重新加入以调整顺序。</p>{/if}
            {#if pickerOpen}<section class="service-picker" aria-label="复用已有服务"><p>加入已有命令，不会重复创建服务。共享命令的修改会同步到其他方案。</p>{#each services.filter(s=>!selected?.serviceIds.includes(s.id)) as service}<button type="button" class="qd-btn qd-btn-secondary qd-btn-sm" onclick={()=>joinService(service.id)}><ActionIcon name="plus" size={14}/>{service.name||serviceKindLabels[service.kind??'unknown']}<code>{service.command||'待填写'}</code></button>{/each}{#if services.every(s=>selected?.serviceIds.includes(s.id))}<small>已有服务都在当前方案中，需要独立命令时点击“新增命令”。</small>{/if}</section>{/if}
          </section>
          {:else}<div class="empty">点击左侧“新增方案”，添加一组启动命令。</div>{/if}
          {#if candidates.some(c=>!candidateService(c))&&selected}<details class="alternative"><summary>其他识别到的备用命令</summary><p class="hint">同一类型的备用命令不会重复加入；点击后添加到当前方案。</p>{#each candidates.filter(c=>!candidateService(c)) as candidate}<button type="button" class="candidate" onclick={()=>joinCandidate(candidate)}><span>{serviceKindLabels[candidateKind(candidate)]} · {candidate.label}<code>{candidate.command}</code><small>{candidate.cwd}</small></span><ActionIcon name="plus" size={16}/></button>{/each}</details>{/if}
          {#if !editing&&(suggesting||suggests.length)}<details class="process-suggestions"><summary>从正在运行的进程填写配置</summary>{#if suggesting}<p class="hint">扫描中…</p>{:else}{#each suggests as suggestion}<button type="button" class="qd-btn qd-btn-secondary" onclick={()=>applySuggest(suggestion)}>{suggestion.name??suggestion.process} · :{suggestion.port}</button>{/each}{/if}</details>{/if}
        </fieldset>
      </div>
    </div>
    <footer class="modal-footer">{#if error}<p class="error" role="alert">{error}</p>{/if}<div class="footer-actions"><span>{saving?'保存请求仍在处理，关闭后会继续完成':`${projectCount} 个已有项目 · 保存后从项目卡片选择方案`}</span><button type="button" class="qd-btn qd-btn-secondary" onclick={requestClose}>取消</button><button type="submit" class="qd-btn qd-btn-primary" disabled={saving||detecting}>{#if saving}<span class="qd-spinner qd-spinner-light"></span>{/if}{saving?'保存中…':editing?'保存':'添加'}</button></div></footer>
  </form>
</div></div>
{/if}
<style>
.overlay{position:fixed;inset:0;padding:16px;background:#0009;display:flex;align-items:center;justify-content:center;z-index:100;animation:qd-fade-in 160ms ease}.modal{width:min(1100px,100%);height:min(720px,calc(100dvh - 32px));display:flex;flex-direction:column;background:var(--color-qd-surface);border:1px solid var(--color-qd-border);border-radius:12px;box-shadow:0 24px 80px #0008;overflow:hidden}.modal-header{display:flex;align-items:center;justify-content:space-between;padding:12px 16px;border-bottom:1px solid var(--color-qd-border);flex-shrink:0}.modal-header h2{font-size:14px;font-weight:600;color:var(--color-qd-ink);margin:0}.modal-header h2 span{font-size:11px;color:var(--color-qd-muted);font-weight:400;margin-left:12px}form{display:flex;flex-direction:column;flex:1;min-height:0}.split-layout{display:grid;grid-template-columns:210px minmax(0,1fr);flex:1;min-height:0}.plan-sidebar{overflow:auto;border-right:1px solid var(--color-qd-border);padding:14px;background:var(--color-qd-window)}.sidebar-title{font-size:11px;color:var(--color-qd-muted);margin:0 0 12px}.plan-nav{width:100%;display:grid;gap:5px;padding:11px;text-align:left;border:1px solid var(--color-qd-border);border-radius:8px;background:var(--color-qd-surface);margin-bottom:8px;color:var(--color-qd-body);cursor:pointer}.plan-nav[aria-pressed=true]{border-color:var(--color-qd-brand);background:var(--color-qd-brand-soft)}.plan-nav strong{display:flex;gap:6px;align-items:center;font-size:12px;color:var(--color-qd-ink);overflow-wrap:anywhere}.plan-nav strong span{color:var(--color-qd-brand);font-size:10px;font-weight:400}.plan-nav small{font-size:10px;line-height:1.5;color:var(--color-qd-muted);overflow-wrap:anywhere}.add-plan{width:100%;border-style:dashed;margin-top:4px}.sidebar-hint{font-size:11px;color:var(--color-qd-muted);line-height:1.8;margin-top:16px}.workspace{min-height:0;overflow:auto;padding:16px 18px}fieldset{border:0;padding:0;margin:0;min-width:0}label{display:grid;gap:5px;color:var(--color-qd-muted);font-size:11px;min-width:0}.basics{display:grid;grid-template-columns:1fr 1.6fr auto;gap:10px;align-items:end}.directory-row{display:flex;gap:6px}.directory-row input{min-width:0;flex:1}.basics .qd-input{font-size:12px}.detect{height:34px}.group-settings{margin-top:14px;padding:12px;background:var(--color-qd-elevated);border:1px solid var(--color-qd-border);border-radius:8px}.group-label{display:flex;align-items:baseline;gap:10px;margin-bottom:10px}.group-label strong{font-size:12px;font-weight:500;color:var(--color-qd-ink)}.group-label>span{font-size:10px;color:var(--color-qd-muted)}.group-row{display:flex;gap:14px;align-items:center}.group-choices{display:flex;flex-wrap:wrap;gap:6px;flex:1;min-width:0}.group-option{padding:6px 12px;font-size:11px;color:var(--color-qd-body);background:var(--color-qd-surface);border:1px solid var(--color-qd-border);border-radius:6px;cursor:pointer}.group-option[aria-pressed=true]{color:var(--color-qd-brand);background:var(--color-qd-brand-soft);border-color:var(--color-qd-brand)}.custom-group{width:235px;max-width:45%;position:relative}.custom-group>span{position:absolute;top:-8px;left:8px;font-size:9px;background:var(--color-qd-elevated);padding:0 4px;color:var(--color-qd-muted)}.custom-group input{font-size:11px}summary{cursor:pointer;font-size:11px;color:var(--color-qd-muted)}.recognition-status{display:flex;align-items:center;gap:7px;font-size:11px;line-height:1.7;color:var(--color-qd-muted);min-height:28px;margin:7px 0}.plan-content{padding-top:12px;border-top:1px solid var(--color-qd-border)}.plan-heading{display:flex;align-items:center;gap:8px}.plan-name{font-size:13px;font-weight:600;flex:1;min-width:0}.default-tag{font-size:10px;color:var(--color-qd-brand);background:var(--color-qd-brand-soft);padding:5px 8px;border-radius:12px}.entry-hint{display:flex;gap:6px;align-items:center;font-size:11px;color:var(--color-qd-running);margin:10px 0}.commands{display:grid;gap:10px}.plan-tools{display:flex;gap:6px;align-items:center;flex-wrap:wrap;margin-top:12px}.start-mode{display:flex;gap:5px;align-items:center;margin-left:auto}.start-mode span{font-size:10px;color:var(--color-qd-muted)}.start-mode button[aria-pressed=true]{border-color:var(--color-qd-brand);color:var(--color-qd-brand);background:var(--color-qd-brand-soft)}.hint{font-size:11px;color:var(--color-qd-muted);line-height:1.7;margin:8px 0}.empty{padding:20px;border:1px dashed var(--color-qd-border);border-radius:8px;font-size:12px;color:var(--color-qd-muted)}.service-picker{display:flex;gap:6px;flex-wrap:wrap;padding:12px;border:1px dashed var(--color-qd-border);border-radius:8px;margin-top:12px}.service-picker p{width:100%;font-size:11px;margin:0 0 6px;color:var(--color-qd-muted)}.service-picker code{font-size:10px;color:var(--color-qd-muted);max-width:200px;overflow-wrap:anywhere;white-space:normal}.service-picker small{font-size:11px;color:var(--color-qd-muted)}.alternative,.process-suggestions{border-top:1px solid var(--color-qd-border);padding-top:12px;margin-top:14px}.candidate{width:100%;display:flex;align-items:center;justify-content:space-between;gap:10px;padding:10px;border:1px solid var(--color-qd-border);border-radius:8px;background:var(--color-qd-elevated);text-align:left;color:var(--color-qd-body);margin-top:8px;cursor:pointer}.candidate span{display:grid;gap:4px;font-size:12px}.candidate code,.candidate small{font-size:10px;overflow-wrap:anywhere}.candidate code{color:var(--color-qd-brand)}.candidate small{color:var(--color-qd-muted)}.modal-footer{padding:10px 16px;border-top:1px solid var(--color-qd-border);flex-shrink:0}.footer-actions{display:flex;align-items:center;gap:8px}.footer-actions>span{margin-right:auto;font-size:11px;color:var(--color-qd-muted)}.error{font-size:12px;color:var(--color-qd-danger);background:var(--color-qd-danger-soft);padding:8px;border-radius:6px;margin-bottom:8px}
@media(max-width:850px){.basics{grid-template-columns:1fr auto}.directory{grid-column:1/-1;grid-row:2}.split-layout{grid-template-columns:180px minmax(0,1fr)}.workspace{padding:14px}.group-row{flex-direction:column;align-items:stretch}.custom-group{max-width:none;width:100%;margin-top:3px}.modal-header h2 span{display:none}}
@media(max-width:600px){.split-layout{grid-template-columns:1fr;grid-template-rows:auto minmax(0,1fr)}.plan-sidebar{display:flex;gap:8px;overflow-x:auto;border-right:0;border-bottom:1px solid var(--color-qd-border);padding:8px}.sidebar-title,.sidebar-hint{display:none}.plan-nav{min-width:140px;margin:0;padding:8px}.add-plan{min-width:105px;width:auto;margin:0}.footer-actions>span{display:none}.start-mode{margin-left:0}}
</style>
