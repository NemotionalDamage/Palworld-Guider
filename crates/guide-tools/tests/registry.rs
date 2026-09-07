use chrono::{Duration as ChronoDuration, Utc};
use game_knowledge::KnowledgeStore;
use guide_core::GuideEngine;
use guide_tools::{ToolBudget, ToolRegistry, ToolStatus};
use knowledge_index::KnowledgeIndex;
use serde_json::{json, Value};
use std::fs;
use std::time::{Duration, Instant};

const DATA_DIRECTORY: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/reviewed");

fn test_registry(configured: Option<&str>) -> ToolRegistry {
    let mut lines = Vec::new();
    for file_name in ["sources.jsonl", "facts.jsonl"] {
        lines.extend(
            fs::read_to_string(format!("{DATA_DIRECTORY}/{file_name}"))
                .expect("dataset reads")
                .lines()
                .map(str::to_string),
        );
    }
    lines.extend(referenced_habitat_support_lines(&lines));
    let records = lines
        .iter()
        .filter(|line| !line.contains("\"record_type\":\"conflict\""))
        .map(|line| serde_json::from_str(line.as_str()))
        .collect::<Result<Vec<_>, _>>()
        .expect("conflict-free fixtures parse");
    let store = KnowledgeStore::from_records(records).expect("test store validates");
    let configured = configured.map(str::to_string);
    let index = KnowledgeIndex::from_store(&store, configured.clone()).expect("index builds");
    let engine = GuideEngine::new(store, configured);
    ToolRegistry::new(engine, index)
}

fn referenced_habitat_support_lines(base_lines: &[String]) -> Vec<String> {
    let mut needed_zones = std::collections::BTreeSet::new();
    for line in base_lines {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if value.get("record_type").and_then(Value::as_str) != Some("pal") {
            continue;
        }
        let Some(zone_ids) = value.get("habitat_ids").and_then(Value::as_array) else {
            continue;
        };
        for zone_id in zone_ids.iter().filter_map(Value::as_str) {
            needed_zones.insert(zone_id.to_string());
        }
    }
    if needed_zones.is_empty() {
        return Vec::new();
    }
    let mut support = Vec::new();
    let maps_path = format!("{DATA_DIRECTORY}/maps.jsonl");
    support.extend(
        fs::read_to_string(&maps_path)
            .expect("maps file reads")
            .lines()
            .map(str::to_string),
    );
    let zones_path = format!("{DATA_DIRECTORY}/pal_habitat_zones.jsonl");
    support.extend(
        fs::read_to_string(&zones_path)
            .expect("habitat zones file reads")
            .lines()
            .filter(|line| {
                serde_json::from_str::<Value>(line)
                    .ok()
                    .and_then(|value| value.get("id").and_then(Value::as_str).map(str::to_string))
                    .is_some_and(|id| needed_zones.contains(&id))
            })
            .map(str::to_string),
    );
    support
}

fn fresh_budget(max_calls: usize) -> ToolBudget {
    ToolBudget::new(max_calls, Instant::now() + Duration::from_secs(30))
}

#[test]
fn manifest_publishes_all_tools_with_schemas() {
    let registry = test_registry(None);
    let definitions = registry.definitions();
    let names = definitions
        .iter()
        .map(|definition| definition.name.as_str())
        .collect::<Vec<_>>();
    for expected in [
        "resolve_name",
        "get_item",
        "get_pal",
        "get_recipe",
        "get_technology",
        "search_structured_knowledge",
        "calculate_materials",
        "calculate_shortage",
        "calculate_craftable_count",
        "calculate_breeding_result",
        "calculate_breeding_chain",
        "get_conflicting_records",
        "import_player_snapshot",
        "analyze_inventory",
        "analyze_party",
        "suggest_next_goals",
    ] {
        assert!(names.contains(&expected), "missing tool: {expected}");
    }
    for definition in definitions {
        assert!(!definition.description.is_empty());
        assert_eq!(definition.parameters_schema["type"], "object");
    }
}

#[test]
fn canonical_dataset_publishes_localized_entity_names() {
    let store = KnowledgeStore::load_directory(DATA_DIRECTORY).expect("dataset is valid");
    let index = KnowledgeIndex::from_store(&store, None).expect("index builds");
    let engine = GuideEngine::new(store, None);
    let registry = ToolRegistry::new(engine, index);
    let names = registry.known_entity_names();

    assert!(names.contains("皮皮鸡"));
    assert!(names.contains("帕鲁矿碎块"));
}

