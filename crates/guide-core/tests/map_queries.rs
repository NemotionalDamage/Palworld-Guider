use game_knowledge::{
    Confidence, KnowledgeRecord, KnowledgeStore, LocaleNames, MapBounds, MapDefinitionRecord,
    MapPointKind, MapPointRecord, PalHabitatZoneRecord, PalRecord, PalSpawnPlacementKind,
    Provenance, ReviewStatus, SourceRecord, WorldCoordinate,
};
use guide_core::{AnswerStatus, GuideEngine};

#[test]
fn locates_coordinates_and_returns_deterministic_nearby_points() {
    let engine = engine(records());
    let location = WorldCoordinate {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };
    let answer = engine.locate_coordinate(location);
    assert_eq!(answer.status, AnswerStatus::Ok);
    let located = answer.data.expect("main map location exists");
    assert_eq!(located.map_id, "MAP_MAIN");
    assert_eq!(located.normalized.pixel_x, 6216);
    assert_eq!(located.normalized.pixel_y, 4096);
    assert!(answer
        .uncertainty
        .iter()
        .any(|item| item.contains("region")));

    let points = engine
        .find_nearby_map_points(location, Some(MapPointKind::FastTravel), 2)
        .data
        .expect("map points exist");
    assert_eq!(points.len(), 2);
    assert_eq!(points[0].id, "POINT_NEAR");
    assert_eq!(points[1].id, "POINT_FAR");
    assert_eq!(points[0].distance, 5000.0);
    assert_eq!(points[0].bearing_degrees, 90.0);
}

#[test]
fn keeps_tree_coordinates_out_of_main_map() {
    let engine = engine(records());
    let answer = engine.locate_coordinate(WorldCoordinate {
        x: 500_000.0,
        y: -600_000.0,
        z: 0.0,
    });
    assert_eq!(answer.status, AnswerStatus::Ok);
    assert_eq!(
        answer.data.expect("tree location exists").map_id,
        "MAP_TREE"
    );
}

#[test]
fn ranks_pal_habitats_by_reviewed_radius_then_distance() {
    let engine = engine(records());
    let origin = WorldCoordinate {
        x: 10_000.0,
        y: 0.0,
        z: 0.0,
    };
    let answer = engine.find_pal_spawn_zones("Test Pal", origin, 10);
    assert_eq!(answer.status, AnswerStatus::Ok);
    let zones = answer.data.expect("habitat zones exist");
    assert_eq!(zones[0].zone_id, "ZONE_NEAR");
    assert!(zones[0].within_reviewed_radius);
    assert!(!zones[1].within_reviewed_radius);
}

fn records() -> Vec<KnowledgeRecord> {
    let pal = PalRecord {
        id: "PAL_TEST".to_string(),
        names: LocaleNames {
            en: "Test Pal".to_string(),
            zh_hans: Some("测试帕鲁".to_string()),
        },
        work_suitability: Vec::new(),
        drops: Vec::new(),
        habitat_ids: vec!["ZONE_NEAR".to_string(), "ZONE_FAR".to_string()],
        element_type1: None,
        element_type2: None,
        native_row_id: Some("TestPal".to_string()),
        local_evidence: None,
        provenance: provenance(),
    };
    let map = MapDefinitionRecord {
        id: "MAP_MAIN".to_string(),
        native_name: "PL_MainWorld5".to_string(),
        names: LocaleNames {
            en: "Main Map".to_string(),
            zh_hans: Some("主地图".to_string()),
        },
        bounds: MapBounds {
            min_x: -1_099_400.0,
            max_x: 349_400.0,
            min_y: -724_400.0,
            max_y: 724_400.0,
            min_z: -10_000.0,
            max_z: 100_000.0,
        },
        logical_size: 8192,
        priority: 0,
        texture_asset_path: None,
        provenance: provenance(),
    };
    let tree = MapDefinitionRecord {
        id: "MAP_TREE".to_string(),
        native_name: "PL_MainWorld5_Tree".to_string(),
        names: LocaleNames {
            en: "World Tree".to_string(),
            zh_hans: Some("世界树".to_string()),
        },
        bounds: MapBounds {
            min_x: 347_351.5,
            max_x: 689_148.5,
            min_y: -818_197.0,
            max_y: -476_400.0,
            min_z: 0.0,
            max_z: 100_000.0,
        },
        logical_size: 8192,
        priority: 1,
        texture_asset_path: None,
        provenance: provenance(),
    };
    let near = map_point("POINT_NEAR", "Near Point", 5000.0, 0.0);
    let far = map_point("POINT_FAR", "Far Point", 100_000.0, 0.0);
    let zone_near = habitat_zone("ZONE_NEAR", 10_000.0);
    let zone_far = habitat_zone("ZONE_FAR", 520_000.0);
    vec![
        KnowledgeRecord::Pal(pal),
        KnowledgeRecord::MapDefinition(map),
        KnowledgeRecord::MapDefinition(tree),
        KnowledgeRecord::MapPoint(near),
        KnowledgeRecord::MapPoint(far),
        KnowledgeRecord::PalHabitatZone(zone_near),
        KnowledgeRecord::PalHabitatZone(zone_far),
    ]
}

