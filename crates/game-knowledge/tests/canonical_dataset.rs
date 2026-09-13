use game_knowledge::{Confidence, ReviewStatus, WorkKind};

const SOURCE_ID: &str = "SRC-PALDB-V1_0-20260831";
const LOCAL_BUILD_SOURCE_ID: &str = "SRC-LOCAL-BUILD-24575825-20260902";
const MATERIAL_SOURCE_ID: &str = "SRC-PALDB-MATERIALS-V1_0-20260912";

#[test]
fn loads_local_build_source() {
    let store = game_knowledge::KnowledgeStore::load_directory("../../data/reviewed")
        .expect("canonical reviewed dataset must be valid");

    let source = store
        .source(LOCAL_BUILD_SOURCE_ID)
        .expect("local build source exists");

    assert_eq!(source.applicable_game_version, "1.0");
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
    assert_eq!(source.applicable_game_version, "1.0");
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
    assert_eq!(wood.names.zh_hans.as_deref(), Some("木材"));
    assert_eq!(
        wood.description.as_deref(),
        Some(
            "Material for structures and items. Obtained by chopping down trees. Can be broken down at a Crusher to obtain Fiber."
        )
    );
    assert!(wood
        .acquisition_leads
        .iter()
        .any(|lead| lead.action == "Chop trees"));
    assert_eq!(wood.provenance.source_id, MATERIAL_SOURCE_ID);
    assert_eq!(wood.provenance.applicable_game_version, "1.0");
    assert_eq!(wood.provenance.review_status, ReviewStatus::Reviewed);

    let materials = store
        .source(MATERIAL_SOURCE_ID)
        .expect("reviewed Paldb materials source must be registered");
    assert_eq!(materials.retrieved_on, "2026-09-12");
    assert!(materials
        .evidence_urls
        .iter()
        .any(|url| url == "https://paldb.cc/cn/Wool"));

    let wool = store.item("ITEM_WOOL").expect("Wool item exists");
    assert_eq!(wool.names.zh_hans.as_deref(), Some("羊毛"));
    assert_eq!(wool.rarity, "Common");
    assert!(wool
        .acquisition_leads
        .iter()
        .any(|lead| lead.action == "Dropped by Lamball"));
    assert_eq!(wool.provenance.source_id, MATERIAL_SOURCE_ID);

    let mutton = store
        .item("ITEM_LAMBALL_MUTTON")
        .expect("Lamball Mutton item exists");
    assert_eq!(mutton.names.zh_hans.as_deref(), Some("棉悠悠的羊肉"));
    assert_eq!(mutton.rarity, "Common");
    assert_eq!(mutton.provenance.source_id, MATERIAL_SOURCE_ID);

    let lamball = store.pal("PAL_LAMBALL").expect("Lamball exists");
    assert_eq!(lamball.names.zh_hans.as_deref(), Some("棉悠悠"));
    assert!(lamball
        .provenance
        .corroborating_source_ids
        .iter()
        .any(|id| id == MATERIAL_SOURCE_ID));

    let recipe = store
        .recipe("RECIPE_BAT")
        .expect("local-build Wooden Club recipe exists");
    assert_eq!(recipe.output.item_id, "ITEM_BAT");
    assert_eq!(recipe.output.quantity, 1);
    assert_eq!(recipe.ingredients.len(), 1);
    assert_eq!(recipe.ingredients[0].item_id, "ITEM_WOOD");
    assert_eq!(recipe.ingredients[0].quantity, 5);
    assert_eq!(
        recipe.technology_id.as_deref(),
        Some("TECH_BATTLE_MELEEWEAPON_BAT")
    );

    let wooden_club = store
        .item("ITEM_BAT")
        .expect("local-build Wooden Club item exists");
    assert_eq!(wooden_club.names.en, "Wooden Club");
    assert_eq!(wooden_club.names.zh_hans.as_deref(), Some("木棒"));
    let bat = store
        .item("ITEM_BAT2")
        .expect("local-build Bat item exists");
    assert_eq!(bat.names.en, "Bat");

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

    assert_eq!(store.items().count(), 1892);
    let pendant = store
        .item("ITEM_ACCESSORY_AT_1")
        .expect("first clear local-build item exists");
    assert_eq!(pendant.names.en, "Attack Pendant");
    assert_eq!(pendant.names.zh_hans.as_deref(), Some("攻击吊坠"));
    assert_eq!(
        pendant.description.as_deref(),
        Some("An accessory that slightly raises Attack.")
    );
    assert_eq!(pendant.rarity, "Rare");
    assert!(!pendant.acquisition_leads.is_empty());
    assert_eq!(pendant.native_row_id.as_deref(), Some("Accessory_AT_1"));
    let evidence = pendant
        .local_evidence
        .as_ref()
        .expect("promoted local-build item carries evidence metadata");
    assert!(!evidence
        .unresolved_fields
        .iter()
        .any(|field| field == "acquisition_leads"));
    assert!(!evidence
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
        assert!(!item.acquisition_leads.is_empty());
        assert!(item.rarity != "unknown");
    }
}

