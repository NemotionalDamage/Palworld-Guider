# Phase G1 Local-Build Intake Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the verified local Palworld Build 24575825 exports into auditable, deterministic, reviewed knowledge while preserving conflicts and refusing unreviewed bulk promotion.

**Architecture:** Raw FModel JSON remains under gitignored `.local/research/local-build/raw`. A Rust parser in `game-knowledge` validates the local table shape, resolves native row IDs and localization, emits candidate records plus an audit report, and never writes canonical data directly. Canonical promotion is batched and reviewed through JSONL with source provenance, native row IDs, corroboration, and explicit conflicts.

**Tech Stack:** Rust, serde JSON deserialization, existing `game-knowledge` typed records and validation, JSONL canonical review artifacts.

**Spec:** `AGENTS.md`; `PHASES.md`; `docs/phase-records/phase-g1.md`

## Global Constraints

- Treat `AGENTS.md` as the governing product specification.
- Raw exports and generated candidates stay out of Git under `.local/`.
- Store only reviewed, transformed facts and provenance in canonical data.
- The production parser and intake pipeline are Rust; Python may inspect local research data but is not the product pipeline.
- Exact local target-build data outranks secondary sources, but a difference creates a visible conflict rather than an automatic overwrite.
- Do not guess field semantics, crafting-station relationships, map object identity, or game-version claims.
- Every source record and promoted fact must pass `KnowledgeStore::from_records`.
- Run `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` before closing G1.
- Update `PHASES.md` and `docs/phase-records/phase-g1.md` at every durable gate or blocker.

## Current Export Baseline

- Installed game: Steam App 1623730, Build 24575825, using the project owner's local `Pal-Windows.pak`; the machine-specific installation path remains outside Git.
- FModel mapping: `PalworldModding/UsefulFiles` commit `0e4ae19a05ba0d9fb95d859c09b28f168cb3624f`, labeled 1.0.3.
- Export manifest: `.local/research/local-build/FModel-Export-Manifest.txt`.
- Manifest SHA-256: `C5850F0EDCE381850526420218F6C0DBC20570BBA97FCE0F23D0978703EFABA7`.
- Preserved raw exports: `.local/research/local-build/raw`, 518 files, 135,737,060 bytes.
- Completeness gate: 518/518 manifest files present, all parse as JSON, zero missing.
- Important table counts: Item 2,466; Item Recipe 1,414; Technology Unlock 588; Pal Parameter 753; Pal Drop 1,044; EN/zh-Hans Item Name 1,994 each.
- One manifest table, `DT_SupplyIncident_NPC_Sakura01`, is valid JSON with zero rows; it is not a blocker.
- Terminal rendering can make valid Chinese appear garbled; verify Unicode code points before calling localization corrupt. Literal locale placeholders such as `zh-hans text` must be rejected.

---

### Task 1: Register The Local Build Source

**Files:**
- Modify: `data/reviewed/sources.jsonl`
- Modify: `docs/reference-data/source-log.md`
- Test: `crates/game-knowledge/tests/canonical_dataset.rs`

**Interfaces:**
- Consumes: existing `SourceRecord` and `Confidence::VerifiedTarget`.
- Produces: source ID `SRC-LOCAL-BUILD-24575825-20260902` for every later local provenance and corroboration reference.

- [ ] **Step 1: Write the failing source assertion**

Append this test to `canonical_dataset.rs`, replacing only the test name if it already exists:

```rust
#[test]
fn loads_local_build_source() {
    let store = KnowledgeStore::load_directory("data/reviewed").unwrap();
    let source = store
        .source("SRC-LOCAL-BUILD-24575825-20260902")
        .expect("local build source exists");

    assert_eq!(source.applicable_game_version, "1.0.3");
    assert_eq!(source.confidence, Confidence::VerifiedTarget);
    assert_eq!(source.review_status, ReviewStatus::Reviewed);
    assert!(source
        .notes
        .as_deref()
        .unwrap()
        .contains("Steam Build 24575825"));
}
```

