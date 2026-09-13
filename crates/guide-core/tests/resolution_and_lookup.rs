use game_knowledge::{
    Confidence, ConflictRecord, ConflictResolution, KnowledgeRecord, KnowledgeStore, LocaleNames,
    Provenance, RecipeIngredient, RecipeItem, RecipeRecord, ReviewStatus, SourceRecord,
};
use guide_core::{AnswerStatus, GuideEngine, WorldCoordinate};

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
    test_store_with_duplicate_rarity(with_duplicate_name.then_some("Rare"))
}

fn test_store_with_duplicate_rarity(duplicate_rarity: Option<&str>) -> KnowledgeStore {
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
    if let Some(rarity) = duplicate_rarity {
        records.push(KnowledgeRecord::Item(game_knowledge::ItemRecord {
            id: "ITEM_ALPHA_ALT".to_string(),
            names: names("alpha  stone"),
            description: None,
            rarity: rarity.to_string(),
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
fn resolves_structured_localized_technology_names() {
    let records = vec![
        KnowledgeRecord::Source(source()),
        KnowledgeRecord::Technology(game_knowledge::TechnologyRecord {
            id: "TECH_WORKBENCH".to_string(),
            names: LocaleNames {
                en: "Primitive Workbench".to_string(),
                zh_hans: Some("原始的作业台".to_string()),
            },
            level: 1,
            native_row_id: None,
            local_evidence: None,
            provenance: provenance(),
        }),
    ];
    let store = KnowledgeStore::from_records(records).expect("test records validate");
    let engine = GuideEngine::new(store, None);

    let answer = engine.lookup_technology("原始的作业台");

    assert_eq!(answer.status, AnswerStatus::Ok);
    assert_eq!(
        answer.data.expect("technology resolves").id,
        "TECH_WORKBENCH"
    );
}

#[test]
fn item_lookup_names_and_quantifies_recipe_relations() {
    let records = vec![
        KnowledgeRecord::Source(source()),
        KnowledgeRecord::Item(game_knowledge::ItemRecord {
            id: "ITEM_PALDIUM_FRAGMENT".to_string(),
            names: LocaleNames {
                en: "Paldium Fragment".to_string(),
                zh_hans: Some("帕鲁矿碎块".to_string()),
            },
            description: None,
            rarity: "Common".to_string(),
            acquisition_leads: Vec::new(),
            native_row_id: None,
            local_evidence: None,
            provenance: provenance(),
        }),
        KnowledgeRecord::Item(game_knowledge::ItemRecord {
            id: "ITEM_PAL_SPHERE".to_string(),
            names: LocaleNames {
                en: "Pal Sphere".to_string(),
                zh_hans: Some("帕鲁球".to_string()),
            },
            description: None,
            rarity: "Common".to_string(),
            acquisition_leads: Vec::new(),
            native_row_id: None,
            local_evidence: None,
            provenance: provenance(),
        }),
        KnowledgeRecord::Recipe(RecipeRecord {
            id: "RECIPE_PAL_SPHERE".to_string(),
            output: RecipeItem {
                item_id: "ITEM_PAL_SPHERE".to_string(),
                quantity: 1,
            },
            ingredients: vec![RecipeIngredient {
                item_id: "ITEM_PALDIUM_FRAGMENT".to_string(),
                quantity: 1,
            }],
            crafting_stations: vec!["Pal Sphere Workbench".to_string()],
            technology_id: None,
            unlock_item_id: None,
            byproducts: Vec::new(),
            native_row_id: None,
            local_evidence: None,
            provenance: provenance(),
        }),
    ];
    let store = KnowledgeStore::from_records(records).expect("test records validate");
    let engine = GuideEngine::new(store, None);

    let answer = engine.lookup_item("帕鲁矿碎块");

    assert_eq!(answer.status, AnswerStatus::Ok);
    let item = answer.data.expect("item resolves");
    let relation = item
        .used_as_ingredient
        .iter()
        .find(|recipe| recipe.id == "RECIPE_PAL_SPHERE")
        .expect("Pal Sphere uses Paldium Fragment");
    assert_eq!(relation.output_item_name, "Pal Sphere");
    assert_eq!(relation.ingredient_quantity, Some(1));
}

#[test]
fn bare_duplicate_name_answers_the_lowest_tier() {
    let engine = GuideEngine::new(test_store(true), None);

    let answer = engine.lookup_item("alpha stone");
    assert_eq!(answer.status, AnswerStatus::Ok);
    let item = answer.data.expect("the lowest tier answers a bare name");
    assert_eq!(item.id, "ITEM_ALPHA");
    assert_eq!(item.rarity, "Common");
}

#[test]
fn reports_ambiguous_and_unknown_names_without_guessing() {
    let engine = GuideEngine::new(test_store_with_duplicate_rarity(Some("Common")), None);

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
fn explains_schematics_for_rank_variant_items_and_recipes() {
    let records = vec![
        KnowledgeRecord::Source(source()),
        KnowledgeRecord::Item(game_knowledge::ItemRecord {
            id: "ITEM_ARMOR".to_string(),
            names: names("Test Armor"),
            description: None,
            rarity: "Common".to_string(),
            acquisition_leads: vec![],
            native_row_id: Some("TestArmor".to_string()),
            local_evidence: None,
            provenance: provenance(),
        }),
        KnowledgeRecord::Item(game_knowledge::ItemRecord {
            id: "ITEM_ARMOR_5".to_string(),
            names: names("Test Armor"),
            description: None,
            rarity: "Legendary".to_string(),
            acquisition_leads: vec![],
            native_row_id: Some("TestArmor_5".to_string()),
            local_evidence: None,
            provenance: provenance(),
        }),
        KnowledgeRecord::Item(game_knowledge::ItemRecord {
            id: "ITEM_BLUEPRINT_TESTARMOR_5".to_string(),
            names: names("Test Armor Schematic 4"),
            description: None,
            rarity: "Legendary".to_string(),
            acquisition_leads: vec![],
            native_row_id: Some("Blueprint_TestArmor_5".to_string()),
            local_evidence: None,
            provenance: provenance(),
        }),
        KnowledgeRecord::Item(game_knowledge::ItemRecord {
            id: "ITEM_INGOT".to_string(),
            names: names("Test Ingot"),
            description: None,
            rarity: "Common".to_string(),
            acquisition_leads: vec![],
            native_row_id: Some("TestIngot".to_string()),
            local_evidence: None,
            provenance: provenance(),
        }),
        KnowledgeRecord::Recipe(game_knowledge::RecipeRecord {
            id: "RECIPE_TESTARMOR".to_string(),
            output: game_knowledge::RecipeItem {
                item_id: "ITEM_ARMOR".to_string(),
                quantity: 1,
            },
            ingredients: vec![game_knowledge::RecipeIngredient {
                item_id: "ITEM_INGOT".to_string(),
                quantity: 10,
            }],
            crafting_stations: vec!["Test Bench".to_string()],
            technology_id: None,
            unlock_item_id: None,
            byproducts: Vec::new(),
            native_row_id: Some("TestArmor".to_string()),
            local_evidence: None,
            provenance: provenance(),
        }),
        KnowledgeRecord::Recipe(game_knowledge::RecipeRecord {
            id: "RECIPE_TESTARMOR_5".to_string(),
            output: game_knowledge::RecipeItem {
                item_id: "ITEM_ARMOR_5".to_string(),
                quantity: 1,
            },
            ingredients: vec![game_knowledge::RecipeIngredient {
                item_id: "ITEM_INGOT".to_string(),
                quantity: 60,
            }],
            crafting_stations: vec!["Test Bench".to_string()],
            technology_id: None,
            unlock_item_id: Some("ITEM_BLUEPRINT_TESTARMOR_5".to_string()),
            byproducts: Vec::new(),
            native_row_id: Some("TestArmor_5".to_string()),
            local_evidence: None,
            provenance: provenance(),
        }),
    ];
    let store = KnowledgeStore::from_records(records).expect("rank variant records validate");
    let engine = GuideEngine::new(store, None);

    let answer = engine.lookup_item("Test Armor");
    assert_eq!(answer.status, AnswerStatus::Ok);
    let item = answer.data.expect("a bare name answers the base tier");
    assert_eq!(item.id, "ITEM_ARMOR");
    let lead = item
        .acquisition_leads
        .iter()
        .find(|lead| lead.action.contains("Higher tiers"))
        .expect("the base tier points at the schematic-only tiers");
    assert!(lead
        .notes
        .as_deref()
        .unwrap_or_default()
        .contains("legendary"));

    let answer = engine.lookup_recipe("Test Armor");
    assert_eq!(answer.status, AnswerStatus::Ok);
    let lookup = answer.data.expect("a bare name answers the base recipe");
    assert_eq!(lookup.recipe.id, "RECIPE_TESTARMOR");
    assert!(lookup
        .schematic_leads
        .iter()
        .any(|lead| lead.action.contains("Higher tiers")));

    let answer = engine.lookup_recipe_filtered("Test Armor", Some("legendary"));
    assert_eq!(answer.status, AnswerStatus::Ok);
    let lookup = answer.data.expect("the requested tier resolves");
    assert_eq!(lookup.recipe.id, "RECIPE_TESTARMOR_5");
    assert_eq!(lookup.recipe.ingredients[0].quantity, 60);
    let lead = lookup
        .schematic_leads
        .iter()
        .find(|lead| lead.action.contains("Requires a schematic"))
        .expect("the higher tier requires its matching schematic");
    let notes = lead.notes.as_deref().unwrap_or_default();
    assert!(notes.contains("Test Armor Schematic 4"));
    assert!(notes.contains("legendary"));

    let answer = engine.lookup_recipe("Legendary Test Armor");
    assert_eq!(answer.status, AnswerStatus::Ok);
    let lookup = answer
        .data
        .expect("a leading tier word selects the higher tier");
    assert_eq!(lookup.recipe.id, "RECIPE_TESTARMOR_5");
}

#[test]
fn treats_a_recipe_without_an_unlock_item_as_craftable() {
    let item = |id: &str, name: &str, rarity: &str, row: &str| {
        KnowledgeRecord::Item(game_knowledge::ItemRecord {
            id: id.to_string(),
            names: names(name),
            description: None,
            rarity: rarity.to_string(),
            acquisition_leads: vec![],
            native_row_id: Some(row.to_string()),
            local_evidence: None,
            provenance: provenance(),
        })
    };
    let recipe = |id: &str, output: &str, unlock_item_id: Option<&str>| {
        KnowledgeRecord::Recipe(RecipeRecord {
            id: id.to_string(),
            output: RecipeItem {
                item_id: output.to_string(),
                quantity: 1,
            },
            ingredients: vec![RecipeIngredient {
                item_id: "ITEM_INGOT".to_string(),
                quantity: 1,
            }],
            crafting_stations: vec!["Test Bench".to_string()],
            technology_id: None,
            unlock_item_id: unlock_item_id.map(str::to_string),
            byproducts: Vec::new(),
            native_row_id: Some(id.trim_start_matches("RECIPE_").to_string()),
            local_evidence: None,
            provenance: provenance(),
        })
    };
    let records = vec![
        KnowledgeRecord::Source(source()),
        item("ITEM_ARMOR", "Test Armor", "Common", "TestArmor"),
        item("ITEM_ARMOR_5", "Test Armor", "Legendary", "TestArmor_5"),
        item(
            "ITEM_BLUEPRINT_TESTARMOR_5",
            "Test Armor Schematic 4",
            "Legendary",
            "Blueprint_TestArmor_5",
        ),
        item("ITEM_INGOT", "Test Ingot", "Common", "TestIngot"),
        recipe("RECIPE_TESTARMOR", "ITEM_ARMOR", None),
        recipe("RECIPE_TESTARMOR_5", "ITEM_ARMOR_5", None),
    ];
    let store = KnowledgeStore::from_records(records).expect("tier records validate");
    let engine = GuideEngine::new(store, None);

    let answer = engine.lookup_item("Test Armor");
    assert_eq!(answer.status, AnswerStatus::Ok);
    let armor = answer.data.expect("the base tier answers a bare name");
    assert!(armor
        .acquisition_leads
        .iter()
        .all(|lead| !lead.action.contains("schematic")));

    let answer = engine.lookup_recipe_filtered("Test Armor", Some("legendary"));
    assert_eq!(answer.status, AnswerStatus::Ok);
    let lookup = answer.data.expect("the higher tier resolves");
    assert!(lookup.schematic_leads.is_empty());
}

#[test]
fn keeps_an_exact_name_that_starts_with_a_rarity_word() {
    let records = vec![
        KnowledgeRecord::Source(source()),
        KnowledgeRecord::Item(game_knowledge::ItemRecord {
            id: "ITEM_SPHERE".to_string(),
            names: names("Sphere"),
            description: None,
            rarity: "Common".to_string(),
            acquisition_leads: vec![],
            native_row_id: Some("Sphere".to_string()),
            local_evidence: None,
            provenance: provenance(),
        }),
        KnowledgeRecord::Item(game_knowledge::ItemRecord {
            id: "ITEM_LEGENDARYSPHERE".to_string(),
            names: names("Legendary Sphere"),
            description: None,
            rarity: "Legendary".to_string(),
            acquisition_leads: vec![],
            native_row_id: Some("LegendarySphere".to_string()),
            local_evidence: None,
            provenance: provenance(),
        }),
    ];
    let store = KnowledgeStore::from_records(records).expect("sphere records validate");
    let engine = GuideEngine::new(store, None);

    let answer = engine.lookup_item("Legendary Sphere");
    assert_eq!(answer.status, AnswerStatus::Ok);
    let item = answer
        .data
        .expect("the exact name wins over the tier split");
    assert_eq!(item.id, "ITEM_LEGENDARYSPHERE");
}

#[test]
fn keeps_schematic_advice_out_of_families_without_schematic_rows() {
    let tier = |id: &str, rarity: &str, row: &str| {
        KnowledgeRecord::Item(game_knowledge::ItemRecord {
            id: id.to_string(),
            names: names("Test Egg"),
            description: None,
            rarity: rarity.to_string(),
            acquisition_leads: vec![],
            native_row_id: Some(row.to_string()),
            local_evidence: None,
            provenance: provenance(),
        })
    };
    let item = |id: &str, name: &str, rarity: &str, row: &str| {
        KnowledgeRecord::Item(game_knowledge::ItemRecord {
            id: id.to_string(),
            names: names(name),
            description: None,
            rarity: rarity.to_string(),
            acquisition_leads: vec![],
            native_row_id: Some(row.to_string()),
            local_evidence: None,
            provenance: provenance(),
        })
    };
    let records = vec![
        KnowledgeRecord::Source(source()),
        tier("ITEM_EGG_01", "Common", "TestEgg_01"),
        tier("ITEM_EGG_02", "Uncommon", "TestEgg_02"),
        item("ITEM_RELIC", "Test Relic", "Common", "TestRelic"),
        item("ITEM_RELIC_5", "Test Relic", "Legendary", "TestRelic_5"),
        item(
            "ITEM_BLUEPRINT_TESTRELIC",
            "Test Relic Schematic 1",
            "Uncommon",
            "Blueprint_TestRelic",
        ),
        item(
            "ITEM_BLUEPRINT_TESTRELIC_5",
            "Test Relic Schematic 4",
            "Legendary",
            "Blueprint_TestRelic_5",
        ),
    ];
    let store = KnowledgeStore::from_records(records).expect("tier records validate");
    let engine = GuideEngine::new(store, None);

    let answer = engine.lookup_item("Test Egg");
    assert_eq!(answer.status, AnswerStatus::Ok);
    let egg = answer.data.expect("the base tier answers a bare name");
    assert_eq!(egg.id, "ITEM_EGG_01");
    assert!(egg
        .acquisition_leads
        .iter()
        .all(|lead| !lead.action.contains("schematic")));

    let answer = engine.lookup_item("Test Relic");
    assert_eq!(answer.status, AnswerStatus::Ok);
    let relic = answer.data.expect("the base tier answers a bare name");
    assert_eq!(relic.id, "ITEM_RELIC");
    let lead = relic
        .acquisition_leads
        .iter()
        .find(|lead| lead.action.contains("Requires a schematic"))
        .expect("a schematic-locked base tier says so");
    assert!(lead
        .notes
        .as_deref()
        .unwrap_or_default()
        .contains("Test Relic Schematic 1"));
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
            unlock_item_id: None,
            byproducts: vec![],
            native_row_id: None,
            local_evidence: None,
            provenance: provenance(),
        }),
        KnowledgeRecord::Pal(game_knowledge::PalRecord {
            id: "PAL_DROPPER".to_string(),
            names: names("Dropper"),
            work_suitability: vec![],
            drops: vec![game_knowledge::DropSource {
                item_id: "ITEM_ALPHA".to_string(),
                min_quantity: 1,
                max_quantity: 1,
                probability_percent: 100.0,
            }],
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
    assert_eq!(answer.version.knowledge_version, "1.0");

    let answer = engine.lookup_pal("Lamball");
    assert_eq!(answer.status, AnswerStatus::Ok);
    let pal = answer.data.expect("Lamball resolves");
    assert_eq!(pal.id, "PAL_LAMBALL");
    assert_eq!(pal.work_suitability.len(), 3);
    assert_eq!(pal.drops.len(), 2);

    let answer = engine.lookup_recipe("Roast Reindrix");
    assert_eq!(answer.status, AnswerStatus::Ok);
    let recipe = answer.data.expect("recipe resolves");
    assert_eq!(recipe.recipe.output.item_id, "ITEM_BAKEDMEAT_ICEDEER");

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
fn pal_lookup_shares_habitat_leads_without_the_internal_wild_spawn_verdict() {
    let engine = GuideEngine::load_directory("../../data/reviewed", None)
        .expect("canonical dataset must load");

    let answer = engine.lookup_pal("Pengullet Lux");
    assert_eq!(answer.status, AnswerStatus::Ok);
    let fishing_only = answer.data.expect("fishing-only Pal resolves");
    assert_eq!(fishing_only.id, "PAL_PENGUIN_ELECTRIC");
    assert!(fishing_only.habitat_ids.is_empty());
    assert!(fishing_only
        .habitat_leads
        .iter()
        .any(|lead| lead.action == "Fishing spots"));
    assert!(!serde_json::to_string(&fishing_only)
        .expect("Pal lookup serializes")
        .contains("wild_spawn_review"));

    let answer = engine.lookup_pal("Bellanoir");
    assert_eq!(answer.status, AnswerStatus::Ok);
    let raid_only = answer.data.expect("raid-only Pal resolves");
    assert_eq!(raid_only.id, "PAL_NIGHTLADY");
    assert!(raid_only.habitat_leads.is_empty());
    assert!(!serde_json::to_string(&raid_only)
        .expect("Pal lookup serializes")
        .contains("wild_spawn_review"));

    let answer = engine.lookup_pal("Panthalus");
    assert_eq!(answer.status, AnswerStatus::Ok);
    let boss_only = answer.data.expect("boss-only Pal resolves");
    assert_eq!(boss_only.id, "PAL_KINGWHALE");
    assert!(boss_only.work_suitability.is_empty());
    assert!(boss_only
        .habitat_leads
        .iter()
        .any(|lead| lead.action == "Boss encounter"));
    assert!(!serde_json::to_string(&boss_only)
        .expect("Pal lookup serializes")
        .contains("wild_spawn_review"));
}

#[test]
fn pal_spawn_zone_lookup_offers_habitat_leads_when_no_field_zone_exists() {
    let engine = GuideEngine::load_directory("../../data/reviewed", None)
        .expect("canonical dataset must load");

    let fishing_only = engine.find_pal_spawn_zones(
        "Pengullet Lux",
        WorldCoordinate {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
        5,
    );
    assert_eq!(fishing_only.status, AnswerStatus::Unknown);
    assert!(fishing_only
        .uncertainty
        .iter()
        .any(|message| message.contains("habitat leads: Fishing spots")));

    let raid_only = engine.find_pal_spawn_zones(
        "Bellanoir",
        WorldCoordinate {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
        5,
    );
    assert_eq!(raid_only.status, AnswerStatus::Unknown);
    assert_eq!(
        raid_only.uncertainty,
        vec!["this Pal has no reviewed target-build habitat coverage".to_string()]
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
    assert!(answer
        .provenance
        .iter()
        .any(|p| p.confidence == "reviewed_secondary" || p.confidence == "verified_target"));
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
            unlock_item_id: None,
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
            unlock_item_id: None,
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
            work_suitability: vec![],
            drops: vec![game_knowledge::DropSource {
                item_id: "ITEM_ALPHA".to_string(),
                min_quantity: 1,
                max_quantity: 1,
                probability_percent: 100.0,
            }],
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
            unlock_item_id: None,
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
