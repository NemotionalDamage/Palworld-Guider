//! Source-contract tests for the read-only UE4SS Lua adapter.
//!
//! These tests read the adapter sources on disk and enforce the approved
//! transport-only interface contract: exact environment names, a bounded
//! four-tool manifest, guarded chat handling, the validated active-Otomo
//! chain, finite-position guards, and the absence of file IPC, mutation
//! tools, chat-body logging, and token logging.

use std::fs;
use std::path::Path;

const ADAPTER_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../adapter/read-only/ue4ss");

fn read(relative: &str) -> String {
    let path = Path::new(ADAPTER_ROOT).join(relative);
    fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!("failed to read {}: {error}", path.display());
    })
}

fn assert_contains(source: &str, needle: &str, context: &str) {
    assert!(
        source.contains(needle),
        "{context}: expected source to contain `{needle}`"
    );
}

fn assert_not_contains(source: &str, needle: &str, context: &str) {
    assert!(
        !source.contains(needle),
        "{context}: expected source to NOT contain `{needle}`"
    );
}

fn main_lua() -> String {
    read("Scripts/main.lua")
}

fn transport_lua() -> String {
    read("Scripts/pal_transport.lua")
}

#[test]
fn lua_adapter_files_exist() {
    assert!(
        Path::new(ADAPTER_ROOT).join("Scripts/main.lua").is_file(),
        "Scripts/main.lua must exist"
    );
    assert!(
        Path::new(ADAPTER_ROOT)
            .join("Scripts/pal_transport.lua")
            .is_file(),
        "Scripts/pal_transport.lua must exist"
    );
    assert!(
        Path::new(ADAPTER_ROOT)
            .join("Scripts/pal_json.lua")
            .is_file(),
        "Scripts/pal_json.lua must exist"
    );
}

#[test]
fn transport_uses_guider_env_names_and_default_port() {
    let transport = transport_lua();
    assert_contains(
        &transport,
        "PALWORLD_GUIDER_GATEWAY_PORT",
        "pal_transport.lua environment name",
    );
    assert_contains(
        &transport,
        "PALWORLD_GUIDER_GATEWAY_TOKEN",
        "pal_transport.lua environment name",
    );
    assert_contains(&transport, "DEFAULT_PORT = 8071", "default gateway port");
}

#[test]
fn lua_chat_prefix_is_exactly_guide() {
    let main = main_lua();
    assert_contains(
        &main,
        "COMMAND_PREFIX = \"!guide \"",
        "chat command prefix must be exactly `!guide `",
    );
    assert_not_contains(
        &main,
        "COMMAND_PREFIX = \"!g \"",
        "legacy `!g ` prefix must not be used",
    );
}

#[test]
fn manifest_contains_exactly_four_guider_tools() {
    let transport = transport_lua();
    for tool in [
        "ping",
        "get_player_status",
        "get_active_pal_status",
        "send_chat_message",
    ] {
        assert_contains(
            &transport,
            &format!("name = \"{tool}\""),
            "capability manifest tool",
        );
    }
    assert_not_contains(&transport, "\"get_game_info\"", "manifest");
    assert_not_contains(&transport, "\"move_to\"", "manifest");
    assert_not_contains(&transport, "\"follow_player\"", "manifest");
}
#[test]
fn request_dispatch_covers_exactly_the_four_tools() {
    let main = main_lua();
    for tool in [
        "\"ping\"",
        "\"get_player_status\"",
        "\"get_active_pal_status\"",
        "\"send_chat_message\"",
    ] {
        assert_contains(&main, &format!("frame.tool == {tool}"), "request dispatch");
    }
}

#[test]
fn unknown_tool_returns_unsupported() {
    let main = main_lua();
    assert_contains(
        &main,
        "\"tool is not whitelisted\"",
        "unknown-tool response",
    );
    assert_contains(&main, "\"unsupported\"", "unknown-tool response status");
}

#[test]
fn ping_returns_guider_service() {
    let main = main_lua();
    assert_contains(
        &main,
        "service = \"palworld-guider\"",
        "ping response service",
    );
    assert_contains(&main, "\"palworld-guider\"", "ping response service");
}

#[test]
fn finite_position_guard_present() {
    let main = main_lua();
    assert_contains(&main, "finite_number", "position guard");
    assert_contains(
        &main,
        "finite_number(x) and finite_number(y) and finite_number(z)",
        "finite x/y/z position guard",
    );
}

#[test]
fn chat_send_uses_only_system_chat_api() {
    let main = main_lua();
    assert_contains(&main, "SendSystemToPlayerChat", "chat output API");
    assert_not_contains(&main, "SendChatToPlayer", "chat output API");
    assert_not_contains(&main, "AddChatMessage", "chat output API");
}

