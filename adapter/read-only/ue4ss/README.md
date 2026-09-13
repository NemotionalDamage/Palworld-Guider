# Palworld Guider UE4SS Adapter (Read-Only)

The UE4SS adapter is the read-only in-game transport for the Palworld
Guider loopback guide server. It is a same-name Lua/C++ hybrid: Lua owns
the validated chat-input hook, the read-only game reads, JSON, and
schema-2 session state; the C++ shim owns only the authenticated loopback
WinHTTP WebSocket carriage.

## Boundaries

- The adapter reads only the player position, the active-Otomo
  identity/position, and player base-camp positions. It never reads
  inventory, party lists, health, stamina, nearby actors, resources, or
  saves.
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

- `PALWORLD_GUIDER_GATEWAY_PORT` — loopback gateway port read by the Lua
  transport, default `8071`. `Start-InGameGuide.ps1` binds it to the
  launching session from its `-AdapterPort` value.
- `PALWORLD_GUIDER_GATEWAY_TOKEN` — shared bearer token (16-4096
  characters, no control characters), read from the game process
  environment. `Start-InGameGuide.ps1` generates the token in memory,
  binds it to the launching PowerShell session, and launches Steam from
  that session, so the Rust gateway and the game read the same value; it
  is never printed or written to a file or the registry.

## Tools

The capability manifest is selected at build time by the
`GUIDER_LUA_CAPABILITY` CMake value (default `full`), not by runtime user
input. The allowed values are exactly `noop`, `chat`, and `full`:

- `noop` — transport heartbeat and `ping` only; no chat hook or tool.
- `chat` — `ping` and `send_chat_message`; player, active-Otomo, and
  base-camp reads are disabled in both the manifest and dispatch.
- `full` — all five tools below, matching current behavior.

With the default `full` manifest, the adapter advertises exactly five
tools:

- `ping` — connectivity check.
- `get_player_status` — finite player position or `unavailable`.
- `get_active_pal_status` — guarded Otomo identity and finite position
  or `unavailable`.
- `get_base_camps` — player base-camp names and finite positions, or
  `unavailable`.
- `send_chat_message` — one non-empty message, maximum 500 characters.

Any other tool name returns `unsupported` with
`"tool is not whitelisted"`.

## Quick Start

Clone the repository and run the clone-to-game path on a supported
Palworld installation:

```powershell
git clone https://github.com/NemotionalDamage/Palworld-Guider.git
cd Palworld-Guider
cargo build --release
.\scripts\Setup-InGameGuide.ps1
.\scripts\Start-InGameGuide.ps1
```

- `Setup-InGameGuide.ps1` runs the read-only preflight (supported build
  and pinned UE4SS DLL hash), refuses a blocked build and an existing
  `Mods\PalworldGuider`, backs up the newest save, builds the default
  `full` package, and installs it.
- `Start-InGameGuide.ps1` validates the installed package, generates the
  gateway token in memory, binds it to the session, starts the loopback
  guide server, and relaunches Palworld through Steam so the game
  inherits the token. Palworld must be closed when Start runs.

See the [repository README](../../../README.md) and
[docs/deployment.md](../../../docs/deployment.md) for prerequisites,
supported-build and blocked-build behavior, in-game usage (`!guide ping`
and `!guide <question>`), and uninstall.

## Developer Build

The standard path builds automatically with the default `full`
capability. Developers can build a specific capability directly:

```powershell
.\scripts\Build-Ue4ssAdapter.ps1 -Ue4ssDll "C:\path\to\UE4SS_v3.0.1\UE4SS.dll"
.\scripts\Build-Ue4ssAdapter.ps1 -Ue4ssDll "C:\path\to\UE4SS_v3.0.1\UE4SS.dll" -Capability chat
```

- `-Ue4ssDll` (required) — the pinned UE4SS 3.0.1 DLL; its SHA256
  (`8AC18FBFFC1EF96B0662D4A2D537B3F224C26D65CAABA7989A9404C566102B26`)
  is verified before anything is staged.
