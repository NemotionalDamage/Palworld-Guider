use game_knowledge::{
    AcquisitionLead, AliasRecord, BreedingRuleRecord, Confidence, ConflictRecord,
    ConflictResolution, DropSource, HabitatRecord, ItemRecord, KnowledgeRecord, KnowledgeStore,
    LocaleNames, PalRecord, ProgressionRelationKind, ProgressionRelationshipRecord, Provenance,
    RecipeIngredient, RecipeItem, RecipeRecord, ReviewStatus, SourceRecord, TechnologyRecord,
    WorkKind, WorkSuitability,
};
use std::fs;

const SOURCE_ID: &str = "SRC-PALDB-20260831";

#[test]
fn load_directory_reads_class_files_without_a_monolithic_facts_file() {
    let unique = format!("palworld-guider-class-files-{}", std::process::id());
    let root = std::env::temp_dir().join(unique);
    fs::create_dir_all(&root).expect("create temporary knowledge directory");
    fs::write(
        root.join("sources.jsonl"),
        serde_json::to_string(&KnowledgeRecord::Source(source())).unwrap(),
    )
    .expect("write sources");
    fs::write(
        root.join("items.jsonl"),
        serde_json::to_string(&item_record("ITEM_CLASS_FILE", None)).unwrap(),
    )
    .expect("write items");

    let store = KnowledgeStore::load_directory(&root)
        .expect("classified item file must load without facts.jsonl");
    assert!(store.item("ITEM_CLASS_FILE").is_some());

    fs::remove_file(root.join("items.jsonl")).expect("remove items");
    fs::remove_file(root.join("sources.jsonl")).expect("remove sources");
    fs::remove_dir(&root).expect("remove temporary knowledge directory");
}

fn source() -> SourceRecord {
    SourceRecord {
        id: SOURCE_ID.to_string(),
        title: "Paldb reviewed item and Pal pages".to_string(),
        supplier: "project owner".to_string(),
        retrieved_on: "2026-08-31".to_string(),
        evidence_urls: vec!["https://paldb.cc/Wood".to_string()],
        applicable_game_version: "1.0.3".to_string(),
        reviewer: "Codex".to_string(),
        review_status: ReviewStatus::Reviewed,
        confidence: Confidence::ReviewedSecondary,
        notes: Some("Fan-maintained database; reviewed field by field.".to_string()),
    }
}

#[test]
fn local_identity_preserves_legacy_records_and_rejects_duplicates() {
    let legacy_json = r#"{
        "record_type": "item",
        "id": "ITEM_LEGACY",
        "names": { "en": "Legacy" },
        "description": null,
        "rarity": "common",
        "acquisition_leads": [],
        "provenance": {
            "source_id": "SRC-PALDB-20260831",
            "applicable_game_version": "1.0.3",
            "retrieved_on": "2026-08-31",
            "reviewer": "Codex",
            "review_status": "reviewed",
            "confidence": "reviewed_secondary"
        }
    }"#;
    let legacy: KnowledgeRecord = serde_json::from_str(legacy_json).unwrap();

    assert!(KnowledgeStore::from_records(vec![KnowledgeRecord::Source(source()), legacy]).is_ok());

    let mut first = item_record("ITEM_FIRST", Some("SharedNativeRow"));
    let mut second = item_record("ITEM_SECOND", Some("SharedNativeRow"));
    if let (KnowledgeRecord::Item(first), KnowledgeRecord::Item(second)) = (&mut first, &mut second)
    {
        first.provenance.corroborating_source_ids = vec![SOURCE_ID.to_string()];
        second.provenance.corroborating_source_ids = vec!["SRC_MISSING".to_string()];
    }

    let errors =
        KnowledgeStore::from_records(vec![KnowledgeRecord::Source(source()), first, second])
            .expect_err("duplicate native identity and unregistered corroboration must fail");

    assert!(errors
        .iter()
        .any(|error| error.message.contains("duplicate native row ID")));
    assert!(errors
        .iter()
        .any(|error| error.field == "provenance.corroborating_source_ids"));
}

fn item_record(id: &str, native_row_id: Option<&str>) -> KnowledgeRecord {
    KnowledgeRecord::Item(ItemRecord {
        id: id.to_string(),
        names: names("Test"),
        description: None,
        rarity: "common".to_string(),
        acquisition_leads: vec![],
        native_row_id: native_row_id.map(str::to_string),
        local_evidence: None,
        provenance: provenance(),
    })
}

fn provenance() -> Provenance {
    Provenance {
        source_id: SOURCE_ID.to_string(),
        applicable_game_version: "1.0.3".to_string(),
        retrieved_on: "2026-08-31".to_string(),
        reviewer: "Codex".to_string(),
        review_status: ReviewStatus::Reviewed,
        confidence: Confidence::ReviewedSecondary,
        change_risk: None,
        corroborating_source_ids: Vec::new(),
    }
}

