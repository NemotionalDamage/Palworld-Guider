# Palworld Guider

Palworld Guider is a read-only in-game advisor that helps players understand items, materials, Pals, crafting, and progression directly in their play flow.

The project is guide-first and keeps the player in control. It will provide:

- factual explanations from a versioned knowledge base
- state-aware material and goal-gap analysis
- concise next-step recommendations
- an in-game chat interface once the offline guide is reliable

The current repository contains the project charter, reference-data policy, validated Rust knowledge schema, a small reviewed seed dataset, the Phase G2 deterministic offline CLI, the Phase G3 grounded natural-language guide, and the Phase G4 state-aware planner.

## Development Path

1. **G0** — project charter and reference baseline
2. **G1** — knowledge schema and reviewed source intake
3. **G2** — deterministic offline guide CLI
4. **G3** — grounded natural-language guide
5. **G4** — state-aware advisor and progression planner
6. **G5** — Web and read-only in-game interfaces
7. **G6** — knowledge maintenance and hardening

## Offline CLI

Run from the repository root with the default reviewed dataset:

```powershell
cargo run -p guide-core -- lookup item Wood
cargo run -p guide-core -- materials 3 "Wooden Club"
cargo run -p guide-core -- shortage --inventory "Wood=6" 3 "Wooden Club"
cargo run -p guide-core -- craftable --inventory "Wood=14" "Wooden Club"
cargo run -p guide-core -- breeding Lamball Lamball
cargo run -p guide-core -- chain 4 Lamball Lamball
```

All commands return JSON with status, data, provenance, version, uncertainty, and errors. They run offline and never use an LLM or live game state.

## Natural-Language Guide

The G3 agent combines exact structured lookup, Tantivy lexical retrieval, and typed deterministic tools behind a bounded provider loop:

```powershell
cargo run -p guide-agent -- ask "How do I get Wood?" --provider ollama --model llama3.2
$env:OPENAI_API_KEY = "..."; cargo run -p guide-agent -- ask "Materials for 3 Wooden Clubs?" --provider openai --model gpt-4o-mini
```

Provider configuration:

- `openai` requires the `OPENAI_API_KEY` environment variable; `--base-url` can override the chat-completions endpoint.
- `ollama` uses `OLLAMA_BASE_URL` or defaults to `http://localhost:11434`.
- `--max-tool-calls` (default 6) and `--timeout-seconds` (default 60) bound every run.

Every answer records tool calls, provenance, knowledge versions, uncertainty, and errors. The model sees only registry-published tools, deterministic Rust functions are the intended source for calculations, and final quantities and breeding claims require exact claim-relevant tool evidence. Tests use a scripted mock provider and stay offline.

## State-Aware Advisor

G4 supports explicit user-entered JSON snapshots attached directly in Rust:

```rust
let registry = ToolRegistry::new(engine, index)
    .with_state_snapshot_json(&snapshot_json)?;
```

The snapshot is optional and off by default. It is validated for schema version, source, consent metadata, freshness, evidence kind, quantities, party slots, goals, and preferences before planner tools expose redacted inventory-gap, party-work, and next-goal results. The model never receives the raw snapshot; `import_player_snapshot` returns only source, version, freshness, and missing-field metadata.

There is no save parser, server API, game-process reader, UE4SS adapter, file watcher, or mutation path in this phase.

See `PHASES.md` for current progress and `AGENTS.md` for the governing specification.
