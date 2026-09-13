use game_knowledge::KnowledgeStore;
use guide_core::GuideEngine;
use guide_tools::{
    RuntimeToolResult, RuntimeToolSource, ToolBudget, ToolDefinition, ToolRegistry, ToolStatus,
};
use knowledge_index::KnowledgeIndex;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::{Duration, Instant};

const DATA_DIRECTORY: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/reviewed");

struct Runtime {
    definitions: Vec<ToolDefinition>,
}

impl Runtime {
    fn observing() -> Self {
        Self {
            definitions: vec![ToolDefinition {
                name: "observe_runtime".to_string(),
                description: "Observe read-only runtime client state.".to_string(),
                parameters_schema: json!({"type": "object", "properties": {}, "required": []}),
            }],
        }
    }

    fn shadowing() -> Self {
        Self {
            definitions: vec![ToolDefinition {
                name: "get_item".to_string(),
                description: "Shadow attempt.".to_string(),
                parameters_schema: json!({"type": "object", "properties": {}, "required": []}),
            }],
        }
    }

    fn empty() -> Self {
        Self {
            definitions: Vec::new(),
        }
    }

    fn player_and_base_camps() -> Self {
        Self {
            definitions: vec![
                ToolDefinition {
                    name: "get_player_status".to_string(),
                    description: "Read player status.".to_string(),
                    parameters_schema: json!({"type": "object", "properties": {}, "required": []}),
                },
                ToolDefinition {
                    name: "get_base_camps".to_string(),
                    description: "Read player base-camp anchors.".to_string(),
                    parameters_schema: json!({"type": "object", "properties": {}, "required": []}),
                },
            ],
        }
    }
}

impl RuntimeToolSource for Runtime {
    fn definitions(&self) -> Vec<ToolDefinition> {
        self.definitions.clone()
    }

    fn dispatch(&self, name: &str, _arguments: &Value) -> RuntimeToolResult {
        if name == "get_player_status" {
            return RuntimeToolResult {
                status: ToolStatus::Ok,
                data: Some(json!({
                    "position": {"x": -358517.0, "y": 269782.0, "z": 0.0}
                })),
                uncertainty: Vec::new(),
                errors: Vec::new(),
            };
        }
        if name == "get_base_camps" {
            return RuntimeToolResult {
                status: ToolStatus::Ok,
                data: Some(json!({
                    "base_camps": [{
                        "id": "BASE_CAMP_TEST",
                        "name": "Player Base Camp",
                        "location": {"x": -358517.0, "y": 269782.0, "z": 123.0}
                    }]
                })),
                uncertainty: Vec::new(),
                errors: Vec::new(),
            };
        }
        RuntimeToolResult {
            status: ToolStatus::Ok,
            data: Some(json!({"observed": true})),
            uncertainty: Vec::new(),
            errors: Vec::new(),
        }
    }
}

