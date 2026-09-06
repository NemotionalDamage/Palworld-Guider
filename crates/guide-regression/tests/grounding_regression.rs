use game_knowledge::KnowledgeStore;
use guide_agent::{AgentConfig, AgentLimits, AgentStatus, GuideAgent};
use guide_core::GuideEngine;
use guide_tools::ToolRegistry;
use knowledge_index::KnowledgeIndex;
use provider::{ChatProvider, ChatRequest, ChatResponse, MockProvider};
use serde_json::{json, Value};
use std::fs;
use std::sync::Arc;
use std::time::Duration;

const DATA_DIRECTORY: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/reviewed");

struct ProviderHandle(Arc<MockProvider>);

impl ChatProvider for ProviderHandle {
    fn name(&self) -> &'static str {
        self.0.name()
    }
    fn complete(&self, request: &ChatRequest) -> Result<ChatResponse, provider::ProviderError> {
        self.0.complete(request)
    }
}

fn test_store() -> KnowledgeStore {
    let mut lines = Vec::new();
    for file_name in ["sources.jsonl", "facts.jsonl"] {
        lines.extend(
            fs::read_to_string(format!("{DATA_DIRECTORY}/{file_name}"))
                .expect("canonical dataset reads")
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
        .expect("conflict-free test fixtures parse");
    KnowledgeStore::from_records(records).expect("conflict-free test store validates")
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

fn test_registry(configured: Option<&str>) -> ToolRegistry {
    let store = test_store();
    let configured = configured.map(str::to_string);
    let index = KnowledgeIndex::from_store(&store, configured.clone()).expect("index builds");
    let engine = GuideEngine::new(store, configured);
    ToolRegistry::new(engine, index)
}

fn agent_config(max_tool_calls: usize, max_reply_characters: usize) -> AgentConfig {
    AgentConfig {
        limits: AgentLimits {
            max_tool_calls,
            timeout: Duration::from_secs(30),
        },
        max_reply_characters,
    }
}

fn scripted_agent(
    configured: Option<&str>,
    responses: Vec<ChatResponse>,
    max_tool_calls: usize,
    max_reply_characters: usize,
) -> GuideAgent {
    let provider = Arc::new(MockProvider::scripted(responses));
    GuideAgent::new(
        test_registry(configured),
        Box::new(ProviderHandle(provider.clone())),
        agent_config(max_tool_calls, max_reply_characters),
    )
}

fn submit_ok(sentences: &[&str], slots: &[&str]) -> ChatResponse {
    ChatResponse::tool(
        "submit_answer",
        "submit_answer",
        json!({
            "status": "ok",
            "sentences": sentences,
            "slots": slots
        }),
    )
}

#[test]
fn numeric_literal_in_draft_triggers_fallback() {
    let agent = scripted_agent(
        None,
        vec![
            ChatResponse::tool(
                "call_1",
                "calculate_materials",
                json!({"query": "Wooden Club", "quantity": 3}),
            ),
            submit_ok(&["You need 12 Wood."], &[]),
        ],
        4,
        1200,
    );
    let answer = agent.ask("Materials for 3 Wooden Clubs?");
    assert_eq!(answer.status, AgentStatus::Ok);
    assert!(answer.answer.as_deref().unwrap().contains("Need 15 Wood."));
    assert!(answer
        .uncertainty
        .iter()
        .any(|m| m.contains("numeric literal")));
}

#[test]
fn number_word_in_draft_triggers_fallback() {
    let agent = scripted_agent(
        None,
        vec![ChatResponse::tool(
            "submit_answer",
            "submit_answer",
            json!({
                "status": "unknown",
                "sentences": ["This is one of the most common options."],
                "slots": []
            }),
        )],
        4,
        1200,
    );
    let answer = agent.ask("What is Wood?");
    assert_eq!(answer.status, AgentStatus::Unknown);
    assert!(answer
        .uncertainty
        .iter()
        .any(|m| m.contains("numeric word")));
}

#[test]
fn unknown_slot_triggers_fallback() {
    let agent = scripted_agent(
        None,
        vec![
            ChatResponse::tool(
                "call_1",
                "calculate_materials",
                json!({"query": "Wooden Club", "quantity": 3}),
            ),
            submit_ok(&["You need {q9} Wood."], &["q9"]),
        ],
        4,
        1200,
    );
    let answer = agent.ask("Materials for 3 Wooden Clubs?");
    assert_eq!(answer.status, AgentStatus::Ok);
    assert!(answer.answer.as_deref().unwrap().contains("Need 15 Wood."));
    assert!(answer
        .uncertainty
        .iter()
        .any(|m| m.contains("unknown slot q9")));
}

#[test]
fn fallback_produces_player_facing_text_without_internal_markers() {
    let agent = scripted_agent(
        None,
        vec![
            ChatResponse::tool(
                "call_1",
                "calculate_materials",
                json!({"query": "Wooden Club", "quantity": 3}),
            ),
            submit_ok(&["You need 12 Wood."], &[]),
        ],
        4,
        1200,
    );
    let answer = agent.ask("Materials for 3 Wooden Clubs?");
    let text = answer.answer.as_deref().unwrap();
    assert!(!text.contains("{v1}"));
    assert!(!text.contains("numeric literal"));
    assert!(!text.contains("RenderError"));
    assert!(!text.contains("FactSlot"));
}

#[test]
fn empty_draft_triggers_fallback() {
    let agent = scripted_agent(
        None,
        vec![
            ChatResponse::tool(
                "call_1",
                "calculate_materials",
                json!({"query": "Wooden Club", "quantity": 3}),
            ),
            ChatResponse::tool(
                "submit_answer",
                "submit_answer",
                json!({
                    "status": "ok",
                    "sentences": [""],
                    "slots": []
                }),
            ),
        ],
        4,
        1200,
    );
    let answer = agent.ask("Materials for 3 Wooden Clubs?");
    assert!(answer.answer.is_some());
    let text = answer.answer.as_deref().unwrap();
    assert!(!text.is_empty());
}

#[test]
fn provider_failure_returns_error_answer() {
    let agent = scripted_agent(None, vec![], 4, 1200);
    let answer = agent.ask("What is Wood?");
    assert_eq!(answer.status, AgentStatus::Error);
    assert!(answer.answer.is_none());
    assert!(!answer.errors.is_empty());
}

#[test]
fn budget_exhaustion_produces_fallback() {
    let agent = scripted_agent(
        None,
        vec![
            ChatResponse::tool("call_1", "get_item", json!({"query": "wood"})),
            ChatResponse::tool("call_2", "get_item", json!({"query": "wood"})),
            ChatResponse::tool("call_3", "get_item", json!({"query": "wood"})),
        ],
        2,
        1200,
    );
    let answer = agent.ask("What is Wood?");
    assert!(answer.answer.is_some());
    assert!(answer
        .uncertainty
        .iter()
        .any(|m| m.contains("budget exhausted") || m.contains("no deterministic tool evidence")));
}
