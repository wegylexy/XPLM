$ErrorActionPreference = 'Stop'
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path

if (Get-Process -Name 'X-Plane*' -ErrorAction SilentlyContinue) {
    Write-Host 'X-Plane already running.'
    exit 0
}

$xpRoot = & (Join-Path $scriptDir 'find-xplane-root.ps1')
$exe = Get-ChildItem -Path $xpRoot -Filter 'X-Plane*.exe' -File | Select-Object -First 1
if (-not $exe) { throw "Could not find an X-Plane executable under $xpRoot" }

Write-Host "Starting $($exe.FullName)..."
Start-Process -FilePath $exe.FullName

$deadline = (Get-Date).AddSeconds(120)
while (-not (Get-Process -Name 'X-Plane*' -ErrorAction SilentlyContinue)) {
    if ((Get-Date) -gt $deadline) { throw 'Timed out waiting for X-Plane to start.' }
    Start-Sleep -Seconds 1
}
Write-Host 'X-Plane process detected; it may still be mid-boot (first cold launch can take a while before flight loops start ticking).'
