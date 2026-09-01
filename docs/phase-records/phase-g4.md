# Phase G4 Record

## Scope Delivered

- Added the `state-snapshot` crate with strict `state_snapshot_v1` deserialization, source and evidence classification, consent metadata, bounded quantities and levels, duplicate checks, future-time checks, missing-section reporting, and deterministic error collection.
- Added inclusive freshness evaluation, TTL handling, completeness reporting, canonical ordering, and redacted summaries that omit consent IDs, consent objects, capture timestamps, and grant timestamps.
- Added the `guide-planner` crate. It combines an explicit snapshot with `GuideEngine` to analyze craft-goal inventory shortages, party work coverage and gaps, craftable-now recipes, unlock requirements, and reviewed technology-to-recipe progression relationships.
- Added deterministic recommendation ranking over relevance, effort, benefit, risk, and uncertainty. Results are bounded to three through five steps when sufficient evidence exists and fall back to three general state-collection steps when state is missing.
- Added preference-gated long-horizon advice. Minimal or no-spoiler settings expose only the immediate next unlock; mechanics or progression settings permit one broader technology-stage review.
- Extended the typed tool registry with `import_player_snapshot`, `analyze_inventory`, `analyze_party`, and `suggest_next_goals`. Snapshots are attached in Rust from explicit JSON; no snapshot argument is published to the model.
- Kept `import_player_snapshot` metadata-only after a final boundary audit. It returns source kind, game version, freshness, and missing fields; inventory and party details remain behind purpose-specific analysis tools.
- Preserved provenance, knowledge versions, uncertainty, and errors through planner envelopes and agent answers. Provider requests never receive raw snapshots, consent IDs, consent objects, or capture metadata.
- Preserved the dynamic-state boundary: no save parser, server API, UE4SS adapter, file watcher, game-process reader, runtime game I/O, or mutation path exists.

## Red-Green Evidence

- Snapshot validation first failed because `state_snapshot` and its public types did not exist; five validation tests then passed after implementation.
- Snapshot summaries first failed because freshness, completeness, and summary APIs did not exist; four summary tests then passed.
- Planner tests first failed because `guide_planner` did not exist. Seven initial behavior tests covered inventory, party, craftability, readiness, determinism, stale state, stale knowledge, adversarial goals, and fallback.
- Progression-goal analysis first failed because non-craft goals were skipped; reviewed unlock relationships then propagated into readiness and provenance.
- Preference-gated lookahead first failed because `long_horizon` and spoiler level were not consulted; the dedicated preference regression then passed.
- State-tool integration first failed because `with_state_snapshot_json` did not exist; seventeen registry tests and the agent raw-boundary regression then passed.
- A final audit found that `import_player_snapshot` returned an all-field redacted summary. The failing regression forced it to return only question-neutral metadata, keeping state details behind purpose-specific tools.

## Acceptance Mapping

- Deterministic recommendations: repeated planner calls compare equal and stable score/ID ordering is tested.
- Reason and requirement basis: every recommendation test checks reason, requirements, alternatives, benefit, knowledge IDs, and state basis.
- Supplied and missing state: shortage, party, craftable, unlock, and completeness tests distinguish supplied state from unknown fields.
- Missing-state degradation: three bounded general guidance steps are returned with explicit uncertainty.
- Spoiler and long-horizon control: minimal, progression, and disabled lookahead variants are tested.
- Dynamic sources off by default: no snapshot is attached unless Rust code explicitly supplies one.
- Model boundary: provider requests are serialized in tests and checked for consent IDs, consent objects, and capture metadata; state details come only from typed tool envelopes.
- No runtime game I/O or mutation: implementation and tests contain no game process, save reader, server API, adapter, or mutation path.

## Verification

Fresh final gates:

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test` with 136 tests
- `git diff --check`

All passed after the final snapshot-import boundary correction.

## Durable Uncertainty

- Only explicit user-entered JSON snapshots are supported. Copied-save parsing remains unsupported and requires a separate explicit opt-in and validation design.
- The reviewed seed dataset remains intentionally small, so planner breadth is limited to the reviewed entities and relationships.
- Provider validation remains offline through the scripted mock. Real OpenAI-compatible and Ollama endpoints were not exercised.
- No real game, save file, server API, UE4SS adapter, or dynamic player state was used.
- There is no end-user state import interface yet; the stable Rust registry API is the G4 boundary, with Web and interface work deferred to G5.
