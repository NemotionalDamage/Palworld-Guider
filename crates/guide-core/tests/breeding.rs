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
        corroborating_source_ids: Vec::new(),
    }
}

fn pal(id: &str, name: &str) -> KnowledgeRecord {
    KnowledgeRecord::Pal(game_knowledge::PalRecord {
        id: id.to_string(),
        names: LocaleNames {
            en: name.to_string(),
            zh_hans: None,
        },
        work_suitability: vec![],
        drops: vec![],
        habitat_ids: vec![],
        habitat_leads: vec![],
        wild_spawn_review: None,
        element_type1: None,
        element_type2: None,
        breeding_combi_rank: None,
        breeding_combi_priority: None,
        breeding_ignore_combi: false,
        breeding_self_only: false,
        native_row_id: None,
        local_evidence: None,
        provenance: provenance(),
    })
}

fn ranked_pal(id: &str, name: &str, combi_rank: u32, combi_priority: u32) -> KnowledgeRecord {
    let mut value = serde_json::to_value(pal(id, name)).expect("Pal record serializes");
    value["breeding_combi_rank"] = serde_json::json!(combi_rank);
    value["breeding_combi_priority"] = serde_json::json!(combi_priority);
    serde_json::from_value(value).expect("ranked Pal record deserializes")
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

fn rule_with_notes(
    id: &str,
    parent_a: &str,
    parent_b: &str,
    child: &str,
    notes: &str,
) -> KnowledgeRecord {
    let mut value =
        serde_json::to_value(rule(id, parent_a, parent_b, child)).expect("rule serializes");
    value["notes"] = serde_json::json!(notes);
    serde_json::from_value(value).expect("annotated rule deserializes")
}

#[test]
fn reports_gender_dependent_unique_combination_with_both_outcomes() {
    let engine = engine(vec![
        pal("PAL_CATMAGE", "Katress"),
        pal("PAL_FOXMAGE", "Wixen"),
        pal("PAL_CATMAGE_FIRE", "Katress Ignis"),
        pal("PAL_FOXMAGE_DARK", "Wixen Noct"),
        rule_with_notes(
            "RULE_KATRESS_FEMALE",
            "PAL_CATMAGE",
            "PAL_FOXMAGE",
            "PAL_CATMAGE_FIRE",
            "Gender-dependent unique combination: Katress \u{2640} + Wixen \u{2642} = Katress Ignis.",
        ),
        rule_with_notes(
            "RULE_KATRESS_MALE",
            "PAL_CATMAGE",
            "PAL_FOXMAGE",
            "PAL_FOXMAGE_DARK",
            "Gender-dependent unique combination: Katress \u{2642} + Wixen \u{2640} = Wixen Noct.",
        ),
    ]);

    let answer = engine.calculate_breeding_result("Katress", "Wixen");
    assert_eq!(answer.status, AnswerStatus::Ambiguous);
    let message = answer.uncertainty.join(" ");
    assert!(message.contains("Katress Ignis [PAL_CATMAGE_FIRE]"));
    assert!(message.contains("Wixen Noct [PAL_FOXMAGE_DARK]"));
    assert!(message.contains("PAL_CATMAGE_FIRE"));
    assert!(message.contains("PAL_FOXMAGE_DARK"));
    assert!(message.contains("Katress \u{2640} + Wixen \u{2642} = Katress Ignis"));
    assert!(message.contains("Katress \u{2642} + Wixen \u{2640} = Wixen Noct"));

    let chain = engine
        .calculate_breeding_chain("Katress", "Wixen Noct", 1)
        .data
        .expect("gender-dependent chain is reachable");
    assert_eq!(chain.steps.len(), 1);
    assert_eq!(chain.steps[0].child_id, "PAL_FOXMAGE_DARK");
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
fn calculates_normal_breeding_result_from_combi_ranks() {
    let engine = engine(vec![
        ranked_pal("PAL_A", "Alpha", 1000, 100_000),
        ranked_pal("PAL_B", "Beta", 1500, 150_000),
        ranked_pal("PAL_LOWER", "Lower Child", 1240, 124_000),
        ranked_pal("PAL_TARGET", "Target Child", 1250, 125_000),
        ranked_pal("PAL_UPPER", "Upper Child", 1260, 126_000),
    ]);

    let answer = engine.calculate_breeding_result("Beta", "alpha");
    assert_eq!(answer.status, AnswerStatus::Ok);
    let result = answer.data.expect("breeding result exists");
    assert_eq!(result.parent_a_id, "PAL_B");
    assert_eq!(result.parent_b_id, "PAL_A");
    assert_eq!(result.child_id, "PAL_TARGET");
    assert_eq!(result.rule_id, "BREEDING_FORMULA");
}

#[test]
fn prefers_special_breeding_combination_over_rank_formula() {
    let engine = engine(vec![
        ranked_pal("PAL_A", "Alpha", 1000, 100_000),
        ranked_pal("PAL_B", "Beta", 1500, 150_000),
        ranked_pal("PAL_TARGET", "Target Child", 1250, 125_000),
        ranked_pal("PAL_SPECIAL", "Special Child", 900, 90_000),
        rule("RULE_AB_SPECIAL", "PAL_A", "PAL_B", "PAL_SPECIAL"),
    ]);

    let answer = engine.calculate_breeding_result("Alpha", "Beta");
    assert_eq!(answer.status, AnswerStatus::Ok);
    let result = answer.data.expect("breeding result exists");
    assert_eq!(result.child_id, "PAL_SPECIAL");
    assert_eq!(result.rule_id, "RULE_AB_SPECIAL");
}

#[test]
fn explores_calculated_results_in_shortest_chain_candidates() {
    let engine = engine(vec![
        ranked_pal("PAL_START", "Start", 1001, 200_000),
        ranked_pal("PAL_PARTNER", "Partner", 1004, 150_000),
        ranked_pal("PAL_MIDDLE", "Middle", 1003, 100_000),
        ranked_pal("PAL_SECOND_PARTNER", "Second Partner", 1008, 100_000),
        ranked_pal("PAL_TARGET", "Target", 1006, 100_000),
    ]);

    let answer = engine.calculate_breeding_chain("Start", "Target", 2);
    assert_eq!(answer.status, AnswerStatus::Ambiguous);
    assert!(answer.data.is_none());
    assert!(answer
        .uncertainty
        .iter()
        .any(|message| message.contains("PAL_MIDDLE -> PAL_TARGET")));
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

#[test]
fn reports_ambiguous_breeding_parent_names_with_candidates() {
    let engine = engine(vec![
        pal("PAL_ALPHA_1", "Same Name"),
        pal("PAL_ALPHA_2", "Same Name"),
        pal("PAL_BETA", "Beta"),
    ]);

    let answer = engine.calculate_breeding_result("Same Name", "Beta");
    assert_eq!(answer.status, AnswerStatus::Ambiguous);
    assert!(answer.data.is_none());
    assert!(answer
        .uncertainty
        .iter()
        .any(|message| message.contains("PAL_ALPHA_1") && message.contains("PAL_ALPHA_2")));
}

#[test]
fn reports_ambiguous_breeding_chain_endpoints_with_candidates() {
    let engine = engine(vec![
        pal("PAL_START_1", "Start"),
        pal("PAL_START_2", "Start"),
        pal("PAL_UNIQUE_START", "Unique Start"),
        pal("PAL_TARGET_1", "Target"),
        pal("PAL_TARGET_2", "Target"),
    ]);

    let start_answer = engine.calculate_breeding_chain("Start", "Target", 3);
    assert_eq!(start_answer.status, AnswerStatus::Ambiguous);
    assert!(start_answer.data.is_none());
    assert!(start_answer
        .uncertainty
        .iter()
        .any(|message| message.contains("PAL_START_1") && message.contains("PAL_START_2")));

    let target_answer = engine.calculate_breeding_chain("Unique Start", "Target", 3);
    assert_eq!(target_answer.status, AnswerStatus::Ambiguous);
    assert!(target_answer
        .uncertainty
        .iter()
        .any(|message| message.contains("PAL_TARGET_1") && message.contains("PAL_TARGET_2")));
}
