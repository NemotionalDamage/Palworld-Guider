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

## Local-Build Data Pilot

The project owner approved reopening G1 for a narrowly scoped extraction pilot against installed Steam build 24575825. The pilot must first reproduce the already reviewed early-game item, recipe, technology, and Lamball facts before broader intake is considered. Raw exports stay under gitignored .local/research/local-build; only reviewed transformed facts and provenance may enter canonical data.

The initial missing-mapping blocker is resolved. FModel 4.4.4 now uses PalworldModding/UsefulFiles mapping commit 0e4ae19a05ba0d9fb95d859c09b28f168cb3624f (labeled 1.0.3). The exported DT_PalDropItem_Common table contains 1,044 rows. SheepBall000 confirms Wool 1-3 and Meat_SheepBall 1, each at 100 percent. DT_PalMonsterParameter contains 753 rows; SheepBall confirms WorkSuitability_Handcraft, WorkSuitability_Transport, and WorkSuitability_MonsterFarm are each level 1. Raw JSON remains under .local/research/local-build; do not guess field meaning or promote unreviewed exports into canonical game facts.

The full approved export batch is complete. The preserved manifest references 518 files and 135,737,060 bytes, with SHA-256 C5850F0EDCE381850526420218F6C0DBC20570BBA97FCE0F23D0978703EFABA7. The completeness audit matched 518/518 files, found zero missing or invalid JSON files, and ignored 11 extra exported Pal assets outside the manifest. One manifest table, DT_SupplyIncident_NPC_Sakura01, is valid JSON with zero rows. Key table counts are Item 2,466; Item Recipe 1,414; Technology Unlock 588; Pal Parameter 753; Pal Drop 1,044; and EN/zh-Hans Item Name 1,994 each.

The next implementation step is the reviewed Rust intake plan in docs/superpowers/plans/2026-09-02-phase-g1-local-build-intake.md. It registers the local source, adds native-row and corroboration metadata, parses the five core table families, resolves localization with Unicode and placeholder gates, cross-checks pilot facts, promotes only reviewed non-conflicting candidates, and provides a candidate-generation CLI under .local. The known local Axe_Tier_00 identity conflict must remain explicit rather than silently replacing the current Wooden Club entity.

## Local-Build Intake Status

The verified local source `SRC-LOCAL-BUILD-24575825-20260902` is registered. Item, Pal, technology, and recipe records now support optional unique `native_row_id` values; provenance supports registered `corroborating_source_ids`; and candidate-only local-evidence metadata records source table, localization status, unresolved fields, and transformation notes.

The generic Rust parser covers the five core DataTable families and the selected localization families. Real-build coverage is:

| Table family | Rows | Parse failures |
|---|---:|---:|
| DT_ItemDataTable | 2,466 | 0 |
| DT_ItemRecipeDataTable | 1,414 | 0 |
| DT_TechnologyRecipeUnlock | 588 | 0 |
| DT_PalMonsterParameter | 753 | 0 |
| DT_PalDropItem_Common | 1,044 | 0 |
| EN/zh-Hans DT_ItemNameText_Common | 1,994 each | 0 parser failures; localization rejections counted separately |
| EN/zh-Hans DT_ItemDescriptionText_Common | 1,924 each | 0 parser failures; localization rejections counted separately |
| EN/zh-Hans DT_PalNameText_Common | 322 each | 0 |

The parser reports total, parsed, skipped, failed, observed, missing, null, `None`, empty, zero, false, unsupported-enum, duplicate, reference, and representative-error counts. Representative errors use a stable FNV-1a row hash. Localization preserves Unicode and separately reports hits, misses, rejections, and conflicts. `SourceString` and `LocalizedString` are identical in all selected real localization rows; the 461 reported rejections are genuine placeholder/invalid-Unicode rows.

`local-build-intake --batch all` writes only under `.local/research/local-build/candidates/`. Repeated real-data runs produce byte-identical candidate and report files. Current machine-audited totals are 4,633 candidate records and 18,555 explicit row outcomes: 1,520 item candidates, 915 recipe candidates, 309 Pal candidates, 369 provable unlock relationships, 499 unresolved recipe references, 4,094 unresolved drop-rate units, and 461 localization rejections. Technology-record candidates are intentionally zero because technology-name localization and map-object identity remain unverified. Pal drop rates are not promoted as percentages while their representation is unresolved. No unreviewed candidate has entered `data/reviewed/`.

## Canonical Backfill Audit

