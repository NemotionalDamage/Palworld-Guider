# Palworld Guider Phase Progress

Last updated: 2026-08-30, Asia/Shanghai.

## Current Status

Phase G0 is complete. This repository is established as an independent guide-first project. The governing documents, source policy, and reference-intake baseline are in place. No Rust implementation, game adapter, or detailed game dataset exists yet.

Phase G1 is in progress. The phase is limited to the Rust workspace, `game-knowledge` schema and validation, reviewed JSONL source intake, provenance propagation, conflict representation, and canonical-dataset tests. No CLI, retrieval, LLM, runtime advisor, game adapter, or dynamic-state source is in scope.

The G1–G6 roadmap has been refined around a hybrid Rust RAG and deterministic Tool Use architecture: reviewed structured data first, exact Rust calculators second, grounded natural-language retrieval third, state-aware planning fourth, and integrated interfaces last.

The product is split into three delivery stages. Release Stage 1, spanning G0–G3, produces a public static-information agent with no game-process, save, server-API, or adapter access. Release Stage 2 spans G4–G6 and adds explicit, consented dynamic-state sources for one configured user, such as imported snapshots, copied saves, private-server REST APIs, and verified UE4SS reads. Release Stage 3 is deferred until Stage 2 passes and will specify private multi-player identity, consent, authorization, routing, and data isolation.

The older `Pal` project remains separate. Its control-oriented roadmap does not govern this repository.

## Phase Summary

| Phase | Status | Purpose |
|---|---|---|
| G0 | Complete | Establish the independent charter, roadmap, and reference-data rules |
| G1 | In progress | Define knowledge schemas and ingest reviewed sources |
| G2 | Not started | Build the deterministic offline guide CLI |
| G3 | Not started | Add hybrid retrieval and grounded LLM answers |
| G4 | Not started | Add state-aware advice and progression planning |
| G5 | Not started | Build Web and read-only in-game interfaces |
| G6 | Not started | Harden versioning, knowledge maintenance, and operations |

## Release Stage Summary

| Stage | Phases | Status | Purpose |
|---|---|---|---|
| Stage 1: Public Static-Information Agent | G0–G3 | G0 complete, G1–G3 pending | Answer public game-knowledge and calculation questions without a running game |
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

- Selected Paldb `v1.0.3` as the first reviewed-secondary source for a deliberately small seed dataset.
- Defined the Phase G1 test-first implementation plan in `docs/superpowers/plans/2026-08-31-phase-g1.md`.

Remaining:

- Implement the Rust workspace, `game-knowledge` crate, validation, and tests.
- Register the selected source and normalize the reviewed seed records.
- Run formatting, lint, test, and canonical-dataset gates.

### Blockers

None.

## Phase Boundaries

- Current scope remains documentation and project definition.
- Game integration and adapter work are deferred to the defined later phases.
- Game facts await reviewed source intake.
- The product remains a read-only guide and advisor.