fn map_point(id: &str, name: &str, x: f64, y: f64) -> MapPointRecord {
    MapPointRecord {
        id: id.to_string(),
        map_id: "MAP_MAIN".to_string(),
        native_id: id.to_string(),
        kind: MapPointKind::FastTravel,
        names: LocaleNames {
            en: name.to_string(),
            zh_hans: None,
        },
        location: WorldCoordinate { x, y, z: 0.0 },
        local_evidence: game_knowledge::LocalEvidenceMetadata {
            source_table: "test".to_string(),
            localization_status: game_knowledge::LocalizationStatus::Resolved,
            unresolved_fields: Vec::new(),
            transformation_notes: "test".to_string(),
        },
        provenance: provenance(),
    }
}

fn habitat_zone(id: &str, x: f64) -> PalHabitatZoneRecord {
    PalHabitatZoneRecord {
        id: id.to_string(),
        map_id: "MAP_MAIN".to_string(),
        native_placement_id: id.to_string(),
        native_spawner_name: id.to_string(),
        placement_kind: PalSpawnPlacementKind::Field,
        location: WorldCoordinate { x, y: 0.0, z: 0.0 },
        radius: if x == 10_000.0 { 20_000.0 } else { 5_000.0 },
        raw_pal_id: "TestPal".to_string(),
        pal_id: "PAL_TEST".to_string(),
        variant_labels: Vec::new(),
        level_min: 1,
        level_max: 10,
        count_min: 1,
        count_max: 2,
        time_of_day: None,
        weather: None,
        allows_randomizer: false,
        respawn_cool_time_seconds: 0.0,
        local_evidence: game_knowledge::LocalEvidenceMetadata {
            source_table: "test".to_string(),
            localization_status: game_knowledge::LocalizationStatus::Resolved,
            unresolved_fields: Vec::new(),
            transformation_notes: "test".to_string(),
        },
        provenance: provenance(),
    }
}

fn provenance() -> Provenance {
    Provenance {
        source_id: "SRC_TEST".to_string(),
        applicable_game_version: "1.0.3".to_string(),
        retrieved_on: "2026-09-06".to_string(),
        reviewer: "test".to_string(),
        review_status: ReviewStatus::Reviewed,
        confidence: Confidence::VerifiedTarget,
        change_risk: None,
        corroborating_source_ids: Vec::new(),
    }
}

fn engine(records: Vec<KnowledgeRecord>) -> GuideEngine {
    let source = SourceRecord {
        id: "SRC_TEST".to_string(),
        title: "Map test source".to_string(),
        supplier: "test".to_string(),
        retrieved_on: "2026-09-06".to_string(),
        evidence_urls: vec!["local://test".to_string()],
        applicable_game_version: "1.0.3".to_string(),
        reviewer: "test".to_string(),
        review_status: ReviewStatus::Reviewed,
        confidence: Confidence::VerifiedTarget,
        notes: None,
    };
    let mut all_records = vec![KnowledgeRecord::Source(source)];
    all_records.extend(records);
    let store = KnowledgeStore::from_records(all_records).expect("records validate");
    GuideEngine::new(store, Some("1.0.3".to_string()))
}
