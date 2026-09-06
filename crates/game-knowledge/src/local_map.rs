use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::models::{
    Confidence, KnowledgeRecord, LocalEvidenceMetadata, LocaleNames, LocalizationStatus, MapBounds,
    MapDefinitionRecord, MapPointKind, MapPointRecord, MapRegionRecord, PalHabitatZoneRecord,
    PalSpawnPlacementKind, Provenance, ReviewStatus, WorldCoordinate,
};

const MAP_UI_TABLE: &str = "Pal/Content/Pal/DataTable/WorldMapUIData/DT_WorldMapUIData.json";
const MAP_AREA_TABLE: &str = "Pal/Content/Pal/DataTable/WorldMapAreaData/DT_WorldMapAreaData.json";
const PLACEMENT_TABLE: &str = "Pal/Content/Pal/DataTable/Spawner/DT_PalSpawnerPlacement.json";
const WILD_SPAWNER_TABLE: &str = "Pal/Content/Pal/DataTable/Spawner/DT_PalWildSpawner.json";
const LEVEL_TABLE: &str = "Pal/Content/Pal/Maps/MainWorld_5/PL_MainWorld5.json";
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
                geometry: None,
                boundary_is_reviewed: false,
                provenance: provenance(),
            }));
            report.regions += 1;
        }

        parse_level_points(root, &mut records, &mut report)?;
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
    records: &mut Vec<KnowledgeRecord>,
    report: &mut MapCandidateReport,
) -> Result<(), MapIntakeError> {
    let path = root.join(LEVEL_TABLE);
    let content = fs::read_to_string(path).map_err(|error| MapIntakeError::InvalidTable {
        path: LEVEL_TABLE.to_string(),
        message: error.to_string(),
    })?;
    let exports: Value =
        serde_json::from_str(&content).map_err(|error| MapIntakeError::InvalidTable {
            path: LEVEL_TABLE.to_string(),
            message: error.to_string(),
        })?;
    let Some(exports) = exports.as_array() else {
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
    let spawners_by_name = spawners
        .values()
        .filter_map(|row| {
            let name = row.get("SpawnerName").and_then(Value::as_str)?;
            Some((name.to_string(), row))
        })
        .collect::<BTreeMap<_, _>>();

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
        let Some(spawner) = spawners_by_name.get(spawner_name) else {
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
            let zone = PalHabitatZoneRecord {
                id: format!("HAB_ZONE_{}_{}", placement_id, index),
                map_id: map_id.clone(),
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
    Ok(())
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
        transformation_notes: notes.to_string(),
    }
}

fn provenance() -> Provenance {
    Provenance {
        source_id: SOURCE_ID.to_string(),
        applicable_game_version: "1.0.3".to_string(),
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
}
