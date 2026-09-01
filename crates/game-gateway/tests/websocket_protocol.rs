use game_gateway::{
    FrameKind, FrameValidator, TaskStatus, WebSocketGateway, WebSocketGatewayConfig,
    PROTOCOL_SCHEMA_VERSION,
};
use serde_json::{json, Value};
use std::net::TcpStream;
use std::time::Duration;
use tungstenite::client::IntoClientRequest;
use tungstenite::http::header::AUTHORIZATION;
use tungstenite::http::HeaderValue;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{connect, Message};

type ClientSocket = tungstenite::WebSocket<MaybeTlsStream<TcpStream>>;

const TOKEN: &str = "guider-protocol-token";

fn validator() -> FrameValidator {
    FrameValidator::new(256)
}

fn frame(kind: &str, sequence: u64) -> Value {
    json!({
        "schema_version": PROTOCOL_SCHEMA_VERSION,
        "type": kind,
        "seq": sequence,
        "timestamp_ms": sequence,
    })
}

#[test]
fn accepts_every_schema_two_frame_kind() {
    let frames = [
        frame("hello", 1),
        json!({
            "schema_version": PROTOCOL_SCHEMA_VERSION,
            "type": "capability_manifest",
            "seq": 2,
            "timestamp_ms": 2,
            "data": {"tools": []}
        }),
        json!({
            "schema_version": PROTOCOL_SCHEMA_VERSION,
            "type": "event",
            "seq": 3,
            "timestamp_ms": 3,
            "event": "chat_message",
            "event_id": "event-3",
            "source": "client",
            "data": {"player_id": "player", "text": "hello"}
        }),
        json!({
            "schema_version": PROTOCOL_SCHEMA_VERSION,
            "type": "tool_call",
            "seq": 4,
            "timestamp_ms": 4,
            "call_id": "call-4",
            "correlation_id": "call-4",
            "tool": "ping",
            "args": {},
            "timeout_ms": 100,
            "deadline_ms": 100
        }),
        json!({
            "schema_version": PROTOCOL_SCHEMA_VERSION,
            "type": "tool_result",
            "seq": 5,
            "timestamp_ms": 5,
            "call_id": "call-4",
            "correlation_id": "call-4",
            "status": "ok",
            "data": {},
            "error": null,
            "elapsed_ms": 1
        }),
        frame("heartbeat", 6),
        json!({
            "schema_version": PROTOCOL_SCHEMA_VERSION,
            "type": "error",
            "seq": 7,
            "timestamp_ms": 7,
            "error": {"code": "invalid_frame", "message": "invalid"}
        }),
    ];

    let mut validator = FrameValidator::new(1024);
    for expected in [
        FrameKind::Hello,
        FrameKind::CapabilityManifest,
        FrameKind::Event,
        FrameKind::ToolCall,
        FrameKind::ToolResult,
        FrameKind::Heartbeat,
        FrameKind::Error,
    ] {
        let parsed = validator
            .validate_value(&frames[parsed_index(expected)])
            .unwrap_or_else(|error| panic!("{expected:?}: {error}"));
        assert_eq!(parsed.kind, expected);
    }
}

fn parsed_index(kind: FrameKind) -> usize {
    match kind {
        FrameKind::Hello => 0,
        FrameKind::CapabilityManifest => 1,
        FrameKind::Event => 2,
        FrameKind::ToolCall => 3,
        FrameKind::ToolResult => 4,
        FrameKind::Heartbeat => 5,
        FrameKind::Error => 6,
    }
}

#[test]
fn accepts_only_strictly_increasing_sequences() {
    let mut validator = validator();
    validator.validate_value(&frame("heartbeat", 4)).unwrap();
    validator.validate_value(&frame("heartbeat", 5)).unwrap();
    let duplicate = validator.validate_value(&frame("heartbeat", 5));
    assert_eq!(
        duplicate.unwrap_err().to_string(),
        "duplicate frame sequence 5"
    );
    let stale = validator.validate_value(&frame("heartbeat", 4));
    assert_eq!(stale.unwrap_err().to_string(), "stale frame sequence 4");
}

