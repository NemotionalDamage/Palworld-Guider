<#
.SYNOPSIS
Validates the installed adapter, binds a session gateway token, starts the
loopback guide-server, and launches Palworld through Steam in the same session.

.DESCRIPTION
Runs read-only preflight, verifies every installed adapter file against the
installed PalworldGuider.sha256 manifest and reviewed package-file contract,
generates a random bearer token in memory, binds it to this PowerShell session
(never to a file, the registry, or the command line), passes provider
environment values only to the child process, and launches guide-server on
loopback ports. Palworld must not already be running: the script closes a
running Steam client and relaunches Steam from this session so the game process
inherits the same token and gateway port. The token is never placed on the
command line, printed, or written to a file.
#>
[CmdletBinding()]
param(
    [ValidateRange(1, 65535)]
    [UInt16]$Port = 8070,
    [ValidateRange(1, 65535)]
    [UInt16]$AdapterPort = 8071,
    [string]$ServerExecutable,
    [string]$SteamExecutable
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$ResolvedSteamExecutable = $null

if ($Port -eq $AdapterPort) {
    throw "Port and AdapterPort must be different so the Web and adapter listeners remain separate"
}

$RepoRoot = Split-Path -Parent $PSScriptRoot
if (-not $ServerExecutable) {
    $ServerExecutable = Join-Path $RepoRoot 'target\release\guide-server.exe'
}
if (-not (Test-Path -LiteralPath $ServerExecutable -PathType Leaf)) {
    throw "guide-server executable not found: $ServerExecutable; run cargo build --release first"
}
$ResolvedServerExecutable = (Resolve-Path -LiteralPath $ServerExecutable).Path

if ($SteamExecutable) {
    if (-not (Test-Path -LiteralPath $SteamExecutable -PathType Leaf)) {
        throw "SteamExecutable not found: $SteamExecutable"
    }
    $ResolvedSteamExecutable = (Resolve-Path -LiteralPath $SteamExecutable).Path
}

$PreflightScript = Join-Path $PSScriptRoot 'Test-InGameGuide.ps1'
$PreflightOutput = @(& powershell.exe -NoProfile -ExecutionPolicy Bypass -File $PreflightScript 2>&1)
foreach ($PreflightLine in $PreflightOutput) {
    Write-Host $PreflightLine
}
if ($LASTEXITCODE -ne 0) {
    throw "preflight failed with exit code $LASTEXITCODE"
}

function Get-PreflightValue {
    param(
        [Parameter(Mandatory = $true)][object[]]$Output,
        [Parameter(Mandatory = $true)][string]$Prefix
    )

    $MatchingLine = $Output |
        ForEach-Object { [string]$_ } |
        Where-Object { $_.StartsWith($Prefix, [System.StringComparison]::OrdinalIgnoreCase) } |
        Select-Object -First 1
    if (-not $MatchingLine) {
        throw "preflight output is missing required value: $Prefix"
    }
    return $MatchingLine.Substring($Prefix.Length)
}

$ResolvedModsDirectory = Get-PreflightValue -Output $PreflightOutput -Prefix 'Mods directory: '
if (-not $ResolvedSteamExecutable) {
    $ResolvedSteamExecutable = Get-PreflightValue -Output $PreflightOutput -Prefix 'Steam executable: '
}
if (-not (Test-Path -LiteralPath $ResolvedSteamExecutable -PathType Leaf)) {
    throw "Steam executable not found: $ResolvedSteamExecutable"
}
$InstalledPackageDirectory = Join-Path $ResolvedModsDirectory 'PalworldGuider'
$InstalledManifestPath = Join-Path $InstalledPackageDirectory 'PalworldGuider.sha256'
$SupportManifestPath = Join-Path $RepoRoot 'adapter\read-only\ue4ss\support-manifest.json'
if (-not (Test-Path -LiteralPath $InstalledPackageDirectory -PathType Container)) {
    throw "installed package not found: $InstalledPackageDirectory"
}
if (-not (Test-Path -LiteralPath $InstalledManifestPath -PathType Leaf)) {
    throw "installed package manifest not found: $InstalledManifestPath"
}
if (-not (Test-Path -LiteralPath $SupportManifestPath -PathType Leaf)) {
    throw "support manifest not found: $SupportManifestPath"
}

$SupportManifest = Get-Content -LiteralPath $SupportManifestPath -Raw | ConvertFrom-Json
$GameVersion = [string]$SupportManifest.game_version
if (-not $GameVersion) {
    throw "support manifest is missing game_version"
}
$ExpectedPackageFiles = @(
    $SupportManifest.package_files |
        Where-Object { $_ -is [string] } |
        ForEach-Object { $_.Replace('/', '\') }
)
$ExpectedPackageFileCount = $ExpectedPackageFiles.Count
$InstalledHashes = @{}
foreach ($ManifestLine in @(Get-Content -LiteralPath $InstalledManifestPath)) {
    $Parts = $ManifestLine -split '\s+', 2
    if ($Parts.Count -eq 2 -and $Parts[0] -match '^[0-9A-Fa-f]{64}$') {
        $InstalledHashes[$Parts[1].Replace('/', '\')] = $Parts[0].ToLowerInvariant()
    }
}
if ($InstalledHashes.Count -ne $ExpectedPackageFileCount) {
    throw "installed package manifest file count does not match the reviewed package contract"
}
$InstalledPaths = @($InstalledHashes.Keys) | Sort-Object
$ExpectedPaths = @($ExpectedPackageFiles) | Sort-Object
if (Compare-Object -ReferenceObject $ExpectedPaths -DifferenceObject $InstalledPaths) {
    throw "installed package manifest paths do not match the reviewed package contract"
}
foreach ($RelativePath in $InstalledHashes.Keys) {
    if ([System.IO.Path]::IsPathRooted($RelativePath) -or $RelativePath -split '[\\/]' -contains '..') {
        throw "installed package manifest contains an unsafe path: $RelativePath"
    }
    $InstalledFile = Join-Path $InstalledPackageDirectory $RelativePath
    if (-not (Test-Path -LiteralPath $InstalledFile -PathType Leaf)) {
        throw "installed package file missing: $InstalledFile"
    }
    $ActualHash = (Get-FileHash -LiteralPath $InstalledFile -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($ActualHash -ne $InstalledHashes[$RelativePath]) {
        throw "installed package hash mismatch: $InstalledFile"
    }
}

$RunningGame = Get-Process -Name 'Palworld*' -ErrorAction SilentlyContinue | Select-Object -First 1
if ($RunningGame) {
    throw "Palworld is already running (process: $($RunningGame.ProcessName)); Start-InGameGuide.ps1 launches Palworld through Steam so the game inherits the session gateway token. Exit Palworld and re-run this script."
}

$RunningSteam = @(Get-Process -Name 'steam' -ErrorAction SilentlyContinue)
if ($RunningSteam.Count -gt 0) {
    Write-Host 'Closing the running Steam client so Palworld inherits this session gateway token...'
    foreach ($SteamProcess in $RunningSteam) {
        $null = $SteamProcess.CloseMainWindow()
    }
    $SteamCloseDeadline = (Get-Date).AddSeconds(30)
    do {
        Start-Sleep -Milliseconds 500
        $RunningSteam = @(Get-Process -Name 'steam' -ErrorAction SilentlyContinue)
    } while ($RunningSteam.Count -gt 0 -and (Get-Date) -lt $SteamCloseDeadline)
    if ($RunningSteam.Count -gt 0) {
        throw 'Steam did not close within 30 seconds; close Steam manually and re-run Start-InGameGuide.ps1'
    }
}

$GatewayToken = [System.Guid]::NewGuid().ToString('N') * 2
$env:PALWORLD_GUIDER_GATEWAY_TOKEN = $GatewayToken
$env:PALWORLD_GUIDER_GATEWAY_PORT = [string]$AdapterPort
$StartInfo = [System.Diagnostics.ProcessStartInfo]::new()
$StartInfo.FileName = $ResolvedServerExecutable
$StartInfo.WorkingDirectory = $RepoRoot
$StartInfo.UseShellExecute = $false

foreach ($ProviderVariable in @(
    'GUIDE_PROVIDER',
    'GUIDE_MODEL',
    'GUIDE_BASE_URL',
    'GUIDE_DISABLE_REASONING',
    'OPENAI_API_KEY'
)) {
    $ProviderValue = [Environment]::GetEnvironmentVariable($ProviderVariable, 'Process')
    if ($null -ne $ProviderValue) {
        $StartInfo.EnvironmentVariables[$ProviderVariable] = $ProviderValue
    }
}
$StartInfo.EnvironmentVariables['PALWORLD_GUIDER_GATEWAY_TOKEN'] = $GatewayToken

function ConvertTo-CommandLineArgument {
    param([Parameter(Mandatory = $true)][string]$Value)

    if ($Value -notmatch '[\s"]') {
        return $Value
    }
    return '"' + $Value.Replace('"', '\"') + '"'
}

$ServerArguments = @(
    '--data',
    (Join-Path $RepoRoot 'data\reviewed'),
    '--game-version',
    $GameVersion,
    '--port',
    [string]$Port,
    '--adapter-port',
    [string]$AdapterPort,
    '--adapter-token-env',
    'PALWORLD_GUIDER_GATEWAY_TOKEN'
) | ForEach-Object { ConvertTo-CommandLineArgument -Value $_ }
$StartInfo.Arguments = $ServerArguments -join ' '

$ServerProcess = [System.Diagnostics.Process]::Start($StartInfo)
Start-Sleep -Seconds 1
if ($ServerProcess.HasExited) {
    throw "guide-server exited during startup with code $($ServerProcess.ExitCode); check provider environment variables and restart Steam if this script closed it"
}

Write-Host "Installed package manifest validated: $InstalledManifestPath"
Write-Host "guide-server started with PID $($ServerProcess.Id)"
Write-Host "Web interface: http://127.0.0.1:$Port/"
Write-Host "Adapter gateway: 127.0.0.1:$AdapterPort"
Write-Host "Launching Palworld through Steam (app 1623730) so the game inherits this session's gateway token and port..."
$null = Start-Process -FilePath $ResolvedSteamExecutable -ArgumentList @('-applaunch', '1623730')
Write-Host 'If Steam asks for sign-in, sign in and Palworld starts automatically.'
