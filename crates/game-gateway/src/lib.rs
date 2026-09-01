//! Schema-2 loopback WebSocket gateway used by the Palworld Guider read-only
//! game adapter.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::io::ErrorKind;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::sleep;
use std::time::SystemTime;
use std::time::{Duration, Instant, UNIX_EPOCH};
use thiserror::Error;
use tungstenite::handshake::server::{Request, Response};
use tungstenite::http::StatusCode;
use tungstenite::protocol::frame::coding::CloseCode;
use tungstenite::protocol::frame::{CloseFrame, Utf8Bytes};
use tungstenite::{accept_hdr, Message};

const SCHEMA_VERSION: u32 = 1;
pub const PROTOCOL_SCHEMA_VERSION: u32 = 2;

// Bounded queue limits shared by every session. Each queue caps at 64
// messages so a slow consumer or adapter cannot grow gateway memory
// without limit; event overflow closes the session instead of silently
// dropping player messages.
const MAX_QUEUED_EVENTS: usize = 64;
const OUTBOUND_QUEUE_CAPACITY: usize = 64;
const MAX_PENDING_CALLS: usize = 64;

#[derive(Debug, Error)]
pub enum GatewayError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("invalid gateway event: {0}")]
    InvalidEvent(String),
    #[error("invalid gateway response: {0}")]
    InvalidResponse(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatEvent {
    pub event_id: String,
    pub sequence: u64,
    pub timestamp_ms: u64,
    pub source: String,
    pub player_id: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolResult {
    pub schema_version: u32,
    #[serde(rename = "type")]
    pub kind: String,
    pub call_id: String,
    pub sequence: u64,
    pub timestamp_ms: u64,
    pub status: TaskStatus,
    pub data: Value,
    pub error: Option<String>,
    pub elapsed_ms: u64,
}

impl ToolResult {
    pub fn ok(data: Value) -> Self {
        Self::new("local-tool", TaskStatus::Ok, data, None)
    }

    pub fn timeout(call_id: impl Into<String>) -> Self {
        Self::new(
            call_id,
            TaskStatus::Timeout,
            Value::Null,
            Some("adapter response timed out".into()),
        )
    }

    pub fn new_status(status: TaskStatus, reason: &str) -> Self {
        Self::new("local-tool", status, Value::Null, Some(reason.to_string()))
    }

    fn new(
        call_id: impl Into<String>,
        status: TaskStatus,
        data: Value,
        error: Option<String>,
    ) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            kind: "tool_result".into(),
            call_id: call_id.into(),
            sequence: 0,
            timestamp_ms: unix_ms(),
            status,
            data,
            error,
            elapsed_ms: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Ok,
    InvalidArgs,
    Unsupported,
    Unavailable,
    Timeout,
    NotFound,
    GameError,
    Cancelled,
    RateLimited,
    InternalError,
}

impl TaskStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::InvalidArgs => "invalid_args",
            Self::Unsupported => "unsupported",
            Self::Unavailable => "unavailable",
            Self::Timeout => "timeout",
            Self::NotFound => "not_found",
            Self::GameError => "game_error",
            Self::Cancelled => "cancelled",
            Self::RateLimited => "rate_limited",
            Self::InternalError => "internal_error",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameKind {
    Hello,
    CapabilityManifest,
    Event,
    ToolCall,
    ToolResult,
    Heartbeat,
    Error,
}

impl FrameKind {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "hello" => Some(Self::Hello),
            "capability_manifest" => Some(Self::CapabilityManifest),
            "event" => Some(Self::Event),
            "tool_call" => Some(Self::ToolCall),
            "tool_result" => Some(Self::ToolResult),
            "heartbeat" => Some(Self::Heartbeat),
            "error" => Some(Self::Error),
            _ => None,
        }
    }
}

fn frame_kind_name(kind: FrameKind) -> &'static str {
    match kind {
        FrameKind::Hello => "hello",
        FrameKind::CapabilityManifest => "capability_manifest",
        FrameKind::Event => "event",
        FrameKind::ToolCall => "tool_call",
        FrameKind::ToolResult => "tool_result",
        FrameKind::Heartbeat => "heartbeat",
        FrameKind::Error => "error",
    }
}

