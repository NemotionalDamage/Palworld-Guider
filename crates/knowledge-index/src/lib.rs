//! Lexical search over reviewed Palworld knowledge.

use game_knowledge::{
    Confidence, HabitatRecord, ItemRecord, KnowledgeStore, PalRecord, PalWazaUnlock, Provenance,
    RecipeRecord, ReviewStatus, TechnologyRecord, TypeEffectivenessRecord, WazaRecord,
    WorkKindDescriptionRecord,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use tantivy::collector::TopDocs;
use tantivy::query::{BooleanQuery, BoostQuery, Occur, Query, QueryParser};
use tantivy::schema::Value as _;
use tantivy::schema::{Field, Schema, TantivyDocument, STORED, TEXT};
use tantivy::{doc, Index, IndexReader};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IndexStatus {
    Ok,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenanceBrief {
    pub source_id: String,
    pub applicable_game_version: String,
    pub retrieved_on: String,
    pub reviewer: String,
    pub review_status: String,
    pub confidence: String,
    pub change_risk: Option<String>,
}

impl ProvenanceBrief {
    fn from_provenance(provenance: &Provenance) -> Self {
        Self {
            source_id: provenance.source_id.clone(),
            applicable_game_version: provenance.applicable_game_version.clone(),
            retrieved_on: provenance.retrieved_on.clone(),
            reviewer: provenance.reviewer.clone(),
            review_status: review_status_value(&provenance.review_status).to_string(),
            confidence: confidence_value(&provenance.confidence).to_string(),
            change_risk: provenance.change_risk.clone(),
        }
    }
}

fn review_status_value(status: &ReviewStatus) -> &'static str {
    match status {
        ReviewStatus::Candidate => "candidate",
        ReviewStatus::Reviewed => "reviewed",
        ReviewStatus::Retired => "retired",
    }
}

fn confidence_value(confidence: &Confidence) -> &'static str {
    match confidence {
        Confidence::VerifiedTarget => "verified_target",
        Confidence::Official => "official",
        Confidence::ReviewedSecondary => "reviewed_secondary",
        Confidence::Community => "community",
        Confidence::Conflicted => "conflicted",
        Confidence::Unknown => "unknown",
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexVersion {
    pub knowledge_version: String,
    pub configured_game_version: Option<String>,
    pub matches: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndexedSummary {
    pub record_id: String,
    pub record_type: String,
    pub title: String,
    pub summary: String,
    pub provenance: ProvenanceBrief,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndexSearchAnswer {
    pub status: IndexStatus,
    pub results: Vec<IndexedSummary>,
    pub version: IndexVersion,
    pub uncertainty: Vec<String>,
}

pub struct KnowledgeIndex {
    index: Index,
    reader: IndexReader,
    summaries: BTreeMap<String, IndexedSummary>,
    record_id_field: Field,
    title_field: Field,
    summary_field: Field,
    aliases_field: Field,
    version: IndexVersion,
}

impl KnowledgeIndex {
    pub fn from_store(
        store: &KnowledgeStore,
        configured_game_version: Option<String>,
    ) -> Result<Self, String> {
        let mut schema_builder = Schema::builder();
        let record_id_field = schema_builder.add_text_field("record_id", STORED);
        let title_field = schema_builder.add_text_field("title", TEXT);
        let summary_field = schema_builder.add_text_field("summary", TEXT);
        let aliases_field = schema_builder.add_text_field("aliases", TEXT);
        let schema = schema_builder.build();
        let index = Index::create_in_ram(schema);
        let mut writer = index
            .writer(50_000_000)
            .map_err(|error| error.to_string())?;

        let mut aliases_by_target: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for alias in store.aliases() {
            aliases_by_target
                .entry(alias.target_id.clone())
                .or_default()
                .push(alias.alias.clone());
        }

        let mut summaries = BTreeMap::new();
        let mut add_summary = |summary: IndexedSummary| {
            let aliases = aliases_by_target
                .get(&summary.record_id)
                .cloned()
                .unwrap_or_default()
                .join(" ");
            let record_id = summary.record_id.clone();
            writer
                .add_document(doc!(
                    record_id_field => record_id.as_str(),
                    title_field => summary.title.as_str(),
                    summary_field => summary.summary.as_str(),
                    aliases_field => aliases.as_str(),
                ))
                .map_err(|error| error.to_string())?;
            summaries.insert(summary.record_id.clone(), summary);
            Ok::<(), String>(())
        };

        for record in store.items() {
            add_summary(item_summary(record, store))?;
        }
        for record in store.pals() {
            add_summary(pal_summary(record, store))?;
        }
        for record in store.technologies() {
            add_summary(technology_summary(record, store))?;
        }
        for record in store.recipes() {
            add_summary(recipe_summary(record, store))?;
        }
        for record in store.habitats() {
            add_summary(habitat_summary(record, store))?;
        }
        for record in store.type_effectiveness() {
            add_summary(type_effectiveness_summary(record))?;
        }
        for record in store.work_kind_descriptions() {
            add_summary(work_kind_description_summary(record))?;
        }
        for record in store.waza() {
            add_summary(waza_summary(record))?;
        }
        for record in store.pal_waza_unlocks() {
            add_summary(pal_waza_unlock_summary(record, store))?;
        }
        writer.commit().map_err(|error| error.to_string())?;
        let reader = index.reader().map_err(|error| error.to_string())?;

        let versions: BTreeSet<String> = summaries
            .values()
            .map(|summary| summary.provenance.applicable_game_version.clone())
            .collect();
        let knowledge_version = match versions.len() {
            0 => "unknown".to_string(),
            1 => versions.into_iter().next().expect("one version is present"),
            _ => "mixed".to_string(),
        };
        let matches = configured_game_version
            .as_ref()
            .map(|configured| *configured == knowledge_version)
            .unwrap_or(true);
        let version = IndexVersion {
            knowledge_version,
            configured_game_version,
            matches,
        };

        Ok(Self {
            index,
            reader,
            summaries,
            record_id_field,
            title_field,
            summary_field,
            aliases_field,
            version,
        })
    }

    pub fn search(&self, query: &str, limit: usize) -> IndexSearchAnswer {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return self.unknown_answer(vec!["empty query".to_string()]);
        }
        if limit == 0 {
            return self.unknown_answer(vec!["result limit must be greater than zero".to_string()]);
        }

        let mut subqueries: Vec<(Occur, Box<dyn Query>)> = Vec::new();
        let title_parser = QueryParser::for_index(&self.index, vec![self.title_field]);
        let summary_parser = QueryParser::for_index(&self.index, vec![self.summary_field]);
        let aliases_parser = QueryParser::for_index(&self.index, vec![self.aliases_field]);
        if let Ok(query) = title_parser.parse_query(trimmed) {
            subqueries.push((Occur::Should, Box::new(BoostQuery::new(query, 3.0))));
        }
        if let Ok(query) = summary_parser.parse_query(trimmed) {
            subqueries.push((Occur::Should, Box::new(BoostQuery::new(query, 1.0))));
        }
        if let Ok(query) = aliases_parser.parse_query(trimmed) {
            subqueries.push((Occur::Should, Box::new(BoostQuery::new(query, 2.0))));
        }
        if subqueries.is_empty() {
            return self.unknown_answer(vec![format!("no reviewed records matched \"{trimmed}\"")]);
        }

        let combined = BooleanQuery::new(subqueries);
        let searcher = self.reader.searcher();
        let top_documents = match searcher.search(&combined, &TopDocs::with_limit(limit)) {
            Ok(documents) => documents,
            Err(error) => {
                return self.unknown_answer(vec![format!("search failed: {error}")]);
            }
        };
        let results = top_documents
            .iter()
            .filter_map(|(_score, document_address)| {
                let document: TantivyDocument = searcher.doc(*document_address).ok()?;
                let record_id = document
                    .get_first(self.record_id_field)
                    .and_then(|value| value.as_str())?
                    .to_string();
                self.summaries.get(&record_id).cloned()
            })
            .collect::<Vec<_>>();
        if results.is_empty() {
            return self.unknown_answer(vec![format!("no reviewed records matched \"{trimmed}\"")]);
        }

        let mut uncertainty = Vec::new();
        self.push_version_warning(&mut uncertainty);
        IndexSearchAnswer {
            status: IndexStatus::Ok,
            results,
            version: self.version.clone(),
            uncertainty,
        }
    }

    pub fn version(&self) -> IndexVersion {
        self.version.clone()
    }

    fn unknown_answer(&self, uncertainty: Vec<String>) -> IndexSearchAnswer {
        let mut uncertainty = uncertainty;
        self.push_version_warning(&mut uncertainty);
        IndexSearchAnswer {
            status: IndexStatus::Unknown,
            results: Vec::new(),
            version: self.version.clone(),
            uncertainty,
        }
    }

    fn push_version_warning(&self, uncertainty: &mut Vec<String>) {
        if let Some(configured) = &self.version.configured_game_version {
            if !self.version.matches {
                uncertainty.push(format!(
                    "knowledge version {} does not match configured game version {}",
                    self.version.knowledge_version, configured
                ));
            }
        }
    }
}

fn item_summary(record: &ItemRecord, store: &KnowledgeStore) -> IndexedSummary {
    let mut summary = format!("Item {}. Rarity {}.", record.names.en, record.rarity);
    if let Some(description) = &record.description {
        summary.push(' ');
        summary.push_str(description);
    }
    if !record.acquisition_leads.is_empty() {
        let actions = record
            .acquisition_leads
            .iter()
            .map(|lead| match &lead.notes {
                Some(notes) => format!("{} ({})", lead.action, notes),
                None => lead.action.clone(),
            })
            .collect::<Vec<_>>()
            .join("; ");
        summary.push_str(" Acquisition: ");
        summary.push_str(&actions);
        summary.push('.');
    }
    let mut crafting = Vec::new();
    for recipe in store.recipes() {
        if recipe.output.item_id == record.id {
            let ingredients = recipe
                .ingredients
                .iter()
                .map(|ingredient| {
                    format!(
                        "{} {}",
                        ingredient.quantity,
                        item_name(store, &ingredient.item_id)
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            crafting.push(format!(
                "craft {} at {} using {}",
                recipe.output.quantity,
                recipe.crafting_stations.join(", "),
                ingredients
            ));
        } else if recipe
            .byproducts
            .iter()
            .any(|byproduct| byproduct.item_id == record.id)
        {
            crafting.push(format!(
                "byproduct of {} at {}",
                item_name(store, &recipe.output.item_id),
                recipe.crafting_stations.join(", ")
            ));
        }
    }
    if !crafting.is_empty() {
        summary.push_str(" Crafting: ");
        summary.push_str(&crafting.join("; "));
        summary.push('.');
    }
    let mut used_as_ingredient = Vec::new();
    for recipe in store.recipes() {
        if recipe
            .ingredients
            .iter()
            .any(|ingredient| ingredient.item_id == record.id)
        {
            used_as_ingredient.push(format!(
                "craft {} at {}",
                item_name(store, &recipe.output.item_id),
                recipe.crafting_stations.join(", ")
            ));
        }
    }
    if !used_as_ingredient.is_empty() {
        summary.push_str(" Used as ingredient: ");
        summary.push_str(&used_as_ingredient.join("; "));
        summary.push('.');
    }
    IndexedSummary {
        record_id: record.id.clone(),
        record_type: "item".to_string(),
        title: record.names.en.clone(),
        summary,
        provenance: ProvenanceBrief::from_provenance(&record.provenance),
    }
}

fn pal_summary(record: &PalRecord, store: &KnowledgeStore) -> IndexedSummary {
    let mut summary = format!("Pal {}.", record.names.en);
    if !record.work_suitability.is_empty() {
        let work = record
            .work_suitability
            .iter()
            .map(|work| {
                let kind = serde_json::to_value(work.kind)
                    .ok()
                    .and_then(|value| value.as_str().map(str::to_string))
                    .unwrap_or_else(|| "unknown".to_string());
                format!("{kind} level {}", work.level)
            })
            .collect::<Vec<_>>()
            .join("; ");
        summary.push_str(" Work suitability: ");
        summary.push_str(&work);
        summary.push('.');
    }
    if !record.drops.is_empty() {
        let drops = record
            .drops
            .iter()
            .map(|drop| {
                let name = item_name(store, &drop.item_id);
                format!(
                    "{name} ({}-{}, {}%)",
                    drop.min_quantity, drop.max_quantity, drop.probability_percent
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        summary.push_str(" Drops: ");
        summary.push_str(&drops);
        summary.push('.');
    }
    if !record.habitat_ids.is_empty() {
        let habitats = record
            .habitat_ids
            .iter()
            .map(|habitat_id| {
                store
                    .habitat(habitat_id)
                    .map(|habitat| habitat.names.en.clone())
                    .unwrap_or_else(|| habitat_id.clone())
            })
            .collect::<Vec<_>>()
            .join("; ");
        summary.push_str(" Habitats: ");
        summary.push_str(&habitats);
        summary.push('.');
    }
    IndexedSummary {
        record_id: record.id.clone(),
        record_type: "pal".to_string(),
        title: record.names.en.clone(),
        summary,
        provenance: ProvenanceBrief::from_provenance(&record.provenance),
    }
}

fn technology_summary(record: &TechnologyRecord, store: &KnowledgeStore) -> IndexedSummary {
    let unlocked_recipes = store
        .recipes()
        .filter(|recipe| recipe.technology_id.as_deref() == Some(record.id.as_str()))
        .map(|recipe| recipe.id.clone())
        .collect::<Vec<_>>()
        .join("; ");
    let mut summary = format!("Technology {} at level {}.", record.names.en, record.level);
    if !unlocked_recipes.is_empty() {
        summary.push_str(" Unlocked recipes: ");
        summary.push_str(&unlocked_recipes);
        summary.push('.');
    }
    IndexedSummary {
        record_id: record.id.clone(),
        record_type: "technology".to_string(),
        title: record.names.en.clone(),
        summary,
        provenance: ProvenanceBrief::from_provenance(&record.provenance),
    }
}

fn recipe_summary(record: &RecipeRecord, store: &KnowledgeStore) -> IndexedSummary {
    let output_name = item_name(store, &record.output.item_id);
    let ingredients = record
        .ingredients
        .iter()
        .map(|ingredient| {
            format!(
                "{} {}",
                ingredient.quantity,
                item_name(store, &ingredient.item_id)
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    let mut summary = format!(
        "Recipe {}: craft {} {} at {} using {}.",
        output_name,
        record.output.quantity,
        output_name,
        record.crafting_stations.join(", "),
        ingredients
    );
    if let Some(technology_id) = &record.technology_id {
        let technology = store
            .technology(technology_id)
            .map(|technology| technology.names.en.clone())
            .unwrap_or_else(|| technology_id.clone());
        summary.push_str(" Requires ");
        summary.push_str(&technology);
        summary.push('.');
    }
    IndexedSummary {
        record_id: record.id.clone(),
        record_type: "recipe".to_string(),
        title: format!("Recipe {output_name}"),
        summary,
        provenance: ProvenanceBrief::from_provenance(&record.provenance),
    }
}

fn habitat_summary(record: &HabitatRecord, store: &KnowledgeStore) -> IndexedSummary {
    let pals = record
        .pal_ids
        .iter()
        .map(|pal_id| {
            store
                .pal(pal_id)
                .map(|pal| pal.names.en.clone())
                .unwrap_or_else(|| pal_id.clone())
        })
        .collect::<Vec<_>>()
        .join("; ");
    let summary = if pals.is_empty() {
        format!("Habitat {}.", record.names.en)
    } else {
        format!("Habitat {}. Pals: {pals}.", record.names.en)
    };
    IndexedSummary {
        record_id: record.id.clone(),
        record_type: "habitat".to_string(),
        title: record.names.en.clone(),
        summary,
        provenance: ProvenanceBrief::from_provenance(&record.provenance),
    }
}

fn type_effectiveness_summary(record: &TypeEffectivenessRecord) -> IndexedSummary {
    let attacking = element_label(&record.attacking_type);
    let defending = element_label(&record.defending_type);
    IndexedSummary {
        record_id: record.id.clone(),
        record_type: "type_effectiveness".to_string(),
        title: format!("{attacking} vs {defending}"),
        summary: format!(
            "{attacking} attacks {defending} for {}x damage.",
            record.multiplier
        ),
        provenance: ProvenanceBrief::from_provenance(&record.provenance),
    }
}

fn work_kind_description_summary(record: &WorkKindDescriptionRecord) -> IndexedSummary {
    let kind = serde_json::to_value(record.work_kind)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| record.id.clone());
    IndexedSummary {
        record_id: record.id.clone(),
        record_type: "work_kind_description".to_string(),
        title: record.names.en.clone(),
        summary: format!("Work kind {kind}: {}.", record.description),
        provenance: ProvenanceBrief::from_provenance(&record.provenance),
    }
}

fn waza_summary(record: &WazaRecord) -> IndexedSummary {
    let element = record
        .element
        .as_ref()
        .map(element_label)
        .unwrap_or_else(|| "untyped".to_string());
    IndexedSummary {
        record_id: record.id.clone(),
        record_type: "waza".to_string(),
        title: record.names.en.clone(),
        summary: format!(
            "Active skill {} ({element}, power {}, cooldown {}s).",
            record.names.en, record.power, record.cool_time
        ),
        provenance: ProvenanceBrief::from_provenance(&record.provenance),
    }
}

fn pal_waza_unlock_summary(record: &PalWazaUnlock, store: &KnowledgeStore) -> IndexedSummary {
    let pal_name = store
        .pal(&record.pal_id)
        .map(|pal| pal.names.en.clone())
        .unwrap_or_else(|| record.pal_id.clone());
    let waza_name = store
        .waza_by_id(&record.waza_id)
        .map(|waza| waza.names.en.clone())
        .unwrap_or_else(|| record.waza_id.clone());
    IndexedSummary {
        record_id: record.id.clone(),
        record_type: "pal_waza_unlock".to_string(),
        title: format!("{pal_name} learns {waza_name}"),
        summary: format!(
            "{pal_name} unlocks {waza_name} at level {}.",
            record.unlock_level
        ),
        provenance: ProvenanceBrief::from_provenance(&record.provenance),
    }
}

fn element_label(kind: &game_knowledge::ElementType) -> String {
    serde_json::to_value(kind)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| "unknown".to_string())
}

fn item_name(store: &KnowledgeStore, item_id: &str) -> String {
    store
        .item(item_id)
        .map(|item| item.names.en.clone())
        .unwrap_or_else(|| item_id.to_string())
}
