//! Bounded in-game chat bridge tests over a real loopback gateway.
//!
//! A fake adapter client owns the adapter side of the schema-2 connection
//! (hello, manifest, event, and strictly increasing result sequences) and
//! answers every `send_chat_message` tool call so the bridge can deliver
//! replies without blocking.

use game_gateway::ChatEvent;
use game_knowledge::KnowledgeStore;
use guide_adapter::{ChatOutcome, GameAdapterRuntime, InGameChatBridge, InGameLimits};
use guide_agent::{AgentConfig, AgentLimits, AgentStatus, GuideAgent};
use guide_core::GuideEngine;
use guide_tools::ToolRegistry;
use knowledge_index::KnowledgeIndex;
use provider::{ChatProvider, ChatRequest, ChatResponse, MockProvider, ProviderError};
use serde_json::json;
use std::fs;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

mod common;
use common::{
    connect_authenticated, event_frame, gateway, hello, manifest, read_json, result, send_json,
};

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

fn test_registry() -> ToolRegistry {
    let store = KnowledgeStore::load_directory(DATA_DIRECTORY).expect("dataset is valid");
    let index = KnowledgeIndex::from_store(&store, None).expect("index builds");
    let engine = GuideEngine::new(store, None);
    ToolRegistry::new(engine, index)
}

fn scripted_agent(responses: Vec<ChatResponse>) -> (Arc<GuideAgent>, Arc<MockProvider>) {
    let provider = Arc::new(MockProvider::scripted(responses));
    let agent = GuideAgent::new(
        test_registry(),
        Box::new(ProviderHandle(provider.clone())),
        AgentConfig {
            limits: AgentLimits {
                max_tool_calls: 4,
                timeout: Duration::from_secs(30),
            },
            max_reply_characters: 1200,
        },
    );
    (Arc::new(agent), provider)
}

fn chat_event(event_id: &str, text: &str) -> ChatEvent {
    ChatEvent {
        event_id: event_id.to_string(),
        sequence: 0,
        timestamp_ms: 0,
        source: "client".to_string(),
        player_id: "player".to_string(),
        text: text.to_string(),
    }
}

fn bridge_limits() -> InGameLimits {
    InGameLimits {
        max_history_exchanges: 4,
        max_asks_per_minute: 6,
        max_question_characters: 1000,
        max_reply_characters: 400,
        poll_interval: Duration::from_millis(75),
    }
}

fn debug_log_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "guide-adapter-chat-debug-{}-{name}.jsonl",
        std::process::id()
    ))
}

fn process_in_worker(
    bridge: &Arc<Mutex<InGameChatBridge>>,
    agent: Arc<GuideAgent>,
    event: ChatEvent,
) -> std::thread::JoinHandle<ChatOutcome> {
    let bridge = Arc::clone(bridge);
    std::thread::spawn(move || bridge.lock().unwrap().process_event(&agent, event))
}

struct FakeAdapter {
    socket: common::ClientSocket,
    sequence: u64,
}

impl FakeAdapter {
    fn connect(address: SocketAddr, tools: &[&str]) -> Self {
        let mut socket = connect_authenticated(address);
        send_json(&mut socket, hello(1));
        send_json(&mut socket, manifest(2, tools));
        Self {
            socket,
            sequence: 2,
        }
    }

    fn send_event(&mut self, text: &str) {
        self.sequence += 1;
        send_json(&mut self.socket, event_frame(self.sequence, text));
    }

    fn respond_ok(&mut self) -> String {
        let call = read_json(&mut self.socket);
        assert_eq!(call["type"], "tool_call");
        assert_eq!(call["tool"], "send_chat_message");
        let message = call["args"]["message"].as_str().unwrap().to_string();
        self.sequence += 1;
        send_json(
            &mut self.socket,
            result(self.sequence, call["call_id"].as_str().unwrap(), "ok"),
        );
        message
    }

    fn expect_no_delivery(&mut self) {
        match common::read_json_timeout(&mut self.socket, Duration::from_millis(300)) {
            Ok(frame) => panic!("expected no delivery but received: {frame}"),
            Err(error) => assert_eq!(
                error, "read timed out",
                "expected an idle socket, got {error}"
            ),
        }
    }
}

