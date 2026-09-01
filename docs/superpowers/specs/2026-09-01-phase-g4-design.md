# Phase G4 State-Aware Advisor Design

## Goal

Phase G4 starts Release Stage 2 by combining reviewed knowledge with an explicitly supplied, read-only player snapshot and player preferences to produce deterministic prioritized advice.

## Architecture

The new `state-snapshot` crate owns the versioned snapshot format, strict validation, source classification, freshness evaluation, completeness reporting, and question-relevant redaction. It does not read files automatically and does not parse game saves.

The new `guide-planner` crate owns deterministic state analysis and progression planning. It consumes a validated snapshot plus the existing `GuideEngine`, reuses the existing material, shortage, and craftable calculators, and emits provenance-bearing inventory-gap, party-work-gap, craftable-now, and goal-readiness results. Recommendations are ranked with a stable score over relevance, effort, benefit, risk, and uncertainty, then capped to three through five steps.

`guide-tools` and `guide-agent` receive planner-backed typed tools. The snapshot is supplied directly to Rust through explicit configuration and is never serialized into the model prompt. Model-visible results contain only redacted, question-relevant summaries, planner findings, provenance, version warnings, conflicts, and uncertainty.

## Snapshot Boundary

The first supported source is `user_entered`. It may arrive from an explicit JSON string or explicit imported JSON value. Every field remains optional at the envelope level, while populated collections and preferences are strictly validated. The snapshot records missing top-level sections rather than fabricating them.

Copied-save parsing remains unsupported in G4 unless separately specified, explicitly enabled, and validated. There is no server API, UE4SS adapter, file watcher, game-process access, runtime game I/O, or world mutation.

## Advice Behavior

The planner distinguishes observed, user-entered, save-derived, assumed, and unknown evidence in its output. Missing inventory, party, unlock, roster, level, goal, or preference data degrades to explicit general guidance. Adversarial or unknown goals are rejected as unknown rather than matched loosely. Spoiler exposure and long-horizon planning are controlled by preferences. Conflicting preferences produce ambiguity instead of a silent choice.

## Verification

Tests must cover valid mock state, invalid state, stale snapshots, freshness boundaries, completeness, source distinction, redaction, deterministic ranking, inventory gaps, party work gaps, craftable-now results, goal readiness, adversarial goals, stale knowledge, conflicting preferences, and missing-state fallback. Final gates remain formatting, Clippy, all tests, and whitespace checks.
