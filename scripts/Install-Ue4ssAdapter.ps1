<#
.SYNOPSIS
Installs a built Palworld Guider UE4SS package into the game Mods directory
after verifying a fresh save backup.

.DESCRIPTION
Refuses to run while Palworld is running, requires a verified non-empty backup
produced by Backup-PalworldSave.ps1 (including per-file SHA256 records) before
any write to the game directory, and requires the package SHA256 manifest
produced by Build-Ue4ssAdapter.ps1. Installs only into the game Mods directory
under the exact PalworldGuider subfolder, using -LiteralPath, includes the
package manifest for startup validation, and updates mods.txt to enable only
the PalworldGuider line while preserving every other mod's line and folder. On
failure before completion, the added package and mods.txt changes are rolled
back.

.PARAMETER PackageDirectory
The built PalworldGuider package directory (from Build-Ue4ssAdapter.ps1).

.PARAMETER ModsDirectory
The game's UE4SS Mods directory (for example
<Your Palworld installation>\Pal\Binaries\Win64\Mods).

.PARAMETER BackupDirectory
A verified backup directory produced by Backup-PalworldSave.ps1.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$PackageDirectory,
    [Parameter(Mandatory = $true)]
    [string]$ModsDirectory,
    [Parameter(Mandatory = $true)]
    [string]$BackupDirectory
)

$ErrorActionPreference = 'Stop'

$RunningGame = Get-Process -Name 'Palworld*' -ErrorAction SilentlyContinue
if ($RunningGame) {
    throw "refusing to install while Palworld is running"
}

$BackupManifestPath = Join-Path $BackupDirectory 'backup-manifest.json'
if (-not (Test-Path -LiteralPath $BackupManifestPath)) {
    throw "refusing to install: no verified backup at $BackupDirectory (missing backup-manifest.json); run Backup-PalworldSave.ps1 first"
}
$BackupRecord = Get-Content -LiteralPath $BackupManifestPath -Raw | ConvertFrom-Json
if (-not $BackupRecord.completed_at -or -not $BackupRecord.file_count -or $BackupRecord.file_count -lt 1) {
    throw "refusing to install: no verified backup at $BackupDirectory (manifest is incomplete)"
}
$BackupFileRecords = @($BackupRecord.files)
if ($BackupFileRecords.Count -ne [int]$BackupRecord.file_count) {
    throw "refusing to install: no verified backup at $BackupDirectory (file records are incomplete)"
}

$BackupDataFiles = @(
    Get-ChildItem -LiteralPath $BackupDirectory -Recurse -File -Force |
        Where-Object { $_.Name -ne 'backup-manifest.json' }
)
if ($BackupDataFiles.Count -ne $BackupFileRecords.Count) {
    throw "refusing to install: no verified backup at $BackupDirectory (file count does not match the manifest)"
}

