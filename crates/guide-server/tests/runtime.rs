use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use game_knowledge::KnowledgeStore;
use guide_agent::{AgentConfig, AgentLimits, GuideAgent};
use guide_core::GuideEngine;
use guide_server::{GuideServer, ServerLimits};
use guide_tools::ToolRegistry;
use knowledge_index::KnowledgeIndex;
use provider::{ChatProvider, ChatRequest, ChatResponse, MockProvider, ProviderError};
use serde_json::{json, Value};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, RwLock,
    },
    time::Duration,
};
use tower::ServiceExt;

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

struct BlockingProvider {
    proceed: Arc<AtomicBool>,
    calls: Mutex<Vec<ChatRequest>>,
}

impl ChatProvider for BlockingProvider {
    fn name(&self) -> &'static str {
        "blocking"
    }

    fn complete(&self, request: &ChatRequest) -> Result<ChatResponse, ProviderError> {
        self.calls
            .lock()
            .expect("blocking provider log is not poisoned")
            .push(request.clone());
        while !self.proceed.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(5));
        }
        Ok(ChatResponse::text("late answer"))
    }
}

struct SlowProvider;

impl ChatProvider for SlowProvider {
    fn name(&self) -> &'static str {
        "slow"
    }

    fn complete(&self, _request: &ChatRequest) -> Result<ChatResponse, ProviderError> {
        std::thread::sleep(Duration::from_millis(100));
        Ok(ChatResponse::text("late"))
    }
}

fn registry() -> ToolRegistry {
    let store = KnowledgeStore::load_directory(DATA_DIRECTORY).expect("dataset is valid");
    let index = KnowledgeIndex::from_store(&store, None).expect("index builds");
    ToolRegistry::new(GuideEngine::new(store, None), index)
}

fn agent_with_provider(provider: Box<dyn ChatProvider>, timeout: Duration) -> GuideAgent {
    GuideAgent::new(
        registry(),
        provider,
        AgentConfig {
            limits: AgentLimits {
                max_tool_calls: 4,
                timeout,
            },
            max_reply_characters: 1200,
        },
    )
}

fn mock_server(responses: Vec<ChatResponse>) -> GuideServer {
    mock_server_with_provider(responses).0
}

fn mock_server_with_provider(responses: Vec<ChatResponse>) -> (GuideServer, Arc<MockProvider>) {
    let provider = Arc::new(MockProvider::scripted(responses));
    (
        GuideServer::new(
            Arc::new(RwLock::new(agent_with_provider(
                Box::new(ProviderHandle(provider.clone())),
                Duration::from_secs(5),
            ))),
            ServerLimits::default(),
        ),
        provider,
    )
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

fn snapshot_value() -> Value {
    let now = chrono::Utc::now() - chrono::Duration::seconds(1);
    json!({
        "schema_version": "state_snapshot_v1",
        "source": {
            "kind": "user_entered",
            "captured_at": now,
            "game_version": "1.0",
            "time_to_live_seconds": 60,
            "consent": {
                "id": "operator-local-session",
                "scope": ["guide_advice"],
                "granted_at": now
            }
        },
        "inventory": [{"item": "Wood", "quantity": 7, "evidence": "user_entered"}]
    })
}

#[tokio::test]
async fn follow_up_context_is_bounded_and_snapshot_redacted() {
    let responses = (0..7)
        .map(|round| ChatResponse::text(format!("answer {round}")))
        .collect();
    let (guide, provider) = mock_server_with_provider(responses);
    let router = guide.router();
    let session_id = create_session(&router).await;
    let response = router
        .clone()
        .oneshot(request(
            "POST",
            &format!("/api/sessions/{session_id}/snapshots"),
            snapshot_value(),
        ))
        .await
        .expect("snapshot attaches");
    assert_eq!(response.status(), StatusCode::OK);

    for round in 0..6 {
        let response = router
            .clone()
            .oneshot(request(
                "POST",
                &format!("/api/sessions/{session_id}/ask"),
                json!({"question": format!("question {round}")}),
            ))
            .await
            .expect("ask runs");
        assert_eq!(response.status(), StatusCode::OK);
    }

    let response = router
        .clone()
        .oneshot(request(
            "POST",
            &format!("/api/sessions/{session_id}/ask"),
            json!({"question": "question 6"}),
        ))
        .await
        .expect("final ask runs");
    assert_eq!(response.status(), StatusCode::OK);
    let calls = provider.calls();
    assert_eq!(calls.len(), 7);
    let prompt = calls[6].messages[0].content.as_str();
    assert!(prompt.contains("question 2"));
    assert!(!prompt.contains("question 0"));
    assert!(!prompt.contains("question 1"));
    assert!(!prompt.contains("operator-local-session"));
    assert!(!prompt.contains("captured_at"));
    assert!(!prompt.contains("consent"));
    assert!(!prompt.contains("Wood"));

    let history = router
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("GET")
                .uri(format!("/api/sessions/{session_id}"))
                .body(Body::empty())
                .expect("history request builds"),
        )
        .await
        .expect("history runs");
    let history_body = json_body(history).await;
    let serialized = serde_json::to_string(&history_body).expect("history serializes");
    assert!(!serialized.contains("operator-local-session"));
    assert!(!serialized.contains("captured_at"));
    assert!(!serialized.contains("consent"));
}

