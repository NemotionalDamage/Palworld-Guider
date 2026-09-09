use game_knowledge::KnowledgeStore;
use guide_agent::{AgentConfig, AgentHistoryEntry, AgentLimits, AgentStatus, GuideAgent};
use guide_core::GuideEngine;
use guide_tools::{RuntimeToolResult, RuntimeToolSource, ToolDefinition, ToolRegistry, ToolStatus};
use knowledge_index::KnowledgeIndex;
use provider::{ChatProvider, ChatRequest, ChatResponse, MockProvider, ProviderError};
use serde_json::{json, Value};
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

struct InvalidThenValidProvider {
    calls: std::sync::Mutex<Vec<ChatRequest>>,
}

impl ChatProvider for InvalidThenValidProvider {
    fn name(&self) -> &'static str {
        "invalid-then-valid"
    }

    fn complete(&self, request: &ChatRequest) -> Result<ChatResponse, ProviderError> {
        let mut calls = self
            .calls
            .lock()
            .expect("invalid-then-valid call log is not poisoned");
        calls.push(request.clone());
        if calls.len() == 1 {
            return Err(ProviderError::InvalidResponse(
                "invalid tool arguments JSON".to_string(),
            ));
        }
        Ok(ChatResponse::text("Wood is useful."))
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
    lines.extend(referenced_habitat_support_lines(&lines));
    let records = lines
        .iter()
        .filter(|line| !line.contains("\"record_type\":\"conflict\""))
        .map(|line| serde_json::from_str(line.as_str()))
        .collect::<Result<Vec<_>, _>>()
        .expect("conflict-free test fixtures parse");
    KnowledgeStore::from_records(records).expect("conflict-free test store validates")
}

fn test_store_with_extra_lines(extra_lines: &[&str]) -> KnowledgeStore {
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
    lines.extend(extra_lines.iter().map(|line| line.to_string()));
    let records = lines
        .iter()
        .filter(|line| !line.contains("\"record_type\":\"conflict\""))
        .map(|line| serde_json::from_str(line.as_str()))
        .collect::<Result<Vec<_>, _>>()
        .expect("test fixtures parse");
    KnowledgeStore::from_records(records).expect("test store with extra lines validates")
}

fn referenced_habitat_support_lines(base_lines: &[String]) -> Vec<String> {
    let mut needed_zones = std::collections::BTreeSet::new();
    for line in base_lines {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
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
                serde_json::from_str::<serde_json::Value>(line)
                    .ok()
                    .and_then(|value| value.get("id").and_then(Value::as_str).map(str::to_string))
                    .is_some_and(|id| needed_zones.contains(&id))
            })
            .map(str::to_string),
    );
    support
}

fn registry_from_store(store: KnowledgeStore, configured: Option<&str>) -> ToolRegistry {
    let configured = configured.map(str::to_string);
    let index = KnowledgeIndex::from_store(&store, configured.clone()).expect("index builds");
    let engine = GuideEngine::new(store, configured);
    ToolRegistry::new(engine, index)
}

