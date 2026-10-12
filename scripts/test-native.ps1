$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
Push-Location (Join-Path $repo 'src-tauri')
try {
    cargo test --offline --lib --no-run
    if ($LASTEXITCODE -ne 0) { throw 'Native test compilation failed' }
    $binary = Get-ChildItem 'target/debug/deps/quick_dev_lib-*.exe' | Sort-Object LastWriteTime -Descending | Select-Object -First 1
    if (!$binary) { throw 'Native test executable not found' }
    $version = (Get-Content (Join-Path $repo 'package.json') -Raw | ConvertFrom-Json).version
    $destination = Join-Path $repo "_tasks/$(Get-Date -Format 'yyyy-MM-dd')-native-tests-v$version"
    New-Item -ItemType Directory -Path $destination -Force | Out-Null
    $copy = Join-Path $destination 'native-tests.exe'
    Copy-Item -LiteralPath $binary.FullName -Destination $copy -Force
    $mt = Get-ChildItem "${env:ProgramFiles(x86)}/Windows Kits/10/bin/*/x64/mt.exe" | Sort-Object FullName -Descending | Select-Object -First 1
    if (!$mt) { throw 'Windows SDK mt.exe is required to embed Common-Controls v6 in the test copy' }
    & $mt.FullName -nologo -manifest (Join-Path $repo 'docs/maintenance/2026-10-11-fixes/tools/native-tests.manifest') "-outputresource:$copy;#1"
    if ($LASTEXITCODE -ne 0) { throw 'Test-copy manifest embedding failed' }
    & $copy --nocapture --test-threads=1
    if ($LASTEXITCODE -ne 0) { throw 'Native test failure' }
} finally { Pop-Location }
