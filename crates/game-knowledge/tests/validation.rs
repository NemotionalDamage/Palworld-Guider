use game_knowledge::{
    AcquisitionLead, AliasRecord, BreedingRuleRecord, Confidence, ConflictRecord,
    ConflictResolution, DropSource, HabitatRecord, ItemRecord, KnowledgeRecord, KnowledgeStore,
    LocaleNames, PalRecord, ProgressionRelationKind, ProgressionRelationshipRecord, Provenance,
    RecipeIngredient, RecipeItem, RecipeRecord, ReviewStatus, SourceRecord, TechnologyRecord,
    WorkKind, WorkSuitability,
};

const SOURCE_ID: &str = "SRC-PALDB-20260831";

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

fn provenance() -> Provenance {
    Provenance {
        source_id: SOURCE_ID.to_string(),
        applicable_game_version: "1.0.3".to_string(),
        retrieved_on: "2026-08-31".to_string(),
        reviewer: "Codex".to_string(),
        review_status: ReviewStatus::Reviewed,
        confidence: Confidence::ReviewedSecondary,
        change_risk: None,
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
            provenance: provenance(),
        }),
        KnowledgeRecord::Item(ItemRecord {
            id: "ITEM_WOODEN_CLUB".to_string(),
            names: names("Wooden Club"),
            description: None,
            rarity: "Common".to_string(),
            acquisition_leads: vec![],
            provenance: provenance(),
        }),
        KnowledgeRecord::Technology(TechnologyRecord {
            id: "TECH_LEVEL_1".to_string(),
            names: names("Technology Level 1"),
            level: 1,
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
        provenance: provenance(),
    }));

    let store = KnowledgeStore::from_records(records).expect("valid extended records");
    assert_eq!(store.conflicts().len(), 1);
    assert_eq!(
        store.conflicts()[0].resolution,
        ConflictResolution::Unresolved
    );
}
