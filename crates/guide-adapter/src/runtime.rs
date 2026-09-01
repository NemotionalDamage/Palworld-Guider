//! Read-only adapter runtime over the authenticated loopback game gateway.
//!
//! The runtime owns the `WebSocketGateway`, exposes the live adapter
//! capabilities (intersected with a compile-time model-visible allowlist),
//! and registers itself as a `RuntimeToolSource` so the guide agent can call
//! read-only in-game tools through the typed tool registry. Failures always
//! close: when no authenticated adapter session is active, every tool call
//! returns an explicit error envelope instead of fabricating state.

use game_gateway::{ChatEvent, TaskStatus, ToolResult, WebSocketGateway};
use guide_tools::{RuntimeToolResult, RuntimeToolSource, ToolDefinition, ToolStatus};
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use thiserror::Error;

/// Read-only adapter tools the language model is allowed to see and call.
pub const MODEL_VISIBLE_RUNTIME_TOOLS: &[&str] = &["get_player_status", "get_active_pal_status"];

/// Internal adapter tools callable by the runtime itself, never by the model.
pub const INTERNAL_RUNTIME_TOOLS: &[&str] = &["send_chat_message"];

const NO_ACTIVE_SESSION: &str = "no authenticated adapter session is active";
const NO_ARGUMENT_SCHEMA: &str = r#"{"type":"object","properties":{},"required":[]}"#;

/// Errors returned by the adapter runtime.
#[derive(Debug, Error)]
pub enum AdapterError {
    #[error("adapter gateway error: {0}")]
    Gateway(#[from] game_gateway::GatewayError),
    #[error("adapter tool call failed: {0}")]
    ToolCall(String),
    #[error("adapter capability is not advertised: {0}")]
    NotAdvertised(String),
}

/// Cloneable handle to the live adapter session and its loopback gateway.
#[derive(Clone)]
pub struct GameAdapterRuntime {
    gateway: Arc<Mutex<WebSocketGateway>>,
}

impl GameAdapterRuntime {
    pub fn new(gateway: WebSocketGateway) -> Self {
        Self {
            gateway: Arc::new(Mutex::new(gateway)),
        }
    }

    pub fn endpoint(&self) -> SocketAddr {
        self.gateway
            .lock()
            .expect("gateway lock poisoned")
            .local_addr()
    }

    /// All capabilities advertised by the authenticated adapter session.
    pub fn capabilities(&self) -> Vec<String> {
        self.gateway
            .lock()
            .expect("gateway lock poisoned")
            .capabilities()
    }

    /// The subset of live capabilities the language model may call. Empty
    /// until a validated capability manifest arrives from the adapter.
    pub fn model_capabilities(&self) -> Vec<String> {
        self.capabilities()
            .into_iter()
            .filter(|capability| MODEL_VISIBLE_RUNTIME_TOOLS.contains(&capability.as_str()))
            .collect()
    }

    /// Wait up to `timeout` for the next player chat event.
    pub fn next_event_timeout(&self, timeout: Duration) -> Result<Option<ChatEvent>, AdapterError> {
        Ok(self
            .gateway
            .lock()
            .expect("gateway lock poisoned")
            .next_event_timeout(timeout)?)
    }

    /// Send one chat message through the internal `send_chat_message` tool.
    pub fn send_chat(&self, message: &str) -> Result<(), AdapterError> {
        let result = self.call_adapter("send_chat_message", &json!({ "message": message }))?;
        if result.status == TaskStatus::Ok {
            Ok(())
        } else {
            Err(AdapterError::ToolCall(result.error.unwrap_or_else(|| {
                format!("adapter tool call failed with {:?}", result.status)
            })))
        }
    }

    fn call_adapter(&self, name: &str, arguments: &Value) -> Result<ToolResult, AdapterError> {
        let live = self.capabilities();
        // An empty live set can mean the session has not finished its
        // manifest yet or no session is active; let the gateway report the
        // authoritative no-session failure in that case.
        if !live.is_empty() && !live.iter().any(|capability| capability == name) {
            return Err(AdapterError::NotAdvertised(name.to_string()));
        }
        self.gateway
            .lock()
            .expect("gateway lock poisoned")
            .call_tool(name, arguments.clone())
            .map_err(AdapterError::from)
    }
}

impl RuntimeToolSource for GameAdapterRuntime {
    fn definitions(&self) -> Vec<ToolDefinition> {
        self.model_capabilities()
            .into_iter()
            .map(|name| ToolDefinition {
                name: name.clone(),
                description: runtime_tool_description(&name),
                parameters_schema: serde_json::from_str(NO_ARGUMENT_SCHEMA)
                    .expect("static no-argument schema is valid"),
            })
            .collect()
    }

    fn dispatch(&self, name: &str, arguments: &Value) -> RuntimeToolResult {
        match self.call_adapter(name, arguments) {
            Ok(result) => map_tool_result(result),
            Err(error) => RuntimeToolResult {
                status: ToolStatus::Error,
                data: None,
                uncertainty: Vec::new(),
                errors: vec![error.to_string()],
            },
        }
    }
}

fn runtime_tool_description(name: &str) -> String {
    match name {
        "get_player_status" => {
            "Return the observed player status (level, position, health) from the authenticated in-game adapter session. The answer is empty or Unknown when no observed state is available.".to_string()
        }
        "get_active_pal_status" => {
            "Return the observed active Pal status from the authenticated in-game adapter session. The answer is empty or Unknown when no observed state is available.".to_string()
        }
        _ => format!("Read-only adapter capability \"{name}\"."),
    }
}

/// Map a gateway tool result into the typed runtime envelope.
///
/// - `Ok` stays `Ok` with its data.
/// - `Unavailable`, `NotFound`, and `Timeout` become `Unknown` with the
///   adapter-provided message in uncertainty.
/// - Every other failure becomes `Error` with the safe error text preserved.
/// - A missing adapter session fails closed as `Error` with the exact
///   no-session message.
fn map_tool_result(result: ToolResult) -> RuntimeToolResult {
    let no_session = result.status == TaskStatus::Unavailable
        && result.error.as_deref() == Some(NO_ACTIVE_SESSION);
    if no_session {
        return RuntimeToolResult {
            status: ToolStatus::Error,
            data: None,
            uncertainty: Vec::new(),
            errors: vec![NO_ACTIVE_SESSION.to_string()],
        };
    }

    let mut uncertainty = Vec::new();
    let mut errors = Vec::new();
    if let Some(message) = &result.error {
        if matches!(
            result.status,
            TaskStatus::Ok | TaskStatus::Unavailable | TaskStatus::NotFound | TaskStatus::Timeout
        ) {
            uncertainty.push(message.clone());
        } else {
            errors.push(message.clone());
        }
    }
    let status = match result.status {
        TaskStatus::Ok => ToolStatus::Ok,
        TaskStatus::Unavailable | TaskStatus::NotFound | TaskStatus::Timeout => ToolStatus::Unknown,
        _ => ToolStatus::Error,
    };
    RuntimeToolResult {
        status,
        data: (!result.data.is_null()).then_some(result.data),
        uncertainty,
        errors,
    }
}
