use game_knowledge::KnowledgeStore;
use guide_agent::{AgentConfig, AgentLimits, AgentStatus, GuideAgent};
use guide_core::GuideEngine;
use guide_tools::{RuntimeToolResult, RuntimeToolSource, ToolDefinition, ToolRegistry, ToolStatus};
use knowledge_index::KnowledgeIndex;
use provider::{ChatProvider, ChatRequest, ChatResponse, MockProvider, ProviderError};
use serde_json::json;
use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

const DATA_DIRECTORY: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/reviewed");

struct ProviderHandle(Arc<MockProvider>);

impl ChatProvider for ProviderHandle {
    fn name(&self) -> &'static str {
        self.0.name()
    }

    fn complete(&self, request: &ChatRequest) -> Result<ChatResponse, ProviderError> {
        self.0.complete(request)
    }
}

fn test_registry(configured: Option<&str>) -> ToolRegistry {
    let store = test_store();
    let configured = configured.map(str::to_string);
    let index = KnowledgeIndex::from_store(&store, configured.clone()).expect("index builds");
    let engine = GuideEngine::new(store, configured);
    ToolRegistry::new(engine, index)
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
    let records = lines
        .iter()
        .filter(|line| !line.contains("\"record_type\":\"conflict\""))
        .map(|line| serde_json::from_str(line.as_str()))
        .collect::<Result<Vec<_>, _>>()
        .expect("conflict-free test fixtures parse");
    KnowledgeStore::from_records(records).expect("conflict-free test store validates")
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
) -> (GuideAgent, Arc<MockProvider>) {
    let provider = Arc::new(MockProvider::scripted(responses));
    let agent = GuideAgent::new(
        test_registry(configured),
        Box::new(ProviderHandle(provider.clone())),
        agent_config(max_tool_calls, max_reply_characters),
    );
    (agent, provider)
}

struct ScriptedRuntimeTools {
    data: serde_json::Value,
}

impl RuntimeToolSource for ScriptedRuntimeTools {
    fn definitions(&self) -> Vec<ToolDefinition> {
        ["get_player_status", "get_active_pal_status"]
            .into_iter()
            .map(|name| ToolDefinition {
                name: name.to_string(),
                description: "observed player and active Pal status".to_string(),
                parameters_schema: json!({"type": "object", "properties": {}, "required": []}),
            })
            .collect()
    }

    fn dispatch(&self, _name: &str, _arguments: &serde_json::Value) -> RuntimeToolResult {
        RuntimeToolResult {
            status: ToolStatus::Ok,
            data: Some(self.data.clone()),
            uncertainty: Vec::new(),
            errors: Vec::new(),
        }
    }
}

