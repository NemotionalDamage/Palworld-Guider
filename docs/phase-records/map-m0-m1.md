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
| Pal habitat zones | 8,164 |
| Canonical Pals with structured habitats | 164 |

The intake report explicitly excludes 71 unmatched placements and 58 unresolved raw Pal references. No unresolved row was silently converted into a location fact.

## Implementation

- Added map-definition, region, point, shape, coordinate, and Pal-habitat-zone schemas with store validation and reference integrity.
- Added deterministic local map intake, candidate generation, audit reporting, and reviewed promotion tools.
- Added coordinate-to-map normalization, deterministic nearest-point ordering, compass bearing, and Pal-zone ranking.
- Registered `locate_coordinate`, `find_nearby_map_points`, and `find_pal_spawn_zones` in the typed guide-tool registry.
- Updated `PalRecord.habitat_ids` only where a canonical Pal resolved from target-build spawner data; remaining coverage gaps continue to report `habitat_ids` as unresolved.

## Uncertainty Boundary

Region boundary geometry was not reviewed, so coordinate-to-region answers remain approximate. Route suggestions are not claimed or implemented because no terrain-verified traversability graph exists. Resource/material locations remain unknown pending a separate reviewed intake.

## Verification

- Candidate intake was validated in canonical context before promotion: 17,479 records, valid.
- Canonical `version-check --game-version 1.0.3` passes with no warnings.
- Full formatting, Clippy, workspace tests, and whitespace checks were run at closeout.