fn gateway_frame_log(
    session_id: u64,
    kind: FrameKind,
    sequence: u64,
    capability_count: Option<usize>,
) -> Value {
    serde_json::json!({
        "level": "info",
        "message": "gateway frame accepted",
        "session_id": session_id,
        "frame_type": frame_kind_name(kind),
        "sequence": sequence,
        "capability_count": capability_count,
    })
}

fn emit_gateway_frame_log(
    session_id: u64,
    kind: FrameKind,
    sequence: u64,
    capability_count: Option<usize>,
) {
    eprintln!(
        "{}",
        gateway_frame_log(session_id, kind, sequence, capability_count)
    );
}

fn gateway_outbound_log(session_id: u64, frame: &Value) -> Value {
    let frame_type = frame
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let sequence = frame.get("seq").and_then(Value::as_u64).unwrap_or_default();
    let mut log = serde_json::json!({
        "level": "info",
        "message": "gateway frame sent",
        "session_id": session_id,
        "frame_type": frame_type,
        "sequence": sequence,
    });
    if frame_type == "tool_call" {
        log["tool_name"] = serde_json::json!(frame.get("tool").and_then(Value::as_str));
    }
    log
}

fn gateway_error_log(session_id: u64, direction: &str, error: &str) -> Value {
    serde_json::json!({
        "level": "error",
        "message": "gateway transport error",
        "session_id": session_id,
        "direction": direction,
        "error": error,
    })
}

fn emit_gateway_outbound_log(session_id: u64, frame: &Value) {
    eprintln!("{}", gateway_outbound_log(session_id, frame));
}

fn emit_gateway_error_log(session_id: u64, direction: &str, error: &str) {
    eprintln!("{}", gateway_error_log(session_id, direction, error));
}

#[cfg(test)]
mod gateway_frame_logging_tests {
    use super::*;

    #[test]
    fn frame_log_contains_metadata_but_never_payload_or_credentials() {
        let log = gateway_frame_log(7, FrameKind::Hello, 1, None);

        assert_eq!(log["level"], "info");
        assert_eq!(log["message"], "gateway frame accepted");
        assert_eq!(log["session_id"], 7);
        assert_eq!(log["frame_type"], "hello");
        assert_eq!(log["sequence"], 1);
        assert!(log["payload"].is_null());
        assert!(log["authorization"].is_null());
    }

