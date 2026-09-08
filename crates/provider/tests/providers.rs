use provider::{
    build_ollama_request, build_openai_request, chat_completions_endpoint, parse_ollama_response,
    parse_openai_response, ChatMessage, ChatProvider, ChatRequest, ChatResponse, MockProvider,
    ProviderError, ToolSpec,
};
use serde_json::json;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;
use std::time::Duration;

fn sample_request() -> ChatRequest {
    ChatRequest {
        system: "You are a grounded test provider.".to_string(),
        messages: vec![ChatMessage {
            role: "user".to_string(),
            content: "What is Wood?".to_string(),
        }],
        tools: vec![ToolSpec {
            name: "get_item".to_string(),
            description: "Look up an item.".to_string(),
            parameters_schema: json!({"type": "object"}),
        }],
    }
}

#[test]
fn openai_parses_content_and_tool_calls() {
    let payload = r#"{
        "choices": [{
            "message": {
                "role": "assistant",
                "content": "Checking Wood.",
                "tool_calls": [{
                    "id": "call_1",
                    "function": {
                        "name": "get_item",
                        "arguments": "{\"query\":\"Wood\"}"
                    }
                }]
            }
        }]
    }"#;
    let response = parse_openai_response(payload).expect("valid OpenAI payload");
    assert_eq!(response.content, "Checking Wood.");
    assert_eq!(response.tool_requests.len(), 1);
    assert_eq!(response.tool_requests[0].id, "call_1");
    assert_eq!(response.tool_requests[0].name, "get_item");
    assert_eq!(response.tool_requests[0].arguments["query"], "Wood");
}

