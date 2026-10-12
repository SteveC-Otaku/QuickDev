import ctypes
from ctypes import wintypes
import json
import os
from pathlib import Path
import socket
import subprocess
import time
import threading
import urllib.request

import argparse
parser=argparse.ArgumentParser()
parser.add_argument("--python",required=True,help="Windows Python environment with Uvicorn installed")
parser.add_argument("--output",default="_tasks/2026-10-12-unified-reload")
options=parser.parse_args()
root=Path(__file__).resolve().parent.parent
temp=root/options.output
temp.mkdir(parents=True,exist_ok=True)
fixture=temp/'legacy stack 中文'
fixture.mkdir(exist_ok=True)
original=temp/'original-pythonpath'
original.mkdir(exist_ok=True)
(original/'sitecustomize.py').write_text("import os\nos.environ['ORIGINAL_SITE_INTACT']='yes'\n",encoding='utf-8')
guard=root/'src-tauri/src/process/python_reload'
python=str(Path(options.python).resolve(strict=True))
environment=os.environ | {'QUICKDEV_RELOAD_GUARD':'1','PYTHONPATH':str(guard)+os.pathsep+str(original),'PYTHONUTF8':'1'}
plain=subprocess.run([python,'-c',"import os; assert os.environ['ORIGINAL_SITE_INTACT']=='yes'; print('普通 Python 中文')"],env=environment,capture_output=True,encoding='utf-8',creationflags=subprocess.CREATE_NO_WINDOW,check=True)
assert '普通 Python 中文' in plain.stdout
exit_fixture=temp/'exit-code-fixture'
exit_fixture.mkdir(exist_ok=True)
(exit_fixture/'uvicorn.py').write_text("import ctypes, sys\nctypes.windll.kernel32.ExitProcess(int(sys.argv[-1]))\n",encoding='utf-8')
for expected in (23, 0xC0000005):
    result=subprocess.run([python,'-m','uvicorn','--reload',str(expected)],
        cwd=exit_fixture,env=environment,capture_output=True,timeout=10,
        creationflags=subprocess.CREATE_NO_WINDOW)
    assert result.returncode == expected, f'Exit code lost: {result.returncode} != {expected}'
print('PASS ordinary Python, original sitecustomize, Unicode and Windows exit codes')
(fixture/'package.json').write_text(json.dumps({'scripts':{'dev':'node frontend.cjs'}}))
(fixture/'frontend.cjs').write_text("require('http').createServer((req,res)=>res.end('ok')).listen(Number(process.env.FRONT_PORT),'127.0.0.1');console.log('FRONT_READY');",encoding='utf-8')
k=ctypes.WinDLL('kernel32',use_last_error=True)
k.CreateJobObjectW.restype=wintypes.HANDLE
k.CreateJobObjectW.argtypes=[ctypes.c_void_p,wintypes.LPCWSTR]
k.AssignProcessToJobObject.argtypes=[wintypes.HANDLE,wintypes.HANDLE]
k.TerminateJobObject.argtypes=[wintypes.HANDLE,wintypes.UINT]
k.CloseHandle.argtypes=[wintypes.HANDLE]
k.OpenProcess.argtypes=[wintypes.DWORD,wintypes.BOOL,wintypes.DWORD]
k.OpenProcess.restype=wintypes.HANDLE
k.IsProcessInJob.argtypes=[wintypes.HANDLE,wintypes.HANDLE,ctypes.POINTER(wintypes.BOOL)]
resume=ctypes.WinDLL('ntdll').NtResumeProcess
resume.argtypes=[wintypes.HANDLE]

def port():
    with socket.socket() as sock:
        sock.bind(('127.0.0.1',0)); return sock.getsockname()[1]

