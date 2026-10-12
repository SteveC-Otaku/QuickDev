param([string]$Python,[string]$Fixture,[int]$Port,[switch]$Cli)
$ErrorActionPreference = 'Stop'
$arguments = @('-m','uvicorn','app:app','--reload','--reload-dir',('"'+$Fixture+'"'),'--host','127.0.0.1','--port',"$Port",'--reload-delay','0.2')
if ($Cli) {
  $scriptsDirectoryJson = & $Python -c "import sysconfig,json; print(json.dumps(sysconfig.get_path('scripts'), ensure_ascii=True))"
  if ($LASTEXITCODE -ne 0) { throw 'Could not locate the Python scripts directory' }
  $Python = Join-Path ($scriptsDirectoryJson | ConvertFrom-Json) 'uvicorn.exe'
  $arguments = $arguments[2..($arguments.Count-1)]
}
# Deliberately reproduce the legacy shared-console project script.
$backend = Start-Process -FilePath $Python -ArgumentList $arguments -WorkingDirectory $Fixture -NoNewWindow -PassThru
$frontend = Start-Process -FilePath 'cmd.exe' -ArgumentList @('/D','/C','npm run dev') -WorkingDirectory $Fixture -NoNewWindow -PassThru
Write-Output "FRONTEND_PID=$($frontend.Id)"
try {
  while (-not $backend.HasExited -and -not $frontend.HasExited) { Start-Sleep -Milliseconds 100 }
  throw 'A stack process unexpectedly exited'
} finally {
  foreach ($child in @($backend,$frontend)) { if (-not $child.HasExited) { taskkill /F /T /PID $child.Id 2>$null | Out-Null } }
}
