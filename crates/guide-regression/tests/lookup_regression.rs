use game_knowledge::WorldCoordinate;
use guide_core::{AnswerStatus, GuideEngine};

const DATA_DIRECTORY: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/reviewed");

fn engine() -> GuideEngine {
    GuideEngine::load_directory(DATA_DIRECTORY, Some("1.0".to_string()))
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
        assert!(!data.recipe.ingredients.is_empty());
        assert!(!data.recipe.crafting_stations.is_empty());
    }
}

#[test]
fn lookup_recipe_for_a_bare_tiered_name_answers_the_common_recipe() {
    let engine = engine();
    let answer = engine.lookup_recipe("metal armor");
    assert_eq!(answer.status, AnswerStatus::Ok);
    let data = answer.data.expect("recipe data is present");
    assert_eq!(data.recipe.output.item_id, "ITEM_COPPERARMOR");
    assert!(data
        .schematic_leads
        .iter()
        .any(|lead| lead.action.contains("Higher tiers")));
}

#[test]
fn lookup_recipe_for_a_schematic_tier_reports_its_schematic() {
    let engine = engine();
    let answer = engine.lookup_recipe_filtered("metal armor", Some("legendary"));
    assert_eq!(answer.status, AnswerStatus::Ok);
    let data = answer.data.expect("recipe data is present");
    assert_eq!(data.recipe.output.item_id, "ITEM_COPPERARMOR_5");
    let lead = data
        .schematic_leads
        .iter()
        .find(|lead| lead.action.contains("Requires a schematic"))
        .expect("the legendary tier needs its schematic");
    assert!(lead
        .notes
        .as_deref()
        .unwrap_or_default()
        .contains("Metal Armor Schematic 4"));

    let answer = engine.lookup_recipe("Legendary Metal Armor");
    assert_eq!(answer.status, AnswerStatus::Ok);
    assert_eq!(
        answer
            .data
            .expect("a leading tier word selects the higher tier")
            .recipe
            .output
            .item_id,
        "ITEM_COPPERARMOR_5"
    );
}

#[test]
fn lookup_recipe_for_a_world_found_family_claims_no_schematic() {
    let engine = engine();
    let answer = engine.lookup_recipe_filtered("grappling gun", Some("legendary"));
    assert_eq!(answer.status, AnswerStatus::Ok);
    let data = answer.data.expect("recipe data is present");
    assert_eq!(data.recipe.output.item_id, "ITEM_GRAPPLINGGUN_5");
    assert!(data.schematic_leads.is_empty());
}

#[test]
fn lookup_recipe_reports_the_station_from_the_unlocking_schematic() {
    let engine = engine();
    let cases = [
        (
            "refined metal armor",
            "uncommon",
            "ITEM_IRONARMOR_2",
            "Production Assembly Line",
        ),
        (
            "metal bat",
            "uncommon",
            "ITEM_BAT3_2",
            "Weapon Assembly Line",
        ),
        (
            "ancient armor",
            "uncommon",
            "ITEM_ANCIENTARMOR_2",
            "Ancient Workbench",
        ),
    ];
    for (query, rarity, output, station) in cases {
        let answer = engine.lookup_recipe_filtered(query, Some(rarity));
        assert_eq!(answer.status, AnswerStatus::Ok, "{query} resolves");
        let data = answer.data.expect("recipe data is present");
        assert_eq!(
            data.recipe.output.item_id, output,
            "{query} answers its tier"
        );
        assert_eq!(
            data.recipe.crafting_stations,
            vec![station.to_string()],
            "{query} is crafted at the station its schematic names"
        );
    }
}

