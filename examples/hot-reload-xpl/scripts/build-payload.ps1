$ErrorActionPreference = 'Stop'
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = Split-Path -Parent (Split-Path -Parent (Split-Path -Parent $scriptDir))

Push-Location $repoRoot
try {
    # Plain `cargo build`, not a manual `-C extra-filename` — passing that
    # through `cargo rustc -- ...` bypasses Cargo's own artifact bookkeeping,
    # so `--message-format=json` reports an *empty* filenames array for a
    # target it doesn't recognize the (self-chosen) output name of. Get the
    # normal, reliably-reported build output instead, then make it unique
    # ourselves by copying it to a freshly-named staging file below.
    $messages = cargo build -p hot-reload-dll --message-format=json | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
} finally {
    Pop-Location
}

# target.name is the underscored crate name, not the hyphenated package name.
$artifact = $messages | Where-Object { $_.reason -eq 'compiler-artifact' -and $_.target.name -eq 'hot_reload_dll' } | Select-Object -Last 1
$builtDll = $artifact.filenames | Where-Object { $_ -like '*.dll' } | Select-Object -First 1
if (-not $builtDll) { throw 'Could not determine the built payload DLL path from cargo output.' }

$id = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
$stagingDir = Join-Path $repoRoot 'target\hot-reload-staging'
New-Item -ItemType Directory -Force -Path $stagingDir | Out-Null
$stagedDll = Join-Path $stagingDir "hot_reload_dll-$id.dll"
Copy-Item -Force $builtDll $stagedDll

$watchDir = Join-Path $env:LocalAppData 'xplm-hotreload'
New-Item -ItemType Directory -Force -Path $watchDir | Out-Null
$watchFile = Join-Path $watchDir 'hot-reload-example.json'
$tmpFile = "$watchFile.tmp"
@{ payload_path = $stagedDll; build_id = "$id" } | ConvertTo-Json | Set-Content -Path $tmpFile -Encoding utf8
Move-Item -Force $tmpFile $watchFile
Write-Host "Published payload build $id -> $stagedDll"
