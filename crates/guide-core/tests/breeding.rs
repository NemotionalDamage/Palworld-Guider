use game_knowledge::{
    BreedingRuleRecord, Confidence, KnowledgeRecord, KnowledgeStore, LocaleNames, Provenance,
    ReviewStatus, SourceRecord,
};
use guide_core::{AnswerStatus, GuideEngine};

fn source() -> SourceRecord {
    SourceRecord {
        id: "SRC-TEST".to_string(),
        title: "Test source".to_string(),
        supplier: "test".to_string(),
        retrieved_on: "2026-01-01".to_string(),
        evidence_urls: vec!["https://example.test/source".to_string()],
        applicable_game_version: "1.0.0".to_string(),
        reviewer: "test".to_string(),
        review_status: ReviewStatus::Reviewed,
        confidence: Confidence::Official,
        notes: None,
    }
}

fn provenance() -> Provenance {
    Provenance {
        source_id: "SRC-TEST".to_string(),
        applicable_game_version: "1.0.0".to_string(),
        retrieved_on: "2026-01-01".to_string(),
        reviewer: "test".to_string(),
        review_status: ReviewStatus::Reviewed,
        confidence: Confidence::Official,
        change_risk: None,
    }
}

fn pal(id: &str, name: &str) -> KnowledgeRecord {
    KnowledgeRecord::Pal(game_knowledge::PalRecord {
        id: id.to_string(),
        names: LocaleNames {
            en: name.to_string(),
            zh_hans: None,
        },
        stats: None,
        work_suitability: vec![],
        drops: vec![],
        habitat_ids: vec![],
        provenance: provenance(),
    })
}

fn rule(id: &str, parent_a: &str, parent_b: &str, child: &str) -> KnowledgeRecord {
    KnowledgeRecord::BreedingRule(BreedingRuleRecord {
        id: id.to_string(),
        parent_a_id: parent_a.to_string(),
        parent_b_id: parent_b.to_string(),
        child_id: child.to_string(),
        notes: None,
        provenance: provenance(),
    })
}

fn engine(records: Vec<KnowledgeRecord>) -> GuideEngine {
    let mut all_records = vec![KnowledgeRecord::Source(source())];
    all_records.extend(records);
    let store = KnowledgeStore::from_records(all_records).expect("test records validate");
    GuideEngine::new(store, None)
}

#[test]
fn calculates_order_insensitive_breeding_result() {
    let engine = engine(vec![
        pal("PAL_A", "Alpha"),
        pal("PAL_B", "Beta"),
        pal("PAL_C", "Child"),
        rule("RULE_AB_C", "PAL_A", "PAL_B", "PAL_C"),
    ]);

    let answer = engine.calculate_breeding_result("Beta", "alpha");
    assert_eq!(answer.status, AnswerStatus::Ok);
    let result = answer.data.expect("breeding result exists");
    assert_eq!(result.parent_a_id, "PAL_A");
    assert_eq!(result.parent_b_id, "PAL_B");
    assert_eq!(result.child_id, "PAL_C");
    assert_eq!(result.rule_id, "RULE_AB_C");
    assert!(answer
        .provenance
        .iter()
        .any(|item| item.source_id == "SRC-TEST"));
}

#[test]
fn finds_shortest_deterministic_breeding_chain() {
    let engine = engine(vec![
        pal("PAL_START", "Start"),
        pal("PAL_MIDDLE", "Middle"),
        pal("PAL_OTHER", "Other"),
        pal("PAL_TARGET", "Target"),
        rule("RULE_START_MIDDLE", "PAL_START", "PAL_OTHER", "PAL_MIDDLE"),
        rule(
            "RULE_MIDDLE_TARGET",
            "PAL_MIDDLE",
            "PAL_OTHER",
            "PAL_TARGET",
        ),
    ]);

    let answer = engine.calculate_breeding_chain("Start", "Target", 4);
    assert_eq!(answer.status, AnswerStatus::Ok);
    let chain = answer.data.expect("breeding chain exists");
    assert_eq!(chain.start_id, "PAL_START");
    assert_eq!(chain.target_id, "PAL_TARGET");
    assert_eq!(chain.steps.len(), 2);
    assert_eq!(chain.steps[0].child_id, "PAL_MIDDLE");
    assert_eq!(chain.steps[1].child_id, "PAL_TARGET");
}

#[test]
fn reports_unknown_ambiguous_and_depth_limited_breeding_paths() {
    let unknown_engine = engine(vec![
        pal("PAL_A", "Alpha"),
        pal("PAL_B", "Beta"),
        pal("PAL_C", "Child"),
        pal("PAL_D", "Delta"),
    ]);
    let answer = unknown_engine.calculate_breeding_result("Alpha", "Beta");
    assert_eq!(answer.status, AnswerStatus::Unknown);
    assert!(answer
        .uncertainty
        .iter()
        .any(|message| message.contains("unknown breeding rule")));

    let ambiguous_engine = engine(vec![
        pal("PAL_A", "Alpha"),
        pal("PAL_B", "Beta"),
        pal("PAL_C", "Child"),
        pal("PAL_D", "Delta"),
        rule("RULE_AB_C", "PAL_A", "PAL_B", "PAL_C"),
        rule("RULE_AB_D", "PAL_B", "PAL_A", "PAL_D"),
    ]);
    let answer = ambiguous_engine.calculate_breeding_result("Alpha", "Beta");
    assert_eq!(answer.status, AnswerStatus::Ambiguous);
    assert!(answer
        .uncertainty
        .iter()
        .any(|message| message.contains("PAL_C")));

    let depth_engine = engine(vec![
        pal("PAL_START", "Start"),
        pal("PAL_OTHER", "Other"),
        pal("PAL_TARGET", "Target"),
        rule("RULE_START_TARGET", "PAL_START", "PAL_OTHER", "PAL_TARGET"),
    ]);
    let answer = depth_engine.calculate_breeding_chain("Start", "Target", 0);
    assert_eq!(answer.status, AnswerStatus::Error);
    assert!(answer.errors.iter().any(|error| error.contains("depth")));
}
