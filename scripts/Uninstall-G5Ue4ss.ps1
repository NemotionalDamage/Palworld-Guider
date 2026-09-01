<#
.SYNOPSIS
Removes the Palworld Guider UE4SS package and its mods.txt entry.

.DESCRIPTION
Refuses to run while Palworld is running. Removes only Mods\PalworldGuider
and the PalworldGuider line from mods.txt, preserving every other mod
folder and mods.txt line. UE4SS itself and all other mods are left
untouched. Reports what was removed.

.PARAMETER ModsDirectory
The game's UE4SS Mods directory (for example
D:\Steam\steamapps\common\Palworld\Pal\Binaries\Win64\Mods).
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$ModsDirectory
)

$ErrorActionPreference = 'Stop'

$RunningGame = Get-Process -Name 'Palworld*' -ErrorAction SilentlyContinue
if ($RunningGame) {
    throw "refusing to uninstall while Palworld is running"
}

$TargetDirectory = Join-Path $ModsDirectory 'PalworldGuider'
$ModsFile = Join-Path $ModsDirectory 'mods.txt'
$Removed = @()

if (Test-Path -LiteralPath $TargetDirectory) {
    Remove-Item -LiteralPath (Join-Path $ModsDirectory 'PalworldGuider') -Recurse -Force
    $Removed += "Mods\PalworldGuider"
}

if (Test-Path -LiteralPath $ModsFile) {
    $ExistingLines = @(Get-Content -LiteralPath $ModsFile)
    $KeptLines = @($ExistingLines | Where-Object { $_ -notmatch '^\s*PalworldGuider\s*:' })
    if ($KeptLines.Count -ne $ExistingLines.Count) {
        Set-Content -LiteralPath $ModsFile -Value $KeptLines -Encoding Ascii
        $Removed += "mods.txt entry 'PalworldGuider : 1'"
    }
}

if ($Removed.Count -eq 0) {
    Write-Host "Removed: nothing to remove (no PalworldGuider package or mods.txt entry found)"
}
else {
    Write-Host "Removed:"
    foreach ($item in $Removed) {
        Write-Host "  $item"
    }
}
