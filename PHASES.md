# Palworld Guider Phase Progress

Last updated: 2026-08-31, Asia/Shanghai.

## Current Status

Phase G0 and G1 are complete. G1 established a typed, validated, provenance-bearing JSONL knowledge foundation and passed an audit correction.

Phase G2 is complete after two audit corrections. Deterministic identifier and Unicode-alias resolution, exact lookups with propagated related conflicts, recursive material and shortage calculations, intermediate-inventory-aware craftable counts, ambiguous-name-safe breeding tools, canonical provenance envelopes, and an offline JSON CLI are implemented and tested. No retrieval index, LLM, runtime advisor, game adapter, or dynamic-state source exists.

The G1–G6 roadmap has been refined around a hybrid Rust RAG and deterministic Tool Use architecture: reviewed structured data first, exact Rust calculators second, grounded natural-language retrieval third, state-aware planning fourth, and integrated interfaces last.

The product is split into three delivery stages. Release Stage 1, spanning G0–G3, produces a public static-information agent with no game-process, save, server-API, or adapter access. Release Stage 2 spans G4–G6 and adds explicit, consented dynamic-state sources for one configured user, such as imported snapshots, copied saves, private-server REST APIs, and verified UE4SS reads. Release Stage 3 is deferred until Stage 2 passes and will specify private multi-player identity, consent, authorization, routing, and data isolation.

The older `Pal` project remains separate. Its control-oriented roadmap does not govern this repository.

## Phase Summary

| Phase | Status | Purpose |
|---|---|---|
| G0 | Complete | Establish the independent charter, roadmap, and reference-data rules |
| G1 | Complete | Define knowledge schemas and ingest reviewed sources |
| G2 | Complete | Build the deterministic offline guide CLI |
| G3 | Not started | Add hybrid retrieval and grounded LLM answers |
| G4 | Not started | Add state-aware advice and progression planning |
| G5 | Not started | Build Web and read-only in-game interfaces |
| G6 | Not started | Harden versioning, knowledge maintenance, and operations |

## Release Stage Summary

| Stage | Phases | Status | Purpose |
|---|---|---|---|
| Stage 1: Public Static-Information Agent | G0–G3 | G0–G2 complete, G3 pending | Answer public game-knowledge and calculation questions without a running game |
| Stage 2: Single-User Dynamic-State Guide | G4–G6 | Deferred until Stage 1 passes | Add consented player and world snapshots for one configured user |
| Stage 3: Private Multi-Player Guide | Future phases after G6 | Deferred until Stage 2 passes | Add per-player identity, consent, authorization, routing, and data isolation for private servers |

## Phase G0 Progress

Completed:

- Created an independent project root.
- Defined the guide-first product mission.
- Defined the read-only capability ladder, scope, and guardrails.
- Defined the Rust core architecture boundary.
- Created the knowledge-source policy.
- Created the reference-data source log and intake template.
- Added the project-owner-supplied candidate reference-source catalog to `AGENTS.md`.
- Refined the architecture and G1–G6 roadmap around hybrid Rust RAG and deterministic Tool Use.
- Defined the Stage 1 static-information and Stage 2 explicit dynamic-state release boundary.
- Defined Stage 3 as deferred private multi-player support following the completed single-user dynamic-state guide.
- Cataloged validated sibling `Pal` communication, provider, registry, runtime, and adapter assets as reuse references.

Remaining:

None.

### Verification

Documentation-only phase. Repository structure, source catalog, phase definitions, and whitespace checks passed; Rust checks do not apply yet.

### Blockers

None.

## Phase G1 Progress

Completed:

- Built the Rust workspace and validated `game-knowledge` schema.
- Registered Paldb `v1.0.3` and normalized a small reviewed seed dataset.
- Passed schema, provenance, reference-integrity, conflict, and canonical-data tests after audit correction.

Detailed evidence remains in `docs/phase-records/phase-g1.md`.

## Phase G2 Progress

Completed:

- Defined the Phase G2 test-first implementation plan in `docs/superpowers/plans/2026-08-31-phase-g2.md`.
- Added the `guide-core` library and offline JSON CLI.
- Implemented deterministic IDs, normalized names, Unicode aliases, ambiguity handling, and unknown handling.
- Implemented item, Pal, technology, and recipe lookups with provenance, configured-version warnings, and related-record conflict propagation.
- Implemented recursive material trees, duplicate aggregation, multiple-output scaling, by-products, intermediate-inventory-aware shortages and craftable limits, checked arithmetic, alternative-recipe ambiguity, cycle detection, and depth limits.
- Implemented order-insensitive breeding results, bounded shortest breeding-chain traversal, and explicit ambiguous parent and endpoint candidates.
- Added CLI commands for lookup, recipe, materials, shortage, craftable, breeding, and chain operations.

Remaining:

None.

### Verification

Passed `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, and `git diff --check` after the second audit correction; 32 tests passed. Detailed evidence and data limitations are recorded in `docs/phase-records/phase-g2.md`.

### Blockers

None.

### Next Required Action

Start Phase G3 to add the reviewed-knowledge index, typed tool registry, and grounded natural-language guide.

## Phase Boundaries

- Current scope is the deterministic offline guide core.
- Game integration and adapter work are deferred to the defined later phases.
- Detailed game coverage remains intentionally incomplete until later reviewed intakes.
- The product remains a read-only guide and advisor.