fn scripted_agent_with_store(
    store: KnowledgeStore,
    configured: Option<&str>,
    responses: Vec<ChatResponse>,
    max_tool_calls: usize,
    max_reply_characters: usize,
) -> (GuideAgent, Arc<MockProvider>) {
    let provider = Arc::new(MockProvider::scripted(responses));
    let agent = GuideAgent::new(
        registry_from_store(store, configured),
        Box::new(ProviderHandle(provider.clone())),
        agent_config(max_tool_calls, max_reply_characters),
    );
    (agent, provider)
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

fn submit_ok(sentences: &[&str], _legacy_slots: &[&str]) -> ChatResponse {
    ChatResponse::tool(
        "submit_answer",
        "submit_answer",
        json!({
            "status": "ok",
            "sentences": sentences,
        }),
    )
}

#[test]
fn map_pixel_questions_are_grounded_before_model_tool_selection() {
    let near = r#"{"record_type":"map_point","id":"MAP_POINT_NEAR","map_id":"MAP_MAIN","native_id":"Near","kind":"fast_travel","names":{"en":"Ancient Civilization Ruins","zh_hans":"古代文明遗址"},"location":{"x":-391978.125,"y":-16978.125,"z":0.0},"local_evidence":{"source_table":"test","localization_status":"resolved","unresolved_fields":[],"transformation_notes":"test"},"provenance":{"source_id":"SRC-LOCAL-BUILD-MAP-24575825-20260906","applicable_game_version":"1.0.3","retrieved_on":"2026-09-06","reviewer":"Codex","review_status":"reviewed","confidence":"verified_target","change_risk":null}}"#;
    let snowfield = r#"{"record_type":"map_point","id":"MAP_POINT_SNOWFIELD","map_id":"MAP_MAIN","native_id":"Snowfield","kind":"fast_travel","names":{"en":"Pristine Snow Field","zh_hans":"纯白雪原"},"location":{"x":0.0,"y":0.0,"z":0.0},"local_evidence":{"source_table":"test","localization_status":"resolved","unresolved_fields":[],"transformation_notes":"test"},"provenance":{"source_id":"SRC-LOCAL-BUILD-MAP-24575825-20260906","applicable_game_version":"1.0.3","retrieved_on":"2026-09-06","reviewer":"Codex","review_status":"reviewed","confidence":"verified_target","change_risk":null}}"#;
    let store = test_store_with_extra_lines(&[near, snowfield]);
    let (agent, provider) = scripted_agent_with_store(
        store,
        None,
        vec![submit_ok(&["最近的传送点是古代文明遗址。"], &[])],
        4,
        1200,
    );

    let answer = agent.ask("地图坐标 4000，4000 附近最近的传送点是什么？");

    assert_eq!(answer.status, AgentStatus::Ok);
    let calls = provider.calls();
    assert!(calls[0].messages[0]
        .content
        .contains("[find_nearby_map_points]"));
    assert!(calls[0].messages[0]
        .content
        .contains("Ancient Civilization Ruins"));
    assert!(calls[0].messages[0].content.contains("古代文明遗址"));
    assert!(calls[0].messages[0].content.contains("\"pixel_x\":4000"));
    let exposed_tools = calls[0]
        .tools
        .iter()
        .map(|tool| tool.name.as_str())
        .collect::<Vec<_>>();
    assert!(!exposed_tools.contains(&"resolve_name"));
    assert!(!exposed_tools.contains(&"find_nearby_map_points"));
    assert!(!exposed_tools.contains(&"locate_coordinate"));
    assert!(!exposed_tools.contains(&"find_pal_spawn_zones"));
    assert!(!exposed_tools.contains(&"calculate_materials"));
}

#[test]
fn unlabeled_map_display_units_are_converted_before_nearest_lookup() {
    let near = r#"{"record_type":"map_point","id":"MAP_POINT_NEAR","map_id":"MAP_MAIN","native_id":"Near","kind":"fast_travel","names":{"en":"Ancient Civilization Ruins","zh_hans":"古代文明遗址"},"location":{"x":-391978.125,"y":-16978.125,"z":0.0},"local_evidence":{"source_table":"test","localization_status":"resolved","unresolved_fields":[],"transformation_notes":"test"},"provenance":{"source_id":"SRC-LOCAL-BUILD-MAP-24575825-20260906","applicable_game_version":"1.0.3","retrieved_on":"2026-09-06","reviewer":"Codex","review_status":"reviewed","confidence":"verified_target","change_risk":null}}"#;
    let snowfield = r#"{"record_type":"map_point","id":"MAP_POINT_SNOWFIELD","map_id":"MAP_MAIN","native_id":"Snowfield","kind":"fast_travel","names":{"en":"Pristine Snow Field","zh_hans":"纯白雪原"},"location":{"x":0.0,"y":0.0,"z":0.0},"local_evidence":{"source_table":"test","localization_status":"resolved","unresolved_fields":[],"transformation_notes":"test"},"provenance":{"source_id":"SRC-LOCAL-BUILD-MAP-24575825-20260906","applicable_game_version":"1.0.3","retrieved_on":"2026-09-06","reviewer":"Codex","review_status":"reviewed","confidence":"verified_target","change_risk":null}}"#;
    let store = test_store_with_extra_lines(&[near, snowfield]);
    let (agent, provider) = scripted_agent_with_store(
        store,
        None,
        vec![submit_ok(&["最近的传送点是古代文明遗址。"], &[])],
        4,
        1200,
    );

    let answer = agent.ask("坐标 -3919,-169 附近最近的传送点是什么？");

    assert_eq!(answer.status, AgentStatus::Ok);
    let message = &provider.calls()[0].messages[0].content;
    assert!(message.contains("Ancient Civilization Ruins"));
    assert!(message.contains("古代文明遗址"));
    assert!(
        message.contains("\"distance\":110.48543456039805"),
        "message={message}"
    );
    assert!(
        !message.contains("\"distance\":392256.5"),
        "message={message}"
    );
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
    assert!(calls[0].messages[0]
        .content
        .starts_with("What should I do next?\n\nGROUNDING"));
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
fn submit_answer_accepts_existing_slot_without_duplicate_declaration() {
    let (agent, _provider) = scripted_agent(
        None,
        vec![
            ChatResponse::tool(
                "call_1",
                "calculate_materials",
                json!({"query": "Wooden Club", "quantity": 3}),
            ),
            submit_ok(&["You need {q1} Wood."], &[]),
        ],
        4,
        1200,
    );
    let answer = agent.ask("Materials for 3 Wooden Clubs?");

    assert_eq!(answer.status, AgentStatus::Ok);
    assert_eq!(answer.answer.as_deref(), Some("You need 15 Wood."));
    assert!(!answer
        .uncertainty
        .iter()
        .any(|message| message.contains("unknown slot")));
}

#[test]
fn scalar_tool_results_become_quantity_slots() {
    let type_line = fs::read_to_string(format!("{DATA_DIRECTORY}/type_effectiveness.jsonl"))
        .expect("type effectiveness data reads")
        .lines()
        .next()
        .expect("type effectiveness record exists")
        .to_string();
    let store = test_store_with_extra_lines(&[&type_line]);
    let (agent, provider) = scripted_agent_with_store(
        store,
        None,
        vec![
            ChatResponse::tool(
                "call_1",
                "get_type_effectiveness",
                json!({"attacking_type": "water", "defending_type": "fire"}),
            ),
            submit_ok(&["Water is {q1} times effective against Fire."], &[]),
        ],
        4,
        1200,
    );
    let answer = agent.ask("水属性打火属性是几倍伤害？");
    assert_eq!(answer.status, AgentStatus::Ok);
    assert_eq!(
        answer.answer.as_deref(),
        Some("Water is 2 times effective against Fire.")
    );
    let fact_sheet = provider.calls()[1].messages[2].content.clone();
    assert!(fact_sheet.contains("entity=\"multiplier\""));
}

#[test]
fn grounded_lookup_tools_are_not_reoffered_to_the_model() {
    let (agent, provider) =
        scripted_agent(None, vec![ChatResponse::text("Wood is useful.")], 4, 1200);
    let answer = agent.ask("What is Wood?");

    assert_eq!(answer.status, AgentStatus::Ok);
    let calls = provider.calls();
    let tool_names = calls[0]
        .tools
        .iter()
        .map(|tool| tool.name.as_str())
        .collect::<Vec<_>>();
    assert!(!tool_names.contains(&"get_item"));
    assert!(!tool_names.contains(&"search_structured_knowledge"));
}

#[test]
fn pal_skill_questions_keep_skill_unlock_lookup_available() {
    let pal_line = r#"{"record_type":"pal","id":"PAL_ANUBIS","names":{"en":"Anubis","zh_hans":"阿努比斯"},"work_suitability":[],"drops":[],"habitat_ids":[],"element_type1":"earth","provenance":{"source_id":"SRC-LOCAL-BUILD-24575825-20260902","applicable_game_version":"1.0.3","retrieved_on":"2026-09-02","reviewer":"Codex","review_status":"reviewed","confidence":"verified_target"}}"#;
    let store = test_store_with_extra_lines(&[pal_line]);
    let (agent, provider) = scripted_agent_with_store(
        store,
        None,
        vec![ChatResponse::text("No skill data.")],
        4,
        1200,
    );
    let answer = agent.ask("阿努比斯会解锁哪些技能");

    assert_eq!(answer.status, AgentStatus::Ok);
    let calls = provider.calls();
    let tool_names = calls[0]
        .tools
        .iter()
        .map(|tool| tool.name.as_str())
        .collect::<Vec<_>>();
    assert!(tool_names.contains(&"get_pal_waza_unlocks"));
    assert!(!tool_names.contains(&"get_pal"));
}

#[test]
fn grounded_pal_lookup_suppresses_structured_search() {
    let (agent, provider) = scripted_agent(
        None,
        vec![ChatResponse::text("Lamball is useful.")],
        4,
        1200,
    );
    let answer = agent.ask("What is Lamball?");

    assert_eq!(answer.status, AgentStatus::Ok);
    let calls = provider.calls();
    let tool_names = calls[0]
        .tools
        .iter()
        .map(|tool| tool.name.as_str())
        .collect::<Vec<_>>();
    assert!(!tool_names.contains(&"search_structured_knowledge"));
}

#[test]
fn grounded_waza_lookup_suppresses_exact_tool_call() {
    let waza_line = fs::read_to_string(format!("{DATA_DIRECTORY}/waza.jsonl"))
        .expect("skill file reads")
        .lines()
        .find(|line| line.contains("\"id\":\"WAZA_AQUAJET\""))
        .expect("Hydro Jet exists")
        .to_string();
    let store = test_store_with_extra_lines(&[&waza_line]);
    let (agent, provider) = scripted_agent_with_store(
        store,
        None,
        vec![ChatResponse::text("水流射击是威力 40 的水属性射击技能。")],
        4,
        1200,
    );
    let answer = agent.ask("水流射击是什么技能");

    assert_eq!(answer.status, AgentStatus::Ok);
    assert!(answer.tool_calls.is_empty());
    let calls = provider.calls();
    assert!(calls[0].messages[0].content.contains("[get_waza]"));
    assert!(!calls[0].tools.iter().any(|tool| tool.name == "get_waza"));
}

#[test]
fn completed_pal_skill_unlocks_suppress_individual_skill_lookups() {
    let unlock_line = fs::read_to_string(format!("{DATA_DIRECTORY}/pal_waza_unlocks.jsonl"))
        .expect("skill unlock file reads")
        .lines()
        .find(|line| line.contains("\"pal_id\":\"PAL_LAMBALL\""))
        .expect("Lamball unlock exists")
        .to_string();
    let waza_line = fs::read_to_string(format!("{DATA_DIRECTORY}/waza.jsonl"))
        .expect("skill file reads")
        .lines()
        .find(|line| line.contains("\"id\":\"WAZA_UNIQUE_SHEEPBALL_ROLL\""))
        .expect("Lamball skill exists")
        .to_string();
    let store = test_store_with_extra_lines(&[&unlock_line, &waza_line]);
    let (agent, provider) = scripted_agent_with_store(
        store,
        None,
        vec![
            ChatResponse::tool("call_1", "get_pal_waza_unlocks", json!({"pal": "Lamball"})),
            submit_ok(&["The skill list is ready."], &[]),
        ],
        4,
        1200,
    );
    let answer = agent.ask("What skills does Lamball learn?");

    assert_eq!(answer.status, AgentStatus::Ok);
    let calls = provider.calls();
    let tool_names = calls[1]
        .tools
        .iter()
        .map(|tool| tool.name.as_str())
        .collect::<Vec<_>>();
    assert!(!tool_names.contains(&"get_waza"));
}

#[test]
fn waza_metadata_fallback_renders_metrics_not_material_needs() {
    let waza_line = fs::read_to_string(format!("{DATA_DIRECTORY}/waza.jsonl"))
        .expect("skill file reads")
        .lines()
        .find(|line| line.contains("\"id\":\"WAZA_AQUAJET\""))
        .expect("Hydro Jet exists")
        .to_string();
    let store = test_store_with_extra_lines(&[&waza_line]);
    let (agent, _provider) = scripted_agent_with_store(
        store,
        None,
        vec![
            ChatResponse::tool("call_1", "get_waza", json!({"query": "水流射击"})),
            ChatResponse::text(""),
        ],
        4,
        1200,
    );
    let answer = agent.ask("水流射击是什么技能");

    assert_eq!(answer.status, AgentStatus::Ok);
    let reply = answer.answer.as_deref().unwrap();
    assert!(reply.contains("power: 40"), "reply: {reply}");
    assert!(reply.contains("cool_time: 2"), "reply: {reply}");
    assert!(!reply.contains("Need "), "reply: {reply}");
}

#[test]
fn shortage_fallback_reports_only_missing_quantities() {
    let (agent, _provider) = scripted_agent(
        None,
        vec![
            ChatResponse::tool(
                "call_1",
                "calculate_shortage",
                json!({
                    "query": "Wood",
                    "quantity": 5,
                    "inventory": [{"item": "Wood", "quantity": 3}]
                }),
            ),
            ChatResponse::text(""),
        ],
        4,
        1200,
    );
    let answer = agent.ask("I have 3 Wood; what is missing for 5 Wood?");

    assert_eq!(answer.status, AgentStatus::Ok);
    assert!(answer
        .answer
        .as_deref()
        .is_some_and(|reply| reply.starts_with("还需要 2 个 Wood。")));
}

#[test]
fn grounded_natural_shortage_number_renders_directly() {
    let (agent, _provider) = scripted_agent(
        None,
        vec![
            ChatResponse::tool(
                "call_1",
                "calculate_shortage",
                json!({
                    "query": "Wood",
                    "quantity": 5,
                    "inventory": [{"item": "Wood", "quantity": 3}]
                }),
            ),
            submit_ok(&["还缺 2 个 Wood。"], &[]),
        ],
        4,
        1200,
    );
    let answer = agent.ask("I have 3 Wood; what is missing for 5 Wood?");

    assert_eq!(answer.status, AgentStatus::Ok);
    assert_eq!(answer.answer.as_deref(), Some("还缺 2 个 Wood。"));
}

#[test]
fn bracket_slot_syntax_does_not_leak_into_answers() {
    let (agent, _provider) = scripted_agent(
        None,
        vec![
            ChatResponse::tool("call_1", "get_item", json!({"query": "Wood"})),
            submit_ok(&["Wood is useful [q1]."], &[]),
            submit_ok(&["Wood is useful."], &[]),
        ],
        4,
        1200,
    );

    let answer = agent.ask("What is Wood?");

    assert_eq!(answer.status, AgentStatus::Ok);
    assert!(answer
        .answer
        .as_deref()
        .is_some_and(|reply| !reply.contains("[q")));
}

#[test]
fn shortage_question_exposes_only_shortage_calculator() {
    let (agent, provider) = scripted_agent(
        None,
        vec![
            ChatResponse::tool(
                "call_1",
                "calculate_shortage",
                json!({
                    "query": "Wood",
                    "quantity": 5,
                    "inventory": [{"item": "Wood", "quantity": 3}]
                }),
            ),
            ChatResponse::text(""),
        ],
        4,
        1200,
    );
    let answer = agent.ask("I have 3 Wood; what is missing for 5 Wood?");

    assert_eq!(answer.status, AgentStatus::Ok);
    let calls = provider.calls();
    let first_tools = calls[0]
        .tools
        .iter()
        .map(|tool| tool.name.as_str())
        .collect::<Vec<_>>();
    assert!(first_tools.contains(&"calculate_shortage"));
    assert!(!first_tools.contains(&"calculate_materials"));
    assert!(!first_tools.contains(&"calculate_craftable_count"));
    let second_tools = calls[1]
        .tools
        .iter()
        .map(|tool| tool.name.as_str())
        .collect::<Vec<_>>();
    assert!(!second_tools.contains(&"calculate_shortage"));
    assert!(!second_tools.contains(&"calculate_materials"));
    assert!(!second_tools.contains(&"calculate_craftable_count"));
}

#[test]
fn chinese_article_one_is_not_treated_as_a_quantity() {
    let (agent, _provider) = scripted_agent(
        None,
        vec![
            ChatResponse::tool("call_1", "get_item", json!({"query": "Wood"})),
            submit_ok(&["Wood 是一种常见材料。"], &[]),
        ],
        4,
        1200,
    );
    let answer = agent.ask("What is Wood?");

    assert_eq!(answer.status, AgentStatus::Ok);
    assert_eq!(answer.answer.as_deref(), Some("Wood 是一种常见材料。"));
}

#[test]
fn invalid_submit_answer_gets_one_correction_round() {
    let (agent, provider) = scripted_agent(
        None,
        vec![
            ChatResponse::tool(
                "call_1",
                "calculate_materials",
                json!({"query": "Wooden Club", "quantity": 3}),
            ),
            submit_ok(&["You need 12 Wood."], &[]),
            submit_ok(&["You need {q1} Wood."], &["q1"]),
        ],
        4,
        1200,
    );
    let answer = agent.ask("Materials for 3 Wooden Clubs?");

    assert_eq!(answer.status, AgentStatus::Ok);
    assert_eq!(answer.answer.as_deref(), Some("You need 15 Wood."));
    assert_eq!(provider.calls().len(), 3);
    let calls = provider.calls();
    let correction = calls[2]
        .messages
        .iter()
        .map(|message| message.content.as_str())
        .find(|content| content.contains("SUBMIT_ANSWER_ERROR"))
        .expect("correction message is sent");
    assert!(correction.contains("numeric literal"));
}

#[test]
fn invalid_provider_tool_json_gets_one_retry() {
    let provider = InvalidThenValidProvider {
        calls: std::sync::Mutex::new(Vec::new()),
    };
    let agent = GuideAgent::new(
        test_registry(None),
        Box::new(provider),
        agent_config(4, 1200),
    );
    let answer = agent.ask("What is Wood?");

    assert_eq!(answer.status, AgentStatus::Ok);
    assert!(answer.answer.is_some());
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
            submit_ok(&["You need {q1} Wood."], &["q1"]),
        ],
        4,
        1200,
    );
    let answer = agent.ask("Materials for 3 Wooden Clubs?");

    assert_eq!(answer.status, AgentStatus::Ok);
    assert_eq!(answer.answer.as_deref(), Some("You need 15 Wood."));
    assert!(!answer
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
            submit_ok(&["You need {q1} Wood."], &["q1"]),
        ],
        4,
        1200,
    );
    let answer = agent.ask("Materials for 3 Wooden Clubs?");

    assert_eq!(answer.status, AgentStatus::Ok);
    assert_eq!(answer.answer.as_deref(), Some("You need 15 Wood."));
    assert!(!answer
        .uncertainty
        .iter()
        .any(|message| message.contains("unknown slot q9")));
}

