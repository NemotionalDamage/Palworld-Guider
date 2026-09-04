# Palworld Guider Phase Progress

Last updated: 2026-09-04, Asia/Shanghai (G6 complete; Pal element-type enrichment in progress).

## Current Status

All phases G0–G6 are complete. Release Stage 1 (public static-information agent) and Release Stage 2 (single-user dynamic-state guide) are complete. Release Stage 3 (private multi-player guide) is deferred until a separate specification is written.

Post-G6 knowledge enrichment: Pal element types (ElementType1/ElementType2) were promoted for all 299 canonical Pals from the local build `DT_PalMonsterParameter`. A new `ElementType` enum, `TypeEffectivenessRecord` schema, and `type_effectiveness.jsonl` data file were added. The type chart (9 super-effective relationships, 2x/0.5x/1x multipliers, dual-type multiplication) was registered from project-owner provision as source `SRC-TYPE-CHART-20260904`. The local-build intake now extracts element types for candidate Pal records. The workspace has 331 tests across 13 crates.

Phase G6 added version compatibility checks, knowledge audit and batch validation CLI, answer regression suites covering lookup, calculation, retrieval, grounding, and version-warning behavior, log rotation with secret redaction for the chat debug log, and comprehensive installation, configuration, troubleshooting, data-updates, deployment, and operations documentation.

Phase G0 is complete. G1 is complete at its original boundary and reopened local-build boundaries. The unified intake standard, completeness audit, first 357-item clear-field promotion, first 298-Pal clear identity/localization promotion, recipe/alias/progression candidate triage, a second full local-build Item batch (1,153 additional Items with rich-text-resolved descriptions and 1,131 matching Simplified Chinese aliases, raising canonical Items to 1,520 and Aliases to 1,805) are complete; six native-row identity duplicates and 22 duplicate-locale aliases were deferred for project-owner resolution, and Pal candidate intake closed with ten unreleased placeholder rows excluded after project-owner confirmation. Broader enrichment and semantic review remain future knowledge maintenance.

Phase G4 is complete. Explicit user-entered read-only snapshots now feed deterministic inventory, party, craftable, goal, progression, and preference-aware planning through typed, redacted tools. No game I/O, save parsing, server API, adapter, or mutation path was added.

Phase G5 is complete. The loopback-only Web interface and approved UE4SS read-only in-game chat path passed their offline and live gates for the recorded local single-player target. Live validation covered visible replies, non-prefixed chat stability, game-restart replacement, guide-server restart and outage recovery, and a manual clean exit with no new crash directory. The validated adapter was uninstalled after the run.

The approved G5 grounding fix passed its offline gate: model drafts use Rust-assigned evidence slots, Rust validates and renders numeric output, and invalid drafts receive deterministic fallback answers.

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
| G5 | Complete | Build Web and read-only in-game interfaces |
| G6 | Complete | Harden versioning, knowledge maintenance, and operations |

## Release Stage Summary

| Stage | Phases | Status | Purpose |
|---|---|---|---|
| Stage 1: Public Static-Information Agent | G0–G3 | Complete | Answer public game-knowledge and calculation questions without a running game |
| Stage 2: Single-User Dynamic-State Guide | G4–G6 | Complete | Add consented player and world snapshots for one configured user |
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
- Expanded the reviewed dataset with six core early-game items, three recipes, Technology Level 2, its Pal Sphere unlock relationship, and localized/common aliases.
- Validated version format, ID charset, conflict shape, alias locale, dates, ranges, references, and provenance drift after two audit corrections.
-- Revalidated the expanded canonical dataset and spot-checked English lookup, Chinese alias lookup, recipe lookup, and cooked-food alias lookup through the offline CLI.

Detailed evidence remains in `docs/phase-records/phase-g1.md`.


### Completed Local-Build Intake And Backfill

The project owner approved reopening G1 for local Palworld Steam Build 24575825 DataTable intake and canonical backfill. Raw exports remain under gitignored `.local/research/local-build`; only reviewed transformed facts and provenance may enter canonical data.

The local-build export gate passed. The preserved manifest contains 518 JSON exports totaling 135,737,060 bytes, with SHA-256 C5850F0EDCE381850526420218F6C0DBC20570BBA97FCE0F23D0978703EFABA7. All 518 files are present and valid JSON; 518/518 matched the manifest, with zero missing. One planned table, DT_SupplyIncident_NPC_Sakura01, legitimately has zero rows. The implementation plan is docs/superpowers/plans/2026-09-02-phase-g1-local-build-intake.md.

