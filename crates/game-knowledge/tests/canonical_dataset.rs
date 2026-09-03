use game_knowledge::{Confidence, ReviewStatus, WorkKind};

const SOURCE_ID: &str = "SRC-PALDB-V1_0_3-20260831";
const LOCAL_BUILD_SOURCE_ID: &str = "SRC-LOCAL-BUILD-24575825-20260902";

#[test]
fn loads_local_build_source() {
    let store = game_knowledge::KnowledgeStore::load_directory("../../data/reviewed")
        .expect("canonical reviewed dataset must be valid");

    let source = store
        .source(LOCAL_BUILD_SOURCE_ID)
        .expect("local build source exists");

    assert_eq!(source.applicable_game_version, "1.0.3");
    assert_eq!(source.review_status, ReviewStatus::Reviewed);
    assert_eq!(source.confidence, Confidence::VerifiedTarget);
    assert!(source
        .notes
        .as_deref()
        .unwrap()
        .contains("Steam Build 24575825"));
}

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

#[test]
fn loads_first_reviewed_local_build_item_batch() {
    let store = game_knowledge::KnowledgeStore::load_directory("../../data/reviewed")
        .expect("canonical reviewed dataset must be valid");

    assert_eq!(store.items().count(), 367);
    let pendant = store
        .item("ITEM_ACCESSORY_AT_1")
        .expect("first clear local-build item exists");
    assert_eq!(pendant.names.en, "Attack Pendant");
    assert_eq!(pendant.names.zh_hans.as_deref(), Some("攻击吊坠"));
    assert_eq!(
        pendant.description.as_deref(),
        Some("An accessory that slightly raises Attack.")
    );
    assert_eq!(pendant.rarity, "unknown");
    assert!(pendant.acquisition_leads.is_empty());
    assert_eq!(pendant.native_row_id.as_deref(), Some("Accessory_AT_1"));
    let evidence = pendant
        .local_evidence
        .as_ref()
        .expect("promoted local-build item carries evidence metadata");
    assert!(evidence
        .unresolved_fields
        .iter()
        .any(|field| field == "acquisition_leads"));
    assert!(evidence
        .unresolved_fields
        .iter()
        .any(|field| field == "rarity_numeric_semantics"));
    assert_eq!(pendant.provenance.source_id, LOCAL_BUILD_SOURCE_ID);
    assert_eq!(pendant.provenance.review_status, ReviewStatus::Reviewed);

    for item in store.items() {
        if item.provenance.source_id != LOCAL_BUILD_SOURCE_ID {
            continue;
        }
        assert!(!item.names.en.trim().is_empty());
        assert!(item
            .names
            .zh_hans
            .as_deref()
            .is_some_and(|name| !name.trim().is_empty()));
        let description = item.description.as_deref().unwrap_or_default();
        assert!(!description.trim().is_empty());
        assert!(!description.contains('<') && !description.contains('>'));
        assert!(!description.contains("\r") && !description.contains("\n"));
        let evidence = item
            .local_evidence
            .as_ref()
            .unwrap_or_else(|| panic!("local-build item {} must carry evidence metadata", item.id));
        assert!(!evidence.transformation_notes.trim().is_empty());
        if item.acquisition_leads.is_empty() {
            assert!(evidence
                .unresolved_fields
                .iter()
                .any(|field| field == "acquisition_leads"));
        }
        if item.rarity == "unknown" {
            assert!(evidence
                .unresolved_fields
                .iter()
                .any(|field| field == "rarity_numeric_semantics"));
        }
    }
}
