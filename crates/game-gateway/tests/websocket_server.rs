use game_gateway::{TaskStatus, WebSocketGateway, WebSocketGatewayConfig};
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;
use tungstenite::client::IntoClientRequest;
use tungstenite::http::header::AUTHORIZATION;
use tungstenite::http::{HeaderValue, Uri};
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{connect, Message};

type ClientSocket = tungstenite::WebSocket<MaybeTlsStream<TcpStream>>;

const TOKEN: &str = "guider-server-token";

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
    let mut socket = connect(request).unwrap().0;
    if let MaybeTlsStream::Plain(stream) = socket.get_mut() {
        // Disable Nagle so burst test frames are not stalled by delayed ACKs.
        stream.set_nodelay(true).unwrap();
    }
    socket
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
fn websocket_upgrade_requires_bearer_token() {
    let gateway = gateway();
    let error = connect(url(gateway.local_addr())).unwrap_err();
    assert_eq!(error.to_string(), "HTTP error: 401 Unauthorized");
}

#[test]
fn authenticated_manifest_and_event_are_accepted() {
    let mut gateway = gateway();
    let mut client = connect_authenticated(gateway.local_addr());
    send_json(&mut client, hello(1));
    send_json(&mut client, manifest(2, &["ping", "get_player_status"]));
    send_json(&mut client, event(3, "where am I?"));

    let received = gateway
        .next_event_timeout(Duration::from_millis(500))
        .unwrap()
        .expect("event");
    assert_eq!(received.player_id, "player");
    assert_eq!(received.text, "where am I?");
    assert_eq!(
        gateway.capabilities(),
        vec!["ping".to_string(), "get_player_status".to_string()]
    );
}

#[test]
fn matching_tool_result_is_returned() {
    let gateway = gateway();
    let mut client = connect_authenticated(gateway.local_addr());
    send_json(&mut client, hello(1));
    send_json(&mut client, manifest(2, &["ping"]));

    let worker = std::thread::spawn(move || gateway.call_tool("ping", json!({})).unwrap());
    let call = read_json(&mut client);
    assert_eq!(call["type"], "tool_call");
    assert_eq!(
        call["seq"], 1,
        "call-ID generation must not consume protocol sequence"
    );
    assert_eq!(call["tool"], "ping");
    send_json(
        &mut client,
        result(3, call["call_id"].as_str().unwrap(), "ok"),
    );
    let result = worker.join().unwrap();
    assert_eq!(result.status, TaskStatus::Ok);
    assert_eq!(result.data, json!({"answered": true}));
}
#[test]
fn tool_call_times_out_and_late_result_cannot_answer_new_call() {
    let gateway = gateway();
    let address = gateway.local_addr();
    let first_gateway = std::sync::Arc::new(std::sync::Mutex::new(gateway));
    let second_gateway = std::sync::Arc::clone(&first_gateway);
    let mut client = connect_authenticated(address);
    send_json(&mut client, hello(1));
    send_json(&mut client, manifest(2, &["ping"]));

    let worker = std::thread::spawn(move || {
        first_gateway
            .lock()
            .unwrap()
            .call_tool("ping", json!({}))
            .unwrap()
    });
    let expired_call = read_json(&mut client);
    let timed_out = worker.join().unwrap();
    assert_eq!(timed_out.status, TaskStatus::Timeout);

    send_json(
        &mut client,
        result(3, expired_call["call_id"].as_str().unwrap(), "ok"),
    );
    let worker = std::thread::spawn(move || {
        second_gateway
            .lock()
            .unwrap()
            .call_tool("ping", json!({}))
            .unwrap()
    });
    let replacement_call = read_json(&mut client);
    assert_ne!(
        replacement_call["call_id"], expired_call["call_id"],
        "late result must not own the new pending call"
    );
    send_json(
        &mut client,
        result(4, replacement_call["call_id"].as_str().unwrap(), "ok"),
    );
    let replacement = worker.join().unwrap();
    assert_eq!(replacement.status, TaskStatus::Ok);
}

#[test]
fn oversized_frame_closes_the_connection() {
    let mut gateway = gateway();
    let mut client = connect_authenticated(gateway.local_addr());
    send_json(&mut client, hello(1));
    let mut oversized = hello(2);
    oversized["data"] = Value::String("x".repeat(2048));
    send_json(&mut client, oversized);

    match client.read().unwrap() {
        Message::Close(frame) => assert_eq!(
            frame.unwrap().code,
            tungstenite::protocol::frame::coding::CloseCode::Policy
        ),
        message => panic!("expected policy close, got {message:?}"),
    }
    assert!(gateway
        .next_event_timeout(Duration::from_millis(20))
        .unwrap()
        .is_none());
}

#[test]
fn binary_frame_closes_the_connection() {
    let mut gateway = gateway();
    let mut client = connect_authenticated(gateway.local_addr());
    send_json(&mut client, hello(1));
    client.send(Message::Binary(vec![1, 2, 3].into())).unwrap();

    match client.read().unwrap() {
        Message::Close(frame) => assert_eq!(
            frame.unwrap().code,
            tungstenite::protocol::frame::coding::CloseCode::Policy
        ),
        message => panic!("expected policy close, got {message:?}"),
    }
    assert!(gateway
        .next_event_timeout(Duration::from_millis(20))
        .unwrap()
        .is_none());
}