#[test]
fn ping_bypasses_the_provider_and_sends_one_reply() {
    let runtime = GameAdapterRuntime::new(gateway());
    let address = runtime.endpoint();
    let mut adapter = FakeAdapter::connect(address, &["send_chat_message"]);
    adapter.send_event("!guide Ping");
    let chat_event = runtime
        .next_event_timeout(Duration::from_millis(500))
        .expect("adapter read must not error")
        .expect("ping event");

    let (agent, provider) = scripted_agent(vec![ChatResponse::text("unexpected provider call")]);
    let bridge = Arc::new(Mutex::new(InGameChatBridge::new(runtime, bridge_limits())));
    let worker = process_in_worker(&bridge, agent, chat_event);
    let delivered = adapter.respond_ok();
    let outcome = worker.join().unwrap();

    assert_eq!(delivered, "Pong: Palworld Guider adapter connected.");
    assert!(outcome.delivered);
    assert!(outcome.delivery_error.is_none());
    assert_eq!(
        outcome.answer.answer.as_deref(),
        Some("Pong: Palworld Guider adapter connected.")
    );
    assert_eq!(outcome.answer.status, AgentStatus::Ok);
    assert!(
        provider.calls().is_empty(),
        "ping must never call the chat provider"
    );
}

#[test]
fn chat_event_runs_agent_and_sends_final_reply_once() {
    let runtime = GameAdapterRuntime::new(gateway());
    let address = runtime.endpoint();
    let mut adapter = FakeAdapter::connect(address, &["send_chat_message"]);
    let (agent, provider) = scripted_agent(vec![
        ChatResponse::tool("call_1", "get_item", json!({"query": "Stone"})),
        ChatResponse::text(
            "Stone is obtained by mining rocks and managing quarry output responsibly.",
        ),
    ]);
    let bridge = Arc::new(Mutex::new(InGameChatBridge::new(runtime, bridge_limits())));

    let worker = process_in_worker(
        &bridge,
        agent,
        chat_event("event-1", "!guide How do I get Stone?"),
    );
    let delivered = adapter.respond_ok();
    let outcome = worker.join().unwrap();

    assert!(outcome.delivered);
    assert!(outcome.delivery_error.is_none());
    assert_eq!(
        delivered,
        "Stone is obtained by mining rocks and managing quarry output responsibly."
    );
    assert_eq!(outcome.answer.status, AgentStatus::Ok);
    assert_eq!(
        outcome.answer.answer.as_deref(),
        Some("Stone is obtained by mining rocks and managing quarry output responsibly.")
    );
    assert_eq!(provider.calls().len(), 2, "agent runs exactly once");
    assert!(
        provider.calls()[0].messages[0]
            .content
            .starts_with("Q: How do I get Stone?"),
        "the current question must lead the grounded prompt: {}",
        provider.calls()[0].messages[0].content
    );
}

#[test]
fn chat_debug_log_records_ask_reply_and_tool_calls() {
    let runtime = GameAdapterRuntime::new(gateway());
    let address = runtime.endpoint();
    let mut adapter = FakeAdapter::connect(address, &["send_chat_message"]);
    let (agent, _provider) = scripted_agent(vec![
        ChatResponse::tool("call_1", "get_item", json!({"query": "Stone"})),
        ChatResponse::text("Stone is obtained by mining rocks."),
    ]);
    let debug_log = debug_log_path("ask");
    let bridge = Arc::new(Mutex::new(
        InGameChatBridge::new(runtime, bridge_limits()).with_debug_log(&debug_log),
    ));

    let worker = process_in_worker(
        &bridge,
        agent,
        chat_event("event-1", "!guide How do I get Stone?"),
    );
    let delivered = adapter.respond_ok();
    let outcome = worker.join().unwrap();

    assert!(outcome.delivered);
    let contents = fs::read_to_string(&debug_log).expect("debug log is written");
    let lines: Vec<_> = contents.lines().collect();
    assert_eq!(lines.len(), 1);
    let entry: serde_json::Value = serde_json::from_str(lines[0]).expect("debug entry is JSON");
    assert_eq!(entry["kind"], "ask");
    assert_eq!(entry["event_id"], "event-1");
    assert_eq!(entry["question"], "How do I get Stone?");
    assert_eq!(entry["reply"], delivered);
    assert_eq!(entry["reply"], "Stone is obtained by mining rocks.");
    assert_eq!(entry["delivered"], true);
    assert_eq!(entry["status"], "ok");
    assert_eq!(entry["tool_calls"][0]["name"], "get_item");
    assert_eq!(entry["tool_calls"][0]["status"], "ok");
    fs::remove_file(&debug_log).ok();
}

