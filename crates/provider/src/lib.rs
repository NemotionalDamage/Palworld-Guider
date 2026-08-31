//! Transport-neutral chat provider contract and adapters.

pub mod mock;
pub mod ollama;
pub mod openai;

pub use mock::MockProvider;
pub use ollama::{build_ollama_request, parse_ollama_response, OllamaProvider};
pub use openai::{build_openai_request, parse_openai_response, OpenAiCompatibleProvider};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub parameters_schema: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

impl ChatMessage {
    pub fn new(role: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: role.into(),
            content: content.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatRequest {
    pub system: String,
    pub messages: Vec<ChatMessage>,
    pub tools: Vec<ToolSpec>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolRequest {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatResponse {
    pub content: String,
    pub tool_requests: Vec<ToolRequest>,
}

impl ChatResponse {
    pub fn text(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            tool_requests: Vec::new(),
        }
    }

    pub fn tool(id: impl Into<String>, name: impl Into<String>, arguments: Value) -> Self {
        Self {
            content: String::new(),
            tool_requests: vec![ToolRequest {
                id: id.into(),
                name: name.into(),
                arguments,
            }],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderError {
    Timeout(String),
    Network(String),
    Api(String),
    InvalidResponse(String),
}

impl fmt::Display for ProviderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Timeout(detail) => write!(formatter, "provider request timed out: {detail}"),
            Self::Network(detail) => write!(formatter, "provider network error: {detail}"),
            Self::Api(detail) => write!(formatter, "provider api error: {detail}"),
            Self::InvalidResponse(detail) => {
                write!(formatter, "invalid provider response: {detail}")
            }
        }
    }
}

impl std::error::Error for ProviderError {}

pub trait ChatProvider: Send + Sync {
    fn name(&self) -> &'static str;
    fn complete(&self, request: &ChatRequest) -> Result<ChatResponse, ProviderError>;
}