The local source and provenance-identity schema are complete. The Rust parser and localization resolver cover Item (2,466 rows), Item Recipe (1,414), Technology Unlock (588), Pal Parameter (753), Pal Drop (1,044), EN/zh-Hans Item Name (1,994 each), Item Description (1,924 each), and Pal Name (322 each). Core-table parse failures are zero; every row has parsed, skipped, or failed coverage. The all-row candidate CLI remains local-only and is deterministic across repeated runs. Its current reviewed-machine audit reports 1,520 item candidates, 915 recipe candidates, 309 Pal candidates, 369 provable unlock relationships, 499 unresolved recipe references, and 4,094 drop-rate units left unresolved. It reports 461 localization rejections, matching the raw tables exactly. These are candidates or explicit skips, not promoted facts.

The field-level canonical backfill gate passed. The Rust audit covers all 38 existing canonical records and 105 field-level facts with zero unclassified facts: 42 exact, 8 partial, 6 conflicting, 37 not represented locally, and 12 unresolved mappings. Mapping success is 75.24%; ambiguous mappings, missing localization, schema failures, and provenance failures are zero. The only explicit disagreements are the two preserved Wooden Club product and recipe conflicts. The report path is restricted to `.local/research/local-build/reports/canonical-backfill.json`, and the final generated report has SHA-256 `ee10be12a962f72da841d7572412fb7c043611da3c966a03069e6dcbe4acccc7`.

### Completed Intake Standard And Completeness Audit

The second reopened G1 subphase added `docs/schemas/knowledge-intake.md`, defining required, conditional, and explicit-unknown fields for Source, Item, Pal, Recipe, Technology, Habitat, Breeding Rule, Alias, Progression Relationship, and Conflict records. The audit covered all 38 canonical facts and all 4,633 local-build candidates.

The canonical set has zero structural-validation failures. Its durable optional-field gaps are seven missing Simplified Chinese names, four missing item descriptions, Lamball stats and habitat coverage, four missing crafting durations, and two missing recipe technology links. The candidate set has complete shared intake fields, but promotion is blocked by item description and acquisition uncertainty, Pal habitat uncertainty, unresolved recipe stations, technology, duration, and byproducts, and 369 progression edges whose Technology endpoints are absent. No unreviewed candidate entered canonical data.

### Item Batch Promotion

Completed:

- Added classified canonical loading for record-type JSONL files.
- Promoted 357 clear local-build Items into `data/reviewed/items.jsonl`: bilingual names, reviewed whitespace-normalized descriptions, native row identity, local evidence, and explicit rarity/acquisition uncertainty.
- Added 357 matching Simplified Chinese aliases into `data/reviewed/aliases.jsonl` so deterministic lookup can resolve new Chinese item names.
- Preserved all existing seed Items and the two explicit Wooden Club conflicts without overwrite.
- Refreshed the local-build audit to 752 canonical records and 1,890 field facts: zero unclassified, schema, provenance, or ambiguity failures; only the six field facts from the two preserved conflicts remain conflicting. Report SHA-256 is `59bb62ea03afd95ffd73e2825b2ed947df96b6d3e31027e40ea08c762ed44ea6`.

Remaining:

None for the accepted clear-field Item batch. It passed the full formatting, Clippy, workspace-test, and whitespace gates. Remaining Item coverage is limited to descriptions with unresolved inline markup, missing descriptions, acquisition enrichment, rarity semantics, and any future changed-build review.

### Pal Batch Promotion

Completed:

- Promoted 298 clear local-build Pals into `data/reviewed/pals.jsonl`: bilingual names, native row identity, local evidence, and explicit stats/work-suitability/drops/habitats uncertainty.
- Added 298 matching Simplified Chinese aliases into `data/reviewed/aliases.jsonl`; canonical Pal coverage is 299 including the existing Lamball record.
- Excluded placeholder-name rows, unresolved placeholder localizations, and the SheepBall native row already represented by Lamball.
- Propagated local-evidence uncertainty into Pal lookup answers and corrected alias auditing to use the Pal localization table for Pal targets.
- Refreshed the local-build audit to 1,348 canonical records and 2,486 field facts with zero unclassified, missing-localization, schema, provenance, or ambiguity failures. Report SHA-256 is `42dfeeb8d801bf1b98eee47ddea8275bb893bf68a4a843bc08dc58001aa67529`.

Remaining:

None for the accepted clear-field Pal batch. It passed the full formatting, Clippy, workspace-test, and whitespace gates. Pal stats, work suitability, drops, and habitats remain explicit unknowns until their semantics are reviewed.
### Unreleased Pal Candidate Closeout

