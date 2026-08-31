use game_knowledge::KnowledgeStore;
use guide_agent::{AgentConfig, AgentLimits, AgentStatus, GuideAgent};
use guide_core::GuideEngine;
use guide_tools::ToolRegistry;
use knowledge_index::KnowledgeIndex;
use provider::{ChatProvider, ChatRequest, ChatResponse, MockProvider, ProviderError};
use serde_json::json;
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
    let store = KnowledgeStore::load_directory(DATA_DIRECTORY).expect("dataset is valid");
    let configured = configured.map(str::to_string);
    let index = KnowledgeIndex::from_store(&store, configured.clone()).expect("index builds");
    let engine = GuideEngine::new(store, configured);
    ToolRegistry::new(engine, index)
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
        AgentConfig {
            limits: AgentLimits {
                max_tool_calls,
                timeout: Duration::from_secs(30),
            },
            max_reply_characters,
        },
    );
    (agent, provider)
}

#[test]
fn executes_tool_request_and_returns_grounded_answer() {
    let (agent, _provider) = scripted_agent(
        None,
        vec![
            ChatResponse::tool("call_1", "get_item", json!({"query": "Wood"})),
            ChatResponse::text("Wood is obtained by chopping trees."),
        ],
        4,
        1200,
    );
    let answer = agent.ask("How do I get Wood?");
    assert_eq!(answer.status, AgentStatus::Ok);
    assert!(answer
        .answer
        .as_deref()
        .unwrap_or_default()
        .contains("chopping trees"));
    assert_eq!(answer.tool_calls.len(), 1);
    assert_eq!(answer.tool_calls[0].name, "get_item");
    assert_eq!(answer.tool_calls[0].status, guide_tools::ToolStatus::Ok);
    assert!(answer
        .provenance
        .iter()
        .any(|provenance| provenance.source_id == "SRC-PALDB-V1_0_3-20260831"));
    assert_eq!(answer.version.knowledge_version, "1.0.3");
}

#[test]
fn model_cannot_bypass_tool_registry() {
    let (agent, _provider) = scripted_agent(
        None,
        vec![
            ChatResponse::tool("call_1", "delete_save", json!({})),
            ChatResponse::text("I could not delete anything."),
        ],
        4,
        1200,
    );
    let answer = agent.ask("Delete my save.");
    assert_eq!(answer.status, AgentStatus::Error);
    assert_eq!(answer.tool_calls.len(), 1);
    assert_eq!(answer.tool_calls[0].status, guide_tools::ToolStatus::Error);
    assert!(answer
        .errors
        .iter()
        .any(|error| error.contains("unknown tool")));
}

