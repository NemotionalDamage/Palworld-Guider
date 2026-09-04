use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use crate::models::{Confidence, KnowledgeRecord, ReviewStatus, SourceRecord};
use crate::ValidationError;

const FACT_FILE_NAMES: [&str; 11] = [
    "facts.jsonl",
    "items.jsonl",
    "pals.jsonl",
    "technologies.jsonl",
    "recipes.jsonl",
    "habitats.jsonl",
    "breeding_rules.jsonl",
    "aliases.jsonl",
    "progression_relationships.jsonl",
    "conflicts.jsonl",
    "type_effectiveness.jsonl",
];

#[derive(Debug, Clone, Default, PartialEq)]
pub struct KnowledgeStore {
    sources: BTreeMap<String, SourceRecord>,
    items: BTreeMap<String, crate::models::ItemRecord>,
    pals: BTreeMap<String, crate::models::PalRecord>,
    technologies: BTreeMap<String, crate::models::TechnologyRecord>,
    recipes: BTreeMap<String, crate::models::RecipeRecord>,
    habitats: BTreeMap<String, crate::models::HabitatRecord>,
    breeding_rules: BTreeMap<String, crate::models::BreedingRuleRecord>,
    aliases: BTreeMap<String, crate::models::AliasRecord>,
    progression_relationships: BTreeMap<String, crate::models::ProgressionRelationshipRecord>,
    conflicts: BTreeMap<String, crate::models::ConflictRecord>,
    type_effectiveness: BTreeMap<String, crate::models::TypeEffectivenessRecord>,
}