- [ ] **Step 2: Run the focused test and verify it fails**

Run: `cargo test -p game-knowledge loads_local_build_source`

Expected: FAIL because `SRC-LOCAL-BUILD-24575825-20260902` is absent.

- [ ] **Step 3: Add the reviewed source record**

Append exactly one JSONL row to `data/reviewed/sources.jsonl`:

```json
{"record_type":"source","id":"SRC-LOCAL-BUILD-24575825-20260902","title":"Palworld local Steam Build 24575825 DataTable export","supplier":"project owner local game installation","retrieved_on":"2026-09-02","evidence_urls":["local://Pal-Windows.pak/Pal/Content/Pal/DataTable","local://Pal-Windows.pak/Pal/Content/L10N/en/Pal/DataTable/Text","local://Pal-Windows.pak/Pal/Content/L10N/zh-Hans/Pal/DataTable/Text"],"applicable_game_version":"1.0.3","reviewer":"Codex","review_status":"reviewed","confidence":"verified_target","notes":"Exact target Steam Build 24575825. FModel used PalworldModding/UsefulFiles mapping commit 0e4ae19a05ba0d9fb95d859c09b28f168cb3624f, labeled 1.0.3. Reviewed manifest SHA-256 C5850F0EDCE381850526420218F6C0DBC20570BBA97FCE0F23D0978703EFABA7 contains 518 transformed JSON exports; raw assets remain local and uncommitted."}
```

Add the matching row to `docs/reference-data/source-log.md`. Scope is `targeted DataTable extraction only`, not a claim that all 518 tables have been reviewed.

- [ ] **Step 4: Run the source gate**

Run: `cargo test -p game-knowledge loads_local_build_source`

Expected: PASS.

- [ ] **Step 5: Commit**

Run:

```powershell
git -c core.autocrlf=false add data/reviewed/sources.jsonl docs/reference-data/source-log.md crates/game-knowledge/tests/canonical_dataset.rs
git -c core.autocrlf=false commit -m "phase g1: register local build source"
```

### Task 2: Add Native Row Identity And Corroborating Provenance

**Files:**
- Modify: `crates/game-knowledge/src/models.rs`
- Modify: `crates/game-knowledge/src/store.rs`
- Modify: `crates/game-knowledge/src/lib.rs`
- Test: `crates/game-knowledge/tests/validation.rs`

**Interfaces:**
- Produces on `ItemRecord`, `PalRecord`, `TechnologyRecord`, and `RecipeRecord`:

```rust
#[serde(default)]
pub native_row_id: Option<String>,
```

- Produces on `Provenance`:

```rust
#[serde(default)]
pub corroborating_source_ids: Vec<String>,
```

- Validation requires nonempty native IDs when present and rejects duplicate native IDs within each of the four record families.
- Validation rejects a corroborating source that is not registered.
- Existing canonical JSON remains compatible because both fields use `#[serde(default)]`.

- [ ] **Step 1: Write failing compatibility and uniqueness tests**

Add tests that construct one existing record without the new fields, proving deserialization remains compatible, and two item records with `native_row_id: Some("Wood".into())`, proving duplicate native identity fails validation.

```rust
#[test]
fn local_identity_preserves_legacy_records_and_rejects_duplicates() {
    let legacy_json = r#"{
        "record_type": "item",
        "id": "ITEM_TEST",
        "names": { "en": "Test" },
        "description": null,
        "rarity": "common",
        "acquisition_leads": [],
        "provenance": {
            "source_id": "SRC-LOCAL-BUILD-24575825-20260902",
            "applicable_game_version": "1.0.3",
            "retrieved_on": "2026-09-02",
            "reviewer": "Codex",
            "review_status": "reviewed",
            "confidence": "verified_target"
        }
    }"#;
    let legacy: KnowledgeRecord = serde_json::from_str(legacy_json).unwrap();
    assert!(KnowledgeStore::from_records(vec![source_record(), legacy]).is_ok());

    let duplicate = item_record("ITEM_TEST_2", Some("Wood"));
    let records = vec![source_record(), item_record("ITEM_TEST", Some("Wood")), duplicate];
    let errors = KnowledgeStore::from_records(records).unwrap_err();
    assert!(errors
        .iter()
        .any(|error| error.message.contains("duplicate native row ID")));
}
```