The project owner confirmed the ten remaining local-build Pal rows (placeholder `en_text`/`Unidentified Pal` names with `zh_Hans_Text` placeholders, including Dragostrophe and Boltmane which have real English names only) are unreleased Pals absent from the current game build. None were promoted and no canonical record, alias, or conflict was added; canonical Pal coverage stays 299. Re-review these rows only after a later build ships real names or localization.


### Recipe, Alias, And Progression Batch Triage

Completed:

- Added the only two newly eligible complete aliases: Simplified Chinese `木材` for `ITEM_WOOD` and `羊毛` for `ITEM_WOOL`.
- Preserved the 357 Item and 298 Pal aliases already promoted; rejected `石头` and `烤野莓` local-build aliases because equivalent legacy aliases already exist.
- Promoted zero of 915 recipe candidates because every crafting-station list is `unresolved`; technology, duration, and empty-byproduct semantics also remain unreviewed.
- Promoted zero of 369 progression relationships because every `from_id` references an absent Technology record and current canonical `to_id` resolution also fails.
- Refreshed the audit to 1,350 canonical records and 2,488 field facts; zero unclassified, missing-localization, schema, provenance, or ambiguity failures. Report SHA-256 is `1a7755973077ea281c173e532e6a5b262fe5d944efce7630052b69caa5348767`.

Remaining:

None for the requested complete-record intake. It passed the full formatting, Clippy, workspace-test, and whitespace gates. Deferred recipes, aliases, and progression relationships remain in the local candidate set until their semantic and reference blockers are reviewed.

### Full Reviewed Item Batch

The second full local-build Item batch resolved item-description inline markup against the target-build localization tables (case-insensitive key fallback, rank-suffix item names, Pal/map/skill/uiCommon text, element-icon drops, `/>s` plurals, and the shared generic implant description). It promoted 1,153 Items and 1,131 matching Simplified Chinese aliases, raising canonical Items to 1,520 and Aliases to 1,805 including legacy records, and deferred six native-row identity duplicates (Wooden Club, Red Berries, Lamball Mutton, Gold Coin, Pal Sphere, and Paldium Fragment rows) with their aliases plus 22 duplicate-locale aliases for project-owner resolution. The refreshed audit covers 3,634 canonical records and 8,231 field facts with zero unclassified, missing-localization, schema, provenance, or ambiguity failures; report SHA-256 is `f584c54a2e1048bae45247c5dfbcaf113b6dccef8b63d2cc89251a78087bc872`.

### Blocker

None. FModel now exports with the PalworldModding/UsefulFiles mapping commit 0e4ae19a05ba0d9fb95d859c09b28f168cb3624f; the 1,044-row drop table and 753-row Pal parameter table were validated. SheepBall000 matches the reviewed Lamball drop facts, and SheepBall matches Handcraft, Transport, and MonsterFarm level 1. Continue only the targeted exports and review gate.

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

Complete.

Completed:

- Approved the Web-first integrated-interface architecture.
- Defined the test-first Web milestone plan in `docs/superpowers/plans/2026-09-01-phase-g5-web.md`.
- Approved the UE4SS read-only in-game chat design in `docs/superpowers/specs/2026-09-01-phase-g5-ue4ss-design.md`.
- Completed the test-first UE4SS implementation plan in `docs/superpowers/plans/2026-09-01-phase-g5-ue4ss.md`.
- Added the loopback-only `guide-server` crate with health, session, snapshot, ask, history, cancellation, and browser-UI routes.
- Added bounded in-memory history, rolling ask and snapshot rate limits, payload and JSON-depth limits, redacted snapshot metadata, provider-failure propagation, server deadlines, cancellation, and blocking-pool agent execution.
- Added explicit environment-only provider startup for OpenAI-compatible and Ollama providers and documented the local Web interface.
- Implemented the authenticated schema-2 loopback `game-gateway` crate with 65,536-byte frames, 64-message queue caps, replacement sessions, and cancellation.
- Implemented runtime tool composition, the bounded in-game adapter bridge, narrow x/y/z observation grounding, and the `guide-server` adapter mode with environment-only tokens.
- Implemented the renamed transport-only `PalworldGuider` UE4SS Lua/native adapter (read-only player position and active-Otomo reads, `!guide ` chat capture) and the build/backup/install/uninstall safety scripts.
- Passed the offline acceptance gate: 250 tests, clippy and fmt clean, native package built with verified SHA256 manifest, exports `start_mod`/`uninstall_mod`, and an offline loopback rehearsal with the exact Pong reply and no provider call.
- Implemented evidence-slot answer drafting, strict Rust rendering, and deterministic fallback for numeric grounding.
- Passed the G5 grounding-fix offline gate with formatting, Clippy, full tests, and whitespace checks clean.
- Normalized OpenAI-compatible base URLs to the chat-completions endpoint, added explicit truncation errors, and supported disabling provider reasoning through `GUIDE_DISABLE_REASONING`.
- Added an opt-in, local-only chat debug log (`PALWORLD_GUIDER_CHAT_DEBUG_LOG`) that records questions, replies, statuses, errors, uncertainty, and tool names without entering Git.
- Mapped entity IDs in tool results to canonical display names so multi-word entities such as Lamball Mutton remain renderable evidence instead of triggering the fallback.
- Verified the entity-evidence correction in live single-player chat; both Lamball replays now render normal drafted answers.
- Fixed a live UTF-8 panic in entity-phrase scanning when drafts or aliases contain multi-byte characters, with CJK boundary regression tests.
- Cleaned player-facing fallback text (no internal entity lists, `{v1}`, or draft-error strings), added a small-talk scope rule to the agent prompt, and reduced the adapter tool budget to 6; live replay confirmed zero internal-text leaks and clean grounded unknowns for uncovered fields.
- Completed the approved live read-only gate for world `3C2BA10146F65256FD1B889FBF5F854F`: `!guide ping` was visible across initial connection, game restart/replacement, guide-server restart, and outage recovery; four replies were delivered with no delivery error.
- Verified that a plain non-prefixed chat message does not crash the fixed adapter and that Palworld can exit manually with no new crash directory.
- Stopped the guide server and uninstalled `PalworldGuider` after validation; the older Pal mods remain disabled.

### Verification

The Web milestone passed full-workspace gates with 166 tests; the UE4SS offline milestone passed fresh gates with 250 tests, a verified native build, and a loopback rehearsal. The grounding fix, live-validation corrections, and final live completion passed fresh formatting, Clippy, full-test, and whitespace gates where applicable. Detailed evidence is in `docs/phase-records/phase-g5.md`.

### Blockers

None. Knowledge coverage gaps are a G6 data-maintenance backlog, not a G5 interface-safety blocker.

## Phase G6 Progress

Completed:

- Added the `guide-maintenance` crate with `VersionReport`, `VersionDimension`, `KnowledgeAudit`, `SourceSummary`, `ConflictSummary`, `StaleRecord`, and `BatchValidation` types.
- Implemented version compatibility checks across game, knowledge, index, and embedding dimensions with mismatch, mixed-version, and unknown-version warnings.
- Implemented knowledge audit functions: `audit_sources` (all registered sources with fact counts), `audit_conflicts` (all conflicts with resolution status), `audit_stale` (records whose game version does not match), and `validate_batch` (JSONL validation without persisting).
- Added the `guide-maintenance` CLI with commands: `version-check`, `audit-sources`, `audit-conflicts`, `audit-stale`, `validate-batch`.
- Added the `guide-regression` crate with 42 end-to-end regression tests covering lookup accuracy, calculation correctness, retrieval hit rate, hallucination rate (grounding gate), and version-warning behavior.
- Added log rotation with secret redaction to `guide-server`: 10 MB file threshold, 5 rotated files, redaction of Bearer tokens, API keys, and gateway tokens on rotation.
- Added documentation: `docs/installation.md`, `docs/configuration.md`, `docs/troubleshooting.md`, `docs/data-updates.md`, `docs/deployment.md`, `docs/operations.md`.
- Added `sources()` iterator to `KnowledgeStore` for public source auditing.

Remaining:

None.

### Verification

`cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` with 327 tests, and `git diff --check` all passed. Detailed evidence remains in `docs/phase-records/phase-g6.md`.

### Blockers

None.

## Phase Boundaries

- Release Stage 1 (G0–G3) and Release Stage 2 (G4–G6) are complete.
- Release Stage 3 (private multi-player guide) is deferred until a separate specification is written.
- Current scope is the explicit, read-only single-user state-aware guide: imported user state, deterministic planning, redacted model summaries, and bounded LLM synthesis.
- Game integration, adapter work, save parsing, server APIs, and all runtime game I/O remain deferred to the defined later phases.
- Detailed game coverage remains intentionally incomplete until later reviewed intakes.
- The product remains a read-only guide and advisor.