#[tokio::test]
async fn provider_failure_is_clear_and_unfabricated() {
    let guide = mock_server(vec![ChatResponse::text("first")]);
    let router = guide.router();
    let session_id = create_session(&router).await;
    let first = router
        .clone()
        .oneshot(request(
            "POST",
            &format!("/api/sessions/{session_id}/ask"),
            json!({"question": "one"}),
        ))
        .await
        .expect("first ask runs");
    assert_eq!(first.status(), StatusCode::OK);

    let second = router
        .oneshot(request(
            "POST",
            &format!("/api/sessions/{session_id}/ask"),
            json!({"question": "two"}),
        ))
        .await
        .expect("second ask runs");
    assert_eq!(second.status(), StatusCode::OK);
    let body = json_body(second).await;
    assert_eq!(body["answer"]["status"], json!("error"));
    assert_eq!(body["answer"]["answer"], Value::Null);
    assert!(body["answer"]["errors"]
        .as_array()
        .expect("errors are array")
        .iter()
        .any(|error| error.as_str().unwrap().contains("provider mock")));
}

#[tokio::test]
async fn agent_deadline_returns_timeout_error() {
    let guide = GuideServer::new(
        Arc::new(RwLock::new(agent_with_provider(
            Box::new(SlowProvider),
            Duration::from_secs(5),
        ))),
        ServerLimits {
            ask_timeout: Duration::from_millis(1),
            ..ServerLimits::default()
        },
    );
    let router = guide.router();
    let session_id = create_session(&router).await;
    let response = router
        .oneshot(request(
            "POST",
            &format!("/api/sessions/{session_id}/ask"),
            json!({"question": "slow?"}),
        ))
        .await
        .expect("ask runs");
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["answer"]["status"], json!("error"));
    assert!(body["answer"]["errors"]
        .as_array()
        .expect("errors are array")
        .iter()
        .any(|error| error.as_str().unwrap().contains("timeout")));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancellation_token_stops_waiting_for_a_blocked_provider() {
    let proceed = Arc::new(AtomicBool::new(false));
    let provider = BlockingProvider {
        proceed: proceed.clone(),
        calls: Mutex::new(Vec::new()),
    };
    let guide = GuideServer::new(
        Arc::new(RwLock::new(agent_with_provider(
            Box::new(provider),
            Duration::from_secs(5),
        ))),
        ServerLimits::default(),
    );
    let router = guide.router();
    let session_id = create_session(&router).await;
    let ask_router = router.clone();
    let ask_task = tokio::spawn(async move {
        ask_router
            .oneshot(request(
                "POST",
                &format!("/api/sessions/{session_id}/ask"),
                json!({
                    "question": "blocked?",
                    "cancellation_token": "1234567890abcdef1234567890abcdef"
                }),
            ))
            .await
            .expect("ask runs")
    });

    tokio::time::sleep(Duration::from_millis(50)).await;
    let cancellation_result = router
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/cancellations/1234567890abcdef1234567890abcdef")
                .body(Body::empty())
                .expect("cancellation request builds"),
        )
        .await
        .expect("cancellation runs");
    proceed.store(true, Ordering::SeqCst);
    let cancellation = cancellation_result;
    assert_eq!(cancellation.status(), StatusCode::ACCEPTED);

    let ask_result = tokio::time::timeout(Duration::from_secs(2), ask_task).await;
    proceed.store(true, Ordering::SeqCst);
    let ask = ask_result
        .expect("ask returns after cancellation")
        .expect("ask task succeeds");
    assert_eq!(ask.status(), StatusCode::CONFLICT);
    let body = json_body(ask).await;
    assert_eq!(
        body,
        json!({"error": {"code": "request_cancelled", "message": "Request was cancelled"}})
    );
    proceed.store(true, Ordering::SeqCst);
}

#[tokio::test]
async fn unknown_cancellation_token_fails_clearly() {
    let guide = mock_server(vec![ChatResponse::text("unused")]);
    let router = guide.router();
    let response = router
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/cancellations/1234567890abcdef1234567890abcdef")
                .body(Body::empty())
                .expect("cancellation request builds"),
        )
        .await
        .expect("cancellation runs");
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        json_body(response).await,
        json!({
            "error": {
                "code": "cancellation_not_found",
                "message": "Cancellation token does not exist or is already finished"
            }
        })
    );
}
