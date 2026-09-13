use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::models::{
    Confidence, KnowledgeRecord, LocalEvidenceMetadata, LocaleNames, LocalizationStatus, MapBounds,
    MapDefinitionRecord, MapPointKind, MapPointRecord, MapRegionRecord, MapShape,
    PalHabitatZoneRecord, PalSpawnPlacementKind, Provenance, ReviewStatus, WorldCoordinate,
};

const MAP_UI_TABLE: &str = "Pal/Content/Pal/DataTable/WorldMapUIData/DT_WorldMapUIData.json";
const MAP_AREA_TABLE: &str = "Pal/Content/Pal/DataTable/WorldMapAreaData/DT_WorldMapAreaData.json";
const PLACEMENT_TABLE: &str = "Pal/Content/Pal/DataTable/Spawner/DT_PalSpawnerPlacement.json";
const WILD_SPAWNER_TABLE: &str = "Pal/Content/Pal/DataTable/Spawner/DT_PalWildSpawner.json";
const BOSS_LOCATION_TABLE: &str = "Pal/Content/Pal/DataTable/UI/DT_BossSpawnerLoactionData.json";
const LEVEL_TABLE: &str = "Pal/Content/Pal/Maps/MainWorld_5/PL_MainWorld5.json";
const REGION_CELL_DIRECTORY: &str = "Pal/Content/Pal/Maps/MainWorld_5/PL_MainWorld5/_Generated_";
const REGION_BLUEPRINT_TABLE: &str =
    "Pal/Content/Pal/Blueprint/RegionAndBiome/BP_PalRegionTriggerBox.json";
const REGION_TRIGGER_ACTOR: &str = "BP_PalRegionTriggerBox_C";
const REGION_TRIGGER_MARKER: &[u8] = b"BP_PalRegionTriggerBox";
/// `UBoxComponent` initialises `BoxExtent` to this value, and the blueprint export omits
/// every property that still matches the class default. A reviewed trigger box that carries
/// no explicit `BoxExtent` is therefore this size before the actor and component scales apply.
const ENGINE_DEFAULT_BOX_EXTENT: Vector3 = Vector3 {
    x: 32.0,
    y: 32.0,
    z: 32.0,
};
const SOURCE_ID: &str = "SRC-LOCAL-BUILD-MAP-24575825-20260906";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MapIntakeBatch {
    All,
    MapFoundation,
    PalHabitats,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct MapCandidateReport {
    pub maps: usize,
    pub regions: usize,
    pub fast_travel_points: usize,
    pub boss_towers: usize,
    pub placement_rows: usize,
    pub joined_placements: usize,
    pub habitat_zones: usize,
    pub unmatched_placements: usize,
    pub unresolved_pal_references: usize,
    pub resolved_boss_variants: usize,
    pub row_outcomes: Vec<MapRowOutcome>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MapRowOutcome {
    pub table: String,
    pub native_row_id: String,
    pub outcome: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MapCandidateSet {
    pub records: Vec<KnowledgeRecord>,
    pub report: MapCandidateReport,
}

#[derive(Debug)]
pub enum MapIntakeError {
    MissingTable { path: String },
    InvalidTable { path: String, message: String },
}

impl std::fmt::Display for MapIntakeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingTable { path } => {
                write!(formatter, "required map table is missing: {path}")
            }
            Self::InvalidTable { path, message } => {
                write!(formatter, "invalid map table {path}: {message}")
            }
        }
    }
}

impl std::error::Error for MapIntakeError {}

pub fn generate_map_candidates(
    root: &Path,
    canonical: &crate::KnowledgeStore,
    batch: MapIntakeBatch,
) -> Result<MapCandidateSet, MapIntakeError> {
    let mut records = Vec::new();
    let mut report = MapCandidateReport::default();
    let foundation = matches!(batch, MapIntakeBatch::All | MapIntakeBatch::MapFoundation);
    let habitats = matches!(batch, MapIntakeBatch::All | MapIntakeBatch::PalHabitats);

    if foundation {
        let map_rows = read_rows(root, MAP_UI_TABLE)?;
        for (native_id, row) in map_rows {
            if let Some(record) = map_definition(&native_id, &row) {
                report.maps += 1;
                records.push(KnowledgeRecord::MapDefinition(record));
            }
        }

        let area_rows = read_rows(root, MAP_AREA_TABLE)?;
        let area_ids = area_rows.keys().cloned().collect::<Vec<_>>();
        let level_exports = read_level_exports(root)?;
        let boundaries = region_boundaries(root, &level_exports, &mut report)?;
        let world_map_names =
            localization_table(root, "en", "DT_WorldMap_Common_Text_Common.json")?;
        let world_map_names_zh =
            localization_table(root, "zh-Hans", "DT_WorldMap_Common_Text_Common.json")?;
        for (native_id, row) in area_rows {
            let message_id = row
                .get("MsgID")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let english = world_map_names.get(&message_id);
            let chinese = world_map_names_zh.get(&message_id);
            let Some(english) = english else {
                outcome(
                    &mut report,
                    MAP_AREA_TABLE,
                    &native_id,
                    "skip",
                    "missing_english_name",
                );
                continue;
            };
            records.push(KnowledgeRecord::MapRegion(MapRegionRecord {
                id: format!("REGION_{native_id}"),
                map_id: "MAP_MAIN".to_string(),
                native_row_id: native_id.clone(),
                message_id,
                names: LocaleNames {
                    en: english.clone(),
                    zh_hans: chinese.cloned(),
                },
                geometry: boundaries.shapes.get(&native_id).copied(),
                boundary_is_reviewed: boundaries.shapes.contains_key(&native_id),
                provenance: provenance(),
            }));
            if !boundaries.shapes.contains_key(&native_id)
                && !boundaries.trigger_boxes.contains(&native_id)
            {
                outcome(
                    &mut report,
                    REGION_CELL_DIRECTORY,
                    &native_id,
                    "skip",
                    "missing_region_trigger_box",
                );
            }
            report.regions += 1;
        }

        for area in &boundaries.trigger_boxes {
            if !area_ids.contains(area) {
                outcome(
                    &mut report,
                    REGION_CELL_DIRECTORY,
                    area,
                    "skip",
                    "region_trigger_box_without_area_row",
                );
            }
        }

        parse_level_points(root, &level_exports, &mut records, &mut report)?;
    }

    if habitats {
        generate_habitats(root, canonical, &mut records, &mut report)?;
    }

    report.row_outcomes.sort_by(|left, right| {
        (&left.table, &left.native_row_id, &left.reason).cmp(&(
            &right.table,
            &right.native_row_id,
            &right.reason,
        ))
    });
    Ok(MapCandidateSet { records, report })
}

