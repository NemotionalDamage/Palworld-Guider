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
use std::sync::{Arc, RwLock};
use std::time::Duration;
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

fn test_agent(responses: Vec<ChatResponse>) -> GuideAgent {
    let store = KnowledgeStore::load_directory(DATA_DIRECTORY).expect("dataset is valid");
    let index = KnowledgeIndex::from_store(&store, None).expect("index builds");
    let engine = GuideEngine::new(store, None);
    GuideAgent::new(
        ToolRegistry::new(engine, index),
        Box::new(ProviderHandle(Arc::new(MockProvider::scripted(responses)))),
        AgentConfig {
            limits: AgentLimits {
                max_tool_calls: 4,
                timeout: Duration::from_secs(5),
            },
            max_reply_characters: 1200,
        },
    )
}

fn server(responses: Vec<ChatResponse>) -> GuideServer {
    GuideServer::new(
        Arc::new(RwLock::new(test_agent(responses))),
        ServerLimits {
            max_asks_per_minute: 1,
            max_snapshots_per_minute: 1,
            ..ServerLimits::default()
        },
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

fn empty_request(method: &str, uri: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .body(Body::empty())
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
            "game_version": "1.0.3",
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
async fn health_is_dependency_free() {
    let response = server(vec![ChatResponse::text("unused")])
        .router()
        .oneshot(empty_request("GET", "/health"))
        .await
        .expect("health runs");
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(json_body(response).await, json!({"status": "ok"}));
}

#[tokio::test]
async fn creates_a_session_without_private_startup_data() {
    let guide = server(vec![ChatResponse::text("unused")]);
    let response = guide
        .router()
        .oneshot(request("POST", "/api/sessions", json!({})))
        .await
        .expect("session creation runs");
    let body = json_body(response).await;
    assert_eq!(body["expires_in_seconds"], json!(1800));
    assert_eq!(body["session_id"].as_str().map(str::len), Some(32));
    let serialized = body.to_string();
    assert!(!serialized.contains("OPENAI_API_KEY"));
    assert!(!serialized.contains("reviewed"));
    assert!(!serialized.contains("C:"));
}

#[tokio::test]
async fn missing_session_returns_typed_not_found() {
    let response = server(vec![ChatResponse::text("unused")])
        .router()
        .oneshot(request(
            "GET",
            "/api/sessions/00000000000000000000000000000000",
            json!({}),
        ))
        .await
        .expect("session lookup runs");
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        json_body(response).await,
        json!({"error": {"code": "session_not_found", "message": "Session does not exist"}})
    );
}

#[tokio::test]
async fn snapshot_import_is_safe_and_feeds_the_agent() {
    let guide = server(vec![
        ChatResponse::tool(
            "state_call",
            "import_player_snapshot",
            json!({"confirmation": "user_entered"}),
        ),
        ChatResponse::text("Snapshot received."),
    ]);
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
        .expect("snapshot import runs");
    assert_eq!(response.status(), StatusCode::OK);
    let metadata = json_body(response).await;
    assert_eq!(metadata["schema_version"], json!("state_snapshot_v1"));
    assert_eq!(metadata["source_kind"], json!("user_entered"));
    assert_eq!(metadata["freshness"], json!("fresh"));
    assert!(metadata.get("captured_at").is_none());
    assert!(metadata.get("consent").is_none());

    let response = router
        .clone()
        .oneshot(request(
            "POST",
            &format!("/api/sessions/{session_id}/ask"),
            json!({"question": "What state do you see?"}),
        ))
        .await
        .expect("ask runs");
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["answer"]["status"], json!("ok"));
    assert_eq!(body["answer"]["answer"], json!("Snapshot received."));
}

#[tokio::test]
async fn invalid_snapshots_fail_without_raw_echo() {
    let guide = server(vec![ChatResponse::text("unused")]);
    let router = guide.router();
    let session_id = create_session(&router).await;
    let mut invalid = snapshot_value();
    invalid["unexpected"] = json!("secret");
    let response = router
        .oneshot(request(
            "POST",
            &format!("/api/sessions/{session_id}/snapshots"),
            invalid,
        ))
        .await
        .expect("snapshot import runs");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = json_body(response).await;
    assert_eq!(body["error"]["code"], json!("invalid_snapshot"));
    let serialized = body.to_string();
    assert!(!serialized.contains("secret"));
    assert!(!serialized.contains("operator-local-session"));
}

#[tokio::test]
async fn oversized_bodies_are_rejected() {
    let guide = server(vec![ChatResponse::text("unused")]);
    let router = guide.router();
    let session_id = create_session(&router).await;
    let large = "x".repeat(70_000);
    let response = router
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/sessions/{session_id}/ask"))
                .header("content-type", "application/json")
                .body(Body::from(json!({"question": large}).to_string()))
                .expect("request builds"),
        )
        .await
        .expect("body limit runs");
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(
        json_body(response).await["error"]["code"],
        json!("payload_too_large")
    );
}

