# Palworld Guider Map Knowledge Design

## Purpose

Add map knowledge to the guide's reviewed, versioned knowledge base so the agent can answer location-aware questions without inventing game facts. The target capabilities are:

1. Convert a player's world coordinate to the applicable map and, when authoritative boundary data is available, the current region name.
2. Find nearby fast-travel points, boss towers, Pal spawn zones, and later resource locations.
3. Given a target Pal or material, recommend nearby candidate locations, directions, distances, and a useful fast-travel anchor.
4. Keep every persisted location fact tied to the exact target game build and reviewed source.

Map data is guide knowledge, not a visual map feature. Map textures are not required and must not be committed.

## Current Evidence

The exact local target build already provides the primary map inputs:

- `DT_WorldMapUIData.json` defines the `MainMap` and `Tree` maps. `MainMap` bounds are `X=-1099400..349400` and `Y=-724400..724400`; both maps have a logical 8192-pixel size. Texture paths remain metadata only.
- `DT_WorldMapAreaData.json` contains 123 region row IDs and localization message IDs.
- English and Simplified Chinese `DT_WorldMap_Common_Text_Common.json` tables provide region names.
- `DT_PalSpawnerPlacement.json` contains 8,253 placement rows for `PL_MainWorld5`, with world location, radius, spawner type, placement type, layer names, and respawn metadata.
- `DT_PalWildSpawner.json` contains 1,691 spawner definitions with up to three Pal IDs, level ranges, group sizes, time/weather restrictions, and randomizer flags.
- English and Simplified Chinese `DT_MapRespawnPointInfoText.json` tables provide fast-travel and respawn labels.

The spawner tables associate through `SpawnerName`. Current local-build validation found:

- 8,182 of 8,253 placement rows map to a wild-spawner definition.
- 364 unique placement spawner names map successfully.
- 477 raw Pal or variant IDs are referenced.
- 71 placement rows remain unmatched and must be deferred or marked unknown rather than inferred.

The pending local-build export is `Pal/Content/Pal/Maps/MainWorld_5/PL_MainWorld5.json`. It is required for authoritative fast-travel and boss-tower actor locations. It may also reveal region boundary volumes or resource actors; those fields require review before use.

## Data Boundary

Persist map definitions and bounds, localized regions, named map points, Pal habitat zones, canonical links, deferred coverage, uncertainty, provenance, version, extraction date, review status, confidence, and relevant hashes.

Do not persist map textures, converted images, raw `PL_MainWorld5` exports, wholesale copyrighted text, community coordinates not tied to the target build, or guessed boundaries, spawn semantics, and material locations.

## Coordinate Model

Stored locations retain original Unreal world coordinates:

```text
x: Location.X
y: Location.Y
z: Location.Z
```

For `MainMap`, derive normalized coordinates as:

```text
u = (world_x + 1099400) / 1448800
v = (world_y + 724400) / 1448800
pixel_x = round(u * 8192)
pixel_y = round(v * 8192)
```

`Tree` uses its own bounds from `DT_WorldMapUIData` and must not reuse `MainMap` bounds. Derived values are recomputed from map definitions and tested; they are not a substitute for source coordinates.

Distance and bearing use Euclidean `X/Y` world units and are reported as approximate game-world distance and compass direction. `Z` is retained for validation and future terrain-aware routing, but is not added to surface-travel distance by default.

## Knowledge Records

### Map Definition

A map definition identifies one logical game map:

- Stable ID, such as `MAP_MAIN` or `MAP_TREE`.
- Native map/world name, such as `PL_MainWorld5`.
- Minimum and maximum `X/Y/Z` bounds.
- Logical size and units.
- Texture asset path retained as metadata only.
- Applicable map priority when relevant.
- Provenance and uncertainty.

### Map Region

A region record represents a named map area:

- Stable region ID derived from the DataTable row name.
- Owning map ID.
- English and Simplified Chinese names resolved through `MsgID`.
- Optional reviewed boundary geometry.
- Provenance and uncertainty.

If no reviewed boundary exists, the region remains searchable but cannot authoritatively answer “what region is this coordinate in?” Point-in-region questions fall back to the nearest reviewed landmark and explicitly identify the region as approximate.

### Map Point

A map point is a discrete named location:

- Stable ID.
- Reviewed kind: fast travel, boss tower, respawn point, settlement, dungeon entrance, or another explicitly reviewed kind.
- Map and world IDs.
- World coordinates.
- Localized names and aliases.
- Optional unlock or progression requirements.
- Provenance and uncertainty.

Fast-travel and boss-tower extraction from `PL_MainWorld5` follows reviewed actor relationships:

- Fast-travel actors: `BP_LevelObject_TowerFastTravelPoint_C`.
- Boss towers: `BP_PalBossTower_C`.
- Coordinates come from the matching component's `RelativeLocation`.
- Fast-travel labels resolve through `FastTravelPointID`.
- Boss labels resolve through reviewed boss-type localization.

### Pal Habitat Zone

The existing vague `habitat_ids` list is upgraded to references to structured map zones. A habitat-zone record contains:

- Stable zone ID.
- Owning map ID.
- Native spawner name and placement-row evidence.
- World coordinate and static radius.
- Placement kind: field, dungeon, dungeon boss, field boss, or imprisonment boss.
- Referenced raw Pal IDs normalized to canonical Pal IDs where reviewed.
- Optional preserved variant labels such as boss or dark variant.
- Level and group-size ranges.
- Time, weather, randomizer, and respawn constraints where available.
- Provenance and uncertainty.

