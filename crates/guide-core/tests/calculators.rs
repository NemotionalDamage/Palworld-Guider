use game_knowledge::{
    Confidence, KnowledgeRecord, KnowledgeStore, LocaleNames, Provenance, RecipeIngredient,
    RecipeItem, RecipeRecord, ReviewStatus, SourceRecord,
};
use guide_core::{AnswerStatus, GuideEngine, InventoryEntry, MaterialAcquisition};

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

fn item(id: &str, name: &str) -> KnowledgeRecord {
    KnowledgeRecord::Item(game_knowledge::ItemRecord {
        id: id.to_string(),
        names: LocaleNames {
            en: name.to_string(),
            zh_hans: None,
        },
        description: None,
        rarity: "Common".to_string(),
        acquisition_leads: vec![],
        native_row_id: None,
        local_evidence: None,
        provenance: provenance(),
    })
}

fn recipe(
    id: &str,
    output: (&str, u32),
    ingredients: &[(&str, u32)],
    byproducts: &[(&str, u32)],
) -> KnowledgeRecord {
    KnowledgeRecord::Recipe(RecipeRecord {
        id: id.to_string(),
        output: RecipeItem {
            item_id: output.0.to_string(),
            quantity: output.1,
        },
        ingredients: ingredients
            .iter()
            .map(|(item_id, quantity)| RecipeIngredient {
                item_id: item_id.to_string(),
                quantity: *quantity,
            })
            .collect(),
        crafting_stations: vec!["Test Bench".to_string()],
        technology_id: None,
        crafting_seconds: None,
        byproducts: byproducts
            .iter()
            .map(|(item_id, quantity)| RecipeItem {
                item_id: item_id.to_string(),
                quantity: *quantity,
            })
            .collect(),
        native_row_id: None,
        local_evidence: None,
        provenance: provenance(),
    })
}

fn test_store(records: Vec<KnowledgeRecord>) -> GuideEngine {
    let mut all_records = vec![KnowledgeRecord::Source(source())];
    all_records.extend(records);
    let store = KnowledgeStore::from_records(all_records).expect("test records validate");
    GuideEngine::new(store, None)
}

#[test]
fn calculates_canonical_materials_shortage_and_craftable_count() {
    let engine =
        GuideEngine::load_directory("../../data/reviewed", None).expect("canonical dataset loads");

    let answer = engine.calculate_materials("Roast Reindrix", 3);
    assert_eq!(answer.status, AnswerStatus::Ok);
    let calculation = answer.data.expect("calculation exists");
    assert_eq!(calculation.target_id, "ITEM_BAKEDMEAT_ICEDEER");
    assert_eq!(calculation.requested_quantity, 3);
    let venison = calculation
        .totals
        .iter()
        .find(|total| total.item_id == "ITEM_MEAT_ICEDEER")
        .expect("Reindrix Venison total exists");
    assert_eq!(venison.required_quantity, 3);
    assert_eq!(calculation.tree.item_id, "ITEM_BAKEDMEAT_ICEDEER");
    assert_eq!(calculation.tree.required_quantity, 3);

    let answer = engine.calculate_shortage(
        "Roast Reindrix",
        3,
        &[InventoryEntry::new("Reindrix Venison", 1)],
    );
    assert_eq!(answer.status, AnswerStatus::Ok);
    let shortage = answer.data.expect("shortage exists");
    assert_eq!(shortage.shortages.len(), 1);
    assert_eq!(shortage.shortages[0].item_id, "ITEM_MEAT_ICEDEER");
    assert_eq!(shortage.shortages[0].required_quantity, 3);
    assert_eq!(shortage.shortages[0].available_quantity, 1);
    assert_eq!(shortage.shortages[0].missing_quantity, 2);

    let answer = engine.calculate_craftable_count(
        "Roast Reindrix",
        &[InventoryEntry::new("Reindrix Venison", 5)],
    );
    assert_eq!(answer.status, AnswerStatus::Ok);
    let craftable = answer.data.expect("craftable result exists");
    assert_eq!(craftable.maximum_additional_count, 5);
    assert_eq!(
        craftable.limiting_material_ids,
        vec!["ITEM_MEAT_ICEDEER".to_string()]
    );
    assert_eq!(
        craftable.material_calculation.tree.item_id,
        "ITEM_BAKEDMEAT_ICEDEER"
    );
    assert_eq!(
        craftable.material_calculation.totals[0].required_quantity,
        1
    );
}

