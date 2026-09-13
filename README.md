# Palworld Guider

> 中文版 README：[README.zh-CN.md](README.zh-CN.md)

A read-only, state-aware in-game advisor for Palworld. It answers questions about items, materials, Pals, recipes, breeding, and progression — grounded in a versioned knowledge base and personalized with the player's own state when available — without ever leaving the game.

The public Web guide, offline CLI tools, and a read-only in-game MOD are implemented. Players can ask questions in the Palworld chat box with `!guide` and get concise, evidence-grounded answers without ever leaving the game.

## Key Features

- **Deterministic knowledge engine** — every game fact comes from reviewed, versioned, source-tracked JSONL data, never from LLM memory. The knowledge base covers 1,883 items, 298 Pals, 1,286 recipes, 1,789 bilingual aliases, 123 world-map regions (120 of them with reviewed trigger-volume boundaries), and 67,952 Pal habitat zones extracted from the local Palworld Steam build.
- **Breeding by formula, not by table** — the tens of thousands of possible parent pairs are compressed into the game's own rule, a per-Pal combi rank plus a small reviewed exception table for special combinations and self-only Pals, so breeding answers stay exact without a combinatorial data dump.
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
             (1,883 items, 298 Pals, 1,286 recipes)
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
| `guide-regression` | 50 end-to-end regression tests covering lookup, calculation, retrieval, grounding, and version warnings |

## Quick Start

Two ways to use the same grounded engine:

- **In-game MOD** - full experience: ask `!guide` questions in the Palworld chat box.
- **Web guide only** - the same answers in a browser; no MOD and no game needed.

### In-Game MOD (full experience)

The read-only in-game MOD answers `!guide` questions from the Palworld
chat box. Everything is automatic: the scripts locate the Steam
installation no matter which drive or folder the game lives in, find the
UE4SS `Mods` directory and the newest save, verify hashes, and never
write to the game directory until a verified save backup exists.

```powershell
git clone https://github.com/NemotionalDamage/Palworld-Guider.git
cd Palworld-Guider
cargo build --release
.\scripts\Setup-InGameGuide.ps1
.\scripts\Start-InGameGuide.ps1
```

#### Prerequisites

- Windows 10/11 x64 with PowerShell 5.1+.
- Palworld installed through Steam. The game folder may be on any drive;
  it is discovered from the Steam libraries automatically. You need a
  single-player or private session for the in-game chat box.
- UE4SS `3.0.1` installed into the game: unzip the UE4SS release into
  `<game>\Pal\Binaries\Win64` so that `UE4SS.dll` sits next to
  `Palworld-Win64-Shipping.exe` (the standard UE4SS install folder).
  Preflight verifies the DLL hash before anything runs.
