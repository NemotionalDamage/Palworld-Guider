# Data Updates

How to add, review, and maintain reviewed knowledge records. The knowledge base is the source of truth for game facts; the LLM is never a source of truth.

Reviewed data lives as JSONL under `data/reviewed/`. Candidate and research output stays under the gitignored `.local/` tree and never enters canonical data until reviewed.

## 1. Knowledge-Update Workflow

### Step 1: Register the source

Before using any new source's facts, register it in `docs/reference-data/source-log.md`. Record the ID, title, supplier, retrieved date, URL or local path, claimed Palworld version, scope, confidence, review status, and notes. See the source log format below.

### Step 2: Export local-build data

Export DataTables from the exact target Palworld Steam build using FModel with the PalworldModding/UsefulFiles mapping. Raw exports stay under `.local/research/local-build/` and are never committed. Record the FModel mapping commit and the export manifest SHA-256 in the source log notes.

### Step 3: Run the local-build intake CLI

Generate candidate records from the raw export. The CLI requires `--root`, `--batch`, and `--output`:

```powershell
cargo run -p game-knowledge --bin local-build-intake -- `
  --root .local/research/local-build `
  --batch all `
  --output .local/research/local-build/candidates/all.jsonl
```

Available batches: `all`, `items`, `recipes`, `technologies`, `pals`, `pal-drops`, `work-suitability`, `localization-aliases`, `relationships`, and `canonical-backfill`.

### Step 4: Review the generated candidates

Open the candidate JSONL under `.local/research/local-build/candidates/` and the accompanying `.report.json`. Review each candidate's fields, unresolved markers, and localization status. A candidate may carry `review_status: candidate` and explicit `unresolved` values, but neither may enter canonical data until its field semantics are reviewed.

### Step 5: Validate a batch before promotion

Validate candidate JSONL without persisting anything:

```powershell
cargo run -p guide-maintenance -- validate-batch .local/research/local-build/candidates/all.jsonl
```

This deserializes each line into a `KnowledgeRecord`, runs `KnowledgeStore` validation, and reports structural, provenance, and reference errors. Fix every error before promoting.

### Step 6: Promote reviewed candidates to canonical data

Move reviewed records into the appropriate classified JSONL file under `data/reviewed/`:

- Items go to `data/reviewed/items.jsonl`
- Pals go to `data/reviewed/pals.jsonl`
- Aliases go to `data/reviewed/aliases.jsonl`
- Recipes, technologies, habitats, breeding rules, progression relationships, and conflicts go to their corresponding files
- The legacy mixed seed set remains in `data/reviewed/facts.jsonl`

Compare existing canonical values field by field: exact matches add corroboration, partial matches retain the reviewed representation, and disagreements create `ConflictRecord` entries rather than silently overwriting.

### Step 7: Re-run version and validation checks

After promotion, confirm the canonical store is consistent:

```powershell
cargo run -p guide-maintenance -- version-check --data data/reviewed --game-version 1.0.3
```

Also run the full workspace gates before committing:

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

## 2. Source Log Format

Every source is registered in `docs/reference-data/source-log.md` as one row with these columns:

| Column | Purpose |
|---|---|
| ID | Stable source identifier, e.g. `SRC-LOCAL-BUILD-24575825-20260902` |
| Title | Short description of what was reviewed |
| Supplier | Who supplied or performed the retrieval |
| Retrieved | `YYYY-MM-DD` retrieval or extraction date |
| URL or local path | HTTP(S) URL or reviewed `local://` path |
| Palworld version | Claimed applicable version |
| Scope | What the source covers (items, recipes, Pals, etc.) |
| Confidence | `verified-target`, `official`, `reviewed-secondary`, `community`, `conflicted`, or `unknown` |
| Review status | `reviewed`, `candidate`, etc. |
| Notes | Mapping version, manifest hash, limitations, deferred rows |

### Source Priority

Lower numbers override higher numbers. A lower-priority source cannot silently override a higher-priority source.

| Priority | Source | Confidence |
|---|---|---|
| 1 | Local target-build data | `verified-target` |
| 2 | Official patch notes or documentation | `official` |
| 3 | User-reviewed secondary sources | `reviewed-secondary` |
| 4 | Community wikis and databases | `community` |
| 5 | LLM-derived hypotheses | never persisted as facts |

Only `verified-target`, `official`, and `reviewed-secondary` records may support normal guide answers. Community records require an explicit caveat. Conflicted and unknown records must not be presented as settled facts.

## 3. Conflict Handling

When two registered sources disagree on a field, the disagreement is recorded as a `ConflictRecord` rather than silently resolved.

### ConflictRecord Fields

| Field | Description |
|---|---|
| `id` | Stable conflict identifier |
| `subject_id` | The fact the conflict is about |
| `field` | The specific field in dispute |
| `values` | Two or more competing values |
| `source_ids` | One or more registered evidence sources |
| `resolution` | `unresolved` or `resolved` |

Conflicts are stored in `conflicts.jsonl` or inline in `facts.jsonl`. Loading never silently selects one competing value; both remain visible in lookups, material calculations, and planner output.

### Audit Conflicts

List all conflicts with their resolution status:

```powershell
cargo run -p guide-maintenance -- audit-conflicts --data data/reviewed
```

The output reports total conflicts, unresolved count, and resolved count. Resolve a conflict only through explicit review and record the resolution in the conflict record.

## 4. Version Tracking

### Per-Fact Provenance

Every fact carries an inline provenance object that includes `applicable_game_version`. This version must exactly match the registered source's version. The knowledge base version is the unique game version across all records.

### Mixed Versions

If the canonical store contains records with different `applicable_game_version` values, the version check reports `knowledge_version: "mixed"` and emits a warning. Mixed versions signal that the knowledge base spans multiple game builds and may contain stale facts.

### Version Compatibility Check

```powershell
cargo run -p guide-maintenance -- version-check --data data/reviewed --game-version 1.0.3
```

The report covers:

- `knowledge_version` — the unique game version across all records, or `mixed`
- `configured_game_version` — the version passed with `--game-version` (or `null`)
- `index_version` — the built retrieval index version
- `embedding_version` — `null` (semantic search is deferred)
- `matches` — whether the knowledge version matches the configured game version
- `warnings` — mismatch, mixed-version, or stale-record warnings

When the configured game version does not match the knowledge version, answers include a version-mismatch uncertainty note.

## 5. guide-maintenance CLI Commands

The `guide-maintenance` binary provides auditing and validation. All commands accept `--data DIR` (default `data/reviewed`) and `--game-version VERSION` where noted.

| Command | Purpose |
|---|---|
| `version-check` | Report version compatibility across all dimensions |
| `audit-sources` | List all registered sources with confidence, review status, and fact counts |
| `audit-conflicts` | List all conflicts with resolution status |
| `audit-stale` | List records whose `applicable_game_version` does not match `--game-version` (requires `--game-version`) |
| `validate-batch FILE` | Validate JSONL records without persisting |

### Examples

```powershell
cargo run -p guide-maintenance -- version-check --data data/reviewed --game-version 1.0.3
cargo run -p guide-maintenance -- audit-sources --data data/reviewed
cargo run -p guide-maintenance -- audit-conflicts --data data/reviewed
cargo run -p guide-maintenance -- audit-stale --data data/reviewed --game-version 1.0.3
cargo run -p guide-maintenance -- validate-batch path/to/candidates.jsonl
```

Each command outputs pretty JSON. `version-check`, `audit-stale`, and `validate-batch` return a nonzero exit code when warnings, stale records, or validation errors are found.
