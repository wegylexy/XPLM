<#
.SYNOPSIS
Prints the first X-Plane install root that actually looks like one (has a
Resources folder), scanning every line of x-plane_install_12.txt/_11.txt —
per X-Plane's own install-location documentation, a line-1-only read isn't
safe: the file can list multiple locations, including stale/moved ones.
#>
$ErrorActionPreference = 'Stop'

$candidates = @(
    "$env:LocalAppData\x-plane_install_12.txt",
    "$env:LocalAppData\x-plane_install_11.txt"
)

foreach ($file in $candidates) {
    if (-not (Test-Path $file)) { continue }
    foreach ($line in Get-Content $file) {
        $root = $line.Trim()
        if ($root -and (Test-Path (Join-Path $root 'Resources'))) {
            Write-Output $root
            exit 0
        }
    }
}

throw "No valid X-Plane install found (checked: $($candidates -join ', '))"
