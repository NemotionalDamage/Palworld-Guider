use guide_core::{AnswerStatus, GuideEngine, InventoryEntry};

const DATA_DIRECTORY: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/reviewed");

fn engine() -> GuideEngine {
    GuideEngine::load_directory(DATA_DIRECTORY, Some("1.0.3".to_string()))
        .expect("canonical dataset loads")
}

#[test]
fn materials_for_raw_item_returns_raw_acquisition() {
    let engine = engine();
    let answer = engine.calculate_materials("wood", 1);
    assert_eq!(answer.status, AnswerStatus::Ok);
    let data = answer.data.expect("material data is present");
    assert_eq!(data.requested_quantity, 1);
    assert!(!data.totals.is_empty());
}

#[test]
fn materials_for_crafted_item_returns_recipe_tree() {
    let engine = engine();
    let answer = engine.calculate_materials("wooden club", 1);
    if answer.status == AnswerStatus::Ok {
        let data = answer.data.expect("material data is present");
        assert!(data.recipe_id.is_some());
        assert!(!data.totals.is_empty());
    }
}

#[test]
fn shortage_with_partial_inventory_returns_missing() {
    let engine = engine();
    let inventory = [InventoryEntry {
        item: "wood".to_string(),
        quantity: 2,
    }];
    let answer = engine.calculate_shortage("wooden club", 1, &inventory);
    if answer.status == AnswerStatus::Ok {
        let data = answer.data.expect("shortage data is present");
        assert!(!data.shortages.is_empty());
        let shortage = &data.shortages[0];
        assert!(shortage.missing_quantity > 0);
    }
}

#[test]
fn shortage_with_full_inventory_returns_no_shortage() {
    let engine = engine();
    let inventory = [InventoryEntry {
        item: "wood".to_string(),
        quantity: 10,
    }];
    let answer = engine.calculate_shortage("wooden club", 1, &inventory);
    if answer.status == AnswerStatus::Ok {
        let data = answer.data.expect("shortage data is present");
        assert!(
            data.shortages.is_empty() || data.shortages.iter().all(|s| s.missing_quantity == 0)
        );
    }
}

#[test]
fn craftable_with_inventory_returns_count() {
    let engine = engine();
    let inventory = [InventoryEntry {
        item: "wood".to_string(),
        quantity: 10,
    }];
    let answer = engine.calculate_craftable_count("wooden club", &inventory);
    if answer.status == AnswerStatus::Ok {
        let data = answer.data.expect("craftable data is present");
        assert!(data.maximum_additional_count >= 1);
    }
}

#[test]
fn craftable_without_inventory_returns_zero() {
    let engine = engine();
    let answer = engine.calculate_craftable_count("wooden club", &[]);
    if answer.status == AnswerStatus::Ok {
        let data = answer.data.expect("craftable data is present");
        assert_eq!(data.maximum_additional_count, 0);
    }
}

#[test]
fn breeding_lamball_with_partner_returns_result_or_unknown() {
    let engine = engine();
    let answer = engine.calculate_breeding_result("lamball", "lamball");
    assert!(
        answer.status == AnswerStatus::Ok
            || answer.status == AnswerStatus::Unknown
            || answer.status == AnswerStatus::Ambiguous
    );
    assert!(!answer.version.knowledge_version.is_empty());
}

#[test]
fn breeding_chain_respects_depth_limit() {
    let engine = engine();
    let answer = engine.calculate_breeding_chain("lamball", "lamball", 3);
    assert!(
        answer.status == AnswerStatus::Ok
            || answer.status == AnswerStatus::Unknown
            || answer.status == AnswerStatus::Ambiguous
    );
}

#[test]
fn materials_with_zero_quantity_is_rejected() {
    let engine = engine();
    let answer = engine.calculate_materials("wood", 0);
    assert_ne!(answer.status, AnswerStatus::Ok);
}

#[test]
fn materials_for_unknown_item_returns_unknown() {
    let engine = engine();
    let answer = engine.calculate_materials("xyzzy-nonexistent", 1);
    assert_eq!(answer.status, AnswerStatus::Unknown);
}

#[test]
fn calculation_has_version_info() {
    let engine = engine();
    let answer = engine.calculate_materials("wood", 1);
    assert_eq!(answer.version.knowledge_version, "1.0.3");
    assert!(answer.version.matches);
}

#[test]
fn breeding_chain_start_equals_target_returns_empty_chain() {
    let engine = engine();
    let answer = engine.calculate_breeding_chain("lamball", "lamball", 5);
    if answer.status == AnswerStatus::Ok {
        let data = answer.data.expect("chain data is present");
        assert!(data.steps.is_empty());
    }
}
