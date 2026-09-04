use game_knowledge::{
    Confidence, ConflictRecord, ConflictResolution, ItemRecord, KnowledgeRecord, KnowledgeStore,
    LocaleNames, Provenance, ReviewStatus, SourceRecord,
};
use guide_maintenance::KnowledgeAudit;

const DATA_DIRECTORY: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/reviewed");

fn synthetic_provenance(version: &str) -> Provenance {
    Provenance {
        source_id: "SRC-SYNTHETIC".to_string(),
        applicable_game_version: version.to_string(),
        retrieved_on: "2026-08-31".to_string(),
        reviewer: "Codex".to_string(),
        review_status: ReviewStatus::Reviewed,
        confidence: Confidence::ReviewedSecondary,
        change_risk: None,
        corroborating_source_ids: Vec::new(),
    }
}

fn synthetic_store_with_conflict() -> KnowledgeStore {
    let records = vec![
        KnowledgeRecord::Source(SourceRecord {
            id: "SRC-SYNTHETIC".to_string(),
            title: "Synthetic fixture".to_string(),
            supplier: "test".to_string(),
            retrieved_on: "2026-08-31".to_string(),
            evidence_urls: vec!["https://example.com".to_string()],
            applicable_game_version: "1.0.3".to_string(),
            reviewer: "Codex".to_string(),
            review_status: ReviewStatus::Reviewed,
            confidence: Confidence::ReviewedSecondary,
            notes: None,
        }),
        KnowledgeRecord::Item(ItemRecord {
            id: "ITEM_WOOD".to_string(),
            names: LocaleNames {
                en: "Wood".to_string(),
                zh_hans: None,
            },
            description: None,
            rarity: "Common".to_string(),
            acquisition_leads: Vec::new(),
            native_row_id: None,
            local_evidence: None,
            provenance: synthetic_provenance("1.0.3"),
        }),
        KnowledgeRecord::Conflict(ConflictRecord {
            id: "CONFLICT_TEST".to_string(),
            subject_id: "ITEM_WOOD".to_string(),
            field: "rarity".to_string(),
            values: vec!["Common".to_string(), "Rare".to_string()],
            source_ids: vec!["SRC-SYNTHETIC".to_string()],
            resolution: ConflictResolution::Unresolved,
            provenance: synthetic_provenance("1.0.3"),
        }),
    ];
    KnowledgeStore::from_records(records).expect("synthetic fixture is valid")
}

#[test]
fn audit_sources_returns_all_sources_with_fact_counts() {
    let store = synthetic_store_with_conflict();
    let summaries = KnowledgeAudit::audit_sources(&store);
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].id, "SRC-SYNTHETIC");
    assert!(summaries[0].fact_count >= 2);
    assert_eq!(summaries[0].applicable_game_version, "1.0.3");
    assert_eq!(summaries[0].review_status, "reviewed");
    assert_eq!(summaries[0].confidence, "reviewed_secondary");
}

#[test]
fn audit_conflicts_returns_all_conflicts_with_resolution() {
    let store = synthetic_store_with_conflict();
    let summaries = KnowledgeAudit::audit_conflicts(&store);
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].id, "CONFLICT_TEST");
    assert_eq!(summaries[0].subject_id, "ITEM_WOOD");
    assert_eq!(summaries[0].resolution, "unresolved");
    assert_eq!(summaries[0].values.len(), 2);
}

#[test]
fn audit_stale_finds_mismatched_records() {
    let store = synthetic_store_with_conflict();
    let stale = KnowledgeAudit::audit_stale(&store, "1.0.4");
    assert!(!stale.is_empty());
    assert!(stale.iter().any(|r| r.record_id == "ITEM_WOOD"));
    assert!(stale.iter().any(|r| r.record_id == "CONFLICT_TEST"));
}

#[test]
fn audit_stale_returns_empty_when_all_match() {
    let store = synthetic_store_with_conflict();
    let stale = KnowledgeAudit::audit_stale(&store, "1.0.3");
    assert!(stale.is_empty());
}