impl KnowledgeStore {
    pub fn from_records(records: Vec<KnowledgeRecord>) -> Result<Self, Vec<ValidationError>> {
        let mut store = Self::default();
        let mut errors = Vec::new();
        let mut fact_ids: BTreeSet<String> = BTreeSet::new();
        let mut alias_keys: BTreeSet<(String, String)> = BTreeSet::new();
        let mut native_item_ids: BTreeMap<String, String> = BTreeMap::new();
        let mut native_pal_ids: BTreeMap<String, String> = BTreeMap::new();
        let mut native_technology_ids: BTreeMap<String, String> = BTreeMap::new();
        let mut native_recipe_ids: BTreeMap<String, String> = BTreeMap::new();

        for record in records {
            match record {
                KnowledgeRecord::Source(record) => {
                    validate_id(&record.id, "source.id", &record.id, &mut errors);
                    if let Some(error) = validate_source(&record) {
                        errors.push(error);
                    }
                    let source_id = record.id.clone();
                    if store.sources.insert(source_id.clone(), record).is_some() {
                        errors.push(ValidationError::new(
                            Some(source_id),
                            "id",
                            "source ID is duplicate",
                        ));
                    }
                }
                KnowledgeRecord::Item(record) => {
                    validate_id(&record.id, "item.id", &record.id, &mut errors);
                    validate_native_row_id(
                        &record,
                        &mut native_item_ids,
                        "item.native_row_id",
                        &mut errors,
                    );
                    validate_local_evidence(&record, &mut errors);
                    if let Some(error) = validate_locale_names(&record.id, &record.names) {
                        errors.push(error);
                    }
                    require_nonempty(
                        Some(record.id.clone()),
                        "rarity",
                        &record.rarity,
                        &mut errors,
                    );
                    for lead in &record.acquisition_leads {
                        require_nonempty(
                            Some(record.id.clone()),
                            "acquisition_leads.action",
                            &lead.action,
                            &mut errors,
                        );
                    }
                    push_optional_error(
                        validate_provenance(&record.provenance, &record.id),
                        &mut errors,
                    );
                    insert_fact(
                        store.items.entry(record.id.clone()),
                        record,
                        &mut fact_ids,
                        &mut errors,
                    );
                }
                KnowledgeRecord::Pal(record) => {
                    validate_id(&record.id, "pal.id", &record.id, &mut errors);
                    validate_native_row_id(
                        &record,
                        &mut native_pal_ids,
                        "pal.native_row_id",
                        &mut errors,
                    );
                    validate_local_evidence(&record, &mut errors);
                    if let Some(error) = validate_locale_names(&record.id, &record.names) {
                        errors.push(error);
                    }
                    for work in &record.work_suitability {
                        if work.level < 1 || work.level > 8 {
                            errors.push(ValidationError::new(
                                Some(record.id.clone()),
                                "work_suitability.level",
                                "work level must be between 1 and 8",
                            ));
                        }
                    }
                    for drop in &record.drops {
                        if drop.min_quantity == 0 {
                            errors.push(ValidationError::new(
                                Some(record.id.clone()),
                                "drops.min_quantity",
                                "minimum quantity must be greater than zero",
                            ));
                        }
                        if drop.max_quantity < drop.min_quantity {
                            errors.push(ValidationError::new(
                                Some(record.id.clone()),
                                "drops.min_quantity",
                                "minimum quantity cannot exceed maximum quantity",
                            ));
                        }
                        if !(0.0..=100.0).contains(&drop.probability_percent) {
                            errors.push(ValidationError::new(
                                Some(record.id.clone()),
                                "drops.probability_percent",
                                "probability must be between 0 and 100",
                            ));
                        }
                    }
                    push_optional_error(
                        validate_provenance(&record.provenance, &record.id),
                        &mut errors,
                    );
                    insert_fact(
                        store.pals.entry(record.id.clone()),
                        record,
                        &mut fact_ids,
                        &mut errors,
                    );
                }
                KnowledgeRecord::Technology(record) => {
                    validate_id(&record.id, "technology.id", &record.id, &mut errors);
                    validate_native_row_id(
                        &record,
                        &mut native_technology_ids,
                        "technology.native_row_id",
                        &mut errors,
                    );
                    validate_local_evidence(&record, &mut errors);
                    if let Some(error) = validate_locale_names(&record.id, &record.names) {
                        errors.push(error);
                    }
                    if record.level == 0 {
                        errors.push(ValidationError::new(
                            Some(record.id.clone()),
                            "level",
                            "technology level must be greater than zero",
                        ));
                    }
                    push_optional_error(
                        validate_provenance(&record.provenance, &record.id),
                        &mut errors,
                    );
                    insert_fact(
                        store.technologies.entry(record.id.clone()),
                        record,
                        &mut fact_ids,
                        &mut errors,
                    );
                }
                KnowledgeRecord::Recipe(record) => {
                    validate_id(&record.id, "recipe.id", &record.id, &mut errors);
                    validate_native_row_id(
                        &record,
                        &mut native_recipe_ids,
                        "recipe.native_row_id",
                        &mut errors,
                    );
                    validate_local_evidence(&record, &mut errors);
                    if record.ingredients.is_empty() {
                        errors.push(ValidationError::new(
                            Some(record.id.clone()),
                            "ingredients",
                            "at least one ingredient is required",
                        ));
                    }
                    if record.crafting_stations.is_empty() {
                        errors.push(ValidationError::new(
                            Some(record.id.clone()),
                            "crafting_stations",
                            "at least one crafting station is required",
                        ));
                    }
                    if let Some(seconds) = record.crafting_seconds {
                        if !seconds.is_finite() || seconds <= 0.0 {
                            errors.push(ValidationError::new(
                                Some(record.id.clone()),
                                "crafting_seconds",
                                "crafting seconds must be a finite positive number",
                            ));
                        }
                    }
                    for byproduct in &record.byproducts {
                        if byproduct.quantity == 0 {
                            errors.push(ValidationError::new(
                                Some(record.id.clone()),
                                "byproducts.quantity",
                                "byproduct quantity must be greater than zero",
                            ));
                        }
                    }
                    if record.output.quantity == 0 {
                        errors.push(ValidationError::new(
                            Some(record.id.clone()),
                            "output.quantity",
                            "output quantity must be greater than zero",
                        ));
                    }
                    for ingredient in &record.ingredients {
                        if ingredient.quantity == 0 {
                            errors.push(ValidationError::new(
                                Some(record.id.clone()),
                                "ingredients.quantity",
                                "ingredient quantity must be greater than zero",
                            ));
                        }
                    }
                    for station in &record.crafting_stations {
                        require_nonempty(
                            Some(record.id.clone()),
                            "crafting_stations",
                            station,
                            &mut errors,
                        );
                    }
                    push_optional_error(
                        validate_provenance(&record.provenance, &record.id),
                        &mut errors,
                    );
                    insert_fact(
                        store.recipes.entry(record.id.clone()),
                        record,
                        &mut fact_ids,
                        &mut errors,
                    );
                }
                KnowledgeRecord::Habitat(record) => {
                    validate_id(&record.id, "habitat.id", &record.id, &mut errors);
                    if let Some(error) = validate_locale_names(&record.id, &record.names) {
                        errors.push(error);
                    }
                    push_optional_error(
                        validate_provenance(&record.provenance, &record.id),
                        &mut errors,
                    );
                    insert_fact(
                        store.habitats.entry(record.id.clone()),
                        record,
                        &mut fact_ids,
                        &mut errors,
                    );
                }
                KnowledgeRecord::BreedingRule(record) => {
                    validate_id(&record.id, "breeding.id", &record.id, &mut errors);
                    push_optional_error(
                        validate_provenance(&record.provenance, &record.id),
                        &mut errors,
                    );
                    insert_fact(
                        store.breeding_rules.entry(record.id.clone()),
                        record,
                        &mut fact_ids,
                        &mut errors,
                    );
                }
                KnowledgeRecord::Alias(record) => {
                    validate_id(&record.id, "alias.id", &record.id, &mut errors);
                    require_nonempty(Some(record.id.clone()), "alias", &record.alias, &mut errors);
                    if record.locale != "en" && record.locale != "zh_hans" {
                        errors.push(ValidationError::new(
                            Some(record.id.clone()),
                            "locale",
                            "alias locale must be en or zh_hans",
                        ));
                    }
                    push_optional_error(
                        validate_provenance(&record.provenance, &record.id),
                        &mut errors,
                    );
                    let key = (record.locale.clone(), record.alias.to_lowercase());
                    let alias_id = record.id.clone();
                    if !alias_keys.insert(key) {
                        errors.push(ValidationError::new(
                            Some(alias_id),
                            "alias",
                            "alias is duplicate for locale",
                        ));
                    }
                    insert_fact(
                        store.aliases.entry(record.id.clone()),
                        record,
                        &mut fact_ids,
                        &mut errors,
                    );
                }
                KnowledgeRecord::ProgressionRelationship(record) => {
                    validate_id(&record.id, "progression.id", &record.id, &mut errors);
                    push_optional_error(
                        validate_provenance(&record.provenance, &record.id),
                        &mut errors,
                    );
                    insert_fact(
                        store.progression_relationships.entry(record.id.clone()),
                        record,
                        &mut fact_ids,
                        &mut errors,
                    );
                }
                KnowledgeRecord::Conflict(record) => {
                    validate_id(&record.id, "conflict.id", &record.id, &mut errors);
                    require_nonempty(Some(record.id.clone()), "field", &record.field, &mut errors);
                    if record.values.len() < 2
                        || record.values.iter().any(|value| value.trim().is_empty())
                    {
                        errors.push(ValidationError::new(
                            Some(record.id.clone()),
                            "values",
                            "conflict requires at least two nonempty values",
                        ));
                    }
                    if record.source_ids.is_empty() {
                        errors.push(ValidationError::new(
                            Some(record.id.clone()),
                            "source_ids",
                            "conflict requires at least one source",
                        ));
                    }
                    push_optional_error(
                        validate_provenance(&record.provenance, &record.id),
                        &mut errors,
                    );
                    insert_fact(
                        store.conflicts.entry(record.id.clone()),
                        record,
                        &mut fact_ids,
                        &mut errors,
                    );
                }
                KnowledgeRecord::TypeEffectiveness(record) => {
                    validate_id(&record.id, "type_effectiveness.id", &record.id, &mut errors);
                    if record.multiplier <= 0.0 || !record.multiplier.is_finite() {
                        errors.push(ValidationError::new(
                            Some(record.id.clone()),
                            "multiplier",
                            "multiplier must be a finite positive number",
                        ));
                    }
                    push_optional_error(
                        validate_provenance(&record.provenance, &record.id),
                        &mut errors,
                    );
                    insert_fact(
                        store.type_effectiveness.entry(record.id.clone()),
                        record,
                        &mut fact_ids,
                        &mut errors,
                    );
                }
            }
        }

        store.validate_references(&fact_ids, &mut errors);

        if errors.is_empty() {
            Ok(store)
        } else {
            Err(errors)
        }
    }