#[test]
fn chat_hook_is_guarded_and_reregistered() {
    let main = main_lua();
    assert_contains(&main, "RegisterHook", "chat hook");
    assert_contains(&main, "UnregisterHook", "chat hook unregister");
    assert_contains(
        &main,
        "/Script/Pal.PalGameStateInGame:BroadcastChatMessage",
        "chat hook target",
    );
    assert_contains(
        &main,
        "chat_hook_registered",
        "guarded re-registration flag",
    );
    assert_contains(&main, "ensure_chat_hook()", "hook re-registration tick");
}

#[test]
fn active_otomo_chain_present() {
    let main = main_lua();
    for needle in [
        "GetLocalPalPlayerController",
        "GetOtomoHolderComponent",
        "TryGetSpawnedOtomoHandle",
        "TryGetIndividualParameter",
        "GetCharacterID",
        "TryGetIndividualActor",
    ] {
        assert_contains(&main, needle, "validated active-Otomo chain");
    }
}

#[test]
fn chat_message_bound_is_500_characters() {
    let main = main_lua();
    assert_contains(&main, "MAX_CHAT_MESSAGE_CHARS = 500", "chat message bound");
    assert_contains(&main, "500", "chat message bound value");
}
#[test]
fn no_file_ipc_in_lua() {
    for (source, context) in [
        (main_lua(), "main.lua"),
        (transport_lua(), "pal_transport.lua"),
        (read("Scripts/pal_json.lua"), "pal_json.lua"),
    ] {
        for needle in [
            "io.open",
            "os.remove",
            "os.rename",
            "io.write",
            "ipc",
            "atomic_write",
        ] {
            assert_not_contains(&source, needle, &format!("{context} file IPC"));
        }
    }
}

#[test]
fn no_chat_body_logging() {
    let main = main_lua();
    assert_not_contains(&main, "tostring(message)", "chat-body logging");
    assert_not_contains(&main, "tostring(instruction)", "chat-body logging");
    assert_not_contains(&main, "print(message)", "chat-body logging");
}

#[test]
fn no_token_logging() {
    let main = main_lua();
    let transport = transport_lua();
    assert_not_contains(&main, "GATEWAY_TOKEN", "token in main.lua");
    assert_not_contains(&transport, "tostring(token)", "token logging");
    assert_not_contains(&transport, "print(token)", "token logging");
}

#[test]
fn forbidden_mutation_and_state_tokens_absent() {
    for (source, context) in [
        (main_lua(), "main.lua"),
        (transport_lua(), "pal_transport.lua"),
    ] {
        for needle in [
            "AddItem",
            "move_to",
            "follow_player",
            "attack",
            "teleport",
            "spawn",
            "health",
            "stamina",
            "inventory",
            "Level.sav",
            "ReadSave",
        ] {
            assert_not_contains(
                &source,
                needle,
                &format!("{context} forbidden token `{needle}`"),
            );
        }
    }
}

#[test]
fn transport_session_state_preserved() {
    let transport = transport_lua();
    for needle in [
        "SCHEMA_VERSION = 2",
        "RECONNECT_DELAY_MS = 250",
        "MAX_RECONNECT_DELAY_MS = 30000",
        "HEARTBEAT_INTERVAL_MS = 15000",
        "MAX_FRAME_BYTES = 65536",
        "QUEUE_LIMIT = 64",
    ] {
        assert_contains(&transport, needle, "transport session constants");
    }
    assert_contains(&transport, "reset_session", "sequence reset on reconnect");
    assert_contains(
        &transport,
        "validate_inbound_sequence",
        "strict inbound sequence",
    );
    assert_contains(&transport, "guider_conn()", "reconnect call");
    assert_contains(&transport, "guider_close()", "close on send failure");
}

#[test]
fn transport_uses_guider_native_callbacks() {
    let transport = transport_lua();
    for callback in [
        "guider_cfg",
        "guider_conn",
        "guider_send",
        "guider_poll",
        "guider_stat",
        "guider_close",
    ] {
        assert_contains(&transport, callback, "native callback");
    }
    assert_not_contains(&transport, "pal_cfg", "old callback names");
    assert_not_contains(&transport, "pal_conn", "old callback names");
}

#[test]
fn strict_json_decode_used() {
    let transport = transport_lua();
    let json = read("Scripts/pal_json.lua");
    assert_contains(&transport, "JSON.decode", "strict JSON decode");
    assert_contains(&transport, "pcall", "guarded JSON parse");
    assert_contains(&json, "function PalJSON.encode", "json encoder");
    assert_contains(&json, "function PalJSON.decode", "json decoder");
    assert_contains(&json, "json_escape", "json escaping");
}
