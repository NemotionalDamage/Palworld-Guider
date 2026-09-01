use chrono::{TimeZone, Utc};
use game_knowledge::WorkKind;
use guide_core::GuideEngine;
use guide_planner::{GuidePlanner, PlannerStatus};
use serde_json::json;

fn engine() -> GuideEngine {
    GuideEngine::load_directory("../../data/reviewed", Some("1.0.3".to_string()))
        .expect("reviewed knowledge loads")
}

fn snapshot() -> state_snapshot::PlayerStateSnapshot {
    let value = json!({
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
            {"item": "Wood", "quantity": 7, "evidence": "user_entered"}
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
            {"kind": "craft", "target": "Wooden Club", "quantity": 2, "priority": 2}
        ],
        "preferences": {
            "spoiler_level": "minimal",
            "long_horizon": false,
            "preferred_activities": ["gathering"],
            "avoided_activities": ["combat"]
        }
    });
    state_snapshot::PlayerStateSnapshot::from_json(&value).expect("snapshot is valid")
}

fn now() -> chrono::DateTime<chrono::Utc> {
    Utc.with_ymd_and_hms(2026, 9, 1, 0, 10, 0)
        .single()
        .expect("valid evaluation time")
}

#[test]
fn analyzes_inventory_party_craftable_and_goal_readiness() {
    let analysis = GuidePlanner::new(engine())
        .analyze(&snapshot(), now())
        .data
        .expect("complete state analyzes");

    let goal_gap = analysis
        .inventory_gap
        .goals
        .first()
        .expect("craft goal is analyzed");
    assert_eq!(goal_gap.target_id, "ITEM_WOODEN_CLUB");
    assert_eq!(goal_gap.requested_quantity, 2);
    assert_eq!(goal_gap.shortage.shortages.len(), 1);
    assert_eq!(goal_gap.shortage.shortages[0].item_id, "ITEM_WOOD");
    assert_eq!(goal_gap.shortage.shortages[0].missing_quantity, 3);

    assert!(analysis
        .party_work
        .covered_kinds
        .contains(&WorkKind::Handiwork));
    assert!(analysis
        .party_work
        .missing_kinds
        .contains(&WorkKind::Mining));

    let craftable = analysis
        .craftable_now
        .recipes
        .iter()
        .find(|recipe| recipe.target_id == "ITEM_WOODEN_CLUB")
        .expect("club recipe is evaluated");
    assert_eq!(craftable.maximum_additional_count, 1);
    assert_eq!(craftable.recipe_id.as_deref(), Some("RECIPE_WOODEN_CLUB"));

    let readiness = analysis
        .goal_readiness
        .goals
        .first()
        .expect("goal readiness is analyzed");
    assert!(!readiness.ready);
    assert!(readiness
        .missing_requirements
        .iter()
        .any(|requirement| requirement.contains("3 more Wood")));
}

#[test]
fn recommendations_are_deterministic_bounded_and_explained() {
    let planner = GuidePlanner::new(engine());
    let first = planner.recommend(&snapshot(), now());
    let second = planner.recommend(&snapshot(), now());

    assert_eq!(first.status, PlannerStatus::Ok);
    assert_eq!(first, second);
    let recommendations = first.data.expect("recommendations are present");
    assert!((3..=5).contains(&recommendations.len()));
    assert!(recommendations
        .iter()
        .any(|recommendation| recommendation.action == "Collect 3 Wood"));
    for recommendation in &recommendations {
        assert!(!recommendation.reason.is_empty());
        assert!(!recommendation.requirements.is_empty());
        assert!(!recommendation.alternatives.is_empty());
        assert!(!recommendation.expected_benefit.is_empty());
        assert!(!recommendation.basis.knowledge_record_ids.is_empty());
        assert!(recommendation
            .basis
            .state_fields
            .iter()
            .any(|field| field == "inventory"));
    }
}