#[test]
fn canonical_pal_skill_unlocks_embed_reviewed_skill_details() {
    let store = KnowledgeStore::load_directory(DATA_DIRECTORY).expect("dataset is valid");
    let index = KnowledgeIndex::from_store(&store, None).expect("index builds");
    let engine = GuideEngine::new(store, None);
    let registry = ToolRegistry::new(engine, index);
    let mut budget = fresh_budget(4);

    let envelope = registry.dispatch(
        "get_pal_waza_unlocks",
        &json!({"pal": "阿努比斯"}),
        &mut budget,
    );

    assert_eq!(envelope.status, ToolStatus::Ok);
    let data = envelope.data.expect("unlock data is present");
    let unlocks = data.as_array().expect("unlock data is a list");
    assert!(!unlocks.is_empty());
    assert!(unlocks.iter().all(|unlock| unlock["waza"].is_object()));
    assert!(unlocks
        .iter()
        .any(|unlock| unlock["waza"]["names"]["zh_hans"] == "碎石霰弹"));
    assert!(unlocks.iter().any(|unlock| unlock["waza"]["power"] == 80));
}

#[test]
fn state_tools_use_attached_snapshot_without_exposing_raw_state() {
    let registry = test_registry(None)
        .with_state_snapshot_json(&snapshot_value())
        .expect("snapshot attaches");
    let mut budget = fresh_budget(8);

    let imported = registry.dispatch(
        "import_player_snapshot",
        &json!({"confirmation": "user_entered"}),
        &mut budget,
    );
    assert_eq!(imported.status, ToolStatus::Ok);
    assert_eq!(
        imported.data.as_ref().expect("summary exists")["source_kind"],
        "user_entered"
    );
    assert!(imported.data.as_ref().expect("summary exists")["inventory"].is_null());
    assert!(imported.data.as_ref().expect("summary exists")["party"].is_null());
    let serialized = serde_json::to_string(&imported).expect("envelope serializes");
    assert!(!serialized.contains("operator-local-session"));
    assert!(!serialized.contains("consent"));
    assert!(!serialized.contains("\"inventory\""));

    let inventory = registry.dispatch("analyze_inventory", &json!({}), &mut budget);
    assert_eq!(inventory.status, ToolStatus::Ok);
    let inventory_data = inventory.data.expect("inventory analysis exists");
    assert_eq!(inventory_data["goals"][0]["target_id"], "ITEM_WOODEN_CLUB");
    assert_eq!(
        inventory_data["goals"][0]["shortage"]["shortages"][0]["missing_quantity"],
        3
    );

    let party = registry.dispatch("analyze_party", &json!({}), &mut budget);
    assert_eq!(party.status, ToolStatus::Ok);
    let party_data = party.data.expect("party analysis exists");
    assert_eq!(party_data["members"][0]["pal_id"], "PAL_LAMBALL");

    let recommendations = registry.dispatch("suggest_next_goals", &json!({}), &mut budget);
    assert_eq!(recommendations.status, ToolStatus::Ok);
    let data = recommendations
        .data
        .as_ref()
        .expect("recommendations exist");
    assert!(data
        .as_array()
        .expect("recommendations are an array")
        .iter()
        .any(|recommendation| recommendation["action"] == "Collect 3 Wood"));
    assert!(!recommendations.provenance.is_empty());
    let serialized = serde_json::to_string(&recommendations).expect("envelope serializes");
    assert!(!serialized.contains("operator-local-session"));
}

#[test]
fn missing_snapshot_returns_unknown_state() {
    let registry = test_registry(None);
    let mut budget = fresh_budget(4);
    let envelope = registry.dispatch("suggest_next_goals", &json!({}), &mut budget);
    assert_eq!(envelope.status, ToolStatus::Unknown);
    assert!(envelope
        .uncertainty
        .iter()
        .any(|message| message.contains("no player state snapshot is configured")));
}

