$ErrorActionPreference = 'Stop'
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = Split-Path -Parent (Split-Path -Parent (Split-Path -Parent $scriptDir))

Push-Location $repoRoot
try {
    cargo build -p hot-reload-xpl
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
} finally {
    Pop-Location
}

$xpRoot = & (Join-Path $scriptDir 'find-xplane-root.ps1')
$dest = Join-Path $xpRoot 'Resources\plugins\hot-reload-xpl\64'
New-Item -ItemType Directory -Force -Path $dest | Out-Null
Copy-Item -Force (Join-Path $repoRoot 'target\debug\hot_reload_xpl.dll') (Join-Path $dest 'win.xpl')
Write-Host "Installed loader to $dest\win.xpl"