fn scripted_agent_with_runtime(
    runtime_data: serde_json::Value,
    responses: Vec<ChatResponse>,
    max_tool_calls: usize,
    max_reply_characters: usize,
) -> (GuideAgent, Arc<MockProvider>) {
    let registry = test_registry(None)
        .try_with_runtime_tools(Arc::new(ScriptedRuntimeTools { data: runtime_data }))
        .expect("runtime tools attach");
    let provider = Arc::new(MockProvider::scripted(responses));
    let agent = GuideAgent::new(
        registry,
        Box::new(ProviderHandle(provider.clone())),
        agent_config(max_tool_calls, max_reply_characters),
    );
    (agent, provider)
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
fn agent_never_receives_raw_snapshot_state() {
    let store = test_store();
    let index = KnowledgeIndex::from_store(&store, None).expect("index builds");
    let engine = GuideEngine::new(store, None);
    let snapshot = json!({
        "schema_version": "state_snapshot_v1",
        "source": {
            "kind": "user_entered",
            "captured_at": "2020-01-01T00:00:00Z",
            "game_version": "1.0.3",
            "time_to_live_seconds": 1,
            "consent": {
                "id": "operator-local-session",
                "scope": ["guide_advice"],
                "granted_at": "2020-01-01T00:00:00Z"
            }
        },
        "inventory": [{"item": "Wood", "quantity": 7, "evidence": "user_entered"}],
        "party": [{"slot": 0, "pal": "Lamball", "evidence": "user_entered"}],
        "unlocked_technologies": [
            {"technology": "Technology Level 1", "evidence": "user_entered"}
        ],
        "captured_pals": [{"pal": "Lamball", "level": 5, "evidence": "user_entered"}],
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
    });
    let registry = ToolRegistry::new(engine, index)
        .with_state_snapshot_json(&snapshot)
        .expect("snapshot attaches");
    let provider = Arc::new(MockProvider::scripted(vec![
        ChatResponse::tool("state_call", "suggest_next_goals", json!({})),
        ChatResponse::text("Use the supplied guidance; refresh your snapshot first."),
    ]));
    let agent = GuideAgent::new(
        registry,
        Box::new(ProviderHandle(provider.clone())),
        agent_config(4, 1200),
    );

    let answer = agent.ask("What should I do next?");

    assert_eq!(answer.status, AgentStatus::Unknown);
    let calls = provider.calls();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].messages[0].content, "What should I do next?");
    for call in &calls {
        let serialized = serde_json::to_string(call).expect("request serializes");
        assert!(!serialized.contains("operator-local-session"));
        assert!(!serialized.contains("consent"));
        assert!(!serialized.contains("captured_at"));
    }
}

#[test]
fn agent_can_attach_a_snapshot_after_construction() {
    let store = KnowledgeStore::load_directory(DATA_DIRECTORY).expect("dataset is valid");
    let index = KnowledgeIndex::from_store(&store, None).expect("index builds");
    let engine = GuideEngine::new(store, None);
    let registry = ToolRegistry::new(engine, index);
    let snapshot = json!({
        "schema_version": "state_snapshot_v1",
        "source": {
            "kind": "user_entered",
            "captured_at": chrono::Utc::now() - chrono::Duration::seconds(1),
            "game_version": "1.0.3",
            "time_to_live_seconds": 60,
            "consent": {
                "id": "operator-local-session",
                "scope": ["guide_advice"],
                "granted_at": chrono::Utc::now() - chrono::Duration::seconds(1)
            }
        }
    });
    let parsed = serde_json::from_value::<state_snapshot::PlayerStateSnapshot>(snapshot.clone())
        .expect("snapshot deserializes");
    let provider = Arc::new(MockProvider::scripted(vec![
        ChatResponse::tool(
            "state_call",
            "import_player_snapshot",
            json!({"confirmation": "user_entered"}),
        ),
        ChatResponse::text("Snapshot received."),
    ]));
    let mut agent = GuideAgent::new(
        registry,
        Box::new(ProviderHandle(provider.clone())),
        agent_config(4, 1200),
    );

    agent.set_state_snapshot(parsed);
    let answer = agent.ask("What state do you see?");

    assert_eq!(answer.status, AgentStatus::Ok);
    assert_eq!(provider.calls().len(), 2);
}

#[test]
fn executes_tool_request_and_returns_slot_answer() {
    let (agent, _provider) = scripted_agent(
        None,
        vec![
            ChatResponse::tool("call_1", "get_item", json!({"query": "Wood"})),
            submit_ok(&["Wood is obtained by chopping trees."], &[]),
        ],
        4,
        1200,
    );
    let answer = agent.ask("How do I get Wood?");

    assert_eq!(answer.status, AgentStatus::Ok);
    assert_eq!(
        answer.answer.as_deref(),
        Some("Wood is obtained by chopping trees.")
    );
    assert_eq!(answer.tool_calls.len(), 1);
    assert_eq!(answer.tool_calls[0].name, "get_item");
    assert_eq!(answer.tool_calls[0].status, ToolStatus::Ok);
    assert!(answer
        .provenance
        .iter()
        .any(|provenance| provenance.source_id == "SRC-PALDB-V1_0_3-20260831"));
    assert_eq!(answer.version.knowledge_version, "1.0.3");
}