    #[test]
    fn outbound_and_error_logs_contain_metadata_but_never_payload_or_credentials() {
        let frame = serde_json::json!({
            "type": "tool_call",
            "seq": 9,
            "tool": "get_player_status",
            "args": {"secret": "do-not-log"}
        });
        let log = gateway_outbound_log(7, &frame);

        assert_eq!(log["level"], "info");
        assert_eq!(log["message"], "gateway frame sent");
        assert_eq!(log["session_id"], 7);
        assert_eq!(log["frame_type"], "tool_call");
        assert_eq!(log["sequence"], 9);
        assert_eq!(log["tool_name"], "get_player_status");
        assert!(log["payload"].is_null());
        assert!(log["authorization"].is_null());

        let error_log = gateway_error_log(7, "read", "connection reset");
        assert_eq!(error_log["level"], "error");
        assert_eq!(error_log["message"], "gateway transport error");
        assert_eq!(error_log["session_id"], 7);
        assert_eq!(error_log["direction"], "read");
        assert_eq!(error_log["error"], "connection reset");
        assert!(error_log["payload"].is_null());
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedFrame {
    pub kind: FrameKind,
    pub sequence: u64,
    pub timestamp_ms: u64,
    pub correlation_id: Option<String>,
    pub data: Value,
}

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("frame payload exceeds {0} bytes")]
    PayloadTooLarge(usize),
    #[error("unsupported schema version {0}")]
    UnsupportedSchema(u32),
    #[error("unsupported frame type {0}")]
    UnsupportedType(String),
    #[error("duplicate frame sequence {0}")]
    DuplicateSequence(u64),
    #[error("stale frame sequence {0}")]
    StaleSequence(u64),
    #[error("invalid {1} field {0}")]
    InvalidField(&'static str, &'static str),
    #[error("missing field {0}")]
    MissingField(&'static str),
    #[error("deserialization error: {0}")]
    Deserialization(#[from] serde_json::Error),
}

#[derive(Debug, Deserialize)]
struct CommonFrame {
    schema_version: u32,
    #[serde(rename = "type")]
    kind: String,
    seq: u64,
    timestamp_ms: u64,
    #[serde(default)]
    correlation_id: Option<String>,
}

pub struct FrameValidator {
    max_payload_bytes: usize,
    last_sequence: Option<u64>,
}

impl FrameValidator {
    pub fn new(max_payload_bytes: usize) -> Self {
        Self {
            max_payload_bytes,
            last_sequence: None,
        }
    }

    pub fn validate_bytes(&mut self, bytes: &[u8]) -> Result<ParsedFrame, ProtocolError> {
        if bytes.len() > self.max_payload_bytes {
            return Err(ProtocolError::PayloadTooLarge(self.max_payload_bytes));
        }
        let value = serde_json::from_slice::<Value>(bytes)?;
        self.validate_value(&value)
    }

    pub fn validate_value(&mut self, value: &Value) -> Result<ParsedFrame, ProtocolError> {
        let encoded_len = serde_json::to_vec(value)?.len();
        if encoded_len > self.max_payload_bytes {
            return Err(ProtocolError::PayloadTooLarge(self.max_payload_bytes));
        }
        let common = serde_json::from_value::<CommonFrame>(value.clone())?;
        if common.schema_version != PROTOCOL_SCHEMA_VERSION {
            return Err(ProtocolError::UnsupportedSchema(common.schema_version));
        }
        let Some(kind) = FrameKind::parse(&common.kind) else {
            return Err(ProtocolError::UnsupportedType(common.kind));
        };
        if let Some(last) = self.last_sequence {
            if common.seq == last {
                return Err(ProtocolError::DuplicateSequence(common.seq));
            }
            if common.seq < last {
                return Err(ProtocolError::StaleSequence(common.seq));
            }
        }
        validate_kind_fields(kind, value)?;
        let frame = ParsedFrame {
            kind,
            sequence: common.seq,
            timestamp_ms: common.timestamp_ms,
            correlation_id: common.correlation_id,
            data: value.get("data").cloned().unwrap_or(Value::Null),
        };
        self.last_sequence = Some(common.seq);
        Ok(frame)
    }
}

fn validate_kind_fields(kind: FrameKind, value: &Value) -> Result<(), ProtocolError> {
    match kind {
        FrameKind::Hello | FrameKind::Heartbeat => Ok(()),
        FrameKind::CapabilityManifest => {
            let tools = value
                .pointer("/data/tools")
                .and_then(Value::as_array)
                .ok_or(ProtocolError::MissingField("data.tools"))?;
            for tool in tools {
                require_object(Some(tool), "data.tools")?;
                require_string(tool.get("name"), "name")?;
                require_string(tool.get("evidence"), "evidence")?;
                require_string(tool.get("status"), "status")?;
                if !tool.get("mutation").is_some_and(Value::is_boolean) {
                    return Err(ProtocolError::InvalidField("mutation", "data.tools"));
                }
                let status = tool.get("status").and_then(Value::as_str).unwrap_or("");
                if !matches!(status, "enabled" | "disabled") {
                    return Err(ProtocolError::InvalidField("status", "data.tools"));
                }
            }
            Ok(())
        }
        FrameKind::Event => {
            if value.get("event").and_then(Value::as_str) != Some("chat_message") {
                return Err(ProtocolError::InvalidField("event", "event"));
            }
            require_string(value.get("event_id"), "event_id")?;
            require_string(value.get("source"), "source")?;
            require_string(value.pointer("/data/player_id"), "data.player_id")?;
            require_string(value.pointer("/data/text"), "data.text")?;
            Ok(())
        }
        FrameKind::ToolCall => {
            let call_id = require_string(value.get("call_id"), "call_id")?;
            require_string(value.get("tool"), "tool")?;
            require_object(value.get("args"), "args")?;
            require_positive_u64(value.get("timeout_ms"), "timeout_ms")?;
            require_positive_u64(value.get("deadline_ms"), "deadline_ms")?;
            if let Some(correlation_id) = value.get("correlation_id").and_then(Value::as_str) {
                if correlation_id != call_id {
                    return Err(ProtocolError::InvalidField("correlation_id", "tool_call"));
                }
            }
            Ok(())
        }
        FrameKind::ToolResult => {
            require_string(value.get("call_id"), "call_id")?;
            let status = require_string(value.get("status"), "status")?;
            serde_json::from_value::<TaskStatus>(Value::String(status.to_string()))
                .map_err(|_| ProtocolError::InvalidField("status", "tool_result"))?;
            if !value.get("data").is_some() {
                return Err(ProtocolError::MissingField("data"));
            }
            if let Some(error) = value.get("error") {
                if !error.is_null() && !error.is_string() {
                    return Err(ProtocolError::InvalidField("error", "tool_result"));
                }
            }
            require_nonnegative_u64(value.get("elapsed_ms"), "elapsed_ms")?;
            Ok(())
        }
        FrameKind::Error => {
            require_string(value.pointer("/error/code"), "error.code")?;
            require_string(value.pointer("/error/message"), "error.message")?;
            Ok(())
        }
    }
}

fn require_object(value: Option<&Value>, field: &'static str) -> Result<(), ProtocolError> {
    value
        .filter(|value: &&Value| value.is_object())
        .map(|_| ())
        .ok_or(ProtocolError::InvalidField(field, "object"))
}

fn require_string<'a>(
    value: Option<&'a Value>,
    field: &'static str,
) -> Result<&'a str, ProtocolError> {
    value
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or(ProtocolError::InvalidField(field, "string"))
}