#[test]
fn chat_debug_log_records_skipped_events() {
    let runtime = GameAdapterRuntime::new(gateway());
    let address = runtime.endpoint();
    let mut adapter = FakeAdapter::connect(address, &["send_chat_message"]);
    let (agent, provider) = scripted_agent(Vec::new());
    let debug_log = debug_log_path("skip");
    let bridge = Arc::new(Mutex::new(
        InGameChatBridge::new(runtime, bridge_limits()).with_debug_log(&debug_log),
    ));

    let worker = process_in_worker(&bridge, agent, chat_event("event-empty", "!guide   "));
    let outcome = worker.join().unwrap();
    adapter.expect_no_delivery();

    assert!(!outcome.delivered);
    let contents = fs::read_to_string(&debug_log).expect("debug log is written");
    let entry: serde_json::Value =
        serde_json::from_str(contents.lines().next().unwrap()).expect("debug entry is JSON");
    assert_eq!(entry["kind"], "skip");
    assert_eq!(entry["event_id"], "event-empty");
    assert_eq!(entry["delivered"], false);
    assert_eq!(entry["status"], "error");
    assert!(entry["errors"]
        .as_array()
        .expect("errors array")
        .iter()
        .any(|error| error.as_str().unwrap_or_default().contains("no question")));
    assert!(provider.calls().is_empty());
    fs::remove_file(&debug_log).ok();
}

#[test]
fn provider_failure_sends_a_clear_non_fabricated_reply() {
    let runtime = GameAdapterRuntime::new(gateway());
    let address = runtime.endpoint();
    let mut adapter = FakeAdapter::connect(address, &["send_chat_message"]);
    // An exhausted mock script is an invalid provider response; the agent
    // retries once and then fails without fabricating an answer.
    let (agent, provider) = scripted_agent(Vec::new());
    let bridge = Arc::new(Mutex::new(InGameChatBridge::new(runtime, bridge_limits())));

    let worker = process_in_worker(
        &bridge,
        agent,
        chat_event("event-1", "!guide What is a Lamball?"),
    );
    let delivered = adapter.respond_ok();
    let outcome = worker.join().unwrap();

    assert!(
        delivered.starts_with("Guide unavailable:"),
        "provider failures must not fabricate an answer: {delivered}"
    );
    assert!(outcome.delivered);
    assert!(outcome.delivery_error.is_none());
    assert_eq!(outcome.answer.status, AgentStatus::Error);
    assert!(outcome.answer.answer.is_none());
    assert!(
        outcome
            .answer
            .errors
            .iter()
            .any(|error| error.contains("provider")),
        "the original error envelope must survive: {:?}",
        outcome.answer.errors
    );
    assert_eq!(
        provider.calls().len(),
        2,
        "invalid provider responses receive exactly one correction retry"
    );
}