Use the existing validation-test helpers where possible; if helpers do not exist, define local helper functions in the test rather than changing public APIs.

- [ ] **Step 2: Run the focused validation test**

Run: `cargo test -p game-knowledge local_identity_preserves_legacy_records_and_rejects_duplicates`

Expected: FAIL because the fields and validation do not exist.

- [ ] **Step 3: Implement the minimal schema and validation**

Add the two defaulted fields, collect native IDs into `BTreeMap<String, String>` maps during `from_records`, and validate corroborating source IDs after all source records are inserted. Do not change canonical IDs; native row identity is evidence, not a replacement for stable guide IDs.

- [ ] **Step 4: Run schema and dataset gates**

Run:

```powershell
cargo fmt --all
cargo test -p game-knowledge
```

Expected: all tests pass.

- [ ] **Step 5: Commit**

```powershell
git -c core.autocrlf=false add crates/game-knowledge/src crates/game-knowledge/tests/validation.rs
git -c core.autocrlf=false commit -m "phase g1: add local row provenance identity"
```

### Task 3: Parse Core Local Tables In Rust

**Files:**
- Create: `crates/game-knowledge/src/local_build.rs`
- Modify: `crates/game-knowledge/src/lib.rs`
- Create: `crates/game-knowledge/tests/fixtures/local-build/Pal/Content/Pal/DataTable/Item/DT_ItemDataTable.json`
- Create: `crates/game-knowledge/tests/fixtures/local-build/Pal/Content/Pal/DataTable/Item/DT_ItemRecipeDataTable.json`
- Create: `crates/game-knowledge/tests/fixtures/local-build/Pal/Content/Pal/DataTable/Technology/DT_TechnologyRecipeUnlock.json`
- Create: `crates/game-knowledge/tests/fixtures/local-build/Pal/Content/Pal/DataTable/Character/DT_PalMonsterParameter.json`
- Create: `crates/game-knowledge/tests/fixtures/local-build/Pal/Content/Pal/DataTable/Character/DT_PalDropItem_Common.json`
- Test: `crates/game-knowledge/tests/local_build.rs`

**Interfaces:**

```rust
pub struct LocalBuildTables;

impl LocalBuildTables {
    pub fn load(root: &Path) -> Result<Self, LocalBuildError>;
    pub fn item(&self, row_id: &str) -> Option<&LocalItemRow>;
    pub fn recipe(&self, row_id: &str) -> Option<&LocalRecipeRow>;
    pub fn technology(&self, row_id: &str) -> Option<&LocalTechnologyRow>;
    pub fn pal(&self, row_id: &str) -> Option<&LocalPalRow>;
    pub fn drops_for(&self, character_id: &str) -> Vec<&LocalDropRow>;
}
```

Public row structs expose only fields used by G1:
- `LocalItemRow`: `type_a`, `type_b`, `rank`, `rarity`, `max_stack_count`, `weight`, `price`, `is_legal_in_game`, `technology_tree_lock`.
- `LocalRecipeRow`: product ID/count, five material ID/count pairs, work amount, workable attribute, unlock item ID.
- `LocalTechnologyRow`: unlocked build object IDs, unlocked item recipe IDs, required technology, required research ID, level cap, tier, cost.
- `LocalPalRow`: stats and all `WorkSuitability_*` levels.
- `LocalDropRow`: character ID, item ID, rate, min, max.

