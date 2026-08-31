# Palworld Guider Phase Progress

Last updated: 2026-09-01, Asia/Shanghai.

## Current Status

Phase G0 and G1 are complete. G1 established a typed, validated, provenance-bearing JSONL knowledge foundation and passed an audit correction.

Phase G2 is complete after a fifth audit correction. Byproduct item provenance and conflicts now propagate into material calculations, item lookup exposes byproduct acquisition relationships, shortage and craftable calculations offset requirements with same-item and cross-item byproducts in an ingredient-order-independent way, credit recipe byproducts only after the craft completes, and apply a first-batch bootstrap seed when a byproduct is also an ingredient of the same recipe. Related-record conflicts propagate through Pal, technology, and recipe lookups. The deterministic offline guide core passes its acceptance gates.

The G1–G6 roadmap has been refined around a hybrid Rust RAG and deterministic Tool Use architecture: reviewed structured data first, exact Rust calculators second, grounded natural-language retrieval third, state-aware planning fourth, and integrated interfaces last.

Phase G3 requires a fifth audit correction. The bounded Rust agent loop, typed registry, deterministic calculators, Tantivy retrieval, provider adapters, provenance, and offline tests are present, and the fourth-correction regressions pass. Remaining bypasses let a numeric claim reuse a different entity's same-value result in the same sentence, and let an explicit unknown answer assert a permitted parent as the offspring. Semantic vector search remains deferred.

The product is split into three delivery stages. Release Stage 1, spanning G0–G3, produces a public static-information agent with no game-process, save, server-API, or adapter access. Release Stage 2 spans G4–G6 and adds explicit, consented dynamic-state sources for one configured user, such as imported snapshots, copied saves, private-server REST APIs, and verified UE4SS reads. Release Stage 3 is deferred until Stage 2 passes and will specify private multi-player identity, consent, authorization, routing, and data isolation.

The older `Pal` project remains separate. Its control-oriented roadmap does not govern this repository.

## Phase Summary

| Phase | Status | Purpose |
|---|---|---|
| G0 | Complete | Establish the independent charter, roadmap, and reference-data rules |
| G1 | Complete | Define knowledge schemas and ingest reviewed sources |
| G2 | Complete | Build the deterministic offline guide CLI |
| G3 | Audit correction required | Add hybrid retrieval and grounded LLM answers |
| G4 | Not started | Add state-aware advice and progression planning |
| G5 | Not started | Build Web and read-only in-game interfaces |
| G6 | Not started | Harden versioning, knowledge maintenance, and operations |

## Release Stage Summary

| Stage | Phases | Status | Purpose |
|---|---|---|---|
| Stage 1: Public Static-Information Agent | G0–G3 | G0–G2 complete; G3 requires audit correction | Answer public game-knowledge and calculation questions without a running game |
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
- Expanded the final-answer grounding gate to parse complete scaled, fractional, scientific-notation, and question-locale Chinese quantity expressions, pair numeric values with their exact result entities, and bind breeding conclusions or explicit unknowns to the requested parent pair and successful result; a fifth audit found same-sentence value reuse and permitted-entity offspring assertions.
- Kept exact and structured lookup ahead of lexical retrieval and deferred semantic vector search.

### Verification

`cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` with 104 tests, and `git diff --check` all passed during the fifth audit. Two additional temporary acceptance regressions reproduced grounding bypasses and were removed after their failure output was captured. Detailed evidence remains in `docs/phase-records/phase-g3.md`.

### Blockers

The numeric grounding check accepts a claim when any same-value result leaf appears in the same sentence, so a value belonging to one entity can authorize a fabricated quantity for another entity in that sentence (for example, Wood's available `5` authorizing `short 5 Wool`). The breeding unknown allowlist is word-level: a permitted parent entity in offspring-assertion position (`Unknown result: the offspring is Lamball.`) passes even though no successful breeding result exists. These fail the requirement that model-generated arithmetic and breeding guesses never become user-visible facts.

Next required action: bind each numeric claim to the entity the claim actually names (co-located phrase), and restrict unknown-breeding answers to uncertainty language without offspring assertions, adding permanent regressions for both bypasses.

## Phase Boundaries

- Current scope is the grounded static-information guide: lexical retrieval, typed tool use, and bounded LLM synthesis over reviewed knowledge.
- Game integration and adapter work are deferred to the defined later phases.
- Detailed game coverage remains intentionally incomplete until later reviewed intakes.
- The product remains a read-only guide and advisor.