#[test]
fn follow_up_history_keeps_only_the_latest_exchange_for_follow_ups() {
    let runtime = GameAdapterRuntime::new(gateway());
    let address = runtime.endpoint();
    let mut adapter = FakeAdapter::connect(address, &["send_chat_message"]);
    let answer = "Chopping trees and managing forest camps are the recommended approach.";
    let (agent, provider) = scripted_agent(vec![ChatResponse::text(answer); 6]);
    let bridge = Arc::new(Mutex::new(InGameChatBridge::new(runtime, bridge_limits())));

    for index in 1..=6 {
        let worker = process_in_worker(
            &bridge,
            Arc::clone(&agent),
            chat_event(
                &format!("event-{index}"),
                &format!("!guide 继续 question {index}"),
            ),
        );
        let delivered = adapter.respond_ok();
        let outcome = worker.join().unwrap();
        assert!(
            outcome.delivered,
            "delivery {index} failed: {:?}",
            outcome.delivery_error
        );
        assert_eq!(delivered, answer);
    }

    let calls = provider.calls();
    assert_eq!(calls.len(), 6);
    let last_prompt = &calls[5].messages[0].content;
    assert!(last_prompt.contains("Q: 继续 question 5"));
    assert!(last_prompt.contains("Q: 继续 question 6"));
    assert!(
        !last_prompt.contains("question 1") && !last_prompt.contains("question 4"),
        "follow-up prompts must keep only the newest exchange: {last_prompt}"
    );
    assert!(
        !last_prompt.contains('{'),
        "prompts must stay text-only: {last_prompt}"
    );
    assert!(
        !last_prompt.contains("TOOL_RESULT"),
        "prompts must stay text-only: {last_prompt}"
    );
    assert!(
        !last_prompt.contains('\\'),
        "prompts must stay text-only: {last_prompt}"
    );
    assert!(
        !last_prompt.contains("C:"),
        "prompts must stay text-only: {last_prompt}"
    );
}

#[test]
fn unrelated_questions_start_with_clean_context() {
    let runtime = GameAdapterRuntime::new(gateway());
    let address = runtime.endpoint();
    let mut adapter = FakeAdapter::connect(address, &["send_chat_message"]);
    let (agent, provider) = scripted_agent(vec![
        ChatResponse::text("first answer about breeding"),
        ChatResponse::text("second answer about fast travel"),
    ]);
    let bridge = Arc::new(Mutex::new(InGameChatBridge::new(runtime, bridge_limits())));

    let worker = process_in_worker(
        &bridge,
        Arc::clone(&agent),
        chat_event("event-1", "!guide how does Lamball breeding work?"),
    );
    let delivered = adapter.respond_ok();
    let outcome = worker.join().unwrap();
    assert!(outcome.delivered);
    assert_eq!(delivered, "first answer about breeding");

    let worker = process_in_worker(
        &bridge,
        Arc::clone(&agent),
        chat_event("event-2", "!guide where is the nearest fast travel point?"),
    );
    let delivered = adapter.respond_ok();
    let outcome = worker.join().unwrap();
    assert!(outcome.delivered);
    assert_eq!(delivered, "second answer about fast travel");

    let calls = provider.calls();
    assert_eq!(calls.len(), 2);
    let second_prompt = &calls[1].messages[0].content;
    assert!(
        !second_prompt.contains("Lamball") && !second_prompt.contains("first answer"),
        "unrelated questions must not inherit earlier exchanges: {second_prompt}"
    );
    assert!(
        second_prompt.starts_with("Q: where is the nearest fast travel point?"),
        "the current question must start the clean prompt: {second_prompt}"
    );
}

#[test]
fn retry_reruns_the_last_question_with_clean_context() {
    let runtime = GameAdapterRuntime::new(gateway());
    let address = runtime.endpoint();
    let mut adapter = FakeAdapter::connect(address, &["send_chat_message"]);
    let (agent, provider) = scripted_agent(vec![
        ChatResponse::text("contaminated first answer"),
        ChatResponse::text("clean retried answer"),
        ChatResponse::text("follow-up answer"),
    ]);
    let bridge = Arc::new(Mutex::new(InGameChatBridge::new(runtime, bridge_limits())));

    let worker = process_in_worker(
        &bridge,
        Arc::clone(&agent),
        chat_event("event-1", "!guide 继续 what is the best early base Pal?"),
    );
    let delivered = adapter.respond_ok();
    let outcome = worker.join().unwrap();
    assert!(outcome.delivered);
    assert_eq!(delivered, "contaminated first answer");

    let worker = process_in_worker(
        &bridge,
        Arc::clone(&agent),
        chat_event("event-2", "!guide retry"),
    );
    let delivered = adapter.respond_ok();
    let outcome = worker.join().unwrap();
    assert!(outcome.delivered);
    assert_eq!(delivered, "clean retried answer");

    let calls = provider.calls();
    assert_eq!(calls.len(), 2);
    let retry_prompt = &calls[1].messages[0].content;
    assert!(
        !retry_prompt.contains("contaminated first answer"),
        "retry must clear history before rerunning: {retry_prompt}"
    );
    assert!(
        retry_prompt.starts_with("Q: 继续 what is the best early base Pal?"),
        "retry must resend the exact last question: {retry_prompt}"
    );

    let worker = process_in_worker(
        &bridge,
        Arc::clone(&agent),
        chat_event("event-3", "!guide 继续 why is that Pal good?"),
    );
    let delivered = adapter.respond_ok();
    let outcome = worker.join().unwrap();
    assert!(outcome.delivered);
    assert_eq!(delivered, "follow-up answer");
    let calls = provider.calls();
    let follow_up_prompt = &calls[2].messages[0].content;
    assert!(
        follow_up_prompt.contains("clean retried answer"),
        "post-retry follow-ups must use the retried exchange: {follow_up_prompt}"
    );
}