fn require_positive_u64(value: Option<&Value>, field: &'static str) -> Result<(), ProtocolError> {
    value
        .and_then(Value::as_u64)
        .filter(|value| *value > 0)
        .map(|_| ())
        .ok_or(ProtocolError::InvalidField(field, "positive integer"))
}

fn require_nonnegative_u64(
    value: Option<&Value>,
    field: &'static str,
) -> Result<(), ProtocolError> {
    value
        .and_then(Value::as_u64)
        .map(|_| ())
        .ok_or(ProtocolError::InvalidField(field, "nonnegative integer"))
}

#[derive(Debug, Clone)]
pub struct WebSocketGatewayConfig {
    pub max_payload_bytes: usize,
    pub call_timeout: Duration,
}

impl Default for WebSocketGatewayConfig {
    fn default() -> Self {
        Self {
            max_payload_bytes: 64 * 1024,
            call_timeout: Duration::from_secs(3),
        }
    }
}

#[derive(Default)]
struct SharedGatewayState {
    capabilities: Vec<String>,
    events: VecDeque<ChatEvent>,
    pending: HashMap<String, std::sync::mpsc::Sender<ToolResult>>,
    active_session: Option<u64>,
    active_sender: Option<std::sync::mpsc::SyncSender<Value>>,
}

#[derive(Default)]
struct GatewaySignals {
    shutdown: AtomicBool,
    startup_nonce: AtomicU64,
    next_call_id: AtomicU64,
    next_session: AtomicU64,
}

pub struct WebSocketGateway {
    local_addr: SocketAddr,
    config: WebSocketGatewayConfig,
    state: Arc<Mutex<SharedGatewayState>>,
    state_changed: Arc<Condvar>,
    signals: Arc<GatewaySignals>,
}