Raw structs use serde field mappings and reject unexpected JSON failure; extra raw fields are ignored.

- [ ] **Step 1: Create synthetic fixtures**

Use synthetic row IDs such as `TestWood`, `TestSphere`, and `TestSheepBall`; do not commit raw game exports. The fixture JSON must copy the exact FModel field names and contain one or two rows per table.

- [ ] **Step 2: Write the failing parser test**

```rust
#[test]
fn loads_core_local_build_tables() {
    let tables = LocalBuildTables::load(
        Path::new("crates/game-knowledge/tests/fixtures/local-build"),
    )
    .unwrap();

    let wood = tables.item("TestWood").unwrap();
    assert_eq!(wood.max_stack_count, 9999);
    assert!(wood.is_legal_in_game);

    let sphere = tables.recipe("TestSphere").unwrap();
    assert_eq!(sphere.product_id, "TestPaldium");
    assert_eq!(sphere.materials[0].quantity, 1);

    let lamball = tables.pal("TestSheepBall").unwrap();
    assert_eq!(lamball.work_suitability("Handcraft"), Some(1));

    let drops = tables.drops_for("TestSheepBall");
    assert_eq!(drops.len(), 1);
    assert_eq!(drops[0].item_id, "TestWool");
}
```

- [ ] **Step 3: Run and verify failure**

Run: `cargo test -p game-knowledge loads_core_local_build_tables`

Expected: FAIL because `local_build.rs` does not exist.

- [ ] **Step 4: Implement deserialization and lookup**

Read each table as a one-element FModel export array with a `Rows` map. Return `LocalBuildError::MissingTable` with the relative package path when a required table is absent. Return no drops rather than an error when a character has no drop row.

- [ ] **Step 5: Run parser and package gates**

```powershell
cargo fmt --all
cargo test -p game-knowledge
```

Expected: all tests pass.

- [ ] **Step 6: Commit**

```powershell
git -c core.autocrlf=false add crates/game-knowledge/src crates/game-knowledge/tests
git -c core.autocrlf=false commit -m "phase g1: parse local build core tables"
```

### Task 4: Resolve Localization Without Trusting Placeholders

**Files:**
- Modify: `crates/game-knowledge/src/local_build.rs`
- Create: `crates/game-knowledge/tests/fixtures/local-build/Pal/Content/L10N/en/Pal/DataTable/Text/DT_ItemNameText_Common.json`
- Create: `crates/game-knowledge/tests/fixtures/local-build/Pal/Content/L10N/en/Pal/DataTable/Text/DT_ItemDescriptionText_Common.json`
- Create: `crates/game-knowledge/tests/fixtures/local-build/Pal/Content/L10N/en/Pal/DataTable/Text/DT_PalNameText_Common.json`
- Create matching `zh-Hans` fixtures for the three name/description tables.
- Test: `crates/game-knowledge/tests/local_build.rs`

**Interfaces:**

```rust
pub struct LocalizationIndex;

impl LocalizationIndex {
    pub fn load(root: &Path) -> Result<Self, LocalBuildError>;
    pub fn item_name(&self, item_row_id: &str, locale: LocalBuildLocale) -> Result<String, LocalBuildError>;
    pub fn item_description(&self, item_row_id: &str, locale: LocalBuildLocale) -> Result<String, LocalBuildError>;
    pub fn pal_name(&self, pal_row_id: &str, locale: LocalBuildLocale) -> Result<String, LocalBuildError>;
}

pub enum LocalBuildLocale {
    English,
    SimplifiedChinese,
}
```

Resolution rules:
- Item name key: `ITEM_NAME_{row_id}`.
- Item description key: `ITEM_DESC_{row_id}` when present.
- Pal name key: `PAL_NAME_{row_id}` when present; verify the exact prefix against the local table before treating it as universal.
- Missing English is an error for candidate generation.
- Missing Simplified Chinese yields `None`, not a fabricated translation.
- Reject empty strings, strings containing U+FFFD, and locale placeholders such as `en Text` or `zh-hans text`.
- Rich references such as `<itemName id=|PalSphere|/>` resolve recursively through the item-name index; `<mapObjectName .../>` resolves only after Task 6 proves the map-object table path.