#[test]
fn rejects_unknown_schema_and_type() {
    let mut validator = validator();
    let mut unknown_schema = frame("heartbeat", 1);
    unknown_schema["schema_version"] = json!(99);
    assert_eq!(
        validator
            .validate_value(&unknown_schema)
            .unwrap_err()
            .to_string(),
        "unsupported schema version 99"
    );

    let mut unknown_type = frame("movement", 2);
    unknown_type["type"] = json!("movement");
    assert_eq!(
        validator
            .validate_value(&unknown_type)
            .unwrap_err()
            .to_string(),
        "unsupported frame type movement"
    );
}

#[test]
fn rejects_missing_common_fields() {
    let mut missing_type = frame("heartbeat", 1);
    missing_type.as_object_mut().unwrap().remove("type");
    assert!(validator()
        .validate_value(&missing_type)
        .unwrap_err()
        .to_string()
        .contains("missing field `type`"));

    let mut missing_sequence = frame("heartbeat", 1);
    missing_sequence.as_object_mut().unwrap().remove("seq");
    assert!(validator()
        .validate_value(&missing_sequence)
        .unwrap_err()
        .to_string()
        .contains("missing field `seq`"));
}

#[test]
fn rejects_payloads_over_configured_limit() {
    let mut large = frame("heartbeat", 1);
    large["data"] = Value::String("x".repeat(256));
    let error = validator().validate_value(&large).unwrap_err();
    assert_eq!(error.to_string(), "frame payload exceeds 256 bytes");
}

#[test]
fn validates_utf8_text_bytes() {
    let bytes = serde_json::to_vec(&frame("heartbeat", 1)).unwrap();
    let parsed = validator().validate_bytes(&bytes).unwrap();
    assert_eq!(parsed.kind, FrameKind::Heartbeat);
    assert_eq!(parsed.sequence, 1);

    let invalid = b"{";
    assert!(validator().validate_bytes(invalid).is_err());
}
#[test]
fn malformed_common_frames_are_rejected() {
    // Missing common fields, unknown schema/type, duplicate/stale sequence,
    // binary input, malformed JSON, and oversized UTF-8 text close safely.
    let mut validator = validator();

    let mut missing_type = frame("heartbeat", 1);
    missing_type.as_object_mut().unwrap().remove("type");
    assert!(validator.validate_value(&missing_type).is_err());

    let mut missing_sequence = frame("heartbeat", 1);
    missing_sequence.as_object_mut().unwrap().remove("seq");
    assert!(validator.validate_value(&missing_sequence).is_err());

    let mut unknown_schema = frame("heartbeat", 1);
    unknown_schema["schema_version"] = json!(99);
    assert!(validator.validate_value(&unknown_schema).is_err());

    let mut unknown_type = frame("movement", 2);
    unknown_type["type"] = json!("movement");
    assert!(validator.validate_value(&unknown_type).is_err());

    validator.validate_value(&frame("heartbeat", 3)).unwrap();
    let duplicate = validator.validate_value(&frame("heartbeat", 3));
    assert_eq!(
        duplicate.unwrap_err().to_string(),
        "duplicate frame sequence 3"
    );
    let stale = validator.validate_value(&frame("heartbeat", 2));
    assert_eq!(stale.unwrap_err().to_string(), "stale frame sequence 2");

    assert!(validator.validate_bytes(b"{").is_err());

    let mut oversized = frame("heartbeat", 4);
    oversized["data"] = Value::String("x".repeat(257));
    assert!(validator.validate_value(&oversized).is_err());
}

fn gateway() -> WebSocketGateway {
    WebSocketGateway::bind(
        "127.0.0.1:0",
        TOKEN,
        WebSocketGatewayConfig {
            max_payload_bytes: 1024,
            call_timeout: Duration::from_millis(80),
        },
    )
    .unwrap()
}

fn url(address: std::net::SocketAddr) -> String {
    format!("ws://{address}")
}

fn connect_authenticated(address: std::net::SocketAddr) -> ClientSocket {
    let mut request = url(address).into_client_request().unwrap();
    request.headers_mut().insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {TOKEN}")).unwrap(),
    );
    connect(request).unwrap().0
}