    pub fn load_directory(path: impl AsRef<Path>) -> Result<Self, Vec<ValidationError>> {
        let path = path.as_ref();
        let mut records = read_jsonl(&path.join("sources.jsonl"))?;
        for file_name in FACT_FILE_NAMES {
            let file_path = path.join(file_name);
            if file_path.exists() {
                records.extend(read_jsonl(&file_path)?);
            }
        }
        Self::from_records(records)
    }

    pub fn source(&self, id: &str) -> Option<&SourceRecord> {
        self.sources.get(id)
    }

    pub fn sources(&self) -> impl Iterator<Item = &SourceRecord> {
        self.sources.values()
    }

    pub fn item(&self, id: &str) -> Option<&crate::models::ItemRecord> {
        self.items.get(id)
    }

    pub fn pal(&self, id: &str) -> Option<&crate::models::PalRecord> {
        self.pals.get(id)
    }

    pub fn technology(&self, id: &str) -> Option<&crate::models::TechnologyRecord> {
        self.technologies.get(id)
    }

    pub fn habitat(&self, id: &str) -> Option<&crate::models::HabitatRecord> {
        self.habitats.get(id)
    }

    pub fn recipe(&self, id: &str) -> Option<&crate::models::RecipeRecord> {
        self.recipes.get(id)
    }