- [ ] **Step 1: Add failing localization tests**

Test three cases: valid EN/zh names, missing zh returns `None`, and literal `zh-hans text` is rejected.

- [ ] **Step 2: Run the focused test**

Run: `cargo test -p game-knowledge localization`

Expected: FAIL.

- [ ] **Step 3: Implement the localization index**

Store strings as Rust `String`; do not normalize away valid Chinese. Keep a set of rejected placeholder keys in the returned audit report.

- [ ] **Step 4: Run package gates**

```powershell
cargo fmt --all
cargo test -p game-knowledge
```

Expected: all tests pass.

- [ ] **Step 5: Commit**

```powershell
git -c core.autocrlf=false add crates/game-knowledge/src/local_build.rs crates/game-knowledge/tests
git -c core.autocrlf=false commit -m "phase g1: resolve local build localization"
```

### Task 5: Build The Pilot Cross-Check Gate

**Files:**
- Modify: `crates/game-knowledge/src/local_build.rs`
- Test: `crates/game-knowledge/tests/local_build.rs`
- Test: `crates/game-knowledge/tests/canonical_dataset.rs`

**Interfaces:**

```rust
pub struct PilotCrossCheck {
    pub matching: Vec<String>,
    pub conflicting: Vec<PilotConflict>,
    pub local_only: Vec<String>,
    pub unresolved: Vec<String>,
}

pub struct PilotConflict {
    pub subject_id: String,
    pub field: String,
    pub canonical_value: String,
    pub local_value: String,
}

pub fn cross_check_pilot(
    tables: &LocalBuildTables,
    localization: &LocalizationIndex,
    store: &KnowledgeStore,
) -> PilotCrossCheck;
```

Pilot subjects:
- Items: `Wood`, `Stone`, `Pal_crystal_S`, `PalSphere`, `Berries`, `Baked_Berries`, `Wool`, `Meat_SheepBall`.
- Recipes: `Axe_Tier_00`, `PalSphere`, `Baked_Berries`.
- Pals: `SheepBall`.
- Technologies: the rows that unlock `Axe_Tier_00`, `PalSphere`, and `Baked_Berries`; discover these by scanning `UnlockItemRecipes`, not by assuming row IDs.

Known result that must remain visible:
- Local `Axe_Tier_00` resolves to English `Stone Axe`; the current canonical entity is `Wooden Club`. This is a product-identity/name conflict and must not be silently overwritten.
- Local `PalSphere` confirms one Paldium Fragment.
- Local `Baked_Berries` confirms one `Berries`.
- Local `SheepBall` confirms Lamball work suitability and drops.
- `DT_ItemRecipeDataTable.WorkableAttribute` alone does not prove a crafting-station name. Preserve existing station strings and their original provenance until the map-object/build-object relation is independently proven.

- [ ] **Step 1: Write failing cross-check tests**

Assert:
- `Wood`, `PalSphere`, `Baked_Berries`, and `SheepBall` appear in matching or corroborated results.
- `Axe_Tier_00` produces a conflict containing `Wooden Club` and `Stone Axe`.
- No pilot result invents a crafting-station relationship.

- [ ] **Step 2: Run focused test**

Run: `cargo test -p game-knowledge cross_check`

Expected: FAIL.

- [ ] **Step 3: Implement comparison and audit output**

Compare typed canonical values after trimming only serde enum prefixes that the parser explicitly understands (for example, `EPalItemTypeA::Material` to `material` when a typed field is introduced). Do not compare by dumping entire JSON rows.

- [ ] **Step 4: Run gates**

