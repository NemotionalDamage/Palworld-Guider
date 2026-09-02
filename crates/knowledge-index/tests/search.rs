use game_knowledge::{
    AliasRecord, Confidence, ItemRecord, KnowledgeRecord, KnowledgeStore, LocaleNames, Provenance,
    ReviewStatus, SourceRecord,
};
use knowledge_index::{IndexStatus, KnowledgeIndex};

const DATA_DIRECTORY: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/reviewed");

fn canonical_store() -> KnowledgeStore {
    KnowledgeStore::load_directory(DATA_DIRECTORY).expect("canonical reviewed dataset is valid")
}

fn synthetic_provenance() -> Provenance {
    Provenance {
        source_id: "SRC-SYNTHETIC".to_string(),
        applicable_game_version: "1.0.3".to_string(),
        retrieved_on: "2026-08-31".to_string(),
        reviewer: "Codex".to_string(),
        review_status: ReviewStatus::Reviewed,
        confidence: Confidence::ReviewedSecondary,
        change_risk: None,
        corroborating_source_ids: Vec::new(),
    }
}

fn synthetic_alias_store() -> KnowledgeStore {
    let records = vec![
        KnowledgeRecord::Source(SourceRecord {
            id: "SRC-SYNTHETIC".to_string(),
            title: "Synthetic index fixture".to_string(),
            supplier: "test".to_string(),
            retrieved_on: "2026-08-31".to_string(),
            evidence_urls: vec!["https://example.com/fixture".to_string()],
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
            provenance: synthetic_provenance(),
        }),
        KnowledgeRecord::Alias(AliasRecord {
            id: "ALIAS_WOOD_ZH".to_string(),
            alias: "木材".to_string(),
            target_id: "ITEM_WOOD".to_string(),
            locale: "zh_hans".to_string(),
            provenance: synthetic_provenance(),
        }),
    ];
    KnowledgeStore::from_records(records).expect("synthetic fixture is valid")
}

#[test]
fn search_wood_returns_reviewed_items_with_provenance() {
    let store = canonical_store();
    let index = KnowledgeIndex::from_store(&store, None).expect("index builds");
    let answer = index.search("wood", 5);
    assert_eq!(answer.status, IndexStatus::Ok);
    let ids: Vec<&str> = answer
        .results
        .iter()
        .map(|r| r.record_id.as_str())
        .collect();
    assert!(ids.contains(&"ITEM_WOOD"));
    assert!(ids.contains(&"ITEM_WOODEN_CLUB"));
    let wood = answer
        .results
        .iter()
        .find(|result| result.record_id == "ITEM_WOOD")
        .expect("Wood result exists");
    assert_eq!(wood.provenance.source_id, "SRC-PALDB-V1_0_3-20260831");
    assert_eq!(wood.provenance.applicable_game_version, "1.0.3");
    assert_eq!(wood.provenance.review_status, "reviewed");
    assert_eq!(wood.provenance.confidence, "reviewed_secondary");
    assert!(wood.summary.contains("Chop trees"));
}

#[test]
fn exact_title_ranks_ahead_of_partial_match() {
    let store = canonical_store();
    let index = KnowledgeIndex::from_store(&store, None).expect("index builds");
    let answer = index.search("wood", 5);
    let ids: Vec<&str> = answer
        .results
        .iter()
        .map(|r| r.record_id.as_str())
        .collect();
    let wood_position = ids
        .iter()
        .position(|id| *id == "ITEM_WOOD")
        .expect("Wood result exists");
    let club_position = ids
        .iter()
        .position(|id| *id == "ITEM_WOODEN_CLUB")
        .expect("Wooden Club result exists");
    assert_eq!(ids.first(), Some(&"ITEM_WOOD"));
    assert!(wood_position < club_position);
}

#[test]
fn search_respects_result_limit() {
    let store = canonical_store();
    let index = KnowledgeIndex::from_store(&store, None).expect("index builds");
    let answer = index.search("wood", 1);
    assert_eq!(answer.status, IndexStatus::Ok);
    assert_eq!(answer.results.len(), 1);
}

#[test]
fn unknown_query_returns_unknown() {
    let store = canonical_store();
    let index = KnowledgeIndex::from_store(&store, None).expect("index builds");
    let answer = index.search("xyzzy", 5);
    assert_eq!(answer.status, IndexStatus::Unknown);
    assert!(answer.results.is_empty());
    assert!(answer
        .uncertainty
        .iter()
        .any(|message| message.contains("no reviewed records matched")));
}

#[test]
fn chinese_alias_is_searchable() {
    let store = synthetic_alias_store();
    let index = KnowledgeIndex::from_store(&store, None).expect("index builds");
    let answer = index.search("木材", 5);
    assert_eq!(answer.status, IndexStatus::Ok);
    assert!(answer
        .results
        .iter()
        .any(|result| result.record_id == "ITEM_WOOD"));
}

#[test]
fn configured_version_mismatch_adds_stale_warning() {
    let store = canonical_store();
    let index = KnowledgeIndex::from_store(&store, Some("0.9".to_string())).expect("index builds");
    let answer = index.search("wood", 5);
    assert_eq!(answer.status, IndexStatus::Ok);
    assert_eq!(answer.version.knowledge_version, "1.0.3");
    assert_eq!(
        answer.version.configured_game_version.as_deref(),
        Some("0.9")
    );
    assert!(!answer.version.matches);
    assert!(answer
        .uncertainty
        .iter()
        .any(|message| message.contains("does not match configured game version 0.9")));
}