#[test]
fn replacement_connection_supersedes_pending_call() {
    let gateway = gateway();
    let address = gateway.local_addr();
    let old_gateway = std::sync::Arc::new(std::sync::Mutex::new(gateway));
    let replacement_gateway = std::sync::Arc::clone(&old_gateway);
    let mut old_client = connect_authenticated(address);
    send_json(&mut old_client, hello(1));
    send_json(&mut old_client, manifest(2, &["ping"]));

    let worker = std::thread::spawn(move || {
        old_gateway
            .lock()
            .unwrap()
            .call_tool("ping", json!({}))
            .unwrap()
    });
    let expired_call = read_json(&mut old_client);
    drop(old_client);
    let expired = worker.join().unwrap();
    assert_eq!(expired.status, TaskStatus::Cancelled);
    assert_eq!(expired.call_id, expired_call["call_id"].as_str().unwrap());

    let mut replacement = connect_authenticated(address);
    send_json(&mut replacement, hello(1));
    send_json(&mut replacement, manifest(2, &["ping"]));
    let worker = std::thread::spawn(move || {
        replacement_gateway
            .lock()
            .unwrap()
            .call_tool("ping", json!({}))
            .unwrap()
    });
    let call = read_json(&mut replacement);
    assert_eq!(
        call["seq"], 1,
        "outbound sequence must reset for each replacement adapter session"
    );
    send_json(
        &mut replacement,
        result(3, call["call_id"].as_str().unwrap(), "ok"),
    );
    assert_eq!(worker.join().unwrap().status, TaskStatus::Ok);
}

#[test]
fn raw_http_request_does_not_enter_message_exchange() {
    let gateway = gateway();
    let uri: Uri = url(gateway.local_addr()).parse().unwrap();
    let mut stream = TcpStream::connect(uri.authority().unwrap().as_str()).unwrap();
    let request = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n",
        uri.path(),
        uri.authority().unwrap()
    );
    stream.write_all(request.as_bytes()).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    assert!(response.contains("401 Unauthorized"), "{response}");
}

#[test]
fn call_tool_without_active_session_returns_unavailable() {
    let gateway = gateway();
    let result = gateway.call_tool("ping", json!({})).unwrap();
    assert_eq!(result.status, TaskStatus::Unavailable);
    assert_eq!(
        result.error.as_deref(),
        Some("no authenticated adapter session is active")
    );
}

#[test]
fn disconnect_cancels_pending_call() {
    let gateway = gateway();
    let address = gateway.local_addr();
    let gateway = std::sync::Arc::new(std::sync::Mutex::new(gateway));
    let mut client = connect_authenticated(address);
    send_json(&mut client, hello(1));
    send_json(&mut client, manifest(2, &["ping"]));

    let worker = std::thread::spawn(move || {
        gateway
            .lock()
            .unwrap()
            .call_tool("ping", json!({}))
            .unwrap()
    });
    let call = read_json(&mut client);
    drop(client);
    let cancelled = worker.join().unwrap();
    assert_eq!(cancelled.status, TaskStatus::Cancelled);
    assert_eq!(cancelled.call_id, call["call_id"].as_str().unwrap());
}

#[test]
fn overflowing_event_queue_closes_the_connection() {
    let mut gateway = gateway();
    let mut client = connect_authenticated(gateway.local_addr());
    if let MaybeTlsStream::Plain(stream) = client.get_mut() {
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
    }
    send_json(&mut client, hello(1));
    send_json(&mut client, manifest(2, &["ping"]));
    for sequence in 3..=67 {
        send_json(&mut client, event(sequence, "queued message"));
    }

    match client.read().unwrap() {
        Message::Close(frame) => assert_eq!(
            frame.unwrap().code,
            tungstenite::protocol::frame::coding::CloseCode::Policy
        ),
        message => panic!("expected policy close, got {message:?}"),
    }

    let mut delivered = 0;
    while let Some(_event) = gateway
        .next_event_timeout(Duration::from_millis(20))
        .unwrap()
    {
        delivered += 1;
        assert!(delivered <= 64, "overflow must not deliver a 65th event");
    }
    assert_eq!(
        delivered, 64,
        "exactly the queue cap is delivered before overflow closes the session"
    );
}

#[test]
fn pending_call_cap_rejects_overflow() {
    let gateway = std::sync::Arc::new(gateway());
    let mut client = connect_authenticated(gateway.local_addr());
    send_json(&mut client, hello(1));
    send_json(&mut client, manifest(2, &["ping"]));

    let mut handles = Vec::new();
    for _ in 0..65 {
        let gateway = std::sync::Arc::clone(&gateway);
        handles.push(std::thread::spawn(move || {
            gateway.call_tool("ping", json!({})).unwrap()
        }));
    }
    let results: Vec<_> = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .collect();
    assert_eq!(
        results
            .iter()
            .filter(|result| result.status == TaskStatus::Timeout)
            .count(),
        64
    );
    assert_eq!(
        results
            .iter()
            .filter(|result| result.status == TaskStatus::RateLimited)
            .count(),
        1
    );
}