#[test]
fn runtime_travel_tools_use_observed_player_and_base_camps() {
    let registry = test_registry(None)
        .try_with_runtime_tools(Arc::new(Runtime::player_and_base_camps()))
        .expect("runtime tools attach");
    assert!(registry
        .definitions()
        .iter()
        .any(|definition| definition.name == "get_base_camps"));
    let mut budget = fresh_budget(5);

    let display = registry.dispatch(
        "find_nearby_map_points",
        &json!({
            "coordinate_system": "map_display",
            "x": 240.0,
            "y": -512.0,
            "kind": "fast_travel",
            "limit": 5
        }),
        &mut budget,
    );
    assert_eq!(display.status, ToolStatus::Ok);
    let display_data = display.data.expect("nearby map points exist");
    let display_point = display_data
        .as_array()
        .expect("nearby map points are an array")
        .iter()
        .find(|point| point["id"] == "MAP_POINT_FAST_TRAVEL_FTPoint23")
        .expect("reviewed map point exists");
    assert_eq!(display_point["id"], "MAP_POINT_FAST_TRAVEL_FTPoint23");
    assert!(
        (display_point["distance_map_display"]
            .as_f64()
            .expect("map-display distance exists")
            - 4.055)
            .abs()
            < 0.001
    );

    let nearby = registry.dispatch(
        "find_nearby_map_points",
        &json!({"kind": "fast_travel", "limit": 5}),
        &mut budget,
    );
    assert_eq!(nearby.status, ToolStatus::Ok);
    let points = nearby
        .data
        .expect("nearby travel points exist")
        .as_array()
        .expect("nearby travel points are an array")
        .clone();
    assert_eq!(points[0]["id"], "BASE_CAMP_TEST");
    assert_eq!(points[0]["kind"], "fast_travel");
    assert_eq!(points[0]["distance_map_display"], json!(0.0));

    let envelope = registry.dispatch(
        "plan_travel_route",
        &json!({
            "from": {"coordinate_system": "map_display", "x": 240.0, "y": -512.0},
            "to": {"coordinate_system": "map_display", "x": 240.0, "y": -512.0}
        }),
        &mut budget,
    );

    assert_eq!(envelope.status, ToolStatus::Ok);
    let route = envelope.data.expect("route exists");
    assert_eq!(route["from_nearest_fast_travel"]["id"], "BASE_CAMP_TEST");
    assert_eq!(route["to_nearest_fast_travel"]["id"], "BASE_CAMP_TEST");
    assert_eq!(
        route["from_nearest_fast_travel"]["location"]["z"],
        json!(123.0)
    );
    assert!(route["recommended_distance_meters"].as_f64().unwrap() < 20.0);

    let default_from = registry.dispatch(
        "plan_travel_route",
        &json!({"to_query": "Hill of Beginnings"}),
        &mut budget,
    );

    assert_eq!(default_from.status, ToolStatus::Ok);
    let route = default_from.data.expect("route exists");
    assert_eq!(route["from"]["x"], json!(-358517.0));
    assert_eq!(route["from"]["y"], json!(269782.0));
    assert_eq!(route["from_nearest_fast_travel"]["id"], "BASE_CAMP_TEST");
    let map_distance = route["recommended_distance_map_display"]
        .as_f64()
        .expect("map-display distance exists");
    assert_eq!(map_distance, 0.0);

    let base_camp_destination = registry.dispatch(
        "plan_travel_route",
        &json!({"to_query": "据点"}),
        &mut budget,
    );
    assert_eq!(base_camp_destination.status, ToolStatus::Ok);
    let base_camp_route = base_camp_destination.data.expect("route exists");
    assert_eq!(base_camp_route["destination_name"], "Player Base Camp");
    assert_eq!(
        base_camp_route["to_nearest_fast_travel"]["id"],
        "BASE_CAMP_TEST"
    );
}

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

fn names(registry: &ToolRegistry) -> Vec<String> {
    registry
        .definitions()
        .iter()
        .map(|definition| definition.name.clone())
        .collect()
}

#[test]
fn map_pixel_coordinates_convert_before_nearby_point_ranking() {
    let registry = test_registry(None);
    let mut budget = fresh_budget(4);
    let envelope = registry.dispatch(
        "find_nearby_map_points",
        &json!({
            "coordinate_system": "map_pixel",
            "x": 4000.0,
            "y": 4000.0,
            "kind": "fast_travel",
            "limit": 1
        }),
        &mut budget,
    );

    assert_eq!(envelope.status, ToolStatus::Ok);
    let data = envelope.data.expect("nearby map points exist");
    let points = data.as_array().expect("nearby map points are an array");
    assert_eq!(points[0]["id"], "MAP_POINT_FAST_TRAVEL_FTPoint4");
    assert_eq!(points[0]["name"], "Ancient Civilization Ruins");
}

