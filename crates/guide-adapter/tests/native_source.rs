//! Source-contract tests for the read-only UE4SS native transport shim.
//!
//! These tests read the native adapter sources on disk and enforce the
//! approved transport-only contract: the `PalworldGuider` mod name, the
//! exact `guider_*` callbacks, the `start_mod`/`uninstall_mod` exports,
//! the 65,536-byte frame and 64-message queue limits, a CMake package
//! that links only UE4SS and WinHTTP, and the absence of game, hook,
//! shell, file-path, and advisory logic in the C++.

use std::fs;
use std::path::Path;

const NATIVE_ROOT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../adapter/read-only/ue4ss/native"
);

fn read(relative: &str) -> String {
    let path = Path::new(NATIVE_ROOT).join(relative);
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

#[test]
fn native_files_exist() {
    for relative in [
        "CMakeLists.txt",
        "UE4SS.def",
        "src/palworld_guider.cpp",
        "src/pal_transport.cpp",
        "include/pal_transport.h",
        "include/Mod/CppUserModBase.hpp",
        "include/File/Macros.hpp",
    ] {
        let path = Path::new(NATIVE_ROOT).join(relative);
        assert!(path.is_file(), "native file must exist: {relative}");
    }
}

#[test]
fn mod_name_is_palworld_guider() {
    let cpp = read("src/palworld_guider.cpp");
    let cmake = read("CMakeLists.txt");
    assert_contains(&cpp, "ModName = STR(\"PalworldGuider\")", "mod name");
    assert_contains(&cmake, "project(PalworldGuider", "cmake project name");
    assert_contains(&cmake, "add_library(PalworldGuider SHARED", "cmake target");
    assert_contains(&cmake, "OUTPUT_NAME main", "cmake output dll");
}

#[test]
fn callbacks_are_exactly_the_guider_six() {
    let cpp = read("src/palworld_guider.cpp");
    for callback in [
        "guider_cfg",
        "guider_conn",
        "guider_send",
        "guider_poll",
        "guider_stat",
        "guider_close",
    ] {
        assert_contains(&cpp, callback, "native callback");
        assert_contains(
            &cpp,
            &format!("lua.register_function(\"{callback}\", &{callback})"),
            "lua callback registration",
        );
    }
    assert_not_contains(&cpp, "pal_cfg", "old callback name");
    assert_not_contains(&cpp, "pal_conn", "old callback name");
    assert_not_contains(&cpp, "pal_send", "old callback name");
}
#[test]
fn exports_present_in_cpp_and_def() {
    let cpp = read("src/palworld_guider.cpp");
    let def = read("UE4SS.def");
    assert_contains(&cpp, "start_mod", "mod export in cpp");
    assert_contains(&cpp, "uninstall_mod", "mod export in cpp");
    assert_contains(&def, "start_mod", "mod export in def");
    assert_contains(&def, "uninstall_mod", "mod export in def");
}

#[test]
fn def_exports_only_start_mod_and_uninstall_mod() {
    let def = read("UE4SS.def");
    let exports: Vec<&str> = def
        .lines()
        .skip_while(|line| !line.trim().eq_ignore_ascii_case("EXPORTS"))
        .skip(1)
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .collect();
    assert_eq!(
        exports,
        vec!["start_mod", "uninstall_mod"],
        "UE4SS.def must export only start_mod and uninstall_mod"
    );
}

#[test]
fn frame_and_queue_limits_present() {
    let transport = read("src/pal_transport.cpp");
    assert_contains(&transport, "65'536", "max frame bytes");
    assert_contains(&transport, "64", "queue limit");
    assert_contains(&transport, "kDefaultFrameLimit", "frame limit constant");
    assert_contains(&transport, "kDefaultQueueLimit", "queue limit constant");
}

#[test]
fn cmake_verifies_ue4ss_sha256() {
    let cmake = read("CMakeLists.txt");
    assert_contains(
        &cmake,
        "8AC18FBFFC1EF96B0662D4A2D537B3F224C26D65CAABA7989A9404C566102B26",
        "UE4SS DLL SHA256 constant",
    );
    assert_contains(&cmake, "file(SHA256 \"${UE4SS_DLL}\"", "cmake hash check");
}

#[test]
fn cmake_accepts_ue4ss_dll_option() {
    let cmake = read("CMakeLists.txt");
    assert_contains(&cmake, "UE4SS_DLL", "cmake option");
    assert_contains(
        &cmake,
        "set(UE4SS_DLL \"\" CACHE FILEPATH",
        "cmake cache variable",
    );
}

#[test]
fn cmake_links_only_ue4ss_and_winhttp() {
    let cmake = read("CMakeLists.txt");
    assert_contains(
        &cmake,
        "target_link_libraries(PalworldGuider PRIVATE UE4SS winhttp)",
        "link line",
    );
    for forbidden in ["ws2_32", "user32", "shell32", "advapi32", "ole32", "gdi32"] {
        assert_not_contains(&cmake, forbidden, "additional link library");
    }
}

#[test]
fn cmake_stages_the_package() {
    let cmake = read("CMakeLists.txt");
    assert_contains(&cmake, "PalworldGuider/dlls", "staged dll directory");
    assert_contains(&cmake, "PalworldGuider/Scripts", "staged scripts directory");
    for script in ["main.lua", "pal_transport.lua", "pal_json.lua"] {
        assert_contains(&cmake, &format!("Scripts/{script}"), "staged script copy");
    }
    assert_contains(&cmake, "copy_if_different", "post-build copy");
}
#[test]
fn transport_core_has_no_game_or_hook_logic() {
    let transport = read("src/pal_transport.cpp");
    let header = read("include/pal_transport.h");
    for source in [&transport, &header] {
        for forbidden in [
            "Unreal",
            "UObject",
            "CppUserModBase",
            "Lua",
            "RegisterHook",
            "UnregisterHook",
            "Palworld",
            "PalAgent",
        ] {
            assert_not_contains(source, forbidden, "transport core game/hook logic");
        }
    }
    assert_contains(&header, "GuiderTransportState", "renamed transport state");
    assert_contains(
        &transport,
        "GuiderTransportState",
        "renamed transport state",
    );
    assert_contains(
        &transport,
        "guider_transport_connect",
        "renamed transport api",
    );
    assert_not_contains(&transport, "pal_transport_", "old transport api");
}

#[test]
fn transport_core_has_no_file_or_shell_logic() {
    let transport = read("src/pal_transport.cpp");
    let header = read("include/pal_transport.h");
    for source in [&transport, &header] {
        for forbidden in [
            "CreateFile",
            "WriteFile",
            "ReadFile",
            "DeleteFile",
            "GetModuleFileName",
            "system(",
            "ShellExecute",
            "CreateProcess",
        ] {
            assert_not_contains(source, forbidden, "transport core file/shell logic");
        }
    }
}

#[test]
fn cpp_shim_has_no_file_hook_or_shell_logic() {
    let cpp = read("src/palworld_guider.cpp");
    for forbidden in [
        "CreateFile",
        "WriteFile",
        "record_lifecycle",
        "RegisterHook",
        "UnregisterHook",
        "GetModuleFileName",
        "system(",
        "ShellExecute",
    ] {
        assert_not_contains(&cpp, forbidden, "shim file/hook/shell logic");
    }
}

#[test]
fn abi_size_assertion_preserved() {
    let cpp = read("src/palworld_guider.cpp");
    let header = read("include/Mod/CppUserModBase.hpp");
    let macros = read("include/File/Macros.hpp");
    assert_contains(
        &cpp,
        "static_assert(sizeof(RC::CppUserModBase) == 192",
        "abi static assert in cpp",
    );
    assert_contains(
        &header,
        "static_assert(sizeof(RC::CppUserModBase) == 192",
        "abi static assert in header",
    );
    assert_contains(&header, "class CppUserModBase", "ue4ss base class");
    assert_contains(&macros, "RC_UE4SS_API", "ue4ss macros");
}
