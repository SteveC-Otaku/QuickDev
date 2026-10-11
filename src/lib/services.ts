import type { Project, ProjectService, LaunchPlan, CommandCandidate } from "./types";

export const serviceKindLabels = {frontend: '网页前端', backend: '后端服务', desktop: '桌面 App', unknown: '待确认'};
export function candidateKind(candidate: CommandCandidate): NonNullable<ProjectService['kind']> {
  if (candidate.kind) return candidate.kind;
  if (isDesktopService(candidate)) return 'desktop';
  if (/django|manage\.py|uvicorn|后端/i.test(`${candidate.command} ${candidate.reason}`)) return 'backend';
  if (/vite|next|nuxt|astro|\/web\b|\\web\b|官网|前端/i.test(`${candidate.command} ${candidate.cwd} ${candidate.label} ${candidate.reason}`)) return 'frontend';
  return 'unknown';
}
export function generateLaunchPlans(services: ProjectService[]): LaunchPlan[] {
  const kind = (service: ProjectService) => service.kind ?? (isDesktopService(service) ? 'desktop' : 'unknown');
  const front = services.filter(s => kind(s) === 'frontend');
  const back = services.filter(s => kind(s) === 'backend');
  const desktop = services.filter(s => kind(s) === 'desktop');
  const plans: LaunchPlan[] = [];
  if (front.length) plans.push({id:'website', name:'官网', serviceIds:[...back,...front].map(s=>s.id), startMode:back.length ? 'sequential' : 'parallel', openServiceId:front[0].id});
  else if (back.length) plans.push({id:'backend',name:'后端',serviceIds:back.map(s=>s.id),startMode:'parallel'});
  desktop.forEach((service,index) => plans.push({id:`app-${index+1}`,name:desktop.length === 1 ? '桌面 App' : `桌面 App ${index+1}`,serviceIds:[service.id],startMode:'parallel',openServiceId:service.id}));
  services.filter(s=>kind(s)==='unknown').forEach((service,index)=>plans.push({id:`task-${index+1}`,name:service.name.trim() || `任务 ${index+1}`,serviceIds:[service.id],startMode:'parallel'}));
  const sameDirectory=(a:string,b:string)=>a.replace(/\\/g,'/').replace(/\/$/,'').toLowerCase()===b.replace(/\\/g,'/').replace(/\/$/,'').toLowerCase();
  const managedFrontend=desktop.some(app=>/\btauri\b/i.test(app.command) && front.some(web=>sameDirectory(app.cwd,web.cwd)));
  const ports=services.map(s=>s.port).filter(p=>p!=null);
  if (services.length > 1 && !managedFrontend && new Set(ports).size===ports.length) plans.push({id:'all',name:'全部',serviceIds:[...back,...front,...desktop,...services.filter(s=>kind(s)==='unknown')].map(s=>s.id),startMode:'sequential',openServiceId:front[0]?.id ?? desktop[0]?.id});
  return plans.filter((plan,index)=>!plans.slice(0,index).some(other=>sameLaunchPlan(other,plan)));
}
export function sameLaunchPlan(a: LaunchPlan, b: LaunchPlan): boolean {
  return a.startMode === b.startMode && a.openServiceId === b.openServiceId
    && a.serviceIds.length === b.serviceIds.length && a.serviceIds.every((id,index)=>id===b.serviceIds[index]);
}

// Re-scan supplies missing data only. Existing commands and explicit overrides win.
export function fillDetectedService(service: ProjectService, candidate: CommandCandidate): ProjectService {
  const port = service.port ?? candidate.port ?? undefined;
  const address = candidate.openUrl && (!service.port || service.port === candidate.port) ? candidate.openUrl : undefined;
  return {...service, command:service.command.trim() ? service.command : candidate.command,
    port, openUrl:service.openUrl?.trim() ? service.openUrl : address};
}

export function mergeDetectedServices(existing: ProjectService[], candidates: CommandCandidate[], newId: ()=>string) {
  const normalize=(cwd:string)=>cwd.replace(/\\/g,'/').replace(/\/+$/,'').toLowerCase();
  const seen=new Set<string>();
  const services=[...existing], recommended:ProjectService[]=[];
  for(const candidate of candidates){
    const kind=candidateKind(candidate), key=`${normalize(candidate.cwd)}|${kind}`;
    if(seen.has(key))continue;
    seen.add(key);
    const index=services.findIndex(s=>normalize(s.cwd)===normalize(candidate.cwd)&&s.kind===kind);
    const detected=kind==='desktop'?{...candidate,port:undefined,openUrl:undefined}:candidate;
    const service=index>=0?fillDetectedService(services[index],detected):{
      id:newId(),name:serviceKindLabels[kind],kind,cwd:candidate.cwd,command:candidate.command,
      port:detected.port??undefined,openUrl:detected.openUrl,readiness:'auto' as const,startupTimeoutSecs:kind==='desktop'?600:60,
    };
    if(index>=0)services[index]=service;else services.push(service);
    recommended.push(service);
  }
  return {services,recommended};
}

