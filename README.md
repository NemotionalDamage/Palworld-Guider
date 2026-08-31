# Palworld Guider

Palworld Guider is a read-only in-game advisor that helps players understand items, materials, Pals, crafting, and progression directly in their play flow.

The project is guide-first and keeps the player in control. It will provide:

- factual explanations from a versioned knowledge base
- state-aware material and goal-gap analysis
- concise next-step recommendations
- an in-game chat interface once the offline guide is reliable

The current repository contains the project charter, reference-data policy, validated Rust knowledge schema, a small reviewed seed dataset, and the Phase G2 deterministic offline CLI.

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

See `PHASES.md` for current progress and `AGENTS.md` for the governing specification.