def write(version):
    (fixture/'app.py').write_text(f'''import json, os
assert os.environ.get('ORIGINAL_SITE_INTACT') == 'yes'
async def app(scope, receive, send):
    if scope['type'] != 'http': return
    await send({{'type':'http.response.start','status':200,'headers':[(b'content-type',b'application/json')]}})
    await send({{'type':'http.response.body','body':json.dumps({{'version':{version},'pid':os.getpid()}}).encode()}})
''',encoding='utf-8')

evidence=[]
for shell,cli in [('powershell',False),('cmd',False),('powershell',True)]:
    api,front=port(),port()
    write(0)
    ps=['powershell','-NoProfile','-NonInteractive','-ExecutionPolicy','Bypass','-File',str(root/'scripts/fixtures/legacy-uvicorn-stack.ps1'),'-Python',python,'-Fixture',str(fixture),'-Port',str(api)]
    if cli: ps.append('-Cli')
    args=ps if shell=='powershell' else 'cmd.exe /D /S /C "'+subprocess.list2cmdline(ps)+'"'
    job=k.CreateJobObjectW(None,None)
    assert job
    output=open(temp/f'{shell}-{cli}.out.log','wb')
    error=open(temp/f'{shell}-{cli}.err.log','wb')
    process=subprocess.Popen(args,env=environment | {'FRONT_PORT':str(front)},stdout=subprocess.PIPE,stderr=subprocess.PIPE,creationflags=subprocess.CREATE_NO_WINDOW|0x00000004)
    def drain(pipe, destination):
        for line in iter(pipe.readline,b''):
            destination.write(line); destination.flush()
    readers=[threading.Thread(target=drain,args=(process.stdout,output),daemon=True),threading.Thread(target=drain,args=(process.stderr,error),daemon=True)]
    for reader in readers: reader.start()
    assert k.AssignProcessToJobObject(job,int(process._handle))
    assert resume(int(process._handle)) == 0
    observations=[]
    try:
        for version in range(4):
            if version: time.sleep(1.1); write(version)
            deadline=time.monotonic()+30
            while time.monotonic()<deadline:
                assert process.poll() is None, 'Legacy stack exited'
                try:
                    with urllib.request.urlopen(f'http://127.0.0.1:{api}/',timeout=1) as response: data=json.load(response)
                    with urllib.request.urlopen(f'http://127.0.0.1:{front}/',timeout=1) as response: assert response.read()==b'ok'
                    if data['version']==version: break
                except OSError: pass
                time.sleep(.2)
            else: raise AssertionError('Reload or frontend failed')
            handle=k.OpenProcess(0x1000,False,data['pid'])
            inside=wintypes.BOOL()
            assert handle and k.IsProcessInJob(handle,job,ctypes.byref(inside)) and inside.value, 'Reload worker escaped Quick Dev Job'
            k.CloseHandle(handle)
            observations.append(data)
        assert len({item['pid'] for item in observations})==4
        error.flush()
        logs=(temp/f'{shell}-{cli}.err.log').read_text(encoding='utf-8',errors='replace')
        assert '控制台已隔离' in logs and 'Started server process' in logs
    finally:
        k.TerminateJobObject(job,1)
        process.wait(timeout=10)
        for reader in readers: reader.join(timeout=3)
        assert all(not reader.is_alive() for reader in readers), 'Pipe handles retained after Job stop'
        process.stdout.close(); process.stderr.close()
        output.close(); error.close(); k.CloseHandle(job)
    time.sleep(.5)
    for listener in [api,front]:
        with socket.socket() as sock: assert sock.connect_ex(('127.0.0.1',listener)) !=0,'Job stop leaked listener'
    evidence.append({'shell':shell,'uvicornCli':cli,'reloads':observations,'jobOwnership':True,'frontendSurvived':True,'logs':True,'portsReleased':True})
    print(f'PASS {shell} CLI={cli}: unchanged legacy stack, three reloads, real npm frontend, worker Job membership, Unicode logs, Job stop')
(temp/'evidence.json').write_text(json.dumps(evidence,indent=2),encoding='utf-8')
