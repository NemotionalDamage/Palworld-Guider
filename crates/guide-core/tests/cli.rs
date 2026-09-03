use serde_json::Value;
use std::process::Command;

const BINARY: &str = env!("CARGO_BIN_EXE_guide-core");

fn run(arguments: &[&str]) -> (Value, i32) {
    let output = Command::new(BINARY)
        .args(arguments)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("guide-core binary must run");
    let stdout = String::from_utf8(output.stdout).expect("CLI writes UTF-8 JSON");
    let value = serde_json::from_str(&stdout).expect("CLI writes one JSON object");
    (value, output.status.code().expect("CLI sets exit code"))
}

#[test]
fn cli_answers_exact_lookup_with_provenance() {
    let (answer, code) = run(&["--data", "../../data/reviewed", "lookup", "item", "stone"]);

    assert_eq!(code, 0);
    assert_eq!(answer["status"], "ok");
    assert_eq!(answer["data"]["id"], "ITEM_STONE");
    assert_eq!(answer["version"]["knowledge_version"], "1.0.3");
    assert!(answer["provenance"]
        .as_array()
        .expect("provenance is an array")
        .iter()
        .any(|item| item["source_id"] == "SRC-PALDB-CORE-ITEMS-V1_0_3-20260902"));
}

#[test]
fn cli_resolves_new_local_build_items_through_chinese_names() {
    let (answer, code) = run(&[
        "--data",
        "../../data/reviewed",
        "lookup",
        "item",
        "攻击吊坠",
    ]);

    assert_eq!(code, 0);
    assert_eq!(answer["status"], "ok");
    assert_eq!(answer["data"]["id"], "ITEM_ACCESSORY_AT_1");
    assert_eq!(answer["data"]["names"]["zh_hans"], "攻击吊坠");
    assert_eq!(
        answer["data"]["description"],
        "An accessory that slightly raises Attack."
    );
}

#[test]
fn cli_calculates_materials_and_shortage_offline() {
    let (answer, code) = run(&[
        "--data",
        "../../data/reviewed",
        "materials",
        "3",
        "Paldium",
        "Fragment",
    ]);
    assert_eq!(code, 0);
    assert_eq!(answer["status"], "ok");
    assert_eq!(answer["data"]["target_id"], "ITEM_PALDIUM_FRAGMENT");
    assert_eq!(
        answer["data"]["totals"][0]["required_quantity"],
        serde_json::json!(15)
    );

    let (answer, code) = run(&[
        "--data",
        "../../data/reviewed",
        "shortage",
        "--inventory",
        "Stone=6",
        "3",
        "Paldium",
        "Fragment",
    ]);
    assert_eq!(code, 0);
    assert_eq!(answer["status"], "ok");
    assert_eq!(
        answer["data"]["shortages"][0]["missing_quantity"],
        serde_json::json!(9)
    );
}

#[test]
fn cli_returns_unknown_and_version_warnings_without_fabrication() {
    let (answer, code) = run(&[
        "--data",
        "../../data/reviewed",
        "lookup",
        "item",
        "does-not-exist",
    ]);
    assert_eq!(code, 1);
    assert_eq!(answer["status"], "unknown");
    assert!(answer["data"].is_null());

    let (answer, code) = run(&[
        "--data",
        "../../data/reviewed",
        "--game-version",
        "1.0.4",
        "breeding",
        "Lamball",
        "Lamball",
    ]);
    assert_eq!(code, 1);
    assert_eq!(answer["status"], "unknown");
    assert!(answer["uncertainty"]
        .as_array()
        .expect("uncertainty is an array")
        .iter()
        .any(|message| message
            .as_str()
            .expect("message is text")
            .contains("does not match configured game version")));
}

#[test]
fn cli_reports_invalid_usage_as_error() {
    let (answer, code) = run(&[]);

    assert_eq!(code, 2);
    assert_eq!(answer["status"], "error");
    assert!(answer["errors"]
        .as_array()
        .expect("errors is an array")
        .iter()
        .any(|message| message.as_str().expect("message is text").contains("usage")));
}
