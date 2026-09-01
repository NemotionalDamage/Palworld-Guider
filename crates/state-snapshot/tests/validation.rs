use serde_json::json;
use state_snapshot::{PlayerStateSnapshot, SnapshotValidation, SnapshotValidator};

fn complete_snapshot() -> serde_json::Value {
    json!({
        "schema_version": "state_snapshot_v1",
        "source": {
            "kind": "user_entered",
            "captured_at": "2026-09-01T00:00:00Z",
            "game_version": "1.0.3",
            "time_to_live_seconds": 900,
            "consent": {
                "id": "operator-local-session",
                "scope": ["guide_advice"],
                "granted_at": "2026-09-01T00:00:00Z"
            }
        },
        "inventory": [
            {"item": "Wood", "quantity": 20, "evidence": "user_entered"}
        ],
        "party": [
            {"slot": 0, "pal": "Lamball", "evidence": "user_entered"}
        ],
        "unlocked_technologies": [
            {"technology": "Technology Level 1", "evidence": "user_entered"}
        ],
        "captured_pals": [
            {"pal": "Lamball", "level": 5, "evidence": "user_entered"}
        ],
        "player_level": {"value": 7, "evidence": "user_entered"},
        "goals": [
            {"kind": "craft", "target": "Wooden Club", "quantity": 1, "priority": 2}
        ],
        "preferences": {
            "spoiler_level": "minimal",
            "long_horizon": false,
            "preferred_activities": ["gathering"],
            "avoided_activities": ["combat"]
        }
    })
}

fn now() -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339("2026-09-01T00:10:00Z")
        .expect("valid evaluation time")
        .with_timezone(&chrono::Utc)
}

#[test]
fn accepts_a_complete_user_entered_snapshot() {
    let snapshot =
        PlayerStateSnapshot::from_json(&complete_snapshot()).expect("complete snapshot is valid");
    let validation = SnapshotValidator.validate(&snapshot, now());

    assert_eq!(
        validation,
        SnapshotValidation {
            valid: true,
            errors: Vec::new(),
            missing_fields: Vec::new()
        }
    );
}

#[test]
fn rejects_invalid_json_and_unknown_fields() {
    let invalid = json!({"schema_version": "state_snapshot_v1"});
    let error = PlayerStateSnapshot::from_json(&invalid).expect_err("required fields are absent");
    assert!(error.to_string().contains("missing field source"));

    let mut unknown_field = complete_snapshot();
    unknown_field["secret_note"] = json!("do not expose");
    let error =
        PlayerStateSnapshot::from_json(&unknown_field).expect_err("unknown fields are rejected");
    assert!(error.to_string().contains("unknown field secret_note"));
}

#[test]
fn collects_semantic_errors_without_stopping_at_the_first_error() {
    let mut value = complete_snapshot();
    value["source"]["kind"] = json!("server_api");
    value["source"]["captured_at"] = json!("2026-09-01T01:00:00Z");
    value["source"]["consent"]["id"] = json!(" ");
    value["inventory"] = json!([
        {"item": "Wood", "quantity": 20, "evidence": "user_entered"},
        {"item": "Wood", "quantity": 5, "evidence": "user_entered"},
        {"item": "Wool", "quantity": 0, "evidence": "user_entered"}
    ]);
    value["party"] = json!([
        {"slot": 0, "pal": "Lamball", "evidence": "user_entered"},
        {"slot": 0, "pal": "Cattiva", "evidence": "user_entered"}
    ]);

    let snapshot = PlayerStateSnapshot::from_json(&value).expect("shape remains deserializable");
    let validation = SnapshotValidator.validate(&snapshot, now());

    assert!(!validation.valid);
    assert!(validation
        .errors
        .iter()
        .any(|error| error.contains("unsupported source kind")));
    assert!(validation
        .errors
        .iter()
        .any(|error| error.contains("capture time is in the future")));
    assert!(validation
        .errors
        .iter()
        .any(|error| error.contains("consent id")));
    assert!(validation
        .errors
        .iter()
        .any(|error| error.contains("duplicate inventory item")));
    assert!(validation
        .errors
        .iter()
        .any(|error| error.contains("quantity must be greater than zero")));
    assert!(validation
        .errors
        .iter()
        .any(|error| error.contains("duplicate party slot")));
}

#[test]
fn rejects_invalid_goals_levels_and_conflicting_preferences() {
    let mut value = complete_snapshot();
    value["captured_pals"] = json!([
        {"pal": "Lamball", "level": 0, "evidence": "user_entered"}
    ]);
    value["player_level"] = json!({"value": 0, "evidence": "user_entered"});
    value["goals"] = json!([
        {"kind": "teleport", "target": "unknown place", "quantity": 1, "priority": 9}
    ]);
    value["preferences"] = json!({
        "spoiler_level": "full_story",
        "long_horizon": true,
        "preferred_activities": ["combat"],
        "avoided_activities": ["combat"]
    });

    let snapshot = PlayerStateSnapshot::from_json(&value).expect("shape remains deserializable");
    let validation = SnapshotValidator.validate(&snapshot, now());

    assert!(!validation.valid);
    assert!(validation
        .errors
        .iter()
        .any(|error| error.contains("Pal level")));
    assert!(validation
        .errors
        .iter()
        .any(|error| error.contains("player level")));
    assert!(validation
        .errors
        .iter()
        .any(|error| error.contains("unsupported goal kind")));
    assert!(validation
        .errors
        .iter()
        .any(|error| error.contains("priority")));
    assert!(validation
        .errors
        .iter()
        .any(|error| error.contains("spoiler level")));
    assert!(validation
        .errors
        .iter()
        .any(|error| error.contains("conflicting preference")));
}

#[test]
fn reports_missing_optional_state_sections_without_fabricating_them() {
    let mut value = complete_snapshot();
    value["inventory"].take();
    value["party"].take();
    value["unlocked_technologies"].take();
    value["captured_pals"].take();
    value["player_level"].take();
    value["goals"].take();
    value["preferences"].take();

    let snapshot = PlayerStateSnapshot::from_json(&value).expect("empty state sections are valid");
    let validation = SnapshotValidator.validate(&snapshot, now());

    assert!(validation.valid);
    assert_eq!(
        validation.missing_fields,
        vec![
            "captured_pals",
            "goals",
            "inventory",
            "party",
            "player_level",
            "preferences",
            "unlocked_technologies"
        ]
    );
}
