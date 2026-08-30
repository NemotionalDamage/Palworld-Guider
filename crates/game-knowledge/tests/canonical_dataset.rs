use game_knowledge::{Confidence, ReviewStatus, WorkKind};

const SOURCE_ID: &str = "SRC-PALDB-V1_0_3-20260831";

#[test]
fn canonical_reviewed_dataset_loads_and_propagates_provenance() {
    let store = game_knowledge::KnowledgeStore::load_directory("../../data/reviewed")
        .expect("canonical reviewed dataset must be valid");

    let source = store
        .source(SOURCE_ID)
        .expect("reviewed Paldb source must be registered");
    assert_eq!(source.applicable_game_version, "1.0.3");
    assert_eq!(source.review_status, ReviewStatus::Reviewed);
    assert_eq!(source.confidence, Confidence::ReviewedSecondary);
    assert!(source
        .evidence_urls
        .iter()
        .any(|url| url == "https://paldb.cc/Wood"));
    assert!(source
        .evidence_urls
        .iter()
        .any(|url| url == "https://paldb.cc/Wooden_Club"));
    assert!(source
        .evidence_urls
        .iter()
        .any(|url| url == "https://paldb.cc/Lamball"));

    let wood = store.item("ITEM_WOOD").expect("Wood item exists");
    assert_eq!(wood.names.en, "Wood");
    assert!(wood
        .acquisition_leads
        .iter()
        .any(|lead| lead.action == "Chop trees"));
    assert_eq!(wood.provenance.source_id, SOURCE_ID);
    assert_eq!(wood.provenance.applicable_game_version, "1.0.3");
    assert_eq!(wood.provenance.review_status, ReviewStatus::Reviewed);

    let recipe = store
        .recipe("RECIPE_WOODEN_CLUB")
        .expect("Wooden Club recipe exists");
    assert_eq!(recipe.output.item_id, "ITEM_WOODEN_CLUB");
    assert_eq!(recipe.output.quantity, 1);
    assert_eq!(recipe.ingredients.len(), 1);
    assert_eq!(recipe.ingredients[0].item_id, "ITEM_WOOD");
    assert_eq!(recipe.ingredients[0].quantity, 5);
    assert_eq!(recipe.technology_id.as_deref(), Some("TECHNOLOGY_LEVEL_1"));

    let technology = store
        .technology("TECHNOLOGY_LEVEL_1")
        .expect("technology exists");
    assert_eq!(technology.level, 1);

    let lamball = store.pal("PAL_LAMBALL").expect("Lamball exists");
    assert!(lamball
        .work_suitability
        .iter()
        .any(|work| { work.kind == WorkKind::Handiwork && work.level == 1 }));
    assert!(lamball
        .work_suitability
        .iter()
        .any(|work| { work.kind == WorkKind::Transporting && work.level == 1 }));
    assert!(lamball
        .work_suitability
        .iter()
        .any(|work| work.kind == WorkKind::Farming && work.level == 1));
    assert!(lamball.drops.iter().any(|drop| {
        drop.item_id == "ITEM_WOOL"
            && drop.min_quantity == 1
            && drop.max_quantity == 3
            && drop.probability_percent == 100.0
    }));
    assert!(lamball.drops.iter().any(|drop| {
        drop.item_id == "ITEM_LAMBALL_MUTTON"
            && drop.min_quantity == 1
            && drop.max_quantity == 1
            && drop.probability_percent == 100.0
    }));
}