#[test]
fn loads_first_reviewed_local_build_pal_batch() {
    let store = game_knowledge::KnowledgeStore::load_directory("../../data/reviewed")
        .expect("canonical reviewed dataset must be valid");

    assert_eq!(store.pals().count(), 299);
    let melpaca = store
        .pal("PAL_ALPACA")
        .expect("first clear local-build Pal exists");
    assert_eq!(melpaca.names.en, "Melpaca");
    assert_eq!(melpaca.names.zh_hans.as_deref(), Some("美露帕"));
    assert!(!melpaca.work_suitability.is_empty());
    assert!(melpaca
        .work_suitability
        .iter()
        .any(|w| w.kind == game_knowledge::WorkKind::Farming && w.level == 2));
    assert!(!melpaca.drops.is_empty());
    assert!(!melpaca.habitat_ids.is_empty());
    assert_eq!(melpaca.native_row_id.as_deref(), Some("Alpaca"));
    let evidence = melpaca
        .local_evidence
        .as_ref()
        .expect("promoted local-build Pal carries evidence metadata");
    assert!(evidence
        .unresolved_fields
        .iter()
        .all(|field| field == "habitat_ids"));
    assert_eq!(melpaca.provenance.source_id, LOCAL_BUILD_SOURCE_ID);
    assert_eq!(melpaca.provenance.review_status, ReviewStatus::Reviewed);

    let fishing_only = store
        .pal("PAL_PENGUIN_ELECTRIC")
        .expect("fishing-only Pal exists");
    assert!(fishing_only.habitat_ids.is_empty());
    assert_eq!(
        fishing_only.wild_spawn_review,
        Some(game_knowledge::WildSpawnReview::ReviewedFishingOnly)
    );
    assert!(fishing_only
        .habitat_leads
        .iter()
        .any(|lead| lead.action == "Fishing spots"));
    let raid_only = store.pal("PAL_NIGHTLADY").expect("raid-only Pal exists");
    assert!(raid_only.habitat_ids.is_empty());
    assert_eq!(
        raid_only.wild_spawn_review,
        Some(game_knowledge::WildSpawnReview::ReviewedAbsent)
    );
    assert!(raid_only.habitat_leads.is_empty());

    let tribe_only = store
        .pal("PAL_FLOWERPRINCE")
        .expect("tribe-stronghold Pal exists");
    assert_eq!(
        tribe_only.wild_spawn_review,
        Some(game_knowledge::WildSpawnReview::ReviewedTribeOnly)
    );
    assert!(tribe_only
        .habitat_leads
        .iter()
        .any(|lead| lead.action == "Tribe stronghold"));

    let event_only = store.pal("PAL_DARKALIEN").expect("event-only Pal exists");
    assert_eq!(
        event_only.wild_spawn_review,
        Some(game_knowledge::WildSpawnReview::ReviewedEventOnly)
    );

    let boss_only = store.pal("PAL_KINGWHALE").expect("boss-only Pal exists");
    assert_eq!(
        boss_only.wild_spawn_review,
        Some(game_knowledge::WildSpawnReview::ReviewedBossOnly)
    );
    assert!(boss_only
        .habitat_leads
        .iter()
        .any(|lead| lead.action == "Boss encounter"));

    let uncapturable = store
        .pal("PAL_WORLDTREEDRAGON")
        .expect("uncapturable Pal exists");
    assert_eq!(
        uncapturable.wild_spawn_review,
        Some(game_knowledge::WildSpawnReview::ReviewedUncapturable)
    );
    assert!(uncapturable.work_suitability.is_empty());
    assert_eq!(
        uncapturable
            .local_evidence
            .as_ref()
            .map(|evidence| evidence.reviewed_empty_fields.clone())
            .unwrap_or_default(),
        vec!["work_suitability".to_string()]
    );

    let self_only_raid = store
        .pal("PAL_RAID_YAKUSHIMABOSS001_GREEN")
        .expect("self-only raid Pal exists");
    assert!(self_only_raid.breeding_combi_rank.is_none());
    assert!(self_only_raid.breeding_self_only);
    assert!(store.breeding_rules().any(|rule| {
        rule.parent_a_id == "PAL_RAID_YAKUSHIMABOSS001_GREEN"
            && rule.parent_b_id == "PAL_RAID_YAKUSHIMABOSS001_GREEN"
            && rule.child_id == "PAL_RAID_YAKUSHIMABOSS001_GREEN"
    }));

    let uncapturable_raid = store
        .pal("PAL_RAID_YAKUSHIMABOSS002")
        .expect("uncapturable raid Pal exists");
    assert!(uncapturable_raid.breeding_combi_rank.is_none());
    assert!(!uncapturable_raid.breeding_self_only);
    assert!(!store
        .breeding_rules()
        .any(|rule| rule.child_id == "PAL_RAID_YAKUSHIMABOSS002"));

    for pal in store.pals() {
        if pal.provenance.source_id != LOCAL_BUILD_SOURCE_ID {
            continue;
        }
        assert!(!pal.names.en.trim().is_empty());
        assert!(pal
            .names
            .zh_hans
            .as_deref()
            .is_some_and(|name| !name.trim().is_empty()));
        assert!(!pal.names.en.eq_ignore_ascii_case("en_text"));
        assert_ne!(pal.names.en, "Unidentified Pal");
        assert_ne!(pal.names.zh_hans.as_deref(), Some("zh_Hans_Text"));
        let habitat_resolved = !pal.habitat_ids.is_empty() || pal.wild_spawn_review.is_some();
        assert_eq!(
            habitat_resolved,
            !pal.local_evidence.as_ref().is_some_and(|evidence| evidence
                .unresolved_fields
                .iter()
                .any(|field| field == "habitat_ids"))
        );
        let work_reviewed_empty = pal.local_evidence.as_ref().is_some_and(|evidence| {
            evidence
                .reviewed_empty_fields
                .iter()
                .any(|field| field == "work_suitability")
        });
        if work_reviewed_empty {
            assert!(pal.work_suitability.is_empty());
        }
        let combi_rank_reviewed_empty = pal.local_evidence.as_ref().is_some_and(|evidence| {
            evidence
                .reviewed_empty_fields
                .iter()
                .any(|field| field == "breeding_combi_rank")
        });
        assert_eq!(pal.breeding_combi_rank.is_none(), combi_rank_reviewed_empty);
        assert!(pal.local_evidence.as_ref().is_some_and(|evidence| evidence
            .unresolved_fields
            .iter()
            .all(|field| field != "drop_probability_representation"
                && field != "work_suitability_semantics")));
    }

    let no_work: Vec<_> = store
        .pals()
        .filter(|pal| {
            pal.provenance.source_id == LOCAL_BUILD_SOURCE_ID && pal.work_suitability.is_empty()
        })
        .map(|pal| pal.id.as_str())
        .collect();
    assert_eq!(
        no_work.len(),
        3,
        "exactly 3 local-build Pals have no work suitability: {no_work:?}"
    );
}

