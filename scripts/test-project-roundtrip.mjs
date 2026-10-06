import fs from 'node:fs/promises';
import ts from 'typescript';
import assert from 'node:assert/strict';
const source=await fs.readFile('src/lib/services.ts','utf8');
const js=ts.transpileModule(source,{compilerOptions:{target:ts.ScriptTarget.ESNext,module:ts.ModuleKind.ESNext}}).outputText;
const helpers=await import('data:text/javascript;base64,'+Buffer.from(js).toString('base64'));
const {validateServices,validateLaunchPlans,projectServices,generateLaunchPlans}=helpers;
const original={id:'roundtrip',name:'官网与 App',cwd:'D:/project/web',command:'npm run dev',port:5173,services:[
{id:'web',name:'官网前端',cwd:'D:/project/web',command:'npm run dev',kind:'frontend',port:5173,readiness:'auto',startupTimeoutSecs:60},
{id:'app',name:'桌面 App',cwd:'D:/project',command:'npm run tauri dev',kind:'desktop',port:undefined,readiness:'auto',startupTimeoutSecs:600},
],launchPlans:[{id:'website',name:'官网',serviceIds:['web'],startMode:'parallel',openServiceId:'web'},{id:'desktop',name:'App',serviceIds:['app'],startMode:'parallel',openServiceId:'app'}],defaultPlanId:'website'};
assert.equal(validateServices(original.services,original.launchPlans),'');
// Rust Option::None is serialized as null, not omitted. Reproduce the real IPC return.
const returned=JSON.parse(JSON.stringify(original,(key,value)=>key==='port'&&value===undefined?null:value));
assert.equal(returned.services[1].port,null);
assert.equal(validateServices(projectServices(returned),returned.launchPlans),'','add -> Rust serialization -> edit without changes must save');
assert.equal(validateServices(returned.services,returned.launchPlans),'','validation also accepts a raw optional null');
const scriptOnly={id:'scripts',name:'后台脚本',cwd:'D:/scripts',command:'node worker.js',port:null,services:[{id:'one',name:'脚本一',cwd:'D:/scripts',command:'node one.js',port:null,kind:null},{id:'two',name:'脚本二',cwd:'D:/scripts',command:'node two.js',port:null,kind:null}],launchPlans:[{id:'scripts',name:'脚本',serviceIds:['one','two'],startMode:'parallel'}]};
assert.equal(validateServices(projectServices(scriptOnly),scriptOnly.launchPlans),'');
assert.equal(validateLaunchPlans(scriptOnly.launchPlans,scriptOnly.services),'','two missing ports do not conflict');
assert.equal(generateLaunchPlans(scriptOnly.services).some(p=>p.id==='all'),true,'missing ports do not hide combined plan');
assert.equal(validateServices(projectServices({...scriptOnly,services:undefined})),'','legacy optional null remains empty');
for(const invalid of [0,-1,65536,1.5,NaN,'5173'])assert.notEqual(validateServices([{...original.services[0],port:invalid}]),'','invalid explicit port still rejected');
assert.notEqual(validateLaunchPlans([{id:'same',name:'重复端口',serviceIds:['web','app'],startMode:'parallel'}],original.services.map(s=>({...s,port:5173}))),'');
console.log('PASS real null serialization roundtrip, desktop/legacy/no-port tasks and invalid/duplicate port guards');

const normalized=helpers.normalizeProject(scriptOnly);
assert.equal(normalized.port,undefined);assert.equal(normalized.services[0].port,undefined);assert.equal(normalized.services[0].kind,undefined);
assert.equal(helpers.normalizeProject({...scriptOnly,port:0}).port,0,'normalization never hides invalid explicit numbers');
// Exercise the actual bootstrap/list IPC adapters with a Rust-shaped response.
const ipcSource=await fs.readFile('src/lib/ipc.ts','utf8');
const ipcJs=ts.transpileModule(ipcSource,{compilerOptions:{target:ts.ScriptTarget.ESNext,module:ts.ModuleKind.ESNext}}).outputText;
const helpersURL='data:text/javascript;base64,'+Buffer.from(js).toString('base64');
const wire=[returned,scriptOnly];
const ipcMock=ipcJs.replace(/import \{ invoke \} from "@tauri-apps\/api\/core";/,'const invoke=async command=>command==="bootstrap"?{projects:wire,settings:{},statuses:[]}:wire;').replace('from "./services"',`from "${helpersURL}"`);
const ipc=await import('data:text/javascript;base64,'+Buffer.from(`const wire=${JSON.stringify(wire)};\n${ipcMock}`).toString('base64'));
for(const projects of [(await ipc.bootstrap()).projects,await ipc.listProjects()]){
  assert.equal(projects[0].services[1].port,undefined);
  assert.equal(validateServices(projectServices(projects[0]),projects[0].launchPlans),'');
  assert.equal(projects[1].port,undefined);assert.equal(projects[1].services[0].kind,undefined);
}
console.log('PASS bootstrap and list_projects normalize Rust null values on every read path');