#[test]
fn budget_exhaustion_is_clear_and_non_fatal() {
    let (agent, _provider) = scripted_agent(
        None,
        vec![
            ChatResponse::tool("call_1", "get_item", json!({"query": "Wood"})),
            ChatResponse::tool("call_2", "get_item", json!({"query": "Wool"})),
            ChatResponse::tool("call_3", "get_item", json!({"query": "Lamball"})),
        ],
        1,
        1200,
    );
    let answer = agent.ask("Tell me everything.");
    assert_eq!(answer.status, AgentStatus::Error);
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
            ChatResponse::tool("call_1", "get_item", json!({"query": "Stone"})),
            ChatResponse::text("I do not have reviewed knowledge about Stone."),
        ],
        4,
        1200,
    );
    let answer = agent.ask("What is Stone?");
    assert_eq!(answer.status, AgentStatus::Unknown);
    assert_eq!(
        answer.tool_calls[0].status,
        guide_tools::ToolStatus::Unknown
    );
    assert!(answer
        .uncertainty
        .iter()
        .any(|message| message.contains("unknown item")));
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
            ChatResponse::text("You need 15 Wood for 3 Wooden Clubs."),
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
fn stale_version_propagates_to_answer() {
    let (agent, _provider) = scripted_agent(
        Some("0.9"),
        vec![
            ChatResponse::tool("call_1", "get_item", json!({"query": "Wood"})),
            ChatResponse::text("Wood is obtained by chopping trees, but the data may be stale."),
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
fn empty_final_answer_is_rejected() {
    let (agent, _provider) = scripted_agent(None, vec![ChatResponse::text("   ")], 4, 1200);
    let answer = agent.ask("What is Wood?");
    assert_eq!(answer.status, AgentStatus::Error);
    assert!(answer.errors.iter().any(|error| error.contains("empty")));
}

#[test]
fn answers_without_tool_evidence_are_flagged() {
    let (agent, _provider) = scripted_agent(
        None,
        vec![ChatResponse::text("Generic advice without facts.")],
        4,
        1200,
    );
    let answer = agent.ask("Any tips?");
    assert_eq!(answer.status, AgentStatus::Unknown);
    assert!(answer
        .uncertainty
        .iter()
        .any(|message| message.contains("no deterministic tool evidence")));
}

#[test]
fn numeric_tampering_is_rejected_by_grounding_gate() {
    let (agent, _provider) = scripted_agent(
        None,
        vec![
            ChatResponse::tool(
                "call_1",
                "calculate_materials",
                json!({"query": "Wooden Club", "quantity": 3}),
            ),
            ChatResponse::text("You need 12 Wood for 3 Wooden Clubs."),
        ],
        4,
        1200,
    );
    let answer = agent.ask("Materials for 3 Wooden Clubs?");
    assert_eq!(answer.status, AgentStatus::Error);
    assert!(answer.answer.is_none());
    assert!(answer
        .errors
        .iter()
        .any(|error| error.contains("unsupported numeric claim \"12\"")));
    assert_eq!(
        answer.tool_calls[0].data.as_ref().expect("tool data")["totals"][0]["required_quantity"],
        15
    );
}

#[test]
fn unsupported_calculation_claims_require_tool_evidence() {
    let (agent, _provider) = scripted_agent(
        None,
        vec![ChatResponse::text("You need 20 Wood for a club.")],
        4,
        1200,
    );
    let answer = agent.ask("How much Wood for a club?");
    assert_eq!(answer.status, AgentStatus::Error);
    assert!(answer.answer.is_none());
    assert!(answer
        .errors
        .iter()
        .any(|error| error.contains("unsupported numeric claim \"20\"")));
}

#[test]
fn version_digits_do_not_authorize_quantities() {
    let (agent, _provider) = scripted_agent(
        Some("1.0.3"),
        vec![
            ChatResponse::tool("call_1", "get_item", json!({"query": "Wood"})),
            ChatResponse::text("You need 3 Wood."),
        ],
        4,
        1200,
    );
    let answer = agent.ask("How much Wood do I need?");
    assert_eq!(answer.status, AgentStatus::Error);
    assert!(answer.answer.is_none());
    assert!(answer
        .errors
        .iter()
        .any(|error| error.contains("unsupported numeric claim \"3\"")
            || error.contains("unsupported calculation claim")));
}

#[test]
fn number_words_are_normalized_and_require_calculator_evidence() {
    let (agent, _provider) = scripted_agent(
        None,
        vec![ChatResponse::text("You need twenty Wood.")],
        4,
        1200,
    );
    let answer = agent.ask("How much Wood do I need?");
    assert_eq!(answer.status, AgentStatus::Error);
    assert!(answer.answer.is_none());
    assert!(answer.errors.iter().any(|error| error.contains("twenty")));
}

#[test]
fn non_ascii_numerals_are_normalized_and_grounded() {
    let (agent, _provider) =
        scripted_agent(None, vec![ChatResponse::text("You need ３ Wood.")], 4, 1200);
    let answer = agent.ask("How much Wood do I need?");
    assert_eq!(answer.status, AgentStatus::Error);
    assert!(answer.answer.is_none());
    assert!(answer
        .errors
        .iter()
        .any(|error| error.contains("unsupported numeric claim \"3\"")));
}

#[test]
fn scaled_number_words_require_exact_calculator_values() {
    let (agent, _provider) = scripted_agent(
        None,
        vec![
            ChatResponse::tool(
                "call_1",
                "calculate_materials",
                json!({"query": "Wooden Club", "quantity": 3}),
            ),
            ChatResponse::text("You need fifteen thousand Wood."),
        ],
        4,
        1200,
    );
    let answer = agent.ask("Materials for 3 Wooden Clubs?");
    assert_eq!(answer.status, AgentStatus::Error);
    assert!(answer.answer.is_none());
    assert!(answer
        .errors
        .iter()
        .any(|error| error.contains("unsupported numeric claim \"fifteen thousand\"")));
}

#[test]
fn complete_number_words_can_restate_exact_calculator_results() {
    let (agent, _provider) = scripted_agent(
        None,
        vec![
            ChatResponse::tool(
                "call_1",
                "calculate_materials",
                json!({"query": "Wooden Club", "quantity": 3}),
            ),
            ChatResponse::text("You need fifteen Wood for three Wooden Clubs."),
        ],
        4,
        1200,
    );
    let answer = agent.ask("Materials for 3 Wooden Clubs?");
    assert_eq!(answer.status, AgentStatus::Ok);
    assert!(answer.errors.is_empty());
}

#[test]
fn fractional_number_words_require_exact_calculator_values() {
    let (agent, _provider) = scripted_agent(
        None,
        vec![
            ChatResponse::tool(
                "call_1",
                "calculate_materials",
                json!({"query": "Wooden Club", "quantity": 3}),
            ),
            ChatResponse::text("You need one and a half Wood."),
        ],
        4,
        1200,
    );
    let answer = agent.ask("Materials for 3 Wooden Clubs?");
    assert_eq!(answer.status, AgentStatus::Error);
    assert!(answer.answer.is_none());
    assert!(answer
        .errors
        .iter()
        .any(|error| error.contains("unsupported numeric claim \"one and a half\"")));
}

#[test]
fn leading_decimal_fractions_require_exact_calculator_values() {
    let (agent, _provider) = scripted_agent(
        None,
        vec![
            ChatResponse::tool(
                "call_1",
                "calculate_materials",
                json!({"query": "Wooden Club", "quantity": 1}),
            ),
            ChatResponse::text("You need .5 Wood."),
        ],
        4,
        1200,
    );
    let answer = agent.ask("Materials for 1 Wooden Club?");
    assert_eq!(answer.status, AgentStatus::Error);
    assert!(answer.answer.is_none());
    assert!(answer
        .errors
        .iter()
        .any(|error| error.contains("unsupported numeric claim \".5\"")));
}

#[test]
fn same_value_evidence_cannot_authorize_another_entity_in_the_sentence() {
    let (agent, _provider) = scripted_agent(
        None,
        vec![
            ChatResponse::tool(
                "call_1",
                "calculate_shortage",
                json!({
                    "query": "Wooden Club",
                    "quantity": 3,
                    "inventory": [
                        {"item": "Wood", "quantity": 5},
                        {"item": "Wool", "quantity": 3}
                    ]
                }),
            ),
            ChatResponse::text("You are short 5 Wood and short 5 Wool for Wooden Clubs."),
        ],
        4,
        1200,
    );
    let answer = agent.ask("How short am I for 3 Wooden Clubs?");
    assert_eq!(answer.status, AgentStatus::Error);
    assert!(answer.answer.is_none());
    assert!(answer
        .errors
        .iter()
        .any(|error| error.contains("unsupported numeric claim \"5\"")));
}

#[test]
fn comma_separated_values_cannot_reuse_the_previous_entity() {
    let (agent, _provider) = scripted_agent(
        None,
        vec![
            ChatResponse::tool(
                "call_1",
                "calculate_shortage",
                json!({
                    "query": "Wooden Club",
                    "quantity": 3,
                    "inventory": [
                        {"item": "Wood", "quantity": 5},
                        {"item": "Wool", "quantity": 3}
                    ]
                }),
            ),
            ChatResponse::text("You need 5 Wood, 5 Wool for Wooden Clubs."),
        ],
        4,
        1200,
    );
    let answer = agent.ask("How short am I for 3 Wooden Clubs?");
    assert_eq!(answer.status, AgentStatus::Error);
    assert!(answer.answer.is_none());
    assert!(answer
        .errors
        .iter()
        .any(|error| error.contains("unsupported numeric claim \"5\"")));
}

#[test]
fn scientific_notation_is_one_exact_numeric_claim() {
    let (agent, _provider) = scripted_agent(
        None,
        vec![
            ChatResponse::tool(
                "call_1",
                "calculate_materials",
                json!({"query": "Wooden Club", "quantity": 15}),
            ),
            ChatResponse::text("You need 75e15 Wood for Wooden Clubs."),
        ],
        4,
        1200,
    );
    let answer = agent.ask("Materials for 15 Wooden Clubs?");
    assert_eq!(answer.status, AgentStatus::Error);
    assert!(answer.answer.is_none());
    assert!(answer
        .errors
        .iter()
        .any(|error| error.contains("unsupported numeric claim \"75e15\"")));
}

#[test]
fn chinese_number_words_require_matching_question_locale() {
    let (agent, _provider) = scripted_agent(
        None,
        vec![
            ChatResponse::tool(
                "call_1",
                "calculate_materials",
                json!({"query": "Wooden Club", "quantity": 3}),
            ),
            ChatResponse::text("You need 十五 Wood."),
        ],
        4,
        1200,
    );
    let answer = agent.ask("Materials for 3 Wooden Clubs?");
    assert_eq!(answer.status, AgentStatus::Error);
    assert!(answer.answer.is_none());
    assert!(answer
        .errors
        .iter()
        .any(|error| error.contains("unsupported numeric claim \"十五\"")));
}

#[test]
fn chinese_number_words_restate_results_for_chinese_questions() {
    let (agent, _provider) = scripted_agent(
        None,
        vec![
            ChatResponse::tool(
                "call_1",
                "calculate_materials",
                json!({"query": "Wooden Club", "quantity": 3}),
            ),
            ChatResponse::text("需要十五 Wood。"),
        ],
        4,
        1200,
    );
    let answer = agent.ask("3 个 Wooden Club 需要什么材料？");
    assert_eq!(answer.status, AgentStatus::Ok);
    assert!(answer.errors.is_empty());
}

#[test]
fn explicit_unknown_answers_cannot_append_breeding_guesses() {
    let (agent, _provider) = scripted_agent(
        None,
        vec![
            ChatResponse::tool("call_1", "get_item", json!({"query": "Wood"})),
            ChatResponse::text("Unknown result: the offspring is fluffy."),
        ],
        4,
        1200,
    );
    let answer = agent.ask("What does breeding Lamball and Lamball produce?");
    assert_eq!(answer.status, AgentStatus::Error);
    assert!(answer.answer.is_none());
    assert!(answer
        .errors
        .iter()
        .any(|error| error
            .contains("unsupported breeding claim after an explicit unknown statement")));
}

#[test]
fn unknown_breeding_answers_cannot_assert_a_permitted_offspring() {
    let (agent, _provider) = scripted_agent(
        None,
        vec![
            ChatResponse::tool(
                "call_1",
                "calculate_breeding_result",
                json!({"parent_a": "Lamball", "parent_b": "Lamball"}),
            ),
            ChatResponse::text("Unknown result: the offspring is Lamball."),
        ],
        4,
        1200,
    );
    let answer = agent.ask("What does Lamball plus Lamball produce?");
    assert_eq!(answer.status, AgentStatus::Error);
    assert!(answer.answer.is_none());
    assert!(answer
        .errors
        .iter()
        .any(|error| error
            .contains("unsupported breeding claim after an explicit unknown statement")));
}

#[test]
fn unknown_breeding_answers_cannot_assert_offspring_without_a_copula() {
    let (agent, _provider) = scripted_agent(
        None,
        vec![
            ChatResponse::tool(
                "call_1",
                "calculate_breeding_result",
                json!({"parent_a": "Lamball", "parent_b": "Lamball"}),
            ),
            ChatResponse::text("Unknown result: the offspring Lamball."),
        ],
        4,
        1200,
    );
    let answer = agent.ask("What does Lamball plus Lamball produce?");
    assert_eq!(answer.status, AgentStatus::Error);
    assert!(answer.answer.is_none());
    assert!(answer
        .errors
        .iter()
        .any(|error| error
            .contains("unsupported breeding claim after an explicit unknown statement")));
}

#[test]
fn unknown_breeding_answers_cannot_assert_a_bare_offspring_entity() {
    let (agent, _provider) = scripted_agent(
        None,
        vec![
            ChatResponse::tool(
                "call_1",
                "calculate_breeding_result",
                json!({"parent_a": "Lamball", "parent_b": "Lamball"}),
            ),
            ChatResponse::text("Unknown result: Lamball."),
        ],
        4,
        1200,
    );
    let answer = agent.ask("What does Lamball plus Lamball produce?");
    assert_eq!(answer.status, AgentStatus::Error);
    assert!(answer.answer.is_none());
    assert!(answer
        .errors
        .iter()
        .any(|error| error
            .contains("unsupported breeding claim after an explicit unknown statement")));
}

#[test]
fn generic_breeding_conclusions_require_breeding_evidence() {
    let (agent, _provider) = scripted_agent(
        None,
        vec![
            ChatResponse::tool("call_1", "get_item", json!({"query": "Wood"})),
            ChatResponse::text("Their offspring is fluffy."),
        ],
        4,
        1200,
    );
    let answer = agent.ask("What does breeding Lamball and Lamball produce?");
    assert_eq!(answer.status, AgentStatus::Error);
    assert!(answer.answer.is_none());
    assert!(answer.errors.iter().any(|error| error
        .contains("unsupported breeding claim; a successful breeding result is required")));
}

#[test]
fn breeding_questions_reject_context_free_generic_claims() {
    let (agent, _provider) = scripted_agent(
        None,
        vec![
            ChatResponse::tool("call_1", "get_item", json!({"query": "Wood"})),
            ChatResponse::text("It is fluffy."),
        ],
        4,
        1200,
    );
    let answer = agent.ask("What does breeding Lamball and Lamball produce?");
    assert_eq!(answer.status, AgentStatus::Error);
    assert!(answer.answer.is_none());
    assert!(answer.errors.iter().any(|error| error
        .contains("unsupported breeding claim; a successful breeding result is required")));
}

#[test]
fn breeding_evidence_is_bound_to_the_answered_parent_pair() {
    let (agent, _provider) = scripted_agent(
        None,
        vec![
            ChatResponse::tool(
                "call_1",
                "calculate_breeding_result",
                json!({"parent_a": "Lamball", "parent_b": "Lamball"}),
            ),
            ChatResponse::tool(
                "call_2",
                "calculate_breeding_result",
                json!({"parent_a": "Wool", "parent_b": "Wool"}),
            ),
            ChatResponse::text("Lamball plus Lamball produces Wool."),
        ],
        4,
        1200,
    );
    let answer = agent.ask("What does Lamball plus Lamball produce?");
    assert_eq!(answer.status, AgentStatus::Error);
    assert!(answer.answer.is_none());
    assert!(answer
        .errors
        .iter()
        .any(|error| error.contains("unsupported breeding claim \"Wool\"")));
}

#[test]
fn unrelated_lookup_cannot_wrap_unknown_breeding_result() {
    let (agent, _provider) = scripted_agent(
        None,
        vec![
            ChatResponse::tool(
                "call_1",
                "calculate_breeding_result",
                json!({"parent_a": "Lamball", "parent_b": "Lamball"}),
            ),
            ChatResponse::tool("call_2", "get_item", json!({"query": "Wool"})),
            ChatResponse::text("Lamball and Lamball produce Wool."),
        ],
        4,
        1200,
    );
    let answer = agent.ask("What does Lamball plus Lamball produce?");
    assert_eq!(answer.status, AgentStatus::Error);
    assert!(answer.answer.is_none());
    assert!(answer
        .errors
        .iter()
        .any(|error| error.contains("unsupported entity claim \"Wool\"")
            || error.contains("unsupported breeding claim \"Wool\"")));
}

#[test]
fn breeding_entity_evidence_ignores_unrelated_arguments() {
    let (agent, _provider) = scripted_agent(
        None,
        vec![
            ChatResponse::tool(
                "call_1",
                "calculate_breeding_result",
                json!({"parent_a": "Lamball", "parent_b": "Lamball"}),
            ),
            ChatResponse::tool("call_2", "get_item", json!({"query": "Wool"})),
            ChatResponse::text("Lamball plus Lamball gives Wool."),
        ],
        4,
        1200,
    );
    let answer = agent.ask("What does Lamball plus Lamball give?");
    assert_eq!(answer.status, AgentStatus::Error);
    assert!(answer.answer.is_none());
    assert!(answer
        .errors
        .iter()
        .any(|error| error.contains("unsupported entity claim \"Wool\"")));
}

#[test]
fn breeding_guesses_beyond_tool_evidence_are_rejected() {
    let (agent, _provider) = scripted_agent(
        None,
        vec![
            ChatResponse::tool(
                "call_1",
                "calculate_breeding_result",
                json!({"parent_a": "Lamball", "parent_b": "Lamball"}),
            ),
            ChatResponse::text("Lamball and Lamball produce Wool."),
        ],
        4,
        1200,
    );
    let answer = agent.ask("What does Lamball plus Lamball produce?");
    assert_eq!(answer.status, AgentStatus::Error);
    assert!(answer.answer.is_none());
    assert!(answer
        .errors
        .iter()
        .any(|error| error.contains("unsupported entity claim \"Wool\"")));
}

#[test]
fn unknown_breeding_answers_may_echo_only_question_entities() {
    let (agent, _provider) = scripted_agent(
        None,
        vec![
            ChatResponse::tool(
                "call_1",
                "calculate_breeding_result",
                json!({"parent_a": "Lamball", "parent_b": "Lamball"}),
            ),
            ChatResponse::text("No reviewed breeding result is available for Lamball and Lamball."),
        ],
        4,
        1200,
    );
    let answer = agent.ask("What does Lamball plus Lamball produce?");
    assert_eq!(answer.status, AgentStatus::Unknown);
    assert!(answer.answer.is_some());
    assert!(answer.errors.is_empty());
}

#[test]
fn reply_truncation_is_visible() {
    let (agent, _provider) = scripted_agent(
        None,
        vec![
            ChatResponse::tool("call_1", "get_item", json!({"query": "Wood"})),
            ChatResponse::text(
                "Wood is obtained by chopping trees and managing forest camps responsibly.",
            ),
        ],
        4,
        10,
    );
    let answer = agent.ask("Long answer?");
    assert_eq!(answer.answer.as_deref().map(str::len), Some(10));
    assert!(answer
        .uncertainty
        .iter()
        .any(|message| message.contains("truncated")));
}

#[test]
fn unknown_breeding_answers_cannot_assert_a_bare_entity_after_unknown() {
    let (agent, _provider) = scripted_agent(
        None,
        vec![
            ChatResponse::tool(
                "call_1",
                "calculate_breeding_result",
                json!({"parent_a": "Lamball", "parent_b": "Lamball"}),
            ),
            ChatResponse::text("Unknown: Lamball."),
        ],
        4,
        1200,
    );
    let answer = agent.ask("What does Lamball plus Lamball produce?");
    assert_eq!(answer.status, AgentStatus::Error);
    assert!(answer.answer.is_none());
    assert!(answer
        .errors
        .iter()
        .any(|error| error
            .contains("unsupported breeding claim after an explicit unknown statement")));
}

#[test]
fn unknown_breeding_answers_cannot_assert_offspring_before_the_keyword() {
    let (agent, _provider) = scripted_agent(
        None,
        vec![
            ChatResponse::tool(
                "call_1",
                "calculate_breeding_result",
                json!({"parent_a": "Lamball", "parent_b": "Lamball"}),
            ),
            ChatResponse::text("Unknown: Lamball is the offspring."),
        ],
        4,
        1200,
    );
    let answer = agent.ask("What does Lamball plus Lamball produce?");
    assert_eq!(answer.status, AgentStatus::Error);
    assert!(answer.answer.is_none());
    assert!(answer
        .errors
        .iter()
        .any(|error| error
            .contains("unsupported breeding claim after an explicit unknown statement")));
}

#[test]
fn reversed_clause_entities_cannot_reuse_shared_values() {
    let (agent, _provider) = scripted_agent(
        None,
        vec![
            ChatResponse::tool(
                "call_1",
                "calculate_shortage",
                json!({
                    "query": "Wooden Club",
                    "quantity": 3,
                    "inventory": [
                        {"item": "Wood", "quantity": 5},
                        {"item": "Wool", "quantity": 3}
                    ]
                }),
            ),
            ChatResponse::text("You need 5 Wool and 5 Wood for Wooden Clubs."),
        ],
        4,
        1200,
    );
    let answer = agent.ask("How short am I for 3 Wooden Clubs?");
    assert_eq!(answer.status, AgentStatus::Error);
    assert!(answer.answer.is_none());
    assert!(answer
        .errors
        .iter()
        .any(|error| error.contains("unsupported numeric claim \"5\"")));
}

#[test]
fn chinese_unknown_answers_cannot_assert_a_bare_entity() {
    let (agent, _provider) = scripted_agent(
        None,
        vec![
            ChatResponse::tool(
                "call_1",
                "calculate_breeding_result",
                json!({"parent_a": "Lamball", "parent_b": "Lamball"}),
            ),
            ChatResponse::text("未知：Lamball。"),
        ],
        4,
        1200,
    );
    let answer = agent.ask("Lamball 和 Lamball 繁殖会得到什么？");
    assert_eq!(answer.status, AgentStatus::Error);
    assert!(answer.answer.is_none());
}

#[test]
fn unknown_answers_accept_context_only_parent_entities() {
    let (agent, _provider) = scripted_agent(
        None,
        vec![
            ChatResponse::tool(
                "call_1",
                "calculate_breeding_result",
                json!({"parent_a": "Lamball", "parent_b": "Lamball"}),
            ),
            ChatResponse::text("No reviewed result for Lamball and Lamball."),
        ],
        4,
        1200,
    );
    let answer = agent.ask("What does Lamball plus Lamball produce?");
    assert_eq!(answer.status, AgentStatus::Unknown);
    assert!(answer.answer.is_some());
    assert!(answer.errors.is_empty());
}

#[test]
fn unknown_answers_accept_subject_position_parent_entities() {
    let (agent, _provider) = scripted_agent(
        None,
        vec![
            ChatResponse::tool(
                "call_1",
                "calculate_breeding_result",
                json!({"parent_a": "Lamball", "parent_b": "Lamball"}),
            ),
            ChatResponse::text("Breeding Lamball and Lamball is unknown."),
        ],
        4,
        1200,
    );
    let answer = agent.ask("What does Lamball plus Lamball produce?");
    assert_eq!(answer.status, AgentStatus::Unknown);
    assert!(answer.answer.is_some());
    assert!(answer.errors.is_empty());
}
