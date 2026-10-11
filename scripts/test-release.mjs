import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
const read = p => JSON.parse(fs.readFileSync(p, 'utf8'));
const version = read('package.json').version;
assert.equal(read('package-lock.json').version, version);
assert.equal(read('package-lock.json').packages[''].version, version);
assert.equal(read('src-tauri/tauri.conf.json').version, version);
assert.equal(read('release/changelog.json')[0].version, version);
assert.equal(fs.readFileSync('src-tauri/Cargo.toml','utf8').match(/^version = "([^"]+)"/m)[1],version);
assert.equal(fs.readFileSync('src-tauri/Cargo.lock','utf8').match(/name = "quick-dev"\r?\nversion = "([^"]+)"/)[1],version);
const root = fs.mkdtempSync(path.join(os.tmpdir(), 'quickdev-release-test-'));
try {
  for (const dir of ['scripts','src-tauri','release/dist/maintenance','website/img']) fs.mkdirSync(path.join(root,dir),{recursive:true});
  fs.copyFileSync('scripts/release.mjs',path.join(root,'scripts/release.mjs'));
  fs.writeFileSync(path.join(root,'src-tauri/tauri.conf.json'),JSON.stringify({version}));
  fs.copyFileSync('release/changelog.json',path.join(root,'release/changelog.json'));
  const sentinel=path.join(root,'release/dist/maintenance/keep.txt');fs.writeFileSync(sentinel,'keep');
  fs.writeFileSync(path.join(root,'website/index.html'),'__VERSION__ __SIZE_X64__ __SIZE_ARM64__ <!--CHANGELOG-->');
  for(const [triple,arch] of [['x86_64-pc-windows-msvc','x64'],['aarch64-pc-windows-msvc','arm64']]){
    const dir=path.join(root,'src-tauri/target',triple,'release/bundle/nsis');fs.mkdirSync(dir,{recursive:true});
    const exe=path.join(dir,'QuickDev_'+version+'_'+arch+'-setup.exe');fs.writeFileSync(exe,'fixture');fs.writeFileSync(exe+'.sig','fixture-signature');
  }
  for(let i=0;i<2;i++)execFileSync(process.execPath,[path.join(root,'scripts/release.mjs'),'--skip-build'],{cwd:root,stdio:'pipe'});
  assert.equal(fs.readFileSync(sentinel,'utf8'),'keep');
  const out=path.join(root,'release/dist','v'+version);
  assert.equal(read(path.join(out,'github/latest.json')).version,version);
  assert.ok(read(path.join(out,'github/latest.json')).platforms['windows-x86_64'].url.includes('/v'+version+'/'));
  assert.ok(fs.readFileSync(path.join(out,'oss/index.html'),'utf8').includes(version));
  const githubManifest=fs.readFileSync(path.join(out,'github/latest.json'),'utf8');
  const githubInstaller=fs.readFileSync(path.join(out,'github',`QuickDev_${version}_x64-setup.exe`));
  execFileSync(process.execPath,[path.join(root,'scripts/release.mjs'),'--website-only'],{cwd:root,stdio:'pipe'});
  assert.equal(fs.readFileSync(path.join(out,'github/latest.json'),'utf8'),githubManifest);
  assert.deepEqual(fs.readFileSync(path.join(out,'github',`QuickDev_${version}_x64-setup.exe`)),githubInstaller);
  for(const arch of ['x64','arm64'])fs.writeFileSync(path.join(out,'github',`QuickDev_${version}_${arch}-setup.exe.sig`),'fixture-signature');
  fs.renameSync(path.join(root,'src-tauri/target'),path.join(root,'src-tauri/retained-target-fixture'));
  execFileSync(process.execPath,[path.join(root,'scripts/release.mjs'),'--website-only'],{cwd:root,stdio:'pipe'});
  assert.equal(fs.readFileSync(path.join(out,'github/latest.json'),'utf8'),githubManifest);
  console.log('PASS version consistency; isolated dual-architecture packaging; repeat release preserves previous deliveries (fixture artifacts only)');
} finally {fs.rmSync(root,{recursive:true,force:true});}
