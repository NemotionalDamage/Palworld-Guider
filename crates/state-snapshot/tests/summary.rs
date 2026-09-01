use chrono::{TimeZone, Utc};
use serde_json::json;
use state_snapshot::{
    PlayerStateSnapshot, SnapshotFreshness, SnapshotSummary, SnapshotSummaryOptions,
};

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
            {"item": "Wood", "quantity": 20, "evidence": "user_entered"},
            {"item": "Wool", "quantity": 3, "evidence": "observed"}
        ],
        "party": [
            {"slot": 1, "pal": "Lamball", "evidence": "user_entered"},
            {"slot": 0, "pal": "Cattiva", "evidence": "user_entered"}
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

fn time(minutes: u32, seconds: u32) -> chrono::DateTime<chrono::Utc> {
    Utc.with_ymd_and_hms(2026, 9, 1, 0, minutes, seconds)
        .single()
        .expect("valid evaluation time")
}

#[test]
fn evaluates_freshness_inclusively_and_reports_missing_ttl() {
    let snapshot = PlayerStateSnapshot::from_json(&complete_snapshot()).expect("valid snapshot");
    assert_eq!(snapshot.freshness(time(15, 0)), SnapshotFreshness::Fresh);
    assert_eq!(snapshot.freshness(time(15, 1)), SnapshotFreshness::Stale);

    let mut value = complete_snapshot();
    value["source"]["time_to_live_seconds"].take();
    let snapshot = PlayerStateSnapshot::from_json(&value).expect("valid snapshot");
    assert_eq!(
        snapshot.freshness(time(15, 0)),
        SnapshotFreshness::UnknownTtl
    );
}

#[test]
fn summarizes_only_question_relevant_state_without_private_source_fields() {
    let snapshot = PlayerStateSnapshot::from_json(&complete_snapshot()).expect("valid snapshot");
    let question = "What can I craft with Wood, and can Lamball cover party work?";
    let summary = snapshot.summarize(
        question,
        &SnapshotSummaryOptions::for_question(question),
        time(10, 0),
    );

    assert_eq!(summary.freshness, SnapshotFreshness::Fresh);
    assert_eq!(summary.source_kind, "user_entered");
    assert_eq!(
        summary
            .inventory
            .as_ref()
            .expect("inventory is relevant")
            .len(),
        2
    );
    assert_eq!(summary.party.as_ref().expect("party is relevant").len(), 2);
    assert!(summary.goals.is_none());
    assert!(summary.preferences.is_none());
    assert!(summary.evidence_kinds.contains("user_entered"));
    assert!(summary.evidence_kinds.contains("observed"));

    let serialized = serde_json::to_string(&summary).expect("summary is serializable");
    assert!(!serialized.contains("operator-local-session"));
    assert!(!serialized.contains("consent"));
    assert!(!serialized.contains("captured_at"));
    assert!(!serialized.contains("granted_at"));
}

#[test]
fn include_all_returns_stable_order_and_completeness() {
    let mut value = complete_snapshot();
    value["captured_pals"].take();
    value["player_level"].take();
    value["goals"].take();
    value["preferences"].take();
    let snapshot = PlayerStateSnapshot::from_json(&value).expect("valid snapshot");

    let completeness = snapshot.completeness();
    assert_eq!(
        completeness.missing_fields,
        vec!["captured_pals", "goals", "player_level", "preferences"]
    );

    let summary = snapshot.summarize(
        "summarize player state",
        &SnapshotSummaryOptions::include_all(),
        time(10, 0),
    );
    assert_eq!(
        summary
            .inventory
            .as_ref()
            .expect("inventory is included")
            .first()
            .expect("inventory is sorted")
            .item,
        "Wood"
    );
    assert_eq!(
        summary
            .party
            .as_ref()
            .expect("party is included")
            .first()
            .expect("party is slot sorted")
            .pal,
        "Cattiva"
    );
    assert_eq!(summary.missing_fields, completeness.missing_fields);
}

#[test]
fn stale_snapshot_remains_visible_as_stale() {
    let snapshot = PlayerStateSnapshot::from_json(&complete_snapshot()).expect("valid snapshot");
    let summary = snapshot.summarize(
        "What do I have?",
        &SnapshotSummaryOptions::include_all(),
        time(15, 1),
    );
    assert_eq!(summary.freshness, SnapshotFreshness::Stale);
    assert_eq!(
        summary,
        SnapshotSummary {
            freshness: SnapshotFreshness::Stale,
            ..summary.clone()
        }
    );
}