#[test]
fn stale_and_invalid_snapshots_are_rejected_clearly() {
    let mut stale = snapshot_value();
    stale["source"]["captured_at"] = json!("2020-01-01T00:00:00Z");
    stale["source"]["time_to_live_seconds"] = json!(1);
    let registry = test_registry(None)
        .with_state_snapshot_json(&stale)
        .expect("shape is valid");
    let mut budget = fresh_budget(4);
    let envelope = registry.dispatch("suggest_next_goals", &json!({}), &mut budget);
    assert_eq!(envelope.status, ToolStatus::Unknown);
    assert!(envelope
        .uncertainty
        .iter()
        .any(|message| message.contains("snapshot is stale")));

    let mut invalid = snapshot_value();
    invalid["source"]["kind"] = json!("server_api");
    let error = match test_registry(None).with_state_snapshot_json(&invalid) {
        Err(error) => error,
        Ok(_) => panic!("unsupported source is rejected"),
    };
    assert!(error.contains("unsupported source kind"));
}

fn snapshot_value() -> serde_json::Value {
    let captured_at = Utc::now() - ChronoDuration::seconds(1);
    json!({
        "schema_version": "state_snapshot_v1",
        "source": {
            "kind": "user_entered",
            "captured_at": captured_at.to_rfc3339(),
            "game_version": "1.0.3",
            "time_to_live_seconds": 900,
            "consent": {
                "id": "operator-local-session",
                "scope": ["guide_advice"],
                "granted_at": captured_at.to_rfc3339()
            }
        },
        "inventory": [
            {"item": "Wood", "quantity": 7, "evidence": "user_entered"}
        ],
        "party": [
            {"slot": 0, "pal": "Lamball", "evidence": "user_entered"}
        ],
        "unlocked_technologies": [
            {"technology": "Technology Level 1", "evidence": "user_entered"}
        ],
        "captured_pals": [
            {"pal": "Lamball", "level": 5, "evidence": "user_entered"}
        ],
        "player_level": {"value": 7, "evidence": "user_entered"},
        "goals": [
            {"kind": "craft", "target": "Wooden Club", "quantity": 2, "priority": 2}
        ],
        "preferences": {
            "spoiler_level": "minimal",
            "long_horizon": false,
            "preferred_activities": ["gathering"],
            "avoided_activities": ["combat"]
        }
    })
}

#[test]
fn dispatches_item_lookup_with_provenance() {
    let registry = test_registry(None);
    let mut budget = fresh_budget(4);
    let envelope = registry.dispatch("get_item", &json!({"query": "Wood"}), &mut budget);
    assert_eq!(envelope.status, ToolStatus::Ok);
    let data = envelope.data.expect("item data is present");
    assert_eq!(data["id"], "ITEM_WOOD");
    assert_eq!(
        envelope.provenance[0].source_id,
        "SRC-PALDB-V1_0_3-20260831"
    );
    assert_eq!(envelope.version.knowledge_version, "1.0.3");
    assert!(envelope.version.matches);
    assert!(envelope.errors.is_empty());
}

#[test]
fn rejects_missing_required_argument() {
    let registry = test_registry(None);
    let mut budget = fresh_budget(4);
    let envelope = registry.dispatch("get_item", &json!({}), &mut budget);
    assert_eq!(envelope.status, ToolStatus::Error);
    assert!(envelope.errors.iter().any(|error| error.contains("query")));
    assert!(envelope.data.is_none());
}

#[test]
fn rejects_unknown_tool() {
    let registry = test_registry(None);
    let mut budget = fresh_budget(4);
    let envelope = registry.dispatch("delete_save", &json!({}), &mut budget);
    assert_eq!(envelope.status, ToolStatus::Error);
    assert!(envelope
        .errors
        .iter()
        .any(|error| error.contains("unknown tool")));
}

#[test]
fn calculator_result_is_deterministic() {
    let registry = test_registry(None);
    let mut budget = fresh_budget(4);
    let envelope = registry.dispatch(
        "calculate_materials",
        &json!({"query": "Wooden Club", "quantity": 3}),
        &mut budget,
    );
    assert_eq!(envelope.status, ToolStatus::Ok);
    let data = envelope.data.expect("calculation is present");
    assert_eq!(data["totals"][0]["item_id"], "ITEM_WOOD");
    assert_eq!(data["totals"][0]["required_quantity"], 15);
    assert!(!envelope.provenance.is_empty());
}