- Rust stable 1.98+ (<https://rustup.rs>).
- VS 2022 Build Tools with the "Desktop development with C++" workload
  (CMake and Ninja included).
- Provider environment variables in the PowerShell session that runs
  `Start-InGameGuide.ps1` - see [Configuration](#configuration) below.

#### Setup

`Setup-InGameGuide.ps1` is one command and needs no paths: the game
root, the UE4SS DLL, and the newest save are auto-discovered (optional
`-GameRoot`, `-Ue4ssDll`, `-SaveDirectory`, and `-OutputDirectory` only
override discovery). It runs a read-only preflight first, refuses to
continue if `Mods\PalworldGuider` already exists (run the uninstaller
first), creates a verified backup of the newest save under
`.local\backups\palworld`, builds and stages the MOD package, verifies
the staged hash manifest, and only then installs `Mods\PalworldGuider`
and enables it in `mods.txt`, preserving every other mod. `-WhatIf`
reports the planned stages without changing anything. Any preflight
failure stops the command before a single file is written.

#### Configure The Provider (once)

Run the interactive setup once; the API key prompt hides what you type:

```powershell
.\scripts\Set-GuideProvider.ps1
```

The settings are saved to `.local\set-provider.ps1` (excluded from Git), and
`Start-InGameGuide.ps1` loads them automatically from then on. Re-run the
script with `-Force` to change them. You can still set `GUIDE_PROVIDER`,
`GUIDE_MODEL`, `GUIDE_BASE_URL`, and `OPENAI_API_KEY` in a session instead;
session values override the saved file. With `GUIDE_PROVIDER=ollama`, Ollama
must be running (`ollama serve`) and the model named in `GUIDE_MODEL` must
be pulled (`ollama list`).

#### Start

Close Palworld if it is running, then run:

```powershell
.\scripts\Start-InGameGuide.ps1
```

`Start-InGameGuide.ps1` revalidates the installed package and hash
manifest, generates a random gateway token in memory and binds it to
this PowerShell session (never to a file, the registry, or the command
line), starts the loopback guide server, closes a stale Steam client,
and relaunches Palworld through Steam from the same session so the game
process inherits the token. It prints the loopback addresses and never
prints the token:

```text
Web interface: http://127.0.0.1:8070/
Adapter gateway: 127.0.0.1:8071
```

Change the loopback ports with `-Port` and `-AdapterPort` if needed.

#### In-Game Usage

Enter a single-player or private session and type in the chat box:

```text
!guide ping
!guide <question>
!guide retry
!guide new
```

- `!guide ping` returns `Pong: Palworld Guider adapter connected.`
  without calling the provider.
- `!guide <question>` goes through the same grounded agent loop as the
  Web interface and answers items, materials, Pals, recipes, breeding,
  and progression questions with cited knowledge-base evidence.
- Every question starts with clean context by default; only questions that
  explicitly refer to the previous answer (继续/刚才/continue/again) carry
  the latest exchange as follow-up context.
- `!guide retry` clears the context and re-asks your previous question.
- `!guide new` clears the conversation without re-asking anything.

Relaunching the game later from the same Steam window keeps the session
token. If Steam was fully closed, re-run `Start-InGameGuide.ps1` so the
game inherits a fresh token.

If the game crashes while entering a save with UE4SS enabled (heap
corruption `0xc0000374`), UE4SS needs its world-load hooks disabled as
described in [docs/troubleshooting.md](docs/troubleshooting.md) entry
12; the adapter does not use those hooks.

#### Uninstall

Close Palworld and remove only the MOD:

```powershell
.\scripts\Uninstall-Ue4ssAdapter.ps1 -ModsDirectory "<game>\Pal\Binaries\Win64\Mods"
```

`<game>` is the Palworld installation folder shown by Steam (Manage >
Browse local files). The uninstaller removes `Mods\PalworldGuider` and
its `mods.txt` line and preserves every other mod and UE4SS itself.

### Web Guide Only (no MOD, no game)

If you only want to try the Web experience, you do not need UE4SS, the
MOD, or the game running. Clone, build, and start the loopback server:

```powershell
git clone https://github.com/NemotionalDamage/Palworld-Guider.git
cd Palworld-Guider
cargo build --release

.\scripts\Set-GuideProvider.ps1
.\scripts\Start-WebGuide.ps1
```

The provider setup above is the same one used by the MOD, so you only ever
configure it once per clone.

Open <http://127.0.0.1:8070/> in a browser. The Web guide shares the
same grounded engine as the MOD: ask the same questions, attach a
player-state snapshot for state-aware planning, inspect the provenance
behind every answer, and follow up in one thread. Stop the server with
Ctrl+C in the terminal.

## Command-Line Tools

### Deterministic Offline CLI (no LLM)

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

### Natural-Language Guide (with LLM)

```powershell
# Ollama (local)
cargo run -p guide-agent -- ask "How do I get Wood?" --provider ollama --model llama3.2

# OpenAI-compatible
$env:OPENAI_API_KEY = "your-key"
cargo run -p guide-agent -- ask "Materials for 3 Wooden Clubs?" --provider openai --model gpt-4o-mini
```

### Knowledge Maintenance

```powershell
cargo run -p guide-maintenance -- version-check --data data/reviewed --game-version 1.0
cargo run -p guide-maintenance -- audit-sources --data data/reviewed
cargo run -p guide-maintenance -- audit-conflicts --data data/reviewed
cargo run -p guide-maintenance -- audit-stale --data data/reviewed --game-version 1.0
cargo run -p guide-maintenance -- validate-batch path/to/candidates.jsonl
```

## Configuration

All configuration is environment-only. `Set-GuideProvider.ps1` stores the provider values in the gitignored `.local\set-provider.ps1`, so no credentials appear on the command line or in Git.

Both start scripts wait up to 300 seconds for a model response. If your model is consistently faster, pass a smaller value such as `-ProviderTimeoutSeconds 120`.

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
  guide-regression/         # Answer regression suites (50 tests)

data/
  reviewed/                 # Canonical reviewed JSONL knowledge base
    sources.jsonl           #   15 registered sources
    items.jsonl             #   1,883 items
    pals.jsonl              #   298 Pals
    recipes.jsonl           #   1,286 recipes
    breeding_rules.jsonl    #   248 special breeding combinations
    map_regions.jsonl       #   123 world-map regions (120 with boundaries)
    aliases.jsonl           #   1,789 bilingual aliases
    facts.jsonl             #   32 seed facts & progression edges

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
  build-breeding-data.mjs #  Regenerate the reviewed breeding formula and rules
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

- **Reviewed** — every fact is extracted from the local Palworld Steam build using FModel with the PalworldModding/UsefulFiles mapping, then field-level reviewed before promotion.
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

The workspace has 432 tests across 13 crates, including:

- Schema, reference-integrity, version, alias, and conflict tests
- Calculator tests for recipe trees, shortages, craftable counts, breeding, and cycle detection
- Mock-provider agent loop and grounding-gate regression tests
- State-snapshot validation and planner ranking tests
- 42 end-to-end regression tests for lookup accuracy, calculation correctness, retrieval hit rate, hallucination rate, and version-warning behavior

## Safety and Read-Only Boundary

- The Web server and adapter gateway bind **only** `127.0.0.1`. The `--host` flag is explicitly rejected.
- The adapter reads only player position, active-Otomo identity/position, and player base-camp positions. No inventory, party, health, stamina, save, or nearby-actor reads.
- The model-visible tool allowlist is fixed at compile time: `get_player_status`, `get_active_pal_status`, and `get_base_camps`. `send_chat_message` is internal and never callable by the model.
- Snapshot data exposed to the API is redacted to schema version, source kind, game version, freshness, and missing fields. Raw snapshots never reach provider prompts.
- No mutation path exists: no movement, combat, gathering, construction, inventory mutation, or world writes.

## Documentation

| Document | Content |
|---|---|
| [docs/installation.md](docs/installation.md) | Build, data layout, first run, verification commands |
| [docs/configuration.md](docs/configuration.md) | Environment variables, CLI flags, server/payload limits, log rotation |
| [docs/deployment.md](docs/deployment.md) | In-game MOD setup/start details, Web-only deployment, API endpoints, safety checks |
| [docs/troubleshooting.md](docs/troubleshooting.md) | 12 common issues with causes and fixes |
| [docs/data-updates.md](docs/data-updates.md) | Knowledge-update workflow, source log, conflict handling, version tracking |
| [docs/operations.md](docs/operations.md) | Performance budgets, crash recovery, backup, secret redaction, monitoring |
| [docs/mod-rollout-plan.md](docs/mod-rollout-plan.md) | Rollout plan for the in-game MOD source path and the installer release path |

## License

This project is a private guide tool. It stores only reviewed, transformed facts and source metadata — no game assets, copyrighted text, or wholesale page dumps. See [docs/knowledge-source-policy.md](docs/knowledge-source-policy.md) for the data and copyright policy.