    pub fn conflicts(&self) -> Vec<&crate::models::ConflictRecord> {
        self.conflicts.values().collect()
    }

    pub fn items(&self) -> impl Iterator<Item = &crate::models::ItemRecord> {
        self.items.values()
    }

    pub fn pals(&self) -> impl Iterator<Item = &crate::models::PalRecord> {
        self.pals.values()
    }

    pub fn technologies(&self) -> impl Iterator<Item = &crate::models::TechnologyRecord> {
        self.technologies.values()
    }

    pub fn habitats(&self) -> impl Iterator<Item = &crate::models::HabitatRecord> {
        self.habitats.values()
    }

    pub fn recipes(&self) -> impl Iterator<Item = &crate::models::RecipeRecord> {
        self.recipes.values()
    }

    pub fn aliases(&self) -> impl Iterator<Item = &crate::models::AliasRecord> {
        self.aliases.values()
    }

    pub fn breeding_rules(&self) -> impl Iterator<Item = &crate::models::BreedingRuleRecord> {
        self.breeding_rules.values()
    }

    pub fn progression_relationships(
        &self,
    ) -> impl Iterator<Item = &crate::models::ProgressionRelationshipRecord> {
        self.progression_relationships.values()
    }

    pub fn type_effectiveness(
        &self,
    ) -> impl Iterator<Item = &crate::models::TypeEffectivenessRecord> {
        self.type_effectiveness.values()
    }

    pub fn conflicts_for_subject(&self, subject_id: &str) -> Vec<&crate::models::ConflictRecord> {
        self.conflicts
            .values()
            .filter(|record| record.subject_id == subject_id)
            .collect()
    }

    fn provenances(&self) -> Vec<(&String, &crate::models::Provenance)> {
        let mut result = Vec::new();
        for (id, record) in &self.items {
            result.push((id, &record.provenance));
        }
        for (id, record) in &self.pals {
            result.push((id, &record.provenance));
        }
        for (id, record) in &self.technologies {
            result.push((id, &record.provenance));
        }
        for (id, record) in &self.recipes {
            result.push((id, &record.provenance));
        }
        for (id, record) in &self.habitats {
            result.push((id, &record.provenance));
        }
        for (id, record) in &self.breeding_rules {
            result.push((id, &record.provenance));
        }
        for (id, record) in &self.aliases {
            result.push((id, &record.provenance));
        }
        for (id, record) in &self.progression_relationships {
            result.push((id, &record.provenance));
        }
        for (id, record) in &self.conflicts {
            result.push((id, &record.provenance));
        }
        result
    }

