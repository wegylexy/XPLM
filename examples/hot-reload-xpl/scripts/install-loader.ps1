$ErrorActionPreference = 'Stop'
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = Split-Path -Parent (Split-Path -Parent (Split-Path -Parent $scriptDir))

$xpRoot = & (Join-Path $scriptDir 'find-xplane-root.ps1')
$dest = Join-Path $xpRoot 'Resources\plugins\hot-reload-xpl\64'
$destFile = Join-Path $dest 'win.xpl'

$xplaneRunning = [bool](Get-Process -Name 'X-Plane*' -ErrorAction SilentlyContinue)
if ($xplaneRunning -and (Test-Path $destFile)) {
    # X-Plane already has this DLL open/locked -- Copy-Item -Force would fail
    # outright, and even if it didn't, sim/operation/reload_plugins can't make
    # an already-running X-Plane pick up a changed file for a plugin it
    # already loaded at boot without a restart. Nothing useful to do here.
    Write-Host 'X-Plane is already running with the loader loaded; leaving it as-is (restart X-Plane to apply loader changes).'
    exit 0
}

Push-Location $repoRoot
try {
    cargo build -p hot-reload-xpl
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
} finally {
    Pop-Location
}

New-Item -ItemType Directory -Force -Path $dest | Out-Null
Copy-Item -Force (Join-Path $repoRoot 'target\debug\hot_reload_xpl.dll') $destFile
Write-Host "Installed loader to $destFile"

if ($xplaneRunning) {
    # destFile didn't exist yet above, so it wasn't locked -- but X-Plane was
    # already running before this install, so it never scanned this plugin
    # folder at boot and won't notice it exists without a restart.
    Write-Host 'X-Plane is already running: restart it to pick up this newly-installed loader -- there is no way to make a running X-Plane notice a new plugin without a restart.'
}