#[test]
fn scales_multiple_outputs_and_aggregates_duplicate_ingredients() {
    let engine = test_store(vec![
        item("ITEM_RESULT", "Result"),
        item("ITEM_WOOD", "Wood"),
        item("ITEM_BONUS", "Bonus"),
        recipe(
            "RECIPE_RESULT",
            ("ITEM_RESULT", 2),
            &[("ITEM_WOOD", 3), ("ITEM_WOOD", 4)],
            &[("ITEM_BONUS", 2)],
        ),
    ]);

    let answer = engine.calculate_materials("Result", 5);
    assert_eq!(answer.status, AnswerStatus::Ok);
    let calculation = answer.data.expect("calculation exists");
    assert_eq!(calculation.totals[0].item_id, "ITEM_WOOD");
    assert_eq!(calculation.totals[0].required_quantity, 21);
    assert_eq!(calculation.byproducts[0].item_id, "ITEM_BONUS");
    assert_eq!(calculation.byproducts[0].quantity, 6);
}

#[test]
fn detects_alternative_recipes_cycles_depth_and_invalid_quantities() {
    let alternative_store = vec![
        item("ITEM_RESULT", "Result"),
        item("ITEM_WOOD", "Wood"),
        recipe("RECIPE_A", ("ITEM_RESULT", 1), &[("ITEM_WOOD", 1)], &[]),
        recipe("RECIPE_B", ("ITEM_RESULT", 1), &[("ITEM_WOOD", 2)], &[]),
    ];
    let engine = test_store(alternative_store);
    let answer = engine.calculate_materials("Result", 1);
    assert_eq!(answer.status, AnswerStatus::Ok);
    let calculation = answer.data.expect("raw fallback calculation exists");
    assert!(calculation.recipe_id.is_none());
    assert!(matches!(
        calculation.tree.acquisition,
        MaterialAcquisition::Raw
    ));
    assert_eq!(calculation.totals[0].item_id, "ITEM_RESULT");
    assert_eq!(calculation.totals[0].required_quantity, 1);
    assert!(answer
        .uncertainty
        .iter()
        .any(|message| message.contains("RECIPE_A") && message.contains("RECIPE_B")));

    let cycle_store = vec![
        item("ITEM_A", "A"),
        item("ITEM_B", "B"),
        recipe("RECIPE_A", ("ITEM_A", 1), &[("ITEM_B", 1)], &[]),
        recipe("RECIPE_B", ("ITEM_B", 1), &[("ITEM_A", 1)], &[]),
    ];
    let engine = test_store(cycle_store);
    let answer = engine.calculate_materials("A", 1);
    assert_eq!(answer.status, AnswerStatus::Error);
    assert!(answer.errors.iter().any(|error| error.contains("cycle")));

    let mut deep_records = Vec::new();
    deep_records.push(item("ITEM_FINAL", "Final"));
    for level in 0..12 {
        let id = format!("ITEM_L{level}");
        let next_id = if level == 0 {
            "ITEM_FINAL".to_string()
        } else {
            format!("ITEM_L{}", level - 1)
        };
        deep_records.push(item(&id, &id));
        deep_records.push(recipe(
            &format!("RECIPE_L{level}"),
            (&next_id, 1),
            &[(&id, 1)],
            &[],
        ));
    }
    let engine = test_store(deep_records);
    let answer = engine.calculate_materials("Final", 1);
    assert_eq!(answer.status, AnswerStatus::Error);
    assert!(answer.errors.iter().any(|error| error.contains("depth")));

    let engine = test_store(vec![item("ITEM_WOOD", "Wood")]);
    let answer = engine.calculate_materials("Wood", 0);
    assert_eq!(answer.status, AnswerStatus::Error);
    assert!(answer.errors.iter().any(|error| error.contains("quantity")));
}

#[test]
fn supports_raw_targets_and_rejects_unknown_inventory() {
    let engine = test_store(vec![item("ITEM_WOOD", "Wood")]);

    let answer = engine.calculate_materials("Wood", 7);
    assert_eq!(answer.status, AnswerStatus::Ok);
    let calculation = answer.data.expect("raw calculation exists");
    assert_eq!(calculation.recipe_id, None);
    assert_eq!(calculation.totals[0].required_quantity, 7);

    let answer = engine.calculate_craftable_count("Wood", &[InventoryEntry::new("Wood", 10)]);
    assert_eq!(answer.status, AnswerStatus::Unknown);
    assert!(answer.data.is_none());

    let answer = engine.calculate_shortage("Wood", 10, &[InventoryEntry::new("Unknown", 1)]);
    assert_eq!(answer.status, AnswerStatus::Error);
    assert!(answer
        .errors
        .iter()
        .any(|error| error.contains("unknown inventory item")));
}