- `-Capability` — `noop`, `chat`, or `full`; defaults to `full`. The
  default `full` build is required for the player-facing
  `!guide <question>` flow.
- `-OutputDirectory` — staging root; defaults to
  `<repo>\.local\build\ue4ss-adapter`.

The script verifies the UE4SS DLL SHA256, builds the native MSVC x64 shim
with the VS 2022 Build Tools toolchain (CMake and Ninja), stages
`PalworldGuider/`, and writes and verifies a `PalworldGuider.sha256`
manifest of the staged package. It never writes to the game directory.
The staged package contains:

```text
PalworldGuider/dlls/main.dll
PalworldGuider/Scripts/main.lua
PalworldGuider/Scripts/pal_transport.lua
PalworldGuider/Scripts/pal_json.lua
```

## Backup And Install

`Setup-InGameGuide.ps1` performs discovery, backup, build, verification,
and install in one step: it finds the newest save containing `Level.sav`,
creates and verifies a backup under `<repo>\.local\backups\palworld`, and
installs only into `<game>\Pal\Binaries\Win64\Mods\PalworldGuider`. It
refuses an existing `Mods\PalworldGuider` (uninstall it first) and refuses
blocked builds before any backup or write.

Advanced manual flow with the same guarantees:

1. Close Palworld; no `Palworld*` process may be running.
2. Build the package (above) and create a verified backup:

   ```powershell
   .\scripts\Backup-PalworldSave.ps1 `
     -SourceDirectory <newest save directory from preflight> `
     -DestinationDirectory "$PWD\.local\backups\palworld\<backup-name>"
   ```

   The backup script refuses to run while a `Palworld*` process is
   running, refuses a source without `Level.sav`, refuses to overwrite a
   non-empty destination, and refuses destinations outside the repository
   `.local\` tree. It copies the world directory with `-LiteralPath`,
   verifies file count and total byte length against the source, and
   writes `backup-manifest.json` recording the completion time and a
   30-day retention window. The source save is never modified.

3. Install:

   ```powershell
   .\scripts\Install-Ue4ssAdapter.ps1 `
     -PackageDirectory "$PWD\.local\build\ue4ss-adapter\PalworldGuider" `
     -ModsDirectory "<game>\Pal\Binaries\Win64\Mods" `
     -BackupDirectory "<repo>\.local\backups\palworld\<backup-name>"
   ```

   The installer requires the game to be stopped, verifies the backup
   manifest before any write to the game directory, verifies the staged
   files against `PalworldGuider.sha256`, installs only into
   `Mods\PalworldGuider`, and updates `mods.txt` to enable only the
   `PalworldGuider : 1` line while preserving every other mod's line and
   folder. On any failure before completion it rolls back what it added.

## Start

Run `Start-InGameGuide.ps1` after setup or any manual install:

```powershell
.\scripts\Start-InGameGuide.ps1
```

The script revalidates the installed package and hash manifest, generates
the gateway token in memory and binds it to the launching session,
forwards provider environment variables to the child process, and launches
the loopback guide server. Palworld must not already be running: any
running Steam client is closed gracefully, then Palworld (Steam app
`1623730`) is launched through Steam from the same session so the game
inherits the token. It prints the Web interface and adapter gateway
addresses and never prints the token or provider credentials.

## Uninstall

1. Close Palworld and confirm no `Palworld*` process is running.
2. Run:

```powershell
.\scripts\Uninstall-Ue4ssAdapter.ps1 -ModsDirectory "<game>\Pal\Binaries\Win64\Mods"
```

The uninstaller removes only `Mods\PalworldGuider` and the
`PalworldGuider` line from `mods.txt`. It never deletes other mod
folders, never modifies other mods.txt entries, and leaves UE4SS itself
untouched. It reports exactly what it removed.

Setup refuses a new installation while `Mods\PalworldGuider` exists and
points to this uninstaller, so at most one Guider adapter is ever enabled.
Hashes of installed files must always match the built artifacts.