foreach ($BackupFileRecord in $BackupFileRecords) {
    foreach ($propertyName in @('path', 'sha256', 'length')) {
        if (-not $BackupFileRecord.PSObject.Properties[$propertyName]) {
            throw "refusing to install: no verified backup at $BackupDirectory (a file record is incomplete)"
        }
    }
    $RelativePath = [string]$BackupFileRecord.path
    if (
        [System.IO.Path]::IsPathRooted($RelativePath) -or
        $RelativePath -split '[\\/]' -contains '..' -or
        -not $RelativePath
    ) {
        throw "refusing to install: backup manifest contains an unsafe path: $RelativePath"
    }
    $BackupFile = Join-Path $BackupDirectory $RelativePath
    if (-not (Test-Path -LiteralPath $BackupFile -PathType Leaf)) {
        throw "refusing to install: backup file missing: $BackupFile"
    }
    if ((Get-Item -LiteralPath $BackupFile).Length -ne [long]$BackupFileRecord.length) {
        throw "refusing to install: backup file length mismatch: $BackupFile"
    }
    $BackupHash = (Get-FileHash -LiteralPath $BackupFile -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($BackupHash -ne [string]$BackupFileRecord.sha256) {
        throw "refusing to install: backup file hash mismatch: $BackupFile"
    }
}

$PackageManifestPath = Join-Path (Split-Path -Parent $PackageDirectory) 'PalworldGuider.sha256'
if (-not (Test-Path -LiteralPath $PackageManifestPath)) {
    throw "refusing to install: package hash manifest not found at $PackageManifestPath; run Build-Ue4ssAdapter.ps1 first"
}
$ExpectedHashes = @{}
foreach ($manifestLine in @(Get-Content -LiteralPath $PackageManifestPath)) {
    $parts = $manifestLine -split '\s+', 2
    if ($parts.Count -eq 2) {
        $ExpectedHashes[$parts[1]] = $parts[0].ToLowerInvariant()
    }
}
if ($ExpectedHashes.Count -eq 0) {
    throw "refusing to install: package manifest contains no verifiable entries: $PackageManifestPath"
}
foreach ($relativePath in $ExpectedHashes.Keys) {
    $stagedFile = Join-Path $PackageDirectory $relativePath
    if (-not (Test-Path -LiteralPath $stagedFile)) {
        throw "refusing to install: staged package file missing: $stagedFile"
    }
    $actualHash = (Get-FileHash -LiteralPath $stagedFile -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actualHash -ne $ExpectedHashes[$relativePath]) {
        throw "refusing to install: package hash mismatch for $stagedFile"
    }
}

$TargetDirectory = Join-Path $ModsDirectory 'PalworldGuider'
if (Test-Path -LiteralPath $TargetDirectory) {
    throw "refusing to install: Mods\PalworldGuider already exists at $TargetDirectory; run Uninstall-Ue4ssAdapter.ps1 first"
}
$ModsFile = Join-Path $ModsDirectory 'mods.txt'
$OriginalModsText = if (Test-Path -LiteralPath $ModsFile) { Get-Content -LiteralPath $ModsFile -Raw } else { $null }
$InstalledManifestPath = Join-Path $TargetDirectory 'PalworldGuider.sha256'

try {
    New-Item -ItemType Directory -Path $TargetDirectory -Force | Out-Null
    foreach ($packageChild in @(Get-ChildItem -LiteralPath $PackageDirectory -Force)) {
        Copy-Item -LiteralPath $packageChild.FullName -Destination $TargetDirectory -Recurse -Force
    }
    Copy-Item -LiteralPath $PackageManifestPath -Destination $InstalledManifestPath

    $ExistingLines = if (Test-Path -LiteralPath $ModsFile) { @(Get-Content -LiteralPath $ModsFile) } else { @() }
    $KeptLines = @($ExistingLines | Where-Object { $_ -notmatch '^\s*PalworldGuider\s*:' })
    $UpdatedLines = @($KeptLines) + @('PalworldGuider : 1')
    Set-Content -LiteralPath $ModsFile -Value $UpdatedLines -Encoding Ascii

    Write-Host "Installed PalworldGuider into Mods\PalworldGuider at $TargetDirectory and enabled it in $ModsFile"
}
catch {
    Write-Warning "Install failed; rolling back PalworldGuider additions"
    if (Test-Path -LiteralPath (Join-Path $ModsDirectory 'PalworldGuider')) {
        Remove-Item -LiteralPath (Join-Path $ModsDirectory 'PalworldGuider') -Recurse -Force
    }
    if ($null -eq $OriginalModsText) {
        if (Test-Path -LiteralPath $ModsFile) {
            Remove-Item -LiteralPath $ModsFile -Force
        }
    }
    elseif (Test-Path -LiteralPath $ModsFile) {
        Set-Content -LiteralPath $ModsFile -Value $OriginalModsText -Encoding Ascii
    }
    throw
}
