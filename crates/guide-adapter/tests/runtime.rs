//! Runtime tool surface tests for the in-game adapter.
//!
//! Each test drives a real loopback `WebSocketGateway` and a fake adapter
//! client speaking the schema-2 protocol with Bearer authentication and
//! independent strictly increasing sequences for hello, manifest, event, and
//! result frames.

use guide_adapter::{GameAdapterRuntime, INTERNAL_RUNTIME_TOOLS, MODEL_VISIBLE_RUNTIME_TOOLS};
use guide_tools::{RuntimeToolSource, ToolStatus};
use serde_json::json;
use std::time::{Duration, Instant};

mod common;
use common::{
    connect_authenticated, event_frame, gateway, hello, manifest, read_json, result, send_json,
};

fn wait_for_capabilities(runtime: &GameAdapterRuntime, expected: &[&str]) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if runtime.capabilities() == expected {
            return;
        }
        if Instant::now() >= deadline {
            panic!(
                "gateway never published manifest {expected:?}; saw {:?}",
                runtime.capabilities()
            );
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn no_runtime_tools_are_visible_before_a_manifest() {
    let runtime = GameAdapterRuntime::new(gateway());

    assert!(runtime.capabilities().is_empty());
    assert!(runtime.model_capabilities().is_empty());
    assert!(runtime.definitions().is_empty());
    assert!(runtime.endpoint().ip().is_loopback());
}

#[test]
fn manifest_is_intersected_with_the_compile_time_allowlist() {
    let gateway = gateway();
    let address = gateway.local_addr();
    let runtime = GameAdapterRuntime::new(gateway);
    let mut client = connect_authenticated(address);
    send_json(&mut client, hello(1));
    send_json(
        &mut client,
        manifest(
            2,
            &[
                "get_player_status",
                "get_active_pal_status",
                "send_chat_message",
                "move_to",
                "inventory_read",
            ],
        ),
    );
    wait_for_capabilities(
        &runtime,
        &[
            "get_player_status",
            "get_active_pal_status",
            "send_chat_message",
            "move_to",
            "inventory_read",
        ],
    );

    assert_eq!(
        runtime.model_capabilities(),
        vec![
            "get_player_status".to_string(),
            "get_active_pal_status".to_string()
        ]
    );

    let definitions = runtime.definitions();
    assert_eq!(definitions.len(), 2);
    assert_eq!(definitions[0].name, "get_player_status");
    assert_eq!(definitions[1].name, "get_active_pal_status");
    for definition in &definitions {
        assert_eq!(
            definition.parameters_schema,
            json!({"type": "object", "properties": {}, "required": []}),
            "runtime tools must publish no-argument schemas"
        );
        assert!(
            MODEL_VISIBLE_RUNTIME_TOOLS.contains(&definition.name.as_str()),
            "model-visible definitions must come from the compile-time allowlist"
        );
    }
    assert!(!definitions
        .iter()
        .any(|definition| definition.name == "send_chat_message"));
}

#[test]
fn unavailable_adapter_and_read_failure_map_to_clear_envelopes() {
    let runtime = GameAdapterRuntime::new(gateway());

    let no_session = runtime.dispatch("get_player_status", &json!({}));
    assert_eq!(no_session.status, ToolStatus::Error);
    assert_eq!(no_session.data, None);
    assert_eq!(
        no_session.errors,
        vec!["no authenticated adapter session is active".to_string()]
    );

    let send_error = runtime.send_chat("hello");
    let error = send_error.expect_err("send_chat must fail closed without a session");
    assert!(
        error
            .to_string()
            .contains("no authenticated adapter session is active"),
        "unexpected send_chat error: {error}"
    );

    // Read failure: a connected adapter advertises the read tool but never
    // answers, so the call fails after the gateway call timeout.
    let gateway = gateway();
    let address = gateway.local_addr();
    let runtime = GameAdapterRuntime::new(gateway);
    let mut client = connect_authenticated(address);
    send_json(&mut client, hello(1));
    send_json(&mut client, manifest(2, &["get_player_status"]));
    wait_for_capabilities(&runtime, &["get_player_status"]);

    let runtime_clone = runtime.clone();
    let worker =
        std::thread::spawn(move || runtime_clone.dispatch("get_player_status", &json!({})));
    let call = read_json(&mut client);
    assert_eq!(call["type"], "tool_call");
    assert_eq!(call["tool"], "get_player_status");
    // Deliberately never answer: the adapter read fails after the call timeout.

    let failure = worker.join().unwrap();
    assert_eq!(failure.status, ToolStatus::Unknown);
    assert!(failure.data.is_none());
    assert!(failure.errors.is_empty());
    assert!(
        failure
            .uncertainty
            .iter()
            .any(|message| message.contains("timed out")),
        "expected a timeout uncertainty, got {failure:?}"
    );
}

#[test]
fn send_chat_is_internal_only() {
    assert!(INTERNAL_RUNTIME_TOOLS.contains(&"send_chat_message"));

    let gateway = gateway();
    let address = gateway.local_addr();
    let runtime = GameAdapterRuntime::new(gateway);
    let mut client = connect_authenticated(address);
    send_json(&mut client, hello(1));
    send_json(&mut client, manifest(2, &["send_chat_message"]));
    wait_for_capabilities(&runtime, &["send_chat_message"]);

    assert_eq!(runtime.model_capabilities(), Vec::<String>::new());
    assert!(runtime.definitions().is_empty());
    assert!(!runtime
        .definitions()
        .iter()
        .any(|definition| definition.name == "send_chat_message"));

    let runtime_clone = runtime.clone();
    let worker = std::thread::spawn(move || {
        runtime_clone.dispatch("send_chat_message", &json!({"message": "hello in game"}))
    });
    let call = read_json(&mut client);
    assert_eq!(call["type"], "tool_call");
    assert_eq!(call["tool"], "send_chat_message");
    assert_eq!(call["args"]["message"], "hello in game");
    send_json(
        &mut client,
        result(3, call["call_id"].as_str().unwrap(), "ok"),
    );
    let outcome = worker.join().unwrap();
    assert_eq!(outcome.status, ToolStatus::Ok);
    assert_eq!(outcome.data, Some(json!({"answered": true})));
    assert!(outcome.errors.is_empty());

    let runtime_clone = runtime.clone();
    let worker = std::thread::spawn(move || runtime_clone.send_chat("hello via send_chat"));
    let call = read_json(&mut client);
    assert_eq!(call["type"], "tool_call");
    assert_eq!(call["tool"], "send_chat_message");
    assert_eq!(call["args"]["message"], "hello via send_chat");
    send_json(
        &mut client,
        result(4, call["call_id"].as_str().unwrap(), "ok"),
    );
    worker
        .join()
        .unwrap()
        .expect("send_chat succeeds when advertised");
}

#[test]
fn events_are_available_through_next_event_timeout() {
    let gateway = gateway();
    let address = gateway.local_addr();
    let runtime = GameAdapterRuntime::new(gateway);
    let mut client = connect_authenticated(address);
    send_json(&mut client, hello(1));
    send_json(&mut client, manifest(2, &["send_chat_message"]));
    send_json(&mut client, event_frame(3, "!guide ping"));

    let chat_event = runtime
        .next_event_timeout(Duration::from_millis(500))
        .expect("adapter read must not error")
        .expect("queued chat event");
    assert_eq!(chat_event.text, "!guide ping");
    assert_eq!(chat_event.player_id, "player");
    assert_eq!(chat_event.event_id, "event-3");

    assert!(runtime
        .next_event_timeout(Duration::from_millis(20))
        .expect("idle read must not error")
        .is_none());
}
