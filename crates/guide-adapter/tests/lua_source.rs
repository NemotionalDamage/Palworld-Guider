//! Source-contract tests for the read-only UE4SS Lua adapter.
//!
//! These tests read the adapter sources on disk and enforce the approved
//! transport-only interface contract: exact environment names, a bounded
//! five-tool manifest, the compile-time capability switch that overrides the
//! manifest from main.lua, guarded chat handling, the validated active-Otomo
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

fn repo(relative: &str) -> String {
    read(&format!("../../../{relative}"))
}

#[test]
fn lua_capability_switch_is_compile_time_and_default_full() {
    let main = main_lua();
    let cmake = repo("adapter/read-only/ue4ss/native/CMakeLists.txt");
    let build_script = repo("scripts/Build-Ue4ssAdapter.ps1");
    let support_manifest = read("support-manifest.json");

    assert_contains(
        &main,
        "local LUA_CAPABILITY = \"@GUIDER_LUA_CAPABILITY@\"",
        "templated compile-time capability",
    );
    assert_not_contains(&main, "os.getenv(", "runtime capability input");
    assert_contains(
        &cmake,
        "set(GUIDER_LUA_CAPABILITY \"full\" CACHE STRING",
        "CMake capability default",
    );
    assert_contains(
        &cmake,
        "GUIDER_LUA_CAPABILITY MATCHES \"^(noop|chat|full)$\"",
        "CMake capability allowlist",
    );
    assert_contains(
        &cmake,
        "configure_file(\"${GUIDER_SCRIPTS_DIR}/main.lua\"",
        "CMake Lua templating",
    );
    assert_contains(
        &build_script,
        "[ValidateSet('noop', 'chat', 'full')][string]$Capability = 'full'",
        "build wrapper capability allowlist",
    );
    assert_contains(
        &build_script,
        "-DGUIDER_LUA_CAPABILITY=$Capability",
        "build wrapper forwards capability",
    );
    assert_contains(
        &support_manifest,
        "\"lua_capability\": {\n    \"default\": \"full\",\n    \"values\": [\"noop\", \"chat\", \"full\"]\n  }",
        "support manifest capability contract",
    );
}

#[test]
fn capability_manifest_matches_each_mode() {
    let main = main_lua();
    let transport = transport_lua();
    assert_contains(&main, "capability == \"noop\"", "noop manifest branch");
    assert_contains(&main, "capability == \"chat\"", "chat manifest branch");
    assert_contains(&main, "capability == \"full\"", "full manifest branch");
    assert_contains(
        &main,
        "{name = \"ping\", mutation = false, evidence = \"A\", status = \"enabled\"}",
        "noop retains ping",
    );

    let noop_start = main
        .find("if capability == \"noop\" then")
        .expect("noop branch");
    let noop_end = main[noop_start..]
        .find("elseif capability == \"chat\" then")
        .map(|offset| noop_start + offset)
        .expect("chat branch");
    let noop_branch = &main[noop_start..noop_end];
    for disabled_tool in [
        "get_player_status",
        "get_active_pal_status",
        "get_base_camps",
        "send_chat_message",
    ] {
        assert_not_contains(noop_branch, disabled_tool, "noop capability manifest");
    }

    let chat_start = noop_end;
    let chat_end = main[chat_start..]
        .find("elseif capability == \"full\" then")
        .map(|offset| chat_start + offset)
        .expect("full branch");
    let chat_branch = &main[chat_start..chat_end];
    assert_contains(chat_branch, "send_chat_message", "chat capability manifest");
    for disabled_tool in [
        "get_player_status",
        "get_active_pal_status",
        "get_base_camps",
    ] {
        assert_not_contains(chat_branch, disabled_tool, "chat capability manifest");
    }
    assert_contains(
        &main,
        "data = {tools = tools}",
        "override manifest embeds selected tools",
    );
    for forbidden in [
        "GUIDER_LUA_CAPABILITY",
        "set_capability",
        "capability == \"noop\"",
        "capability == \"chat\"",
    ] {
        assert_not_contains(
            &transport,
            forbidden,
            "transport must stay capability-agnostic",
        );
    }
}

#[test]
fn main_overrides_transport_manifest_before_connect() {
    let main = main_lua();
    assert_contains(
        &main,
        "PalTransport.capability_manifest_frame = capability_manifest_frame",
        "main-only manifest override",
    );
    assert_contains(
        &main,
        "local function capability_manifest_frame(sequence)",
        "main defines the overriding manifest frame",
    );
    let override_position = main
        .find("PalTransport.capability_manifest_frame = capability_manifest_frame")
        .expect("manifest override");
    let connect_position = main.find("if not PalTransport.connect()").expect("connect");
    assert!(
        override_position < connect_position,
        "capability manifest override must happen before PalTransport.connect()"
    );
}

#[test]
fn dispatch_and_chat_hook_match_each_capability_mode() {
    let main = main_lua();
    assert_contains(
        &main,
        "local READ_CAPABILITY_ENABLED = LUA_CAPABILITY == \"full\"",
        "full-only read switch",
    );
    assert_contains(
        &main,
        "local CHAT_CAPABILITY_ENABLED = LUA_CAPABILITY == \"chat\" or LUA_CAPABILITY == \"full\"",
        "chat/full chat switch",
    );
    assert_contains(
        &main,
        "READ_CAPABILITY_ENABLED and frame.tool == \"get_player_status\"",
        "player read dispatch gate",
    );
    assert_contains(
        &main,
        "READ_CAPABILITY_ENABLED and frame.tool == \"get_active_pal_status\"",
        "Otomo read dispatch gate",
    );
    assert_contains(
        &main,
        "READ_CAPABILITY_ENABLED and frame.tool == \"get_base_camps\"",
        "base-camp read dispatch gate",
    );
    assert_contains(
        &main,
        "CHAT_CAPABILITY_ENABLED and frame.tool == \"send_chat_message\"",
        "chat tool dispatch gate",
    );
    assert_contains(
        &main,
        "if CHAT_CAPABILITY_ENABLED then\n    ensure_chat_hook()",
        "initial chat hook gate",
    );
    assert_contains(
        &main,
        "if CHAT_CAPABILITY_ENABLED then\n        ensure_chat_hook()",
        "poll chat hook gate",
    );
}

