# Palworld Guider UE4SS Adapter (Read-Only)

The UE4SS adapter is the read-only in-game transport for the Palworld
Guider Stage 2 service. It is a same-name Lua/C++ hybrid: Lua owns the
validated chat-input hook, the read-only game reads, JSON, and schema-2
session state; the C++ shim owns only the authenticated loopback
WinHTTP WebSocket carriage.

## Boundaries

- The adapter reads only the player position and the active-Otomo
  identity/position. It never reads inventory, party lists, health,
  stamina, nearby actors, resources, or saves.
- The adapter never mutates the game: no `AddItem`, `move_to`,
  `follow_player`, `attack`, `spawn`, `teleport`, movement, combat,
  gathering, construction, or body-lease APIs.
- The Lua layer contains no file IPC (`io.open`, `os.remove`, atomic
  writes, or response-via-file). All traffic uses the loopback
  WebSocket carrier.
- The native shim contains no Unreal or Palworld API, hook, shell,
  file-path, provider, or advisory logic.
- Chat hook re-registration is guarded, chat bodies are never logged,
  and the gateway token is never written or logged.

## Layout

```text
Scripts/main.lua            hooks, reads, tool handlers, 50 ms poll loop
Scripts/pal_transport.lua   schema-2 session state, reconnect, heartbeat
Scripts/pal_json.lua        strict JSON encode/decode
native/                     MSVC WinHTTP carrier (main.dll)
```

## Environment

- `PALWORLD_GUIDER_GATEWAY_PORT` — loopback gateway port, default `8071`.
- `PALWORLD_GUIDER_GATEWAY_TOKEN` — shared bearer token (16-4096
  characters, no control characters). The Rust gateway must use the same
  token.

## Tools

The capability manifest advertises exactly four tools:

- `ping` — connectivity check.
- `get_player_status` — finite player position or `unavailable`.
- `get_active_pal_status` — guarded Otomo identity and finite position
  or `unavailable`.
- `send_chat_message` — one non-empty message, maximum 500 characters.

Any other tool name returns `unsupported` with
`"tool is not whitelisted"`.

## Build

Prerequisites: VS 2022 Build Tools (bundled CMake and Ninja). The packaged
build, backup, install, and uninstall scripts live in `scripts/` at the
repository root.

```powershell
.\scripts\Build-Ue4ssAdapter.ps1 -Ue4ssDll "C:\path\to\UE4SS_v3.0.1\UE4SS.dll"
```

The script verifies the UE4SS DLL SHA256
(`8AC18FBFFC1EF96B0662D4A2D537B3F224C26D65CAABA7989A9404C566102B26`),
builds the native MSVC x64 shim, stages `PalworldGuider/` under
`.local\build\ue4ss-adapter` (or `-OutputDirectory`), and writes and
verifies a `PalworldGuider.sha256` manifest of the staged package. It
never writes to the game directory. The staged package contains:

```text
PalworldGuider/dlls/main.dll
PalworldGuider/Scripts/main.lua
PalworldGuider/Scripts/pal_transport.lua
PalworldGuider/Scripts/pal_json.lua
```

## Backup (required before install)

Always back up the world save before installing or updating the adapter:

```powershell
.\scripts\Backup-PalworldSave.ps1 `
  -SourceDirectory "$env:LOCALAPPDATA\Pal\Saved\SaveGames\76561198694570145\3C2BA10146F65256FD1B889FBF5F854F" `
  -DestinationDirectory "$PWD\.local\backups\palworld\<world-id>"
```

The script refuses to run while a `Palworld*` process is running, refuses
a source without `Level.sav`, refuses to overwrite a non-empty
destination, and refuses destinations outside the repo `.local\` tree. It
copies the world directory with `-LiteralPath`, verifies file count and
total byte length against the source, and writes `backup-manifest.json`
recording the completion time and a 30-day retention window. The source
save is never modified.

## Install

1. Close Palworld and confirm no `Palworld*` process is running
   (`Palworld-Win64-Shipping` or `Palworld`).
2. Build the package and create a verified backup with the scripts above.
3. Install:

```powershell
.\scripts\Install-Ue4ssAdapter.ps1 `
  -PackageDirectory "$PWD\.local\build\ue4ss-adapter\PalworldGuider" `
  -ModsDirectory "<Your Palworld installation>\Pal\Binaries\Win64\Mods" `
  -BackupDirectory "$PWD\.local\backups\palworld\<world-id>"
```

The installer requires the game to be stopped, verifies the backup
manifest before any write to the game directory, verifies the staged files
against `PalworldGuider.sha256`, installs only into
`Mods\PalworldGuider`, and updates `mods.txt` to enable only the
`PalworldGuider : 1` line while preserving every other mod's line and
folder. On any failure before completion it rolls back what it added.

4. Start the Rust gateway with matching port and token.
5. Start the supported single-player or private-server session and type
   `!guide <question>` in chat.

## Uninstall

1. Close Palworld and confirm no `Palworld*` process is running.
2. Run:

```powershell
.\scripts\Uninstall-Ue4ssAdapter.ps1 -ModsDirectory "<Your Palworld installation>\Pal\Binaries\Win64\Mods"
```

The uninstaller removes only `Mods\PalworldGuider` and the
`PalworldGuider` line from `mods.txt`. It never deletes other mod
folders, never modifies other mods.txt entries, and leaves UE4SS itself
untouched. It reports exactly what it removed.

Adapter installation remains separately gated: only one Guider adapter may be
enabled, and hashes of the staged files must match the built artifacts.