    fn validate_references(&self, fact_ids: &BTreeSet<String>, errors: &mut Vec<ValidationError>) {
        for record in self.sources.values() {
            for url in &record.evidence_urls {
                if !(url.starts_with("https://")
                    || url.starts_with("http://")
                    || url.starts_with("local://"))
                {
                    errors.push(ValidationError::new(
                        Some(record.id.clone()),
                        "evidence_urls",
                        "source URL must use HTTP or HTTPS",
                    ));
                }
            }
        }

        for record in self.recipes.values() {
            require_reference(
                &record.id,
                "output.item_id",
                &record.output.item_id,
                self.items.keys(),
                errors,
            );
            for ingredient in &record.ingredients {
                require_reference(
                    &record.id,
                    "ingredients.item_id",
                    &ingredient.item_id,
                    self.items.keys(),
                    errors,
                );
            }
            if let Some(technology_id) = &record.technology_id {
                require_reference(
                    &record.id,
                    "technology_id",
                    technology_id,
                    self.technologies.keys(),
                    errors,
                );
            }
            for byproduct in &record.byproducts {
                require_reference(
                    &record.id,
                    "byproducts.item_id",
                    &byproduct.item_id,
                    self.items.keys(),
                    errors,
                );
            }
        }

        for record in self.pals.values() {
            for drop in &record.drops {
                require_reference(
                    &record.id,
                    "drops.item_id",
                    &drop.item_id,
                    self.items.keys(),
                    errors,
                );
            }
            for habitat_id in &record.habitat_ids {
                require_reference(
                    &record.id,
                    "habitat_ids",
                    habitat_id,
                    self.habitats.keys(),
                    errors,
                );
            }
        }

        for record in self.habitats.values() {
            for pal_id in &record.pal_ids {
                require_reference(
                    &record.id,
                    "habitat.pal_ids",
                    pal_id,
                    self.pals.keys(),
                    errors,
                );
            }
        }

        for record in self.breeding_rules.values() {
            require_reference(
                &record.id,
                "breeding.parent_a_id",
                &record.parent_a_id,
                self.pals.keys(),
                errors,
            );
            require_reference(
                &record.id,
                "breeding.parent_b_id",
                &record.parent_b_id,
                self.pals.keys(),
                errors,
            );
            require_reference(
                &record.id,
                "breeding.child_id",
                &record.child_id,
                self.pals.keys(),
                errors,
            );
        }

        for record in self.aliases.values() {
            require_fact_reference(&record.id, "target_id", &record.target_id, fact_ids, errors);
        }

        for record in self.progression_relationships.values() {
            require_fact_reference(&record.id, "from_id", &record.from_id, fact_ids, errors);
            require_fact_reference(&record.id, "to_id", &record.to_id, fact_ids, errors);
        }

        for record in self.conflicts.values() {
            require_fact_reference(
                &record.id,
                "subject_id",
                &record.subject_id,
                fact_ids,
                errors,
            );
            for source_id in &record.source_ids {
                if !self.sources.contains_key(source_id) {
                    errors.push(ValidationError::new(
                        Some(record.id.clone()),
                        "source_ids",
                        "conflict references an unregistered source",
                    ));
                }
            }
        }

        for (record_id, provenance) in self.provenances() {
            let Some(source) = self.sources.get(&provenance.source_id) else {
                errors.push(ValidationError::new(
                    Some(record_id.clone()),
                    "provenance.source_id",
                    "fact references an unregistered source",
                ));
                continue;
            };
            if source.applicable_game_version != provenance.applicable_game_version
                || source.retrieved_on != provenance.retrieved_on
                || source.reviewer != provenance.reviewer
                || source.review_status != provenance.review_status
                || source.confidence != provenance.confidence
            {
                errors.push(ValidationError::new(
                    Some(record_id.clone()),
                    "provenance",
                    "fact provenance does not match its registered source",
                ));
            }
            for source_id in &provenance.corroborating_source_ids {
                if !self.sources.contains_key(source_id) {
                    errors.push(ValidationError::new(
                        Some(record_id.clone()),
                        "provenance.corroborating_source_ids",
                        "corroborating source is not registered",
                    ));
                }
            }
        }
    }
}

fn validate_native_row_id<T>(
    record: &T,
    native_ids: &mut BTreeMap<String, String>,
    field: &'static str,
    errors: &mut Vec<ValidationError>,
) where
    T: NativeRowRecord,
{
    let record_id = record.record_id().to_string();
    if let Some(native_row_id) = record.native_row_id() {
        if native_row_id.trim().is_empty() {
            errors.push(ValidationError::new(
                Some(record_id),
                field,
                "native row ID must not be empty",
            ));
        } else if let Some(existing_id) =
            native_ids.insert(native_row_id.to_string(), record_id.clone())
        {
            errors.push(ValidationError::new(
                Some(record_id),
                field,
                format!("duplicate native row ID also used by {existing_id}"),
            ));
        }
    }
}