#[test]
fn openai_rejects_malformed_payloads() {
    assert!(matches!(
        parse_openai_response("not json"),
        Err(ProviderError::InvalidResponse(_))
    ));
    assert!(matches!(
        parse_openai_response("{}"),
        Err(ProviderError::InvalidResponse(_))
    ));
    assert!(matches!(
        parse_openai_response(r#"{"choices":[]}"#),
        Err(ProviderError::InvalidResponse(_))
    ));
}

#[test]
fn openai_reports_api_errors_without_secrets() {
    let error = parse_openai_response(r#"{"error":{"message":"invalid api key"}}"#)
        .expect_err("error payload is rejected");
    match error {
        ProviderError::Api(message) => assert_eq!(message, "invalid api key"),
        other => panic!("unexpected error: {other:?}"),
    }
    let displayed = ProviderError::Api("invalid api key".to_string()).to_string();
    assert!(displayed.contains("api"));
    assert!(!displayed.contains("sk-secret"));
}

#[test]
fn ollama_parses_content_and_tool_calls() {
    let payload = r#"{
        "message": {
            "role": "assistant",
            "content": "Checking Wood.",
            "tool_calls": [{
                "function": {
                    "name": "get_item",
                    "arguments": {"query": "Wood"}
                }
            }]
        }
    }"#;
    let response = parse_ollama_response(payload).expect("valid Ollama payload");
    assert_eq!(response.content, "Checking Wood.");
    assert_eq!(response.tool_requests.len(), 1);
    assert_eq!(response.tool_requests[0].name, "get_item");
    assert_eq!(response.tool_requests[0].arguments["query"], "Wood");
}

#[test]
fn ollama_reports_error_payloads() {
    let error = parse_ollama_response(r#"{"error":"model not found"}"#)
        .expect_err("error payload is rejected");
    match error {
        ProviderError::Api(message) => assert!(message.contains("model not found")),
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn mock_replays_script_then_reports_exhaustion() {
    let provider = MockProvider::scripted(vec![
        ChatResponse::text("first"),
        ChatResponse::text("second"),
    ]);
    let request = sample_request();
    assert_eq!(
        provider.complete(&request).expect("first response").content,
        "first"
    );
    assert_eq!(
        provider
            .complete(&request)
            .expect("second response")
            .content,
        "second"
    );
    let error = provider
        .complete(&request)
        .expect_err("script is exhausted");
    assert!(
        matches!(error, ProviderError::InvalidResponse(message) if message.contains("exhausted"))
    );
    let calls = provider.calls();
    assert_eq!(calls.len(), 3);
    assert_eq!(calls[0].messages[0].content, "What is Wood?");
    assert_eq!(calls[0].tools[0].name, "get_item");
}

#[test]
fn openai_request_includes_system_tools_and_auto_choice() {
    let request = build_openai_request("test-model", &sample_request(), false);
    assert_eq!(request["model"], "test-model");
    assert_eq!(request["messages"][0]["role"], "system");
    assert_eq!(
        request["messages"][0]["content"],
        "You are a grounded test provider."
    );
    assert_eq!(request["messages"][1]["role"], "user");
    assert_eq!(request["tools"][0]["type"], "function");
    assert_eq!(request["tools"][0]["function"]["name"], "get_item");
    assert_eq!(request["tool_choice"], "auto");
    assert!(request.get("thinking").is_none());
}

#[test]
fn openai_request_disables_reasoning_when_configured() {
    let request = build_openai_request("test-model", &sample_request(), true);
    assert_eq!(request["thinking"]["type"], "disabled");
}

#[test]
fn base_urls_gain_the_chat_completions_path() {
    assert_eq!(
        chat_completions_endpoint("https://example.test/api/v1"),
        "https://example.test/api/v1/chat/completions"
    );
    assert_eq!(
        chat_completions_endpoint("https://example.test/api/v1/"),
        "https://example.test/api/v1/chat/completions"
    );
}

#[test]
fn complete_endpoints_are_not_duplicated() {
    assert_eq!(
        chat_completions_endpoint("https://example.test/v1/chat/completions"),
        "https://example.test/v1/chat/completions"
    );
}

#[test]
fn openai_response_truncation_is_an_explicit_api_error() {
    let payload = r#"{"choices":[{"finish_reason":"length","message":{"role":"assistant","content":"half an answer"}}]}"#;
    let error = parse_openai_response(payload).expect_err("truncation must be rejected");
    assert!(
        matches!(&error, ProviderError::Api(message) if message.contains("truncated")),
        "unexpected error: {error:?}"
    );
}

#[test]
fn ollama_request_disables_streaming_and_publishes_tools() {
    let request = build_ollama_request("test-model", &sample_request());
    assert_eq!(request["model"], "test-model");
    assert_eq!(request["stream"], false);
    assert_eq!(request["messages"][0]["role"], "system");
    assert_eq!(request["tools"][0]["type"], "function");
    assert_eq!(request["tools"][0]["function"]["name"], "get_item");
}

#[test]
fn provider_error_display_is_clear() {
    assert!(ProviderError::Timeout("30s".to_string())
        .to_string()
        .contains("timed out"));
    assert!(ProviderError::Network("offline".to_string())
        .to_string()
        .contains("network"));
    assert!(ProviderError::Api("bad".to_string())
        .to_string()
        .contains("api"));
    assert!(ProviderError::InvalidResponse("bad".to_string())
        .to_string()
        .contains("invalid provider response"));
}

#[test]
fn openai_response_read_timeouts_are_reported_as_timeouts() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("test listener");
    let address = listener.local_addr().expect("test address");
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("test connection");
        let mut request = [0_u8; 1024];
        let _ = stream.read(&mut request);
        let _ = stream.write_all(
            b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 128\r\n\r\npartial",
        );
        let _ = stream.flush();
        thread::sleep(Duration::from_millis(500));
    });
    let endpoint = format!("http://{address}/chat/completions");
    let provider = provider::OpenAiCompatibleProvider::new(
        endpoint,
        "test-key",
        "test-model",
        Duration::from_millis(100),
        false,
    )
    .expect("test provider");
    let error = provider
        .complete(&sample_request())
        .expect_err("read must time out");
    server.join().expect("test server");
    assert!(
        matches!(error, ProviderError::Timeout(_)),
        "unexpected error: {error:?}"
    );
}
