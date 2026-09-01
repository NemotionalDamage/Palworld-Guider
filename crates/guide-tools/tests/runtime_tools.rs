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
}

impl RuntimeToolSource for Runtime {
    fn definitions(&self) -> Vec<ToolDefinition> {
        self.definitions.clone()
    }

    fn dispatch(&self, _name: &str, _arguments: &Value) -> RuntimeToolResult {
        RuntimeToolResult {
            status: ToolStatus::Ok,
            data: Some(json!({"observed": true})),
            uncertainty: Vec::new(),
            errors: Vec::new(),
        }
    }
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
