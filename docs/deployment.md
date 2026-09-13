# Deployment Guide

This guide covers both Palworld Guider deployment modes: the in-game
MOD path comes first, followed by the Web-only server (which remains
available while the MOD is active).

## UE4SS Adapter Deployment (In-Game MOD)

The UE4SS adapter is the read-only in-game transport that carries
`!guide` questions from the Palworld chat box to the loopback guide
server. The repository `README.md` is the end-user quick start; this
guide documents the detailed behavior of the setup, start, and uninstall
scripts, plus the Web-only server.

### Prerequisites

- Windows 10/11 x64 with PowerShell 5.1+.
- Palworld installed through Steam; the game folder may be on any drive.
  Preflight reads `appmanifest_1623730.acf` from the Steam libraries, so a
  real Steam-installed game is required and `-GameRoot` cannot bypass
  discovery.
- UE4SS `3.0.1` installed into the game at
  `<game>\Pal\Binaries\Win64\UE4SS.dll`. The pinned DLL SHA256 is verified
  during preflight.
- Rust stable 1.98+ and VS 2022 Build Tools with the Desktop development
  with C++ workload (CMake and Ninja) for the source build.
- Provider configuration from `Set-GuideProvider.ps1` (or session environment
  variables): `GUIDE_PROVIDER` (`openai` or `ollama`), `GUIDE_MODEL`, and
  `OPENAI_API_KEY` when `GUIDE_PROVIDER=openai`.

### Setup And Start

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
  the Palworld app manifest, verifies the installed UE4SS DLL hash,
  requires the UE4SS `Mods` directory, and finds the newest save
  containing `Level.sav`. Unreviewed Steam build IDs produce a warning
  only, so normal game updates do not break setup; the UE4SS version and
  pinned DLL hash remain hard requirements.
- Setup refuses to continue when `Mods\PalworldGuider` already exists and
  prints guidance to run `Uninstall-Ue4ssAdapter.ps1` first.
- The newest save is backed up to
  `<repo>\.local\backups\palworld\<timestamp>-<world>` and verified
  before any game write.
- The MOD package is staged under `<repo>\.local\build\ue4ss-adapter` and
  its `PalworldGuider.sha256` manifest is verified.
- Only then is `Mods\PalworldGuider` installed and enabled in `mods.txt`,
  preserving every unrelated mod.

Preflight is read-only and fails closed: on any failure, setup exits
with code `1` before backing up, building, or writing to the game
directory. `-WhatIf` runs preflight and reports the planned stages
without changing anything.

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
to a file, the registry, or the command line. It loads the saved provider
configuration when present, then copies the provider environment variables
(`GUIDE_PROVIDER`, `GUIDE_MODEL`, `GUIDE_BASE_URL`,
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
| `-ProviderTimeoutSeconds` | `300` | Maximum wait for one model response |
| `-ServerExecutable` | `target\release\guide-server.exe` | guide-server binary to launch |
| `-SteamExecutable` | Discovered from Steam's registry `InstallPath` | Steam client used to launch Palworld (app `1623730`) |

#### In-Game Usage

Palworld opens automatically in the Steam client that Start launched;
enter a single-player or private session and use the chat box. The
supported commands are:

```text
!guide ping
!guide <question>
!guide retry
!guide new
```

- `!guide ping` returns `Pong: Palworld Guider adapter connected.` without
  calling the provider.
- `!guide <question>` is routed through the same grounded agent loop as
  the Web interface.
- `!guide retry` clears the context and re-asks the previous question;
  `!guide new` clears the conversation without re-asking. Unrelated
  questions always start with clean context; explicit follow-up markers
  (继续/刚才/continue/again) carry only the latest exchange.

#### Uninstall

Close Palworld, then run:

```powershell
.\scripts\Uninstall-Ue4ssAdapter.ps1 -ModsDirectory "<game>\Pal\Binaries\Win64\Mods"
```

The uninstaller removes only `Mods\PalworldGuider` and the
`PalworldGuider` line from `mods.txt`. Unrelated mods and UE4SS itself
are preserved. Run it before re-running setup whenever
`Mods\PalworldGuider` already exists.

#### If Preflight Fails

Setup and Start never modify anything before the read-only preflight
passes. When preflight exits with code `1`, no backup, build, or game
write has happened - check the printed reason first, then:

- Confirm the installed UE4SS `3.0.1` DLL hash matches the pinned value
  (a UE4SS reinstall or a game update can change the file).
- Confirm the Steam library and save locations are readable and that the
  newest save contains `Level.sav`.
- See [troubleshooting.md](troubleshooting.md) for the common
  causes and fixes, including the UE4SS crash-on-save-entry fix (heap
  corruption `0xc0000374`) and adapter connection failures.

#### Developer Build

The standard path builds automatically. Only developers need
`Build-Ue4ssAdapter.ps1`, which accepts `-Capability` with the values
`noop`, `chat`, or `full` (default `full`) in addition to `-Ue4ssDll` and
`-OutputDirectory`. `noop` supports only the transport heartbeat and
`ping`; `chat` disables player and Otomo reads; `full` is the default
player-facing build. See
[adapter/read-only/ue4ss/README.md](../adapter/read-only/ue4ss/README.md).

#### Adapter Read-Only Boundary

- The adapter reads only the player position, the active-Otomo
  identity/position, and player base-camp positions. No inventory,
  party, health, stamina, save, or nearby-actor reads.
- No mutation path exists: no movement, combat, gathering, construction,
  inventory mutation, or world writes. The model-visible tool allowlist is
  fixed at compile time (`get_player_status`, `get_active_pal_status`,
  `get_base_camps`),
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
- A provider configured once through `Set-GuideProvider.ps1`

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
.\scripts\Set-GuideProvider.ps1   # once per clone
.\scripts\Start-WebGuide.ps1
```

`Start-WebGuide.ps1` loads `.local\set-provider.ps1` and runs the release
server in the foreground. For a one-off manual run without the saved
configuration:

```powershell
$env:GUIDE_PROVIDER = "ollama"        # or "openai"
$env:GUIDE_MODEL = "llama3.2"
./target/release/guide-server --data data/reviewed --game-version 1.0 --port 8070
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

`Start-InGameGuide.ps1` and `Start-WebGuide.ps1` pass the timeout automatically;
you normally do not need to call `guide-server` with adapter flags by hand.

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
- Start-script model timeout: 300 seconds (`-ProviderTimeoutSeconds`, range
  1..=300). Direct `guide-server` startup defaults to 30 seconds.