```powershell
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test -p game-knowledge
```

Expected: all pass.

- [ ] **Step 5: Commit**

```powershell
git -c core.autocrlf=false add crates/game-knowledge/src/local_build.rs crates/game-knowledge/tests
git -c core.autocrlf=false commit -m "phase g1: cross-check local pilot facts"
```

### Task 6: Promote Only Reviewed Non-Conflicting Pilot Facts

**Files:**
- Modify: `data/reviewed/facts.jsonl`
- Modify: `crates/game-knowledge/tests/canonical_dataset.rs`
- Modify: `docs/phase-records/phase-g1.md`
- Modify: `PHASES.md`

**Interfaces:**
- Consumes `native_row_id` and `corroborating_source_ids`.
- Produces canonical records and at least one explicit `ConflictRecord` for `Wooden Club` versus `Stone Axe`.

Promotion rules:
- Add local corroboration to records whose complete compared field matches.
- Do not change `Wood` acquisition from `Chop trees`; the local item table does not encode that action.
- Do not change crafting-station strings from Paldb in this task.
- Keep the Wooden Club/Stone Axe product identity unresolved and visible.
- Add native row IDs only where the local row identity is unambiguous.
- Add no bulk records in this task.

- [ ] **Step 1: Write failing canonical pilot assertions**

Extend `canonical_dataset.rs` to assert:

```rust
assert_eq!(wood.native_row_id.as_deref(), Some("Wood"));
assert!(wood
    .provenance
    .corroborating_source_ids
    .contains(&"SRC-LOCAL-BUILD-24575825-20260902".to_string()));
assert!(store
    .conflicts_for_subject("ITEM_WOODEN_CLUB")
    .iter()
    .any(|conflict| conflict.field == "product_identity"));
```

- [ ] **Step 2: Run and verify failure**

Run: `cargo test -p game-knowledge canonical_dataset`

Expected: FAIL.

- [ ] **Step 3: Apply the narrow canonical edits**

Edit existing records with corroborating source IDs and native IDs. Add one conflict record with IDs/values expressed as stable strings. Do not retire the Paldb source.

- [ ] **Step 4: Run full knowledge gates**

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test -p game-knowledge
```

Expected: all pass.

- [ ] **Step 5: Update phase documentation**

Record that the pilot passed local corroboration, the known identity conflict remains open, station relationships remain unverified from local data, and bulk promotion has not started.

- [ ] **Step 6: Commit**

```powershell
git -c core.autocrlf=false add data/reviewed/facts.jsonl crates/game-knowledge/tests/canonical_dataset.rs docs/phase-records/phase-g1.md PHASES.md
git -c core.autocrlf=false commit -m "phase g1: corroborate pilot facts locally"
```

### Task 7: Add Candidate Generation For Reviewed Batch Expansion

**Files:**
- Create: `crates/game-knowledge/src/bin/local-build-intake.rs`
- Modify: `crates/game-knowledge/src/local_build.rs`
- Modify: `crates/game-knowledge/Cargo.toml` only if a serde or clap dependency is genuinely required.
- Test: `crates/game-knowledge/tests/local_build.rs`

**Interfaces:**

```rust
pub struct IntakeCandidateSet {
    pub records: Vec<KnowledgeRecord>,
    pub report: IntakeReport,
}

pub struct IntakeReport {
    pub input_items: usize,
    pub candidate_items: usize,
    pub rejected_items: usize,
    pub candidate_recipes: usize,
    pub unresolved_recipe_references: usize,
    pub candidate_pals: usize,
    pub localization_rejections: usize,
}