#[test]
fn audit_sources_on_canonical_dataset() {
    let store = KnowledgeStore::load_directory(DATA_DIRECTORY)
        .expect("canonical reviewed dataset is valid");
    let summaries = KnowledgeAudit::audit_sources(&store);
    assert!(summaries.len() >= 2);
    assert!(summaries
        .iter()
        .any(|s| s.id == "SRC-PALDB-V1_0_3-20260831"));
    assert!(summaries
        .iter()
        .any(|s| s.id == "SRC-LOCAL-BUILD-24575825-20260902"));
}

#[test]
fn audit_conflicts_on_canonical_dataset() {
    let store = KnowledgeStore::load_directory(DATA_DIRECTORY)
        .expect("canonical reviewed dataset is valid");
    let summaries = KnowledgeAudit::audit_conflicts(&store);
    assert!(summaries.len() >= 2);
    assert!(summaries
        .iter()
        .any(|c| c.id == "CONFLICT_ITEM_WOODEN_CLUB_PRODUCT_IDENTITY"));
    assert!(summaries.iter().all(|c| c.resolution == "unresolved"));
}

#[test]
fn validate_batch_accepts_valid_records() {
    let lines = vec![
        r#"{"record_type":"source","id":"SRC-TEST","title":"Test","supplier":"test","retrieved_on":"2026-08-31","evidence_urls":["https://example.com"],"applicable_game_version":"1.0.3","reviewer":"Codex","review_status":"reviewed","confidence":"reviewed_secondary","notes":null}"#,
        r#"{"record_type":"item","id":"ITEM_TEST","names":{"en":"Test","zh_hans":null},"description":null,"rarity":"Common","acquisition_leads":[],"provenance":{"source_id":"SRC-TEST","applicable_game_version":"1.0.3","retrieved_on":"2026-08-31","reviewer":"Codex","review_status":"reviewed","confidence":"reviewed_secondary"}}"#,
    ];
    let validation = KnowledgeAudit::validate_batch(&lines);
    assert!(validation.valid);
    assert_eq!(validation.total_records, 2);
    assert!(validation.errors.is_empty());
}

#[test]
fn validate_batch_rejects_invalid_records() {
    let lines = vec![
        r#"{"record_type":"source","id":"SRC-TEST","title":"Test","supplier":"test","retrieved_on":"2026-08-31","evidence_urls":["https://example.com"],"applicable_game_version":"1.0.3","reviewer":"Codex","review_status":"reviewed","confidence":"reviewed_secondary","notes":null}"#,
        r#"{"record_type":"item","id":"ITEM_BAD","names":{"en":"","zh_hans":null},"description":null,"rarity":"Common","acquisition_leads":[],"provenance":{"source_id":"SRC-TEST","applicable_game_version":"1.0.3","retrieved_on":"2026-08-31","reviewer":"Codex","review_status":"reviewed","confidence":"reviewed_secondary"}}"#,
    ];
    let validation = KnowledgeAudit::validate_batch(&lines);
    assert!(!validation.valid);
    assert!(!validation.errors.is_empty());
}

#[test]
fn validate_batch_rejects_unparseable_json() {
    let lines = vec!["not valid json"];
    let validation = KnowledgeAudit::validate_batch(&lines);
    assert!(!validation.valid);
    assert!(validation.errors.iter().any(|e| e.contains("line 1")));
}

#[test]
fn validate_batch_skips_empty_lines() {
    let lines = vec![
        "",
        "  ",
        r#"{"record_type":"source","id":"SRC-TEST","title":"Test","supplier":"test","retrieved_on":"2026-08-31","evidence_urls":["https://example.com"],"applicable_game_version":"1.0.3","reviewer":"Codex","review_status":"reviewed","confidence":"reviewed_secondary","notes":null}"#,
    ];
    let validation = KnowledgeAudit::validate_batch(&lines);
    assert_eq!(validation.total_records, 1);
}