fn names(english: &str) -> LocaleNames {
    LocaleNames {
        en: english.to_string(),
        zh_hans: None,
    }
}

fn valid_records() -> Vec<KnowledgeRecord> {
    vec![
        KnowledgeRecord::Source(source()),
        KnowledgeRecord::Item(ItemRecord {
            id: "ITEM_WOOD".to_string(),
            names: names("Wood"),
            description: None,
            rarity: "Common".to_string(),
            acquisition_leads: vec![AcquisitionLead {
                action: "Chop trees".to_string(),
                notes: None,
            }],
            native_row_id: None,
            local_evidence: None,
            provenance: provenance(),
        }),
        KnowledgeRecord::Item(ItemRecord {
            id: "ITEM_WOODEN_CLUB".to_string(),
            names: names("Wooden Club"),
            description: None,
            rarity: "Common".to_string(),
            acquisition_leads: vec![],
            native_row_id: None,
            local_evidence: None,
            provenance: provenance(),
        }),
        KnowledgeRecord::Technology(TechnologyRecord {
            id: "TECH_LEVEL_1".to_string(),
            names: names("Technology Level 1"),
            level: 1,
            native_row_id: None,
            local_evidence: None,
            provenance: provenance(),
        }),
        KnowledgeRecord::Recipe(RecipeRecord {
            id: "RECIPE_WOODEN_CLUB".to_string(),
            output: RecipeItem {
                item_id: "ITEM_WOODEN_CLUB".to_string(),
                quantity: 1,
            },
            ingredients: vec![RecipeIngredient {
                item_id: "ITEM_WOOD".to_string(),
                quantity: 5,
            }],
            crafting_stations: vec!["Primitive Workbench".to_string()],
            technology_id: Some("TECH_LEVEL_1".to_string()),
            crafting_seconds: None,
            byproducts: Vec::new(),
            native_row_id: None,
            local_evidence: None,
            provenance: provenance(),
        }),
        KnowledgeRecord::ProgressionRelationship(ProgressionRelationshipRecord {
            id: "REL_TECH1_WOODEN_CLUB".to_string(),
            from_id: "TECH_LEVEL_1".to_string(),
            to_id: "RECIPE_WOODEN_CLUB".to_string(),
            relation: ProgressionRelationKind::Unlocks,
            requirement: None,
            provenance: provenance(),
        }),
        KnowledgeRecord::Alias(AliasRecord {
            id: "ALIAS_WOODEN_CLUB_BAT".to_string(),
            alias: "wooden bat".to_string(),
            target_id: "ITEM_WOODEN_CLUB".to_string(),
            locale: "en".to_string(),
            provenance: provenance(),
        }),
    ]
}

#[test]
fn loads_a_valid_minimal_knowledge_store() {
    let store = KnowledgeStore::from_records(valid_records()).expect("valid records must load");

    assert_eq!(
        store.item("ITEM_WOOD").expect("wood exists").rarity,
        "Common"
    );
    assert_eq!(
        store
            .recipe("RECIPE_WOODEN_CLUB")
            .expect("recipe exists")
            .ingredients[0]
            .quantity,
        5
    );
    assert!(store.source(SOURCE_ID).is_some());
}

#[test]
fn rejects_unregistered_or_inconsistent_provenance() {
    let mut records = valid_records();
    let item = match records.last_mut() {
        Some(KnowledgeRecord::Alias(record)) => record,
        _ => panic!("last record must be alias"),
    };
    item.provenance.source_id = "SRC_MISSING".to_string();

    let error = KnowledgeStore::from_records(records)
        .expect_err("facts must reference registered sources")
        .pop()
        .expect("validation error");

    assert_eq!(error.record_id.as_deref(), Some("ALIAS_WOODEN_CLUB_BAT"));
    assert_eq!(error.field, "provenance.source_id");
}

#[test]
fn rejects_duplicate_ids_and_invalid_entity_values() {
    let mut records = valid_records();
    let wood = match records.first_mut() {
        Some(KnowledgeRecord::Source(_)) => records.get_mut(1),
        _ => None,
    };
    if let Some(KnowledgeRecord::Item(record)) = wood {
        record.names.en = " ".to_string();
    }
    records.push(KnowledgeRecord::Item(ItemRecord {
        id: "ITEM_WOODEN_CLUB".to_string(),
        names: names("Duplicate"),
        description: None,
        rarity: String::new(),
        acquisition_leads: vec![],
        native_row_id: None,
        local_evidence: None,
        provenance: provenance(),
    }));

    let errors = KnowledgeStore::from_records(records).expect_err("invalid records must fail");

    assert!(errors.iter().any(|error| error.field == "names.en"));
    assert!(errors.iter().any(|error| error.field == "rarity"));
    assert!(errors.iter().any(|error| error.field == "id"));
}

