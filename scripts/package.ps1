$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $projectRoot
$destination = Join-Path $projectRoot 'dist'
New-Item -ItemType Directory -Path $destination -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $projectRoot 'target\release\recur-lab.exe') -Destination (Join-Path $destination 'recur-lab.exe')
foreach ($name in @('README.md', 'LICENSE', 'VALIDATION.md')) {
    $source = Join-Path $projectRoot $name
    if (Test-Path -LiteralPath $source) { Copy-Item -LiteralPath $source -Destination (Join-Path $destination $name) }
}
$metadataText = cargo metadata --locked --offline --format-version 1 --filter-platform x86_64-pc-windows-msvc
if ($LASTEXITCODE -ne 0) { throw 'cargo metadata failed' }
$metadata = $metadataText | ConvertFrom-Json
$active = @{}
foreach ($node in $metadata.resolve.nodes) { $active[$node.id] = $true }
$notice = [Text.StringBuilder]::new()
[void]$notice.AppendLine('RECUR — third-party licenses (Windows build)')
foreach ($package in $metadata.packages) {
    if (-not $active.ContainsKey($package.id) -or $package.name -eq 'recur-lab') { continue }
    [void]$notice.AppendLine('')
    [void]$notice.AppendLine(('=' * 72))
    [void]$notice.AppendLine("$($package.name) $($package.version) — $($package.license)")
    [void]$notice.AppendLine($package.repository)
    $directory = Split-Path -Parent $package.manifest_path
    $licenseFiles = @(Get-ChildItem -LiteralPath $directory -File | Where-Object { $_.Name -match '^(LICENSE|LICENCE|COPYING|NOTICE)' })
    foreach ($file in $licenseFiles) {
        [void]$notice.AppendLine("--- $($file.Name) ---")
        [void]$notice.AppendLine([IO.File]::ReadAllText($file.FullName))
    }
}
[IO.File]::WriteAllText((Join-Path $destination 'THIRD-PARTY-NOTICES.txt'), $notice.ToString())
Get-ChildItem -LiteralPath $destination -File | Select-Object Name, Length
