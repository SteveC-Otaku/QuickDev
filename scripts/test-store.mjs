import { readFile, writeFile, mkdir, rm } from 'node:fs/promises';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import assert from 'node:assert/strict';
import { compileModule } from 'svelte/compiler';
import ts from 'typescript';

// Exercise the real rune store with controlled IPC, never the user's processes/config.
const source = await readFile('src/lib/store.svelte.ts', 'utf8');
const names = ['isTauri', 'listen', 'openUrl', 'openServiceEntry', 'bootstrap', 'exportConfig', 'getDevLogs', 'importConfig', 'loadSettings', 'killAndRestart', 'killProcess', 'listDevStatus', 'listPorts', 'listProjects', 'removeProject', 'startDev', 'startService', 'focusService', 'stopService', 'stopAllDevs', 'stopDev', 'updateSettings', 'upsertProject'];
const stripped = source.replace(/^import[\s\S]*?from\s+"[^"]+";\s*/gm, '');
const prefix = names.map(name => `const ${name} = (...args) => globalThis.__qdTest.${name}(...args);`).join('\n');
const helpers = (await readFile('src/lib/services.ts', 'utf8')).replace(/^import.*$/gm, '').replace(/export /g, '');
const js = ts.transpileModule(prefix + helpers + stripped, { compilerOptions: { target: ts.ScriptTarget.ESNext, module: ts.ModuleKind.ESNext } }).outputText;
const compiled = compileModule(js, { filename: 'store.svelte.js', generate: 'client' }).js.code;
const dir = resolve('node_modules/.cache/quick-dev-tests');
await mkdir(dir, { recursive: true });
const file = resolve(dir, 'store.mjs');
await writeFile(file, compiled);
const project = { id: 'one', name: '测试项目', cwd: 'C:/test', command: 'npm run dev' };
const defaults = { highlightPorts: [3000], autoRefresh: true, minimizeToTray: true, autoStart: false, hotkey: 'Ctrl+Shift+D', shell: 'powershell' };
let count = 0;
const deferred = () => { let resolve; const promise = new Promise(r => resolve = r); return { promise, resolve }; };
async function test(name, run) {
  globalThis.__qdTest = Object.fromEntries(names.map(n => [n, async () => undefined]));
  Object.assign(globalThis.__qdTest, { listPorts: async () => [], listProjects: async () => [project], listDevStatus: async () => [], getDevLogs: async () => [], loadSettings: async () => defaults });
  const { app } = await import(`${pathToFileURL(file).href}?case=${++count}`);
  try { await run(app, globalThis.__qdTest); console.log(`PASS ${name}`); }
  finally { app.dispose(); }
}
try {
  await test('缺网页地址提供定位配置，运行中不绕过编辑保护',async (app,ipc)=>{
    let browser=0;ipc.openUrl=async()=>browser++;ipc.openServiceEntry=async()=>{throw Error('网页前端尚未配置网页地址，请填写端口或网页地址');};
    const missing={...project,services:[{id:'web',name:'网页前端',cwd:'D:/web',command:'npm run dev',kind:'frontend'}],serviceStatuses:[{id:'web',status:'running'}],status:'running'};
    app.projects=[missing];await app.openService('one','web');
    assert.equal(browser,0);assert.match(app.notice.text,/尚未配置网页地址/);assert.deepEqual(app.notice.configuration,{projectId:'one',serviceId:'web'});
    await app.locateServiceConfiguration('one','web');assert.equal(app.formOpen,false);assert.equal(app.selectedProjectId,'one');assert.equal(app.selectedServiceId,'web');assert.match(app.notice.text,/先停止/);
    app.projects=[{...missing,status:'stopped',serviceStatuses:[{id:'web',status:'stopped'}]}];await app.locateServiceConfiguration('one','web');
    assert.equal(app.formOpen,true);assert.equal(app.editingServiceId,'web');assert.equal(app.editingProject.id,'one');
  });
  await test('未就绪给出等待提示，已就绪委托统一后端打开入口',async (app,ipc)=>{
    const service={id:'web',name:'前端',cwd:'D:/web',command:'npm run dev',kind:'frontend',port:6123,openUrl:'http://127.0.0.2:6123/'};
    app.projects=[{...project,services:[service],serviceStatuses:[{id:'web',status:'starting'}]}];let urls=[];ipc.openServiceEntry=async()=>{throw Error('前端尚未就绪，请查看日志或等待启动完成');};
    await app.openService('one','web');assert.match(app.notice.text,/尚未就绪/);assert.equal(urls.length,0);
    ipc.openServiceEntry=async (...args)=>urls.push(args);app.projects=[{...project,services:[service],serviceStatuses:[{id:'web',status:'running'}]}];await app.openService('one','web');assert.deepEqual(urls,[['one','web']]);
  });
  await test('桌面 App 委托统一入口，不直接打开浏览器', async (app, ipc) => {
    let focused = 0, browser = 0;
    ipc.openServiceEntry = async (id, service) => { assert.equal(id, 'desktop'); assert.equal(service, 'default'); focused++; };
    ipc.openUrl = async () => { browser++; };
    const desktop = {id:'desktop',name:'App',cwd:'C:/app',command:'npm run tauri dev',port:1420,status:'running',serviceStatuses:[{id:'default',status:'running',readiness:'window'}]};
    app.projects = [desktop];
    await app.handleOpen(desktop);
    assert.equal(focused, 1); assert.equal(browser, 0);
    app.ports = [{port:1420,pid:1,process:'node.exe',command:'vite',projectId:'desktop',serviceId:'default'}];
    await app.handleOpenPort(1420); assert.equal(focused,2); assert.equal(browser,0);
    ipc.openServiceEntry = async () => { throw Error('窗口尚未出现'); };
    await app.handleOpen(desktop); assert.match(app.notice.text, /窗口尚未出现/);
  });
  await test('运行时地址可打开，后端迟到的无地址与浏览器失败均可见',async (app,ipc)=>{
    const service={id:'web',name:'前端',cwd:'D:/web',command:'npm run dev',kind:'frontend'};
    app.projects=[{...project,services:[service],serviceStatuses:[{id:'web',status:'starting'}]}];
    let calls=0;ipc.openServiceEntry=async()=>calls++;await app.openService('one','web');assert.equal(calls,1);
    ipc.openServiceEntry=async()=>{throw Error('前端尚未配置网页地址');};await app.openService('one','web');assert.match(app.notice.text,/尚未配置网页地址/);assert.deepEqual(app.notice.configuration,{projectId:'one',serviceId:'web'});
    ipc.openServiceEntry=async()=>{throw Error('打开地址失败：测试浏览器拒绝');};await app.openService('one','web');assert.match(app.notice.text,/测试浏览器拒绝/);
    assert.equal(app.projects[0].services[0].openUrl,undefined);
  });
  await test('并发端口刷新合并为一次请求', async (app, ipc) => {
    let calls = 0; const pending = deferred();
    ipc.listPorts = () => { calls++; return pending.promise; };
    const a = app.refreshPorts(); const b = app.refreshPorts();
    assert.equal(app.portsLoading, true); assert.equal(calls, 1);
    pending.resolve([{ port: 3210, pid: 42 }]); await Promise.all([a, b]);
    assert.equal(app.portsLoading, false); assert.equal(app.ports[0].port, 3210);
  });
  await test('扫描失败保留上次结果并显示错误', async (app, ipc) => {
    app.ports = [{ port: 3000, pid: 42 }]; ipc.listPorts = async () => { throw Error('拒绝访问'); };
    await app.refreshPorts(); assert.equal(app.ports[0].port, 3000); assert.match(app.portsError, /拒绝访问/); assert.equal(app.portsLoading, false);
  });
  await test('快速切换项目不会被旧日志覆盖', async (app, ipc) => {
    const a = deferred(), b = deferred(); ipc.getDevLogs = id => id === 'a' ? a.promise : b.promise;
    const first = app.selectProject('a'); const second = app.selectProject('b');
    b.resolve(['b output']); await second; a.resolve(['a output']); await first;
    assert.equal(app.selectedProjectId, 'b'); assert.equal(app.logs[0].text, 'b output'); assert.equal(app.logsLoading, false);
  });
  await test('状态刷新保留冲突提示', async app => {
    app.conflict = { projectId: 'one', port: 3000, pid: 42, command: 'external' };
    app.applyStatuses([project], []); assert.equal(app.projects[0].status, 'conflict'); assert.equal(app.hasError, true);
  });
  await test('停止操作防止重复点击且失败后可重试', async (app, ipc) => {
    let calls = 0; const pending = deferred(); ipc.stopDev = () => { calls++; return pending.promise; };
    const first = app.handleStop('one'); await app.handleStop('one'); assert.equal(calls, 1);
    pending.resolve(); await first; assert.equal(app.stoppingIds.length, 0);
    ipc.stopDev = async () => { throw Error('停止失败'); }; await app.handleStop('one');
    assert.match(app.notice.text, /停止失败/); assert.equal(app.stoppingIds.length, 0);
  });
  await test('设置保存失败恢复真实持久化状态', async (app, ipc) => {
    ipc.updateSettings = async () => { throw Error('热键被占用'); };
    await app.handleSettingsChange({ ...defaults, hotkey: 'invalid' });
    assert.equal(app.settings.hotkey, defaults.hotkey); assert.equal(app.settingsBusy, false); assert.match(app.notice.text, /热键被占用/);
  });
  await test('结束进程取消确认不会调用系统命令', async (app, ipc) => {
    let calls = 0; ipc.killProcess = async () => calls++;
    const pending = app.handleKill(42); assert.ok(app.confirmation); app.resolveConfirmation(false); await pending;
    assert.equal(calls, 0); assert.equal(app.killBusy, false);
  });
  await test('停止完成不被整张端口表扫描拖住，等待期间反馈可见', async (app, ipc) => {
    const scan = deferred(), stop = deferred(); ipc.listPorts = () => scan.promise; ipc.stopDev = () => stop.promise;
    app.projects = [{ ...project, status: 'running' }];
    const pending = app.handleStop('one'); assert.match(app.operationText, /正在停止/);
    stop.resolve(); await pending;
    assert.equal(app.stoppingIds.length, 0); assert.equal(app.portsLoading, true);
    assert.equal(app.operationText, ''); scan.resolve([]); await app.refreshPorts();
  });
  await test('后端托盘启停状态也显示全局等待提示', async app => {
    app.applyStatuses([project], [{ id: 'one', status: 'starting' }]); assert.match(app.operationText, /正在启动/);
    app.applyStatuses([project], [{ id: 'one', status: 'stopping' }]); assert.match(app.operationText, /正在停止/); assert.equal(app.runningCount, 1);
    app.applyStatuses([project], []); assert.equal(app.operationText, '');
  });
  await test('服务状态拒绝迟到快照并保留部分运行与异常', async app => {
    app.applyStatuses([project], [{id:'one',status:'partial',revision:20,services:[{id:'api',name:'API',status:'running',sessionId:100},{id:'web',name:'前端',status:'failed',message:'启动超时'}]}]);
    app.applyStatusEvent({id:'one',status:'stopped',revision:19,services:[]});
    app.applyStatuses([project], [{id:'one',status:'stopped',revision:18,services:[]}]);
    assert.equal(app.projects[0].status,'partial'); assert.equal(app.projects[0].serviceStatuses[0].sessionId,100);assert.equal(app.hasError,true);assert.equal(app.runningCount,1);
  });
  await test('服务操作立即锁定且重启等待停止成功', async (app,ipc) => {
    const pending=deferred();let stops=0,starts=0;
    ipc.stopService=()=>{stops++;return pending.promise;};ipc.startService=async()=>{starts++;return {ok:true};};
    const first=app.handleService('one','api','restart');await app.handleService('one','api','restart');
    assert.deepEqual(app.serviceBusy,['one/api']);assert.equal(stops,1);assert.equal(starts,0);
    pending.resolve();await first;assert.equal(starts,1);assert.equal(app.serviceBusy.length,0);
    ipc.stopService=async()=>{throw Error('停止失败');};await app.handleService('one','api','restart');assert.equal(starts,1);assert.match(app.notice.text,/停止失败/);
  });
  await test('快速切换服务不会被旧服务日志覆盖', async (app,ipc) => {
    const old=deferred(),current=deferred();app.selectedProjectId='one';ipc.getDevLogs=(id,service)=>service==='api'?old.promise:current.promise;
    const first=app.selectLogService('api');const second=app.selectLogService('web');current.resolve(['web output']);await second;old.resolve(['api output']);await first;
    assert.equal(app.selectedServiceId,'web');assert.equal(app.logs[0].text,'web output');assert.equal(app.logsLoading,false);
  });
  await test('服务日志忽略旧会话并遵守当前筛选', async (app,ipc) => {
    const listeners=new Map();ipc.isTauri=()=>true;ipc.listen=async(name,fn)=>{listeners.set(name,fn);return ()=>listeners.delete(name);};
    const services=[{id:'api',name:'API',status:'running',sessionId:100},{id:'web',name:'前端',status:'running',sessionId:200}];
    ipc.bootstrap=async()=>({projects:[project],settings:defaults,statuses:[{id:'one',status:'running',revision:1,services}]});
    await app.boot();await app.selectProject('one');await app.selectLogService('web');
    const emit=payload=>listeners.get('dev-log')({payload});
    emit({id:'one',serviceId:'web',sessionId:199,line:'old output'});emit({id:'one',serviceId:'api',sessionId:100,line:'other service'});assert.equal(app.logs.length,0);
    emit({id:'one',serviceId:'web',sessionId:200,line:'new output'});assert.equal(app.logs.length,1);assert.equal(app.logs[0].text,'new output');
  });
  await test('保存首页和刷新偏好不额外扫描端口，高亮变更仍刷新', async (app,ipc) => {
    let scans=0; ipc.listPorts=async()=>{scans++;return [];};
    await app.handleSettingsChange({...app.settings,startPage:'projects',refreshIntervalSecs:10});
    assert.equal(scans,0);assert.equal(app.settings.startPage,'projects');
    await app.handleSettingsChange({...app.settings,highlightPorts:[12345]});
    assert.equal(scans,1);
  });
  await test('启动按持久化默认首页展示', async (app,ipc) => {
    ipc.isTauri=()=>true;ipc.listen=async()=>()=>{};
    ipc.bootstrap=async()=>({projects:[project],settings:{...defaults,startPage:'projects',refreshIntervalSecs:30},statuses:[]});
    await app.boot();assert.equal(app.activeTab,'projects');assert.equal(app.settings.refreshIntervalSecs,30);
  });
  await test('方案启停传递选中方案且保留冲突重试方案', async (app,ipc) => {
    app.projects=[project]; let started,stopped,retried;
    ipc.startDev=async (...args)=>{started=args;return {ok:false,conflict:{port:5173,pid:42,command:'external'}};};
    await app.handleStart('one','app');assert.deepEqual(started,['one','app']);assert.equal(app.conflict.planId,'app');
    app.ask=async()=>true;ipc.killAndRestart=async (...args)=>{retried=args;return {ok:true};};
    await app.handleKill(42);assert.deepEqual(retried,[42,'one','app']);
    ipc.stopDev=async (...args)=>{stopped=args;};await app.handleStop('one','website');assert.deepEqual(stopped,['one','website']);
  });
  await test('查看冲突端口切换页面并刷新，空结果有提示',async (app,ipc)=>{
    let scans=0;ipc.listPorts=async()=>{scans++;return [{port:5173,pid:42}];};
    await app.inspectPort(5173,42);assert.equal(app.activeTab,'ports');assert.equal(app.portFocus.port,5173);assert.equal(app.portFocus.pid,42);assert.equal(scans,1);
    ipc.listPorts=async()=>[];await app.inspectPort(8080,99);assert.match(app.notice.text,/已释放/);
  });
  await test('按方案打开对应网页或 App，不混用首个服务入口',async (app,ipc)=>{
    const projectWithPlans={...project,services:[{id:'web',name:'官网',cwd:'D:/web',command:'pnpm dev',port:5173},{id:'app',name:'App',cwd:'D:/app',command:'npm run tauri dev',readiness:'window'}],launchPlans:[{id:'website',name:'官网',serviceIds:['web'],openServiceId:'web'},{id:'desktop',name:'App',serviceIds:['app'],openServiceId:'app'}],defaultPlanId:'website'};
    app.projects=[projectWithPlans];ipc.listProjects=async()=>[projectWithPlans];ipc.listDevStatus=async()=>[{id:'one',status:'running',services:[{id:'web',status:'running'},{id:'app',status:'running'}]}];ipc.startDev=async()=>({ok:true});
    let entries=[];ipc.openServiceEntry=async (...args)=>entries.push(args);
    await app.handleStart('one','desktop');assert.deepEqual(entries,[['one','app']]);
    await app.handleStart('one','website');assert.deepEqual(entries,[['one','app'],['one','web']]);
  });
  console.log(`${count} store checks passed`);
} finally { delete globalThis.__qdTest; await rm(file, { force: true }); }
