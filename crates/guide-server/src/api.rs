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
use guide_core::VersionInfo;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use state_snapshot::{PlayerStateSnapshot, SnapshotValidator};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, RwLock,
    },
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
    agent: Arc<RwLock<GuideAgent>>,
    sessions: Mutex<SessionStore>,
    cancellations: Mutex<HashMap<String, CancellationHandle>>,
    limits: ServerLimits,
}

struct CancellationHandle {
    flag: Arc<AtomicBool>,
    sender: tokio::sync::watch::Sender<bool>,
    created_at: Instant,
}

#[derive(Clone)]
pub struct GuideServer {
    state: GuideServerState,
}

impl GuideServer {
    /// Create the Web server around a shared agent. The same
    /// `Arc<RwLock<GuideAgent>>` may back the in-game adapter service so both
    /// interfaces use one tool registry and one provider budget.
    pub fn new(agent: Arc<RwLock<GuideAgent>>, limits: ServerLimits) -> Self {
        let state = GuideServerState {
            inner: Arc::new(GuideServerInner {
                agent,
                sessions: Mutex::new(SessionStore::new(limits)),
                cancellations: Mutex::new(HashMap::new()),
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
            .route("/api/cancellations/{token}", post(cancel_ask))
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
    cancellation_token: Option<String>,
}

#[derive(Serialize)]
struct AskResponse {
    session_id: String,
    answer: AgentAnswer,
}

enum AskOutcome {
    Completed(AgentAnswer),
    Cancelled,
    TimedOut,
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
        Err(code) => return api_error(code),
    };
    if let Err(code) = check_depth(&value) {
        return api_error(code);
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
        Err(code) => return api_error(code),
    };
    if let Err(code) = check_depth(&value) {
        return api_error(code);
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
        Err(code) => return api_error(code),
    };
    if let Err(code) = check_depth(&value) {
        return api_error(code);
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
    let ask_context = {
        let mut sessions = lock_sessions(&state);
        match sessions.reserve_ask(&session_id, question, now) {
            Ok(ask_context) => ask_context,
            Err(error) => return session_error(error),
        }
    };
    let token = match request.cancellation_token {
        Some(token) => {
            if token.len() != 32 || u128::from_str_radix(&token, 16).is_err() {
                return api_error(ApiErrorCode::InvalidRequest);
            }
            Some(token)
        }
        None => None,
    };
    let cancellation_flag = Arc::new(AtomicBool::new(false));
    let (cancel_sender, mut cancel_receiver) = tokio::sync::watch::channel(false);
    if let Some(token) = token.as_ref() {
        let mut cancellations = lock_cancellations(&state);
        remove_expired_cancellations(
            &mut cancellations,
            Instant::now(),
            state.inner.limits.ask_timeout,
        );
        cancellations.insert(
            token.clone(),
            CancellationHandle {
                flag: cancellation_flag.clone(),
                sender: cancel_sender.clone(),
                created_at: Instant::now(),
            },
        );
    }
    let agent = state.inner.agent.clone();
    let blocking_question = ask_context.current_question.clone();
    let blocking_history = ask_context.history.clone();
    let blocking_flag = cancellation_flag.clone();
    let task = tokio::task::spawn_blocking(move || {
        agent
            .read()
            .expect("agent lock is not poisoned")
            .ask_with_history_and_cancellation(
                &blocking_question,
                &blocking_history,
                &blocking_flag,
            )
    });
    let outcome = tokio::time::timeout(state.inner.limits.ask_timeout, async {
        if token.is_some() {
            tokio::select! {
                answer = task => AskOutcome::Completed(join_answer(answer)),
                _ = cancel_receiver.changed() => AskOutcome::Cancelled,
            }
        } else {
            AskOutcome::Completed(join_answer(task.await))
        }
    })
    .await
    .unwrap_or(AskOutcome::TimedOut);
    if let Some(token) = token.as_ref() {
        lock_cancellations(&state).remove(token);
    }
    match outcome {
        AskOutcome::Cancelled => api_error(ApiErrorCode::RequestCancelled),
        AskOutcome::TimedOut => {
            cancellation_flag.store(true, Ordering::SeqCst);
            let answer = timeout_answer();
            let _ = lock_sessions(&state).complete_ask(&session_id, question, answer.clone());
            Json(AskResponse { session_id, answer }).into_response()
        }
        AskOutcome::Completed(answer) => {
            let _ = lock_sessions(&state).complete_ask(&session_id, question, answer.clone());
            Json(AskResponse { session_id, answer }).into_response()
        }
    }
}

async fn cancel_ask(State(state): State<GuideServerState>, Path(token): Path<String>) -> Response {
    let now = Instant::now();
    let mut cancellations = lock_cancellations(&state);
    remove_expired_cancellations(&mut cancellations, now, state.inner.limits.ask_timeout);
    let Some(handle) = cancellations.remove(&token) else {
        return api_error(ApiErrorCode::CancellationNotFound);
    };
    handle.flag.store(true, Ordering::SeqCst);
    let _ = handle.sender.send(true);
    (StatusCode::ACCEPTED, Json(json!({"status": "cancelled"}))).into_response()
}

fn join_answer(result: Result<AgentAnswer, tokio::task::JoinError>) -> AgentAnswer {
    match result {
        Ok(answer) => answer,
        Err(error) => AgentAnswer {
            status: guide_agent::AgentStatus::Error,
            answer: None,
            tool_calls: Vec::new(),
            provenance: Vec::new(),
            version: unknown_version(),
            uncertainty: Vec::new(),
            errors: vec![format!("agent worker failed: {error}")],
        },
    }
}

fn timeout_answer() -> AgentAnswer {
    AgentAnswer {
        status: guide_agent::AgentStatus::Error,
        answer: None,
        tool_calls: Vec::new(),
        provenance: Vec::new(),
        version: unknown_version(),
        uncertainty: Vec::new(),
        errors: vec!["server ask timeout exceeded".to_string()],
    }
}

fn unknown_version() -> VersionInfo {
    VersionInfo {
        knowledge_version: "unknown".to_string(),
        configured_game_version: None,
        matches: true,
    }
}

fn lock_cancellations(
    state: &GuideServerState,
) -> std::sync::MutexGuard<'_, HashMap<String, CancellationHandle>> {
    state
        .inner
        .cancellations
        .lock()
        .expect("cancellation lock is not poisoned")
}

fn remove_expired_cancellations(
    cancellations: &mut HashMap<String, CancellationHandle>,
    now: Instant,
    timeout: std::time::Duration,
) {
    cancellations.retain(|_, handle| now.duration_since(handle.created_at) <= timeout);
}
fn check_json(body: JsonResult) -> Result<Value, ApiErrorCode> {
    match body {
        Ok(Json(value)) => Ok(value),
        Err(_) => Err(ApiErrorCode::InvalidJson),
    }
}

fn check_depth(value: &Value) -> Result<(), ApiErrorCode> {
    if json_depth(value) > MAX_JSON_DEPTH {
        return Err(ApiErrorCode::JsonDepthExceeded);
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