#[test]
fn submit_answer_is_visible_but_not_dispatched_or_budgeted() {
    let (agent, provider) = scripted_agent(
        None,
        vec![
            ChatResponse::tool(
                "call_1",
                "calculate_materials",
                json!({"query": "Wooden Club", "quantity": 3}),
            ),
            submit_ok(&["You need {q1} Wood."], &["q1"]),
        ],
        1,
        1200,
    );
    let answer = agent.ask("Materials for 3 Wooden Clubs?");

    assert_eq!(answer.status, AgentStatus::Ok);
    assert!(provider.calls()[0]
        .tools
        .iter()
        .any(|tool| tool.name == "submit_answer"));
    assert!(answer
        .tool_calls
        .iter()
        .all(|record| record.name != "submit_answer"));
}

#[test]
fn fact_sheet_is_sent_after_each_tool_result() {
    let (agent, provider) = scripted_agent(
        None,
        vec![
            ChatResponse::tool(
                "call_1",
                "calculate_materials",
                json!({"query": "Wooden Club", "quantity": 3}),
            ),
            submit_ok(&["You need {q1} Wood."], &["q1"]),
        ],
        4,
        1200,
    );
    let answer = agent.ask("Materials for 3 Wooden Clubs?");

    assert_eq!(answer.status, AgentStatus::Ok);
    let fact_sheet_message = &provider.calls()[1].messages[2].content;
    assert!(fact_sheet_message.contains("FACT_SHEET:"));
    assert!(fact_sheet_message.contains("[q1] quantity value=15 entity=\"Wood\""));
}

#[test]
fn submit_answer_renders_quantity_slot() {
    let (agent, _provider) = scripted_agent(
        None,
        vec![
            ChatResponse::tool(
                "call_1",
                "calculate_materials",
                json!({"query": "Wooden Club", "quantity": 3}),
            ),
            submit_ok(&["You need {q1} Wood."], &["q1"]),
        ],
        4,
        1200,
    );
    let answer = agent.ask("Materials for 3 Wooden Clubs?");

    assert_eq!(answer.status, AgentStatus::Ok);
    assert_eq!(answer.answer.as_deref(), Some("You need 15 Wood."));
}

#[test]
fn entity_ids_authorize_multiword_display_names() {
    let (agent, provider) = scripted_agent(
        None,
        vec![
            ChatResponse::tool("call_1", "get_pal", json!({"query": "Lamball"})),
            submit_ok(
                &["Lamball drops Lamball Mutton and Wool for your base."],
                &[],
            ),
        ],
        4,
        1200,
    );
    let answer = agent.ask("What can Lamball do for me?");

    assert_eq!(answer.status, AgentStatus::Ok);
    assert!(
        answer.answer.as_deref().unwrap().contains("Lamball Mutton"),
        "answer: {:?}",
        answer.answer
    );
    assert!(
        !answer
            .uncertainty
            .iter()
            .any(|message| message.contains("model draft invalid")),
        "uncertainty: {:?}",
        answer.uncertainty
    );
    let fact_sheet_message = &provider.calls()[1].messages[2].content;
    assert!(fact_sheet_message.contains("entity name=\"Lamball Mutton\""));
}

#[test]
fn numeric_tampering_has_no_render_path() {
    let (agent, _provider) = scripted_agent(
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
        .any(|message| message.contains("numeric literal")));
}

#[test]
fn unknown_slot_falls_back_to_fact_sheet() {
    let (agent, _provider) = scripted_agent(
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
        .any(|message| message.contains("unknown slot q9")));
}

