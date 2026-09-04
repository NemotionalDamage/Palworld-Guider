# Deployment Guide

This guide covers deploying the Palworld Guider Web interface and the optional UE4SS read-only in-game chat adapter.

## Web Interface Deployment

### Prerequisites

- Rust toolchain (stable) with `cargo`
- Reviewed knowledge data under `data/reviewed/`
- A provider configured through environment variables

### Build

```powershell
cargo build --release
```

The release binary is placed at `target/release/guide-server`.

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

### Loopback Binding

The server always binds `127.0.0.1`. It never listens on a public interface. The `--host` flag is explicitly rejected at parse time. Never expose the server to a network.

### Health Check

```powershell
Invoke-RestMethod -Uri http://127.0.0.1:8070/health
```

Returns `200 OK` with `{"status":"ok"}` when the server is running.

### Browser UI

Open `http://127.0.0.1:8070/` in a browser. The dependency-free local page exposes question input, snapshot attachment, provenance, uncertainty, history, and error regions without external origins.

### API Endpoints

| Method | Path | Description |
|---|---|---|
| `POST` | `/api/sessions` | Create a new session. Returns `session_id` and `expires_in_seconds`. |
| `GET` | `/api/sessions/{id}` | Retrieve session history and attached snapshot metadata. |
| `POST` | `/api/sessions/{id}/snapshots` | Attach a validated player-state snapshot to the session. Returns redacted metadata only. |
| `POST` | `/api/sessions/{id}/ask` | Ask a question. Returns the full agent answer envelope with provenance, uncertainty, tool calls, and errors. |
| `POST` | `/api/cancellations/{token}` | Cancel an in-flight ask by its 32-character hex cancellation token. |

All request bodies are limited to 65,536 bytes and JSON depth of 32. Questions are limited to 2,000 characters.

## UE4SS Adapter Deployment

The UE4SS adapter enables read-only in-game chat. It is optional and off by default.

### Prerequisites

- Palworld Steam build (validated against build `24575825`)
- UE4SS 3.0.1 or later installed and working against the target build
- A built and hash-verified PalworldGuider adapter package

### Build the Adapter

```powershell
.\scripts\Build-G5Ue4ss.ps1 -Ue4ssDll <path-to\UE4SS.dll>
```

This produces a `PalworldGuider` package directory with a SHA256 manifest. The manifest must be verified before installation.

### Backup the Save

Before adapter installation, back up the game save:

```powershell
.\scripts\Backup-G5Save.ps1
```

The install script refuses to proceed without a verified non-empty backup manifest (`backup-manifest.json`).

### Install

```powershell
$env:PALWORLD_GUIDER_GATEWAY_TOKEN = "a-long-random-bearer-token-0123456789"
.\scripts\Install-G5Ue4ss.ps1 -PackageDirectory <built-package> -ModsDirectory <game-Mods-dir> -BackupDirectory <backup-dir>
```

The install script:
- Refuses to run while Palworld is running.
- Requires a verified save backup before any game-directory write.
- Verifies every package file against the SHA256 manifest.
- Installs only into `Mods/PalworldGuider` using `-LiteralPath`.
- Enables the `PalworldGuider` line in `mods.txt` while preserving all other mod entries.
- Rolls back on failure.

### Start Guide-Server with Adapter

```powershell
$env:GUIDE_PROVIDER = "ollama"
$env:GUIDE_MODEL = "llama3.2"
$env:PALWORLD_GUIDER_GATEWAY_TOKEN = "a-long-random-bearer-token-0123456789"
./target/release/guide-server --data data/reviewed --game-version 1.0.3 --port 8070 --adapter-port 8071 --adapter-token-env PALWORLD_GUIDER_GATEWAY_TOKEN
```

The adapter gateway binds only `127.0.0.1:8071`. The Web server on port 8070 remains fully functional.

### In-Game Chat

Type in the Palworld chat box:

```text
!guide your question here
```

The exact question `!guide ping` returns `Pong: Palworld Guider adapter connected.` without calling the provider. All other questions are routed through the bounded agent loop with the same tool registry, provenance, and uncertainty handling as the Web interface.

### Uninstall

```powershell
.\scripts\Uninstall-G5Ue4ss.ps1 -ModsDirectory <game-Mods-dir>
```

This removes exactly `Mods/PalworldGuider` and its `mods.txt` entry. Other mods are preserved.

### Adapter Safety Boundary

- The adapter is read-only: it reads player position and active-Otomo identity/position only.
- No mutation path exists: no movement, combat, gathering, construction, inventory mutation, or world writes.
- No public server automation. The adapter is a local-client observer for a single-player or explicitly authorized private session.
- The model-visible tool allowlist is fixed at compile time: `get_player_status` and `get_active_pal_status`. The internal `send_chat_message` tool is never callable by the model.
- Both listeners (Web and adapter) bind only `127.0.0.1`.

## Safety Checks

### Network Binding

- The server always binds `127.0.0.1`. It never listens on a public interface.
- The `--host` flag is explicitly rejected at CLI parse time with an error message.
- The adapter gateway also binds only `127.0.0.1`.

### No Mutation Path

No movement, combat, gathering, construction, inventory mutation, or world mutation path exists anywhere in the codebase. The guide is read-only.

### Credential Handling

- Provider credentials (`OPENAI_API_KEY`, `PALWORLD_GUIDER_GATEWAY_TOKEN`) are read from environment variables only.
- Credentials are never written to logs, answer envelopes, or Git.
- The adapter token is validated for length (16..=4096 characters), no surrounding whitespace, and no control characters. Its value is never printed; startup output shows only the variable name.

### Snapshot Redaction

- Snapshot data exposed to the API is redacted to only: `schema_version`, `source_kind`, `game_version`, `freshness`, and `missing_fields`.
- Consent IDs, `captured_at` timestamps, and raw inventory contents are never included in API responses or provider requests.
- The model receives only question-relevant summaries derived from the snapshot, never the raw snapshot itself.

### Request Limits

- Maximum request body size: 65,536 bytes.
- Maximum JSON depth: 32.
- Maximum question length: 2,000 characters.
- Rate limits: 10 asks per minute per session, 3 snapshots per minute per session.
- Maximum concurrent sessions: 32.
- Session TTL: 30 minutes.
- Maximum tool calls per question: 6.
- Maximum reply length: 1,200 characters.
- Agent timeout: 30 seconds (configurable via `--timeout-seconds`, range 1..=300).
