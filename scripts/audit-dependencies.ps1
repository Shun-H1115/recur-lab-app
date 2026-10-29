$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$lockText = [IO.File]::ReadAllText((Join-Path $projectRoot 'Cargo.lock'))
$packages = [regex]::Matches($lockText, '(?m)^name = "([^"]+)"\r?\nversion = "([^"]+)"\r?\nsource = "registry\+')
$queries = @($packages | ForEach-Object {
    @{ package = @{ name = $_.Groups[1].Value; ecosystem = 'crates.io' }; version = $_.Groups[2].Value }
})
# Only public dependency names and versions are sent. No project source or local paths.
$body = @{ queries = $queries } | ConvertTo-Json -Depth 6 -Compress
$response = Invoke-RestMethod -Method Post -Uri 'https://api.osv.dev/v1/querybatch' -ContentType 'application/json' -Body $body -TimeoutSec 60
$findings = @()
for ($i = 0; $i -lt $queries.Count; $i++) {
    if ($response.results[$i].vulns) {
        $findings += @{ package = $queries[$i].package.name; version = $queries[$i].version; advisories = @($response.results[$i].vulns.id) }
    }
}
$report = @{ checked = $queries.Count; source = 'OSV'; timestamp = [DateTime]::UtcNow.ToString('o'); findings = $findings }
$report | ConvertTo-Json -Depth 8
