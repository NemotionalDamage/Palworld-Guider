# Palworld Guider

A read-only, state-aware in-game advisor for Palworld. It answers questions about items, materials, Pals, recipes, breeding, and progression — grounded in a versioned knowledge base and personalized with the player's own state when available — without ever leaving the game.

All phases G0–G6 are complete. The project covers Release Stage 1 (public static-information agent) and Release Stage 2 (single-user dynamic-state guide).

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

### Prerequisites

- **Rust toolchain**: stable 1.98+ (<https://rustup.rs>)
- **OS**: Windows 10/11
- **Shell**: PowerShell 5.1+

### Build

```powershell
git clone git@github.com:NemotionalDamage/Palworld-Guider.git
cd Palworld-Guider
cargo build --release
```

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

### 3. Local Web Guide

```powershell
$env:GUIDE_PROVIDER = "ollama"
$env:GUIDE_MODEL = "llama3.2"
cargo run -p guide-server -- --data data/reviewed --port 8070
```

Open <http://127.0.0.1:8070/> in a browser. The dependency-free UI supports questions, snapshot attachment, provenance display, and follow-up history.

For OpenAI-compatible: also set `$env:OPENAI_API_KEY = "your-key"` and optionally `$env:GUIDE_BASE_URL`.

### 4. In-Game Chat (UE4SS Adapter)

The optional read-only in-game adapter requires a built PalworldGuider package and a running UE4SS-enabled Palworld. See [docs/deployment.md](docs/deployment.md) for full build, backup, install, and uninstall instructions.

Quick start once the adapter is installed:

```powershell
$env:GUIDE_PROVIDER = "ollama"
$env:GUIDE_MODEL = "llama3.2"
$env:PALWORLD_GUIDER_GATEWAY_TOKEN = "a-long-random-bearer-token-0123456789"
cargo run -p guide-server -- --data data/reviewed --port 8070 --adapter-port 8071
```

In the Palworld chat box, type:

```text
!guide What do I need to build a saddle?
!guide ping
```

`!guide ping` returns `Pong: Palworld Guider adapter connected.` without calling the provider.

### 5. Knowledge Maintenance

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
| `PALWORLD_GUIDER_GATEWAY_TOKEN` | `guide-server` (adapter mode) | Bearer token for the loopback adapter gateway (16–4096 chars, never logged) |
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
  installation.md
  configuration.md
  troubleshooting.md
  data-updates.md
  deployment.md
  operations.md
  product-goal.md
  knowledge-source-policy.md
  environment.md
  reference-data/
  schemas/
  phase-records/

scripts/                    # UE4SS build, backup, install, uninstall
  Build-G5Ue4ss.ps1
  Backup-G5Save.ps1
  Install-G5Ue4ss.ps1
  Uninstall-G5Ue4ss.ps1
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
| [docs/deployment.md](docs/deployment.md) | Web and UE4SS adapter deployment, API endpoints, safety checks |
| [docs/troubleshooting.md](docs/troubleshooting.md) | 11 common issues with causes and fixes |
| [docs/data-updates.md](docs/data-updates.md) | Knowledge-update workflow, source log, conflict handling, version tracking |
| [docs/operations.md](docs/operations.md) | Performance budgets, crash recovery, backup, secret redaction, monitoring |
| [docs/product-goal.md](docs/product-goal.md) | Problem statement, core user questions, answer standard, success criteria |
| [PHASES.md](PHASES.md) | Phase progress ledger (G0–G6 all complete) |
| [AGENTS.md](AGENTS.md) | Governing project specification |

## Development Path

| Phase | Status | Purpose |
|---|---|---|
| G0 | Complete | Project charter and reference baseline |
| G1 | Complete | Knowledge schema and reviewed source intake |
| G2 | Complete | Deterministic offline guide CLI |
| G3 | Complete | Hybrid retrieval and grounded LLM guide |
| G4 | Complete | State-aware advisor and progression planner |
| G5 | Complete | Web and read-only in-game interfaces |
| G6 | Complete | Knowledge maintenance and hardening |

## License

This project is a private guide tool. It stores only reviewed, transformed facts and source metadata — no game assets, copyrighted text, or wholesale page dumps. See [docs/knowledge-source-policy.md](docs/knowledge-source-policy.md) for the data and copyright policy.
