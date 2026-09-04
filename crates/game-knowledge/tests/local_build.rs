use game_knowledge::{
    audit_canonical_backfill, candidate_output_is_safe, canonical_backfill_output_is_safe,
    generate_candidates, IntakeBatch, LocalBuildError, LocalBuildLocale, LocalBuildTables,
    LocalizationIndex,
};
use serde_json::{json, Map, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn loads_core_local_build_tables_and_reports_every_row() {
    let root = fixture_root();
    write_core_fixtures(&root);

    let tables = LocalBuildTables::load(&root).expect("core fixtures must load");

    assert_eq!(
        tables
            .coverage()
            .iter()
            .map(|table| table.total_rows)
            .sum::<usize>(),
        60
    );
    assert_eq!(
        tables
            .item("FixtureItem00")
            .expect("valid item exists")
            .max_stack_count,
        Some(9999)
    );
    assert!(!tables
        .item("FixtureItem09")
        .expect("parsed illegal item exists")
        .is_legal_in_game
        .unwrap_or(true));
    assert_eq!(tables.item("FixtureItem04"), None);
    assert_eq!(
        tables
            .recipe("FixtureRecipe00")
            .expect("recipe exists")
            .materials
            .len(),
        2
    );
    assert_eq!(
        tables
            .recipe("FixtureRecipe01")
            .expect("multi-ingredient recipe exists")
            .materials
            .len(),
        5
    );
    assert_eq!(tables.recipe("FixtureRecipe03"), None);
    assert_eq!(
        tables
            .technology("FixtureTechnology00")
            .expect("technology exists")
            .unlock_item_recipes,
        vec!["FixtureRecipe00".to_string()]
    );
    assert_eq!(
        tables
            .pal("FixturePal00")
            .expect("Pal exists")
            .work_suitability("Handcraft"),
        Some(1)
    );
    assert_eq!(
        tables
            .pal("FixturePal00")
            .expect("Pal exists")
            .element_type1,
        Some("EPalElementType::Normal".to_string())
    );
    assert_eq!(
        tables
            .pal("FixturePal01")
            .expect("Pal exists")
            .element_type1,
        Some("EPalElementType::Fire".to_string())
    );
    assert_eq!(
        tables
            .pal("FixturePal05")
            .expect("Pal exists")
            .element_type2,
        Some("EPalElementType::Dark".to_string())
    );
    assert_eq!(tables.drops_for("FixturePal00").len(), 2);

    let all_coverage = tables.coverage();
    let item_coverage = all_coverage
        .iter()
        .find(|table| table.name.ends_with("DT_ItemDataTable.json"))
        .expect("item coverage exists");
    assert_eq!(item_coverage.total_rows, 12);
    assert_eq!(item_coverage.parsed_rows, 11);
    assert_eq!(item_coverage.failed_rows, 1);
    assert_eq!(item_coverage.fields_missing["MaxStackCount"], 1);
    assert_eq!(item_coverage.fields_null["Price"], 1);
    assert_eq!(item_coverage.fields_zero["Price"], 1);
    assert_eq!(item_coverage.fields_empty_string["TypeA"], 1);
    assert_eq!(item_coverage.failure_reasons["malformed_number"], 1);
    assert_eq!(item_coverage.representative_errors.len(), 1);

    let missing = LocalBuildTables::load(&root.join("missing"));
    assert!(matches!(missing, Err(LocalBuildError::MissingTable { .. })));
}

#[test]
fn localization_resolves_locales_and_rejects_placeholders() {
    let root = fixture_root();
    write_core_fixtures(&root);

    let localization = LocalizationIndex::load(&root).expect("localization fixtures must load");

    assert_eq!(
        localization
            .item_name("FixtureItem00", LocalBuildLocale::English)
            .expect("valid English name"),
        Some("Fixture English 0".to_string())
    );
    assert_eq!(
        localization
            .item_name("FixtureItem00", LocalBuildLocale::SimplifiedChinese)
            .expect("valid Chinese name"),
        Some("简体中文0".to_string())
    );
    assert_eq!(
        localization
            .item_name("FixtureItem08", LocalBuildLocale::SimplifiedChinese)
            .expect("missing localization is explicit"),
        None
    );
    assert!(localization
        .item_name("FixtureItem07", LocalBuildLocale::SimplifiedChinese)
        .is_err());
    assert!(localization
        .item_description("FixtureItem00", LocalBuildLocale::English)
        .expect("description exists")
        .is_some());
    assert_eq!(
        localization
            .pal_name("FixturePal00", LocalBuildLocale::English)
            .expect("Pal localization exists"),
        Some("Fixture Pal 0".to_string())
    );

    let coverage = localization.coverage();
    assert_eq!(coverage.len(), 6);
    assert!(coverage.iter().any(|table| table.name.contains("zh-Hans")
        && table.name.contains("DT_ItemNameText_Common.json")
        && table.localization_rejections["placeholder"] >= 1));
}

#[test]
fn parses_preserved_real_export_batch_when_present() {
    let root = Path::new("../../.local/research/local-build/raw");
    if !root.exists() {
        return;
    }

    let tables = LocalBuildTables::load(root).expect("preserved real exports must parse");
    let expected_counts = [
        ("DT_ItemDataTable.json", 2_466),
        ("DT_ItemRecipeDataTable.json", 1_414),
        ("DT_TechnologyRecipeUnlock.json", 588),
        ("DT_PalMonsterParameter.json", 753),
        ("DT_PalDropItem_Common.json", 1_044),
    ];
    for coverage in tables.coverage() {
        let expected = expected_counts
            .iter()
            .find(|(suffix, _)| coverage.name.ends_with(suffix))
            .unwrap_or_else(|| panic!("unexpected coverage table {}", coverage.name))
            .1;
        assert_eq!(coverage.total_rows, expected, "{}", coverage.name);
        assert_eq!(
            coverage.total_rows,
            coverage.parsed_rows + coverage.skipped_rows + coverage.failed_rows,
            "{}",
            coverage.name
        );
        assert_eq!(coverage.failed_rows, 0, "{}", coverage.name);
    }

    let localization = LocalizationIndex::load(root).expect("real localization must parse");
    for coverage in localization.coverage() {
        let expected = if coverage.name.contains("DT_ItemNameText_Common") {
            1_994
        } else if coverage.name.contains("DT_ItemDescriptionText_Common") {
            1_924
        } else if coverage.name.contains("DT_PalNameText_Common") {
            322
        } else if coverage.name.contains("DT_MapObjectNameText_Common") {
            617
        } else if coverage.name.contains("DT_SkillNameText_Common") {
            1_157
        } else {
            3_175
        };
        assert_eq!(coverage.total_rows, expected, "{}", coverage.name);
        assert_eq!(
            coverage.total_rows,
            coverage.parsed_rows + coverage.skipped_rows + coverage.failed_rows,
            "{}",
            coverage.name
        );
    }
}

#[test]
fn generate_candidates_processes_every_row_and_reports_skips() {
    let root = fixture_root();
    write_core_fixtures(&root);
    let tables = LocalBuildTables::load(&root).expect("tables load");
    let localization = LocalizationIndex::load(&root).expect("localization loads");

    let candidates = generate_candidates(&tables, &localization, IntakeBatch::All)
        .expect("candidate generation succeeds");
    let report = &candidates.report;

    assert_eq!(report.input_items, 12);
    assert_eq!(report.candidate_items, 9);
    assert_eq!(report.rejected_items, 3);
    assert_eq!(report.input_recipes, 12);
    assert_eq!(report.candidate_recipes, 8);
    assert_eq!(report.unresolved_recipe_references, 3);
    assert_eq!(report.input_technologies, 12);
    assert_eq!(report.candidate_technologies, 0);
    assert_eq!(report.skipped_technologies, 12);
    assert_eq!(report.input_pals, 12);
    assert_eq!(report.candidate_pals, 12);
    assert_eq!(report.localization_rejections, 1);
    assert!(report.unresolved_probability_units >= 23);
    assert_eq!(report.relationship_candidates, 1);

    let by_table: std::collections::BTreeMap<String, usize> = report
        .row_outcomes
        .iter()
        .map(|outcome| (outcome.table.clone(), 1))
        .fold(
            std::collections::BTreeMap::new(),
            |mut map, (table, count)| {
                *map.entry(table).or_insert(0) += count;
                map
            },
        );
    assert_eq!(by_table["DT_ItemDataTable"], 12);
    assert_eq!(by_table["DT_ItemRecipeDataTable"], 12);
    assert_eq!(by_table["DT_TechnologyRecipeUnlock"], 12);
    assert_eq!(by_table["DT_PalMonsterParameter"], 12);
    assert_eq!(by_table["DT_PalDropItem_Common"], 23);
    assert_eq!(by_table["DT_ItemNameText_Common:en"], 12);
    assert_eq!(by_table["DT_ItemNameText_Common:zh-Hans"], 11);

    assert_eq!(report.anti_bias.seed, 0);
    assert_eq!(report.anti_bias.production_entity_special_cases, 0);
    assert!(report
        .anti_bias
        .fixture_counts
        .values()
        .all(|count| *count >= 12));
    assert!(report.row_outcomes.iter().all(|outcome| {
        matches!(
            outcome.outcome.as_str(),
            "candidate" | "skip" | "failure" | "embedded_candidate"
        ) && outcome.stable_hash != 0
    }));
}

#[test]
fn candidate_output_must_stay_under_local_research_directory() {
    assert!(candidate_output_is_safe(Path::new(
        ".local/research/local-build/candidates/items.jsonl"
    )));
    assert!(candidate_output_is_safe(Path::new(
        "C:/repo/.local/research/local-build/candidates/items.jsonl"
    )));
    assert!(!candidate_output_is_safe(Path::new(
        "data/reviewed/facts.jsonl"
    )));
    assert!(!candidate_output_is_safe(Path::new(
        ".local/research/local-build/../secret/items.jsonl"
    )));
}

#[test]
fn canonical_backfill_output_must_use_the_reviewed_report_path() {
    assert!(canonical_backfill_output_is_safe(Path::new(
        ".local/research/local-build/reports/canonical-backfill.json"
    )));
    assert!(canonical_backfill_output_is_safe(Path::new(
        "C:/repo/.local/research/local-build/reports/canonical-backfill.json"
    )));
    assert!(!canonical_backfill_output_is_safe(Path::new(
        ".local/research/local-build/reports/other-report.json"
    )));
    assert!(!canonical_backfill_output_is_safe(Path::new(
        ".local/research/local-build/reports/canonical-backfill.json/../report.json"
    )));
}

#[test]
fn audits_every_canonical_fact_when_real_exports_are_present() {
    let root = Path::new("../../.local/research/local-build/raw");
    if !root.exists() {
        return;
    }
    let tables = LocalBuildTables::load(root).expect("real tables load");
    let localization = LocalizationIndex::load(root).expect("real localization loads");
    let store = game_knowledge::KnowledgeStore::load_directory("../../data/reviewed")
        .expect("canonical dataset loads");

    let report = audit_canonical_backfill(&tables, &localization, &store);

    let head_description = localization
        .expanded_item_description("Head003", LocalBuildLocale::English)
        .expect("Head003 description localization resolves")
        .expect("Head003 description exists");
    assert!(head_description.contains("Ribbuny"));
    assert!(!head_description.contains('<'));
    let blueprint_description = localization
        .expanded_item_description("Blueprint_ClothArmorCold_2", LocalBuildLocale::English)
        .expect("blueprint description localization resolves")
        .expect("blueprint description exists");
    assert!(blueprint_description.contains("Tundra Outfit (Uncommon)"));
    assert!(blueprint_description.contains("Primitive Workbench"));
    let arrow_description = localization
        .expanded_item_description("Arrow_Fire", LocalBuildLocale::English)
        .expect("arrow description localization resolves")
        .expect("arrow description exists");
    assert!(arrow_description.starts_with("Fire Arrows for use with bows."));
    assert!(arrow_description.contains("Fire damage on contact"));
    let awakening_description = localization
        .expanded_item_description("PalAwakening_Fire", LocalBuildLocale::English)
        .expect("awakening description localization resolves")
        .expect("awakening description exists");
    assert!(
        awakening_description.contains("awakens Fire Pals"),
        "unexpected awakening expansion: {awakening_description:?}"
    );
    let implant_description = localization
        .expanded_item_description("PalPassiveSkillChange", LocalBuildLocale::English)
        .expect("implant description localization resolves")
        .expect("implant description exists");
    assert!(implant_description.contains("select the corresponding passive skill"));

    assert_eq!(report.summary.total_records, 3634);
    assert_eq!(report.summary.total_audited_facts, 8231);
    assert_eq!(report.summary.unclassified_facts, 0);
    assert_eq!(report.summary.missing_localization, 0);
    assert!(report.facts.iter().any(|fact| {
        fact.canonical_record_id == "ALIAS_PAL_ALPACA_ZH_HANS"
            && fact.canonical_field == "alias"
            && fact.classification == "corroborated_exact"
            && fact.current_value.contains("美露帕")
    }));
    assert!(report.facts.len() >= 90);
    let whitespace_description_conflicts = report
        .facts
        .iter()
        .filter(|fact| {
            fact.canonical_field == "description" && fact.classification == "conflicting"
        })
        .count();
    assert_eq!(whitespace_description_conflicts, 0);
    assert!(report
        .summary
        .classification_counts
        .contains_key("corroborated_exact"));
    assert!(report
        .summary
        .classification_counts
        .contains_key("corroborated_partial"));
    assert!(report
        .summary
        .classification_counts
        .contains_key("conflicting"));
    assert!(report
        .summary
        .classification_counts
        .contains_key("not_represented_locally"));
    assert!(report
        .summary
        .classification_counts
        .contains_key("unresolved_mapping"));
    assert!(report.facts.iter().any(|fact| {
        fact.canonical_record_id == "ITEM_WOODEN_CLUB"
            && fact.canonical_field == "names.en"
            && fact.classification == "conflicting"
            && fact.current_value.contains("Wooden Club")
            && fact.local_value.contains("Stone Axe")
    }));
    assert!(report.facts.iter().any(|fact| {
        fact.canonical_record_id == "RECIPE_WOODEN_CLUB"
            && fact.canonical_field == "ingredients"
            && fact.classification == "conflicting"
    }));
    assert!(report.facts.iter().any(|fact| {
        fact.canonical_record_id == "ITEM_BAKED_BERRIES"
            && fact.canonical_field == "description"
            && fact.classification == "corroborated_partial"
            && fact.difference_explanation
                == "the reviewed text differs only by target-build whitespace"
    }));
}

fn fixture_root() -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("palworld-guider-local-build-{unique}"))
}