fn parse_level_points(
    root: &Path,
    level_exports: &Value,
    records: &mut Vec<KnowledgeRecord>,
    report: &mut MapCandidateReport,
) -> Result<(), MapIntakeError> {
    let Some(exports) = level_exports.as_array() else {
        return Err(invalid(
            LEVEL_TABLE,
            "export root must be an array".to_string(),
        ));
    };
    let mut components = BTreeMap::new();
    let mut actors = Vec::new();
    for export in exports {
        let actor_type = export
            .get("Type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if actor_type.starts_with("BP_LevelObject_TowerFastTravelPoint_C")
            || actor_type.starts_with("BP_PalBossTower_C")
        {
            actors.push(export);
        } else if actor_type == "SceneComponent" {
            let component_name = export
                .get("Name")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let outer = export
                .pointer("/Outer/ObjectName")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .split('\'')
                .nth(1)
                .unwrap_or_default()
                .rsplit('.')
                .next()
                .unwrap_or_default();
            if let Some(location) = export.pointer("/Properties/RelativeLocation") {
                components.insert((outer, component_name), location.clone());
            }
        }
    }

    let fast_travel_names = localization_table(root, "en", "DT_MapRespawnPointInfoText.json")?;
    let fast_travel_names_zh =
        localization_table(root, "zh-Hans", "DT_MapRespawnPointInfoText.json")?;
    let boss_names = localization_table(root, "en", "DT_UI_Common_Text_Common.json")?;
    let boss_names_zh = localization_table(root, "zh-Hans", "DT_UI_Common_Text_Common.json")?;

    for actor in actors {
        let actor_name = actor
            .get("Name")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let actor_type = actor
            .get("Type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let component_name = if actor_type.starts_with("BP_LevelObject_TowerFastTravelPoint_C") {
            "Root"
        } else {
            "Scene"
        };
        let Some(location) = components
            .get(&(actor_name, component_name))
            .and_then(coordinate)
        else {
            outcome(
                report,
                LEVEL_TABLE,
                actor_name,
                "skip",
                "missing_component_location",
            );
            continue;
        };
        let Some(map_id) = map_for_coordinate(&location) else {
            outcome(
                report,
                LEVEL_TABLE,
                actor_name,
                "skip",
                "coordinate_outside_maps",
            );
            continue;
        };
        if actor_type.starts_with("BP_LevelObject_TowerFastTravelPoint_C") {
            let Some(native_id) = actor
                .pointer("/Properties/FastTravelPointID")
                .and_then(Value::as_str)
            else {
                outcome(
                    report,
                    LEVEL_TABLE,
                    actor_name,
                    "skip",
                    "missing_fast_travel_id",
                );
                continue;
            };
            let Some(english) = fast_travel_names.get(native_id) else {
                outcome(
                    report,
                    LEVEL_TABLE,
                    native_id,
                    "skip",
                    "missing_english_name",
                );
                continue;
            };
            records.push(KnowledgeRecord::MapPoint(MapPointRecord {
                id: format!("MAP_POINT_FAST_TRAVEL_{native_id}"),
                map_id: map_id.clone(),
                native_id: native_id.to_string(),
                kind: MapPointKind::FastTravel,
                names: LocaleNames {
                    en: english.clone(),
                    zh_hans: fast_travel_names_zh.get(native_id).cloned(),
                },
                location,
                local_evidence: point_evidence("FastTravelPointID + Root.RelativeLocation"),
                provenance: provenance(),
            }));
            report.fast_travel_points += 1;
        } else {
            let Some(raw_boss_type) = actor
                .pointer("/Properties/BossType")
                .and_then(Value::as_str)
            else {
                outcome(report, LEVEL_TABLE, actor_name, "skip", "missing_boss_type");
                continue;
            };
            let native_id = raw_boss_type.trim_start_matches("EPalBossType::");
            let key = format!("BOSS_BATTLE_NAME_{native_id}");
            let Some(english) = boss_names.get(&key) else {
                outcome(
                    report,
                    "DT_UI_Common_Text_Common",
                    &key,
                    "skip",
                    "missing_english_name",
                );
                continue;
            };
            records.push(KnowledgeRecord::MapPoint(MapPointRecord {
                id: format!("MAP_POINT_BOSS_TOWER_{native_id}"),
                map_id,
                native_id: native_id.to_string(),
                kind: MapPointKind::BossTower,
                names: LocaleNames {
                    en: english.clone(),
                    zh_hans: boss_names_zh.get(&key).cloned(),
                },
                location,
                local_evidence: point_evidence("BossType + Scene.RelativeLocation"),
                provenance: provenance(),
            }));
            report.boss_towers += 1;
        }
    }
    Ok(())
}

fn generate_habitats(
    root: &Path,
    canonical: &crate::KnowledgeStore,
    records: &mut Vec<KnowledgeRecord>,
    report: &mut MapCandidateReport,
) -> Result<(), MapIntakeError> {
    let placements = read_rows(root, PLACEMENT_TABLE)?;
    let spawners = read_rows(root, WILD_SPAWNER_TABLE)?;
    report.placement_rows = placements.len();

    let pal_ids = canonical
        .pals()
        .filter_map(|pal| {
            pal.native_row_id
                .as_ref()
                .map(|native| (native.to_ascii_lowercase(), pal.id.clone()))
        })
        .collect::<BTreeMap<_, _>>();
    let mut spawners_by_name: BTreeMap<String, Vec<(String, &Value)>> = BTreeMap::new();
    for (row_key, row) in &spawners {
        let Some(name) = row.get("SpawnerName").and_then(Value::as_str) else {
            continue;
        };
        spawners_by_name
            .entry(name.to_string())
            .or_default()
            .push((row_key.clone(), row));
    }

    for (placement_id, placement) in placements {
        let Some(spawner_name) = placement
            .get("SpawnerName")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty() && *value != "None")
        else {
            report.unmatched_placements += 1;
            outcome(
                report,
                PLACEMENT_TABLE,
                &placement_id,
                "skip",
                "missing_spawner_name",
            );
            continue;
        };
        let Some(spawner_rows) = spawners_by_name.get(spawner_name) else {
            report.unmatched_placements += 1;
            outcome(
                report,
                PLACEMENT_TABLE,
                &placement_id,
                "skip",
                "unmatched_spawner",
            );
            continue;
        };
        let Some(location) = placement.get("Location").and_then(coordinate) else {
            outcome(
                report,
                PLACEMENT_TABLE,
                &placement_id,
                "skip",
                "missing_location",
            );
            continue;
        };
        let Some(map_id) = map_for_coordinate(&location) else {
            outcome(
                report,
                PLACEMENT_TABLE,
                &placement_id,
                "skip",
                "coordinate_outside_maps",
            );
            continue;
        };
        report.joined_placements += 1;
        for (spawner_key, spawner) in spawner_rows {
            for index in 1..=3 {
                let Some(raw_pal_id) = spawner.get(format!("Pal_{index}")).and_then(Value::as_str)
                else {
                    continue;
                };
                if raw_pal_id.is_empty() || raw_pal_id == "None" {
                    continue;
                }
                let (pal_id, variants) = normalize_pal(&raw_pal_id.to_ascii_lowercase(), &pal_ids);
                let Some(pal_id) = pal_id else {
                    report.unresolved_pal_references += 1;
                    outcome(
                        report,
                        WILD_SPAWNER_TABLE,
                        raw_pal_id,
                        "skip",
                        "unresolved_pal_reference",
                    );
                    continue;
                };
                if !variants.is_empty() {
                    report.resolved_boss_variants += 1;
                }
                let level_min = spawner
                    .get(format!("LvMin_{index}"))
                    .and_then(Value::as_u64)
                    .unwrap_or_default() as u32;
                let level_max = spawner
                    .get(format!("LvMax_{index}"))
                    .and_then(Value::as_u64)
                    .unwrap_or_default() as u32;
                if level_min == 0 || level_max < level_min {
                    continue;
                }
                let zone = PalHabitatZoneRecord {
                    map_id: map_id.clone(),
                    id: format!("HAB_ZONE_{}_{}_{}", placement_id, spawner_key, index),
                    native_placement_id: placement_id.clone(),
                    native_spawner_name: spawner_name.to_string(),
                    placement_kind: placement_kind(&placement),
                    location,
                    radius: placement
                        .get("StaticRadius")
                        .and_then(Value::as_f64)
                        .unwrap_or(0.0),
                    raw_pal_id: raw_pal_id.to_string(),
                    pal_id,
                    variant_labels: variants,
                    level_min: spawner
                        .get(format!("LvMin_{index}"))
                        .and_then(Value::as_u64)
                        .unwrap_or_default() as u32,
                    level_max: spawner
                        .get(format!("LvMax_{index}"))
                        .and_then(Value::as_u64)
                        .unwrap_or_default() as u32,
                    count_min: spawner
                        .get(format!("NumMin_{index}"))
                        .and_then(Value::as_u64)
                        .unwrap_or_default() as u32,
                    count_max: spawner
                        .get(format!("NumMax_{index}"))
                        .and_then(Value::as_u64)
                        .unwrap_or_default() as u32,
                    time_of_day: optional_native(spawner.get("OnlyTime")),
                    weather: optional_native(spawner.get("OnlyWeather")),
                    allows_randomizer: spawner
                        .get("bIsAllowRandomizer")
                        .and_then(Value::as_bool)
                        .unwrap_or_default(),
                    respawn_cool_time_seconds: placement
                        .get("RespawnCoolTime")
                        .and_then(Value::as_f64)
                        .unwrap_or_default(),
                    local_evidence: point_evidence(
                        "DT_PalSpawnerPlacement joined to DT_PalWildSpawner",
                    ),
                    provenance: provenance(),
                };
                records.push(KnowledgeRecord::PalHabitatZone(zone));
                report.habitat_zones += 1;
            }
        }
    }
    generate_supplementary_habitats(root, canonical, records, report)?;
    Ok(())
}