#[test]
fn unknown_record_returns_unknown() {
    let registry = test_registry(None);
    let mut budget = fresh_budget(4);
    let envelope = registry.dispatch(
        "get_item",
        &json!({"query": "Unreviewed Thing"}),
        &mut budget,
    );
    assert_eq!(envelope.status, ToolStatus::Unknown);
    assert!(envelope.data.is_none());
    assert!(!envelope.uncertainty.is_empty());
}

#[test]
fn stale_version_is_visible() {
    let registry = test_registry(Some("0.9"));
    let mut budget = fresh_budget(4);
    let envelope = registry.dispatch("get_item", &json!({"query": "Wood"}), &mut budget);
    assert_eq!(envelope.status, ToolStatus::Ok);
    assert_eq!(
        envelope.version.configured_game_version.as_deref(),
        Some("0.9")
    );
    assert!(!envelope.version.matches);
    assert!(envelope
        .uncertainty
        .iter()
        .any(|message| message.contains("does not match configured game version 0.9")));
}

#[test]
fn budget_exhaustion_is_non_fatal() {
    let registry = test_registry(None);
    let mut budget = fresh_budget(0);
    let envelope = registry.dispatch("get_item", &json!({"query": "Wood"}), &mut budget);
    assert_eq!(envelope.status, ToolStatus::Error);
    assert!(envelope
        .errors
        .iter()
        .any(|error| error.contains("tool call budget exhausted")));
}

#[test]
fn deadline_exceeded_is_non_fatal() {
    let registry = test_registry(None);
    let mut budget = ToolBudget::new(4, Instant::now() - Duration::from_secs(1));
    let envelope = registry.dispatch("get_item", &json!({"query": "Wood"}), &mut budget);
    assert_eq!(envelope.status, ToolStatus::Error);
    assert!(envelope
        .errors
        .iter()
        .any(|error| error.contains("tool deadline exceeded")));
}

#[test]
fn lexical_search_is_available_through_registry() {
    let registry = test_registry(None);
    let mut budget = fresh_budget(4);
    let envelope = registry.dispatch(
        "search_structured_knowledge",
        &json!({"query": "chop trees", "limit": 3}),
        &mut budget,
    );
    assert_eq!(envelope.status, ToolStatus::Ok);
    let data = envelope.data.expect("search data is present");
    assert!(data["results"]
        .as_array()
        .expect("results are an array")
        .iter()
        .any(|result| result["record_id"] == "ITEM_WOOD"));
    assert!(!envelope.provenance.is_empty());
}

#[test]
fn resolve_name_returns_entity_identity() {
    let registry = test_registry(None);
    let mut budget = fresh_budget(4);
    let envelope = registry.dispatch("resolve_name", &json!({"query": "Wood"}), &mut budget);
    assert_eq!(envelope.status, ToolStatus::Ok);
    let data = envelope.data.expect("resolution is present");
    assert_eq!(data["id"], "ITEM_WOOD");
    assert_eq!(data["kind"], "item");
}

#[test]
fn missing_conflicts_return_unknown() {
    let registry = test_registry(None);
    let mut budget = fresh_budget(4);
    let envelope = registry.dispatch("get_conflicting_records", &json!({}), &mut budget);
    assert_eq!(envelope.status, ToolStatus::Unknown);
    assert!(envelope
        .uncertainty
        .iter()
        .any(|message| message.contains("no conflicting records")));
}

#[test]
fn missing_breeding_rule_returns_unknown() {
    let registry = test_registry(None);
    let mut budget = fresh_budget(4);
    let envelope = registry.dispatch(
        "calculate_breeding_result",
        &json!({"parent_a": "Lamball", "parent_b": "Lamball"}),
        &mut budget,
    );
    assert_eq!(envelope.status, ToolStatus::Unknown);
    assert!(!envelope.uncertainty.is_empty());
}

#[test]
fn rejects_missing_breeding_parent() {
    let registry = test_registry(None);
    let mut budget = fresh_budget(4);
    let envelope = registry.dispatch(
        "calculate_breeding_result",
        &json!({"parent_a": "Lamball"}),
        &mut budget,
    );
    assert_eq!(envelope.status, ToolStatus::Error);
    assert!(envelope
        .errors
        .iter()
        .any(|error| error.contains("parent_b")));
}
