<#
.SYNOPSIS
Starts the local Web guide without the in-game MOD.

.DESCRIPTION
Loads .local\set-provider.ps1 when present (session environment variables
override it), validates the provider configuration, and runs guide-server in
the foreground on a loopback port. Press Ctrl+C to stop the server.
#>
[CmdletBinding()]
param(
    [ValidateRange(1, 65535)][UInt16]$Port = 8070
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$RepoRoot = Split-Path -Parent $PSScriptRoot
$ServerExecutable = Join-Path $RepoRoot 'target\release\guide-server.exe'
if (-not (Test-Path -LiteralPath $ServerExecutable -PathType Leaf)) {
    throw "guide-server executable not found: $ServerExecutable; run cargo build --release first"
}

$ProviderConfigPath = Join-Path $RepoRoot '.local\set-provider.ps1'
$ProviderVariableNames = @(
    'GUIDE_PROVIDER',
    'GUIDE_MODEL',
    'GUIDE_BASE_URL',
    'GUIDE_DISABLE_REASONING',
    'OPENAI_API_KEY'
)
$SessionOverrides = @{}
foreach ($ProviderVariableName in $ProviderVariableNames) {
    $SessionValue = [Environment]::GetEnvironmentVariable($ProviderVariableName, 'Process')
    if ($null -ne $SessionValue) {
        $SessionOverrides[$ProviderVariableName] = $SessionValue
    }
}
if (Test-Path -LiteralPath $ProviderConfigPath -PathType Leaf) {
    Write-Host "Loading provider configuration: $ProviderConfigPath"
    . $ProviderConfigPath
}
foreach ($SessionOverrideName in @($SessionOverrides.Keys)) {
    Set-Item -Path ('Env:' + $SessionOverrideName) -Value $SessionOverrides[$SessionOverrideName]
}

if (-not $env:GUIDE_PROVIDER -or -not $env:GUIDE_MODEL) {
    throw 'GUIDE_PROVIDER and GUIDE_MODEL are not configured; run .\scripts\Set-GuideProvider.ps1 once, or set them in this session'
}
if ($env:GUIDE_PROVIDER -eq 'openai' -and -not $env:OPENAI_API_KEY) {
    throw 'OPENAI_API_KEY is required when GUIDE_PROVIDER=openai; run .\scripts\Set-GuideProvider.ps1 once, or set it in this session'
}

$SupportManifestPath = Join-Path $RepoRoot 'adapter\read-only\ue4ss\support-manifest.json'
if (-not (Test-Path -LiteralPath $SupportManifestPath -PathType Leaf)) {
    throw "support manifest not found: $SupportManifestPath"
}
$SupportManifest = Get-Content -LiteralPath $SupportManifestPath -Raw | ConvertFrom-Json
$GameVersion = [string]$SupportManifest.game_version
if (-not $GameVersion) {
    throw "support manifest is missing game_version"
}

Write-Host "Web guide: http://127.0.0.1:$Port/ (press Ctrl+C to stop)"
& $ServerExecutable `
    --data (Join-Path $RepoRoot 'data\reviewed') `
    --game-version $GameVersion `
    --port ([string]$Port)
exit $LASTEXITCODE