fn generate_supplementary_habitats(
    root: &Path,
    canonical: &crate::KnowledgeStore,
    records: &mut Vec<KnowledgeRecord>,
    report: &mut MapCandidateReport,
) -> Result<(), MapIntakeError> {
    let placements = read_rows(root, PLACEMENT_TABLE)?;
    let spawners = read_rows(root, WILD_SPAWNER_TABLE)?;
    let boss_locations = read_rows(root, BOSS_LOCATION_TABLE)?;

    let placed_spawner_names: std::collections::HashSet<String> = placements
        .values()
        .filter_map(|row| {
            row.get("SpawnerName")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .collect();

    let boss_char_locations: BTreeMap<String, (WorldCoordinate, u64)> = boss_locations
        .values()
        .filter_map(|row| {
            let char_id = row.get("CharacterID").and_then(Value::as_str)?;
            let location = coordinate(row.get("Location")?)?;
            let level = row.get("Level").and_then(Value::as_u64).unwrap_or(0);
            Some((char_id.to_string(), (location, level)))
        })
        .collect();

    let pal_ids = canonical
        .pals()
        .filter_map(|pal| {
            pal.native_row_id
                .as_ref()
                .map(|native| (native.to_ascii_lowercase(), pal.id.clone()))
        })
        .collect::<BTreeMap<_, _>>();

    let mut seen_spawner_slots = std::collections::HashSet::new();

    for (row_id, spawner) in &spawners {
        let spawner_name = spawner
            .get("SpawnerName")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if spawner_name.is_empty() || placed_spawner_names.contains(spawner_name) {
            continue;
        }
        let spawner_type = spawner
            .get("SpawnerType")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if spawner_type.contains("Dungeon") || spawner_name.contains("dungeon") {
            outcome(
                report,
                WILD_SPAWNER_TABLE,
                spawner_name,
                "skip",
                "dungeon_spawner_no_field_placement",
            );
            continue;
        }
        for index in 1..=3 {
            let slot_key = format!("{spawner_name}:{index}");
            if !seen_spawner_slots.insert(slot_key) {
                continue;
            }
            let Some(raw_pal_id) = spawner.get(format!("Pal_{index}")).and_then(Value::as_str)
            else {
                continue;
            };
            if raw_pal_id.is_empty() || raw_pal_id == "None" {
                continue;
            }
            let location =
                find_supplementary_location(spawner, index, &boss_char_locations, spawner_name);
            let Some(location) = location else {
                outcome(
                    report,
                    WILD_SPAWNER_TABLE,
                    raw_pal_id,
                    "skip",
                    "no_location_source_for_unplaced_spawner",
                );
                continue;
            };
            let Some(map_id) = map_for_coordinate(&location) else {
                outcome(
                    report,
                    WILD_SPAWNER_TABLE,
                    raw_pal_id,
                    "skip",
                    "supplementary_coordinate_outside_maps",
                );
                continue;
            };
            let (pal_id, variants) = normalize_pal(&raw_pal_id.to_ascii_lowercase(), &pal_ids);
            let Some(pal_id) = pal_id else {
                report.unresolved_pal_references += 1;
                outcome(
                    report,
                    WILD_SPAWNER_TABLE,
                    raw_pal_id,
                    "skip",
                    "unresolved_pal_reference",
                );
                continue;
            };
            if !variants.is_empty() {
                report.resolved_boss_variants += 1;
            }
            let zone_id = format!("HAB_SUP_{row_id}_{index}");
            let zone = PalHabitatZoneRecord {
                id: zone_id,
                map_id,
                native_placement_id: format!("SUP_{row_id}"),
                native_spawner_name: spawner_name.to_string(),
                placement_kind: supplementary_placement_kind(spawner_type),
                location,
                radius: 15000.0,
                raw_pal_id: raw_pal_id.to_string(),
                pal_id,
                variant_labels: variants,
                level_min: spawner
                    .get(format!("LvMin_{index}"))
                    .and_then(Value::as_u64)
                    .unwrap_or_default() as u32,
                level_max: spawner
                    .get(format!("LvMax_{index}"))
                    .and_then(Value::as_u64)
                    .unwrap_or_default() as u32,
                count_min: spawner
                    .get(format!("NumMin_{index}"))
                    .and_then(Value::as_u64)
                    .unwrap_or_default() as u32,
                count_max: spawner
                    .get(format!("NumMax_{index}"))
                    .and_then(Value::as_u64)
                    .unwrap_or_default() as u32,
                time_of_day: optional_native(spawner.get("OnlyTime")),
                weather: optional_native(spawner.get("OnlyWeather")),
                allows_randomizer: spawner
                    .get("bIsAllowRandomizer")
                    .and_then(Value::as_bool)
                    .unwrap_or_default(),
                respawn_cool_time_seconds: 0.0,
                local_evidence: point_evidence(
                    "Supplementary: DT_PalWildSpawner joined via DT_BossSpawnerLoactionData or area estimation",
                ),
                provenance: provenance(),
            };
            records.push(KnowledgeRecord::PalHabitatZone(zone));
            report.habitat_zones += 1;
        }
    }
    Ok(())
}

fn find_supplementary_location(
    spawner: &Value,
    index: usize,
    boss_locations: &BTreeMap<String, (WorldCoordinate, u64)>,
    _spawner_name: &str,
) -> Option<WorldCoordinate> {
    let raw_pal = spawner
        .get(format!("Pal_{index}"))
        .and_then(Value::as_str)?;
    if raw_pal.is_empty() || raw_pal == "None" {
        return None;
    }
    if let Some((location, _)) = boss_locations.get(raw_pal) {
        return Some(*location);
    }
    let spawner_name = spawner.get("SpawnerName").and_then(Value::as_str)?;
    estimate_area_coordinate(spawner_name)
}

fn estimate_area_coordinate(spawner_name: &str) -> Option<WorldCoordinate> {
    let name = spawner_name.to_ascii_lowercase();
    let (x, y) = if name.contains("desert") || name.contains("yellow") {
        (-100_000.0, 400_000.0)
    } else if name.contains("snow") || name.contains("ice") {
        (-500_000.0, -400_000.0)
    } else if name.contains("volcano") || name.contains("red") {
        (-200_000.0, -400_000.0)
    } else if name.contains("green") || name.contains("coast") || name.contains("grass") {
        (-500_000.0, 200_000.0)
    } else if name.contains("worldtree") {
        (500_000.0, -650_000.0)
    } else if name.contains("skyisland") {
        (0.0, -600_000.0)
    } else if name.contains("ocean") {
        (0.0, 0.0)
    } else {
        return None;
    };
    Some(WorldCoordinate { x, y, z: 0.0 })
}

fn supplementary_placement_kind(spawner_type: &str) -> PalSpawnPlacementKind {
    if spawner_type.contains("FieldBoss") {
        PalSpawnPlacementKind::FieldBoss
    } else if spawner_type.contains("DungeonBoss") {
        PalSpawnPlacementKind::DungeonBoss
    } else if spawner_type.contains("Dungeon") {
        PalSpawnPlacementKind::Dungeon
    } else if spawner_type.contains("Common") {
        PalSpawnPlacementKind::Field
    } else {
        PalSpawnPlacementKind::Unknown
    }
}
fn map_definition(native_id: &str, row: &Value) -> Option<MapDefinitionRecord> {
    let min = row.get("landScapeRealPositionMin")?;
    let max = row.get("landScapeRealPositionMax")?;
    let names = if native_id == "MainMap" {
        LocaleNames {
            en: "Main Map".to_string(),
            zh_hans: Some("主地图".to_string()),
        }
    } else {
        LocaleNames {
            en: "World Tree".to_string(),
            zh_hans: Some("世界树".to_string()),
        }
    };
    Some(MapDefinitionRecord {
        id: if native_id == "MainMap" {
            "MAP_MAIN".to_string()
        } else {
            "MAP_TREE".to_string()
        },
        native_name: if native_id == "MainMap" {
            "PL_MainWorld5".to_string()
        } else {
            "PL_MainWorld5_Tree".to_string()
        },
        names,
        bounds: MapBounds {
            min_x: min.get("X")?.as_f64()?,
            max_x: max.get("X")?.as_f64()?,
            min_y: min.get("Y")?.as_f64()?,
            max_y: max.get("Y")?.as_f64()?,
            min_z: min.get("Z")?.as_f64()?,
            max_z: max.get("Z")?.as_f64()?,
        },
        logical_size: 8192,
        priority: row.get("WorldMapPriority").and_then(Value::as_u64)? as u32,
        texture_asset_path: row
            .pointer("/textureDataMap/0/Value/Texture/AssetPathName")
            .and_then(Value::as_str)
            .map(str::to_string),
        provenance: provenance(),
    })
}

fn normalize_pal(
    raw_id: &str,
    pal_ids: &BTreeMap<String, String>,
) -> (Option<String>, Vec<String>) {
    let raw_id = raw_id.to_ascii_lowercase();
    if let Some(id) = pal_ids.get(&raw_id) {
        return (Some(id.clone()), Vec::new());
    }
    let mut variants = Vec::new();
    let mut normalized = raw_id.to_string();
    if let Some(stripped) = normalized.strip_prefix("boss_") {
        variants.push("boss".to_string());
        normalized = stripped.to_string();
        if let Some(id) = pal_ids.get(&normalized) {
            return (Some(id.clone()), variants);
        }
    }
    for suffix in [
        "_dark",
        "_fire",
        "_ice",
        "_ground",
        "_electric",
        "_dragon",
        "_ghost",
    ] {
        if let Some(stripped) = normalized.strip_suffix(suffix) {
            variants.push(suffix.trim_start_matches('_').to_string());
            if let Some(id) = pal_ids.get(stripped) {
                return (Some(id.clone()), variants);
            }
        }
    }
    (None, variants)
}

fn placement_kind(placement: &Value) -> PalSpawnPlacementKind {
    let placement_type = native(placement.get("PlacementType")).to_ascii_lowercase();
    let spawner_type = native(placement.get("SpawnerType")).to_ascii_lowercase();
    if placement_type.contains("dungeon_boss") || spawner_type.contains("DungeonBoss") {
        PalSpawnPlacementKind::DungeonBoss
    } else if placement_type.contains("field_boss") || spawner_type.contains("FieldBoss") {
        PalSpawnPlacementKind::FieldBoss
    } else if placement_type.contains("imprisonment_boss") {
        PalSpawnPlacementKind::ImprisonmentBoss
    } else if placement_type.contains("dungeon") {
        PalSpawnPlacementKind::Dungeon
    } else if placement_type.contains("field") {
        PalSpawnPlacementKind::Field
    } else {
        PalSpawnPlacementKind::Unknown
    }
}

fn map_for_coordinate(location: &WorldCoordinate) -> Option<String> {
    let main_contains = (-1_099_400.0..=349_400.0).contains(&location.x)
        && (-724_400.0..=724_400.0).contains(&location.y);
    let tree_contains = (347_351.5..=689_148.5).contains(&location.x)
        && (-818_197.0..=-476_400.0).contains(&location.y);
    if tree_contains {
        Some("MAP_TREE".to_string())
    } else if main_contains {
        Some("MAP_MAIN".to_string())
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Vector3 {
    x: f64,
    y: f64,
    z: f64,
}

impl Vector3 {
    const ZERO: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };
    const ONE: Self = Self {
        x: 1.0,
        y: 1.0,
        z: 1.0,
    };
}

#[derive(Debug, Default, Clone, PartialEq)]
struct RegionBoundaries {
    shapes: BTreeMap<String, MapShape>,
    trigger_boxes: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq)]
struct RegionTriggerBox {
    area: String,
    source: String,
    root_location: Option<Vector3>,
    root_yaw: f64,
    root_scale: Vector3,
    box_location: Vector3,
    box_yaw: f64,
    box_scale: Vector3,
    box_extent: Option<Vector3>,
}

impl RegionTriggerBox {
    /// World volume of the trigger box. The actor transform places the box
    /// component, and every scale on the way down multiplies the extent.
    fn shape(&self, root_location: Vector3, extent: Vector3) -> MapShape {
        let (sin, cos) = self.root_yaw.to_radians().sin_cos();
        let local_x = self.root_scale.x * self.box_location.x;
        let local_y = self.root_scale.y * self.box_location.y;
        MapShape::Box {
            center: WorldCoordinate {
                x: root_location.x + cos * local_x - sin * local_y,
                y: root_location.y + sin * local_x + cos * local_y,
                z: root_location.z + self.root_scale.z * self.box_location.z,
            },
            extent_x: (extent.x * self.box_scale.x * self.root_scale.x).abs(),
            extent_y: (extent.y * self.box_scale.y * self.root_scale.y).abs(),
            extent_z: (extent.z * self.box_scale.z * self.root_scale.z).abs(),
            yaw_degrees: normalize_yaw(self.root_yaw + self.box_yaw),
        }
    }
}

fn read_level_exports(root: &Path) -> Result<Value, MapIntakeError> {
    let content = read(root, LEVEL_TABLE)?;
    serde_json::from_str(&content).map_err(|error| invalid(LEVEL_TABLE, error.to_string()))
}

/// Reviewed region boundaries come from the level's `BP_PalRegionTriggerBox`
/// actors. A box that never overrides `BoxExtent` inherits the blueprint
/// default, which lives in the blueprint asset rather than in the level, so the
/// blueprint export is read for that value. When the template overrides nothing
/// its box component serialises no `BoxExtent` at all and the engine default
/// applies, which is still evidence rather than an inference.
fn region_boundaries(
    root: &Path,
    level_exports: &Value,
    report: &mut MapCandidateReport,
) -> Result<RegionBoundaries, MapIntakeError> {
    let blueprint_extent = blueprint_default_extent(root);
    let mut boundaries = RegionBoundaries::default();
    let mut triggers = region_trigger_boxes(level_exports, LEVEL_TABLE);
    triggers.extend(read_cell_trigger_boxes(root)?);
    for trigger in triggers {
        if !boundaries.trigger_boxes.insert(trigger.area.clone()) {
            outcome(
                report,
                &trigger.source,
                &trigger.area,
                "skip",
                "duplicate_region_trigger_box",
            );
            continue;
        }
        let Some(root_location) = trigger.root_location else {
            outcome(
                report,
                &trigger.source,
                &trigger.area,
                "skip",
                "missing_region_trigger_box_transform",
            );
            continue;
        };
        let (extent, evidence) = match (trigger.box_extent, blueprint_extent.as_ref()) {
            (Some(extent), _) => (extent, "trigger_box_extent"),
            (None, Some(blueprint)) => (blueprint.extent, blueprint.evidence),
            (None, None) => {
                outcome(
                    report,
                    &trigger.source,
                    &trigger.area,
                    "skip",
                    "missing_region_trigger_box_extent",
                );
                continue;
            }
        };
        outcome(
            report,
            &trigger.source,
            &trigger.area,
            "use",
            &format!("region_boundary_from_{evidence}"),
        );
        boundaries
            .shapes
            .insert(trigger.area.clone(), trigger.shape(root_location, extent));
    }
    Ok(boundaries)
}

fn read_cell_trigger_boxes(root: &Path) -> Result<Vec<RegionTriggerBox>, MapIntakeError> {
    let directory = root.join(REGION_CELL_DIRECTORY);
    let Ok(entries) = fs::read_dir(&directory) else {
        return Ok(Vec::new());
    };
    let mut paths = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|extension| extension.to_str()) == Some("json"))
        .collect::<Vec<_>>();
    paths.sort();
    let mut triggers = Vec::new();
    for path in paths {
        let bytes = fs::read(&path).map_err(|error| MapIntakeError::InvalidTable {
            path: path.display().to_string(),
            message: error.to_string(),
        })?;
        if !contains_marker(&bytes, REGION_TRIGGER_MARKER) {
            continue;
        }
        let exports: Value =
            serde_json::from_slice(&bytes).map_err(|error| MapIntakeError::InvalidTable {
                path: path.display().to_string(),
                message: error.to_string(),
            })?;
        let source = path
            .strip_prefix(root)
            .map(|relative| relative.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| path.display().to_string());
        triggers.extend(region_trigger_boxes(&exports, &source));
    }
    Ok(triggers)
}

