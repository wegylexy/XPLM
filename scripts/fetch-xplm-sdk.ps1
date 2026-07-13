<#
.SYNOPSIS
Downloads the X-Plane SDK zip and places it in both vendored locations this
workspace's build scripts expect: xplm-sys/vendor/xplm-sdk (full SDK) and
xpmp2-sys/vendor/xplm-sdk/CHeaders/XPLM (headers-only subset).

Both destinations are gitignored — this script is the replacement for a
manual download-and-unzip step, run once per machine (or whenever the SDK
version changes).
#>

$ErrorActionPreference = 'Stop'

$sdkUrl = 'https://developer.x-plane.com/wp-content/plugins/code-sample-generation/sdk_zip_files/XPSDK430.zip'
$repoRoot = Split-Path -Parent $PSScriptRoot
$tmpZip = Join-Path ([System.IO.Path]::GetTempPath()) 'xplm-sdk.zip'
$tmpExtract = Join-Path ([System.IO.Path]::GetTempPath()) "xplm-sdk-extract-$([guid]::NewGuid())"

Write-Host "Downloading $sdkUrl..."
Invoke-WebRequest -Uri $sdkUrl -OutFile $tmpZip

Write-Host "Extracting..."
Expand-Archive -Path $tmpZip -DestinationPath $tmpExtract -Force

# The zip's top-level folder is already named "SDK" (all-caps) when
# extracted — no rename needed, unlike what this repo's README used to claim.
$extractedSdk = Join-Path $tmpExtract 'SDK'
if (-not (Test-Path $extractedSdk)) {
    throw "Expected an 'SDK' folder inside the extracted zip at $tmpExtract, found: $(Get-ChildItem $tmpExtract | Select-Object -ExpandProperty Name)"
}

$xplmSysDest = Join-Path $repoRoot 'xplm-sys\vendor\xplm-sdk'
$xpmp2SysDest = Join-Path $repoRoot 'xpmp2-sys\vendor\xplm-sdk\CHeaders\XPLM'

Write-Host "Installing full SDK to $xplmSysDest..."
if (Test-Path $xplmSysDest) { Remove-Item -Recurse -Force $xplmSysDest }
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $xplmSysDest) | Out-Null
Move-Item -Force $extractedSdk $xplmSysDest

Write-Host "Installing XPLM headers subset to $xpmp2SysDest..."
if (Test-Path $xpmp2SysDest) { Remove-Item -Recurse -Force $xpmp2SysDest }
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $xpmp2SysDest) | Out-Null
Copy-Item -Recurse -Force (Join-Path $xplmSysDest 'CHeaders\XPLM') $xpmp2SysDest

Remove-Item -Force $tmpZip
Remove-Item -Recurse -Force $tmpExtract -ErrorAction SilentlyContinue

Write-Host 'Done. Run `git submodule update --init xpmp2-sys/vendor/XPMP2` if you also need xpmp2/xpmp2-sys.'