#[test]
fn invalid_lua_capability_and_current_build_fail_closed() {
    let main = main_lua();
    let cmake = repo("adapter/read-only/ue4ss/native/CMakeLists.txt");
    let support_manifest = read("support-manifest.json");

    assert_contains(
        &cmake,
        "message(FATAL_ERROR \"GUIDER_LUA_CAPABILITY must be one of: noop, chat, full\")",
        "CMake invalid capability rejection",
    );
    assert_contains(
        &main,
        "if not capability_tools(LUA_CAPABILITY) then\n    print(TAG .. \" invalid compile-time capability switch\")\n    return\nend",
        "invalid capability startup rejection",
    );
    assert_contains(
        &support_manifest,
        "\"game_build_ids\": [\n    \"24575825\",\n    \"25094871\"\n  ]",
        "both live-verified builds are reviewed",
    );
}

#[test]
fn capability_switch_introduces_no_mutation_api() {
    for source in [main_lua(), transport_lua()] {
        for forbidden_api in [
            "SetPlayerPosition",
            "SpawnItem",
            "AddInventoryItem",
            "SetHealth",
            "SetStamina",
            "TeleportTo",
            "MoveTo",
        ] {
            assert_not_contains(&source, forbidden_api, "capability mutation API");
        }
    }
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
fn transport_manifest_remains_full_default_and_is_overridden_by_main() {
    let transport = transport_lua();
    let main = main_lua();
    for tool in [
        "ping",
        "get_player_status",
        "get_active_pal_status",
        "get_base_camps",
        "send_chat_message",
    ] {
        assert_contains(
            &transport,
            &format!("name = \"{tool}\""),
            "unmodified transport manifest tool",
        );
    }
    assert_not_contains(
        &transport,
        "\"get_game_info\"",
        "unmodified transport manifest",
    );
    assert_not_contains(&transport, "\"move_to\"", "unmodified transport manifest");
    assert_not_contains(
        &transport,
        "\"follow_player\"",
        "unmodified transport manifest",
    );
    assert_contains(
        &main,
        "PalTransport.capability_manifest_frame = capability_manifest_frame",
        "runtime manifest override in main",
    );
}
#[test]
fn request_dispatch_covers_exactly_the_five_tools() {
    let main = main_lua();
    for tool in [
        "\"ping\"",
        "\"get_player_status\"",
        "\"get_active_pal_status\"",
        "\"get_base_camps\"",
        "\"send_chat_message\"",
    ] {
        assert_contains(&main, &format!("frame.tool == {tool}"), "request dispatch");
    }
}

#[test]
fn base_camp_lookup_reads_transform_properties() {
    let main = main_lua();
    for expression in [
        "transform.Translation",
        "object.Transform",
        "object.BaseCampPointTransform",
        "component = object.RootComponent",
        "component:K2_GetComponentLocation",
    ] {
        assert_contains(&main, expression, "base-camp transform candidate");
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
fn chat_hook_is_guarded_and_registered_once() {
    let main = main_lua();
    assert_contains(&main, "RegisterHook", "chat hook");
    assert_contains(
        &main,
        "/Script/Pal.PalGameStateInGame:BroadcastChatMessage",
        "chat hook target",
    );
    assert_contains(&main, "chat_hook_registered", "guarded registration flag");
    assert_contains(&main, "ensure_chat_hook()", "hook registration tick");
    assert_not_contains(
        &main,
        "UnregisterHook",
        "hook must never unregister from inside its own callback",
    );
    assert_contains(&main, "publish_once", "deduplicated chat publish");
    assert_contains(
        &main,
        "last_published_text",
        "repeat-broadcast deduplication window",
    );
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

#[test]
fn json_decoder_handles_escaped_strings() {
    // pal_json.lua must decode strings containing JSON escapes (for example
    // \" inside an error message body). The escape branch compares a single
    // character, so it must use a one-character backslash literal; the
    // two-character "\\\\" literal made the branch dead code and truncated
    // any escaped string at the first escaped quote.
    let json = read("Scripts/pal_json.lua");
    assert_contains(
        &json,
        "elseif character == \"\\\\\" then",
        "single-backslash escape branch",
    );
    assert_contains(&json, "b=\"\\b\"", "backspace escape");
    assert_contains(&json, "f=\"\\f\"", "form-feed escape");
    assert_contains(&json, "n=\"\\n\"", "newline escape");
    assert_contains(&json, "r=\"\\r\"", "carriage-return escape");
    assert_contains(&json, "t=\"\\t\"", "tab escape");
    assert_contains(&json, "[\"\\\\\"]=\"\\\\\"", "backslash escape");
    assert_contains(
        &json,
        "\x5b\x27\x22\x27\x5d\x3d\x27\x22\x27",
        "quote escape",
    );
    assert_not_contains(&json, "character == \"\\\\\\\\\"", "dead escape branch");
}