#[test]
fn retry_without_a_previous_question_fails_closed() {
    let runtime = GameAdapterRuntime::new(gateway());
    let address = runtime.endpoint();
    let mut adapter = FakeAdapter::connect(address, &["send_chat_message"]);
    let (agent, provider) = scripted_agent(Vec::new());
    let bridge = Arc::new(Mutex::new(InGameChatBridge::new(runtime, bridge_limits())));

    let worker = process_in_worker(&bridge, agent, chat_event("event-1", "!guide retry"));
    let outcome = worker.join().unwrap();
    adapter.expect_no_delivery();

    assert!(!outcome.delivered);
    assert_eq!(outcome.answer.status, AgentStatus::Error);
    assert!(
        outcome
            .answer
            .errors
            .iter()
            .any(|error| error.contains("no previous question")),
        "retry without history must explain itself: {:?}",
        outcome.answer.errors
    );
    assert!(
        provider.calls().is_empty(),
        "retry without a previous question must not run the provider"
    );
}

#[test]
fn new_clears_history_without_a_provider_run() {
    let runtime = GameAdapterRuntime::new(gateway());
    let address = runtime.endpoint();
    let mut adapter = FakeAdapter::connect(address, &["send_chat_message"]);
    let (agent, provider) = scripted_agent(vec![
        ChatResponse::text("old answer"),
        ChatResponse::text("fresh answer"),
    ]);
    let bridge = Arc::new(Mutex::new(InGameChatBridge::new(runtime, bridge_limits())));

    let worker = process_in_worker(
        &bridge,
        Arc::clone(&agent),
        chat_event("event-1", "!guide what is the best weapon?"),
    );
    let delivered = adapter.respond_ok();
    let outcome = worker.join().unwrap();
    assert!(outcome.delivered);
    assert_eq!(delivered, "old answer");

    let worker = process_in_worker(
        &bridge,
        Arc::clone(&agent),
        chat_event("event-2", "!guide new"),
    );
    let delivered = adapter.respond_ok();
    let outcome = worker.join().unwrap();
    assert!(outcome.delivered);
    assert_eq!(delivered, "Guide session cleared.");
    assert_eq!(outcome.answer.status, AgentStatus::Ok);
    assert_eq!(provider.calls().len(), 1, "new must not call the provider");

    let worker = process_in_worker(
        &bridge,
        Arc::clone(&agent),
        chat_event("event-3", "!guide 继续 what should I craft first?"),
    );
    let delivered = adapter.respond_ok();
    let outcome = worker.join().unwrap();
    assert!(outcome.delivered);
    assert_eq!(delivered, "fresh answer");
    let calls = provider.calls();
    assert_eq!(calls.len(), 2);
    let post_new_prompt = &calls[1].messages[0].content;
    assert!(
        !post_new_prompt.contains("old answer") && !post_new_prompt.contains("best weapon"),
        "new must clear all follow-up context: {post_new_prompt}"
    );
}

