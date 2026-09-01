use crate::{
    error::{api_error, ApiErrorCode},
    session::{ExchangeRecord, ServerLimits, SessionError, SessionStore, SnapshotMetadata},
    ui::INDEX_HTML,
};
use axum::{
    body::{to_bytes, Body},
    extract::{rejection::JsonRejection, Path, State},
    http::{HeaderValue, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use guide_agent::{AgentAnswer, GuideAgent};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use state_snapshot::{PlayerStateSnapshot, SnapshotValidator};
use std::{
    sync::{Arc, Mutex, RwLock},
    time::Instant,
};
use tower_http::limit::RequestBodyLimitLayer;

const MAX_BODY_BYTES: usize = 65_536;
const MAX_JSON_DEPTH: usize = 32;

#[derive(Clone)]
pub struct GuideServerState {
    inner: Arc<GuideServerInner>,
}

struct GuideServerInner {
    agent: RwLock<GuideAgent>,
    sessions: Mutex<SessionStore>,
    limits: ServerLimits,
}

#[derive(Clone)]
pub struct GuideServer {
    state: GuideServerState,
}

impl GuideServer {
    pub fn new(agent: GuideAgent, limits: ServerLimits) -> Self {
        let state = GuideServerState {
            inner: Arc::new(GuideServerInner {
                agent: RwLock::new(agent),
                sessions: Mutex::new(SessionStore::new(limits)),
                limits,
            }),
        };
        Self { state }
    }

    pub fn router(&self) -> Router {
        Router::new()
            .route("/", get(index))
            .route("/health", get(health))
            .route("/api/sessions", post(create_session))
            .route("/api/sessions/{session_id}", get(session_history))
            .route(
                "/api/sessions/{session_id}/snapshots",
                post(attach_snapshot),
            )
            .route("/api/sessions/{session_id}/ask", post(ask))
            .with_state(self.state.clone())
            .layer(middleware::from_fn(payload_limit))
            .layer(RequestBodyLimitLayer::new(MAX_BODY_BYTES))
    }
}

#[derive(Serialize)]
struct SessionResponse {
    session_id: String,
    expires_in_seconds: u64,
}

#[derive(Serialize)]
struct SessionHistoryResponse {
    session_id: String,
    snapshot: Option<SnapshotMetadata>,
    exchanges: Vec<ExchangeRecord>,
}

#[derive(Deserialize)]
struct AskRequest {
    question: String,
}

#[derive(Serialize)]
struct AskResponse {
    session_id: String,
    answer: AgentAnswer,
}

async fn index() -> Response {
    (
        StatusCode::OK,
        [(
            axum::http::header::CONTENT_TYPE,
            HeaderValue::from_static("text/html; charset=utf-8"),
        )],
        INDEX_HTML,
    )
        .into_response()
}

async fn health() -> Json<Value> {
    Json(json!({"status": "ok"}))
}

async fn payload_limit(request: Request, next: Next) -> Response {
    let (parts, body) = request.into_parts();
    let bytes = match to_bytes(body, MAX_BODY_BYTES.saturating_add(1)).await {
        Ok(bytes) => bytes,
        Err(_) => return api_error(ApiErrorCode::PayloadTooLarge),
    };
    if bytes.len() > MAX_BODY_BYTES {
        return api_error(ApiErrorCode::PayloadTooLarge);
    }
    next.run(Request::from_parts(parts, Body::from(bytes)))
        .await
}

type Request = axum::http::Request<Body>;
type JsonResult = Result<Json<Value>, JsonRejection>;

async fn create_session(State(state): State<GuideServerState>, body: JsonResult) -> Response {
    let value = match check_json(body) {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Err(response) = check_depth(&value) {
        return response;
    }
    if !value.is_object() {
        return api_error(ApiErrorCode::InvalidRequest);
    }
    let now = Instant::now();
    let mut sessions = lock_sessions(&state);
    let record = match sessions.create_session(now) {
        Ok(record) => record,
        Err(error) => return session_error(error),
    };
    Json(SessionResponse {
        session_id: record.id,
        expires_in_seconds: state.inner.limits.session_ttl.as_secs(),
    })
    .into_response()
}

async fn session_history(
    State(state): State<GuideServerState>,
    Path(session_id): Path<String>,
) -> Response {
    let record = {
        let sessions = lock_sessions(&state);
        match sessions.session(&session_id, Instant::now()) {
            Ok(record) => record,
            Err(error) => return session_error(error),
        }
    };
    Json(SessionHistoryResponse {
        session_id: record.id,
        snapshot: record.snapshot,
        exchanges: record.exchanges,
    })
    .into_response()
}

async fn attach_snapshot(
    State(state): State<GuideServerState>,
    Path(session_id): Path<String>,
    body: JsonResult,
) -> Response {
    let value = match check_json(body) {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Err(response) = check_depth(&value) {
        return response;
    }
    let snapshot = match PlayerStateSnapshot::from_json(&value) {
        Ok(snapshot) => snapshot,
        Err(_) => return api_error(ApiErrorCode::InvalidSnapshot),
    };
    let validation = SnapshotValidator.validate(&snapshot, chrono::Utc::now());
    if !validation.valid {
        return api_error(ApiErrorCode::InvalidSnapshot);
    }
    let snapshot_for_agent = snapshot.clone();
    let now = Instant::now();
    let metadata = {
        let mut sessions = lock_sessions(&state);
        match sessions.attach_snapshot(&session_id, snapshot, now) {
            Ok(metadata) => metadata,
            Err(error) => return session_error(error),
        }
    };
    state
        .inner
        .agent
        .write()
        .expect("agent lock is not poisoned")
        .set_state_snapshot(snapshot_for_agent);
    Json(metadata).into_response()
}

async fn ask(
    State(state): State<GuideServerState>,
    Path(session_id): Path<String>,
    body: JsonResult,
) -> Response {
    let value = match check_json(body) {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Err(response) = check_depth(&value) {
        return response;
    }
    let request: AskRequest = match serde_json::from_value(value) {
        Ok(request) => request,
        Err(_) => return api_error(ApiErrorCode::InvalidRequest),
    };
    let question = request.question.trim();
    if question.is_empty() || question.chars().count() > 2_000 {
        return api_error(ApiErrorCode::InvalidQuestion);
    }
    let now = Instant::now();
    let prompt = {
        let mut sessions = lock_sessions(&state);
        match sessions.reserve_ask(&session_id, question, now) {
            Ok(prompt) => prompt,
            Err(error) => return session_error(error),
        }
    };
    let answer = state
        .inner
        .agent
        .read()
        .expect("agent lock is not poisoned")
        .ask(&prompt);
    let _ = lock_sessions(&state).complete_ask(&session_id, question, answer.clone());
    Json(AskResponse { session_id, answer }).into_response()
}

fn check_json(body: JsonResult) -> Result<Value, Response> {
    match body {
        Ok(Json(value)) => Ok(value),
        Err(_) => Err(api_error(ApiErrorCode::InvalidJson)),
    }
}

fn check_depth(value: &Value) -> Result<(), Response> {
    if json_depth(value) > MAX_JSON_DEPTH {
        return Err(api_error(ApiErrorCode::JsonDepthExceeded));
    }
    Ok(())
}

fn json_depth(value: &Value) -> usize {
    match value {
        Value::Array(values) => values
            .iter()
            .map(json_depth)
            .max()
            .map(|depth| depth + 1)
            .unwrap_or(1),
        Value::Object(fields) => fields
            .values()
            .map(json_depth)
            .max()
            .map(|depth| depth + 1)
            .unwrap_or(1),
        _ => 1,
    }
}

fn lock_sessions(state: &GuideServerState) -> std::sync::MutexGuard<'_, SessionStore> {
    state
        .inner
        .sessions
        .lock()
        .expect("session lock is not poisoned")
}

fn session_error(error: SessionError) -> Response {
    let code = match error {
        SessionError::NotFound => ApiErrorCode::SessionNotFound,
        SessionError::Expired => ApiErrorCode::SessionExpired,
        SessionError::SessionLimitReached => ApiErrorCode::SessionLimitReached,
        SessionError::AskRateLimited { .. } => ApiErrorCode::AskRateLimited,
        SessionError::SnapshotRateLimited { .. } => ApiErrorCode::SnapshotRateLimited,
    };
    api_error(code)
}
