# Phase G6: Knowledge Maintenance And Hardening

Date: 2026-09-04

## Goal

Keep the guide trustworthy across game updates and long sessions by adding version compatibility checks, knowledge audit and update workflows, answer regression suites, operations hardening, and comprehensive documentation.

## Scope Delivered

### Task 1+2: Version Checks And Knowledge Audit

New crate: `crates/guide-maintenance/`

- `VersionReport` struct aggregating knowledge version, configured game version, index version, and embedding version (deferred) with `matches` flag and `warnings` vector.
- `version_check(store, configured_game_version, index_version)` function that collects all unique `applicable_game_version` values from the store, determines single/mixed/unknown, compares with configured game version, and optionally checks index and embedding version consistency.
- `KnowledgeAudit` with:
  - `audit_sources(store)` — lists all registered sources with title, version, date, review status, confidence, and per-source fact count.
  - `audit_conflicts(store)` — lists all conflict records with subject, field, values, source IDs, and resolution status.
  - `audit_stale(store, configured_game_version)` — lists all records whose `applicable_game_version` does not match the configured version.
  - `validate_batch(lines)` — deserializes JSONL lines into `KnowledgeRecord`, runs `KnowledgeStore::from_records()` validation without persisting, and returns `BatchValidation` with valid flag, record count, and errors.
- CLI binary `guide-maintenance` with commands: `version-check`, `audit-sources`, `audit-conflicts`, `audit-stale`, `validate-batch`.
- Added `sources()` public iterator to `KnowledgeStore` in `crates/game-knowledge/src/store.rs`.

Tests: 19 (9 version-check, 10 audit) in `tests/version_check.rs` and `tests/audit.rs`.

### Task 3: Answer Regression Suites

New crate: `crates/guide-regression/`

- `tests/lookup_regression.rs` (9 tests): item lookup by name, Chinese name, and ID; Pal lookup with work suitability; recipe lookup; unknown item; version info in lookup answers.
- `tests/calculator_regression.rs` (12 tests): raw and crafted materials, shortage with partial/full inventory, craftable with/without inventory, breeding result and chain, zero-quantity rejection, unknown item, version info in calculations, empty chain for start==target.
- `tests/retrieval_regression.rs` (7 tests): search for "wood", "sphere", nonexistent term; result limit; empty query; provenance in results; version info.
- `tests/grounding_regression.rs` (7 tests): numeric literal fallback, number word fallback, unknown slot fallback, player-facing fallback text without internal markers, empty draft fallback, provider failure, budget exhaustion.
- `tests/version_regression.rs` (7 tests): matching version, mismatched version, None configured, version info in every answer type, version mismatch in calculator and breeding answers, configured version propagation.

Total regression tests: 42.

### Task 4: Log Rotation And Operations Hardening

Modified: `crates/guide-server/`

- New module `crates/guide-server/src/log_rotation.rs` with `LogRotationConfig` (10 MB threshold, 5 rotated files) and `rotate_log_if_needed(path, config)`.
- Rotation renames the current log to `{path}.rotated-{timestamp_ms}`, redacts sensitive patterns (Bearer, OPENAI_API_KEY, PALWORLD_GUIDER_GATEWAY_TOKEN, GUIDE_GATEWAY_TOKEN, sk-) from rotated files, and deletes old rotated files beyond the limit.
- Integrated into `main.rs`: rotation runs at server startup before the adapter attaches the debug log.
- 8 unit tests in `log_rotation.rs`.

### Task 5: Documentation

Six new documentation files:

- `docs/installation.md` — prerequisites, build, data layout, first runs of guide-core, guide-server, and guide-maintenance.
- `docs/configuration.md` — all 8 environment variables, CLI flags for guide-core/guide-server/guide-maintenance, server limits, payload limits, log rotation settings, provider setup.
- `docs/troubleshooting.md` — 11 common issues with symptom/cause/fix.
- `docs/data-updates.md` — knowledge-update workflow, source log format, conflict handling, version tracking, guide-maintenance CLI commands.
- `docs/deployment.md` — web interface deployment, UE4SS adapter deployment, safety checks.
- `docs/operations.md` — performance budgets, log rotation, crash recovery, backup procedure, secret redaction, monitoring.

## Acceptance Gate Results

| Criterion | Result |
|---|---|
| Version drift is visible to the user | Pass — `VersionReport` with warnings; version mismatch propagated to every answer envelope |
| Knowledge changes are auditable | Pass — `audit_sources`, `audit_conflicts`, `audit_stale`, `validate_batch` CLI |
| Regressions protect common facts, recipe calculations, shortages, breeding chains, and NL answers | Pass — 42 regression tests covering lookup, calculation, retrieval, grounding, and version behavior |
| Formatting, lint, tests, and answer regressions pass | Pass — 327 tests, fmt clean, clippy clean, diff clean |

## Verification Commands

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
git diff --check
```

All passed with 327 tests across 13 crates.

## New Crates

| Crate | Purpose | Tests |
|---|---|---|
| `guide-maintenance` | Version checks, knowledge audit, batch validation, CLI | 19 |
| `guide-regression` | End-to-end answer regression suites | 42 |

## Modified Files

- `Cargo.toml` — added `guide-maintenance` and `guide-regression` to workspace members.
- `crates/game-knowledge/src/store.rs` — added `sources()` public iterator.
- `crates/guide-server/src/lib.rs` — added `log_rotation` module and re-exports.
- `crates/guide-server/src/main.rs` — integrated log rotation at startup.

## Blockers

None.