/// Cheap byte scan so that only world partition cells that can carry a region
/// trigger are parsed as JSON; the target build ships roughly ten thousand
/// cells and only a handful of them contain region triggers.
fn contains_marker(bytes: &[u8], marker: &[u8]) -> bool {
    if marker.is_empty() || bytes.len() < marker.len() {
        return false;
    }
    let mut cursor = 0;
    while cursor + marker.len() <= bytes.len() {
        let Some(offset) = bytes[cursor..].iter().position(|byte| *byte == marker[0]) else {
            return false;
        };
        let start = cursor + offset;
        if start + marker.len() > bytes.len() {
            return false;
        }
        if &bytes[start..start + marker.len()] == marker {
            return true;
        }
        cursor = start + 1;
    }
    false
}

fn region_trigger_boxes(exports: &Value, source: &str) -> Vec<RegionTriggerBox> {
    let Some(exports) = exports.as_array() else {
        return Vec::new();
    };
    let mut areas = BTreeMap::<String, String>::new();
    let mut roots = BTreeMap::<String, &Value>::new();
    let mut boxes = BTreeMap::<String, &Value>::new();
    for export in exports {
        if export.get("Type").and_then(Value::as_str) == Some(REGION_TRIGGER_ACTOR) {
            let (Some(actor), Some(area)) = (
                export.get("Name").and_then(Value::as_str),
                export
                    .pointer("/Properties/AreaName/Key")
                    .and_then(Value::as_str),
            ) else {
                continue;
            };
            areas.insert(actor.to_string(), area.to_string());
            continue;
        }
        let Some(owner) = export
            .pointer("/Outer/ObjectName")
            .and_then(Value::as_str)
            .and_then(owner_name)
        else {
            continue;
        };
        match export.get("Name").and_then(Value::as_str) {
            Some("DefaultSceneRoot") => roots.insert(owner.to_string(), export),
            Some("Box") => boxes.insert(owner.to_string(), export),
            _ => None,
        };
    }
    areas
        .into_iter()
        .map(|(actor, area)| {
            let root = roots.get(&actor).copied();
            let box_component = boxes.get(&actor).copied();
            RegionTriggerBox {
                area,
                source: source.to_string(),
                root_location: root
                    .and_then(|root| root.pointer("/Properties/RelativeLocation"))
                    .and_then(vector),
                root_yaw: root.and_then(relative_yaw).unwrap_or(0.0),
                root_scale: root
                    .and_then(|root| root.pointer("/Properties/RelativeScale3D"))
                    .and_then(vector)
                    .unwrap_or(Vector3::ONE),
                box_location: box_component
                    .and_then(|component| component.pointer("/Properties/RelativeLocation"))
                    .and_then(vector)
                    .unwrap_or(Vector3::ZERO),
                box_yaw: box_component.and_then(relative_yaw).unwrap_or(0.0),
                box_scale: box_component
                    .and_then(|component| component.pointer("/Properties/RelativeScale3D"))
                    .and_then(vector)
                    .unwrap_or(Vector3::ONE),
                box_extent: box_component
                    .and_then(|component| component.pointer("/Properties/BoxExtent"))
                    .and_then(vector),
            }
        })
        .collect()
}