#[test]
fn lookup_recipe_for_a_single_tier_schematic_names_its_schematic() {
    let engine = engine();
    let answer = engine.lookup_recipe("phantom ring");
    assert_eq!(answer.status, AnswerStatus::Ok);
    let data = answer.data.expect("recipe data is present");
    assert_eq!(data.recipe.output.item_id, "ITEM_ACCESSORY_AVOID_1");
    assert_eq!(
        data.recipe.unlock_item_id.as_deref(),
        Some("ITEM_BLUEPRINT_ACCESSORY_AVOID_1_FIX")
    );
    let lead = data
        .schematic_leads
        .iter()
        .find(|lead| lead.action.contains("Requires a schematic"))
        .expect("an item with no sibling tier still names the schematic that unlocks it");
    assert!(lead
        .notes
        .as_deref()
        .unwrap_or_default()
        .contains("Phantom Ring Schematic"));
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
    assert_eq!(answer.version.knowledge_version, "1.0");
    assert_eq!(
        answer.version.configured_game_version,
        Some("1.0".to_string())
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

#[test]
fn lookup_recipe_answers_every_recipe_for_an_item_that_has_several() {
    let engine = engine();
    let answer = engine.lookup_recipe("carbon fiber");
    assert_eq!(answer.status, AnswerStatus::Ok);
    let data = answer.data.expect("recipe data is present");
    assert_eq!(data.recipe.output.item_id, "ITEM_CARBONFIBER");
    let mut recipes = vec![data.recipe.id.clone()];
    recipes.extend(
        data.alternative_recipes
            .iter()
            .map(|recipe| recipe.id.clone()),
    );
    recipes.sort();
    assert_eq!(recipes, vec!["RECIPE_CARBONFIBER", "RECIPE_CARBONFIBER_2"]);
    assert!(data
        .alternative_recipes
        .iter()
        .all(|recipe| recipe.output.item_id == "ITEM_CARBONFIBER"));
}

#[test]
fn lookup_recipe_answers_all_thirteen_paldium_fragment_recipes() {
    let engine = engine();
    let answer = engine.lookup_recipe("paldium fragment");
    assert_eq!(answer.status, AnswerStatus::Ok);
    let data = answer.data.expect("recipe data is present");
    assert_eq!(data.recipe.output.item_id, "ITEM_PALDIUM_FRAGMENT");
    assert_eq!(data.alternative_recipes.len(), 12);
    let stone = data
        .alternative_recipes
        .iter()
        .find(|recipe| recipe.id == "RECIPE_PAL_CRYSTAL_S_1")
        .expect("the crushed-Stone recipe is part of the set");
    assert_eq!(stone.ingredients[0].item_id, "ITEM_STONE");
    assert_eq!(stone.ingredients[0].quantity, 5);
    assert_eq!(stone.crafting_stations, vec!["Crusher".to_string()]);
}

#[test]
fn locating_a_coordinate_resolves_the_reviewed_region_boundary() {
    let engine = engine();

    let sunlit_isle = engine
        .locate_coordinate(WorldCoordinate {
            x: -336_952.84,
            y: 342_898.47,
            z: 2_221.0,
        })
        .data
        .expect("main map location exists");
    assert_eq!(
        sunlit_isle.approximate_region.as_deref(),
        Some("Sunlit Isle")
    );

    let beyond_sunlit_isle = engine
        .locate_coordinate(WorldCoordinate {
            x: -336_952.84 + 22_500.0,
            y: 342_898.47,
            z: 2_221.0,
        })
        .data
        .expect("main map location exists");
    assert_ne!(
        beyond_sunlit_isle.approximate_region.as_deref(),
        Some("Sunlit Isle")
    );

    let waterlily_gorge = engine
        .locate_coordinate(WorldCoordinate {
            x: -808_495.4,
            y: -89_961.8,
            z: 64_672.0,
        })
        .data
        .expect("main map location exists");
    assert_eq!(
        waterlily_gorge.approximate_region.as_deref(),
        Some("Waterlily Gorge")
    );

    let empty_ocean = engine.locate_coordinate(WorldCoordinate {
        x: -1_000_000.0,
        y: 700_000.0,
        z: 0.0,
    });
    assert_eq!(
        empty_ocean
            .data
            .expect("main map location exists")
            .approximate_region,
        None
    );
    assert!(empty_ocean
        .uncertainty
        .iter()
        .any(|item| item.contains("no reviewed region boundary")));
}
