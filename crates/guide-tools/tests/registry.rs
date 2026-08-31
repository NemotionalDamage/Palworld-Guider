use game_knowledge::KnowledgeStore;
use guide_core::GuideEngine;
use guide_tools::{ToolBudget, ToolRegistry, ToolStatus};
use knowledge_index::KnowledgeIndex;
use serde_json::json;
use std::time::{Duration, Instant};

const DATA_DIRECTORY: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/reviewed");

fn test_registry(configured: Option<&str>) -> ToolRegistry {
    let store = KnowledgeStore::load_directory(DATA_DIRECTORY).expect("dataset is valid");
    let configured = configured.map(str::to_string);
    let index = KnowledgeIndex::from_store(&store, configured.clone()).expect("index builds");
    let engine = GuideEngine::new(store, configured);
    ToolRegistry::new(engine, index)
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
    ] {
        assert!(names.contains(&expected), "missing tool: {expected}");
    }
    for definition in definitions {
        assert!(!definition.description.is_empty());
        assert_eq!(definition.parameters_schema["type"], "object");
    }
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
    let envelope = registry.dispatch("get_item", &json!({"query": "Stone"}), &mut budget);
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
