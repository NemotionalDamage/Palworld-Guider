//! Shared fake-adapter client helpers for the schema-2 loopback protocol.
//! Each test binary uses a different subset, so unused helpers are expected.
#![allow(dead_code)]

use game_gateway::{WebSocketGateway, WebSocketGatewayConfig};
use serde_json::{json, Value};
use std::io::ErrorKind;
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;
use tungstenite::client::IntoClientRequest;
use tungstenite::http::header::AUTHORIZATION;
use tungstenite::http::HeaderValue;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{connect, Message};

pub type ClientSocket = tungstenite::WebSocket<MaybeTlsStream<TcpStream>>;

pub const TOKEN: &str = "guider-server-token";

pub fn gateway() -> WebSocketGateway {
    WebSocketGateway::bind(
        "127.0.0.1:0",
        TOKEN,
        WebSocketGatewayConfig {
            max_payload_bytes: 64 * 1024,
            call_timeout: Duration::from_millis(80),
        },
    )
    .expect("loopback gateway binds")
}

pub fn url(address: SocketAddr) -> String {
    format!("ws://{address}")
}

pub fn connect_authenticated(address: SocketAddr) -> ClientSocket {
    let mut request = url(address).into_client_request().unwrap();
    request.headers_mut().insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {TOKEN}")).unwrap(),
    );
    let mut socket = connect(request).unwrap().0;
    if let MaybeTlsStream::Plain(stream) = socket.get_mut() {
        // Disable Nagle so burst frames are not stalled by delayed ACKs.
        stream.set_nodelay(true).unwrap();
    }
    socket
}

pub fn send_json(socket: &mut ClientSocket, frame: Value) {
    socket
        .send(Message::Text(frame.to_string().into()))
        .unwrap();
}

pub fn hello(sequence: u64) -> Value {
    json!({
        "schema_version": 2,
        "type": "hello",
        "seq": sequence,
        "timestamp_ms": sequence
    })
}

pub fn manifest(sequence: u64, enabled: &[&str]) -> Value {
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

pub fn event_frame(sequence: u64, text: &str) -> Value {
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

pub fn result(sequence: u64, call_id: &str, status: &str) -> Value {
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

pub fn read_json(socket: &mut ClientSocket) -> Value {
    match socket.read().unwrap() {
        Message::Text(text) => serde_json::from_str(&text).unwrap(),
        Message::Close(frame) => panic!("unexpected close: {frame:?}"),
        message => panic!("unexpected message: {message:?}"),
    }
}

pub fn read_json_timeout(socket: &mut ClientSocket, timeout: Duration) -> Result<Value, String> {
    if let MaybeTlsStream::Plain(stream) = socket.get_ref() {
        stream.set_read_timeout(Some(timeout)).ok();
    }
    match socket.read() {
        Ok(Message::Text(text)) => serde_json::from_str(&text).map_err(|error| error.to_string()),
        Ok(Message::Close(frame)) => Err(format!("unexpected close: {frame:?}")),
        Ok(message) => Err(format!("unexpected message: {message:?}")),
        Err(tungstenite::Error::Io(io_error))
            if matches!(io_error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) =>
        {
            Err("read timed out".to_string())
        }
        Err(error) => Err(format!("read error: {error}")),
    }
}