fn validate_local_evidence<T>(record: &T, errors: &mut Vec<ValidationError>)
where
    T: NativeRowRecord,
{
    if let Some(evidence) = record.local_evidence() {
        if evidence.source_table.trim().is_empty() {
            errors.push(ValidationError::new(
                Some(record.record_id().to_string()),
                "local_evidence.source_table",
                "source table must not be empty",
            ));
        }
        if evidence.transformation_notes.trim().is_empty() {
            errors.push(ValidationError::new(
                Some(record.record_id().to_string()),
                "local_evidence.transformation_notes",
                "transformation notes are required",
            ));
        }
    }
}

trait NativeRowRecord {
    fn record_id(&self) -> &str;
    fn native_row_id(&self) -> Option<&str>;
    fn local_evidence(&self) -> Option<&crate::models::LocalEvidenceMetadata>;
}

macro_rules! impl_native_row_record {
    ($record_type:ty, $id_field:ident) => {
        impl NativeRowRecord for $record_type {
            fn record_id(&self) -> &str {
                &self.$id_field
            }

            fn native_row_id(&self) -> Option<&str> {
                self.native_row_id.as_deref()
            }

            fn local_evidence(&self) -> Option<&crate::models::LocalEvidenceMetadata> {
                self.local_evidence.as_ref()
            }
        }
    };
}

impl_native_row_record!(crate::models::ItemRecord, id);
impl_native_row_record!(crate::models::PalRecord, id);
impl_native_row_record!(crate::models::TechnologyRecord, id);
impl_native_row_record!(crate::models::RecipeRecord, id);

fn read_jsonl(path: &Path) -> Result<Vec<KnowledgeRecord>, Vec<ValidationError>> {
    let content = fs::read_to_string(path).map_err(|error| {
        vec![ValidationError::new(
            None,
            "jsonl",
            format!("cannot read {}: {}", path.display(), error),
        )]
    })?;
    let mut records = Vec::new();
    let mut errors = Vec::new();

    for (index, line) in content.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str(line) {
            Ok(record) => records.push(record),
            Err(error) => errors.push(ValidationError::new(
                None,
                "jsonl",
                format!("{} line {}: {}", path.display(), index + 1, error),
            )),
        }
    }

    if errors.is_empty() {
        Ok(records)
    } else {
        Err(errors)
    }
}

fn insert_fact<T>(
    entry: std::collections::btree_map::Entry<String, T>,
    record: T,
    fact_ids: &mut BTreeSet<String>,
    errors: &mut Vec<ValidationError>,
) {
    let id = entry.key().clone();
    if !fact_ids.insert(id.clone()) {
        errors.push(ValidationError::new(
            Some(id),
            "id",
            "fact ID is duplicate across record types",
        ));
    }
    match entry {
        std::collections::btree_map::Entry::Occupied(_) => drop(record),
        std::collections::btree_map::Entry::Vacant(entry) => {
            entry.insert(record);
        }
    }
}

fn validate_id(
    value: &str,
    field: &'static str,
    record_id: &str,
    errors: &mut Vec<ValidationError>,
) {
    if value.is_empty()
        || !value.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '_' || character == '-'
        })
        || !value
            .chars()
            .any(|character| character.is_ascii_alphanumeric())
    {
        errors.push(ValidationError::new(
            Some(record_id.to_string()),
            field,
            "identifier must contain at least one ASCII letter or digit and only ASCII letters, digits, hyphen, or underscore",
        ));
    }
}

fn validate_source(record: &SourceRecord) -> Option<ValidationError> {
    let fields = [
        ("title", record.title.as_str()),
        ("supplier", record.supplier.as_str()),
        ("reviewer", record.reviewer.as_str()),
        (
            "applicable_game_version",
            record.applicable_game_version.as_str(),
        ),
    ];
    for (field, value) in fields {
        if value.trim().is_empty() {
            return Some(ValidationError::new(
                Some(record.id.clone()),
                field,
                "source field must not be empty",
            ));
        }
    }
    if !is_valid_game_version(&record.applicable_game_version) {
        return Some(ValidationError::new(
            Some(record.id.clone()),
            "applicable_game_version",
            "version must be dot-separated numeric segments",
        ));
    }
    if !is_iso_date(&record.retrieved_on) {
        return Some(ValidationError::new(
            Some(record.id.clone()),
            "retrieved_on",
            "date must use YYYY-MM-DD",
        ));
    }
    if record.evidence_urls.is_empty() {
        return Some(ValidationError::new(
            Some(record.id.clone()),
            "evidence_urls",
            "at least one evidence URL is required",
        ));
    }
    None
}

