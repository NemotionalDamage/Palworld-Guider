# Deployment Guide

This guide covers both Palworld Guider deployment modes. The in-game MOD
clone-to-game path comes first; the Web-only path follows and remains
available while the MOD is active.

## UE4SS Adapter Deployment (In-Game MOD)

The UE4SS adapter is the read-only in-game transport for the guide
server. It is validated only against Palworld Steam build `24575825`
(game `1.0.3`); Steam build `25094871` is blocked and every preflight
refuses it before any state change.

### Prerequisites

- Windows 10/11 x64 with PowerShell 5.1+.
- Palworld Steam build `24575825` (game `1.0.3`) installed through Steam.
  Preflight reads `appmanifest_1623730.acf` from the Steam libraries, so a
  Steam-installed game is required and `-GameRoot` cannot bypass build
  validation.
- UE4SS `3.0.1` installed into the game at
  `<game>\Pal\Binaries\Win64\UE4SS.dll`. The pinned DLL SHA256 is verified
  during preflight.
- Rust stable 1.98+ and VS 2022 Build Tools with the Desktop development
  with C++ workload (CMake and Ninja) for the source build.
- Provider environment variables in the shell that later runs
  `Start-InGameGuide.ps1`: `GUIDE_PROVIDER` (`openai` or `ollama`),
  `GUIDE_MODEL`, and `OPENAI_API_KEY` when `GUIDE_PROVIDER=openai`.

### Clone-To-Game Path

```powershell
git clone https://github.com/NemotionalDamage/Palworld-Guider.git
cd Palworld-Guider
cargo build --release
.\scripts\Setup-InGameGuide.ps1
.\scripts\Start-InGameGuide.ps1
```

#### Setup

`Setup-InGameGuide.ps1` runs, in order: read-only preflight; an
existing-installation check; a verified save backup; a package build and
hash verification; and the explicit game install. Details:

- Preflight (`Test-InGameGuide.ps1`) locates the Steam installation and
  the Palworld app manifest, verifies the build ID against the reviewed
  support manifest, verifies the installed UE4SS DLL hash, requires the
  UE4SS `Mods` directory, and finds the newest save containing
  `Level.sav`.
- Setup refuses to continue when `Mods\PalworldGuider` already exists and
  prints guidance to run `Uninstall-Ue4ssAdapter.ps1` first.
- The newest save is backed up to
  `<repo>\.local\backups\palworld\<timestamp>-<world>` and verified
  before any game write.
- The MOD package is staged under `<repo>\.local\build\ue4ss-adapter` and
  its `PalworldGuider.sha256` manifest is verified.
- Only then is `Mods\PalworldGuider` installed and enabled in `mods.txt`,
  preserving every unrelated mod.

On a blocked or unsupported build, setup fails closed with exit code `1`
before backing up, building, or writing to the game directory. `-WhatIf`
runs preflight and reports the planned stages without changing anything.

Setup options (all optional; omitted paths are discovered automatically):

| Parameter | Purpose |
|---|---|
| `-GameRoot` | Palworld game directory; must equal the Steam-manifest path |
| `-Ue4ssDll` | Path to the pinned UE4SS 3.0.1 DLL |
| `-SaveDirectory` | Save directory containing the newest `Level.sav` |
| `-OutputDirectory` | Package staging root (default `<repo>\.local\build\ue4ss-adapter`) |
| `-WhatIf` | Report planned stages; never back up, build, or install |

#### Start

`Start-InGameGuide.ps1` validates the installed package files and hash
manifest, generates a random 64-character gateway token in memory, and
binds it (and the gateway port) to the launching PowerShell session — never
to a file, the registry, or the command line. It copies the provider
environment variables (`GUIDE_PROVIDER`, `GUIDE_MODEL`, `GUIDE_BASE_URL`,
`GUIDE_DISABLE_REASONING`, `OPENAI_API_KEY`) and the token into the
child-process environment, launches `target\release\guide-server.exe` (or
`-ServerExecutable`), and verifies it is listening. Palworld must not
already be running: if a Steam client is running it is closed gracefully,
and Palworld is then relaunched through Steam (app `1623730`) from this
session so the game process inherits the same token and port. It prints the
loopback addresses and never prints the token or provider credentials:

```text
Web interface: http://127.0.0.1:<port>/
Adapter gateway: 127.0.0.1:<adapter-port>
```

Start options (all optional):

| Parameter | Default | Purpose |
|---|---|---|
| `-Port` | `8070` | Loopback Web server port |
| `-AdapterPort` | `8071` | Loopback adapter gateway port; must differ from `-Port` |
| `-ServerExecutable` | `target\release\guide-server.exe` | guide-server binary to launch |
| `-SteamExecutable` | Discovered from Steam's registry `InstallPath` | Steam client used to launch Palworld (app `1623730`) |

#### In-Game Usage

Palworld opens automatically in the Steam client that Start launched;
enter a single-player or private session and use the chat box. The
supported commands are:

```text
!guide ping
!guide <question>
```

- `!guide ping` returns `Pong: Palworld Guider adapter connected.` without
  calling the provider.
- `!guide <question>` is routed through the same grounded agent loop as
  the Web interface.

#### Uninstall

Close Palworld, then run:

```powershell
.\scripts\Uninstall-Ue4ssAdapter.ps1 -ModsDirectory "<game>\Pal\Binaries\Win64\Mods"
```

The uninstaller removes only `Mods\PalworldGuider` and the
`PalworldGuider` line from `mods.txt`. Unrelated mods and UE4SS itself
are preserved. Run it before re-running setup whenever
`Mods\PalworldGuider` already exists.

#### Blocked And Unsupported Builds

Preflight is read-only and fails closed. Steam build `25094871` is still
listed in `blocked_game_build_ids`; setup and start exit `1` with
`Palworld build 25094871 is blocked by the support manifest` before any
backup, build, or game write. Builds outside the reviewed matrix
(`24575825`) are refused the same way, as is any UE4SS DLL whose SHA256
does not match the pinned hash. The `25094871` entry moves into
`game_build_ids` only after the `full` capability probe passes - see the
probe record in `docs/mod-rollout-plan.md`.

#### Verified Manual Flow On A Blocked Build (25094871)

The scripts refuse `25094871` by design, yet on 2026-09-08 the adapter
transport and the chat path were verified live on that build after a
one-time UE4SS settings fix. The complete step-by-step narrative is in
the repository `README.md` ("Manual Start On A Blocked Build"); the
parts that differ from the supported path are summarized here.

First, disable the crashing UE4SS world-load hooks once. In
`<game>\Pal\Binaries\Win64\UE4SS-settings.ini`:

```ini
[Hooks]
HookInitGameState = 0
HookCallFunctionByNameWithArguments = 0
HookBeginPlay  = 0
HookLocalPlayerExec = 0
```

Keep `HookProcessInternal = 1` and `HookProcessLocalScriptFunction = 1`.
Without this fix, UE4SS 3.0.1 crashes on save entry (heap corruption
`0xc0000374`); with it, the game loads normally and `!guide` works,
because the adapter chat hook (`PalGameStateInGame:BroadcastChatMessage`)
and transport use none of the disabled hooks.

Then build, back up, and install exactly as in the supported path, but
with explicit paths because preflight stops first:

```powershell
cargo build --release
.\scripts\Build-Ue4ssAdapter.ps1 -Ue4ssDll "<game>\Pal\Binaries\Win64\UE4SS.dll" -Capability chat -OutputDirectory "$PWD\.local\build\ue4ss-adapter"
.\scripts\Backup-PalworldSave.ps1 -SourceDirectory "<newest save dir containing Level.sav>" -DestinationDirectory "$PWD\.local\backups\palworld\manual-start"
.\scripts\Install-Ue4ssAdapter.ps1 -PackageDirectory "$PWD\.local\build\ue4ss-adapter\PalworldGuider" -ModsDirectory "<game>\Pal\Binaries\Win64\Mods" -BackupDirectory "$PWD\.local\backups\palworld\manual-start"
```

`-Capability chat` is the build verified on `25094871`; the default
`full` build stays reserved for supported `24575825` until the `full`
layer probe passes. Start the loopback server and launch the game from
the same PowerShell session so the game inherits the token:

```powershell
$env:GUIDE_PROVIDER = "ollama"          # or "openai"
$env:GUIDE_MODEL = "llama3.2"
# when GUIDE_PROVIDER=openai also set: $env:OPENAI_API_KEY = "your-key"
$env:PALWORLD_GUIDER_GATEWAY_TOKEN = [guid]::NewGuid().ToString("N") + [guid]::NewGuid().ToString("N")
$env:PALWORLD_GUIDER_GATEWAY_PORT = "8071"
Start-Process -FilePath "$PWD\target\release\guide-server.exe" -WorkingDirectory $PWD -WindowStyle Hidden -ArgumentList @("--data", "$PWD\data\reviewed", "--game-version", "1.0.3", "--port", "8070", "--adapter-port", "8071", "--adapter-token-env", "PALWORLD_GUIDER_GATEWAY_TOKEN")
Start-Sleep -Seconds 2
Get-Process steam -ErrorAction SilentlyContinue | ForEach-Object { $null = $_.CloseMainWindow() }
Start-Sleep -Seconds 5
Start-Process "<Steam install>\Steam.exe" -ArgumentList "-applaunch", "1623730"
```

Enter a save and send `!guide ping` in the chat box; the expected reply
is `Pong: Palworld Guider adapter connected.` Exit the game and stop
`guide-server` when done. Verified scope on `25094871`: noop transport
layer and chat layer; the `full` layer (player/Otomo reads and
`!guide <question>`) is pending probe.

#### Developer Build

The standard path builds automatically. Only developers need
`Build-Ue4ssAdapter.ps1`, which accepts `-Capability` with the values
`noop`, `chat`, or `full` (default `full`) in addition to `-Ue4ssDll` and
`-OutputDirectory`. `noop` supports only the transport heartbeat and
`ping`; `chat` disables player and Otomo reads; `full` is the default
player-facing build. See
[adapter/read-only/ue4ss/README.md](../adapter/read-only/ue4ss/README.md).

#### Adapter Read-Only Boundary

- The adapter reads only the player position and the active-Otomo
  identity/position. No inventory, party, health, stamina, save, or
  nearby-actor reads.
- No mutation path exists: no movement, combat, gathering, construction,
  inventory mutation, or world writes. The model-visible tool allowlist is
  fixed at compile time (`get_player_status`, `get_active_pal_status`),
  and `send_chat_message` is internal.
- The Web server and adapter gateway bind only `127.0.0.1`; chat bodies
  are never logged and the gateway token is never printed or written.

## Web-Only Deployment (No MOD)

The guide also runs as a local Web service without the MOD. Palworld does
not need to be installed or running, and the Web interface remains
available at `http://127.0.0.1:8070/` while the MOD is active.

### Prerequisites

- Rust toolchain (stable) with `cargo`
- Reviewed knowledge data under `data/reviewed/`
- A provider configured through environment variables

### Build

```powershell
cargo build --release
```

The release binary is placed at `target/release/guide-server.exe`.

### Environment Variables

| Variable | Required | Description |
|---|---|---|
| `GUIDE_PROVIDER` | Yes | Provider kind: `openai` or `ollama`. |
| `GUIDE_MODEL` | Yes | Model name for the selected provider (e.g. `gpt-4o-mini`, `llama3.2`). |
| `OPENAI_API_KEY` | Yes for `openai` | API key for the OpenAI-compatible provider. Read from the environment only; never logged. |
| `GUIDE_BASE_URL` | No | Overrides the provider chat-completions endpoint. For `ollama`, defaults to `http://localhost:11434`. |
| `GUIDE_DISABLE_REASONING` | No | Set to `1` or `true` to disable reasoning/thinking for capable models. |
| `OLLAMA_BASE_URL` | No | Alternative override for the Ollama endpoint. |
| `PALWORLD_GUIDER_CHAT_DEBUG_LOG` | No | Opt-in local JSONL debug log path under a gitignored directory. Records questions, replies, statuses, errors, uncertainty, and tool names. |

### Run

```powershell
$env:GUIDE_PROVIDER = "ollama"
$env:GUIDE_MODEL = "llama3.2"
./target/release/guide-server --data data/reviewed --game-version 1.0.3 --port 8070
```

For an OpenAI-compatible provider:

