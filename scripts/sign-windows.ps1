param(
    [Parameter(Mandatory = $true, Position = 0)]
    [string]$FilePath
)

# 代码签名脚本：构建 MSI/EXE 时由 Tauri 调用
# 设置环境变量后执行打包即可签名；未配置时跳过（开发构建不受影响）
#
#   $env:QUICKDEV_SIGN_THUMBPRINT = "证书指纹"
#   $env:QUICKDEV_TIMESTAMP_URL   = "http://timestamp.digicert.com"  # 可选
#
# 获取指纹: Get-ChildItem Cert:\CurrentUser\My | Format-List Subject, Thumbprint

$thumbprint = $env:QUICKDEV_SIGN_THUMBPRINT
if ([string]::IsNullOrWhiteSpace($thumbprint)) {
    Write-Host "[sign] QUICKDEV_SIGN_THUMBPRINT 未设置，跳过签名: $FilePath"
    exit 0
}

$timestampUrl = $env:QUICKDEV_TIMESTAMP_URL
if ([string]::IsNullOrWhiteSpace($timestampUrl)) {
    $timestampUrl = "http://timestamp.digicert.com"
}

$signtool = "${env:ProgramFiles(x86)}\Windows Kits\10\bin\10.0.22621.0\x64\signtool.exe"
if (-not (Test-Path $signtool)) {
    $signtool = Get-ChildItem -Path "${env:ProgramFiles(x86)}\Windows Kits\10\bin" -Recurse -Filter "signtool.exe" -ErrorAction SilentlyContinue |
        Sort-Object FullName -Descending |
        Select-Object -First 1 -ExpandProperty FullName
}

if (-not $signtool -or -not (Test-Path $signtool)) {
    Write-Error "[sign] 未找到 signtool.exe，请安装 Windows SDK"
    exit 1
}

Write-Host "[sign] 正在签名: $FilePath"
& $signtool sign /fd SHA256 /tr $timestampUrl /td SHA256 /sha1 $thumbprint /v $FilePath
if ($LASTEXITCODE -ne 0) {
    Write-Error "[sign] 签名失败，退出码 $LASTEXITCODE"
    exit $LASTEXITCODE
}

Write-Host "[sign] 完成"
exit 0