#[test]
fn converts_multi_output_craftable_batches_to_item_count() {
    let engine = test_store(vec![
        item("ITEM_RESULT", "Result"),
        item("ITEM_WOOD", "Wood"),
        recipe(
            "RECIPE_RESULT",
            ("ITEM_RESULT", 5),
            &[("ITEM_WOOD", 1)],
            &[],
        ),
    ]);

    let answer = engine.calculate_craftable_count("Result", &[InventoryEntry::new("Wood", 1)]);
    assert_eq!(answer.status, AnswerStatus::Ok);
    let craftable = answer.data.expect("craftable result exists");
    assert_eq!(craftable.maximum_additional_count, 5);
    assert_eq!(
        craftable.limiting_material_ids,
        vec!["ITEM_WOOD".to_string()]
    );
}

#[test]
fn reports_craftable_count_overflow_for_multiple_outputs() {
    let engine = test_store(vec![
        item("ITEM_RESULT", "Result"),
        item("ITEM_WOOD", "Wood"),
        recipe(
            "RECIPE_RESULT",
            ("ITEM_RESULT", u32::MAX),
            &[("ITEM_WOOD", 1)],
            &[],
        ),
    ]);

    let answer = engine.calculate_craftable_count("Result", &[InventoryEntry::new("Wood", 2)]);
    assert_eq!(answer.status, AnswerStatus::Error);
    assert!(answer.data.is_none());
    assert!(answer
        .errors
        .iter()
        .any(|error| error.contains("craftable count overflow")));
}

#[test]
fn consumes_intermediate_inventory_for_shortage() {
    let engine = test_store(vec![
        item("ITEM_RESULT", "Result"),
        item("ITEM_INGOT", "Ingot"),
        item("ITEM_ORE", "Ore"),
        recipe(
            "RECIPE_RESULT",
            ("ITEM_RESULT", 1),
            &[("ITEM_INGOT", 1)],
            &[],
        ),
        recipe("RECIPE_INGOT", ("ITEM_INGOT", 1), &[("ITEM_ORE", 1)], &[]),
    ]);

    let answer = engine.calculate_shortage("Result", 1, &[InventoryEntry::new("Ingot", 1)]);
    assert_eq!(answer.status, AnswerStatus::Ok);
    let shortage = answer.data.expect("shortage result exists");
    assert!(shortage.shortages.is_empty());
}

#[test]
fn consumes_intermediate_inventory_for_craftable_count() {
    let engine = test_store(vec![
        item("ITEM_RESULT", "Result"),
        item("ITEM_INGOT", "Ingot"),
        item("ITEM_ORE", "Ore"),
        recipe(
            "RECIPE_RESULT",
            ("ITEM_RESULT", 1),
            &[("ITEM_INGOT", 1)],
            &[],
        ),
        recipe("RECIPE_INGOT", ("ITEM_INGOT", 1), &[("ITEM_ORE", 1)], &[]),
    ]);

    let answer = engine.calculate_craftable_count("Result", &[InventoryEntry::new("Ingot", 1)]);
    assert_eq!(answer.status, AnswerStatus::Ok);
    let craftable = answer.data.expect("craftable result exists");
    assert_eq!(craftable.maximum_additional_count, 1);
    assert_eq!(
        craftable.limiting_material_ids,
        vec!["ITEM_ORE".to_string()]
    );
}