#[test]
fn number_word_in_draft_falls_back() {
    let (agent, _provider) = scripted_agent(
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
    let answer = agent.ask("How do I get Wood?");

    assert_eq!(answer.status, AgentStatus::Unknown);
    assert!(answer
        .answer
        .as_deref()
        .unwrap()
        .contains("I don't have enough reviewed data"));
    assert!(answer
        .uncertainty
        .iter()
        .any(|message| message.contains("numeric word")));
}

#[test]
fn chinese_number_word_in_draft_falls_back() {
    let (agent, _provider) = scripted_agent(
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
                    "sentences": ["需要一个 Wood。"],
                    "slots": []
                }),
            ),
        ],
        4,
        1200,
    );
    let answer = agent.ask("需要多少 Wood？");

    assert_eq!(answer.status, AgentStatus::Ok);
    assert!(answer.answer.as_deref().unwrap().contains("Need 15 Wood."));
    assert!(answer
        .uncertainty
        .iter()
        .any(|message| message.contains("numeric word")));
}

#[test]
fn model_returns_free_text_without_submit_answer() {
    let (agent, _provider) = scripted_agent(
        None,
        vec![
            ChatResponse::tool("call_1", "get_item", json!({"query": "Wood"})),
            ChatResponse::text("Wood comes from chopping trees."),
        ],
        4,
        1200,
    );
    let answer = agent.ask("How do I get Wood?");

    assert_eq!(answer.status, AgentStatus::Ok);
    assert!(answer.answer.is_some());
    assert!(answer
        .uncertainty
        .iter()
        .any(|message| message.contains("model did not call submit_answer")));
}

#[test]
fn empty_free_text_uses_fallback() {
    let (agent, _provider) = scripted_agent(None, vec![ChatResponse::text("   ")], 4, 1200);
    let answer = agent.ask("What is Wood?");

    assert_eq!(answer.status, AgentStatus::Unknown);
    assert!(answer
        .answer
        .as_deref()
        .unwrap()
        .contains("I don't have enough reviewed data"));
    assert!(answer
        .uncertainty
        .iter()
        .any(|message| message.contains("model did not call submit_answer")));
}

#[test]
fn answers_without_tool_evidence_are_flagged() {
    let (agent, _provider) =
        scripted_agent(None, vec![submit_ok(&["I don't know."], &[])], 4, 1200);
    let answer = agent.ask("Any tips?");

    assert_eq!(answer.status, AgentStatus::Unknown);
    assert!(answer
        .uncertainty
        .iter()
        .any(|message| message.contains("no deterministic tool evidence")));
}

#[test]
fn steps_are_numbered_by_renderer() {
    let (agent, _provider) = scripted_agent(
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
                    "sentences": ["Gather materials:"],
                    "steps": ["Collect {q1} Wood.", "Craft the club."],
                    "slots": ["q1"]
                }),
            ),
        ],
        4,
        1200,
    );
    let answer = agent.ask("Materials for 3 Wooden Clubs?");

    assert_eq!(
        answer.answer.as_deref(),
        Some("Gather materials:\n1. Collect 15 Wood.\n2. Craft the club.")
    );
}

#[test]
fn position_slots_render_exact_observation() {
    let (agent, _provider) = scripted_agent_with_runtime(
        json!({"position": {"x": -1.5, "y": 2.25, "z": 3.0}}),
        vec![
            ChatResponse::tool("call_1", "get_player_status", json!({})),
            ChatResponse::tool(
                "submit_answer",
                "submit_answer",
                json!({
                    "status": "ok",
                    "sentences": ["Position: {o1}, {o2}, {o3}."],
                    "slots": ["o1", "o2", "o3"]
                }),
            ),
        ],
        4,
        1200,
    );
    let answer = agent.ask("Where is my player?");

    assert_eq!(answer.status, AgentStatus::Ok);
    assert!(answer
        .answer
        .as_deref()
        .unwrap()
        .starts_with("Position: x=-1.5, y=2.25, z=3"));
}

