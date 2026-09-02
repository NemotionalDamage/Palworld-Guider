use crate::{ChatProvider, ChatRequest, ChatResponse, ProviderError, ToolRequest, ToolSpec};
use serde_json::{json, Value};
use std::time::Duration;

pub struct OpenAiCompatibleProvider {
    client: reqwest::blocking::Client,
    endpoint: String,
    api_key: String,
    model: String,
    disable_reasoning: bool,
}

impl OpenAiCompatibleProvider {
    pub fn new(
        endpoint: impl Into<String>,
        api_key: impl Into<String>,
        model: impl Into<String>,
        timeout: Duration,
        disable_reasoning: bool,
    ) -> Result<Self, ProviderError> {
        let client = reqwest::blocking::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|error| ProviderError::Network(error.to_string()))?;
        Ok(Self {
            client,
            endpoint: endpoint.into(),
            api_key: api_key.into(),
            model: model.into(),
            disable_reasoning,
        })
    }
}

pub fn chat_completions_endpoint(base_url: &str) -> String {
    let trimmed = base_url.trim_end_matches('/');
    if trimmed.ends_with("/chat/completions") {
        trimmed.to_string()
    } else {
        format!("{trimmed}/chat/completions")
    }
}

impl ChatProvider for OpenAiCompatibleProvider {
    fn name(&self) -> &'static str {
        "openai-compatible"
    }

    fn complete(&self, request: &ChatRequest) -> Result<ChatResponse, ProviderError> {
        let payload = build_openai_request(&self.model, request, self.disable_reasoning);
        let response = self
            .client
            .post(&self.endpoint)
            .bearer_auth(&self.api_key)
            .json(&payload)
            .send()
            .map_err(map_request_error)?;
        let status = response.status();
        let body = response
            .text()
            .map_err(|error| ProviderError::Network(error.to_string()))?;
        if !status.is_success() {
            return Err(ProviderError::Api(format!(
                "HTTP {}: {}",
                status.as_u16(),
                truncate(&body, 500)
            )));
        }
        parse_openai_response(&body)
    }
}

pub fn build_openai_request(model: &str, request: &ChatRequest, disable_reasoning: bool) -> Value {
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
    let mut payload = json!({
        "model": model,
        "messages": messages,
        "tools": request.tools.iter().map(function_json).collect::<Vec<_>>(),
        "tool_choice": "auto",
    });
    if disable_reasoning {
        payload["thinking"] = json!({"type": "disabled"});
    }
    payload
}

fn function_json(tool: &ToolSpec) -> Value {
    json!({
        "type": "function",
        "function": {
            "name": tool.name,
            "description": tool.description,
            "parameters": tool.parameters_schema,
        },
    })
}

pub fn parse_openai_response(payload: &str) -> Result<ChatResponse, ProviderError> {
    let root: Value = serde_json::from_str(payload)
        .map_err(|error| ProviderError::InvalidResponse(error.to_string()))?;
    if let Some(error) = root.get("error") {
        let message = error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("unknown API error");
        return Err(ProviderError::Api(message.to_string()));
    }
    let choice = root
        .get("choices")
        .and_then(|choices| choices.get(0))
        .ok_or_else(|| ProviderError::InvalidResponse("missing choices[0]".to_string()))?;
    if choice.get("finish_reason").and_then(Value::as_str) == Some("length") {
        return Err(ProviderError::Api(
            "model output was truncated (finish_reason=length); retry the question or ask for a shorter answer"
                .to_string(),
        ));
    }
    let message = choice
        .get("message")
        .ok_or_else(|| ProviderError::InvalidResponse("missing choices[0].message".to_string()))?;
    let content = message
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let mut tool_requests = Vec::new();
    if let Some(calls) = message.get("tool_calls").and_then(Value::as_array) {
        for call in calls {
            tool_requests.push(parse_tool_call(call)?);
        }
    }
    Ok(ChatResponse {
        content,
        tool_requests,
    })
}

fn parse_tool_call(call: &Value) -> Result<ToolRequest, ProviderError> {
    let id = call
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
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
    let arguments = parse_arguments(function.get("arguments"))?;
    Ok(ToolRequest {
        id,
        name,
        arguments,
    })
}

fn parse_arguments(arguments: Option<&Value>) -> Result<Value, ProviderError> {
    match arguments {
        None => Ok(json!({})),
        Some(Value::String(value)) if value.trim().is_empty() => Ok(json!({})),
        Some(Value::String(value)) => serde_json::from_str(value).map_err(|error| {
            ProviderError::InvalidResponse(format!("invalid tool arguments JSON: {error}"))
        }),
        Some(value @ Value::Object(_)) => Ok(value.clone()),
        Some(_) => Err(ProviderError::InvalidResponse(
            "tool arguments must be a JSON object".to_string(),
        )),
    }
}

fn map_request_error(error: reqwest::Error) -> ProviderError {
    if error.is_timeout() {
        ProviderError::Timeout(error.to_string())
    } else {
        ProviderError::Network(error.to_string())
    }
}

fn truncate(text: &str, maximum: usize) -> String {
    text.chars().take(maximum).collect()
}