#[test]
fn offsets_same_item_byproducts_in_shortage_and_craftable() {
    let engine = test_store(vec![
        item("ITEM_RESULT", "Result"),
        item("ITEM_WOOD", "Wood"),
        item("ITEM_LOG", "Log"),
        recipe(
            "RECIPE_RESULT",
            ("ITEM_RESULT", 1),
            &[("ITEM_WOOD", 2)],
            &[],
        ),
        recipe(
            "RECIPE_WOOD",
            ("ITEM_WOOD", 1),
            &[("ITEM_LOG", 1)],
            &[("ITEM_WOOD", 1)],
        ),
    ]);

    let answer = engine.calculate_shortage("Result", 1, &[]);
    assert_eq!(answer.status, AnswerStatus::Ok);
    let shortage = answer.data.expect("shortage result exists");
    assert_eq!(shortage.shortages.len(), 1);
    assert_eq!(shortage.shortages[0].item_id, "ITEM_LOG");
    assert_eq!(shortage.shortages[0].required_quantity, 1);
    assert_eq!(shortage.shortages[0].missing_quantity, 1);

    let answer = engine.calculate_craftable_count("Result", &[InventoryEntry::new("Log", 1)]);
    assert_eq!(answer.status, AnswerStatus::Ok);
    let craftable = answer.data.expect("craftable result exists");
    assert_eq!(craftable.maximum_additional_count, 1);
    assert_eq!(
        craftable.limiting_material_ids,
        vec!["ITEM_LOG".to_string()]
    );
}

#[test]
fn uses_different_item_byproducts_inside_recipe_tree() {
    let engine = test_store(vec![
        item("ITEM_RESULT", "Result"),
        item("ITEM_WOOD", "Wood"),
        item("ITEM_LOG", "Log"),
        item("ITEM_PLANK", "Plank"),
        recipe(
            "RECIPE_RESULT",
            ("ITEM_RESULT", 1),
            &[("ITEM_WOOD", 2), ("ITEM_PLANK", 1)],
            &[],
        ),
        recipe(
            "RECIPE_WOOD",
            ("ITEM_WOOD", 1),
            &[("ITEM_LOG", 1)],
            &[("ITEM_WOOD", 1), ("ITEM_PLANK", 1)],
        ),
    ]);

    let answer = engine.calculate_shortage("Result", 1, &[]);
    assert_eq!(answer.status, AnswerStatus::Ok);
    let shortage = answer.data.expect("shortage result exists");
    assert_eq!(shortage.shortages.len(), 1);
    assert_eq!(shortage.shortages[0].item_id, "ITEM_LOG");
    assert_eq!(shortage.shortages[0].required_quantity, 1);
}

#[test]
fn cross_item_byproduct_offsets_are_order_independent() {
    let engine = test_store(vec![
        item("ITEM_RESULT_A", "Result A"),
        item("ITEM_RESULT_B", "Result B"),
        item("ITEM_WOOD", "Wood"),
        item("ITEM_FIBER", "Fiber"),
        item("ITEM_LOG", "Log"),
        recipe(
            "RECIPE_RESULT_A",
            ("ITEM_RESULT_A", 1),
            &[("ITEM_WOOD", 1), ("ITEM_FIBER", 1)],
            &[],
        ),
        recipe(
            "RECIPE_RESULT_B",
            ("ITEM_RESULT_B", 1),
            &[("ITEM_FIBER", 1), ("ITEM_WOOD", 1)],
            &[],
        ),
        recipe(
            "RECIPE_WOOD",
            ("ITEM_WOOD", 1),
            &[("ITEM_LOG", 1)],
            &[("ITEM_FIBER", 1)],
        ),
    ]);

    // Fiber is covered by the Wood recipe byproduct; the answer must not depend
    // on the ingredient declaration order of the target recipe.
    for target in ["Result A", "Result B"] {
        let answer = engine.calculate_shortage(target, 1, &[]);
        assert_eq!(answer.status, AnswerStatus::Ok);
        let shortage = answer.data.expect("shortage result exists");
        assert_eq!(shortage.shortages.len(), 1);
        assert_eq!(shortage.shortages[0].item_id, "ITEM_LOG");
        assert_eq!(shortage.shortages[0].required_quantity, 1);
        assert_eq!(shortage.shortages[0].missing_quantity, 1);

        let answer = engine.calculate_craftable_count(target, &[InventoryEntry::new("Log", 1)]);
        assert_eq!(answer.status, AnswerStatus::Ok);
        let craftable = answer.data.expect("craftable result exists");
        assert_eq!(craftable.maximum_additional_count, 1);
    }
}