fn validate_provenance(
    provenance: &crate::models::Provenance,
    record_id: &str,
) -> Option<ValidationError> {
    if provenance.source_id.trim().is_empty() {
        return Some(ValidationError::new(
            Some(record_id.to_string()),
            "provenance.source_id",
            "source ID must not be empty",
        ));
    }
    if provenance.reviewer.trim().is_empty() {
        return Some(ValidationError::new(
            Some(record_id.to_string()),
            "provenance",
            "reviewer is required",
        ));
    }
    if !is_valid_game_version(&provenance.applicable_game_version) {
        return Some(ValidationError::new(
            Some(record_id.to_string()),
            "provenance.applicable_game_version",
            "version must be dot-separated numeric segments",
        ));
    }
    if !is_iso_date(&provenance.retrieved_on) {
        return Some(ValidationError::new(
            Some(record_id.to_string()),
            "provenance.retrieved_on",
            "date must use YYYY-MM-DD",
        ));
    }
    if provenance.review_status != ReviewStatus::Reviewed {
        return Some(ValidationError::new(
            Some(record_id.to_string()),
            "provenance.review_status",
            "persisted facts must be reviewed",
        ));
    }
    if matches!(
        provenance.confidence,
        Confidence::Community | Confidence::Conflicted | Confidence::Unknown
    ) {
        return Some(ValidationError::new(
            Some(record_id.to_string()),
            "provenance.confidence",
            "persisted facts cannot use community, conflicted, or unknown confidence",
        ));
    }
    None
}

fn validate_locale_names(
    record_id: &str,
    names: &crate::models::LocaleNames,
) -> Option<ValidationError> {
    if names.en.trim().is_empty() {
        return Some(ValidationError::new(
            Some(record_id.to_string()),
            "names.en",
            "English name is required",
        ));
    }
    if names
        .zh_hans
        .as_ref()
        .is_some_and(|name| name.trim().is_empty())
    {
        return Some(ValidationError::new(
            Some(record_id.to_string()),
            "names.zh_hans",
            "Chinese name cannot be blank when present",
        ));
    }
    None
}

fn require_nonempty(
    record_id: Option<String>,
    field: &'static str,
    value: &str,
    errors: &mut Vec<ValidationError>,
) {
    if value.trim().is_empty() {
        errors.push(ValidationError::new(
            record_id,
            field,
            "value must not be empty",
        ));
    }
}

fn require_reference<'a>(
    record_id: &str,
    field: &'static str,
    value: &str,
    valid: impl IntoIterator<Item = &'a String>,
    errors: &mut Vec<ValidationError>,
) {
    let direct_match = valid.into_iter().any(|candidate| candidate == value);
    if !direct_match {
        errors.push(ValidationError::new(
            Some(record_id.to_string()),
            field,
            "reference does not resolve to the required record type",
        ));
    }
}

fn require_fact_reference(
    record_id: &str,
    field: &'static str,
    value: &str,
    fact_ids: &BTreeSet<String>,
    errors: &mut Vec<ValidationError>,
) {
    if !fact_ids.contains(value) {
        errors.push(ValidationError::new(
            Some(record_id.to_string()),
            field,
            "reference does not resolve",
        ));
    }
}

fn push_optional_error(error: Option<ValidationError>, errors: &mut Vec<ValidationError>) {
    if let Some(error) = error {
        errors.push(error);
    }
}

fn is_valid_game_version(value: &str) -> bool {
    if value.is_empty() {
        return false;
    }
    let mut segment_count = 0_usize;
    for segment in value.split('.') {
        segment_count += 1;
        if segment.is_empty() || !segment.bytes().all(|byte| byte.is_ascii_digit()) {
            return false;
        }
    }
    segment_count >= 2
}

fn is_iso_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }
    if !bytes
        .iter()
        .enumerate()
        .all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit())
    {
        return false;
    }

    let year = value[0..4].parse::<u16>().expect("four ASCII digits");
    let month = value[5..7].parse::<u8>().expect("two ASCII digits");
    let day = value[8..10].parse::<u8>().expect("two ASCII digits");
    let leap_year = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days_in_month = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if leap_year {
                29
            } else {
                28
            }
        }
        _ => return false,
    };

    year > 0 && (1..=days_in_month).contains(&day)
}