/// Extent that a `BP_PalRegionTriggerBox` instance inherits when it overrides nothing.
struct BlueprintBoxExtent {
    extent: Vector3,
    evidence: &'static str,
}

fn blueprint_default_extent(root: &Path) -> Option<BlueprintBoxExtent> {
    let content = fs::read_to_string(root.join(REGION_BLUEPRINT_TABLE)).ok()?;
    let exports: Value = serde_json::from_str(&content).ok()?;
    blueprint_box_extent(&exports)
}

/// The blueprint serialises only the properties its box component overrides, so an absent
/// `BoxExtent` is evidence that the template keeps the engine default rather than a gap.
fn blueprint_box_extent(exports: &Value) -> Option<BlueprintBoxExtent> {
    let mut fallback = None;
    for export in exports.as_array()? {
        if export.get("Type").and_then(Value::as_str) != Some("BoxComponent") {
            continue;
        }
        let explicit = export.pointer("/Properties/BoxExtent").and_then(vector);
        let candidate = BlueprintBoxExtent {
            extent: explicit.unwrap_or(ENGINE_DEFAULT_BOX_EXTENT),
            evidence: if explicit.is_some() {
                "blueprint_box_extent"
            } else {
                "blueprint_engine_default_extent"
            },
        };
        if export
            .get("Name")
            .and_then(Value::as_str)
            .is_some_and(|name| name.starts_with("Box"))
        {
            return Some(candidate);
        }
        fallback = fallback.or(Some(candidate));
    }
    fallback
}

