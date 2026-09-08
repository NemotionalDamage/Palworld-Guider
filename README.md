# Palworld Guider

A read-only, state-aware in-game advisor for Palworld. It answers questions about items, materials, Pals, recipes, breeding, and progression — grounded in a versioned knowledge base and personalized with the player's own state when available — without ever leaving the game.

The public Web guide, offline CLI tools, and a read-only in-game MOD are implemented. The in-game MOD targets Palworld Steam build `24575825` (game `1.0.3`) and is officially supported there. Steam build `25094871` is still listed in `blocked_game_build_ids` until the `full` capability probe passes, but the adapter transport and the `!guide ping` chat path were verified live on `25094871` (2026-09-08) after a one-time UE4SS 3.0.1 settings fix - see the manual-start flow under [In-Game MOD](#in-game-mod-recommended).

## Key Features

- **Deterministic knowledge engine** — every game fact comes from reviewed, versioned, source-tracked JSONL data, never from LLM memory. The knowledge base covers 1,520 items, 299 Pals, and 1,805 bilingual aliases extracted from the local Palworld Steam build.
- **Hybrid Rust RAG + deterministic tool use** — the agent loop combines exact identifier resolution, structured lookup, Tantivy lexical retrieval, and typed Rust calculators behind a bounded LLM provider loop. The model phrases answers but can never invent game facts or fabricate quantities.
- **Grounded answer enforcement** — numeric quantities, breeding results, and entity claims in model drafts are validated against tool evidence through structural adjacency checks. Invalid drafts receive deterministic fallback answers instead of fabricated content.
- **State-aware planning** — optional user-entered player-state snapshots enable inventory-gap analysis, party-work-gap detection, craftable-now checks, and prioritized next-step recommendations (3–5 ranked steps with reasons, requirements, and uncertainty).
- **In-game chat interface** — an optional read-only UE4SS adapter delivers `!guide` questions from the Palworld chat box to the guide server and returns concise replies, all over an authenticated loopback WebSocket.
- **Version and conflict awareness** — version mismatches, stale records, and conflicting sources are surfaced explicitly rather than silently resolved. The maintenance CLI audits sources, conflicts, and staleness.
- **Strict read-only boundary** — no movement, combat, gathering, construction, inventory mutation, or world mutation path exists anywhere in the codebase. The Web server and adapter gateway bind only `127.0.0.1`.

## Architecture

```text
                        ┌─────────────┐
  Player question ──── │ guide-agent │ ──── Grounded answer
        or !guide chat  │ (bounded   │      with provenance,
                         │  LLM loop) │      uncertainty, tools
                        └──────┬──────┘
                               │ typed tool calls
                    ┌──────────┼──────────┐
                    ▼          ▼          ▼
             ┌──────────┐ ┌─────────┐ ┌────────────┐
             │guide-core│ │knowledge│ │guide-planner│
             │(lookup + │ │-index   │ │(state-aware│
             │ calc)    │ │(Tantivy)│ │ planning)  │
             └────┬─────┘ └────┬────┘ └─────┬──────┘
                  │            │             │
                  ▼            ▼             ▼
             ┌──────────────────────────────────┐
             │         game-knowledge            │
             │  (JSONL store + validation)      │
             └──────────────────────────────────┘
                          │
                          ▼
                   data/reviewed/*.jsonl
             (1,520 items, 299 Pals, 1,805 aliases)
```

### Workspace Crates

| Crate | Purpose |
|---|---|
| `game-knowledge` | Typed schemas, JSONL store loader, validation, local-build intake CLI |
| `knowledge-index` | In-RAM Tantivy lexical index over reviewed summaries |
| `guide-core` | Deterministic lookup, recipe, materials, shortage, and breeding calculators |
| `guide-tools` | Typed tool registry with JSON schemas, budgets, and result envelopes |
| `provider` | OpenAI-compatible and Ollama chat providers with timeouts and error taxonomy |
| `guide-agent` | Bounded agent loop with grounding gate, cancellation, and reply limits |
| `state-snapshot` | Versioned read-only `PlayerStateSnapshot` schema with freshness and completeness |
| `guide-planner` | State-aware progression planner with ranked next-step recommendations |
| `guide-server` | Loopback-only Axum Web API, browser UI, and optional adapter gateway |
| `game-gateway` | Authenticated schema-2 loopback WebSocket gateway for the UE4SS adapter |
| `guide-adapter` | In-game chat bridge and adapter runtime with tool composition |
| `guide-maintenance` | Version-check, knowledge audit, and batch-validation CLI |
| `guide-regression` | 42 end-to-end regression tests covering lookup, calculation, retrieval, grounding, and version warnings |

## Quick Start

### In-Game MOD (Recommended)

The read-only in-game MOD is the recommended path: clone the repository,
build once, then run one setup command and one start command.

```powershell
git clone https://github.com/NemotionalDamage/Palworld-Guider.git
cd Palworld-Guider
cargo build --release
.\scripts\Setup-InGameGuide.ps1
.\scripts\Start-InGameGuide.ps1
```

#### Prerequisites

- Windows 10/11 x64 with PowerShell 5.1+
- Palworld Steam build `24575825` (game `1.0.3`) - the supported target.
  Steam build `25094871` is still in `blocked_game_build_ids`, and the
  setup/start scripts refuse it; a verified manual flow for it is
  documented below
- UE4SS `3.0.1` installed at `<game>\Pal\Binaries\Win64\UE4SS.dll`; the
  pinned DLL hash is verified during preflight
- Rust stable 1.98+ (<https://rustup.rs>)
- VS 2022 Build Tools with the Desktop development with C++ workload
  (CMake and Ninja included)
- Provider environment variables in the shell that runs
  `Start-InGameGuide.ps1`: `GUIDE_PROVIDER` (`openai` or `ollama`),
  `GUIDE_MODEL`, and `OPENAI_API_KEY` when `GUIDE_PROVIDER=openai`

#### Setup

`Setup-InGameGuide.ps1` runs the read-only preflight first (Steam
manifest and Palworld build, UE4SS DLL hash, newest `Level.sav`), refuses
an existing `Mods\PalworldGuider` (run `Uninstall-Ue4ssAdapter.ps1`
first), creates a verified save backup under `.local\backups\palworld`,
builds and stages the package under `.local\build`, verifies the staged
hash manifest, and only then installs into the game directory. Optional
`-GameRoot`, `-Ue4ssDll`, `-SaveDirectory`, and `-OutputDirectory`
override discovery; `-WhatIf` reports the planned stages without
changing anything. On a blocked or unsupported build, setup fails closed
with exit code `1` before backing up, building, or writing.

#### Start

`Start-InGameGuide.ps1` revalidates the installed package, generates the
gateway token in memory, and binds it to the launching PowerShell session
(never to a file, the registry, or the command line). It forwards provider
environment values only to the child process, starts the loopback guide
server, and relaunches Palworld through Steam from that same session so the
game process inherits the token. Run it with Palworld closed. It prints the
Web interface and adapter gateway addresses and never prints the gateway
token or provider credentials.

#### In-Game Usage

Palworld opens automatically in the Steam client that
`Start-InGameGuide.ps1` launched; enter a single-player or private session
and type in the chat box:

```text
!guide ping
!guide <question>
```

`!guide ping` returns `Pong: Palworld Guider adapter connected.` without
calling the provider; `!guide <question>` routes through the same
grounded agent loop as the Web interface. Relaunching the game later from
the same Steam window keeps the session token; fully exiting Steam and
re-running `Start-InGameGuide.ps1` rotates it.

#### Manual Start On A Blocked Build (verified on Steam build 25094871)

`Setup-InGameGuide.ps1` and `Start-InGameGuide.ps1` refuse builds that
are still listed in `blocked_game_build_ids` (today: Steam build
`25094871`) - that refusal is intentional and fails closed. The sequence
below is the developer flow that was verified live on 2026-09-08 on Steam
build `25094871` with UE4SS `3.0.1` (save
`3C2BA10146F65256FD1B889FBF5F854F`): the game loads the save without
crashing, and `!guide ping` returns the fixed pong message through the
live gateway.

**One-time UE4SS crash fix.** UE4SS 3.0.1 crashes this Palworld build
when its world-load hooks are enabled (heap corruption `0xc0000374` on
save entry, reproduced even with PalworldGuider disabled). Edit
`<game>\Pal\Binaries\Win64\UE4SS-settings.ini` and set:

```ini
[Hooks]
HookInitGameState = 0
HookCallFunctionByNameWithArguments = 0
HookBeginPlay  = 0
HookLocalPlayerExec = 0
```

Keep `HookProcessInternal = 1` and `HookProcessLocalScriptFunction = 1`
(the Guider adapter and its chat hook
`/Script/Pal.PalGameStateInGame:BroadcastChatMessage` do not use the
disabled hooks). Optionally set `GuiConsoleEnabled = 0`. The change
affects every UE4SS mod: only re-enable the hooks if another mod on this
build requires them.

**Then run these steps from one PowerShell session at the repository
root** (the same session must launch the game so it inherits the gateway
token):

1. Build the release server and the verified adapter package:

   ```powershell
   cargo build --release
   .\scripts\Build-Ue4ssAdapter.ps1 -Ue4ssDll "<game>\Pal\Binaries\Win64\UE4SS.dll" -Capability chat -OutputDirectory "$PWD\.local\build\ue4ss-adapter"
   ```

   `chat` is the capability verified on this build (transport plus
   `!guide ping`). The default `full` capability additionally enables
   the player and active-Otomo reads and remains the player-facing build
   for the supported Steam build `24575825`; the `full` layer probe on
   `25094871` is pending, so use `chat` here until it passes.

2. Back up the newest save before any game-directory write:

   ```powershell
   .\scripts\Backup-PalworldSave.ps1 -SourceDirectory "$env:LOCALAPPDATA\Pal\Saved\SaveGames\76561198694570145\3C2BA10146F65256FD1B889FBF5F854F" -DestinationDirectory "$PWD\.local\backups\palworld\manual-start"
   ```

   The Steam account id and world id differ per machine; the source is
   the newest directory containing `Level.sav`. Setup normally discovers
   it, but on a blocked build the scripts stop before discovery, so pass
   it explicitly.

3. Install and enable the package (`PalworldGuider : 1` is added to
   `mods.txt`):

   ```powershell
   .\scripts\Install-Ue4ssAdapter.ps1 -PackageDirectory "$PWD\.local\build\ue4ss-adapter\PalworldGuider" -ModsDirectory "<game>\Pal\Binaries\Win64\Mods" -BackupDirectory "$PWD\.local\backups\palworld\manual-start"
   ```

4. Configure the provider and bind a fresh gateway token to this
   session:

   ```powershell
   $env:GUIDE_PROVIDER = "ollama"        # or "openai"
   $env:GUIDE_MODEL = "llama3.2"
   # when GUIDE_PROVIDER=openai also set: $env:OPENAI_API_KEY = "your-key"
   # optional: $env:GUIDE_BASE_URL = "https://..."
   $env:PALWORLD_GUIDER_GATEWAY_TOKEN = [guid]::NewGuid().ToString("N") + [guid]::NewGuid().ToString("N")
   $env:PALWORLD_GUIDER_GATEWAY_PORT = "8071"
   ```

5. Start the loopback guide server from this session and confirm the
   adapter listener is up:

   ```powershell
   $server = Start-Process -FilePath "$PWD\target\release\guide-server.exe" -WorkingDirectory $PWD -WindowStyle Hidden -ArgumentList @("--data", "$PWD\data\reviewed", "--game-version", "1.0.3", "--port", "8070", "--adapter-port", "8071", "--adapter-token-env", "PALWORLD_GUIDER_GATEWAY_TOKEN") -PassThru
   Start-Sleep -Seconds 2
   Get-NetTCPConnection -LocalPort 8071 -State Listen -ErrorAction SilentlyContinue
   ```

   The server prints `Palworld Guider listening on http://127.0.0.1:8070`
   and `adapter=127.0.0.1:8071`; it never prints the token.

6. Close Steam if it is running, then launch Palworld through Steam from
   this same session so the game process inherits the token and port:

   ```powershell
   Get-Process steam -ErrorAction SilentlyContinue | ForEach-Object { $null = $_.CloseMainWindow() }
   Start-Sleep -Seconds 5
   Start-Process "<Steam install>\Steam.exe" -ArgumentList "-applaunch", "1623730"
   ```

7. Enter a single-player or private save, open the chat box, and send:

   ```text
   !guide ping
   ```

   The game chat returns `Pong: Palworld Guider adapter connected.` The
   gateway log confirms the exchange: `hello`, `capability_manifest`,
   `heartbeat`, `event`, `tool_call` (`send_chat_message`), and
   `tool_result` frames are accepted.

8. When done, exit Palworld, stop the guide server, and either keep the
   package enabled for the next run or uninstall it:

   ```powershell
   Stop-Process -Name guide-server -Force -ErrorAction SilentlyContinue
   .\scripts\Uninstall-Ue4ssAdapter.ps1 -ModsDirectory "<game>\Pal\Binaries\Win64\Mods"
   ```

**Scope:** on `25094871`, layers 1 (noop) and 2 (chat) are verified; the
`full` capability layer (`!guide <question>` with player and Otomo reads)
is not yet probed, and the manifest keeps `25094871` blocked until it
passes. Follow the probe record in
[docs/mod-rollout-plan.md](docs/mod-rollout-plan.md).

#### Uninstall And Unsupported Builds

Close Palworld and remove only the MOD:

```powershell
.\scripts\Uninstall-Ue4ssAdapter.ps1 -ModsDirectory "<game>\Pal\Binaries\Win64\Mods"
```

The uninstaller removes `Mods\PalworldGuider` and its `mods.txt` line and
preserves every other mod and UE4SS itself. If Palworld is on a build
outside the reviewed support matrix (Steam build `25094871` is still in
`blocked_game_build_ids`), preflight and setup fail closed before any
change. The scripts never force the MOD onto an unsupported build; the
manual flow above documents how such a build is verified before it can
be unblocked. See [docs/deployment.md](docs/deployment.md) for full
details.

### 1. Deterministic Offline CLI (no LLM)

Works completely offline with no provider or game state:

```powershell
cargo run -p guide-core -- lookup item Wood
cargo run -p guide-core -- recipe "Wooden Club"
cargo run -p guide-core -- materials 3 "Wooden Club"
cargo run -p guide-core -- shortage --inventory "Wood=6" 3 "Wooden Club"
cargo run -p guide-core -- craftable --inventory "Wood=14" "Wooden Club"
cargo run -p guide-core -- breeding Lamball Lamball
cargo run -p guide-core -- chain 4 Lamball Lamball
```

All commands return JSON with `status`, `data`, `provenance`, `version`, `uncertainty`, and `errors`. Exit codes: `0` = ok, `1` = unknown/ambiguous, `2` = error.

### 2. Natural-Language Guide (with LLM)

```powershell
# Ollama (local)
cargo run -p guide-agent -- ask "How do I get Wood?" --provider ollama --model llama3.2

# OpenAI-compatible
$env:OPENAI_API_KEY = "your-key"
cargo run -p guide-agent -- ask "Materials for 3 Wooden Clubs?" --provider openai --model gpt-4o-mini
```

### 3. Web-Only Guide (No MOD)

```powershell
$env:GUIDE_PROVIDER = "ollama"
$env:GUIDE_MODEL = "llama3.2"
cargo run -p guide-server -- --data data/reviewed --port 8070
```

Open <http://127.0.0.1:8070/> in a browser. The dependency-free UI supports questions, snapshot attachment, provenance display, and follow-up history.

For OpenAI-compatible: also set `$env:OPENAI_API_KEY = "your-key"` and optionally `$env:GUIDE_BASE_URL`.

### 4. Knowledge Maintenance

```powershell
cargo run -p guide-maintenance -- version-check --data data/reviewed --game-version 1.0.3
cargo run -p guide-maintenance -- audit-sources --data data/reviewed
cargo run -p guide-maintenance -- audit-conflicts --data data/reviewed
cargo run -p guide-maintenance -- audit-stale --data data/reviewed --game-version 1.0.3
cargo run -p guide-maintenance -- validate-batch path/to/candidates.jsonl
```

## Configuration

All configuration is environment-only. No credentials appear on the command line or in Git.

| Variable | Used by | Description |
|---|---|---|
| `GUIDE_PROVIDER` | `guide-server` | `openai` or `ollama` (required) |
| `GUIDE_MODEL` | `guide-server` | Model name, e.g. `gpt-4o-mini` or `llama3.2` (required) |
| `OPENAI_API_KEY` | `guide-server`, `guide-agent` | API key for OpenAI-compatible providers (read from env only, never logged) |
| `GUIDE_BASE_URL` | `guide-server` | Overrides the provider endpoint |
| `GUIDE_DISABLE_REASONING` | `guide-server`, `guide-agent` | Set to `1` to suppress reasoning tokens |
| `OLLAMA_BASE_URL` | `guide-agent` | Overrides the Ollama endpoint (defaults to `http://localhost:11434`) |
| `PALWORLD_GUIDER_GATEWAY_TOKEN` | `guide-server`, in-game adapter | Bearer token for the loopback adapter gateway (16–4096 chars, never logged); `Start-InGameGuide.ps1` binds it to the launching session |
| `PALWORLD_GUIDER_GATEWAY_PORT` | in-game adapter | Loopback gateway port read by the adapter; `Start-InGameGuide.ps1` sets it to `-AdapterPort` (default `8071`) |
| `PALWORLD_GUIDER_CHAT_DEBUG_LOG` | `guide-server` (optional) | JSONL debug log path under a gitignored directory |

See [docs/configuration.md](docs/configuration.md) for full CLI flags, server limits, payload limits, and log rotation details.

## Project Structure

```text
adapter/
  read-only/ue4ss/          # UE4SS Lua/C++ hybrid adapter (read-only)

crates/                     # 13 Rust workspace crates
  game-knowledge/           # Schemas, JSONL store, validation, intake CLI
  knowledge-index/          # Tantivy lexical index
  guide-core/               # Deterministic lookup & calculation CLI
  guide-tools/              # Typed tool registry
  provider/                 # OpenAI-compatible & Ollama providers
  guide-agent/              # Bounded agent loop with grounding gate
  state-snapshot/           # Player-state schema
  guide-planner/            # State-aware progression planner
  guide-server/             # Loopback Axum Web API + browser UI
  game-gateway/             # Authenticated loopback WebSocket gateway
  guide-adapter/            # In-game chat bridge
  guide-maintenance/        # Version-check & audit CLI
  guide-regression/         # Answer regression suites (42 tests)

data/
  reviewed/                 # Canonical reviewed JSONL knowledge base
    sources.jsonl           #   3 registered sources
    items.jsonl              #   1,520 items
    pals.jsonl               #   299 Pals
    aliases.jsonl            #   1,805 bilingual aliases
    facts.jsonl              #   38 seed facts & progression edges

docs/                       # Full project documentation
  mod-rollout-plan.md
  installation.md
  configuration.md
  troubleshooting.md
  data-updates.md
  deployment.md
  operations.md
  knowledge-source-policy.md
  reference-data/
  schemas/

scripts/                  # UE4SS preflight, build, backup, install, setup, start, uninstall
  Test-InGameGuide.ps1
  Build-Ue4ssAdapter.ps1
  Backup-PalworldSave.ps1
  Install-Ue4ssAdapter.ps1
  Uninstall-Ue4ssAdapter.ps1
  Setup-InGameGuide.ps1
  Start-InGameGuide.ps1
```

## Knowledge Base

The knowledge base is the single source of truth for game facts. It is:

- **Reviewed** — every fact is extracted from the local Palworld Steam build (build 24575825) using FModel with the PalworldModding/UsefulFiles mapping, then field-level reviewed before promotion.
- **Versioned** — every record carries `applicable_game_version`, source ID, extraction date, review status, and confidence.
- **Provenance-tracked** — sources are registered in `docs/reference-data/source-log.md` with priority ordering: local target-build data > official docs > reviewed secondary > community.
- **Conflict-visible** — disagreements between sources are recorded as `ConflictRecord` entries, never silently resolved.
- **Diffable** — stored as JSONL for easy review and Git diffing.

Source priority:

| Priority | Source | Confidence |
|---|---|---|
| 1 | Local target-build data | `verified-target` |
| 2 | Official patch notes/docs | `official` |
| 3 | User-reviewed secondary | `reviewed-secondary` |
| 4 | Community wikis/databases | `community` |
| 5 | LLM-derived hypotheses | never persisted as facts |

See [docs/data-updates.md](docs/data-updates.md) for the full knowledge-update workflow.

## Testing

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

The workspace has 327 tests across 13 crates, including:

- Schema, reference-integrity, version, alias, and conflict tests
- Calculator tests for recipe trees, shortages, craftable counts, breeding, and cycle detection
- Mock-provider agent loop and grounding-gate regression tests
- State-snapshot validation and planner ranking tests
- 42 end-to-end regression tests for lookup accuracy, calculation correctness, retrieval hit rate, hallucination rate, and version-warning behavior

## Safety and Read-Only Boundary

- The Web server and adapter gateway bind **only** `127.0.0.1`. The `--host` flag is explicitly rejected.
- The adapter reads only player position and active-Otomo identity/position. No inventory, party, health, stamina, save, or nearby-actor reads.
- The model-visible tool allowlist is fixed at compile time: `get_player_status` and `get_active_pal_status`. `send_chat_message` is internal and never callable by the model.
- Snapshot data exposed to the API is redacted to schema version, source kind, game version, freshness, and missing fields. Raw snapshots never reach provider prompts.
- No mutation path exists: no movement, combat, gathering, construction, inventory mutation, or world writes.

## Documentation

| Document | Content |
|---|---|
| [docs/installation.md](docs/installation.md) | Build, data layout, first run, verification commands |
| [docs/configuration.md](docs/configuration.md) | Environment variables, CLI flags, server/payload limits, log rotation |
| [docs/deployment.md](docs/deployment.md) | In-game MOD clone-to-game path, then Web-only deployment, API endpoints, safety checks |
| [docs/troubleshooting.md](docs/troubleshooting.md) | 12 common issues with causes and fixes |
| [docs/data-updates.md](docs/data-updates.md) | Knowledge-update workflow, source log, conflict handling, version tracking |
| [docs/operations.md](docs/operations.md) | Performance budgets, crash recovery, backup, secret redaction, monitoring |
| [docs/mod-rollout-plan.md](docs/mod-rollout-plan.md) | Level 1 clone-to-game and Level 2 installer rollout plan |

## Adapter Validation Status

The reviewed knowledge base and the adapter package target Palworld
`1.0.3` / Steam build `24575825` (the `game_build_ids` entry). On
2026-09-08 the adapter transport was verified live on Steam build
`25094871` with UE4SS `3.0.1` after the world-load-hook fix above:

- Layer 1 (`noop`) passed: the save loads and stays stable, and the
  gateway accepts the adapter `hello`, `capability_manifest`, and
  heartbeat frames.
- Layer 2 (`chat`) passed: `!guide ping` returns
  `Pong: Palworld Guider adapter connected.` through the live gateway
  (`event` to `send_chat_message` to `tool_result`).
- Layer 3 (`full`) is not yet probed on `25094871`, so that build remains
  in `blocked_game_build_ids` and `Setup-InGameGuide.ps1` /
  `Start-InGameGuide.ps1` still refuse it before any state change.

Until the `full` layer passes on `25094871`, use the `chat` capability
build (verified above) or Steam build `24575825` for the full
player-facing experience. The game itself remains usable without
PalworldGuider installed; the uninstaller removes it cleanly.

## License

This project is a private guide tool. It stores only reviewed, transformed facts and source metadata — no game assets, copyrighted text, or wholesale page dumps. See [docs/knowledge-source-policy.md](docs/knowledge-source-policy.md) for the data and copyright policy.
