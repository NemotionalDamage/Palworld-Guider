<#
.SYNOPSIS
Validates the Palworld, UE4SS, save, and Guider support state.

.DESCRIPTION
Performs read-only discovery and validation. It discovers Steam libraries from
libraryfolders.vdf, reads Palworld's appmanifest_1623730.acf, finds the newest
Level.sav, verifies the reviewed build and pinned UE4SS DLL hash, and validates
the repository support manifest. It never changes files.
#>
[CmdletBinding()]
param(
    [string]$GameRoot,
    [string]$Ue4ssDll,
    [string]$SaveDirectory
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

function Get-VdfString {
    param(
        [Parameter(Mandatory = $true)][string]$Content,
        [Parameter(Mandatory = $true)][string]$Key
    )

    $match = [regex]::Match(
        $Content,
        '"' + [regex]::Escape($Key) + '"\s+"((?:\\.|[^"])*)"'
    )
    if (-not $match.Success) {
        return $null
    }

    return $match.Groups[1].Value.Replace('\\', '\')
}

function Get-SteamLibraryFoldersVdf {
    $steamRegistryKeys = @(
        'HKLM:\SOFTWARE\WOW6432Node\Valve\Steam',
        'HKLM:\SOFTWARE\Valve\Steam'
    )
    $candidates = @()

    foreach ($steamRegistryKey in $steamRegistryKeys) {
        if (-not (Test-Path -LiteralPath $steamRegistryKey)) {
            continue
        }

        $steamSettings = Get-ItemProperty -LiteralPath $steamRegistryKey
        $installPathProperty = $steamSettings.PSObject.Properties['InstallPath']
        if ($installPathProperty -and $installPathProperty.Value) {
            $steamInstallPath = [string]$installPathProperty.Value
            $candidates += Join-Path $steamInstallPath 'config\libraryfolders.vdf'
        }
    }

    if (${env:ProgramFiles(x86)}) {
        $candidates += Join-Path ${env:ProgramFiles(x86)} 'Steam\config\libraryfolders.vdf'
    }
    if ($env:ProgramFiles) {
        $candidates += Join-Path $env:ProgramFiles 'Steam\config\libraryfolders.vdf'
    }

    foreach ($candidate in $candidates) {
        if (Test-Path -LiteralPath $candidate -PathType Leaf) {
            return $candidate
        }
    }

    throw "Steam libraryfolders.vdf not found; install Steam. Build validation always requires appmanifest_1623730.acf, so -GameRoot cannot bypass Steam manifest discovery."
}

function Get-SteamLibraryRoots {
    param([Parameter(Mandatory = $true)][string]$LibraryFoldersVdf)

    $content = Get-Content -LiteralPath $LibraryFoldersVdf -Raw
    $pathMatches = [regex]::Matches(
        $content,
        '"path"\s+"((?:\\.|[^"])*)"'
    )
    $libraryRoots = foreach ($match in $pathMatches) {
        $match.Groups[1].Value.Replace('\\', '\')
    }

    if (-not $libraryRoots) {
        throw "no Steam library paths found in $LibraryFoldersVdf"
    }

    return $libraryRoots | Select-Object -Unique
}

function Get-SteamExecutablePath {
    $steamRegistryKeys = @(
        'HKLM:\SOFTWARE\WOW6432Node\Valve\Steam',
        'HKLM:\SOFTWARE\Valve\Steam'
    )
    $candidates = @()

    foreach ($steamRegistryKey in $steamRegistryKeys) {
        if (-not (Test-Path -LiteralPath $steamRegistryKey)) {
            continue
        }

        $steamSettings = Get-ItemProperty -LiteralPath $steamRegistryKey
        $installPathProperty = $steamSettings.PSObject.Properties['InstallPath']
        if ($installPathProperty -and $installPathProperty.Value) {
            $candidates += Join-Path ([string]$installPathProperty.Value) 'Steam.exe'
        }
    }

    if (${env:ProgramFiles(x86)}) {
        $candidates += Join-Path ${env:ProgramFiles(x86)} 'Steam\Steam.exe'
    }
    if ($env:ProgramFiles) {
        $candidates += Join-Path $env:ProgramFiles 'Steam\Steam.exe'
    }

    foreach ($candidate in $candidates) {
        if (Test-Path -LiteralPath $candidate -PathType Leaf) {
            return (Resolve-Path -LiteralPath $candidate).Path
        }
    }

    throw "Steam.exe not found; install Steam and verify the client path"
}

$LibraryFoldersVdf = Get-SteamLibraryFoldersVdf
$SteamLibraryRoots = Get-SteamLibraryRoots -LibraryFoldersVdf $LibraryFoldersVdf
$PalworldAppManifestPath = $null

foreach ($SteamLibraryRoot in $SteamLibraryRoots) {
    $candidate = Join-Path $SteamLibraryRoot 'steamapps\appmanifest_1623730.acf'
    if (Test-Path -LiteralPath $candidate -PathType Leaf) {
        $PalworldAppManifestPath = $candidate
        break
    }
}

if (-not $PalworldAppManifestPath) {
    throw "Palworld Steam manifest appmanifest_1623730.acf not found in libraries listed by $LibraryFoldersVdf"
}

$PalworldAppManifestContent = Get-Content -LiteralPath $PalworldAppManifestPath -Raw
$PalworldInstallDirectoryName = Get-VdfString -Content $PalworldAppManifestContent -Key 'installdir'
$PalworldBuildId = Get-VdfString -Content $PalworldAppManifestContent -Key 'buildid'
if (-not $PalworldInstallDirectoryName -or -not $PalworldBuildId) {
    throw "Palworld Steam manifest is missing installdir or buildid: $PalworldAppManifestPath"
}

$SteamLibraryContainingPalworld = $null
foreach ($SteamLibraryRoot in $SteamLibraryRoots) {
    if ($PalworldAppManifestPath.StartsWith($SteamLibraryRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
        $SteamLibraryContainingPalworld = $SteamLibraryRoot
        break
    }
}
if (-not $SteamLibraryContainingPalworld) {
    throw "could not determine the Steam library containing $PalworldAppManifestPath"
}

$ManifestGameRoot = Join-Path (
    Join-Path $SteamLibraryContainingPalworld 'steamapps\common'
) $PalworldInstallDirectoryName

if (-not (Test-Path -LiteralPath $ManifestGameRoot -PathType Container)) {
    throw "Palworld game directory from Steam manifest not found: $ManifestGameRoot"
}
$ResolvedManifestGameRoot = (Resolve-Path -LiteralPath $ManifestGameRoot).Path

if ($GameRoot) {
    if (-not (Test-Path -LiteralPath $GameRoot -PathType Container)) {
        throw "GameRoot not found: $GameRoot"
    }
    $ResolvedGameRoot = (Resolve-Path -LiteralPath $GameRoot).Path
} else {
    $ResolvedGameRoot = $ResolvedManifestGameRoot
}

if (-not $ResolvedGameRoot.Equals($ResolvedManifestGameRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "GameRoot $ResolvedGameRoot does not match the Palworld game directory from the Steam manifest: $ResolvedManifestGameRoot"
}

$PalworldWin64Directory = Join-Path $ResolvedGameRoot 'Pal\Binaries\Win64'
$ModsDirectory = Join-Path $PalworldWin64Directory 'Mods'
if (-not (Test-Path -LiteralPath $ModsDirectory -PathType Container)) {
    throw "UE4SS Mods directory not found: $ModsDirectory"
}

if ($Ue4ssDll) {
    if (-not (Test-Path -LiteralPath $Ue4ssDll -PathType Leaf)) {
        throw "Ue4ssDll not found: $Ue4ssDll"
    }
    $ResolvedUe4ssDll = (Resolve-Path -LiteralPath $Ue4ssDll).Path
} else {
    $ue4ssCandidates = @(
        (Join-Path $PalworldWin64Directory 'UE4SS.dll'),
        (Join-Path $ResolvedGameRoot 'UE4SS.dll')
    )
    $ResolvedUe4ssDll = $ue4ssCandidates |
        Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } |
        Select-Object -First 1
    if (-not $ResolvedUe4ssDll) {
        throw "UE4SS.dll not found; pass -Ue4ssDll with the pinned UE4SS 3.0.1 package path"
    }
}

if ($SaveDirectory) {
    if (-not (Test-Path -LiteralPath $SaveDirectory -PathType Container)) {
        throw "SaveDirectory not found: $SaveDirectory"
    }
    $saveSearchRoot = (Resolve-Path -LiteralPath $SaveDirectory).Path
    $levelSavePath = Join-Path $saveSearchRoot 'Level.sav'
    if (-not (Test-Path -LiteralPath $levelSavePath -PathType Leaf)) {
        throw "Level.sav not found in supplied SaveDirectory: $saveSearchRoot"
    }
    $ResolvedSaveDirectory = $saveSearchRoot
} else {
    $saveSearchRoot = Join-Path $env:LOCALAPPDATA 'Pal\Saved\SaveGames'
    if (-not (Test-Path -LiteralPath $saveSearchRoot -PathType Container)) {
        throw "Palworld save root not found: $saveSearchRoot"
    }
    $newestLevelSave = Get-ChildItem -LiteralPath $saveSearchRoot -Recurse -Filter 'Level.sav' -File -ErrorAction SilentlyContinue |
        Sort-Object -Property LastWriteTime -Descending |
        Select-Object -First 1
    if (-not $newestLevelSave) {
        throw "Level.sav not found under $saveSearchRoot; pass -SaveDirectory explicitly"
    }
    $levelSavePath = $newestLevelSave.FullName
    $ResolvedSaveDirectory = $newestLevelSave.DirectoryName
}

$RepoRoot = Split-Path -Parent $PSScriptRoot
$SupportManifestPath = Join-Path $RepoRoot 'adapter\read-only\ue4ss\support-manifest.json'
if (-not (Test-Path -LiteralPath $SupportManifestPath -PathType Leaf)) {
    throw "support manifest not found: $SupportManifestPath"
}

try {
    $SupportManifest = Get-Content -LiteralPath $SupportManifestPath -Raw | ConvertFrom-Json
} catch {
    throw "support manifest is not valid JSON: $SupportManifestPath"
}

foreach ($propertyName in @(
    'schema_version',
    'game_build_ids',
    'game_version',
    'ue4ss_version',
    'ue4ss_dll_sha256',
    'member_variable_layout_sha256',
    'package_files',
    'blocked_game_build_ids'
)) {
    if (-not $SupportManifest.PSObject.Properties[$propertyName]) {
        throw "support manifest is missing required property $propertyName"
    }
}

$AllowedGameBuildIds = @($SupportManifest.game_build_ids | Where-Object { $_ -is [string] })
$BlockedGameBuildIds = @($SupportManifest.blocked_game_build_ids | Where-Object { $_ -is [string] })
$PackageFiles = @($SupportManifest.package_files | Where-Object { $_ -is [string] })
if ($AllowedGameBuildIds.Count -eq 0 -or $BlockedGameBuildIds.Count -eq 0 -or $PackageFiles.Count -eq 0) {
    throw "support manifest game build or package file lists are empty or invalid"
}
if (-not $SupportManifest.game_version -or -not $SupportManifest.ue4ss_version) {
    throw "support manifest game or UE4SS version is empty"
}
if ($SupportManifest.ue4ss_version -ne '3.0.1') {
    throw "support manifest UE4SS version must be 3.0.1"
}
foreach ($packageFile in $PackageFiles) {
    if ([System.IO.Path]::IsPathRooted($packageFile) -or $packageFile -split '[\\/]' -contains '..') {
        throw "support manifest package path must be relative to PalworldGuider and contain no parent traversal: $packageFile"
    }
}

if ($BlockedGameBuildIds -contains $PalworldBuildId) {
    throw "Palworld build $PalworldBuildId is blocked by the support manifest"
}
if ($AllowedGameBuildIds -notcontains $PalworldBuildId) {
    throw "Palworld build $PalworldBuildId is not supported; reviewed build: $($AllowedGameBuildIds -join ', ')"
}

$ActualUe4ssDllSha256 = (Get-FileHash -LiteralPath $ResolvedUe4ssDll -Algorithm SHA256).Hash
if ($ActualUe4ssDllSha256 -ne $SupportManifest.ue4ss_dll_sha256) {
    throw "UE4SS.dll SHA256 mismatch: expected $($SupportManifest.ue4ss_dll_sha256), got $ActualUe4ssDllSha256"
}

$SteamExecutable = Get-SteamExecutablePath

Write-Host "Preflight passed for Palworld build $PalworldBuildId"
Write-Host "Game root: $ResolvedGameRoot"
Write-Host "UE4SS.dll: $ResolvedUe4ssDll"
Write-Host "Steam executable: $SteamExecutable"
Write-Host "Mods directory: $ModsDirectory"
Write-Host "Newest save directory: $ResolvedSaveDirectory"
exit 0