fn owner_name(object_name: &str) -> Option<&str> {
    let inner = object_name.split('\'').nth(1).unwrap_or(object_name);
    inner.rsplit('.').next()
}

fn relative_yaw(component: &Value) -> Option<f64> {
    component
        .pointer("/Properties/RelativeRotation/Yaw")
        .and_then(Value::as_f64)
}

fn vector(value: &Value) -> Option<Vector3> {
    Some(Vector3 {
        x: value.get("X")?.as_f64()?,
        y: value.get("Y")?.as_f64()?,
        z: value.get("Z")?.as_f64()?,
    })
}

fn normalize_yaw(degrees: f64) -> f64 {
    let remainder = degrees % 360.0;
    let normalized = if remainder > 180.0 {
        remainder - 360.0
    } else if remainder <= -180.0 {
        remainder + 360.0
    } else {
        remainder
    };
    if normalized == 0.0 {
        0.0
    } else {
        normalized
    }
}
fn read_rows(root: &Path, relative_path: &str) -> Result<BTreeMap<String, Value>, MapIntakeError> {
    let content = read(root, relative_path)?;
    let value: Value = serde_json::from_str(&content)
        .map_err(|error| invalid(relative_path, error.to_string()))?;
    let Some(rows) = value.pointer("/0/Rows").and_then(Value::as_object) else {
        return Err(invalid(
            relative_path,
            "expected /0/Rows object".to_string(),
        ));
    };
    Ok(rows
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect())
}