#[test]
fn rejects_invalid_ranges_and_broken_references() {
    let mut records = valid_records();
    records.push(KnowledgeRecord::Pal(PalRecord {
        id: "PAL_LAMBALL".to_string(),
        names: names("Lamball"),
        stats: None,
        work_suitability: vec![WorkSuitability {
            kind: WorkKind::Handiwork,
            level: 0,
        }],
        drops: vec![DropSource {
            item_id: "ITEM_MISSING".to_string(),
            min_quantity: 3,
            max_quantity: 1,
            probability_percent: 101.0,
        }],
        habitat_ids: vec![],
        element_type1: None,
        element_type2: None,
        native_row_id: None,
        local_evidence: None,
        provenance: provenance(),
    }));

    let errors = KnowledgeStore::from_records(records).expect_err("invalid Pal must fail");

    assert!(errors
        .iter()
        .any(|error| error.field == "work_suitability.level"));
    assert!(errors
        .iter()
        .any(|error| error.field == "drops.min_quantity"));
    assert!(errors
        .iter()
        .any(|error| error.field == "drops.probability_percent"));
    assert!(errors.iter().any(|error| error.field == "drops.item_id"));
}

#[test]
fn validates_all_related_fact_schemas_and_keeps_conflicts_visible() {
    let mut records = valid_records();
    records.push(KnowledgeRecord::Habitat(HabitatRecord {
        id: "HAB_GRASSLANDS".to_string(),
        names: names("Grasslands"),
        pal_ids: vec!["PAL_LAMBALL".to_string()],
        provenance: provenance(),
    }));
    records.push(KnowledgeRecord::BreedingRule(BreedingRuleRecord {
        id: "BREED_LAMBALL_PAIR".to_string(),
        parent_a_id: "PAL_LAMBALL".to_string(),
        parent_b_id: "PAL_MISSING".to_string(),
        child_id: "PAL_LAMBALL".to_string(),
        notes: None,
        provenance: provenance(),
    }));
    records.push(KnowledgeRecord::Conflict(ConflictRecord {
        id: "CONFLICT_WOODEN_CLUB_WOOD".to_string(),
        subject_id: "RECIPE_WOODEN_CLUB".to_string(),
        field: "ingredients[ITEM_WOOD].quantity".to_string(),
        values: vec!["5".to_string(), "6".to_string()],
        source_ids: vec![SOURCE_ID.to_string()],
        resolution: ConflictResolution::Unresolved,
        provenance: provenance(),
    }));

    let errors =
        KnowledgeStore::from_records(records.clone()).expect_err("broken references must fail");
    assert!(errors.iter().any(|error| error.field == "habitat.pal_ids"));
    assert!(errors
        .iter()
        .any(|error| error.field == "breeding.parent_b_id"));

    records.retain(|record| !matches!(record, KnowledgeRecord::Habitat(_)));
    for record in &mut records {
        let KnowledgeRecord::BreedingRule(record) = record else {
            continue;
        };
        record.parent_b_id = "PAL_LAMBALL".to_string();
    }
    records.push(KnowledgeRecord::Pal(PalRecord {
        id: "PAL_LAMBALL".to_string(),
        names: names("Lamball"),
        stats: None,
        work_suitability: vec![],
        drops: vec![],
        habitat_ids: vec![],
        element_type1: None,
        element_type2: None,
        native_row_id: None,
        local_evidence: None,
        provenance: provenance(),
    }));

    let store = KnowledgeStore::from_records(records).expect("valid extended records");
    assert_eq!(store.conflicts().len(), 1);
    assert_eq!(
        store.conflicts()[0].resolution,
        ConflictResolution::Unresolved
    );
}

#[test]
fn rejects_recipe_reference_and_shape_violations() {
    let mut records = valid_records();
    let recipe = records
        .iter_mut()
        .find_map(|record| match record {
            KnowledgeRecord::Recipe(record) => Some(record),
            _ => None,
        })
        .expect("valid records contain recipe");
    recipe.output.item_id = "TECH_LEVEL_1".to_string();
    recipe.ingredients[0].item_id = "ITEM_MISSING".to_string();
    recipe.byproducts = vec![RecipeItem {
        item_id: "ITEM_MISSING".to_string(),
        quantity: 0,
    }];

    let errors = KnowledgeStore::from_records(records)
        .expect_err("wrong reference types and missing ingredients must fail");
    assert!(errors.iter().any(|error| error.field == "output.item_id"));
    assert!(errors
        .iter()
        .any(|error| error.field == "ingredients.item_id"));
    assert!(errors
        .iter()
        .any(|error| error.field == "byproducts.item_id"));
    assert!(errors
        .iter()
        .any(|error| error.field == "byproducts.quantity"));

    let mut records = valid_records();
    let recipe = records
        .iter_mut()
        .find_map(|record| match record {
            KnowledgeRecord::Recipe(record) => Some(record),
            _ => None,
        })
        .expect("valid records contain recipe");
    recipe.ingredients.clear();
    recipe.crafting_stations.clear();
    recipe.crafting_seconds = Some(-1.0);

    let errors = KnowledgeStore::from_records(records)
        .expect_err("incomplete and invalid recipe values must fail");
    assert!(errors.iter().any(|error| error.field == "ingredients"));
    assert!(errors
        .iter()
        .any(|error| error.field == "crafting_stations"));
    assert!(errors.iter().any(|error| error.field == "crafting_seconds"));
}

