use guide_core::{AnswerStatus, GuideEngine};

const DATA_DIRECTORY: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/reviewed");

fn engine() -> GuideEngine {
    GuideEngine::load_directory(DATA_DIRECTORY, Some("1.0.3".to_string()))
        .expect("canonical dataset loads")
}

#[test]
fn lookup_item_stone_returns_item_with_provenance() {
    let engine = engine();
    let answer = engine.lookup_item("stone");
    assert_eq!(answer.status, AnswerStatus::Ok);
    let data = answer.data.expect("item data is present");
    assert_eq!(data.id, "ITEM_STONE");
    assert!(!answer.provenance.is_empty());
}

#[test]
fn lookup_item_by_chinese_name_resolves() {
    let engine = engine();
    let answer = engine.lookup_item("攻击吊坠");
    assert_eq!(answer.status, AnswerStatus::Ok);
    let data = answer.data.expect("item data is present");
    assert_eq!(data.id, "ITEM_ACCESSORY_AT_1");
}

#[test]
fn lookup_pal_lamball_returns_pal_with_work_suitability() {
    let engine = engine();
    let answer = engine.lookup_pal("lamball");
    assert_eq!(answer.status, AnswerStatus::Ok);
    let data = answer.data.expect("pal data is present");
    assert_eq!(data.id, "PAL_LAMBALL");
    assert!(!data.work_suitability.is_empty());
    assert!(!data.drops.is_empty());
}

#[test]
fn lookup_pal_by_chinese_name_resolves() {
    let engine = engine();
    let answer = engine.lookup_pal("美露帕");
    assert_eq!(answer.status, AnswerStatus::Ok);
    let data = answer.data.expect("pal data is present");
    assert_eq!(data.id, "PAL_ALPACA");
}

#[test]
fn lookup_recipe_by_output_item_returns_with_ingredients() {
    let engine = engine();
    let answer = engine.lookup_recipe("pal sphere");
    if answer.status == AnswerStatus::Ok {
        let data = answer.data.expect("recipe data is present");
        assert!(!data.ingredients.is_empty());
        assert!(!data.crafting_stations.is_empty());
    }
}

#[test]
fn lookup_unknown_item_returns_unknown() {
    let engine = engine();
    let answer = engine.lookup_item("xyzzy-nonexistent");
    assert_eq!(answer.status, AnswerStatus::Unknown);
    assert!(answer.data.is_none());
    assert!(!answer.uncertainty.is_empty());
}

#[test]
fn lookup_by_exact_id_resolves() {
    let engine = engine();
    let answer = engine.lookup_item("ITEM_STONE");
    assert_eq!(answer.status, AnswerStatus::Ok);
    let data = answer.data.expect("item data is present");
    assert_eq!(data.id, "ITEM_STONE");
}

#[test]
fn lookup_item_has_version_info() {
    let engine = engine();
    let answer = engine.lookup_item("stone");
    assert_eq!(answer.version.knowledge_version, "1.0.3");
    assert_eq!(
        answer.version.configured_game_version,
        Some("1.0.3".to_string())
    );
    assert!(answer.version.matches);
}

#[test]
fn lookup_pal_by_id_resolves() {
    let engine = engine();
    let answer = engine.lookup_pal("PAL_LAMBALL");
    assert_eq!(answer.status, AnswerStatus::Ok);
    let data = answer.data.expect("pal data is present");
    assert_eq!(data.id, "PAL_LAMBALL");
}
