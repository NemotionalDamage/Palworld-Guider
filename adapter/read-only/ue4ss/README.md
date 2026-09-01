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

```powershell
$vsdev = 'C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\Common7\Tools\VsDevCmd.bat'
$cmake = 'C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin\cmake.exe'
$build = "$PWD\.local\build\g5-ue4ss-ninja"
cmd /c "call `"$vsdev`" -arch=x64 && `"$cmake`" -S adapter\read-only\ue4ss\native -B `"$build`" -G Ninja -DUE4SS_DLL=`"$PWD\.local\vendor\UE4SS_v3.0.1\UE4SS.dll`" && `"$cmake`" --build `"$build`""
```

The CMake build verifies the UE4SS DLL SHA256
(`8AC18FBFFC1EF96B0662D4A2D537B3F224C26D65CAABA7989A9404C566102B26`),
derives the UE4SS import library from the verified DLL, links only the
UE4SS import library and `winhttp`, and stages
`PalworldGuider/dlls/main.dll` plus the three `Scripts` files.

## Install

1. Close Palworld and back up the save.
2. Copy the built `PalworldGuider` package into the UE4SS `Mods`
   directory.
3. Confirm `Mods/PalworldGuider/Scripts/main.lua` and
   `Mods/PalworldGuider/dlls/main.dll` exist.
4. Start the Rust gateway with matching port and token.
5. Start the supported single-player or private-server session and type
   `!g <question>` in chat.

Live validation remains a separately gated action: only one Guider
adapter may be enabled, and hashes of the staged files must match the
built artifacts.
