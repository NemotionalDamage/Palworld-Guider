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


### SRC-PALDB-CORE-ITEMS-V1_0_3-20260902
- Evidence: Paldb English and Simplified Chinese pages for Stone, Paldium Fragment, Pal Sphere, Gold Coin, Red Berries, and Baked Berries
- Retrieved: 2026-09-02
- Applicable version claimed by source: 1.0.3
- Review result: reviewed-secondary
- Normalized scope: six item records with localized names and reviewed acquisition leads; three recipes; Technology Level 2; its Pal Sphere unlock relationship; Simplified Chinese names; and common English colloquial aliases.

Paldb current pages report Paldium Fragment as five Stone at a Crusher, Pal Sphere as one Paldium Fragment, and Baked Berries as one Red Berry at a Campfire. The basic Pal Sphere recipe therefore contains no Wood or Stone in this source snapshot. These facts have not been cross-checked against a second reviewed source.

## Durable Uncertainty


- The seed facts have not been cross-checked against a second reviewed source.
- Paldb is a community database and may lag behind a Palworld patch; each persisted fact carries a change-risk warning.
- The expanded core-item set has not been cross-checked against a second reviewed source.
- No habitat, breeding rule, Pal stats, or broader item-use claim was promoted from the core-item source.
- The current dataset is intentionally small and is not a complete game encyclopedia.

## Verification

- Red test 1: `cargo test` initially failed because the typed schema API did not exist.
- Red test 2: `cargo test --test canonical_dataset` initially failed because `data/reviewed/sources.jsonl` did not exist.
- Final format gate: `cargo-fmt --all -- --check` passed.
- Final lint gate: `cargo-clippy --all-targets -- -D warnings` passed.
- Post-audit test gate: `cargo test` passed with eight tests across schema validation and the canonical dataset.

- Expanded-dataset gate: cargo test -p game-knowledge passed with nine tests across schema validation and the canonical dataset.
- Expanded-dataset CLI spot checks resolved Stone, the Chinese Pal Sphere name, the Pal Sphere recipe, and the roasted-berry alias to reviewed records with provenance.

## Audit Correction

An accepted audit reopened the phase because the initial validator allowed missing ingredient references, wrong-type references through a global fact-ID fallback, impossible calendar dates, and incomplete recipe fields. The source log also retained a stale “no sources” sentence.

The correction adds regressions for missing recipe ingredient references, recipe output pointing to a non-item fact, empty ingredients or crafting stations, negative crafting time, impossible dates, and provenance drift from the registered source. Typed references now resolve only to their required record type; generic alias, progression, and conflict references use a separate any-fact check. Recipe ingredients, stations, and present crafting time are validated. Dates use real Gregorian calendar validation, and the stale source-log sentence was removed.

No checks were intentionally skipped.

## Second Audit Correction

A second accepted audit reopened the phase because version format, conflict shape, identifier charset, and alias locale checks were incomplete. `nonsense-version` loaded as an applicable game version, a conflict with empty `values` loaded and mislabeled lookups as ambiguous, pure punctuation IDs such as `-` loaded, and arbitrary alias locales such as `xx` were accepted.

The correction adds a dot-separated numeric version rule for sources and provenance, requires at least two nonempty conflict values and one evidence source per conflict, requires at least one ASCII letter or digit in every ID, and restricts alias locales to `en` and `zh_hans`. Regression tests cover all four cases. `cargo test -p game-knowledge` passes with nine tests across schema validation and the canonical dataset.