`PalRecord.habitat_ids` points to these zone IDs. Lookup tools resolve referenced records; unresolved IDs are never presented to the model as place names.

Raw IDs such as `BOSS_*` and `*_Dark` are not silently collapsed into a base Pal. Each mapping is either reviewed and normalized to a canonical Pal with a preserved variant label, or retained as an explicit unresolved variant with uncertainty.

Unmatched spawner placements, including rows whose `SpawnerName` is `None`, remain in an audit artifact and are excluded from authoritative habitat answers until reviewed.

### Resource Location

Resource locations are a later record kind for ore, trees, eggs, chests, and other harvestable or collectible actors. They require exact actor or DataTable evidence from the target build. They are not inferred from Pal habitats or community maps.

## Agent Tools

Deterministic Rust tools are added after canonical records are available:

- `get_map_definition(map_id)`
- `locate_coordinate(world_x, world_y, world_z)`
- `get_map_region(region_id)`
- `find_nearby_map_points(coordinate, filters, limit)`
- `get_pal_habitats(pal_id)`
- `find_pal_spawn_zones(pal_id, coordinate, limit)`
- `suggest_map_route(origin, destination, preferences)`

Initial route guidance is conservative:

- Return target bearing and approximate world distance.
- Return the nearest reviewed fast-travel point to origin and destination.
- Return candidate regions or landmarks.
- State that the result is not a terrain-verified walking path.

Turn-by-turn or obstacle-aware routing requires a separate reviewed traversability graph and must not be claimed by the initial implementation.

## Intake Flow

1. Register or extend the local-build source entry with the map and spawner scope, extraction date, mapping commit, and manifest hash.
2. Parse map UI, region, and localization DataTables into candidate map definitions and regions.
3. Parse `PL_MainWorld5` into candidate fast-travel and boss-tower map points.
4. Join `DT_PalSpawnerPlacement` to `DT_PalWildSpawner` by `SpawnerName`.
5. Normalize raw Pal and variant references against canonical Pal records.
6. Emit candidate JSONL and an audit report under `.local/research/local-build/candidates/`.
7. Review coverage, unmatched rows, localization gaps, coordinate bounds, and variant mappings.
8. Validate candidates with `guide-maintenance validate-batch`.
9. Promote only reviewed records into canonical JSONL files.
10. Run the full workspace gates before commit.

## Staged Delivery

### M0: Map Foundation

Deliver map definitions, localized regions, fast-travel points, and boss towers. Add coordinate conversion and nearest-point lookup. Keep region answers approximate unless authoritative boundaries are found.

Acceptance:

- Map bounds and coordinate conversion are tested.
- Region localization is tested for English and Simplified Chinese.
- Fast-travel and boss-tower IDs, labels, and coordinates are reviewed.
- Nearest-point lookup returns deterministic distance and bearing.
- No raw texture or raw level asset enters Git.

### M1: Pal Habitat Mapping

Promote reviewed Pal habitat zones and update `PalRecord.habitat_ids`.

Acceptance:

- Every promoted zone has target-build provenance.
- Canonical Pal links and preserved variants are reviewed.
- Unmatched placements and unresolved Pal variants are audited.
- Pal lookup can return nearby habitat zones from a supplied coordinate.
- Empty habitat lists disappear only for Pals with reviewed coverage; remaining gaps remain explicit.

### M2: Location-Aware Advice

Combine player state, map knowledge, Pal habitats, progression data, and later resource locations into prioritized advice.

Acceptance:

- Given a coordinate and target Pal, return ranked candidate zones with reasons.
- Given a coordinate and target material, answer only from reviewed resource records or explicitly return unknown.
- Recommendations distinguish observed state, user-entered state, assumptions, and unknowns.
- Route claims remain bounded to distance, bearing, landmarks, and fast-travel anchors until a traversability graph exists.

## Error And Uncertainty Rules

- Missing map: return `unknown`; do not silently choose another map.
- Coordinate outside reviewed bounds: return an out-of-bounds warning.
- Missing region boundary: return the nearest reviewed landmark with an approximate-region warning.
- Conflicting coordinates or names: expose a conflict record.
- Mixed game versions: propagate the version-mismatch warning.
- Unmatched spawner row: exclude from settled facts and include it in audit coverage.
- Unknown material location: return unknown rather than suggesting a visually similar location.

## Verification

Tests must cover:

- Map-bound normalization and `Tree` versus `MainMap` isolation.
- Round-trip conversion between world and normalized map coordinates.
- Region ID to English and Simplified Chinese localization.
- Fast-travel and boss-tower extraction from representative raw shapes.
- Nearest-point ordering, distance, and bearing.
- Pal placement to wild-spawner joins.
- Variant normalization and unresolved variants.
- Missing rows, `None` spawner names, malformed locations, and duplicate IDs.
- Canonical reference integrity for every habitat and map point.
- Version mismatch and uncertainty propagation.

Final implementation gates:

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets -D warnings
cargo test
cargo run -p guide-maintenance -- validate-batch <candidate-file>
cargo run -p guide-maintenance -- version-check --data data/reviewed --game-version 1.0.3
git diff --check
```
