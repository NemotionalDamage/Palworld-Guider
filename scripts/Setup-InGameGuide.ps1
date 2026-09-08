<#
.SYNOPSIS
Composes preflight, verified backup, build, hash verification, and install.

.DESCRIPTION
Runs the read-only preflight first, refuses an existing Mods/PalworldGuider
installation, creates a fresh hash-verified save backup under the repository's
.local/backups tree, stages and verifies the package under .local/build, and
only then invokes the explicit game installer. With -WhatIf, setup performs
preflight and reports the planned stages but does not back up, build, or touch
the game directory.
#>
[CmdletBinding(SupportsShouldProcess = $true)]
param(
    [string]$GameRoot,
    [string]$Ue4ssDll,
    [string]$SaveDirectory,
    [string]$OutputDirectory
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$RepoRoot = Split-Path -Parent $PSScriptRoot
$BuildRoot = [System.IO.Path]::GetFullPath((Join-Path $RepoRoot '.local\build'))
$BackupRoot = [System.IO.Path]::GetFullPath((Join-Path $RepoRoot '.local\backups\palworld'))
$PreflightScript = Join-Path $PSScriptRoot 'Test-InGameGuide.ps1'
$BackupScript = Join-Path $PSScriptRoot 'Backup-PalworldSave.ps1'
$BuildScript = Join-Path $PSScriptRoot 'Build-Ue4ssAdapter.ps1'
$InstallScript = Join-Path $PSScriptRoot 'Install-Ue4ssAdapter.ps1'

if ($OutputDirectory) {
    if (-not [System.IO.Path]::IsPathRooted($OutputDirectory)) {
        throw "OutputDirectory must be an absolute path under $BuildRoot"
    }
    $ResolvedOutputDirectory = [System.IO.Path]::GetFullPath($OutputDirectory)
} else {
    $ResolvedOutputDirectory = Join-Path $BuildRoot 'ue4ss-adapter'
}
$AllowedBuildPrefix = $BuildRoot + [System.IO.Path]::DirectorySeparatorChar
if (-not $ResolvedOutputDirectory.StartsWith($AllowedBuildPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "refusing setup output outside .local\build: $ResolvedOutputDirectory is not under $BuildRoot"
}

$PreflightArguments = @(
    '-NoProfile',
    '-ExecutionPolicy',
    'Bypass',
    '-File',
    $PreflightScript
)
if ($GameRoot) {
    $PreflightArguments += @('-GameRoot', $GameRoot)
}
if ($Ue4ssDll) {
    $PreflightArguments += @('-Ue4ssDll', $Ue4ssDll)
}
if ($SaveDirectory) {
    $PreflightArguments += @('-SaveDirectory', $SaveDirectory)
}

$PreflightOutput = @(& powershell.exe @PreflightArguments 2>&1)
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

$ResolvedGameRoot = Get-PreflightValue -Output $PreflightOutput -Prefix 'Game root: '
$ResolvedUe4ssDll = Get-PreflightValue -Output $PreflightOutput -Prefix 'UE4SS.dll: '
$ResolvedModsDirectory = Get-PreflightValue -Output $PreflightOutput -Prefix 'Mods directory: '
$ResolvedSaveDirectory = Get-PreflightValue -Output $PreflightOutput -Prefix 'Newest save directory: '
$TargetPackageDirectory = Join-Path $ResolvedModsDirectory 'PalworldGuider'
if (Test-Path -LiteralPath $TargetPackageDirectory) {
    throw "refusing setup: Mods\PalworldGuider already exists at $TargetPackageDirectory; run Uninstall-Ue4ssAdapter.ps1 first"
}

if ($WhatIfPreference) {
    Write-Host "What if: preflight passed for the discovered Palworld installation"
    Write-Host "What if: backing up $ResolvedSaveDirectory to a new directory under $BackupRoot"
    Write-Host "What if: building and staging the package in $ResolvedOutputDirectory"
    Write-Host "What if: verifying the staged package SHA256 manifest"
    Write-Host "What if: installing $ResolvedOutputDirectory\PalworldGuider into $TargetPackageDirectory"
    Write-Host "-WhatIf never invokes the real game installer; run setup again without -WhatIf to install"
    return
}

$WorldIdentity = Split-Path -Leaf $ResolvedSaveDirectory
$Timestamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$BackupDestination = Join-Path $BackupRoot "$Timestamp-$WorldIdentity"
$AllowedBackupPrefix = $BackupRoot + [System.IO.Path]::DirectorySeparatorChar
if (-not $BackupDestination.StartsWith($AllowedBackupPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "internal setup error: backup destination escaped $BackupRoot"
}

& $BackupScript -SourceDirectory $ResolvedSaveDirectory -DestinationDirectory $BackupDestination

$OutputManifestPath = Join-Path $ResolvedOutputDirectory 'PalworldGuider.sha256'
$OutputPackageDirectory = Join-Path $ResolvedOutputDirectory 'PalworldGuider'
& $BuildScript -Ue4ssDll $ResolvedUe4ssDll -OutputDirectory $ResolvedOutputDirectory

function Verify-PackageManifest {
    param(
        [Parameter(Mandatory = $true)][string]$PackageDirectory,
        [Parameter(Mandatory = $true)][string]$ManifestPath
    )

    if (-not (Test-Path -LiteralPath $ManifestPath -PathType Leaf)) {
        throw "package hash manifest not found: $ManifestPath"
    }
    $ExpectedHashes = @{}
    foreach ($ManifestLine in @(Get-Content -LiteralPath $ManifestPath)) {
        $Parts = $ManifestLine -split '\s+', 2
        if ($Parts.Count -eq 2 -and $Parts[0] -match '^[0-9A-Fa-f]{64}$') {
            $ExpectedHashes[$Parts[1]] = $Parts[0].ToLowerInvariant()
        }
    }
    if ($ExpectedHashes.Count -eq 0) {
        throw "package hash manifest contains no verifiable entries: $ManifestPath"
    }
    foreach ($RelativePath in $ExpectedHashes.Keys) {
        $PackageFile = Join-Path $PackageDirectory $RelativePath
        if (-not (Test-Path -LiteralPath $PackageFile -PathType Leaf)) {
            throw "package file missing: $PackageFile"
        }
        $ActualHash = (Get-FileHash -LiteralPath $PackageFile -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($ActualHash -ne $ExpectedHashes[$RelativePath]) {
            throw "package hash mismatch: $PackageFile"
        }
    }
}

Verify-PackageManifest -PackageDirectory $OutputPackageDirectory -ManifestPath $OutputManifestPath
& $InstallScript -PackageDirectory $OutputPackageDirectory -ModsDirectory $ResolvedModsDirectory -BackupDirectory $BackupDestination
