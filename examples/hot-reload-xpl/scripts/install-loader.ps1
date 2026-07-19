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
$destFile = Join-Path $dest 'win.xpl'
$isFreshInstall = -not (Test-Path $destFile)

New-Item -ItemType Directory -Force -Path $dest | Out-Null
Copy-Item -Force (Join-Path $repoRoot 'target\debug\hot_reload_xpl.dll') $destFile
Write-Host "Installed loader to $destFile"

# X-Plane only scans Resources/plugins at startup. If the loader wasn't
# installed before this run and X-Plane is already running, it won't notice
# the new plugin without a restart -- unless we tell it to rescan over UDP.
$xplaneRunning = [bool](Get-Process -Name 'X-Plane*' -ErrorAction SilentlyContinue)
if ($isFreshInstall -and $xplaneRunning) {
    Write-Host 'Fresh install with X-Plane already running; asking it to reload plugins over UDP...'
    Push-Location $repoRoot
    try {
        cargo run -q -p flybywireless-xplm-reloader --bin reload-plugins
        if ($LASTEXITCODE -ne 0) {
            Write-Host 'Reload request failed; restart X-Plane manually to pick up the new plugin.'
        }
    } finally {
        Pop-Location
    }
}