fn send_json(socket: &mut ClientSocket, frame: Value) {
    socket
        .send(Message::Text(frame.to_string().into()))
        .unwrap();
}

fn hello(sequence: u64) -> Value {
    json!({
        "schema_version": 2,
        "type": "hello",
        "seq": sequence,
        "timestamp_ms": sequence
    })
}

fn manifest(sequence: u64, enabled: &[&str]) -> Value {
    json!({
        "schema_version": 2,
        "type": "capability_manifest",
        "seq": sequence,
        "timestamp_ms": sequence,
        "data": {"tools": enabled.iter().map(|name| json!({
            "name": name,
            "mutation": name == &"send_chat_message",
            "evidence": "A",
            "status": "enabled"
        })).collect::<Vec<_>>()}
    })
}

fn event(sequence: u64, text: &str) -> Value {
    json!({
        "schema_version": 2,
        "type": "event",
        "event": "chat_message",
        "event_id": format!("event-{sequence}"),
        "seq": sequence,
        "timestamp_ms": sequence,
        "source": "client",
        "data": {"player_id": "player", "text": text}
    })
}

fn result(sequence: u64, call_id: &str, status: &str) -> Value {
    json!({
        "schema_version": 2,
        "type": "tool_result",
        "call_id": call_id,
        "correlation_id": call_id,
        "seq": sequence,
        "timestamp_ms": sequence,
        "status": status,
        "data": {"answered": true},
        "error": null,
        "elapsed_ms": 1
    })
}

fn read_json(socket: &mut ClientSocket) -> Value {
    match socket.read().unwrap() {
        Message::Text(text) => serde_json::from_str(&text).unwrap(),
        Message::Close(frame) => panic!("unexpected close: {frame:?}"),
        message => panic!("unexpected message: {message:?}"),
    }
}

#[test]
fn hello_manifest_event_and_tool_result_close_one_session() {
    // Bearer auth, hello seq 1, manifest seq 2, event seq 3, one correlated
    // tool call/result; capabilities appear only after validated manifest.
    let mut gateway = gateway();
    let mut client = connect_authenticated(gateway.local_addr());
    assert!(
        gateway.capabilities().is_empty(),
        "capabilities must stay empty until the manifest is validated"
    );

    send_json(&mut client, hello(1));
    send_json(&mut client, manifest(2, &["ping"]));
    send_json(&mut client, event(3, "where am I?"));

    let received = gateway
        .next_event_timeout(Duration::from_millis(500))
        .unwrap()
        .expect("event");
    assert_eq!(received.player_id, "player");
    assert_eq!(received.text, "where am I?");
    assert_eq!(
        gateway.capabilities(),
        vec!["ping".to_string()],
        "capabilities appear only after a validated manifest"
    );

    let worker = std::thread::spawn(move || gateway.call_tool("ping", json!({})).unwrap());
    let call = read_json(&mut client);
    assert_eq!(call["type"], "tool_call");
    assert_eq!(
        call["seq"], 1,
        "outbound sequence must start at 1 for the session"
    );
    assert_eq!(call["tool"], "ping");
    send_json(
        &mut client,
        result(4, call["call_id"].as_str().unwrap(), "ok"),
    );
    let result = worker.join().unwrap();
    assert_eq!(result.status, TaskStatus::Ok);
    assert_eq!(result.data, json!({"answered": true}));
}

#[test]
fn missing_or_wrong_bearer_token_is_rejected_before_frames() {
    // Absent and wrong bearer values fail during HTTP upgrade.
    let gateway = gateway();
    let address = gateway.local_addr();

    let absent = connect(url(address)).unwrap_err();
    assert_eq!(absent.to_string(), "HTTP error: 401 Unauthorized");

    let mut wrong_request = url(address).into_client_request().unwrap();
    wrong_request.headers_mut().insert(
        AUTHORIZATION,
        HeaderValue::from_str("Bearer wrong-token").unwrap(),
    );
    let wrong = connect(wrong_request).unwrap_err();
    assert_eq!(wrong.to_string(), "HTTP error: 401 Unauthorized");
}