impl WebSocketGateway {
    pub fn bind(
        address: &str,
        bearer_token: &str,
        config: WebSocketGatewayConfig,
    ) -> Result<Self, GatewayError> {
        if bearer_token.is_empty() {
            return Err(GatewayError::InvalidResponse(
                "websocket bearer token must not be empty".into(),
            ));
        }
        let listener = TcpListener::bind(address)?;
        listener.set_nonblocking(true)?;
        let local_addr = listener.local_addr()?;
        let signals = Arc::new(GatewaySignals {
            shutdown: AtomicBool::new(false),
            startup_nonce: AtomicU64::new(unix_ns()),
            next_call_id: AtomicU64::new(0),
            next_session: AtomicU64::new(1),
        });
        let state = Arc::new(Mutex::new(SharedGatewayState::default()));
        let state_changed = Arc::new(Condvar::new());
        let thread_state = Arc::clone(&state);
        let thread_signals = Arc::clone(&signals);
        let thread_state_changed = Arc::clone(&state_changed);
        let token = bearer_token.to_string();
        let thread_config = config.clone();

        std::thread::Builder::new()
            .name("palworld-guider-gateway".into())
            .spawn(move || {
                accept_connections(
                    listener,
                    token,
                    thread_config,
                    thread_state,
                    thread_state_changed,
                    thread_signals,
                );
            })?;

        Ok(Self {
            local_addr,
            config,
            state,
            state_changed,
            signals,
        })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    pub fn capabilities(&self) -> Vec<String> {
        self.state
            .lock()
            .expect("gateway state lock poisoned")
            .capabilities
            .clone()
    }

    pub fn next_event_timeout(
        &mut self,
        timeout: Duration,
    ) -> Result<Option<ChatEvent>, GatewayError> {
        let deadline = Instant::now() + timeout;
        let mut state = self.state.lock().expect("gateway state lock poisoned");
        loop {
            if let Some(event) = state.events.pop_front() {
                self.state_changed.notify_all();
                return Ok(Some(event));
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Ok(None);
            }
            let (updated_state, _) = self
                .state_changed
                .wait_timeout(state, remaining.min(Duration::from_millis(20)))
                .expect("gateway state lock poisoned");
            state = updated_state;
        }
    }

    pub fn call_tool(&self, tool: &str, args: Value) -> Result<ToolResult, GatewayError> {
        let call_id = self.next_call_id();
        let timeout_ms = self.config.call_timeout.as_millis().min(u64::MAX as u128) as u64;
        let frame = json_frame(tool, &call_id, &args, timeout_ms);
        let (result_sender, result_receiver) = std::sync::mpsc::channel::<ToolResult>();
        {
            let mut state = self.state.lock().expect("gateway state lock poisoned");
            if state.pending.len() >= MAX_PENDING_CALLS {
                return Ok(local_result(
                    &call_id,
                    TaskStatus::RateLimited,
                    "too many pending tool calls",
                ));
            }
            state.pending.insert(call_id.clone(), result_sender);
        }

        let sender = {
            self.state
                .lock()
                .expect("gateway state lock poisoned")
                .active_sender
                .clone()
        };
        let Some(sender) = sender else {
            self.remove_pending(&call_id);
            return Ok(local_result(
                &call_id,
                TaskStatus::Unavailable,
                "no authenticated adapter session is active",
            ));
        };
        match sender.try_send(frame) {
            Ok(()) => {}
            Err(std::sync::mpsc::TrySendError::Disconnected(_)) => {
                self.remove_pending(&call_id);
                return Ok(local_result(
                    &call_id,
                    TaskStatus::Cancelled,
                    "adapter session disconnected",
                ));
            }
            Err(std::sync::mpsc::TrySendError::Full(_)) => {
                self.remove_pending(&call_id);
                return Ok(local_result(
                    &call_id,
                    TaskStatus::RateLimited,
                    "adapter outbound queue full",
                ));
            }
        }

        match result_receiver.recv_timeout(self.config.call_timeout) {
            Ok(result) => Ok(result),
            Err(_) => {
                self.remove_pending(&call_id);
                Ok(ToolResult::timeout(call_id))
            }
        }
    }

    fn next_call_id(&self) -> String {
        let nonce = self.signals.startup_nonce.load(Ordering::SeqCst);
        let call_id = self.signals.next_call_id.fetch_add(1, Ordering::SeqCst);
        format!("ws-{nonce}-{call_id:020}")
    }

    fn remove_pending(&self, call_id: &str) {
        self.state
            .lock()
            .expect("gateway state lock poisoned")
            .pending
            .remove(call_id);
    }
}

impl Drop for WebSocketGateway {
    fn drop(&mut self) {
        self.signals.shutdown.store(true, Ordering::SeqCst);
    }
}
fn accept_connections(
    listener: TcpListener,
    token: String,
    config: WebSocketGatewayConfig,
    state: Arc<Mutex<SharedGatewayState>>,
    state_changed: Arc<Condvar>,
    signals: Arc<GatewaySignals>,
) {
    loop {
        if signals.shutdown.load(Ordering::SeqCst) {
            return;
        }
        match listener.accept() {
            Ok((stream, _)) => {
                let state = Arc::clone(&state);
                let state_changed = Arc::clone(&state_changed);
                let signals = Arc::clone(&signals);
                let token = token.clone();
                let config = config.clone();
                std::thread::Builder::new()
                    .name("palworld-guider-gateway".into())
                    .spawn(move || {
                        run_session(stream, token, config, state, state_changed, signals)
                    })
                    .ok();
            }
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                sleep(Duration::from_millis(2));
            }
            Err(_) => return,
        }
    }
}