#[tokio::test]
async fn excessive_json_depth_is_rejected() {
    let guide = server(vec![ChatResponse::text("unused")]);
    let router = guide.router();
    let session_id = create_session(&router).await;
    let mut nested = json!({"question": "hello"});
    for _ in 0..80 {
        nested = json!([nested]);
    }
    let response = router
        .oneshot(request(
            "POST",
            &format!("/api/sessions/{session_id}/ask"),
            nested,
        ))
        .await
        .expect("depth validation runs");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        json_body(response).await["error"]["code"],
        json!("json_depth_exceeded")
    );
}

#[tokio::test]
async fn empty_and_overlong_questions_are_rejected() {
    let guide = server(vec![ChatResponse::text("unused")]);
    let router = guide.router();
    let session_id = create_session(&router).await;
    for question in [String::new(), "x".repeat(2001)] {
        let response = router
            .clone()
            .oneshot(request(
                "POST",
                &format!("/api/sessions/{session_id}/ask"),
                json!({"question": question}),
            ))
            .await
            .expect("question validation runs");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            json_body(response).await["error"]["code"],
            json!("invalid_question")
        );
    }
}

#[tokio::test]
async fn history_and_rate_limits_are_visible() {
    let guide = server(vec![
        ChatResponse::text("Initial answer."),
        ChatResponse::text("Unused."),
    ]);
    let router = guide.router();
    let session_id = create_session(&router).await;
    let ask = request(
        "POST",
        &format!("/api/sessions/{session_id}/ask"),
        json!({"question": "First question"}),
    );
    let response = router.clone().oneshot(ask).await.expect("ask runs");
    assert_eq!(response.status(), StatusCode::OK);

    let response = router
        .clone()
        .oneshot(empty_request("GET", &format!("/api/sessions/{session_id}")))
        .await
        .expect("history runs");
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_body(response).await;
    assert_eq!(body["exchanges"][0]["question"], json!("First question"));
    assert_eq!(
        body["exchanges"][0]["answer"]["answer"],
        json!("Initial answer.")
    );

    let response = router
        .clone()
        .oneshot(request(
            "POST",
            &format!("/api/sessions/{session_id}/ask"),
            json!({"question": "Second question"}),
        ))
        .await
        .expect("rate limit runs");
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(
        json_body(response).await["error"]["code"],
        json!("ask_rate_limited")
    );
}

#[tokio::test]
async fn snapshot_rate_limit_is_visible() {
    let guide = server(vec![ChatResponse::text("unused")]);
    let router = guide.router();
    let session_id = create_session(&router).await;
    for round in 0..2 {
        let response = router
            .clone()
            .oneshot(request(
                "POST",
                &format!("/api/sessions/{session_id}/snapshots"),
                snapshot_value(),
            ))
            .await
            .expect("snapshot import runs");
        if round == 0 {
            assert_eq!(response.status(), StatusCode::OK);
        } else {
            assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
            assert_eq!(
                json_body(response).await["error"]["code"],
                json!("snapshot_rate_limited")
            );
        }
    }
}

#[tokio::test]
async fn browser_ui_is_local_and_complete() {
    let response = server(vec![ChatResponse::text("unused")])
        .router()
        .oneshot(empty_request("GET", "/"))
        .await
        .expect("UI runs");
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("UI reads");
    let page = String::from_utf8(bytes.to_vec()).expect("UI is UTF-8");
    assert!(page.contains("Palworld Guider"));
    assert!(page.contains("question"));
    assert!(page.contains("snapshot"));
    assert!(page.contains("provenance"));
    assert!(page.contains("uncertainty"));
    assert!(page.contains("error"));
    assert!(!page.contains("https://"));
    assert!(!page.contains("http://"));
}
