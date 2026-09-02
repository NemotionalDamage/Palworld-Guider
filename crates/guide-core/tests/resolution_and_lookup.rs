use game_knowledge::{
    Confidence, ConflictRecord, ConflictResolution, KnowledgeRecord, KnowledgeStore, LocaleNames,
    Provenance, ReviewStatus, SourceRecord,
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

fn provenance_with(source_id: &str, version: &str) -> Provenance {
    Provenance {
        source_id: source_id.to_string(),
        applicable_game_version: version.to_string(),
        retrieved_on: "2026-01-03".to_string(),
        reviewer: "test".to_string(),
        review_status: ReviewStatus::Reviewed,
        confidence: Confidence::Official,
        change_risk: None,
        corroborating_source_ids: Vec::new(),
    }
}

fn source_with(id: &str, version: &str) -> SourceRecord {
    SourceRecord {
        id: id.to_string(),
        title: "Test source".to_string(),
        supplier: "test".to_string(),
        retrieved_on: "2026-01-03".to_string(),
        evidence_urls: vec!["https://example.test/source".to_string()],
        applicable_game_version: version.to_string(),
        reviewer: "test".to_string(),
        review_status: ReviewStatus::Reviewed,
        confidence: Confidence::Official,
        notes: None,
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
            native_row_id: None,
            local_evidence: None,
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
            native_row_id: None,
            local_evidence: None,
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
fn preserves_unicode_aliases_and_rejects_empty_normalized_queries() {
    let records = vec![
        KnowledgeRecord::Source(source()),
        KnowledgeRecord::Item(game_knowledge::ItemRecord {
            id: "ITEM_WOOD".to_string(),
            names: names("Wood"),
            description: None,
            rarity: "Common".to_string(),
            acquisition_leads: vec![],
            native_row_id: None,
            local_evidence: None,
            provenance: provenance(),
        }),
        KnowledgeRecord::Alias(game_knowledge::AliasRecord {
            id: "ALIAS_WOOD_ZH".to_string(),
            alias: "木头".to_string(),
            target_id: "ITEM_WOOD".to_string(),
            locale: "zh_hans".to_string(),
            provenance: provenance(),
        }),
    ];
    let store = KnowledgeStore::from_records(records).expect("Unicode alias validates");
    let engine = GuideEngine::new(store, None);

    let answer = engine.lookup_item("木头");
    assert_eq!(answer.status, AnswerStatus::Ok);
    assert_eq!(
        answer.data.as_ref().expect("Unicode alias resolves").id,
        "ITEM_WOOD"
    );

    let answer = engine.lookup_item("!!!");
    assert_eq!(answer.status, AnswerStatus::Unknown);
    assert!(answer.data.is_none());
}

#[test]
fn propagates_conflicts_from_item_relations() {
    let records = vec![
        KnowledgeRecord::Source(source()),
        KnowledgeRecord::Item(game_knowledge::ItemRecord {
            id: "ITEM_ALPHA".to_string(),
            names: names("Alpha Stone"),
            description: None,
            rarity: "Common".to_string(),
            acquisition_leads: vec![],
            native_row_id: None,
            local_evidence: None,
            provenance: provenance(),
        }),
        KnowledgeRecord::Item(game_knowledge::ItemRecord {
            id: "ITEM_WOOD".to_string(),
            names: names("Wood"),
            description: None,
            rarity: "Common".to_string(),
            acquisition_leads: vec![],
            native_row_id: None,
            local_evidence: None,
            provenance: provenance(),
        }),
        KnowledgeRecord::Recipe(game_knowledge::RecipeRecord {
            id: "RECIPE_ALPHA".to_string(),
            output: game_knowledge::RecipeItem {
                item_id: "ITEM_ALPHA".to_string(),
                quantity: 1,
            },
            ingredients: vec![game_knowledge::RecipeIngredient {
                item_id: "ITEM_WOOD".to_string(),
                quantity: 1,
            }],
            crafting_stations: vec!["Test Bench".to_string()],
            technology_id: None,
            crafting_seconds: None,
            byproducts: vec![],
            native_row_id: None,
            local_evidence: None,
            provenance: provenance(),
        }),
        KnowledgeRecord::Pal(game_knowledge::PalRecord {
            id: "PAL_DROPPER".to_string(),
            names: names("Dropper"),
            stats: None,
            work_suitability: vec![],
            drops: vec![game_knowledge::DropSource {
                item_id: "ITEM_ALPHA".to_string(),
                min_quantity: 1,
                max_quantity: 1,
                probability_percent: 100.0,
            }],
            habitat_ids: vec![],
            native_row_id: None,
            local_evidence: None,
            provenance: provenance(),
        }),
        KnowledgeRecord::Conflict(ConflictRecord {
            id: "CONFLICT_RECIPE".to_string(),
            subject_id: "RECIPE_ALPHA".to_string(),
            field: "ingredients".to_string(),
            values: vec!["1 Wood".to_string(), "2 Wood".to_string()],
            source_ids: vec!["SRC-TEST".to_string()],
            resolution: ConflictResolution::Unresolved,
            provenance: provenance(),
        }),
        KnowledgeRecord::Conflict(ConflictRecord {
            id: "CONFLICT_PAL".to_string(),
            subject_id: "PAL_DROPPER".to_string(),
            field: "drops".to_string(),
            values: vec!["always".to_string(), "sometimes".to_string()],
            source_ids: vec!["SRC-TEST".to_string()],
            resolution: ConflictResolution::Unresolved,
            provenance: provenance(),
        }),
    ];
    let store = KnowledgeStore::from_records(records).expect("related records validate");
    let engine = GuideEngine::new(store, None);

    let answer = engine.lookup_item("alpha stone");
    assert_eq!(answer.status, AnswerStatus::Ambiguous);
    assert!(answer
        .uncertainty
        .iter()
        .any(|message| message.contains("RECIPE_ALPHA")));
    assert!(answer
        .uncertainty
        .iter()
        .any(|message| message.contains("PAL_DROPPER")));
}

#[test]
fn exact_canonical_lookups_include_facts_and_provenance() {
    let engine = GuideEngine::load_directory("../../data/reviewed", None)
        .expect("canonical dataset must load");

    let answer = engine.lookup_item("stone");
    assert_eq!(answer.status, AnswerStatus::Ok);
    let item = answer.data.expect("Stone resolves");
    assert_eq!(item.id, "ITEM_STONE");
    assert_eq!(item.names.en, "Stone");
    assert!(item
        .acquisition_leads
        .iter()
        .any(|lead| lead.action == "Mine rocks"));
    assert!(item.provenance.source_id.contains("PALDB"));
    assert_eq!(answer.version.knowledge_version, "1.0.3");

    let answer = engine.lookup_pal("Lamball");
    assert_eq!(answer.status, AnswerStatus::Ok);
    let pal = answer.data.expect("Lamball resolves");
    assert_eq!(pal.id, "PAL_LAMBALL");
    assert_eq!(pal.work_suitability.len(), 3);
    assert_eq!(pal.drops.len(), 2);

    let answer = engine.lookup_recipe("Paldium Fragment");
    assert_eq!(answer.status, AnswerStatus::Ok);
    let recipe = answer.data.expect("recipe resolves");
    assert_eq!(recipe.id, "RECIPE_PALDIUM_FRAGMENT");
    assert_eq!(recipe.output.item_id, "ITEM_PALDIUM_FRAGMENT");
    assert_eq!(recipe.technology_id, None);

    let answer = engine.lookup_technology("Technology Level 2");
    assert_eq!(answer.status, AnswerStatus::Ok);
    let technology = answer.data.expect("technology resolves");
    assert_eq!(technology.level, 2);
    assert_eq!(
        technology.unlocked_recipe_ids,
        vec!["RECIPE_PAL_SPHERE".to_string()]
    );
}

#[test]
fn serializes_provenance_enums_consistently() {
    let engine =
        GuideEngine::load_directory("../../data/reviewed", None).expect("canonical data loads");
    let answer = engine.lookup_item("stone");
    assert_eq!(answer.status, AnswerStatus::Ok);
    let item = answer.data.expect("Stone resolves");

    assert_eq!(answer.provenance[0].review_status, "reviewed");
    assert_eq!(answer.provenance[0].confidence, "reviewed_secondary");
    assert_eq!(
        item.provenance.review_status,
        game_knowledge::ReviewStatus::Reviewed
    );
    assert_eq!(
        item.provenance.confidence,
        game_knowledge::Confidence::ReviewedSecondary
    );
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

#[test]
fn propagates_byproduct_provenance_and_conflicts_into_materials() {
    let records = vec![
        KnowledgeRecord::Source(source()),
        KnowledgeRecord::Source(source_with("SRC-C", "1.0.2")),
        KnowledgeRecord::Item(game_knowledge::ItemRecord {
            id: "ITEM_RESULT".to_string(),
            names: names("Result"),
            description: None,
            rarity: "Common".to_string(),
            acquisition_leads: vec![],
            native_row_id: None,
            local_evidence: None,
            provenance: provenance(),
        }),
        KnowledgeRecord::Item(game_knowledge::ItemRecord {
            id: "ITEM_WOOD".to_string(),
            names: names("Wood"),
            description: None,
            rarity: "Common".to_string(),
            acquisition_leads: vec![],
            native_row_id: None,
            local_evidence: None,
            provenance: provenance(),
        }),
        KnowledgeRecord::Item(game_knowledge::ItemRecord {
            id: "ITEM_BONUS".to_string(),
            names: names("Bonus"),
            description: None,
            rarity: "Common".to_string(),
            acquisition_leads: vec![],
            native_row_id: None,
            local_evidence: None,
            provenance: provenance_with("SRC-C", "1.0.2"),
        }),
        KnowledgeRecord::Recipe(game_knowledge::RecipeRecord {
            id: "RECIPE_RESULT".to_string(),
            output: game_knowledge::RecipeItem {
                item_id: "ITEM_RESULT".to_string(),
                quantity: 1,
            },
            ingredients: vec![game_knowledge::RecipeIngredient {
                item_id: "ITEM_WOOD".to_string(),
                quantity: 1,
            }],
            crafting_stations: vec!["Test Bench".to_string()],
            technology_id: None,
            crafting_seconds: None,
            byproducts: vec![game_knowledge::RecipeItem {
                item_id: "ITEM_BONUS".to_string(),
                quantity: 2,
            }],
            native_row_id: None,
            local_evidence: None,
            provenance: provenance(),
        }),
        KnowledgeRecord::Conflict(ConflictRecord {
            id: "CONFLICT_BONUS".to_string(),
            subject_id: "ITEM_BONUS".to_string(),
            field: "description".to_string(),
            values: vec!["one".to_string(), "two".to_string()],
            source_ids: vec!["SRC-TEST".to_string()],
            resolution: ConflictResolution::Unresolved,
            provenance: provenance(),
        }),
    ];
    let store = KnowledgeStore::from_records(records).expect("byproduct records validate");
    let engine = GuideEngine::new(store, None);

    let answer = engine.calculate_materials("Result", 1);
    assert_eq!(answer.status, AnswerStatus::Ambiguous);
    assert!(answer
        .provenance
        .iter()
        .any(|summary| summary.source_id == "SRC-C"));
    assert_eq!(answer.version.knowledge_version, "mixed");
    assert!(answer
        .uncertainty
        .iter()
        .any(|message| message.contains("ITEM_BONUS")));
}

#[test]
fn lists_byproduct_acquisition_in_item_lookup() {
    let records = vec![
        KnowledgeRecord::Source(source()),
        KnowledgeRecord::Item(game_knowledge::ItemRecord {
            id: "ITEM_RESULT".to_string(),
            names: names("Result"),
            description: None,
            rarity: "Common".to_string(),
            acquisition_leads: vec![],
            native_row_id: None,
            local_evidence: None,
            provenance: provenance(),
        }),
        KnowledgeRecord::Item(game_knowledge::ItemRecord {
            id: "ITEM_WOOD".to_string(),
            names: names("Wood"),
            description: None,
            rarity: "Common".to_string(),
            acquisition_leads: vec![],
            native_row_id: None,
            local_evidence: None,
            provenance: provenance(),
        }),
        KnowledgeRecord::Item(game_knowledge::ItemRecord {
            id: "ITEM_BONUS".to_string(),
            names: names("Bonus"),
            description: None,
            rarity: "Common".to_string(),
            acquisition_leads: vec![],
            native_row_id: None,
            local_evidence: None,
            provenance: provenance(),
        }),
        KnowledgeRecord::Recipe(game_knowledge::RecipeRecord {
            id: "RECIPE_RESULT".to_string(),
            output: game_knowledge::RecipeItem {
                item_id: "ITEM_RESULT".to_string(),
                quantity: 1,
            },
            ingredients: vec![game_knowledge::RecipeIngredient {
                item_id: "ITEM_WOOD".to_string(),
                quantity: 1,
            }],
            crafting_stations: vec!["Test Bench".to_string()],
            technology_id: None,
            crafting_seconds: None,
            byproducts: vec![game_knowledge::RecipeItem {
                item_id: "ITEM_BONUS".to_string(),
                quantity: 2,
            }],
            native_row_id: None,
            local_evidence: None,
            provenance: provenance(),
        }),
    ];
    let store = KnowledgeStore::from_records(records).expect("byproduct lookup records validate");
    let engine = GuideEngine::new(store, None);

    let answer = engine.lookup_item("Bonus");
    assert_eq!(answer.status, AnswerStatus::Ok);
    let item = answer.data.expect("Bonus lookup exists");
    assert_eq!(item.byproduct_of.len(), 1);
    assert_eq!(item.byproduct_of[0].id, "RECIPE_RESULT");
    assert_eq!(item.byproduct_of[0].source_output_item_id, "ITEM_RESULT");
    assert_eq!(item.byproduct_of[0].byproduct_quantity, 2);
}

#[test]
fn propagates_related_conflicts_for_pal_technology_and_recipe_lookups() {
    let records = vec![
        KnowledgeRecord::Source(source()),
        KnowledgeRecord::Item(game_knowledge::ItemRecord {
            id: "ITEM_ALPHA".to_string(),
            names: names("Alpha Stone"),
            description: None,
            rarity: "Common".to_string(),
            acquisition_leads: vec![],
            native_row_id: None,
            local_evidence: None,
            provenance: provenance(),
        }),
        KnowledgeRecord::Item(game_knowledge::ItemRecord {
            id: "ITEM_WOOD".to_string(),
            names: names("Wood"),
            description: None,
            rarity: "Common".to_string(),
            acquisition_leads: vec![],
            native_row_id: None,
            local_evidence: None,
            provenance: provenance(),
        }),
        KnowledgeRecord::Pal(game_knowledge::PalRecord {
            id: "PAL_DROPPER".to_string(),
            names: names("Dropper"),
            stats: None,
            work_suitability: vec![],
            drops: vec![game_knowledge::DropSource {
                item_id: "ITEM_ALPHA".to_string(),
                min_quantity: 1,
                max_quantity: 1,
                probability_percent: 100.0,
            }],
            habitat_ids: vec![],
            native_row_id: None,
            local_evidence: None,
            provenance: provenance(),
        }),
        KnowledgeRecord::Technology(game_knowledge::TechnologyRecord {
            id: "TECH_ONE".to_string(),
            names: names("Tech One"),
            level: 1,
            native_row_id: None,
            local_evidence: None,
            provenance: provenance(),
        }),
        KnowledgeRecord::Recipe(game_knowledge::RecipeRecord {
            id: "RECIPE_ALPHA".to_string(),
            output: game_knowledge::RecipeItem {
                item_id: "ITEM_ALPHA".to_string(),
                quantity: 1,
            },
            ingredients: vec![game_knowledge::RecipeIngredient {
                item_id: "ITEM_WOOD".to_string(),
                quantity: 1,
            }],
            crafting_stations: vec!["Test Bench".to_string()],
            technology_id: Some("TECH_ONE".to_string()),
            crafting_seconds: None,
            byproducts: vec![],
            native_row_id: None,
            local_evidence: None,
            provenance: provenance(),
        }),
        KnowledgeRecord::Conflict(ConflictRecord {
            id: "CONFLICT_DROP_ITEM".to_string(),
            subject_id: "ITEM_ALPHA".to_string(),
            field: "description".to_string(),
            values: vec!["a".to_string(), "b".to_string()],
            source_ids: vec!["SRC-TEST".to_string()],
            resolution: ConflictResolution::Unresolved,
            provenance: provenance(),
        }),
        KnowledgeRecord::Conflict(ConflictRecord {
            id: "CONFLICT_RECIPE".to_string(),
            subject_id: "RECIPE_ALPHA".to_string(),
            field: "ingredients".to_string(),
            values: vec!["1".to_string(), "2".to_string()],
            source_ids: vec!["SRC-TEST".to_string()],
            resolution: ConflictResolution::Unresolved,
            provenance: provenance(),
        }),
        KnowledgeRecord::Conflict(ConflictRecord {
            id: "CONFLICT_INGREDIENT".to_string(),
            subject_id: "ITEM_WOOD".to_string(),
            field: "rarity".to_string(),
            values: vec!["common".to_string(), "rare".to_string()],
            source_ids: vec!["SRC-TEST".to_string()],
            resolution: ConflictResolution::Unresolved,
            provenance: provenance(),
        }),
    ];
    let store = KnowledgeStore::from_records(records).expect("related lookup records validate");
    let engine = GuideEngine::new(store, None);

    let pal_answer = engine.lookup_pal("PAL_DROPPER");
    assert_eq!(pal_answer.status, AnswerStatus::Ambiguous);
    assert!(pal_answer
        .uncertainty
        .iter()
        .any(|message| message.contains("ITEM_ALPHA")));

    let technology_answer = engine.lookup_technology("Tech One");
    assert_eq!(technology_answer.status, AnswerStatus::Ambiguous);
    assert!(technology_answer
        .uncertainty
        .iter()
        .any(|message| message.contains("RECIPE_ALPHA")));

    let recipe_answer = engine.lookup_recipe("RECIPE_ALPHA");
    assert_eq!(recipe_answer.status, AnswerStatus::Ambiguous);
    assert!(recipe_answer
        .uncertainty
        .iter()
        .any(|message| message.contains("ITEM_WOOD")));
}