fn localization_table(
    root: &Path,
    locale: &str,
    file_name: &str,
) -> Result<BTreeMap<String, String>, MapIntakeError> {
    let relative_path = format!("Pal/Content/L10N/{locale}/Pal/DataTable/Text/{file_name}");
    let content = read(root, &relative_path)?;
    let value: Value = serde_json::from_str(&content)
        .map_err(|error| invalid(&relative_path, error.to_string()))?;
    let Some(rows) = value.pointer("/0/Rows").and_then(Value::as_object) else {
        return Err(invalid(
            &relative_path,
            "expected /0/Rows object".to_string(),
        ));
    };
    Ok(rows
        .iter()
        .filter_map(|(key, row)| {
            row.pointer("/TextData/LocalizedString")
                .and_then(Value::as_str)
                .map(|text| (key.clone(), text.to_string()))
        })
        .collect())
}

fn read(root: &Path, relative_path: &str) -> Result<String, MapIntakeError> {
    let path = root.join(relative_path);
    if !path.exists() {
        return Err(MapIntakeError::MissingTable {
            path: relative_path.to_string(),
        });
    }
    fs::read_to_string(path).map_err(|error| MapIntakeError::InvalidTable {
        path: relative_path.to_string(),
        message: error.to_string(),
    })
}

fn coordinate(value: &Value) -> Option<WorldCoordinate> {
    Some(WorldCoordinate {
        x: value.get("X")?.as_f64()?,
        y: value.get("Y")?.as_f64()?,
        z: value.get("Z")?.as_f64()?,
    })
}

fn optional_native(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(|text| text.trim_start_matches("EPal").to_string())
}