#[test]
fn promotes_only_complete_recipe_alias_and_progression_candidates() {
    let store = game_knowledge::KnowledgeStore::load_directory("../../data/reviewed")
        .expect("canonical reviewed dataset must be valid");

    assert_eq!(store.aliases().count(), 1806);
    for (alias_id, alias_value, target_id) in [
        ("ALIAS_ITEM_WOOD_ZH_HANS", "木材", "ITEM_WOOD"),
        ("ALIAS_ITEM_WOOL_ZH_HANS", "羊毛", "ITEM_WOOL"),
    ] {
        let alias = store
            .aliases()
            .find(|alias| alias.id == alias_id)
            .expect("complete local-build Item alias exists");
        assert_eq!(alias.alias, alias_value);
        assert_eq!(alias.target_id, target_id);
        assert_eq!(alias.locale, "zh_hans");
        assert_eq!(alias.provenance.source_id, LOCAL_BUILD_SOURCE_ID);
        assert_eq!(alias.provenance.review_status, ReviewStatus::Reviewed);
    }

    assert_eq!(store.recipes().count(), 1288);
    assert_eq!(store.progression_relationships().count(), 383);
    assert!(store
        .recipes()
        .any(|recipe| recipe.provenance.source_id == LOCAL_BUILD_SOURCE_ID));
    assert!(store
        .progression_relationships()
        .any(|relationship| relationship.provenance.source_id == LOCAL_BUILD_SOURCE_ID));
}