#[test]
fn hand_written_position_falls_back_to_exact_observation() {
    let (agent, _provider) = scripted_agent_with_runtime(
        json!({"position": {"x": -1.5, "y": 2.25, "z": 3.0}}),
        vec![
            ChatResponse::tool("call_1", "get_player_status", json!({})),
            submit_ok(&["Position: x=-1.5, y=2.25, z=4."], &[]),
        ],
        4,
        1200,
    );
    let answer = agent.ask("Where is my player?");

    assert_eq!(answer.status, AgentStatus::Ok);
    assert!(answer
        .answer
        .as_deref()
        .unwrap()
        .starts_with("Position: x=-1.5, y=2.25, z=3"));
    assert!(answer
        .uncertainty
        .iter()
        .any(|message| message.contains("numeric literal")));
}

#[test]
fn entity_outside_evidence_falls_back() {
    let (agent, _provider) = scripted_agent(
        None,
        vec![
            ChatResponse::tool("call_1", "get_item", json!({"query": "Wood"})),
            submit_ok(&["Wool is the better choice."], &[]),
        ],
        4,
        1200,
    );
    let answer = agent.ask("What should I gather?");

    assert_eq!(answer.status, AgentStatus::Ok);
    assert!(answer
        .answer
        .as_deref()
        .unwrap()
        .contains("Related reviewed records: Wood"));
    assert!(answer
        .uncertainty
        .iter()
        .any(|message| message.contains("unsupported entity Wool")));
}

#[test]
fn version_slots_are_rust_rendered() {
    let (agent, _provider) = scripted_agent(
        None,
        vec![
            ChatResponse::tool("call_1", "get_item", json!({"query": "Wood"})),
            submit_ok(&["Guide data version: {v1}."], &["v1"]),
        ],
        4,
        1200,
    );
    let answer = agent.ask("What version is the data?");

    assert_eq!(answer.status, AgentStatus::Ok);
    assert_eq!(answer.answer.as_deref(), Some("Guide data version: 1.0.3."));
}

#[test]
fn quantities_route_through_deterministic_calculator() {
    let (agent, _provider) = scripted_agent(
        None,
        vec![
            ChatResponse::tool(
                "call_1",
                "calculate_materials",
                json!({"query": "Wooden Club", "quantity": 3}),
            ),
            submit_ok(&["You need {q1} Wood."], &["q1"]),
        ],
        4,
        1200,
    );
    let answer = agent.ask("Materials for 3 Wooden Clubs?");

    assert_eq!(answer.status, AgentStatus::Ok);
    let record = &answer.tool_calls[0];
    assert_eq!(record.name, "calculate_materials");
    assert_eq!(
        record.data.as_ref().expect("calculation data")["totals"][0]["required_quantity"],
        15
    );
}

#[test]
fn model_cannot_bypass_tool_registry() {
    let (agent, _provider) = scripted_agent(
        None,
        vec![
            ChatResponse::tool("call_1", "delete_save", json!({})),
            ChatResponse::tool(
                "submit_answer",
                "submit_answer",
                json!({
                    "status": "unknown",
                    "sentences": ["That action is unavailable."],
                    "slots": []
                }),
            ),
        ],
        4,
        1200,
    );
    let answer = agent.ask("Delete my save.");

    assert_eq!(answer.status, AgentStatus::Unknown);
    assert_eq!(answer.tool_calls.len(), 1);
    assert_eq!(answer.tool_calls[0].status, ToolStatus::Error);
    assert!(answer
        .errors
        .iter()
        .any(|error| error.contains("unknown tool")));
    assert!(answer.answer.is_some());
}

