# Palworld Guider Phase Progress

Last updated: 2026-09-01, Asia/Shanghai.

## Current Status

Phase G0 and G1 are complete. G1 established a typed, validated, provenance-bearing JSONL knowledge foundation and passed an audit correction.

Phase G4 is complete. Explicit user-entered read-only snapshots now feed deterministic inventory, party, craftable, goal, progression, and preference-aware planning through typed, redacted tools. No game I/O, save parsing, server API, adapter, or mutation path was added.

Phase G5 is in progress. The approved architecture starts with a loopback-only Axum Web API and minimal browser UI that reuse the existing Rust guide core, preserve bounded in-memory sessions, and expose provenance and uncertainty. Runtime private-server REST, UE4SS, and in-game chat adapters remain blocked until the Web path is stable and the user records the exact live target constraints.

Phase G2 is complete after a fifth audit correction. Byproduct item provenance and conflicts now propagate into material calculations, item lookup exposes byproduct acquisition relationships, shortage and craftable calculations offset requirements with same-item and cross-item byproducts in an ingredient-order-independent way, credit recipe byproducts only after the craft completes, and apply a first-batch bootstrap seed when a byproduct is also an ingredient of the same recipe. Related-record conflicts propagate through Pal, technology, and recipe lookups. The deterministic offline guide core passes its acceptance gates.

The G1–G6 roadmap has been refined around a hybrid Rust RAG and deterministic Tool Use architecture: reviewed structured data first, exact Rust calculators second, grounded natural-language retrieval third, state-aware planning fourth, and integrated interfaces last.

Phase G3 is complete after a seventh audit correction. The bounded Rust agent loop, typed registry, deterministic calculators, Tantivy retrieval, provider adapters, provenance, and offline tests are present, and the sixth-correction regressions pass. Numeric claims bind to the entity phrase immediately following the number through a structural adjacency check with no connector-word list; explicit unknown breeding answers allow permitted parent entities only as context, never in result-assertion position, including bare, copula-less, and keyword-preceded offspring forms in English and Chinese. Semantic vector search remains deferred.

The product is split into three delivery stages. Release Stage 1, spanning G0–G3, produces a public static-information agent with no game-process, save, server-API, or adapter access. Release Stage 2 spans G4–G6 and adds explicit, consented dynamic-state sources for one configured user, such as imported snapshots, copied saves, private-server REST APIs, and verified UE4SS reads. Release Stage 3 is deferred until Stage 2 passes and will specify private multi-player identity, consent, authorization, routing, and data isolation.

The older `Pal` project remains separate. Its control-oriented roadmap does not govern this repository.

## Phase Summary

| Phase | Status | Purpose |
|---|---|---|
| G0 | Complete | Establish the independent charter, roadmap, and reference-data rules |
| G1 | Complete | Define knowledge schemas and ingest reviewed sources |
| G2 | Complete | Build the deterministic offline guide CLI |
| G3 | Complete | Add hybrid retrieval and grounded LLM answers |
| G4 | Complete | Add state-aware advice and progression planning |
| G5 | In progress | Build Web and read-only in-game interfaces |
| G6 | Not started | Harden versioning, knowledge maintenance, and operations |

## Release Stage Summary

| Stage | Phases | Status | Purpose |
|---|---|---|---|
| Stage 1: Public Static-Information Agent | G0–G3 | Complete | Answer public game-knowledge and calculation questions without a running game |
| Stage 2: Single-User Dynamic-State Guide | G4–G6 | In progress; G4 complete, G5 next | Add consented player and world snapshots for one configured user |
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
- Validated version format, ID charset, conflict shape, alias locale, dates, ranges, references, and provenance drift after two audit corrections.
- Passed schema, provenance, reference-integrity, conflict, and canonical-data tests with nine `game-knowledge` tests.

Detailed evidence remains in `docs/phase-records/phase-g1.md`.

## Phase G2 Progress

Completed:

- Defined the Phase G2 test-first implementation plan in `docs/superpowers/plans/2026-08-31-phase-g2.md`.
- Added the `guide-core` library and offline JSON CLI.
- Implemented deterministic IDs, normalized names, Unicode aliases, ambiguity handling, and unknown handling.
- Implemented item, Pal, technology, and recipe lookups with provenance, configured-version warnings, and related-record conflict propagation.
- Implemented recursive material trees, duplicate aggregation, multiple-output scaling, by-product totals, intermediate-inventory-aware shortages and craftable limits, checked arithmetic, alternative-recipe ambiguity, cycle detection, and depth limits.
- Implemented order-insensitive breeding results, bounded shortest breeding-chain traversal, and explicit ambiguous parent and endpoint candidates.
- Added CLI commands for lookup, recipe, materials, shortage, craftable, breeding, and chain operations.
- Propagated byproduct item provenance and conflict subjects into material calculations.
- Exposed byproduct acquisition relationships in item lookup.
- Offset shortage and craftable requirements with same-item and cross-item byproduct production.
- Made cross-item byproduct offsets order-independent for shortage and craftable calculations.
- Credited recipe byproducts after ingredients are satisfied and applied a first-batch seed for self-consuming byproducts.
- Propagated related-record conflicts through Pal, technology, and recipe lookups.