#[test]
fn rejects_impossible_dates_and_provenance_drift() {
    let mut records = valid_records();
    if let Some(KnowledgeRecord::Source(record)) = records.first_mut() {
        record.retrieved_on = "9999-99-99".to_string();
    }
    if let Some(KnowledgeRecord::Alias(record)) = records.last_mut() {
        record.provenance.retrieved_on = "9999-99-99".to_string();
    }

    let errors =
        KnowledgeStore::from_records(records).expect_err("impossible calendar dates must fail");
    assert!(errors.iter().any(|error| error.field == "retrieved_on"));
    assert!(errors
        .iter()
        .any(|error| error.field == "provenance.retrieved_on"));

    let mut records = valid_records();
    if let Some(KnowledgeRecord::Alias(record)) = records.last_mut() {
        record.provenance.applicable_game_version = "1.0.4".to_string();
    }

    let errors = KnowledgeStore::from_records(records)
        .expect_err("fact provenance cannot drift from its source");
    assert!(errors.iter().any(|error| error.field == "provenance"));
}

#[test]
fn rejects_malformed_versions_ids_conflict_values_and_alias_locales() {
    let mut records = valid_records();
    if let Some(KnowledgeRecord::Source(record)) = records.first_mut() {
        record.applicable_game_version = "nonsense-version".to_string();
    }
    if let Some(KnowledgeRecord::Alias(record)) = records.last_mut() {
        record.provenance.applicable_game_version = "nonsense-version".to_string();
    }
    let errors = KnowledgeStore::from_records(records)
        .expect_err("versions must be dot-separated numeric segments");
    assert!(errors
        .iter()
        .any(|error| error.field == "applicable_game_version"));
    assert!(errors
        .iter()
        .any(|error| error.field == "provenance.applicable_game_version"));

    let mut records = valid_records();
    let item = records
        .iter_mut()
        .find_map(|record| match record {
            KnowledgeRecord::Item(record) => Some(record),
            _ => None,
        })
        .expect("valid records contain an item");
    item.id = "-".to_string();
    let errors = KnowledgeStore::from_records(records)
        .expect_err("identifiers need at least one letter or digit");
    assert!(errors.iter().any(|error| error.field == "item.id"));

    let mut records = valid_records();
    records.push(KnowledgeRecord::Conflict(ConflictRecord {
        id: "CONFLICT_EMPTY".to_string(),
        subject_id: "ITEM_WOOD".to_string(),
        field: "description".to_string(),
        values: vec![],
        source_ids: vec![SOURCE_ID.to_string()],
        resolution: ConflictResolution::Unresolved,
        provenance: provenance(),
    }));
    let errors =
        KnowledgeStore::from_records(records).expect_err("conflict with empty values must fail");
    assert!(errors.iter().any(|error| error.field == "values"));

    let mut records = valid_records();
    records.push(KnowledgeRecord::Conflict(ConflictRecord {
        id: "CONFLICT_BLANK".to_string(),
        subject_id: "ITEM_WOOD".to_string(),
        field: "description".to_string(),
        values: vec!["one".to_string(), "  ".to_string()],
        source_ids: vec![],
        resolution: ConflictResolution::Unresolved,
        provenance: provenance(),
    }));
    let errors = KnowledgeStore::from_records(records)
        .expect_err("conflict with blank values or no sources must fail");
    assert!(errors.iter().any(|error| error.field == "values"));
    assert!(errors.iter().any(|error| error.field == "source_ids"));

    let mut records = valid_records();
    let alias = records
        .iter_mut()
        .find_map(|record| match record {
            KnowledgeRecord::Alias(record) => Some(record),
            _ => None,
        })
        .expect("valid records contain an alias");
    alias.locale = "xx".to_string();
    let errors =
        KnowledgeStore::from_records(records).expect_err("unsupported alias locale must fail");
    assert!(errors.iter().any(|error| error.field == "locale"));
}
