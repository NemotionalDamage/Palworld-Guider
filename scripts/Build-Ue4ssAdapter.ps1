<#
.SYNOPSIS
Builds the Palworld Guider UE4SS package (dlls/main.dll plus the Lua scripts)
and writes a SHA256 manifest of the staged package files.

.DESCRIPTION
The build validates that -Ue4ssDll is the exact pinned UE4SS.dll (SHA256
verified), builds the native MSVC x64 shim with the VS 2022 Build Tools
toolchain, stages PalworldGuider/ under -OutputDirectory, and writes and
verifies PalworldGuider.sha256. It never writes to the game directory.

.PARAMETER Ue4ssDll
Absolute path to the target UE4SS.dll (must match the pinned SHA256).

.PARAMETER OutputDirectory
Directory that receives the build tree and the staged PalworldGuider/
package. Defaults to <repo>\.local\build\ue4ss-adapter.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$Ue4ssDll,
    [string]$OutputDirectory,
    [ValidateSet('noop', 'chat', 'full')][string]$Capability = 'full'
)

$ErrorActionPreference = 'Stop'

$ExpectedUe4ssDllSha256 = '8AC18FBFFC1EF96B0662D4A2D537B3F224C26D65CAABA7989A9404C566102B26'

if (-not (Test-Path -LiteralPath $Ue4ssDll)) {
    throw "UE4SS DLL not found: $Ue4ssDll"
}

$ActualUe4ssDllSha256 = (Get-FileHash -LiteralPath $Ue4ssDll -Algorithm SHA256).Hash
if ($ActualUe4ssDllSha256 -ne $ExpectedUe4ssDllSha256) {
    throw "UE4SS DLL SHA256 mismatch: expected $ExpectedUe4ssDllSha256, got $ActualUe4ssDllSha256"
}

$RepoRoot = Split-Path -Parent $PSScriptRoot
if (-not $OutputDirectory) {
    $OutputDirectory = Join-Path $RepoRoot '.local\build\ue4ss-adapter'
}

$VswhereExe = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
if (-not (Test-Path -LiteralPath $VswhereExe -PathType Leaf)) {
    throw "vswhere.exe not found. Install Visual Studio 2022 Build Tools with the 'Desktop development with C++' workload (including MSVC v143 x64/x86 build tools), then retry."
}

$VisualStudioRoot = & $VswhereExe `
    -latest `
    -products * `
    -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 `
    -property installationPath
if ($LASTEXITCODE -ne 0 -or -not $VisualStudioRoot -or -not (Test-Path -LiteralPath $VisualStudioRoot -PathType Container)) {
    throw "Visual Studio 2022 with the 'Desktop development with C++' workload was not found. Install Visual Studio 2022 Build Tools, MSVC v143 x64/x86 build tools, CMake, and Ninja, then retry."
}

$VsDevCmd = Join-Path $VisualStudioRoot 'Common7\Tools\VsDevCmd.bat'
$CmakeExe = Join-Path $VisualStudioRoot 'Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin\cmake.exe'
$NinjaExe = Join-Path $VisualStudioRoot 'Common7\IDE\CommonExtensions\Microsoft\CMake\Ninja\ninja.exe'

foreach ($toolPath in @($VsDevCmd, $CmakeExe, $NinjaExe)) {
    if (-not (Test-Path -LiteralPath $toolPath)) {
        throw "required Visual Studio 2022 C++ component not found: $toolPath. Install the 'Desktop development with C++' workload, including CMake and Ninja, then retry."
    }
}

$NativeSource = Join-Path $RepoRoot 'adapter\read-only\ue4ss\native'
$BuildCommand = "call `"$VsDevCmd`" -arch=x64 && `"$CmakeExe`" -S `"$NativeSource`" -B `"$OutputDirectory`" -G Ninja -DUE4SS_DLL=`"$Ue4ssDll`" -DGUIDER_LUA_CAPABILITY=$Capability && `"$CmakeExe`" --build `"$OutputDirectory`""
& cmd /d /c $BuildCommand
if ($LASTEXITCODE -ne 0) {
    throw "CMake build failed with exit code $LASTEXITCODE"
}

$PackageDirectory = Join-Path $OutputDirectory 'PalworldGuider'
$PackageFiles = @(
    (Join-Path $PackageDirectory 'dlls\main.dll'),
    (Join-Path $PackageDirectory 'Scripts\main.lua'),
    (Join-Path $PackageDirectory 'Scripts\pal_transport.lua'),
    (Join-Path $PackageDirectory 'Scripts\pal_json.lua')
)

foreach ($packageFile in $PackageFiles) {
    if (-not (Test-Path -LiteralPath $packageFile)) {
        throw "staged package file missing after build: $packageFile"
    }
}

$ManifestPath = Join-Path $OutputDirectory 'PalworldGuider.sha256'
$ManifestLines = foreach ($packageFile in $PackageFiles) {
    $fileHash = (Get-FileHash -LiteralPath $packageFile -Algorithm SHA256).Hash.ToLowerInvariant()
    $relativePath = $packageFile.Substring($PackageDirectory.Length + 1)
    "$fileHash  $relativePath"
}
Set-Content -LiteralPath $ManifestPath -Value $ManifestLines -Encoding Ascii

$ExpectedHashes = @{}
foreach ($manifestLine in $ManifestLines) {
    $parts = $manifestLine -split '\s+', 2
    $ExpectedHashes[$parts[1]] = $parts[0]
}
foreach ($packageFile in $PackageFiles) {
    $relativePath = $packageFile.Substring($PackageDirectory.Length + 1)
    $fileHash = (Get-FileHash -LiteralPath $packageFile -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($ExpectedHashes[$relativePath] -ne $fileHash) {
        throw "package hash verification failed for $relativePath"
    }
}

Write-Host "Built and verified PalworldGuider package at $PackageDirectory (manifest: $ManifestPath)"