pub fn generate_candidates(
    tables: &LocalBuildTables,
    localization: &LocalizationIndex,
    batch: IntakeBatch,
) -> Result<IntakeCandidateSet, LocalBuildError>;
```

CLI:

```powershell
cargo run -p game-knowledge --bin local-build-intake -- --root .local/research/local-build/raw --batch items --output .local/research/local-build/candidates/items.jsonl
```

Batch order:
1. `items`: only `bLegalInGame == true`, valid English names, and non-placeholder zh-Hans when present.
2. `recipes`: only products and all referenced ingredients already present in the same candidate set or canonical store; unresolved references remain in the report.
3. `technology`: item-recipe unlocks first; build-object unlocks wait for verified map-object identity.
4. `pals`: stats, work suitability, and drops for legal Pal rows with resolvable names and item references.
5. `map-objects`: deferred until station/build relations are independently proven.

The CLI always writes to `.local/`; it must refuse an output path outside `.local/research/local-build/candidates/`.

- [ ] **Step 1: Write failing candidate-generation tests**

Use synthetic fixtures to assert legal items become candidates, placeholder localization is rejected, and a recipe with an unresolved ingredient is reported rather than emitted.

- [ ] **Step 2: Run focused test**

Run: `cargo test -p game-knowledge generate_candidates`

Expected: FAIL.

- [ ] **Step 3: Implement deterministic candidate generation**

Candidate provenance uses `SRC-LOCAL-BUILD-24575825-20260902`, `ReviewStatus::Candidate`, and `Confidence::VerifiedTarget`; candidates never enter `data/reviewed/facts.jsonl`.

- [ ] **Step 4: Run CLI smoke checks**

Run the `items` and `recipes` commands against `.local/research/local-build/raw`, then verify:
- output files exist under `.local`;
- report counts are deterministic across two runs;
- no canonical file changes.

- [ ] **Step 5: Run full gates**

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Expected: all pass.

- [ ] **Step 6: Commit**

```powershell
git -c core.autocrlf=false add crates/game-knowledge
git -c core.autocrlf=false commit -m "phase g1: generate reviewed local candidates"
```

### Task 8: Close The G1 Local-Build Pilot Gate

**Files:**
- Modify: `docs/phase-records/phase-g1.md`
- Modify: `PHASES.md`
- Modify: `docs/reference-data/source-log.md` only if source scope changed.

**Interfaces:**
- Consumes all prior tasks.
- Produces the final G1 gate result.

- [ ] **Step 1: Run guide spot checks**

Use the existing `guide-core` CLI or tests to check:
- `Wood`
- `Stone`
- `Pal Sphere`
- `Red Berries`
- `Baked Berries`
- `Lamball`
- `Wooden Club` conflict visibility

Expected: exact lookups retain provenance and report uncertainty/conflict where applicable.

- [ ] **Step 2: Run full release gates**

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
git -c core.autocrlf=false diff --check
```

Expected: all pass.

- [ ] **Step 3: Record the durable boundary**

The phase record must state:
- 518 exports preserved locally;
- pilot local corroboration passed;
- Wooden Club/Stone Axe conflict remains open;
- crafting-station relations remain unverified from local data;
- bulk candidate generation exists but no unreviewed bulk promotion occurred;
- semantic review is required before each canonical batch.

- [ ] **Step 4: Commit**

```powershell
git -c core.autocrlf=false add PHASES.md docs/phase-records/phase-g1.md docs/reference-data/source-log.md
git -c core.autocrlf=false commit -m "phase g1: close local build pilot"
```

## Execution Sequence

Execute Tasks 1–8 in order. Do not begin Task 7 until Task 6 passes and its conflict decision is documented. Do not start G2 or G5 work.

## Self-Review

- Spec coverage: provenance, local-source precedence, conflict preservation, Rust pipeline, validation, documentation, and no unreviewed bulk promotion are all represented.
- Placeholder scan: no task relies on an unspecified implementation step; deferred work is expressed as explicit gates rather than `TODO`.
- Type consistency: `native_row_id`, `corroborating_source_ids`, `LocalBuildTables`, `LocalizationIndex`, `PilotCrossCheck`, and `IntakeCandidateSet` use one naming contract throughout.