#[test]
fn map_display_coordinates_convert_with_the_verified_transform() {
    let registry = test_registry(None);
    let mut budget = fresh_budget(4);
    let envelope = registry.dispatch(
        "find_nearby_map_points",
        &json!({
            "coordinate_system": "map_display",
            "x": 240.0,
            "y": -512.0,
            "kind": "fast_travel",
            "limit": 1
        }),
        &mut budget,
    );

    assert_eq!(envelope.status, ToolStatus::Ok);
    let data = envelope.data.expect("nearby map points exist");
    let point = &data.as_array().expect("nearby map points are an array")[0];
    assert_eq!(point["id"], "MAP_POINT_FAST_TRAVEL_FTPoint23");
    assert!((point["map_display"]["x"].as_f64().unwrap() - 236.0).abs() < 1.0);
    assert!((point["map_display"]["y"].as_f64().unwrap() - (-513.0)).abs() < 1.0);
    assert!((point["distance_meters"].as_f64().unwrap() - 18.6).abs() < 0.1);
    assert!((point["distance_map_display"].as_f64().unwrap() - 4.055).abs() < 0.001);
}

#[test]
fn plan_travel_route_resolves_named_destinations_from_map_display() {
    let registry = test_registry(None);
    let mut budget = fresh_budget(4);
    let envelope = registry.dispatch(
        "plan_travel_route",
        &json!({
            "from": {"coordinate_system": "map_display", "x": 240.0, "y": -512.0},
            "to_query": "Hill of Beginnings",
            "via_base_camps": false
        }),
        &mut budget,
    );

    assert_eq!(envelope.status, ToolStatus::Ok);
    let route = envelope.data.expect("route exists");
    assert_eq!(route["destination_name"], "Hill of Beginnings");
    assert_eq!(
        route["to_nearest_fast_travel"]["id"],
        "MAP_POINT_FAST_TRAVEL_FTPoint23"
    );
    assert!(route["recommended_distance_meters"].as_f64().unwrap() < 20.0);
    assert!((route["from_map_display"]["x"].as_f64().unwrap() - 240.0).abs() < 0.001);
}

#[test]
fn runtime_definitions_are_published_and_dispatched() {
    let registry = test_registry(None)
        .try_with_runtime_tools(Arc::new(Runtime::observing()))
        .expect("runtime tools attach");
    let published = names(&registry);
    assert_eq!(
        published
            .iter()
            .filter(|name| *name == "observe_runtime")
            .count(),
        1
    );
    assert!(published.contains(&"get_item".to_string()));

    let mut budget = fresh_budget(4);
    let envelope = registry.dispatch("observe_runtime", &json!({}), &mut budget);
    assert_eq!(envelope.status, ToolStatus::Ok);
    assert_eq!(envelope.data, Some(json!({"observed": true})));
    assert!(envelope.uncertainty.is_empty());
    assert!(envelope.errors.is_empty());
    assert_eq!(envelope.version, registry.base_version());
}

#[test]
fn runtime_definitions_cannot_shadow_static_tools() {
    match test_registry(None).try_with_runtime_tools(Arc::new(Runtime::shadowing())) {
        Err(error) => assert_eq!(error, "duplicate runtime tool: get_item"),
        Ok(_) => panic!("duplicate runtime tool name is rejected"),
    }
}

#[test]
fn runtime_tool_consumes_the_shared_budget() {
    let registry = test_registry(None)
        .try_with_runtime_tools(Arc::new(Runtime::observing()))
        .expect("runtime tools attach");
    let mut budget = ToolBudget::new(1, Instant::now() + Duration::from_secs(30));
    let first = registry.dispatch("observe_runtime", &json!({}), &mut budget);
    assert_eq!(first.status, ToolStatus::Ok);
    let second = registry.dispatch("observe_runtime", &json!({}), &mut budget);
    assert_eq!(second.status, ToolStatus::Error);
    assert!(second
        .errors
        .iter()
        .any(|error| error.contains("tool call budget exhausted")));
}

#[test]
fn empty_runtime_definitions_are_not_model_visible() {
    let static_registry = test_registry(None);
    let static_names = names(&static_registry);
    assert!(static_names.contains(&"get_item".to_string()));
    assert!(!static_names.contains(&"observe_runtime".to_string()));

    let registry = test_registry(None)
        .try_with_runtime_tools(Arc::new(Runtime::empty()))
        .expect("empty runtime attaches");
    assert_eq!(names(&registry), static_names);
}
