# Map Foundation And Pal Habitat Mapping Record

## Scope

Implemented the M0 map foundation and M1 Pal habitat mapping boundaries from `PALMAP_PROMPT.md` against exact local Steam Build 24575825 exports. No texture, converted image, raw level asset, `_Generated_` bulk export, or guessed resource location was committed.

## Source And Evidence

- Registered `SRC-LOCAL-BUILD-MAP-24575825-20260906` with the five targeted exports and SHA-256 hashes.
- Parsed `DT_WorldMapUIData`, `DT_WorldMapAreaData`, localized world-map/fast-travel/UI tables, `DT_PalSpawnerPlacement`, `DT_PalWildSpawner`, and `PL_MainWorld5`.
- Kept raw exports under `.local/research/local-build/raw`; candidate and audit artifacts remain outside Git under `.local/research/local-build/candidates`.

## Promoted Coverage

| Fact family | Count |
|---|---:|
| Logical maps | 2 |
| Localized regions | 123 |
| Fast-travel points | 152 |
| Boss towers | 8 |
| Joined spawner placements | 8,182 |
| Pal habitat zones | 67,952 |
| Canonical Pals with structured habitats | 282 of 299 |

The intake report explicitly excludes 71 unmatched placements and 678 unresolved raw Pal references. No unresolved row was silently converted into a location fact.

## Habitat Completion Pass (2026-09-06)

A follow-up widened the M1 spawner join after the first promote left 135 Pals without `habitat_ids`. The root cause was that `DT_PalWildSpawner` stores one row per Pal slot for the same `SpawnerName`; the original join kept only one row per name, dropping most slot Pals. Fixes:

- Emit every wild-spawner row for a placement name; zone IDs now include the spawner row key so multi-row names stay unique.
- Added a supplementary pass that resolves unplaced spawners through `DT_BossSpawnerLoactionData` by `CharacterID`, then falls back to biome-area coordinate estimation for named area spawners.
- Invalid inverted level ranges from the source (for example `LvMin_2=35, LvMax_2=34`) are skipped instead of persisted.
- `promote-map-intake` now replaces reviewed map files with the generated set and skips empty record families, preventing stale or duplicated canonical state.
- Habitat backfill also updates legacy Pal records that live in `facts.jsonl` (for example the Lamball record, `PAL_LAMBALL`, whose canonical identity lives there); untouched lines are preserved byte-for-byte.
- Integration fixtures in `guide-agent`, `guide-planner`, `guide-tools`, and `guide-regression` load only `sources.jsonl` and `facts.jsonl`, so they now share a helper that appends the referenced habitat-zone and map records to keep the conflict-free test store valid.
- Remaining 17 uncovered Pals are raid bosses, tower or legendary encounters, or breeding-limited variants with no wild field spawner.

## Implementation

- Added map-definition, region, point, shape, coordinate, and Pal-habitat-zone schemas with store validation and reference integrity.
- Added deterministic local map intake, candidate generation, audit reporting, and reviewed promotion tools.
- Added coordinate-to-map normalization, deterministic nearest-point ordering, compass bearing, and Pal-zone ranking.
- Registered `locate_coordinate`, `find_nearby_map_points`, and `find_pal_spawn_zones` in the typed guide-tool registry.
- Updated `PalRecord.habitat_ids` only where a canonical Pal resolved from target-build spawner data; remaining coverage gaps continue to report `habitat_ids` as unresolved.

## Uncertainty Boundary

Region boundary geometry was not reviewed, so coordinate-to-region answers remain approximate. Route suggestions are not claimed or implemented because no terrain-verified traversability graph exists. Resource/material locations remain unknown pending a separate reviewed intake.

## Verification

- Candidate intake `map-foundation-v1.jsonl` (68,237 records: 2 maps, 123 regions, 160 points, 67,952 habitat zones) validates in canonical context before promotion.
- Canonical `version-check --game-version 1.0.3` passes with no warnings.
- Full formatting, Clippy, workspace tests (all green), and whitespace checks were re-run after the 2026-09-06 habitat completion pass.