#[test]
fn number_word_in_draft_falls_back() {
    let (agent, _provider) = scripted_agent(
        None,
        vec![
            ChatResponse::tool(
                "submit_answer",
                "submit_answer",
                json!({
                    "status": "unknown",
                    "sentences": ["This is one of the most common options."]
                }),
            ),
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
    assert!(!answer
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
                    "sentences": ["需要一个 Wood。"]
                }),
            ),
            submit_ok(&["You need {q1} Wood."], &["q1"]),
        ],
        4,
        1200,
    );
    let answer = agent.ask("需要多少 Wood？");

    assert_eq!(answer.status, AgentStatus::Ok);
    assert_eq!(answer.answer.as_deref(), Some("You need 15 Wood."));
    assert!(!answer
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
fn grounded_pal_habitat_question_answers_without_exact_tool_call() {
    let pal_line = r#"{"record_type":"pal","id":"PAL_CHICKENPAL","names":{"en":"Chikipi","zh_hans":"皮皮鸡"},"work_suitability":[],"drops":[],"habitat_ids":[],"element_type1":"normal","provenance":{"source_id":"SRC-LOCAL-BUILD-24575825-20260902","applicable_game_version":"1.0.3","retrieved_on":"2026-09-02","reviewer":"Codex","review_status":"reviewed","confidence":"verified_target"}}"#;
    let alias_line = r#"{"record_type":"alias","id":"ALIAS_PAL_CHICKENPAL_ZH_HANS","alias":"皮皮鸡","target_id":"PAL_CHICKENPAL","locale":"zh_hans","provenance":{"source_id":"SRC-LOCAL-BUILD-24575825-20260902","applicable_game_version":"1.0.3","retrieved_on":"2026-09-02","reviewer":"Codex","review_status":"reviewed","confidence":"verified_target"}}"#;
    let store = test_store_with_extra_lines(&[pal_line, alias_line]);
    let (agent, _provider) = scripted_agent_with_store(
        store,
        None,
        vec![ChatResponse::text(
            "皮皮鸡的栖息地记录已匹配到多个已审核生成区域。",
        )],
        4,
        1200,
    );
    let answer = agent.ask("皮皮鸡住在哪");

    assert_eq!(answer.status, AgentStatus::Ok);
    assert!(answer.tool_calls.is_empty());
    let reply = answer.answer.as_deref().unwrap();
    assert!(reply.contains("皮皮鸡"));
    assert!(answer
        .provenance
        .iter()
        .any(|provenance| provenance.source_id.starts_with("SRC-LOCAL-BUILD")));
}

#[test]
fn conversation_history_stays_separate_from_current_question_grounding() {
    let (agent, provider) = scripted_agent(
        None,
        vec![submit_ok(&["皮皮鸡的栖息地信息暂时未知。"], &[])],
        4,
        1200,
    );

    let _answer = agent.ask_with_history(
        "皮皮鸡住在哪",
        &[AgentHistoryEntry::new(
            "地图坐标 4000,4000 附近最近的传送点是什么",
            Some("古代文明遗址是较近的传送点。".to_string()),
        )],
    );

    let request = &provider.calls()[0];
    assert_eq!(
        request
            .messages
            .iter()
            .map(|message| message.role.as_str())
            .collect::<Vec<_>>(),
        vec!["user", "assistant", "user"]
    );
    assert!(request.messages[0].content.contains("地图坐标 4000,4000"));
    assert_eq!(request.messages[1].content, "古代文明遗址是较近的传送点。");
    assert!(request.messages[2].content.starts_with("皮皮鸡住在哪"));
    assert!(!request.messages[2].content.contains("4000"));
    assert!(!request.messages[2].content.contains("传送点"));
}

#[test]
fn grounded_item_use_question_answers_without_exact_tool_call() {
    let (agent, provider) = scripted_agent(
        None,
        vec![ChatResponse::text("帕鲁矿碎块可用于制作帕鲁球。")],
        4,
        1200,
    );
    let answer = agent.ask("帕鲁矿碎块怎么用");

    assert_eq!(answer.status, AgentStatus::Ok);
    assert!(answer.tool_calls.is_empty());
    let calls = provider.calls();
    let tool_names = calls[0]
        .tools
        .iter()
        .map(|tool| tool.name.as_str())
        .collect::<Vec<_>>();
    assert!(!tool_names.contains(&"resolve_name"));
    assert!(!tool_names.contains(&"get_recipe"));
    assert!(!tool_names.contains(&"get_pal"));
    assert!(!tool_names.contains(&"get_technology"));
    assert!(!tool_names.contains(&"get_type_effectiveness"));
    assert!(!tool_names.contains(&"get_work_kind_descriptions"));
    assert!(!tool_names.contains(&"calculate_materials"));
    assert!(!tool_names.contains(&"get_conflicting_records"));
    let reply = answer.answer.as_deref().unwrap();
    assert!(reply.contains("帕鲁矿碎块"));
    assert!(reply.contains("帕鲁球"));
}

#[test]
fn recipe_question_grounds_localized_natural_answer() {
    let (agent, provider) = scripted_agent(
        None,
        vec![submit_ok(&["制作 1 个帕鲁球需要 1 个帕鲁矿碎块。"], &[])],
        4,
        1200,
    );
    let answer = agent.ask("帕鲁球怎么做");

    assert_eq!(answer.status, AgentStatus::Ok);
    assert_eq!(
        answer.answer.as_deref(),
        Some("制作 1 个帕鲁球需要 1 个帕鲁矿碎块。")
    );
    assert!(answer.tool_calls.is_empty());
    assert!(provider.calls()[0].messages[0]
        .content
        .contains("RECIPE_PAL_SPHERE"));
}

#[test]
fn empty_free_text_uses_fallback() {
    let (agent, _provider) = scripted_agent(None, vec![ChatResponse::text("   ")], 4, 1200);
    let answer = agent.ask("What is Wood?");

    assert_eq!(answer.status, AgentStatus::Ok);
    assert!(answer
        .answer
        .as_deref()
        .is_some_and(|text| !text.is_empty()));
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
                    "steps": ["Collect {q1} Wood.", "Craft the club."]
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
                    "sentences": ["Position: {o1}, {o2}, {o3}."]
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
            submit_ok(&["Position: {o1}, {o2}, {o3}."], &["o1", "o2", "o3"]),
        ],
        4,
        1200,
    );
    let answer = agent.ask("Where is my player?");

    assert_eq!(answer.status, AgentStatus::Ok);
    assert_eq!(
        answer.answer.as_deref(),
        Some("Position: x=-1.5, y=2.25, z=3.")
    );
    assert!(!answer
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
            submit_ok(&["Wood is the better choice."], &[]),
        ],
        4,
        1200,
    );
    let answer = agent.ask("What should I gather?");

    assert_eq!(answer.status, AgentStatus::Ok);
    assert_eq!(answer.answer.as_deref(), Some("Wood is the better choice."));
    assert!(!answer
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
fn direct_material_questions_skip_the_provider_and_render_in_chinese() {
    let club_alias = r#"{"record_type":"alias","id":"ALIAS_ITEM_WOODEN_CLUB_ZH_HANS","alias":"木棒","target_id":"ITEM_WOODEN_CLUB","locale":"zh_hans","provenance":{"source_id":"SRC-PALDB-V1_0_3-20260831","applicable_game_version":"1.0.3","retrieved_on":"2026-08-31","reviewer":"Codex","review_status":"reviewed","confidence":"reviewed_secondary"}}"#;
    let wood_alias = r#"{"record_type":"alias","id":"ALIAS_ITEM_WOOD_ZH_HANS","alias":"木材","target_id":"ITEM_WOOD","locale":"zh_hans","provenance":{"source_id":"SRC-PALDB-V1_0_3-20260831","applicable_game_version":"1.0.3","retrieved_on":"2026-08-31","reviewer":"Codex","review_status":"reviewed","confidence":"reviewed_secondary"}}"#;
    let store = test_store_with_extra_lines(&[club_alias, wood_alias]);
    let (agent, provider) = scripted_agent_with_store(store, None, Vec::new(), 4, 1200);

    let answer = agent.ask("制造3个木棒需要什么材料？");

    assert_eq!(answer.status, AgentStatus::Ok);
    assert_eq!(
        answer.answer.as_deref(),
        Some("制造 3 个木棒需要：木材 15 个。")
    );
    assert_eq!(
        provider.calls().len(),
        0,
        "material arithmetic must not wait for the provider"
    );
    assert_eq!(answer.tool_calls.len(), 1);
    assert_eq!(answer.tool_calls[0].name, "calculate_materials");
    assert_eq!(answer.tool_calls[0].arguments["quantity"], 3);
    assert!(answer.uncertainty.is_empty());

    let answer = agent.ask("制造一个木棒需要什么材料？");

    assert_eq!(answer.status, AgentStatus::Ok);
    assert_eq!(
        answer.answer.as_deref(),
        Some("制造 1 个木棒需要：木材 5 个。")
    );
    assert_eq!(provider.calls().len(), 0);
    assert_eq!(
        answer
            .tool_calls
            .last()
            .expect("calculation exists")
            .arguments["quantity"],
        1
    );
}

#[test]
fn direct_material_routing_requires_a_material_question() {
    let club_alias = r#"{"record_type":"alias","id":"ALIAS_ITEM_WOODEN_CLUB_ZH_HANS","alias":"木棒","target_id":"ITEM_WOODEN_CLUB","locale":"zh_hans","provenance":{"source_id":"SRC-PALDB-V1_0_3-20260831","applicable_game_version":"1.0.3","retrieved_on":"2026-08-31","reviewer":"Codex","review_status":"reviewed","confidence":"reviewed_secondary"}}"#;
    let store = test_store_with_extra_lines(&[club_alias]);
    let (agent, provider) = scripted_agent_with_store(
        store,
        None,
        vec![submit_ok(&["需要原始工作台。"], &[])],
        4,
        1200,
    );

    let _answer = agent.ask("制造3个木棒要什么工作台？");

    assert_eq!(
        provider.calls().len(),
        1,
        "non-material crafting questions must not take the direct material path"
    );
}

#[test]
fn direct_material_questions_accept_item_before_quantity() {
    let club_alias = r#"{"record_type":"alias","id":"ALIAS_ITEM_WOODEN_CLUB_ZH_HANS","alias":"木棒","target_id":"ITEM_WOODEN_CLUB","locale":"zh_hans","provenance":{"source_id":"SRC-PALDB-V1_0_3-20260831","applicable_game_version":"1.0.3","retrieved_on":"2026-08-31","reviewer":"Codex","review_status":"reviewed","confidence":"reviewed_secondary"}}"#;
    let wood_alias = r#"{"record_type":"alias","id":"ALIAS_ITEM_WOOD_ZH_HANS","alias":"木材","target_id":"ITEM_WOOD","locale":"zh_hans","provenance":{"source_id":"SRC-PALDB-V1_0_3-20260831","applicable_game_version":"1.0.3","retrieved_on":"2026-08-31","reviewer":"Codex","review_status":"reviewed","confidence":"reviewed_secondary"}}"#;
    let store = test_store_with_extra_lines(&[club_alias, wood_alias]);
    let (agent, provider) = scripted_agent_with_store(store, None, Vec::new(), 4, 1200);

    let answer = agent.ask("制造木棒三个需要什么材料？");

    assert_eq!(answer.status, AgentStatus::Ok);
    assert_eq!(
        answer.answer.as_deref(),
        Some("制造 3 个木棒需要：木材 15 个。")
    );
    assert_eq!(provider.calls().len(), 0);
}

#[test]
fn system_prompt_requires_the_player_language_for_model_answers() {
    let (agent, provider) =
        scripted_agent(None, vec![ChatResponse::text("Wood 是基础材料。")], 4, 1200);

    let answer = agent.ask("介绍一下木材");

    assert_eq!(answer.status, AgentStatus::Unknown);
    let request = &provider.calls()[0];
    assert!(
        request
            .system
            .contains("same language as the player's question"),
        "system prompt must preserve the player's language: {}",
        request.system
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
                    "sentences": ["That action is unavailable."]
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
            ChatResponse::text(""),
        ],
        1,
        1200,
    );
    let answer = agent.ask("Tell me everything.");

    assert_eq!(answer.status, AgentStatus::Ok);
    assert!(answer
        .answer
        .as_deref()
        .is_some_and(|text| !text.is_empty()));
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
                    "sentences": ["Reviewed knowledge is unavailable."]
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
