use guide_agent::AgentAnswer;
use guide_core::VersionInfo;
use guide_server::{ExchangeRecord, ServerLimits, SessionError, SessionStore};
use serde_json::json;
use state_snapshot::PlayerStateSnapshot;
use std::time::{Duration, Instant};

fn start() -> Instant {
    Instant::now()
}

fn answer(text: &str) -> AgentAnswer {
    AgentAnswer {
        status: guide_agent::AgentStatus::Ok,
        answer: Some(text.to_string()),
        tool_calls: Vec::new(),
        provenance: Vec::new(),
        version: VersionInfo {
            knowledge_version: "reviewed-v1".to_string(),
            configured_game_version: None,
            matches: true,
        },
        uncertainty: Vec::new(),
        errors: Vec::new(),
    }
}

fn snapshot() -> PlayerStateSnapshot {
    let value = json!({
        "schema_version": "state_snapshot_v1",
        "source": {
            "kind": "user_entered",
            "captured_at": chrono::Utc::now() - chrono::Duration::seconds(1),
            "game_version": "1.0.3",
            "time_to_live_seconds": 60,
            "consent": {
                "id": "operator-local-session",
                "scope": ["guide_advice"],
                "granted_at": chrono::Utc::now() - chrono::Duration::seconds(1)
            }
        },
        "inventory": [
            {"item": "Wood", "quantity": 7, "evidence": "user_entered"}
        ]
    });
    PlayerStateSnapshot::from_json(&value).expect("snapshot is valid")
}

fn complete_ask(
    store: &mut SessionStore,
    session_id: &str,
    question: &str,
    text: &str,
    _now: Instant,
) {
    store
        .complete_ask(session_id, question, answer(text))
        .expect("ask completes");
}

#[test]
fn creates_an_opaque_empty_bounded_session() {
    let mut store = SessionStore::new(ServerLimits::default());
    let start = start();
    let record = store.create_session(start).expect("session is created");

    assert_eq!(record.id.len(), 32);
    assert!(u128::from_str_radix(&record.id, 16).is_ok());
    assert_eq!(record.created_at, start);
    assert_eq!(record.expires_at, start + Duration::from_secs(1800));
    assert!(record.exchanges.is_empty());
    assert_eq!(record.snapshot, None);
}

#[test]
fn follow_up_history_is_bounded() {
    let mut store = SessionStore::new(ServerLimits::default());
    let start = start();
    let session = store.create_session(start).expect("session is created");

    for round in 0..6 {
        let now = start + Duration::from_secs(round);
        let question = format!("question {round}");
        let prompt = store
            .reserve_ask(&session.id, &question, now)
            .expect("ask is reserved");
        assert!(prompt.contains(&question));
        complete_ask(
            &mut store,
            &session.id,
            &question,
            &format!("answer {round}"),
            now,
        );
    }

    let prompt = store
        .reserve_ask(&session.id, "question 6", start + Duration::from_secs(6))
        .expect("ask is reserved");
    assert!(!prompt.contains("question 0"));
    assert!(!prompt.contains("answer 0"));
    assert!(prompt.contains("question 2"));
    assert!(prompt.contains("answer 2"));
    assert!(prompt.contains("question 6"));

    let record = store
        .session(&session.id, start + Duration::from_secs(6))
        .expect("session exists");
    assert_eq!(record.exchanges.len(), 4);
    assert_eq!(record.exchanges[0].question, "question 2");
}

#[test]
fn rolling_ask_rate_window_is_enforced_and_expires() {
    let limits = ServerLimits {
        max_asks_per_minute: 2,
        ..ServerLimits::default()
    };
    let mut store = SessionStore::new(limits);
    let start = start();
    let session = store.create_session(start).expect("session is created");

    store
        .reserve_ask(&session.id, "one", start)
        .expect("first ask is allowed");
    store
        .reserve_ask(&session.id, "two", start)
        .expect("second ask is allowed");
    let error = store
        .reserve_ask(&session.id, "three", start)
        .expect_err("third ask is rate limited");
    assert_eq!(
        error,
        SessionError::AskRateLimited {
            retry_after_secs: 60
        }
    );

    store
        .reserve_ask(&session.id, "later", start + Duration::from_secs(61))
        .expect("rate window expires");
}