fn run_session(
    stream: TcpStream,
    token: String,
    config: WebSocketGatewayConfig,
    state: Arc<Mutex<SharedGatewayState>>,
    state_changed: Arc<Condvar>,
    signals: Arc<GatewaySignals>,
) {
    let expected_token = format!("Bearer {token}");
    #[allow(clippy::result_large_err)]
    let callback = |request: &Request, response: Response| {
        let authorized = request
            .headers()
            .get("Authorization")
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value == expected_token);
        if authorized {
            Ok(response)
        } else {
            let error = Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .body(Some("Unauthorized".to_string()))
                .expect("static unauthorized response");
            Err(error)
        }
    };
    let Ok(mut socket) = accept_hdr(stream, callback) else {
        return;
    };
    if socket.get_ref().set_nonblocking(true).is_err() {
        return;
    }

    let session_id = signals.next_session.fetch_add(1, Ordering::SeqCst);
    let (outbound_sender, outbound_receiver) =
        std::sync::mpsc::sync_channel::<Value>(OUTBOUND_QUEUE_CAPACITY);
    {
        let mut shared = state.lock().expect("gateway state lock poisoned");
        cancel_pending(
            &mut shared,
            TaskStatus::Cancelled,
            "adapter session replaced",
        );
        shared.capabilities.clear();
        shared.active_session = Some(session_id);
        shared.active_sender = Some(outbound_sender);
    }
    state_changed.notify_all();

    let mut validator = FrameValidator::new(config.max_payload_bytes);
    let mut session_stage = SessionStage::AwaitingHello;
    let mut outbound_sequence = 0_u64;
    loop {
        if signals.shutdown.load(Ordering::SeqCst) {
            break;
        }
        match socket.read() {
            Ok(Message::Text(text)) => {
                let Ok(value) = serde_json::from_str::<Value>(&text) else {
                    let _ = close_with_policy(&mut socket, "malformed JSON");
                    break;
                };
                let frame = match validator.validate_bytes(value.to_string().as_bytes()) {
                    Ok(frame) => frame,
                    Err(_) => {
                        let _ = close_with_policy(&mut socket, "invalid gateway frame");
                        break;
                    }
                };
                match handle_frame(frame, &value, &mut session_stage, &state, session_id) {
                    Ok(()) => {}
                    Err(reason) => {
                        let _ = close_with_policy(&mut socket, reason);
                        break;
                    }
                }
                state_changed.notify_all();
            }
            Ok(Message::Close(_)) => break,
            Ok(Message::Ping(_)) | Ok(Message::Pong(_)) => {}
            Ok(_) => {
                let _ = close_with_policy(&mut socket, "binary frames are unsupported");
                break;
            }
            Err(tungstenite::Error::Io(error)) if error.kind() == ErrorKind::WouldBlock => {}
            Err(error) => {
                emit_gateway_error_log(session_id, "read", &error.to_string());
                break;
            }
        }

        match outbound_receiver.recv_timeout(Duration::from_millis(1)) {
            Ok(mut frame) => {
                let Some(next_sequence) = outbound_sequence.checked_add(1) else {
                    let _ = close_with_policy(&mut socket, "sequence exhausted");
                    break;
                };
                outbound_sequence = next_sequence;
                frame["seq"] = Value::from(next_sequence);
                emit_gateway_outbound_log(session_id, &frame);
                if let Err(error) = socket
                    .send(Message::Text(frame.to_string().into()))
                    .and_then(|_| socket.flush())
                {
                    emit_gateway_error_log(session_id, "send", &error.to_string());
                    break;
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    let mut shared = state.lock().expect("gateway state lock poisoned");
    if shared.active_session == Some(session_id) {
        shared.active_session = None;
        shared.active_sender = None;
        cancel_pending(
            &mut shared,
            TaskStatus::Cancelled,
            "adapter session disconnected",
        );
    }
    state_changed.notify_all();
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SessionStage {
    AwaitingHello,
    AwaitingManifest,
    Ready,
}

fn handle_frame(
    frame: ParsedFrame,
    value: &Value,
    stage: &mut SessionStage,
    state: &Arc<Mutex<SharedGatewayState>>,
    session_id: u64,
) -> Result<(), &'static str> {
    let kind = frame.kind;
    match (frame.kind, *stage) {
        (FrameKind::Hello, SessionStage::AwaitingHello) => {
            *stage = SessionStage::AwaitingManifest;
        }
        (FrameKind::CapabilityManifest, SessionStage::AwaitingManifest) => {
            let capabilities = frame
                .data
                .get("tools")
                .and_then(Value::as_array)
                .map(|tools| {
                    tools
                        .iter()
                        .filter(|tool| {
                            tool.get("status").and_then(Value::as_str) == Some("enabled")
                        })
                        .filter_map(|tool| tool.get("name").and_then(Value::as_str))
                        .map(str::to_string)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            state
                .lock()
                .expect("gateway state lock poisoned")
                .capabilities = capabilities;
            *stage = SessionStage::Ready;
        }
        (FrameKind::Event, SessionStage::Ready) => {
            let mut shared = state.lock().expect("gateway state lock poisoned");
            if shared.events.len() >= MAX_QUEUED_EVENTS {
                // Overflow closes the session with a policy close instead of
                // silently dropping player messages; the adapter reconnects
                // with backoff and restarts hello/manifest.
                return Err("event queue full");
            }
            shared.events.push_back(ChatEvent {
                event_id: value
                    .get("event_id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                sequence: frame.sequence,
                timestamp_ms: frame.timestamp_ms,
                source: value
                    .get("source")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                player_id: frame
                    .data
                    .get("player_id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                text: frame
                    .data
                    .get("text")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
            });
        }
        (FrameKind::ToolResult, SessionStage::Ready) => {
            let call_id = value
                .get("call_id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let status = serde_json::from_value::<TaskStatus>(
                value.get("status").cloned().unwrap_or(Value::Null),
            )
            .unwrap_or(TaskStatus::GameError);
            let result = ToolResult {
                schema_version: PROTOCOL_SCHEMA_VERSION,
                kind: "tool_result".into(),
                call_id,
                sequence: frame.sequence,
                timestamp_ms: frame.timestamp_ms,
                status,
                data: frame.data,
                error: value
                    .get("error")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                elapsed_ms: value.get("elapsed_ms").and_then(Value::as_u64).unwrap_or(0),
            };
            let sender = state
                .lock()
                .expect("gateway state lock poisoned")
                .pending
                .remove(&result.call_id);
            if let Some(sender) = sender {
                let _ = sender.send(result);
            }
        }
        (FrameKind::Heartbeat, SessionStage::Ready) | (FrameKind::Error, SessionStage::Ready) => {}
        _ => return Err("invalid session state"),
    }
    let capability_count = if kind == FrameKind::CapabilityManifest {
        Some(
            state
                .lock()
                .expect("gateway state lock poisoned")
                .capabilities
                .len(),
        )
    } else {
        None
    };
    emit_gateway_frame_log(session_id, kind, frame.sequence, capability_count);
    Ok(())
}
fn cancel_pending(state: &mut SharedGatewayState, status: TaskStatus, reason: &str) {
    for (call_id, sender) in state.pending.drain() {
        let _ = sender.send(local_result(&call_id, status, reason));
    }
}

fn local_result(call_id: &str, status: TaskStatus, reason: &str) -> ToolResult {
    ToolResult {
        schema_version: PROTOCOL_SCHEMA_VERSION,
        kind: "tool_result".into(),
        call_id: call_id.to_string(),
        sequence: 0,
        timestamp_ms: unix_ms(),
        status,
        data: Value::Null,
        error: Some(reason.to_string()),
        elapsed_ms: 0,
    }
}

fn json_frame(tool: &str, call_id: &str, args: &Value, timeout_ms: u64) -> Value {
    serde_json::json!({
        "schema_version": PROTOCOL_SCHEMA_VERSION,
        "type": "tool_call",
        "call_id": call_id,
        "seq": 0,
        "timestamp_ms": unix_ms(),
        "correlation_id": call_id,
        "tool": tool,
        "args": args,
        "timeout_ms": timeout_ms,
        "deadline_ms": unix_ms().saturating_add(timeout_ms)
    })
}

fn close_with_policy<S: std::io::Read + std::io::Write>(
    socket: &mut tungstenite::WebSocket<S>,
    reason: &'static str,
) -> Result<(), tungstenite::Error> {
    socket.close(Some(CloseFrame {
        code: CloseCode::Policy,
        reason: Utf8Bytes::from_static(reason),
    }))
}

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis().min(u64::MAX as u128) as u64)
        .unwrap_or_default()
}

fn unix_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_nanos().min(u64::MAX as u128) as u64)
        .unwrap_or_default()
}