fn native(value: Option<&Value>) -> String {
    value
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn point_evidence(notes: &str) -> LocalEvidenceMetadata {
    LocalEvidenceMetadata {
        source_table: "PL_MainWorld5 + DataTable".to_string(),
        localization_status: LocalizationStatus::Resolved,
        unresolved_fields: Vec::new(),
        reviewed_empty_fields: Vec::new(),
        transformation_notes: notes.to_string(),
    }
}

fn provenance() -> Provenance {
    Provenance {
        source_id: SOURCE_ID.to_string(),
        applicable_game_version: "1.0".to_string(),
        retrieved_on: "2026-09-06".to_string(),
        reviewer: "Codex".to_string(),
        review_status: ReviewStatus::Reviewed,
        confidence: Confidence::VerifiedTarget,
        change_risk: None,
        corroborating_source_ids: Vec::new(),
    }
}

fn outcome(report: &mut MapCandidateReport, table: &str, row_id: &str, result: &str, reason: &str) {
    report.row_outcomes.push(MapRowOutcome {
        table: table.to_string(),
        native_row_id: row_id.to_string(),
        outcome: result.to_string(),
        reason: reason.to_string(),
    });
}

fn invalid(path: &str, message: String) -> MapIntakeError {
    MapIntakeError::InvalidTable {
        path: path.to_string(),
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_assignment_keeps_main_and_tree_bounds_isolated() {
        let main = WorldCoordinate {
            x: -500_000.0,
            y: 0.0,
            z: 0.0,
        };
        let tree = WorldCoordinate {
            x: 500_000.0,
            y: -600_000.0,
            z: 0.0,
        };
        let outside = WorldCoordinate {
            x: 1_000_000.0,
            y: 0.0,
            z: 0.0,
        };
        assert_eq!(map_for_coordinate(&main).as_deref(), Some("MAP_MAIN"));
        assert_eq!(map_for_coordinate(&tree).as_deref(), Some("MAP_TREE"));
        assert_eq!(map_for_coordinate(&outside), None);
    }

    #[test]
    fn boss_and_variant_ids_preserve_context() {
        let pals = BTreeMap::from([
            ("lilyqueen".to_string(), "PAL_LILYQUEEN".to_string()),
            ("icewitch".to_string(), "PAL_ICEWITCH".to_string()),
        ]);
        let (boss, variants) = normalize_pal("BOSS_LilyQueen", &pals);
        assert_eq!(boss.as_deref(), Some("PAL_LILYQUEEN"));
        assert_eq!(variants, vec!["boss"]);
        let (case_insensitive, variants) = normalize_pal("IceWitch", &pals);
        assert_eq!(case_insensitive.as_deref(), Some("PAL_ICEWITCH"));
        assert!(variants.is_empty());
    }

    #[test]
    fn placement_type_is_normalized_case_insensitively() {
        let value = serde_json::json!({"PlacementType": "EPalSpawnerPlacementType::Field"});
        assert_eq!(placement_kind(&value), PalSpawnPlacementKind::Field);
    }

    #[test]
    fn supplementary_placement_kind_maps_spawner_types() {
        assert_eq!(
            supplementary_placement_kind("EPalSpawnedCharacterType::FieldBoss"),
            PalSpawnPlacementKind::FieldBoss
        );
        assert_eq!(
            supplementary_placement_kind("EPalSpawnedCharacterType::Common"),
            PalSpawnPlacementKind::Field
        );
        assert_eq!(
            supplementary_placement_kind("EPalSpawnedCharacterType::RandomDungeonBoss"),
            PalSpawnPlacementKind::DungeonBoss
        );
        assert_eq!(
            supplementary_placement_kind("EPalSpawnedCharacterType::Undefined"),
            PalSpawnPlacementKind::Unknown
        );
    }

    #[test]
    fn area_estimation_resolves_biome_named_spawners() {
        let desert = estimate_area_coordinate("desert_orange_ALL_Suzakus").expect("desert area");
        assert_eq!(map_for_coordinate(&desert).as_deref(), Some("MAP_MAIN"));
        let worldtree = estimate_area_coordinate("worldtree_9_01_A_FBOSS_1").expect("tree area");
        assert_eq!(map_for_coordinate(&worldtree).as_deref(), Some("MAP_TREE"));
        assert!(estimate_area_coordinate("unrelated_area_name").is_none());
    }
    fn trigger_box(
        root_yaw: f64,
        root_scale: Vector3,
        box_location: Vector3,
        box_yaw: f64,
        box_extent: Option<Vector3>,
    ) -> RegionTriggerBox {
        RegionTriggerBox {
            area: "TEST".to_string(),
            source: "test".to_string(),
            root_location: Some(Vector3 {
                x: 1000.0,
                y: 2000.0,
                z: 300.0,
            }),
            root_yaw,
            root_scale,
            box_location,
            box_yaw,
            box_scale: Vector3::ONE,
            box_extent,
        }
    }

    fn assert_close(left: f64, right: f64) {
        assert!((left - right).abs() < 1e-9, "{left} != {right}");
    }

    #[test]
    fn trigger_box_geometry_applies_actor_and_component_transforms() {
        let extent = Vector3 {
            x: 100.0,
            y: 200.0,
            z: 300.0,
        };
        let plain = trigger_box(0.0, Vector3::ONE, Vector3::ZERO, 0.0, Some(extent));
        let root = plain.root_location.expect("fixture root location");
        let shape = plain.shape(root, extent);
        assert_eq!(
            shape,
            MapShape::Box {
                center: WorldCoordinate {
                    x: 1000.0,
                    y: 2000.0,
                    z: 300.0,
                },
                extent_x: 100.0,
                extent_y: 200.0,
                extent_z: 300.0,
                yaw_degrees: 0.0,
            }
        );
        assert!(shape.contains(&WorldCoordinate {
            x: 1090.0,
            y: 2190.0,
            z: 590.0,
        }));
        assert!(!shape.contains(&WorldCoordinate {
            x: 1110.0,
            y: 2000.0,
            z: 300.0,
        }));
        assert!(!shape.contains(&WorldCoordinate {
            x: 1000.0,
            y: 2000.0,
            z: 700.0,
        }));

        let scaled = trigger_box(
            0.0,
            Vector3 {
                x: 2.0,
                y: 3.0,
                z: 4.0,
            },
            Vector3 {
                x: 10.0,
                y: 20.0,
                z: 30.0,
            },
            0.0,
            Some(extent),
        );
        let root = scaled.root_location.expect("fixture root location");
        let MapShape::Box {
            center,
            extent_x,
            extent_y,
            extent_z,
            yaw_degrees,
        } = scaled.shape(root, extent)
        else {
            panic!("trigger boxes produce box geometry");
        };
        assert_close(center.x, 1020.0);
        assert_close(center.y, 2060.0);
        assert_close(center.z, 420.0);
        assert_close(extent_x, 200.0);
        assert_close(extent_y, 600.0);
        assert_close(extent_z, 1200.0);
        assert_close(yaw_degrees, 0.0);
    }

    #[test]
    fn trigger_box_yaw_rotates_the_component_offset_and_the_volume() {
        let extent = Vector3 {
            x: 10.0,
            y: 20.0,
            z: 30.0,
        };
        let yawed = trigger_box(
            90.0,
            Vector3::ONE,
            Vector3 {
                x: 100.0,
                y: 0.0,
                z: 0.0,
            },
            0.0,
            Some(extent),
        );
        let root = yawed.root_location.expect("fixture root location");
        let shape = yawed.shape(root, extent);
        let MapShape::Box {
            center,
            yaw_degrees,
            ..
        } = shape
        else {
            panic!("trigger boxes produce box geometry");
        };
        // A 90 degree yaw sends the box component offset along world +Y, and the
        // local X axis of the volume then also runs along world +Y.
        assert_close(center.x, 1000.0);
        assert_close(center.y, 2100.0);
        assert_close(yaw_degrees, 90.0);
        assert!(shape.contains(&WorldCoordinate {
            x: 1000.0,
            y: 2109.0,
            z: 300.0,
        }));
        assert!(!shape.contains(&WorldCoordinate {
            x: 1000.0,
            y: 2111.0,
            z: 300.0,
        }));
        assert!(shape.contains(&WorldCoordinate {
            x: 981.0,
            y: 2100.0,
            z: 300.0,
        }));
        assert!(!shape.contains(&WorldCoordinate {
            x: 979.0,
            y: 2100.0,
            z: 300.0,
        }));
    }

    #[test]
    fn yaw_is_normalized_to_a_signed_turn() {
        assert_close(normalize_yaw(0.0), 0.0);
        assert_close(normalize_yaw(360.0), 0.0);
        assert_close(normalize_yaw(370.0), 10.0);
        assert_close(normalize_yaw(225.0), -135.0);
        assert_close(normalize_yaw(-190.0), 170.0);
        assert_close(normalize_yaw(180.0), 180.0);
        assert_close(normalize_yaw(-180.0), 180.0);
        assert!(normalize_yaw(-0.0).is_sign_positive());
    }

    #[test]
    fn marker_scan_finds_region_trigger_bytes() {
        assert!(contains_marker(
            b"{\"Type\":\"BP_PalRegionTriggerBox_C\"}",
            REGION_TRIGGER_MARKER
        ));
        assert!(!contains_marker(
            b"{\"Type\":\"BP_PalSpawner_C\"}",
            REGION_TRIGGER_MARKER
        ));
        assert!(!contains_marker(b"", REGION_TRIGGER_MARKER));
        assert!(contains_marker(
            REGION_TRIGGER_MARKER,
            REGION_TRIGGER_MARKER
        ));
        assert!(!contains_marker(
            b"BP_PalRegionTriggerBo",
            REGION_TRIGGER_MARKER
        ));
    }

    #[test]
    fn blueprint_extent_falls_back_to_the_engine_default() {
        let explicit = serde_json::json!([{
            "Type": "BoxComponent",
            "Name": "Box_GEN_VARIABLE",
            "Properties": { "BoxExtent": { "X": 10.0, "Y": 20.0, "Z": 30.0 } },
        }]);
        let resolved = blueprint_box_extent(&explicit).expect("explicit blueprint extent");
        assert_close(resolved.extent.x, 10.0);
        assert_close(resolved.extent.z, 30.0);
        assert_eq!(resolved.evidence, "blueprint_box_extent");

        let inherited = serde_json::json!([{
            "Type": "BoxComponent",
            "Name": "Box_GEN_VARIABLE",
            "Properties": { "bHiddenInSceneCapture": true },
        }]);
        let resolved = blueprint_box_extent(&inherited).expect("inherited engine default");
        assert_close(resolved.extent.x, 32.0);
        assert_close(resolved.extent.z, 32.0);
        assert_eq!(resolved.evidence, "blueprint_engine_default_extent");

        let absent = serde_json::json!([{ "Type": "SceneComponent", "Name": "DefaultSceneRoot" }]);
        assert!(blueprint_box_extent(&absent).is_none());
    }

    #[test]
    fn an_engine_default_extent_still_scales_with_the_actor_transform() {
        let trigger = trigger_box(
            0.0,
            Vector3 {
                x: 50.0,
                y: 50.0,
                z: 250.0,
            },
            Vector3::ZERO,
            0.0,
            None,
        );
        let root = trigger.root_location.expect("fixture root location");
        let MapShape::Box {
            extent_x,
            extent_y,
            extent_z,
            ..
        } = trigger.shape(root, ENGINE_DEFAULT_BOX_EXTENT)
        else {
            panic!("trigger boxes produce box geometry");
        };
        assert_close(extent_x, 1600.0);
        assert_close(extent_y, 1600.0);
        assert_close(extent_z, 8000.0);
    }
}
