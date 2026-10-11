import fs from 'node:fs/promises';
import ts from 'typescript';
import assert from 'node:assert/strict';
const source=await fs.readFile('src/lib/services.ts','utf8');
const js=ts.transpileModule(source,{compilerOptions:{target:ts.ScriptTarget.ESNext,module:ts.ModuleKind.ESNext}}).outputText;
const {generateLaunchPlans,validateLaunchPlans,validateServices,candidateKind,automaticOpenService,resolvedOpenService,projectPathName,commonProjectDirectory,sameLaunchPlan,mergeDetectedServices}=await import('data:text/javascript;base64,'+Buffer.from(js).toString('base64'));
const services=[{id:'web',name:'官网前端',cwd:'D:/web',command:'pnpm dev',kind:'frontend'},{id:'api',name:'官网后端',cwd:'D:/api',command:'python manage.py runserver',kind:'backend'},{id:'app',name:'桌面端',cwd:'D:/app',command:'npm run tauri dev',kind:'desktop'}];
const plans=generateLaunchPlans(services);
assert.deepEqual(plans.map(p=>p.id),['website','app-1','all']);
assert.deepEqual(plans[0].serviceIds,['api','web']);
assert.equal(plans[0].startMode,'sequential');assert.equal(plans[0].openServiceId,'web');
assert.deepEqual(plans[1].serviceIds,['app']);assert.equal(plans[1].openServiceId,'app');
assert.equal(validateLaunchPlans(plans,services,'website'),'');
assert.notEqual(validateLaunchPlans([{...plans[0],serviceIds:['missing']}],services),'');
assert.notEqual(validateLaunchPlans(plans,services,'missing'),'');
assert.notEqual(validateLaunchPlans([{...plans[0],openServiceId:'app'}],services),'');
assert.equal(candidateKind({command:'npm run dev',kind:'desktop'}),'desktop');
assert.equal(candidateKind({command:'python manage.py runserver',reason:'manage.py'}),'backend');
assert.equal(candidateKind({command:'custom launch',cwd:'D:/custom',label:'自定义',reason:''}),'unknown');
const tauriServices=[{...services[0],cwd:'D:/root'},{...services[2],cwd:'D:/root'}];
assert.deepEqual(generateLaunchPlans(tauriServices).map(p=>p.id),['website','app-1']);
const alternate=[{...services[0],port:5173},{...services[0],id:'preview',port:5173}];
const exclusive=[{id:'dev',name:'开发',serviceIds:['web'],startMode:'parallel'},{id:'preview',name:'预览',serviceIds:['preview'],startMode:'parallel'}];
assert.equal(validateServices(alternate,exclusive),'');assert.equal(validateLaunchPlans(exclusive,alternate),'');
assert.notEqual(validateServices(alternate),'');assert.notEqual(validateLaunchPlans([{...exclusive[0],serviceIds:['web','preview']}],alternate),'');
console.log('PASS launch plan generation, dependency order, open targets, validation and classification');

assert.equal(projectPathName('C:\\Projects\\中文项目\\'), '中文项目');
assert.equal(projectPathName('/projects/site/'), 'site');
assert.equal(automaticOpenService(plans[0],services),'web');
assert.equal(automaticOpenService(plans[1],services),'app');
assert.equal(automaticOpenService(plans[2],services),'app');
assert.equal(automaticOpenService({serviceIds:['api']},services),undefined);
console.log('PASS automatic project names and automatic App/web/background entry selection');

assert.equal(commonProjectDirectory([{cwd:'D:/Projects/site/web'},{cwd:'D:/Projects/site/api'}]),'D:/Projects/site');
assert.equal(commonProjectDirectory([{cwd:'D:/one/shared'},{cwd:'D:/two/shared'}]),'D:/one/shared');
assert.equal(commonProjectDirectory([{cwd:'D:/site/web'}]),'D:/site/web');
console.log('PASS separate recognition root and service directories');

assert.deepEqual(generateLaunchPlans(services.slice(0,2)).map(p=>p.id),['website']);
assert.equal(sameLaunchPlan(plans[0],{...plans[0],id:'renamed',name:'其他名称'}),true);
assert.equal(sameLaunchPlan(plans[0],{...plans[0],serviceIds:['web','api']}),false);
assert.equal(sameLaunchPlan(plans[0],{...plans[0],openServiceId:'api'}),false);
assert.equal(sameLaunchPlan(plans[0],{...plans[0],startMode:'parallel'}),false);
const candidate={cwd:'d:\\web\\',command:'npm run dev',kind:'frontend',port:6123,openUrl:'http://127.0.0.2:6123/',label:'Vite',reason:'配置'};
const filled=mergeDetectedServices([{...services[0],command:'custom dev'}],[candidate],()=>{throw Error('must reuse existing service');});
assert.equal(filled.services[0].port,6123);assert.equal(filled.services[0].openUrl,candidate.openUrl);assert.equal(filled.services[0].command,'custom dev');
assert.deepEqual(mergeDetectedServices(filled.services,[candidate,candidate],()=>{throw Error('must remain idempotent');}),filled);
const manual={...services[0],port:9001,openUrl:'http://localhost:9001/admin',command:'manual launch'};
assert.deepEqual(mergeDetectedServices([manual],[candidate],()=>{throw Error('must reuse');}).services,[manual]);
assert.equal(mergeDetectedServices([{...manual,openUrl:undefined}],[candidate],()=>{throw Error('must reuse');}).services[0].openUrl,undefined,'detected URL must not contradict manual port');
assert.equal(mergeDetectedServices([],[candidate,candidate],()=> 'new').services.length,1);
console.log('PASS semantic plan deduplication, literal address merging, manual overrides and repeated-scan idempotence');

const mixed={id:'mixed',name:'网页和 App',serviceIds:['web','app','api'],startMode:'parallel',openServiceId:'web'};
for(const target of ['web','app','api']) {
  let saved={...mixed,openServiceId:target};
  for(let pass=0;pass<3;pass++) {
    saved=JSON.parse(JSON.stringify({...saved,openServiceId:resolvedOpenService(saved,services)}));
    assert.equal(saved.openServiceId,target,'valid explicit target survives repeated saves');
  }
}
assert.equal(resolvedOpenService({...mixed,openServiceId:'deleted'},services),'app');
assert.equal(resolvedOpenService({...mixed,serviceIds:['web','api'],openServiceId:'app'},services),'web');
assert.equal(resolvedOpenService({...mixed,serviceIds:['missing','web'],openServiceId:'missing'},services),'web');
console.log('PASS mixed web/desktop explicit entry survives repeated serialization; invalid entry falls back');
