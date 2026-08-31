use game_knowledge::{
    Confidence, KnowledgeRecord, KnowledgeStore, LocaleNames, Provenance, ReviewStatus,
    SourceRecord,
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

fn names(english: &str) -> LocaleNames {
    LocaleNames {
        en: english.to_string(),
        zh_hans: None,
    }
}

fn test_store(with_duplicate_name: bool) -> KnowledgeStore {
    let records = vec![
        KnowledgeRecord::Source(source()),
        KnowledgeRecord::Item(game_knowledge::ItemRecord {
            id: "ITEM_ALPHA".to_string(),
            names: names("Alpha Stone"),
            description: Some("A test item".to_string()),
            rarity: "Common".to_string(),
            acquisition_leads: vec![],
            provenance: provenance(),
        }),
        KnowledgeRecord::Alias(game_knowledge::AliasRecord {
            id: "ALIAS_ALPHA".to_string(),
            alias: "alpha rock".to_string(),
            target_id: "ITEM_ALPHA".to_string(),
            locale: "en".to_string(),
            provenance: provenance(),
        }),
    ];
    let mut records = records;
    if with_duplicate_name {
        records.push(KnowledgeRecord::Item(game_knowledge::ItemRecord {
            id: "ITEM_ALPHA_ALT".to_string(),
            names: names("alpha  stone"),
            description: None,
            rarity: "Rare".to_string(),
            acquisition_leads: vec![],
            provenance: provenance(),
        }));
    }

    KnowledgeStore::from_records(records).expect("test records must validate")
}

#[test]
fn resolves_ids_names_and_reviewed_aliases() {
    let engine = GuideEngine::new(test_store(false), None);

    let answer = engine.lookup_item("ITEM_ALPHA");
    assert_eq!(answer.status, AnswerStatus::Ok);
    let item = answer.data.expect("exact ID resolves");
    assert_eq!(item.id, "ITEM_ALPHA");

    let answer = engine.lookup_item("ALPHA   STONE!");
    assert_eq!(answer.status, AnswerStatus::Ok);
    assert_eq!(
        answer.data.as_ref().expect("name resolves").id,
        "ITEM_ALPHA"
    );

    let answer = engine.lookup_item("Alpha Rock");
    assert_eq!(answer.status, AnswerStatus::Ok);
    assert_eq!(
        answer.data.as_ref().expect("alias resolves").id,
        "ITEM_ALPHA"
    );
}

#[test]
fn reports_ambiguous_and_unknown_names_without_guessing() {
    let engine = GuideEngine::new(test_store(true), None);

    let answer = engine.lookup_item("alpha stone");
    assert_eq!(answer.status, AnswerStatus::Ambiguous);
    assert!(answer.data.is_none());
    assert!(answer
        .uncertainty
        .iter()
        .any(|message| message.contains("ambiguous")));

    let answer = engine.lookup_item("does not exist");
    assert_eq!(answer.status, AnswerStatus::Unknown);
    assert!(answer.data.is_none());
    assert!(answer
        .uncertainty
        .iter()
        .any(|message| message.contains("unknown")));
}

#[test]
fn exact_canonical_lookups_include_facts_and_provenance() {
    let engine = GuideEngine::load_directory("../../data/reviewed", None)
        .expect("canonical dataset must load");

    let answer = engine.lookup_item("wood");
    assert_eq!(answer.status, AnswerStatus::Ok);
    let item = answer.data.expect("Wood resolves");
    assert_eq!(item.id, "ITEM_WOOD");
    assert_eq!(item.names.en, "Wood");
    assert!(item
        .acquisition_leads
        .iter()
        .any(|lead| lead.action == "Chop trees"));
    assert!(item.provenance.source_id.contains("PALDB"));
    assert_eq!(answer.version.knowledge_version, "1.0.3");

    let answer = engine.lookup_pal("Lamball");
    assert_eq!(answer.status, AnswerStatus::Ok);
    let pal = answer.data.expect("Lamball resolves");
    assert_eq!(pal.id, "PAL_LAMBALL");
    assert_eq!(pal.work_suitability.len(), 3);
    assert_eq!(pal.drops.len(), 2);

    let answer = engine.lookup_recipe("Wooden Club");
    assert_eq!(answer.status, AnswerStatus::Ok);
    let recipe = answer.data.expect("recipe resolves");
    assert_eq!(recipe.id, "RECIPE_WOODEN_CLUB");
    assert_eq!(recipe.output.item_id, "ITEM_WOODEN_CLUB");
    assert_eq!(recipe.technology_id.as_deref(), Some("TECHNOLOGY_LEVEL_1"));

    let answer = engine.lookup_technology("Technology Level 1");
    assert_eq!(answer.status, AnswerStatus::Ok);
    let technology = answer.data.expect("technology resolves");
    assert_eq!(technology.level, 1);
    assert_eq!(technology.unlocked_recipe_ids, vec!["RECIPE_WOODEN_CLUB"]);
}

#[test]
fn exposes_configured_version_mismatch() {
    let engine = GuideEngine::new(test_store(false), Some("1.0.1".to_string()));
    let answer = engine.lookup_item("alpha rock");

    assert_eq!(answer.status, AnswerStatus::Ok);
    assert!(!answer.version.matches);
    assert!(answer
        .uncertainty
        .iter()
        .any(|message| message.contains("does not match configured game version")));
}
