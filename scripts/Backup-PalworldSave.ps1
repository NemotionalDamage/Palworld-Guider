<#
.SYNOPSIS
Backs up a Palworld world save directory into the repo .local backup area
with verification and a 30-day retention manifest.

.DESCRIPTION
Refuses to run when Level.sav is missing, when the destination already
contains files, when the destination resolves outside the repo .local
directory, or while a Palworld process is running. Copies the world
directory with -LiteralPath, verifies file count, total byte length, and each
file's SHA256 against the source, and records completion time, retention, and
per-file records in backup-manifest.json. The source save is never modified.

.PARAMETER SourceDirectory
Absolute path to the world save directory that contains Level.sav
(for example ...\SaveGames\76561198694570145\3C2BA10146F65256FD1B889FBF5F854F).

.PARAMETER DestinationDirectory
Absolute destination path under <repo>\.local\backups\palworld\ that will hold a
verified copy of the world directory.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$SourceDirectory,
    [Parameter(Mandatory = $true)]
    [string]$DestinationDirectory
)

$ErrorActionPreference = 'Stop'
$SourceDirectory = (Resolve-Path -LiteralPath $SourceDirectory).Path

$LevelSavPath = Join-Path $SourceDirectory 'Level.sav'
if (-not (Test-Path -LiteralPath $LevelSavPath)) {
    throw "refusing to back up: Level.sav not found in $SourceDirectory"
}

$RunningGame = Get-Process -Name 'Palworld*' -ErrorAction SilentlyContinue
if ($RunningGame) {
    $ProcessNames = ($RunningGame | ForEach-Object { $_.ProcessName }) -join ', '
    throw "refusing to back up while Palworld is running (processes: $ProcessNames)"
}

$RepoRoot = Split-Path -Parent $PSScriptRoot
$LocalRootPath = Join-Path $RepoRoot '.local'
if (-not (Test-Path -LiteralPath $LocalRootPath -PathType Container)) {
    New-Item -ItemType Directory -Path $LocalRootPath -Force | Out-Null
}
$LocalRoot = (Resolve-Path -LiteralPath $LocalRootPath).Path
if (-not [System.IO.Path]::IsPathRooted($DestinationDirectory)) {
    throw "refusing to back up outside .local: destination must be an absolute path"
}
$DestinationFull = [System.IO.Path]::GetFullPath($DestinationDirectory)
if (-not $DestinationFull.StartsWith($LocalRoot + [System.IO.Path]::DirectorySeparatorChar, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "refusing to back up outside .local: $DestinationFull is not under $LocalRoot"
}

if (Test-Path -LiteralPath $DestinationFull) {
    $ExistingCount = @(Get-ChildItem -LiteralPath $DestinationFull -Force -ErrorAction SilentlyContinue).Count
    if ($ExistingCount -gt 0) {
        throw "refusing to overwrite non-empty backup destination: $DestinationFull"
    }
}

New-Item -ItemType Directory -Path $DestinationFull -Force | Out-Null
$SourceChildren = Get-ChildItem -LiteralPath $SourceDirectory -Force
foreach ($SourceChild in $SourceChildren) {
    Copy-Item -LiteralPath $SourceChild.FullName -Destination $DestinationFull -Recurse -Force
}

$SourceFiles = @(Get-ChildItem -LiteralPath $SourceDirectory -Recurse -File -Force)
$CopiedFiles = @(Get-ChildItem -LiteralPath $DestinationFull -Recurse -File -Force)
if ($SourceFiles.Count -ne $CopiedFiles.Count) {
    throw "backup verification failed: source file count $($SourceFiles.Count) does not match copied $($CopiedFiles.Count)"
}
$SourceBytes = ($SourceFiles | Measure-Object -Property Length -Sum).Sum
$CopiedBytes = ($CopiedFiles | Measure-Object -Property Length -Sum).Sum
if ($SourceBytes -ne $CopiedBytes) {
    throw "backup verification failed: source bytes $SourceBytes do not match copied $CopiedBytes"
}

$FileRecords = @(
    foreach ($CopiedFile in $CopiedFiles) {
        $RelativePath = $CopiedFile.FullName.Substring($DestinationFull.Length + 1)
        $SourceFile = $SourceFiles | Where-Object {
            $_.FullName.Substring($SourceDirectory.Length + 1) -eq $RelativePath
        } | Select-Object -First 1
        if (-not $SourceFile) {
            throw "backup verification failed: unexpected copied file $RelativePath"
        }
        $SourceHash = (Get-FileHash -LiteralPath $SourceFile.FullName -Algorithm SHA256).Hash
        $CopiedHash = (Get-FileHash -LiteralPath $CopiedFile.FullName -Algorithm SHA256).Hash
        if ($SourceHash -ne $CopiedHash) {
            throw "backup verification failed: SHA256 mismatch for $RelativePath"
        }
        [ordered]@{
            path   = $RelativePath
            sha256 = $SourceHash.ToLowerInvariant()
            length = $SourceFile.Length
        }
    }
)

$RetentionDays = 30
$CompletedAt = Get-Date
$ManifestRecord = [ordered]@{
    source          = $SourceDirectory
    destination     = $DestinationFull
    completed_at    = $CompletedAt.ToString('o')
    retention_days  = $RetentionDays
    retention_until = $CompletedAt.AddDays($RetentionDays).ToString('o')
    file_count      = $CopiedFiles.Count
    total_bytes     = $CopiedBytes
    files           = $FileRecords
}
$ManifestPath = Join-Path $DestinationFull 'backup-manifest.json'
$ManifestRecord | ConvertTo-Json | Set-Content -LiteralPath $ManifestPath -Encoding Utf8

Write-Host "Backed up $($CopiedFiles.Count) files ($CopiedBytes bytes) to $DestinationFull (retained $RetentionDays days)"