export function serviceOpenIssue(service: ProjectService, status?: string): 'address' | 'not-ready' | undefined {
  if (!isDesktopService(service) && !service.openUrl?.trim() && !service.port) return 'address';
  return status !== 'running' ? 'not-ready' : undefined;
}
export function validateLaunchPlans(plans: LaunchPlan[], services: ProjectService[], defaultId?: string): string {
  const ids = new Set<string>();
  for (const plan of plans) {
    if (!/^[a-zA-Z0-9_-]+$/.test(plan.id) || ids.has(plan.id) || !plan.name.trim()) return '请填写启动方案名称，方案 ID 不能重复';
    ids.add(plan.id);
    if (!plan.serviceIds.length || new Set(plan.serviceIds).size !== plan.serviceIds.length || plan.serviceIds.some(id=>!services.some(s=>s.id===id))) return '每个启动方案至少选择一个有效服务';
    if (plan.openServiceId && !plan.serviceIds.includes(plan.openServiceId)) return '打开入口需属于该方案';
    const ports=plan.serviceIds.map(id=>services.find(s=>s.id===id)?.port).filter(p=>p!=null);
    if(new Set(ports).size!==ports.length) return `方案“${plan.name}”中的服务端口重复，请拆成不同方案`;
  }
  return defaultId && !ids.has(defaultId) ? '请选择有效的默认启动方案' : '';
}

export function isDesktopService(service: Pick<ProjectService, 'command' | 'readiness' | 'kind'>): boolean {
  const words = service.command.split(/\s+/).map(token => token.replace(/^['"]|['"]$/g, '').toLowerCase());
  return service.readiness === 'window' || ((service.readiness ?? 'auto') === 'auto' && (service.kind === 'desktop' || words.some(token => ['electron', 'electron-vite'].includes(token)) || words.includes('tauri') && words.includes('dev')));
}

export function projectServices(project: Project): ProjectService[] {
  return project.services?.length ? project.services.map((service, index) => ({...service, name: service.name.trim() || `任务 ${index + 1}`, port:service.port??undefined, kind:service.kind??undefined})) : [{ id: "default", name: "服务", cwd: project.cwd, command: project.command, port: project.port??undefined, openUrl: project.openUrl, readiness: "auto", startupTimeoutSecs: 60 }];
}

export function validateServices(services: ProjectService[], plans:LaunchPlan[]=[]): string {
  const ids = new Set<string>(), ports = new Set<number>();
  for (const service of services) {
    if(service.kind!=null && !Object.keys(serviceKindLabels).includes(service.kind)) return '服务分类无效';
    if (!service.cwd.trim() || !service.command.trim()) return "每个任务都需要执行目录和启动命令";
    if (!/^[a-zA-Z0-9_-]+$/.test(service.id) || ids.has(service.id)) return "服务 ID 无效或重复";
    ids.add(service.id);
    if (service.port != null) {
      if (!Number.isInteger(service.port) || service.port < 1 || service.port > 65535 || !plans.length && ports.has(service.port)) return "服务端口必须为 1–65535，联合启动的服务不能使用相同端口";
      ports.add(service.port);
    }
    const timeout = service.startupTimeoutSecs ?? 60;
    if (!Number.isInteger(timeout) || timeout < 1 || timeout > 3600) return "启动超时必须为 1–3600 秒";
    if (service.readiness === "port" && !service.port) return "端口检查需要填写预期端口";
    for (const url of [service.openUrl, service.healthUrl].filter(Boolean)) {
      try { if (!["http:", "https:"].includes(new URL(url!).protocol)) return "地址需要使用 http:// 或 https://"; }
      catch { return "地址格式不正确"; }
    }
    if (service.readiness === "http") {
      try { if (!["localhost", "127.0.0.1", "[::1]"].includes(new URL(service.healthUrl ?? "").hostname)) return "健康检查地址必须指向本机"; }
      catch { return "HTTP 检查需要完整的本机健康地址"; }
    }
  }
  return "";
}

export function projectPathName(path: string): string {
  return path.trim().replace(/[\\/]+$/, '').split(/[\\/]/).at(-1) || '';
}
export function automaticOpenService(plan: LaunchPlan, services: ProjectService[]): string | undefined {
  const members = plan.serviceIds.map(id=>services.find(s=>s.id===id)).filter((s): s is ProjectService=>!!s);
  return members.find(s=>isDesktopService(s))?.id
    ?? members.find(s=>s.kind==='frontend')?.id
    ?? members.find(s=>s.kind!=='backend' && (s.openUrl || s.port))?.id;
}
export function resolvedOpenService(plan: LaunchPlan, services: ProjectService[]): string | undefined {
  return plan.openServiceId && plan.serviceIds.includes(plan.openServiceId) && services.some(s=>s.id===plan.openServiceId)
    ? plan.openServiceId : automaticOpenService(plan,services);
}

export function commonProjectDirectory(services: ProjectService[]): string {
  const parts=services.map(s=>s.cwd.replace(/\\/g,'/').replace(/\/$/,'').split('/'));
  if(!parts.length) return '';
  let count=0;
  while(count<parts[0].length && parts.every(path=>path[count]?.toLowerCase()===parts[0][count].toLowerCase())) count++;
  return count>1 ? parts[0].slice(0,count).join('/') : services[0].cwd;
}

// Rust serializes missing optional values as null; keep the UI's optional fields consistent.
export function normalizeProject(project: Project): Project {
  return {...project, port:project.port??undefined,
    services:project.services?.map(service=>({...service,port:service.port??undefined,kind:service.kind??undefined}))};
}