Remaining:

None.

### Verification

`cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` with 40 tests, and `git diff --check` all passed after the fifth audit correction.

### Blockers

None.

## Phase G3 Progress

Completed:

- Defined the Phase G3 test-first implementation plan in `docs/superpowers/plans/2026-08-31-phase-g3.md`.
- Added the `knowledge-index` crate with a Tantivy lexical index over deterministic structured summaries, aliases, provenance, versions, and unknown/stale envelopes.
- Added the `guide-tools` crate with twelve typed tools, JSON schemas, argument validation, standard envelopes, budgets, and deadlines.
- Added the `provider` crate with the `ChatProvider` contract, OpenAI-compatible and Ollama adapters, timeouts, an error taxonomy, and a scripted mock.
- Added the `guide-agent` crate with a bounded grounded loop, cancellation, reply limits, provenance propagation, and an `ask` CLI.
- Expanded the final-answer grounding gate to parse complete scaled, fractional, scientific-notation, and question-locale Chinese quantity expressions, pair numeric values with their exact result entities, bind breeding conclusions or explicit unknowns to the requested parent pair and successful result, bind numeric claims to the entity phrase immediately following the number (structural adjacency, independent of connector-word lists), and restrict explicit unknown breeding answers to uncertainty language with permitted parents only as context; a seventh audit replaced the keyword-anchored assertion check with a context-position rule.
- Kept exact and structured lookup ahead of lexical retrieval and deferred semantic vector search.

### Verification

`cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` with 115 tests, and `git diff --check` all passed after the seventh audit correction. Detailed evidence remains in `docs/phase-records/phase-g3.md`.

### Blockers

None.

## Phase G4 Progress

Completed:

- Defined and approved the G4 architecture in `docs/superpowers/specs/2026-09-01-phase-g4-design.md`.
- Defined the test-first implementation plan in `docs/superpowers/plans/2026-09-01-phase-g4.md`.
- Added the strict `state_snapshot_v1` schema with source, consent, evidence, state, goal, and preference validation.
- Added inclusive freshness evaluation, completeness reporting, deterministic field ordering, and question-relevant summaries that omit consent and raw source metadata.
- Added deterministic inventory-gap, party-work-gap, craftable-now, craft- and progression-goal readiness analyses.
- Added stable relevance, effort, benefit, risk, and uncertainty ranking for three to five explained recommendations, with missing-state fallback and reviewed unlock-relationship propagation.
- Added preference-gated long-horizon and spoiler-sensitive lookahead advice.
- Added typed snapshot confirmation, inventory, party, and next-goal tools with explicit Rust-side attachment, redacted summaries, provenance, version, uncertainty, and errors.
- Verified the bounded agent request path never serializes consent IDs, consent objects, or raw capture metadata into provider requests.

Verification:

`cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` with 136 tests, and `git diff --check` all passed after the final import-boundary correction. Detailed evidence remains in `docs/phase-records/phase-g4.md`.

Remaining:

None.

### Blockers

None.

## Phase G5 Progress

Completed:

- Approved the Web-first integrated-interface architecture.
- Defined the test-first Web milestone plan in `docs/superpowers/plans/2026-09-01-phase-g5-web.md`.

Remaining:

- Implement the Web path test-first and pass offline verification.
- Record the exact Palworld target version, platform, load mode, ownership, backup, and adapter constraints.
- Add only the explicitly configured read-only retrieval and thin in-game chat paths validated for that target.

### Verification

Pending.

### Blockers

- The live adapter target definition has not yet been supplied; no runtime game, private-server REST, or UE4SS integration may begin before it is recorded.

## Phase Boundaries

- Current scope is the explicit, read-only single-user state-aware guide: imported user state, deterministic planning, redacted model summaries, and bounded LLM synthesis.
- Game integration, adapter work, save parsing, server APIs, and all runtime game I/O remain deferred to the defined later phases.
- Detailed game coverage remains intentionally incomplete until later reviewed intakes.
- The product remains a read-only guide and advisor.
