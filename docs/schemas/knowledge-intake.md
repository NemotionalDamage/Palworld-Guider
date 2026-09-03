# Knowledge Intake Field Standard

This standard defines the fields required for each canonical Knowledge v1 record class. It complements the wire schema in `docs/schemas/knowledge-v1.md`: the Rust schema defines what can parse and validate, while this document defines what a complete, reviewable intake batch must contain.

## Storage And Classification

- Canonical reviewed records are JSONL objects under `data/reviewed/`.
- Every fact line is self-describing through `record_type`; a future batch may split fact lines by class, but loading and validation remain record-type based rather than filename based.
- Candidate output remains under gitignored `.local/research/local-build/candidates/`. A candidate may use `review_status: candidate` and explicit `unresolved` values, but neither may enter canonical reviewed data until its field semantics are reviewed.
- A field is either structurally required, conditionally required, or explicitly unknown. Silence is not uncertainty: an unknown value must use the schema null/empty form plus `local_evidence.unresolved_fields` or a provenance change-risk note.

## Common Fields

Every fact has `record_type`, a stable unique `id`, and `provenance`. Provenance must reference a registered source and exactly match its version, retrieval date, reviewer, review status, and confidence. `change_risk` is required when a patch can change the fact or semantics remain uncertain. `corroborating_source_ids` is optional and may contain only registered sources supporting the same reviewed value.

Local-build Item, Pal, Technology, and Recipe records additionally require:

- `native_row_id`: nonempty native DataTable row ID, unique within the record class.
- `local_evidence.source_table`: exact DataTable path or clear table name.
- `local_evidence.localization_status`: `resolved`, `partial`, `missing`, or `not_applicable`.
- `local_evidence.unresolved_fields`: every semantic field left null, empty, or marked unresolved.
- `local_evidence.transformation_notes`: the reviewed native-to-Knowledge-v1 transformation.

## Source

Required fields are `id`, `title`, `supplier`, `retrieved_on`, nonempty `evidence_urls`, `applicable_game_version`, `reviewer`, `review_status`, and `confidence`. Dates use `YYYY-MM-DD`; evidence URLs use HTTP(S) or a reviewed `local://` path. Versions use dot-separated numeric segments with at least two segments. `notes` is optional and should record review scope, mapping version, manifest hash, and durable limitations.

## Item

- Required: `names.en`, `rarity`, and common provenance. `rarity` may be `unknown` only when its semantics are explicitly unresolved.
- Conditionally required: `names.zh_hans` when target-build localization resolves it; `description` when reviewed source text is available; and at least one direct `acquisition_leads` entry, a resolvable recipe/drop relation, or explicit unknown.
- Local build: `native_row_id` and complete `local_evidence`.

## Pal

- Required: `names.en` and common provenance.
- Conditionally required: `names.zh_hans` when localization resolves; reviewed `stats`; only confirmed `work_suitability` entries with levels 1–5; reviewed `drops`; and `habitat_ids` resolving to Habitat records.
- Null stats, empty work suitability, empty drops, or empty habitats is valid only as explicit uncertainty or a reviewed negative statement.
- Local build: `native_row_id` and complete `local_evidence`.

## Recipe

- Required: positive `output`, at least one positive ingredient, at least one reviewed crafting station, and common provenance.
- Conditionally required: `technology_id` when reviewed; finite positive `crafting_seconds` when reviewed; and reviewed byproduct semantics. An empty byproduct list means no byproducts only after review.
- `crafting_stations: ["unresolved"]` is valid only in candidate data and blocks canonical promotion.
- Local build: `native_row_id` and complete `local_evidence`.

## Technology

- Required: `names.en`, positive reviewed `level`, and common provenance.
- Conditionally required: `names.zh_hans` when localization resolves.
- Local build: `native_row_id` and complete `local_evidence`.

## Habitat

- Required: `names.en` and common provenance.
- Conditionally required: `names.zh_hans` when reviewed localization resolves and `pal_ids` resolving to Pal records.
- An empty `pal_ids` list requires explicit unknown or a reviewed statement that no Pals use the habitat.
- Map points, regions, fast-travel points, boss locations, coordinates, and resource locations are not Habitat records. They require a separate reviewed map schema before ingestion.

## Breeding Rule

- Required: `parent_a_id`, `parent_b_id`, and `child_id`, each resolving to a Pal record, plus common provenance.
- Optional: reviewed special-case or uncertainty `notes`.

## Alias

- Required: nonempty `alias`, `target_id` resolving to the intended existing fact, `locale` set to `en` or `zh_hans`, and common provenance.
- A localized alias may equal the target's localized canonical name when needed for current locale resolution.
- English colloquial aliases must be independently reviewed and must not silently override a canonical name.

## Progression Relationship

- Required: `from_id`, `to_id`, typed `relation`, and common provenance.
- Conditionally required: nonempty reviewed `requirement` text when the edge has a threshold.
- Null `requirement` means no additional requirement, not unknown.

## Conflict

- Required: `subject_id`, nonempty `field`, at least two nonempty `values`, at least two registered `source_ids`, typed `resolution`, and common provenance.
- Conflicts remain visible. Loading never silently selects one competing value.

## Promotion Checklist

Before a candidate record becomes canonical:

1. Its source is registered and provenance exactly matches that source.
2. Every structurally and conditionally required field is present or explicitly unknown.
3. Every entity reference resolves to the required record type.
4. Every unresolved or unknown field names its semantic blocker in `local_evidence.unresolved_fields`.
5. Existing canonical values are compared field by field: exact matches add corroboration, partial matches retain the reviewed representation, and disagreements create Conflict records.
6. No game fact is inferred from an LLM, community search snippet, or undocumented field semantics.