The field-level canonical backfill gate passed. The Rust auditor loads the validated canonical store, maps through native IDs and exact structural relationships, and emits one primary classification for every reviewed game field. It covers all 38 canonical records and 105 field-level facts:

| Classification | Facts |
|---|---:|
| `corroborated_exact` | 42 |
| `corroborated_partial` | 8 |
| `conflicting` | 6 |
| `not_represented_locally` | 37 |
| `unresolved_mapping` | 12 |
| `ambiguous_mapping` | 0 |
| `invalid_canonical_record` | 0 |
| Unclassified | 0 |

Mapping success is 75.24%. Missing localization, schema failures, provenance failures, and unclassified facts are zero. The six conflicting field facts all derive from the two preserved explicit conflicts: Wooden Club versus Stone Axe product identity and the conflicting Wooden Club ingredients. No old value or provenance was overwritten.

Reviewed backfill actions add unique native row IDs, add local corroboration only after field review, and preserve the two explicit conflict records. The report records every required field, classification, difference, proposed action, and review status. `local-build-intake --batch canonical-backfill` accepts only `.local/research/local-build/reports/canonical-backfill.json`; the final generated report has SHA-256 `ee10be12a962f72da841d7572412fb7c043611da3c966a03069e6dcbe4acccc7`. The report and all raw or bulk generated data remain uncommitted.

Crafting stations, map objects, stat scales, rarity semantics, and drop-probability units remain unresolved. Technology identities also remain unverified, so local corroboration is not claimed for those records.

The final reopened-G1 gate passed `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, and `git -c core.autocrlf=false diff --check`. Runtime adapter, agent, planner, calculator, and tool-registry tests use stable conflict-free views or non-conflicting records so their behavioral assertions remain independent of the two preserved canonical conflicts; explicit conflict propagation remains separately tested. No required command was skipped.

## Unified Intake Standard And Completeness Audit

The project owner approved a second reopened G1 subphase to define one field standard per record class before expanding the local-build candidate set. The durable standard is `docs/schemas/knowledge-intake.md`. It distinguishes structural validity from knowledge completeness, requires local-build Item, Pal, Technology, and Recipe records to carry native-row and local-evidence metadata, and requires every null, empty, or unresolved semantic field to be documented explicitly.

The canonical dataset audit covered all 38 reviewed facts. The Rust loader reports zero structural-validation failures. Literal optional-field gaps are:

- Seven records have null `names.zh_hans`: four Items, one Pal, and both Technologies.
- Four Items have null `description`: Wood, Wooden Club, Wool, and Lamball Mutton.
- Three Items have empty direct `acquisition_leads`; Wooden Club is covered by its recipe and Wool and Lamball Mutton by Lamball drops, so these are relational rather than unknown acquisition paths.
- Lamball has null `stats` and an empty `habitat_ids` list; the latter remains a known map and habitat coverage gap.
- All four Recipes have null `crafting_seconds`; Paldium Fragment and Baked Berries also have null `technology_id`.
- Both Progression relationships have null `requirement`, which the standard defines as no additional requirement rather than a missing field.
- All 17 Aliases omit `change_risk`; this is allowed because their primary sources and target records retain version risk.

The 4,633-row candidate audit found no missing shared intake fields: every candidate has its class fields, provenance, and, where applicable, native and local evidence metadata. Knowledge-completeness blockers are material:

- Item: 78 of 1,520 descriptions are null without a listed description blocker, and all 1,520 acquisition-lead lists are empty without a listed acquisition blocker. Rarity is explicitly unknown and its numeric-semantics blocker is recorded.
- Pal: all 309 stats are null with a recorded stats-scale blocker, all 309 drop lists are empty with a recorded drop-probability blocker, and all 309 habitat lists are empty without an explicit habitat blocker.
- Recipe: all 915 station lists contain only `unresolved`, so the batch cannot be promoted as reviewed recipe knowledge yet. Technology and duration are null for all 915, and empty-byproduct semantics are not explicit.
- Alias: all 1,520 rows are structurally complete; Chinese values intentionally mirror canonical localized names for current locale resolution.
- Progression relationship: all 369 rows are structurally complete, but every `from_id` references a Technology record absent from the candidate set, so reference integrity blocks promotion.

The safe first promotion surface is therefore not all 4,633 records. Item identity and localization can be reviewed after adding explicit description and acquisition uncertainty; Pal identity and localization can be reviewed after adding explicit habitat uncertainty; Recipe and Progression batches require station, technology, duration, and byproduct semantic review. No candidate was promoted during this audit.
