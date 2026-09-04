# Installation

## Prerequisites

- **Rust toolchain**: stable channel, version 1.98 or later. Install from <https://rustup.rs>.
- **Operating system**: Windows 10 or Windows 11.
- **Shell**: PowerShell 5.1 or later (the project is developed and tested on Windows PowerShell).

Verify the toolchain before proceeding:

```powershell
rustc --version
cargo --version
```

## Building From Source

Clone the repository and build all workspace crates in release mode:

```powershell
git clone git@github.com:NemotionalDamage/Palworld-Guider.git
cd Palworld-Guider
cargo build --release
```

The workspace contains these crates:

| Crate | Purpose |
|---|---|
| `game-knowledge` | Typed schemas, validation, and JSONL store loader |
| `knowledge-index` | In-RAM Tantivy lexical index over reviewed summaries |
| `guide-core` | Deterministic lookup and calculation engine |
| `guide-tools` | Typed tool registry for the agent loop |
| `provider` | OpenAI-compatible and Ollama chat providers |
| `guide-agent` | Bounded agent loop with grounding gate |
| `state-snapshot` | Versioned read-only player-state schema |
| `guide-planner` | State-aware progression planner |
| `guide-server` | Loopback-only Axum Web API and browser UI |
| `game-gateway` | Authenticated loopback WebSocket gateway |
| `guide-adapter` | In-game chat bridge and adapter runtime |
| `guide-maintenance` | Version-check, audit, and batch-validation CLI |
| `guide-regression` | Answer regression suites |

## Data Directory Layout

Reviewed knowledge is stored as versioned, diffable JSONL under `data/reviewed/`. The loader (`KnowledgeStore::load_directory`) reads `sources.jsonl` first (required), then loads each fact file below if it exists.

### Required file

| File | Contents |
|---|---|
| `sources.jsonl` | Registered source records with provenance metadata |

### Core fact files

| File | Contents |
|---|---|
| `items.jsonl` | Item records with use, acquisition leads, and unlock references |
| `pals.jsonl` | Pal records with stats, work suitability, and drops |
| `aliases.jsonl` | Alias records mapping alternative names to canonical IDs |
| `facts.jsonl` | Additional reviewed facts and progression relationships |

### Optional fact files

| File | Contents |
|---|---|
| `technologies.jsonl` | Technology unlock records with requirements |
| `recipes.jsonl` | Recipe records with ingredients, outputs, and by-products |
| `habitats.jsonl` | Pal habitat records with spawn locations and conditions |
| `breeding_rules.jsonl` | Breeding rule records mapping parent pairs to offspring |
| `progression_relationships.jsonl` | Progression graph edges between entities |
| `conflicts.jsonl` | Conflict records for contradictory sources |

Missing optional files are skipped silently. A missing or invalid `sources.jsonl` causes a load error.

## First Run: guide-core CLI

The deterministic CLI works offline without an LLM or live game state. Run from the repository root:

```powershell
cargo run -p guide-core -- lookup item stone
```

Other guide-core commands:

```powershell
cargo run -p guide-core -- recipe "Wooden Club"
cargo run -p guide-core -- materials 3 "Wooden Club"
cargo run -p guide-core -- shortage --inventory "Wood=6" 3 "Wooden Club"
cargo run -p guide-core -- craftable --inventory "Wood=14" "Wooden Club"
cargo run -p guide-core -- breeding Lamball Lamball
cargo run -p guide-core -- chain 4 Lamball Lamball
```

All commands return JSON with `status`, `data`, `provenance`, `version`, `uncertainty`, and `errors`. Exit codes: `0` for `ok`, `1` for `unknown` or `ambiguous`, `2` for `error`.

## First Run: guide-server

The Web guide server requires a provider configured through environment variables.

### Set provider configuration

For an Ollama provider:

```powershell
$env:GUIDE_PROVIDER = "ollama"
$env:GUIDE_MODEL = "llama3.2"
```

For an OpenAI-compatible provider:

```powershell
$env:GUIDE_PROVIDER = "openai"
$env:GUIDE_MODEL = "gpt-4o-mini"
$env:OPENAI_API_KEY = "your-api-key"
```

### Start the server

```powershell
cargo run -p guide-server -- --data data/reviewed --port 8070
```

The server always binds `127.0.0.1` and never listens on a public address.

### Verify the health endpoint

In a separate terminal:

```powershell
curl http://127.0.0.1:8070/health
```

A healthy server returns:

```json
{"status":"ok"}
```

The browser UI is available at <http://127.0.0.1:8070/>.

## guide-maintenance CLI

The maintenance CLI checks version compatibility and audits the knowledge base.

### Version check

Report version compatibility across knowledge, index, and game dimensions:

```powershell
cargo run -p guide-maintenance -- version-check --data data/reviewed --game-version 1.0.3
```

Exit code `0` means no warnings; `1` means version drift or stale records were found.

### Other maintenance commands

```powershell
cargo run -p guide-maintenance -- audit-sources --data data/reviewed
cargo run -p guide-maintenance -- audit-conflicts --data data/reviewed
cargo run -p guide-maintenance -- audit-stale --data data/reviewed --game-version 1.0.3
cargo run -p guide-maintenance -- validate-batch path/to/batch.jsonl
```

- `audit-sources`: lists all registered sources with fact counts.
- `audit-conflicts`: lists all conflicts with resolution status.
- `audit-stale`: lists records whose game version does not match the configured version (requires `--game-version`).
- `validate-batch`: validates JSONL records without persisting them.

## Verification Commands

Before committing changes, run the standard workspace gates:

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```