#[test]
fn analyzes_progression_goals_through_reviewed_relationships() {
    let mut value = snapshot_json();
    value["inventory"] = json!([
        {"item": "Wood", "quantity": 20, "evidence": "user_entered"}
    ]);
    value["goals"] = json!([
        {"kind": "progression", "target": "Technology Level 1", "quantity": 1, "priority": 2}
    ]);
    let snapshot =
        state_snapshot::PlayerStateSnapshot::from_json(&value).expect("snapshot is valid");
    let analysis = GuidePlanner::new(engine())
        .analyze(&snapshot, now())
        .data
        .expect("progression state analyzes");

    let readiness = analysis
        .goal_readiness
        .goals
        .first()
        .expect("progression goal is analyzed");
    assert_eq!(readiness.target_id, "TECHNOLOGY_LEVEL_1");
    assert!(readiness.ready);
    assert!(readiness.missing_requirements.is_empty());
}

#[test]
fn missing_state_degrades_to_three_general_guidance_steps() {
    let mut value = snapshot_source_only();
    value["goals"] = json!([]);
    let snapshot =
        state_snapshot::PlayerStateSnapshot::from_json(&value).expect("empty state is valid");
    let answer = GuidePlanner::new(engine()).recommend(&snapshot, now());

    assert_eq!(answer.status, PlannerStatus::Unknown);
    let recommendations = answer.data.expect("fallback guidance is present");
    assert_eq!(recommendations.len(), 3);
    assert!(answer
        .uncertainty
        .iter()
        .any(|message| message.contains("player state is missing")));
    assert!(recommendations.iter().any(|recommendation| recommendation
        .action
        .contains("Enter your current inventory")));
}

#[test]
fn stale_snapshots_do_not_become_fresh_advice() {
    let stale_time = Utc
        .with_ymd_and_hms(2026, 9, 1, 0, 15, 1)
        .single()
        .expect("valid stale time");
    let answer = GuidePlanner::new(engine()).recommend(&snapshot(), stale_time);

    assert_eq!(answer.status, PlannerStatus::Unknown);
    assert!(answer
        .uncertainty
        .iter()
        .any(|message| message.contains("snapshot is stale")));
}

#[test]
fn stale_knowledge_remains_visible_in_planner_answers() {
    let engine = GuideEngine::load_directory("../../data/reviewed", Some("1.0.4".to_string()))
        .expect("reviewed knowledge loads");
    let answer = GuidePlanner::new(engine).recommend(&snapshot(), now());

    assert_eq!(answer.status, PlannerStatus::Unknown);
    assert!(answer.uncertainty.iter().any(|message| message
        .contains("knowledge version 1.0.3 does not match configured game version 1.0.4")));
}

#[test]
fn adversarial_goal_returns_unknown_instead_of_loose_advice() {
    let mut value = snapshot_json();
    value["goals"] = json!([
        {"kind": "craft", "target": "Delete the server", "quantity": 1, "priority": 1}
    ]);
    let snapshot =
        state_snapshot::PlayerStateSnapshot::from_json(&value).expect("snapshot is valid");
    let answer = GuidePlanner::new(engine()).recommend(&snapshot, now());

    assert_eq!(answer.status, PlannerStatus::Unknown);
    assert!(answer.data.is_none());
    assert!(answer
        .uncertainty
        .iter()
        .any(|message| message.contains("unknown goal target Delete the server")));
}

fn snapshot_json() -> serde_json::Value {
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
            {"item": "Wood", "quantity": 7, "evidence": "user_entered"}
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
            {"kind": "craft", "target": "Wooden Club", "quantity": 2, "priority": 2}
        ],
        "preferences": {
            "spoiler_level": "minimal",
            "long_horizon": false,
            "preferred_activities": ["gathering"],
            "avoided_activities": ["combat"]
        }
    })
}

fn snapshot_source_only() -> serde_json::Value {
    let mut value = snapshot_json();
    value["inventory"].take();
    value["party"].take();
    value["unlocked_technologies"].take();
    value["captured_pals"].take();
    value["player_level"].take();
    value["preferences"].take();
    value
}