#[test]
fn budget_exhaustion_uses_fallback() {
    let (agent, _provider) = scripted_agent(
        None,
        vec![
            ChatResponse::tool("call_1", "get_item", json!({"query": "Wood"})),
            ChatResponse::tool("call_2", "get_item", json!({"query": "Wool"})),
        ],
        1,
        1200,
    );
    let answer = agent.ask("Tell me everything.");

    assert_eq!(answer.status, AgentStatus::Ok);
    assert!(answer
        .answer
        .as_deref()
        .unwrap()
        .contains("Related reviewed records: Wood"));
    assert!(answer
        .errors
        .iter()
        .any(|error| error.contains("tool call budget exhausted")));
    assert_eq!(answer.tool_calls.len(), 2);
}

#[test]
fn provider_failure_is_clear_and_non_fatal() {
    let (agent, _provider) = scripted_agent(None, Vec::new(), 4, 1200);
    let answer = agent.ask("What is Wood?");

    assert_eq!(answer.status, AgentStatus::Error);
    assert!(answer
        .errors
        .iter()
        .any(|error| error.contains("mock provider script exhausted")));
    assert!(answer.tool_calls.is_empty());
}

#[test]
fn unknown_knowledge_propagates_to_answer() {
    let (agent, _provider) = scripted_agent(
        None,
        vec![
            ChatResponse::tool("call_1", "get_item", json!({"query": "Unreviewed Thing"})),
            ChatResponse::tool(
                "submit_answer",
                "submit_answer",
                json!({
                    "status": "unknown",
                    "sentences": ["Reviewed knowledge is unavailable."],
                    "slots": []
                }),
            ),
        ],
        4,
        1200,
    );
    let answer = agent.ask("What is Unreviewed Thing?");

    assert_eq!(answer.status, AgentStatus::Unknown);
    assert_eq!(answer.tool_calls[0].status, ToolStatus::Unknown);
    assert!(answer
        .uncertainty
        .iter()
        .any(|message| message.contains("unknown item")));
}

#[test]
fn cancellation_stops_before_provider_calls() {
    let (agent, provider) = scripted_agent(None, vec![ChatResponse::text("Too late.")], 4, 1200);
    let cancelled = AtomicBool::new(true);
    let answer = agent.ask_with_cancellation("What is Wood?", &cancelled);

    assert_eq!(answer.status, AgentStatus::Error);
    assert!(answer
        .errors
        .iter()
        .any(|error| error.contains("cancelled")));
    assert!(provider.calls().is_empty());
    assert!(cancelled.load(Ordering::SeqCst));
}

#[test]
fn timeout_stops_before_provider_calls() {
    let provider = Arc::new(MockProvider::scripted(vec![ChatResponse::text(
        "Too late.",
    )]));
    let agent = GuideAgent::new(
        test_registry(None),
        Box::new(ProviderHandle(provider.clone())),
        AgentConfig {
            limits: AgentLimits {
                max_tool_calls: 4,
                timeout: Duration::ZERO,
            },
            max_reply_characters: 1200,
        },
    );
    let answer = agent.ask("What is Wood?");

    assert_eq!(answer.status, AgentStatus::Error);
    assert!(answer
        .errors
        .iter()
        .any(|error| error.contains("timeout exceeded")));
    assert!(provider.calls().is_empty());
}

#[test]
fn stale_version_propagates_to_answer() {
    let (agent, _provider) = scripted_agent(
        Some("0.9"),
        vec![
            ChatResponse::tool("call_1", "get_item", json!({"query": "Wood"})),
            submit_ok(&["Wood is useful, but the data may be stale."], &[]),
        ],
        4,
        1200,
    );
    let answer = agent.ask("How do I get Wood?");

    assert!(!answer.version.matches);
    assert!(answer
        .uncertainty
        .iter()
        .any(|message| message.contains("does not match configured game version 0.9")));
}

#[test]
fn reply_truncation_is_applied_by_renderer() {
    let (agent, _provider) = scripted_agent(
        None,
        vec![submit_ok(
            &["Wood is useful for building and crafting."],
            &[],
        )],
        4,
        10,
    );
    let answer = agent.ask("What is Wood?");

    assert_eq!(answer.answer.as_deref().map(str::len), Some(10));
}
