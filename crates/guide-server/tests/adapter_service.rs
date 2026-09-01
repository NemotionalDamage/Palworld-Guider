//! Adapter-mode service tests for the guide server.
//!
//! A real loopback gateway is driven by a fake adapter client speaking the
//! schema-2 protocol with Bearer authentication. The service runs on its own
//! dedicated thread while the shared agent stays usable by the Web server.

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use game_knowledge::KnowledgeStore;
use guide_adapter::{GameAdapterRuntime, InGameChatBridge, InGameLimits};
use guide_agent::{AgentConfig, AgentLimits, GuideAgent};
use guide_core::GuideEngine;
use guide_server::{start_adapter_service, GuideServer, ServerLimits};
use guide_tools::ToolRegistry;
use knowledge_index::KnowledgeIndex;
use provider::{ChatProvider, ChatRequest, ChatResponse, MockProvider, ProviderError};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;
use tower::ServiceExt;

mod common;
use common::{
    connect_authenticated, event_frame, gateway, hello, manifest, result_with_error, send_json,
};

const DATA_DIRECTORY: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/reviewed");
const PONG_REPLY: &str = "Pong: Palworld Guider adapter connected.";

struct ProviderHandle(Arc<MockProvider>);

impl ChatProvider for ProviderHandle {
    fn name(&self) -> &'static str {
        self.0.name()
    }

    fn complete(&self, request: &ChatRequest) -> Result<ChatResponse, ProviderError> {
        self.0.complete(request)
    }
}

fn registry() -> ToolRegistry {
    let store = KnowledgeStore::load_directory(DATA_DIRECTORY).expect("dataset is valid");
    let index = KnowledgeIndex::from_store(&store, None).expect("index builds");
    ToolRegistry::new(GuideEngine::new(store, None), index)
}

fn shared_agent(responses: Vec<ChatResponse>) -> (Arc<RwLock<GuideAgent>>, Arc<MockProvider>) {
    let provider = Arc::new(MockProvider::scripted(responses));
    let agent = GuideAgent::new(
        registry(),
        Box::new(ProviderHandle(provider.clone())),
        AgentConfig {
            limits: AgentLimits {
                max_tool_calls: 4,
                timeout: Duration::from_secs(5),
            },
            max_reply_characters: 1200,
        },
    );
    (Arc::new(RwLock::new(agent)), provider)
}

fn adapter_limits() -> InGameLimits {
    InGameLimits {
        max_history_exchanges: 4,
        max_asks_per_minute: 6,
        max_question_characters: 1000,
        max_reply_characters: 400,
        poll_interval: Duration::from_millis(20),
    }
}

fn wait_for_capabilities(runtime: &GameAdapterRuntime, expected: &[&str]) {
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        if runtime.capabilities() == expected {
            return;
        }
        if std::time::Instant::now() >= deadline {
            panic!(
                "gateway never published manifest {expected:?}; saw {:?}",
                runtime.capabilities()
            );
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

struct FakeAdapter {
    socket: common::ClientSocket,
    sequence: u64,
}

impl FakeAdapter {
    fn connect(address: std::net::SocketAddr, tools: &[&str]) -> Self {
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
        let call = common::read_json(&mut self.socket);
        assert_eq!(call["type"], "tool_call");
        assert_eq!(call["tool"], "send_chat_message");
        let message = call["args"]["message"].as_str().unwrap().to_string();
        self.sequence += 1;
        send_json(
            &mut self.socket,
            common::result(self.sequence, call["call_id"].as_str().unwrap(), "ok"),
        );
        message
    }

    fn respond_unavailable(&mut self) {
        let call = common::read_json(&mut self.socket);
        assert_eq!(call["type"], "tool_call");
        assert_eq!(call["tool"], "send_chat_message");
        self.sequence += 1;
        send_json(
            &mut self.socket,
            result_with_error(
                self.sequence,
                call["call_id"].as_str().unwrap(),
                "unavailable",
                Some("no authenticated adapter session is active"),
            ),
        );
    }
}

fn request(method: &str, uri: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .expect("request builds")
}

async fn json_body(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body reads");
    serde_json::from_slice(&bytes).expect("body is JSON")
}

async fn create_session(router: &axum::Router) -> String {
    let response = router
        .clone()
        .oneshot(request("POST", "/api/sessions", json!({})))
        .await
        .expect("session creation runs");
    assert_eq!(response.status(), StatusCode::OK);
    json_body(response).await["session_id"]
        .as_str()
        .expect("session ID is text")
        .to_string()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn adapter_service_processes_events_serially_and_stops() {
    let gateway = gateway();
    let address = gateway.local_addr();
    let runtime = GameAdapterRuntime::new(gateway);
    let (agent, _provider) = shared_agent(Vec::new());
    let bridge = Arc::new(Mutex::new(InGameChatBridge::new(
        runtime.clone(),
        adapter_limits(),
    )));
    let shutdown = Arc::new(AtomicBool::new(false));
    let handle = start_adapter_service(agent, runtime.clone(), bridge, shutdown.clone());

    let mut adapter = FakeAdapter::connect(address, &["send_chat_message"]);
    wait_for_capabilities(&runtime, &["send_chat_message"]);
    adapter.send_event("!guide ping");

    // The service must deliver exactly one Pong reply through send_chat.
    let message = adapter.respond_ok();
    assert_eq!(message, PONG_REPLY);

    shutdown.store(true, Ordering::SeqCst);
    let result = handle.join().expect("adapter service thread joins");
    assert_eq!(result, Ok(()), "shutdown must stop the service cleanly");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn adapter_gateway_errors_do_not_crash_the_web_server() {
    let gateway = gateway();
    let address = gateway.local_addr();
    let runtime = GameAdapterRuntime::new(gateway);
    // One shared agent feeds both the Web server and the adapter service.
    let (agent, provider) = shared_agent(vec![ChatResponse::text("advice")]);
    let web = GuideServer::new(Arc::clone(&agent), ServerLimits::default());
    let bridge = Arc::new(Mutex::new(InGameChatBridge::new(
        runtime.clone(),
        adapter_limits(),
    )));
    let shutdown = Arc::new(AtomicBool::new(false));
    let handle = start_adapter_service(
        Arc::clone(&agent),
        runtime.clone(),
        bridge,
        shutdown.clone(),
    );

    let mut adapter = FakeAdapter::connect(address, &["send_chat_message"]);
    wait_for_capabilities(&runtime, &["send_chat_message"]);
    adapter.send_event("!guide ping");
    // The gateway reports the session-less failure on the delivery call; the
    // bridge records the delivery error and the service keeps polling.
    adapter.respond_unavailable();

    // The Web server keeps serving with the same shared agent.
    let router = web.router();
    let session_id = create_session(&router).await;
    let response = router
        .clone()
        .oneshot(request(
            "POST",
            &format!("/api/sessions/{session_id}/ask"),
            json!({"question": "what should I do?"}),
        ))
        .await
        .expect("ask runs");
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["answer"]["answer"], json!("advice"));
    assert_eq!(
        provider.calls().len(),
        1,
        "exactly one web ask reaches the provider"
    );

    // The service is still alive after the gateway error and processes the
    // next event normally.
    adapter.send_event("!guide ping");
    let message = adapter.respond_ok();
    assert_eq!(message, PONG_REPLY);

    shutdown.store(true, Ordering::SeqCst);
    let result = handle.join().expect("adapter service thread joins");
    assert_eq!(result, Ok(()));
}