fn write_core_fixtures(root: &Path) {
    let mut items = Map::new();
    let mut recipes = Map::new();
    let mut technologies = Map::new();
    let mut pals = Map::new();
    let mut drops = Map::new();
    let mut english_names = Map::new();
    let mut chinese_names = Map::new();
    let mut english_descriptions = Map::new();
    let mut chinese_descriptions = Map::new();
    let mut english_pal_names = Map::new();
    let mut chinese_pal_names = Map::new();

    for index in 0..12 {
        let item_id = format!("FixtureItem{index:02}");
        let mut item = Map::new();
        item.insert("TypeA".into(), json!("EPalItemTypeA::Material"));
        item.insert("TypeB".into(), json!("EPalItemTypeB::Material"));
        item.insert("Rank".into(), json!(1));
        item.insert("Rarity".into(), json!(index % 5));
        if index != 8 {
            item.insert("MaxStackCount".into(), json!(9999));
        }
        if index == 4 {
            item.insert("MaxStackCount".into(), json!("not-a-number"));
        }
        item.insert("Weight".into(), json!(1.0));
        if index == 6 {
            item.insert("Price".into(), Value::Null);
        } else {
            item.insert("Price".into(), json!(if index == 5 { 0 } else { 1 }));
        }
        item.insert("bLegalInGame".into(), json!(index != 9));
        if index != 7 {
            item.insert("TechnologyTreeLock".into(), json!(0));
        }
        if index == 10 {
            item.insert("TypeA".into(), json!(""));
        }
        items.insert(item_id.clone(), Value::Object(item));

        let recipe_id = format!("FixtureRecipe{index:02}");
        let mut recipe = Map::new();
        recipe.insert("Product_Id".into(), json!(item_id));
        if index == 3 {
            recipe.insert("Product_Count".into(), json!("not-a-number"));
        } else {
            recipe.insert("Product_Count".into(), json!(1));
        }
        recipe.insert("WorkAmount".into(), json!(100.0));
        recipe.insert("WorkableAttribute".into(), json!(0));
        recipe.insert("UnlockItemID".into(), json!("None"));
        recipe.insert("Material1_Id".into(), json!("FixtureItem00"));
        recipe.insert("Material1_Count".into(), json!(1));
        recipe.insert("Material2_Id".into(), json!("FixtureItem01"));
        recipe.insert("Material2_Count".into(), json!(2));
        for material_index in 3..=5 {
            let identifier = if index == 1 {
                match material_index {
                    3 => "FixtureItem02".to_string(),
                    4 => "FixtureItem05".to_string(),
                    _ => "FixtureItem06".to_string(),
                }
            } else {
                "None".to_string()
            };
            recipe.insert(format!("Material{material_index}_Id"), json!(identifier));
            recipe.insert(
                format!("Material{material_index}_Count"),
                json!(if index == 1 { 3 } else { 0 }),
            );
        }
        recipes.insert(recipe_id.clone(), Value::Object(recipe));

        let mut technology = Map::new();
        technology.insert("UnlockBuildObjects".into(), json!([]));
        technology.insert(
            "UnlockItemRecipes".into(),
            json!(if index == 0 {
                vec![recipe_id]
            } else {
                Vec::<String>::new()
            }),
        );
        technology.insert("RequireTechnology".into(), json!("None"));
        technology.insert("RequireResearchId".into(), json!("None"));
        technology.insert("LevelCap".into(), json!(index + 1));
        technology.insert("Tier".into(), json!(index));
        technology.insert("Cost".into(), json!(index));
        technologies.insert(
            format!("FixtureTechnology{index:02}"),
            Value::Object(technology),
        );

        let pal_id = format!("FixturePal{index:02}");
        let mut pal = Map::new();
        pal.insert("IsPal".into(), json!(true));
        pal.insert("Hp".into(), json!(70));
        pal.insert("MeleeAttack".into(), json!(70));
        pal.insert("ShotAttack".into(), json!(70));
        pal.insert("Defense".into(), json!(70));
        pal.insert(
            "ElementType1".into(),
            json!(match index % 3 {
                0 => "EPalElementType::Normal",
                1 => "EPalElementType::Fire",
                _ => "EPalElementType::Water",
            }),
        );
        pal.insert(
            "ElementType2".into(),
            json!(if index == 5 {
                "EPalElementType::Dark"
            } else {
                "EPalElementType::None"
            }),
        );
        for field in [
            "EmitFlame",
            "Watering",
            "Seeding",
            "GenerateElectricity",
            "Handcraft",
            "Collection",
            "Deforest",
            "Mining",
            "OilExtraction",
            "ProductMedicine",
            "Cool",
            "Transport",
            "MonsterFarm",
        ] {
            pal.insert(
                format!("WorkSuitability_{field}"),
                json!(if field == "Handcraft" { 1 } else { 0 }),
            );
        }
        pals.insert(pal_id.clone(), Value::Object(pal));

        let mut drop = Map::new();
        drop.insert("CharacterID".into(), json!(pal_id));
        drop.insert("Level".into(), json!(index));
        drop.insert("ItemId1".into(), json!("FixtureDropA"));
        drop.insert("Rate1".into(), json!(100.0));
        drop.insert("min1".into(), json!(1));
        drop.insert("Max1".into(), json!(3));
        drop.insert(
            "ItemId2".into(),
            json!(if index == 11 { "None" } else { "FixtureDropB" }),
        );
        drop.insert("Rate2".into(), json!(100.0));
        drop.insert("min2".into(), json!(1));
        drop.insert("Max2".into(), json!(1));
        for drop_index in 3..=10 {
            drop.insert(format!("ItemId{drop_index}"), json!("None"));
            drop.insert(format!("Rate{drop_index}"), json!(0.0));
            drop.insert(format!("min{drop_index}"), json!(0));
            drop.insert(format!("Max{drop_index}"), json!(0));
        }
        drops.insert(format!("FixtureDrop{index:02}"), Value::Object(drop));

        english_names.insert(
            format!("ITEM_NAME_{item_id}"),
            localization_row(&format!("Fixture English {index}")),
        );
        if index != 8 {
            let chinese = if index == 7 {
                "zh-hans text".to_string()
            } else {
                format!("简体中文{index}")
            };
            chinese_names.insert(format!("ITEM_NAME_{item_id}"), localization_row(&chinese));
        }
        english_descriptions.insert(
            format!("ITEM_DESC_{item_id}"),
            localization_row(&format!("Fixture description {index}")),
        );
        chinese_descriptions.insert(
            format!("ITEM_DESC_{item_id}"),
            localization_row(&format!("简体中文描述{index}")),
        );
        english_pal_names.insert(
            format!("PAL_NAME_{pal_id}"),
            localization_row(&format!("Fixture Pal {index}")),
        );
        chinese_pal_names.insert(
            format!("PAL_NAME_{pal_id}"),
            localization_row(&format!("简体中文帕鲁{index}")),
        );
    }

    write_table(
        &root.join("Pal/Content/Pal/DataTable/Item/DT_ItemDataTable.json"),
        &items,
    );
    write_table(
        &root.join("Pal/Content/Pal/DataTable/Item/DT_ItemRecipeDataTable.json"),
        &recipes,
    );
    write_table(
        &root.join("Pal/Content/Pal/DataTable/Technology/DT_TechnologyRecipeUnlock.json"),
        &technologies,
    );
    write_table(
        &root.join("Pal/Content/Pal/DataTable/Character/DT_PalMonsterParameter.json"),
        &pals,
    );
    write_table(
        &root.join("Pal/Content/Pal/DataTable/Character/DT_PalDropItem_Common.json"),
        &drops,
    );
    write_table(
        &root.join("Pal/Content/L10N/en/Pal/DataTable/Text/DT_ItemNameText_Common.json"),
        &english_names,
    );
    write_table(
        &root.join("Pal/Content/L10N/zh-Hans/Pal/DataTable/Text/DT_ItemNameText_Common.json"),
        &chinese_names,
    );
    write_table(
        &root.join("Pal/Content/L10N/en/Pal/DataTable/Text/DT_ItemDescriptionText_Common.json"),
        &english_descriptions,
    );
    write_table(
        &root
            .join("Pal/Content/L10N/zh-Hans/Pal/DataTable/Text/DT_ItemDescriptionText_Common.json"),
        &chinese_descriptions,
    );
    write_table(
        &root.join("Pal/Content/L10N/en/Pal/DataTable/Text/DT_PalNameText_Common.json"),
        &english_pal_names,
    );
    write_table(
        &root.join("Pal/Content/L10N/zh-Hans/Pal/DataTable/Text/DT_PalNameText_Common.json"),
        &chinese_pal_names,
    );
}

fn localization_row(value: &str) -> Value {
    json!({
        "TextData": {
            "Namespace": "Fixture",
            "Key": "FixtureKey",
            "SourceString": value,
            "LocalizedString": value
        }
    })
}

fn write_table(path: &Path, rows: &Map<String, Value>) {
    fs::create_dir_all(path.parent().expect("table path has parent"))
        .expect("create fixture directory");
    let export = json!([{ "Rows": Value::Object(rows.clone()) }]);
    fs::write(
        path,
        serde_json::to_vec_pretty(&export).expect("serialize fixture"),
    )
    .expect("write fixture");
}
