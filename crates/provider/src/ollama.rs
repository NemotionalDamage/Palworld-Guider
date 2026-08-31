use crate::{ChatProvider, ChatRequest, ChatResponse, ProviderError, ToolRequest};
use serde_json::{json, Value};
use std::time::Duration;

pub struct OllamaProvider {
    client: reqwest::blocking::Client,
    endpoint: String,
    model: String,
}

impl OllamaProvider {
    pub fn new(
        base_url: impl AsRef<str>,
        model: impl Into<String>,
        timeout: Duration,
    ) -> Result<Self, ProviderError> {
        let client = reqwest::blocking::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|error| ProviderError::Network(error.to_string()))?;
        let endpoint = format!("{}/api/chat", base_url.as_ref().trim_end_matches('/'));
        Ok(Self {
            client,
            endpoint,
            model: model.into(),
        })
    }
}

impl ChatProvider for OllamaProvider {
    fn name(&self) -> &'static str {
        "ollama"
    }

    fn complete(&self, request: &ChatRequest) -> Result<ChatResponse, ProviderError> {
        let payload = build_ollama_request(&self.model, request);
        let response = self
            .client
            .post(&self.endpoint)
            .json(&payload)
            .send()
            .map_err(|error| {
                if error.is_timeout() {
                    ProviderError::Timeout(error.to_string())
                } else {
                    ProviderError::Network(error.to_string())
                }
            })?;
        let status = response.status();
        let body = response
            .text()
            .map_err(|error| ProviderError::Network(error.to_string()))?;
        if !status.is_success() {
            return Err(ProviderError::Api(format!(
                "HTTP {}: {}",
                status.as_u16(),
                body.chars().take(500).collect::<String>()
            )));
        }
        parse_ollama_response(&body)
    }
}

pub fn build_ollama_request(model: &str, request: &ChatRequest) -> Value {
    let mut messages = vec![json!({
        "role": "system",
        "content": request.system,
    })];
    messages.extend(request.messages.iter().map(|message| {
        json!({
            "role": message.role,
            "content": message.content,
        })
    }));
    json!({
        "model": model,
        "messages": messages,
        "tools": request.tools.iter().map(|tool| {
            json!({
                "type": "function",
                "function": {
                    "name": tool.name,
                    "description": tool.description,
                    "parameters": tool.parameters_schema,
                },
            })
        }).collect::<Vec<_>>(),
        "stream": false,
    })
}

pub fn parse_ollama_response(payload: &str) -> Result<ChatResponse, ProviderError> {
    let root: Value = serde_json::from_str(payload)
        .map_err(|error| ProviderError::InvalidResponse(error.to_string()))?;
    if let Some(error) = root.get("error") {
        let message = error.as_str().unwrap_or("unknown API error");
        return Err(ProviderError::Api(message.to_string()));
    }
    let message = root
        .get("message")
        .ok_or_else(|| ProviderError::InvalidResponse("missing message".to_string()))?;
    let content = message
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let mut tool_requests = Vec::new();
    if let Some(calls) = message.get("tool_calls").and_then(Value::as_array) {
        for call in calls {
            let function = call.get("function").ok_or_else(|| {
                ProviderError::InvalidResponse("tool call is missing function".to_string())
            })?;
            let name = function
                .get("name")
                .and_then(Value::as_str)
                .filter(|name| !name.trim().is_empty())
                .ok_or_else(|| {
                    ProviderError::InvalidResponse("tool call is missing function name".to_string())
                })?
                .to_string();
            let arguments = match function.get("arguments") {
                None => json!({}),
                Some(Value::String(value)) if value.trim().is_empty() => json!({}),
                Some(Value::String(value)) => serde_json::from_str(value).map_err(|error| {
                    ProviderError::InvalidResponse(format!("invalid tool arguments JSON: {error}"))
                })?,
                Some(value @ Value::Object(_)) => value.clone(),
                Some(_) => {
                    return Err(ProviderError::InvalidResponse(
                        "tool arguments must be a JSON object".to_string(),
                    ))
                }
            };
            tool_requests.push(ToolRequest {
                id: String::new(),
                name,
                arguments,
            });
        }
    }
    Ok(ChatResponse {
        content,
        tool_requests,
    })
}