#[test]
fn rolling_snapshot_rate_window_is_enforced_and_expires() {
    let limits = ServerLimits {
        max_snapshots_per_minute: 1,
        ..ServerLimits::default()
    };
    let mut store = SessionStore::new(limits);
    let start = start();
    let session = store.create_session(start).expect("session is created");
    let first_snapshot = snapshot();

    store
        .attach_snapshot(&session.id, first_snapshot.clone(), start)
        .expect("first snapshot is allowed");
    let error = store
        .attach_snapshot(&session.id, first_snapshot, start)
        .expect_err("second snapshot is rate limited");
    assert_eq!(
        error,
        SessionError::SnapshotRateLimited {
            retry_after_secs: 60
        }
    );

    store
        .attach_snapshot(&session.id, snapshot(), start + Duration::from_secs(61))
        .expect("snapshot window expires");
}

#[test]
fn expired_sessions_fail_and_are_removable() {
    let mut store = SessionStore::new(ServerLimits::default());
    let start = start();
    let session = store.create_session(start).expect("session is created");
    let expired_at = start + Duration::from_secs(1801);

    assert_eq!(
        store.session(&session.id, expired_at),
        Err(SessionError::Expired)
    );
    assert_eq!(
        store.reserve_ask(&session.id, "late", expired_at),
        Err(SessionError::Expired)
    );
    assert_eq!(
        store.attach_snapshot(&session.id, snapshot(), expired_at),
        Err(SessionError::Expired)
    );
    assert_eq!(store.remove_expired(expired_at), vec![session.id.clone()]);
    assert_eq!(
        store.session(&session.id, expired_at),
        Err(SessionError::NotFound)
    );
}

#[test]
fn snapshot_metadata_is_safe_and_complete() {
    let mut store = SessionStore::new(ServerLimits::default());
    let start = start();
    let session = store.create_session(start).expect("session is created");
    let metadata = store
        .attach_snapshot(&session.id, snapshot(), start)
        .expect("snapshot attaches");

    assert_eq!(metadata.schema_version, "state_snapshot_v1");
    assert_eq!(metadata.source_kind, "user_entered");
    assert_eq!(metadata.game_version, "1.0.3");
    assert_eq!(metadata.freshness, state_snapshot::SnapshotFreshness::Fresh);
    assert_eq!(
        metadata.missing_fields,
        vec![
            "captured_pals".to_string(),
            "goals".to_string(),
            "party".to_string(),
            "player_level".to_string(),
            "preferences".to_string(),
            "unlocked_technologies".to_string()
        ]
    );
    let value = serde_json::to_value(&metadata).expect("metadata serializes");
    assert_eq!(
        value,
        json!({
            "schema_version": "state_snapshot_v1",
            "source_kind": "user_entered",
            "game_version": "1.0.3",
            "freshness": "fresh",
            "missing_fields": [
                "captured_pals", "goals", "party", "player_level", "preferences", "unlocked_technologies"
            ]
        })
    );
    let serialized = value.to_string();
    assert!(!serialized.contains("operator-local-session"));
    assert!(!serialized.contains("consent"));
    assert!(!serialized.contains("captured_at"));
    assert!(!serialized.contains("Wood"));
}

#[test]
fn exchange_records_keep_bounded_question_and_answer_text() {
    let record = ExchangeRecord::new("What is Wood?", answer("Wood is a material."));
    assert_eq!(record.question, "What is Wood?");
    assert_eq!(record.answer.answer.as_deref(), Some("Wood is a material."));
}