#[test]
fn oversized_questions_and_chat_rate_limits_fail_closed() {
    let runtime = GameAdapterRuntime::new(gateway());
    let address = runtime.endpoint();
    let mut adapter = FakeAdapter::connect(address, &["send_chat_message"]);
    let answer = "Chopping trees and managing forest camps are the recommended approach.";
    let (agent, provider) = scripted_agent(vec![ChatResponse::text(answer); 6]);
    let bridge = Arc::new(Mutex::new(InGameChatBridge::new(runtime, bridge_limits())));

    let oversized = format!("!guide {}", "x".repeat(2000));
    let worker = process_in_worker(
        &bridge,
        Arc::clone(&agent),
        chat_event("event-oversized", &oversized),
    );
    let delivered = adapter.respond_ok();
    let outcome = worker.join().unwrap();
    assert!(outcome.delivered);
    assert_eq!(delivered, answer);
    let first_prompt = &provider.calls()[0].messages[0].content;
    let clamped_question = format!("Q: {}", "x".repeat(1000));
    assert!(
        first_prompt.starts_with(&clamped_question),
        "oversized questions must be clamped before grounding"
    );
    assert!(
        !first_prompt.contains(&"x".repeat(1001)),
        "excess question characters must not leak into the prompt"
    );

    for index in 2..=6 {
        let worker = process_in_worker(
            &bridge,
            Arc::clone(&agent),
            chat_event(
                &format!("event-{index}"),
                &format!("!guide question {index}"),
            ),
        );
        let delivered = adapter.respond_ok();
        let outcome = worker.join().unwrap();
        assert!(
            outcome.delivered,
            "delivery {index} failed: {:?}",
            outcome.delivery_error
        );
        assert_eq!(delivered, answer);
    }
    assert_eq!(provider.calls().len(), 6);

    // The seventh ask inside the rolling one-minute window fails closed:
    // no provider run and no delivery.
    let worker = process_in_worker(
        &bridge,
        Arc::clone(&agent),
        chat_event("event-7", "!guide question 7"),
    );
    let outcome = worker.join().unwrap();
    adapter.expect_no_delivery();

    assert!(!outcome.delivered);
    assert!(outcome.delivery_error.is_none());
    assert_eq!(outcome.answer.status, AgentStatus::Error);
    assert!(
        outcome
            .answer
            .errors
            .iter()
            .any(|error| error.contains("rate limit")),
        "rate-limited outcomes must explain themselves: {:?}",
        outcome.answer.errors
    );
    assert_eq!(
        provider.calls().len(),
        6,
        "rate-limited events must not run the provider"
    );
}

#[test]
fn replies_are_clamped_before_delivery() {
    let runtime = GameAdapterRuntime::new(gateway());
    let address = runtime.endpoint();
    let mut adapter = FakeAdapter::connect(address, &["send_chat_message"]);
    let long_answer = "y".repeat(500);
    let (agent, _provider) = scripted_agent(vec![ChatResponse::text(long_answer.clone())]);
    let bridge = Arc::new(Mutex::new(InGameChatBridge::new(runtime, bridge_limits())));

    let worker = process_in_worker(&bridge, agent, chat_event("event-1", "!guide question"));
    let delivered = adapter.respond_ok();
    let outcome = worker.join().unwrap();

    assert!(outcome.delivered);
    assert_eq!(delivered.chars().count(), 400);
    assert_eq!(delivered, "y".repeat(400));
    assert_eq!(
        outcome
            .answer
            .answer
            .as_deref()
            .map(|text| text.chars().count()),
        Some(500),
        "the original agent envelope must be preserved"
    );
}

#[test]
fn empty_chat_events_are_ignored_without_a_provider_run() {
    let runtime = GameAdapterRuntime::new(gateway());
    let address = runtime.endpoint();
    let mut adapter = FakeAdapter::connect(address, &["send_chat_message"]);
    let (agent, provider) = scripted_agent(Vec::new());
    let bridge = Arc::new(Mutex::new(InGameChatBridge::new(runtime, bridge_limits())));

    let worker = process_in_worker(&bridge, agent, chat_event("event-empty", "!guide    "));
    let outcome = worker.join().unwrap();
    adapter.expect_no_delivery();

    assert!(!outcome.delivered);
    assert!(outcome.delivery_error.is_none());
    assert_eq!(outcome.event_id, "event-empty");
    assert_eq!(outcome.answer.status, AgentStatus::Error);
    assert!(
        outcome
            .answer
            .errors
            .iter()
            .any(|error| error.contains("no question")),
        "ignored events must explain why: {:?}",
        outcome.answer.errors
    );
    assert!(
        provider.calls().is_empty(),
        "ignored events must never run the provider"
    );
}
