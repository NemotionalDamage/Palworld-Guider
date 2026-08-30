# Phase G1 Record

## Scope

Phase G1 established the reviewed, versioned, offline knowledge foundation:

- Rust workspace and `game-knowledge` crate
- typed schemas for sources, provenance, items, recipes, technologies, Pals, work suitability, drops, habitats, breeding rules, aliases, progression relationships, and conflicts
- canonical JSONL storage under `data/reviewed/`
- source registration and provenance propagation
- identifier, locale, date, URL, range, reference-integrity, duplicate, and confidence validation
- explicit conflict records rather than silent selection
- canonical-dataset and validation regression tests

No CLI, retrieval index, LLM, provider, adapter, server, save reader, or dynamic-state source was added.

## Source Review

### SRC-PALDB-V1_0_3-20260831

- Evidence: `https://paldb.cc/Wood`, `https://paldb.cc/Wooden_Club`, and `https://paldb.cc/Lamball`
- Retrieved: 2026-08-31
- Applicable version claimed by source: 1.0.3
- Review result: reviewed-secondary
- Normalized scope:
  - Wood is obtained by chopping trees.
  - Wooden Club outputs one item from five Wood at a Primitive Workbench.
  - Wooden Club is tied to Technology Level 1.
  - Lamball has Handiwork, Transporting, and Farming suitability at level 1.
  - Lamball can drop 1–3 Wool or one Lamball Mutton at 100%.

The source is fan-maintained. Only directly reviewed, transformed facts were stored. The site footer identified version 1.0.3 dated 2026-08-12.

## Durable Uncertainty

- The seed facts have not been cross-checked against a second reviewed source.
- Paldb is a community database and may lag behind a Palworld patch; each persisted fact carries a change-risk warning.
- No habitat, breeding rule, localized alias, Pal stats, or item-use claim was promoted from this source.
- The current dataset is intentionally small and is not a complete game encyclopedia.

## Verification

- Red test 1: `cargo test` initially failed because the typed schema API did not exist.
- Red test 2: `cargo test --test canonical_dataset` initially failed because `data/reviewed/sources.jsonl` did not exist.
- Final format gate: `cargo-fmt --all -- --check` passed.
- Final lint gate: `cargo-clippy --all-targets -- -D warnings` passed.
- Post-audit test gate: `cargo test` passed with eight tests across schema validation and the canonical dataset.

## Audit Correction

An accepted audit reopened the phase because the initial validator allowed missing ingredient references, wrong-type references through a global fact-ID fallback, impossible calendar dates, and incomplete recipe fields. The source log also retained a stale “no sources” sentence.

The correction adds regressions for missing recipe ingredient references, recipe output pointing to a non-item fact, empty ingredients or crafting stations, negative crafting time, impossible dates, and provenance drift from the registered source. Typed references now resolve only to their required record type; generic alias, progression, and conflict references use a separate any-fact check. Recipe ingredients, stations, and present crafting time are validated. Dates use real Gregorian calendar validation, and the stale source-log sentence was removed.

No checks were intentionally skipped.