```powershell
$env:GUIDE_PROVIDER = "openai"
$env:GUIDE_MODEL = "gpt-4o-mini"
$env:OPENAI_API_KEY = "your-key-here"
./target/release/guide-server --data data/reviewed --game-version 1.0.3 --port 8070
```

### CLI Flags

| Flag | Default | Description |
|---|---|---|
| `--data <dir>` | Required | Path to the reviewed knowledge data directory. |
| `--game-version <ver>` | None | Configured game version for version-match warnings. |
| `--port <1-65535>` | `8070` | Loopback Web server port. |
| `--timeout-seconds <1-300>` | `30` | Agent ask timeout. |
| `--adapter-port <1-65535>` | None | Enables in-game adapter mode on this loopback port. |
| `--adapter-token-env <name>` | `PALWORLD_GUIDER_GATEWAY_TOKEN` | Environment variable carrying the adapter bearer token. |
| `--host` | Rejected | Explicitly rejected; the server is loopback-only. |

`Start-InGameGuide.ps1` passes `--data`, `--game-version`, `--port`,
`--adapter-port`, and `--adapter-token-env` automatically; you normally do
not need to call `guide-server` with adapter flags by hand.

### Loopback Binding

The server always binds `127.0.0.1`. It never listens on a public
interface. The `--host` flag is explicitly rejected at parse time. Never
expose the server to a network.

### Health Check

```powershell
Invoke-RestMethod -Uri http://127.0.0.1:8070/health
```

Returns `200 OK` with `{"status":"ok"}` when the server is running.

### Browser UI

Open `http://127.0.0.1:8070/` in a browser. The dependency-free local page
exposes question input, snapshot attachment, provenance, uncertainty,
history, and error regions without external origins.

### API Endpoints

| Method | Path | Description |
|---|---|---|
| `POST` | `/api/sessions` | Create a new session. Returns `session_id` and `expires_in_seconds`. |
| `GET` | `/api/sessions/{id}` | Retrieve session history and attached snapshot metadata. |
| `POST` | `/api/sessions/{id}/snapshots` | Attach a validated player-state snapshot to the session. Returns redacted metadata only. |
| `POST` | `/api/sessions/{id}/ask` | Ask a question. Returns the full agent answer envelope with provenance, uncertainty, tool calls, and errors. |
| `POST` | `/api/cancellations/{token}` | Cancel an in-flight ask by its 32-character hex cancellation token. |

All request bodies are limited to 65,536 bytes and JSON depth of 32.
Questions are limited to 2,000 characters.

## Safety Checks

### Network Binding

- The server always binds `127.0.0.1`. It never listens on a public
  interface.
- The `--host` flag is explicitly rejected at CLI parse time with an error
  message.
- The adapter gateway also binds only `127.0.0.1`.

### No Mutation Path

No movement, combat, gathering, construction, inventory mutation, or world
mutation path exists anywhere in the codebase. The guide is read-only.

### Credential Handling

- Provider credentials (`OPENAI_API_KEY`) are read from the environment
  only. They are never written to logs, answer envelopes, or Git.
- `Start-InGameGuide.ps1` generates `PALWORLD_GUIDER_GATEWAY_TOKEN` in
  memory and binds it only to the launching session and its children
  (guide-server and the Steam client it launches); it is never placed on
  the command line, printed, or written to a file or the registry.
- The adapter token is validated for length (16..=4096 characters), no
  surrounding whitespace, and no control characters.

### Snapshot Redaction

- Snapshot data exposed to the API is redacted to only: `schema_version`,
  `source_kind`, `game_version`, `freshness`, and `missing_fields`.
- Consent IDs, `captured_at` timestamps, and raw inventory contents are
  never included in API responses or provider requests.
- The model receives only question-relevant summaries derived from the
  snapshot, never the raw snapshot itself.

### Request Limits

- Maximum request body size: 65,536 bytes.
- Maximum JSON depth: 32.
- Maximum question length: 2,000 characters.
- Rate limits: 10 asks per minute per session, 3 snapshots per minute per
  session.
- Maximum concurrent sessions: 32.
- Session TTL: 30 minutes.
- Maximum tool calls per question: 6.
- Maximum reply length: 1,200 characters.
- Agent timeout: 30 seconds (configurable via `--timeout-seconds`, range
  1..=300).