#[test]
fn does_not_pre_deduct_root_byproducts_from_own_ingredients() {
    let engine = test_store(vec![
        item("ITEM_RESULT", "Result"),
        item("ITEM_WOOD", "Wood"),
        item("ITEM_BONUS", "Bonus"),
        recipe(
            "RECIPE_RESULT",
            ("ITEM_RESULT", 1),
            &[("ITEM_WOOD", 1), ("ITEM_BONUS", 1)],
            &[("ITEM_BONUS", 1)],
        ),
    ]);

    // The Bonus byproduct is produced only after the craft completes, so the
    // first Result needs a Bonus seed; later batches reuse the byproduct.
    let answer = engine.calculate_shortage("Result", 1, &[]);
    assert_eq!(answer.status, AnswerStatus::Ok);
    let shortage = answer.data.expect("shortage result exists");
    assert_eq!(shortage.shortages.len(), 2);
    let wood = shortage
        .shortages
        .iter()
        .find(|entry| entry.item_id == "ITEM_WOOD")
        .expect("Wood shortage exists");
    assert_eq!(wood.required_quantity, 1);
    assert_eq!(wood.missing_quantity, 1);
    let bonus = shortage
        .shortages
        .iter()
        .find(|entry| entry.item_id == "ITEM_BONUS")
        .expect("Bonus shortage exists");
    assert_eq!(bonus.required_quantity, 1);

    let answer = engine.calculate_shortage("Result", 2, &[]);
    assert_eq!(answer.status, AnswerStatus::Ok);
    let shortage = answer.data.expect("shortage result exists");
    let wood = shortage
        .shortages
        .iter()
        .find(|entry| entry.item_id == "ITEM_WOOD")
        .expect("Wood shortage exists");
    assert_eq!(wood.required_quantity, 2);
    let bonus = shortage
        .shortages
        .iter()
        .find(|entry| entry.item_id == "ITEM_BONUS")
        .expect("Bonus shortage exists");
    assert_eq!(bonus.required_quantity, 1);

    let answer = engine.calculate_craftable_count("Result", &[InventoryEntry::new("Wood", 1)]);
    assert_eq!(answer.status, AnswerStatus::Ok);
    let craftable = answer.data.expect("craftable result exists");
    assert_eq!(craftable.maximum_additional_count, 0);

    let answer = engine.calculate_craftable_count(
        "Result",
        &[
            InventoryEntry::new("Wood", 2),
            InventoryEntry::new("Bonus", 1),
        ],
    );
    assert_eq!(answer.status, AnswerStatus::Ok);
    let craftable = answer.data.expect("craftable result exists");
    assert_eq!(craftable.maximum_additional_count, 2);
}

#[test]
fn does_not_pre_deduct_intermediate_byproducts_from_own_ingredients() {
    let engine = test_store(vec![
        item("ITEM_RESULT", "Result"),
        item("ITEM_WOOD", "Wood"),
        item("ITEM_LOG", "Log"),
        item("ITEM_FIBER", "Fiber"),
        recipe(
            "RECIPE_RESULT",
            ("ITEM_RESULT", 1),
            &[("ITEM_WOOD", 1)],
            &[],
        ),
        recipe(
            "RECIPE_WOOD",
            ("ITEM_WOOD", 1),
            &[("ITEM_LOG", 1), ("ITEM_FIBER", 1)],
            &[("ITEM_FIBER", 1)],
        ),
    ]);

    // The Wood recipe consumes Fiber as an ingredient and also produces Fiber
    // as a byproduct; the first batch still needs a Fiber seed.
    let answer = engine.calculate_shortage("Result", 1, &[]);
    assert_eq!(answer.status, AnswerStatus::Ok);
    let shortage = answer.data.expect("shortage result exists");
    assert_eq!(shortage.shortages.len(), 2);
    let fiber = shortage
        .shortages
        .iter()
        .find(|entry| entry.item_id == "ITEM_FIBER")
        .expect("Fiber shortage exists");
    assert_eq!(fiber.required_quantity, 1);
    let log = shortage
        .shortages
        .iter()
        .find(|entry| entry.item_id == "ITEM_LOG")
        .expect("Log shortage exists");
    assert_eq!(log.required_quantity, 1);

    let answer = engine.calculate_craftable_count("Result", &[InventoryEntry::new("Log", 1)]);
    assert_eq!(answer.status, AnswerStatus::Ok);
    let craftable = answer.data.expect("craftable result exists");
    assert_eq!(craftable.maximum_additional_count, 0);

    let answer = engine.calculate_craftable_count(
        "Result",
        &[
            InventoryEntry::new("Log", 2),
            InventoryEntry::new("Fiber", 1),
        ],
    );
    assert_eq!(answer.status, AnswerStatus::Ok);
    let craftable = answer.data.expect("craftable result exists");
    assert_eq!(craftable.maximum_additional_count, 2);
}
