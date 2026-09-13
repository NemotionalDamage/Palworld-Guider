use game_knowledge::{
    Confidence, ItemRecord, KnowledgeRecord, KnowledgeStore, LocaleNames, Provenance, ReviewStatus,
    SourceRecord,
};
use guide_maintenance::version_check;

const DATA_DIRECTORY: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/reviewed");

fn synthetic_provenance() -> Provenance {
    Provenance {
        source_id: "SRC-SYNTHETIC".to_string(),
        applicable_game_version: "1.0".to_string(),
        retrieved_on: "2026-08-31".to_string(),
        reviewer: "Codex".to_string(),
        review_status: ReviewStatus::Reviewed,
        confidence: Confidence::ReviewedSecondary,
        change_risk: None,
        corroborating_source_ids: Vec::new(),
    }
}

fn synthetic_store() -> KnowledgeStore {
    let records = vec![
        KnowledgeRecord::Source(SourceRecord {
            id: "SRC-SYNTHETIC".to_string(),
            title: "Synthetic fixture".to_string(),
            supplier: "test".to_string(),
            retrieved_on: "2026-08-31".to_string(),
            evidence_urls: vec!["https://example.com".to_string()],
            applicable_game_version: "1.0".to_string(),
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
            provenance: synthetic_provenance(),
        }),
    ];
    KnowledgeStore::from_records(records).expect("synthetic fixture is valid")
}

fn synthetic_mixed_store() -> KnowledgeStore {
    let mut provenance_a = synthetic_provenance();
    provenance_a.source_id = "SRC-SYNTHETIC-A".to_string();
    provenance_a.applicable_game_version = "1.0.2".to_string();
    let mut provenance_b = synthetic_provenance();
    provenance_b.source_id = "SRC-SYNTHETIC-B".to_string();
    provenance_b.applicable_game_version = "1.0".to_string();

    let records = vec![
        KnowledgeRecord::Source(SourceRecord {
            id: "SRC-SYNTHETIC-A".to_string(),
            title: "Fixture A".to_string(),
            supplier: "test".to_string(),
            retrieved_on: "2026-08-31".to_string(),
            evidence_urls: vec!["https://example.com/a".to_string()],
            applicable_game_version: "1.0.2".to_string(),
            reviewer: "Codex".to_string(),
            review_status: ReviewStatus::Reviewed,
            confidence: Confidence::ReviewedSecondary,
            notes: None,
        }),
        KnowledgeRecord::Source(SourceRecord {
            id: "SRC-SYNTHETIC-B".to_string(),
            title: "Fixture B".to_string(),
            supplier: "test".to_string(),
            retrieved_on: "2026-08-31".to_string(),
            evidence_urls: vec!["https://example.com/b".to_string()],
            applicable_game_version: "1.0".to_string(),
            reviewer: "Codex".to_string(),
            review_status: ReviewStatus::Reviewed,
            confidence: Confidence::ReviewedSecondary,
            notes: None,
        }),
        KnowledgeRecord::Item(ItemRecord {
            id: "ITEM_A".to_string(),
            names: LocaleNames {
                en: "ItemA".to_string(),
                zh_hans: None,
            },
            description: None,
            rarity: "Common".to_string(),
            acquisition_leads: Vec::new(),
            native_row_id: None,
            local_evidence: None,
            provenance: provenance_a,
        }),
        KnowledgeRecord::Item(ItemRecord {
            id: "ITEM_B".to_string(),
            names: LocaleNames {
                en: "ItemB".to_string(),
                zh_hans: None,
            },
            description: None,
            rarity: "Common".to_string(),
            acquisition_leads: Vec::new(),
            native_row_id: None,
            local_evidence: None,
            provenance: provenance_b,
        }),
    ];
    KnowledgeStore::from_records(records).expect("synthetic mixed fixture is valid")
}

#[test]
fn version_check_with_matching_configured_version_has_no_warnings() {
    let store = synthetic_store();
    let report = version_check(&store, Some("1.0"), None);
    assert_eq!(report.knowledge_version, "1.0");
    assert!(report.game_version_matches);
    assert!(!report.has_warnings());
    assert!(report.all_match());
}

#[test]
fn version_check_with_mismatched_configured_version_warns() {
    let store = synthetic_store();
    let report = version_check(&store, Some("0.9"), None);
    assert_eq!(report.knowledge_version, "1.0");
    assert!(!report.game_version_matches);
    assert!(report.has_warnings());
    assert!(report
        .warnings
        .iter()
        .any(|w| w.contains("does not match configured game version")));
}

#[test]
fn version_check_with_none_configured_version_matches() {
    let store = synthetic_store();
    let report = version_check(&store, None, None);
    assert_eq!(report.knowledge_version, "1.0");
    assert!(report.game_version_matches);
    assert!(!report.has_warnings());
}

#[test]
fn version_check_with_mixed_versions_warns() {
    let store = synthetic_mixed_store();
    let report = version_check(&store, Some("1.0"), None);
    assert_eq!(report.knowledge_version, "mixed");
    assert!(!report.game_version_matches);
    assert!(report.has_warnings());
    assert!(report
        .warnings
        .iter()
        .any(|w| w.contains("mixed game versions")));
}

#[test]
fn version_check_includes_index_version_when_provided() {
    let store = synthetic_store();
    let report = version_check(&store, Some("1.0"), Some("1.0"));
    assert_eq!(report.index_version, Some("1.0".to_string()));
    assert!(report.all_match());
}

#[test]
fn version_check_with_mismatched_index_version_warns() {
    let store = synthetic_store();
    let report = version_check(&store, Some("1.0"), Some("0.8"));
    assert!(report
        .warnings
        .iter()
        .any(|w| w.contains("index version") && w.contains("does not match")));
}

#[test]
fn version_check_embedding_version_is_none() {
    let store = synthetic_store();
    let report = version_check(&store, None, None);
    assert_eq!(report.embedding_version, None);
}

#[test]
fn version_check_on_canonical_dataset() {
    let store = KnowledgeStore::load_directory(DATA_DIRECTORY)
        .expect("canonical reviewed dataset is valid");
    let report = version_check(&store, Some("1.0"), None);
    assert_eq!(report.knowledge_version, "1.0");
    assert!(report.game_version_matches);
    assert!(!report.has_warnings());
}

#[test]
fn version_check_on_canonical_dataset_with_wrong_version_warns() {
    let store = KnowledgeStore::load_directory(DATA_DIRECTORY)
        .expect("canonical reviewed dataset is valid");
    let report = version_check(&store, Some("0.1"), None);
    assert!(!report.game_version_matches);
    assert!(report.has_warnings());
}
