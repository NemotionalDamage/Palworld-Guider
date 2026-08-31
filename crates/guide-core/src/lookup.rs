use crate::answers::AnswerContext;
use crate::{AnswerStatus, GuideEngine};
use game_knowledge::{
    AcquisitionLead, DropSource, LocaleNames, PalStats, Provenance, RecipeRecord, WorkSuitability,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecipeSummary {
    pub id: String,
    pub output_item_id: String,
    pub output_quantity: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemLookup {
    pub id: String,
    pub names: LocaleNames,
    pub description: Option<String>,
    pub rarity: String,
    pub acquisition_leads: Vec<AcquisitionLead>,
    pub produced_by: Vec<RecipeSummary>,
    pub used_as_ingredient: Vec<RecipeSummary>,
    pub pal_drop_sources: Vec<String>,
    pub provenance: Provenance,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PalLookup {
    pub id: String,
    pub names: LocaleNames,
    pub stats: Option<PalStats>,
    pub work_suitability: Vec<WorkSuitability>,
    pub drops: Vec<DropSource>,
    pub habitat_ids: Vec<String>,
    pub provenance: Provenance,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TechnologyLookup {
    pub id: String,
    pub names: LocaleNames,
    pub level: u32,
    pub unlocked_recipe_ids: Vec<String>,
    pub provenance: Provenance,
}

pub type RecipeLookup = RecipeRecord;

fn ambiguous_message(entity: &str, candidates: &[crate::ResolvedEntity]) -> String {
    let ids = candidates
        .iter()
        .map(|candidate| candidate.id.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    format!("ambiguous {entity} name; candidates: {ids}; no fact was selected")
}

impl GuideEngine {
    pub(crate) fn context(&self) -> AnswerContext<'_> {
        AnswerContext {
            configured_game_version: self.configured_game_version.as_deref(),
        }
    }

    pub fn lookup_item(&self, query: &str) -> crate::GuideAnswer<ItemLookup> {
        let resolved = match self.resolve(query, Some(crate::EntityKind::Item)) {
            crate::resolver::Resolution::Unique(resolved) => resolved,
            crate::resolver::Resolution::Ambiguous(candidates) => {
                return self
                    .context()
                    .ambiguous(ambiguous_message("item", &candidates))
            }
            crate::resolver::Resolution::Unknown => {
                return self
                    .context()
                    .unknown("unknown item; no reviewed record matches")
            }
        };
        let item = self
            .store()
            .item(&resolved.id)
            .expect("resolved item exists");
        let mut provenances = vec![&item.provenance];

        let mut produced_by = Vec::new();
        let mut used_as_ingredient = Vec::new();
        for recipe in self.store().recipes() {
            if recipe.output.item_id == item.id {
                produced_by.push(RecipeSummary {
                    id: recipe.id.clone(),
                    output_item_id: recipe.output.item_id.clone(),
                    output_quantity: recipe.output.quantity,
                });
                provenances.push(&recipe.provenance);
            }
            if recipe
                .ingredients
                .iter()
                .any(|ingredient| ingredient.item_id == item.id)
            {
                used_as_ingredient.push(RecipeSummary {
                    id: recipe.id.clone(),
                    output_item_id: recipe.output.item_id.clone(),
                    output_quantity: recipe.output.quantity,
                });
                provenances.push(&recipe.provenance);
            }
        }

        let mut pal_drop_sources = Vec::new();
        for pal in self.store().pals() {
            if pal.drops.iter().any(|drop| drop.item_id == item.id) {
                pal_drop_sources.push(pal.id.clone());
                provenances.push(&pal.provenance);
            }
        }

        let lookup = ItemLookup {
            id: item.id.clone(),
            names: item.names.clone(),
            description: item.description.clone(),
            rarity: item.rarity.clone(),
            acquisition_leads: item.acquisition_leads.clone(),
            produced_by,
            used_as_ingredient,
            pal_drop_sources,
            provenance: item.provenance.clone(),
        };
        let conflicts = self.store().conflicts_for_subject(&item.id);
        let status = if conflicts.is_empty() {
            AnswerStatus::Ok
        } else {
            AnswerStatus::Ambiguous
        };
        self.context()
            .answer(status, Some(lookup), provenances, conflicts)
    }

    pub fn lookup_pal(&self, query: &str) -> crate::GuideAnswer<PalLookup> {
        let resolved = match self.resolve(query, Some(crate::EntityKind::Pal)) {
            crate::resolver::Resolution::Unique(resolved) => resolved,
            crate::resolver::Resolution::Ambiguous(candidates) => {
                return self
                    .context()
                    .ambiguous(ambiguous_message("Pal", &candidates))
            }
            crate::resolver::Resolution::Unknown => {
                return self
                    .context()
                    .unknown("unknown Pal; no reviewed record matches")
            }
        };
        let pal = self.store().pal(&resolved.id).expect("resolved Pal exists");
        let lookup = PalLookup {
            id: pal.id.clone(),
            names: pal.names.clone(),
            stats: pal.stats.clone(),
            work_suitability: pal.work_suitability.clone(),
            drops: pal.drops.clone(),
            habitat_ids: pal.habitat_ids.clone(),
            provenance: pal.provenance.clone(),
        };
        let conflicts = self.store().conflicts_for_subject(&pal.id);
        let status = if conflicts.is_empty() {
            AnswerStatus::Ok
        } else {
            AnswerStatus::Ambiguous
        };
        self.context()
            .answer(status, Some(lookup), vec![&pal.provenance], conflicts)
    }

    pub fn lookup_technology(&self, query: &str) -> crate::GuideAnswer<TechnologyLookup> {
        let resolved = match self.resolve(query, Some(crate::EntityKind::Technology)) {
            crate::resolver::Resolution::Unique(resolved) => resolved,
            crate::resolver::Resolution::Ambiguous(candidates) => {
                return self
                    .context()
                    .ambiguous(ambiguous_message("technology", &candidates))
            }
            crate::resolver::Resolution::Unknown => {
                return self
                    .context()
                    .unknown("unknown technology; no reviewed record matches")
            }
        };
        let technology = self
            .store()
            .technology(&resolved.id)
            .expect("resolved technology exists");
        let unlocked_recipe_ids = self
            .store()
            .recipes()
            .filter(|recipe| recipe.technology_id.as_deref() == Some(technology.id.as_str()))
            .map(|recipe| recipe.id.clone())
            .collect();
        let lookup = TechnologyLookup {
            id: technology.id.clone(),
            names: technology.names.clone(),
            level: technology.level,
            unlocked_recipe_ids,
            provenance: technology.provenance.clone(),
        };
        let conflicts = self.store().conflicts_for_subject(&technology.id);
        let status = if conflicts.is_empty() {
            AnswerStatus::Ok
        } else {
            AnswerStatus::Ambiguous
        };
        self.context().answer(
            status,
            Some(lookup),
            vec![&technology.provenance],
            conflicts,
        )
    }

    pub fn lookup_recipe(&self, query: &str) -> crate::GuideAnswer<RecipeLookup> {
        let resolved = match self.resolve(query, Some(crate::EntityKind::Recipe)) {
            crate::resolver::Resolution::Unique(resolved) => resolved,
            crate::resolver::Resolution::Ambiguous(candidates) => {
                return self
                    .context()
                    .ambiguous(ambiguous_message("recipe", &candidates))
            }
            crate::resolver::Resolution::Unknown => {
                return self
                    .context()
                    .unknown("unknown recipe; no reviewed output matches")
            }
        };
        let recipe = self
            .store()
            .recipe(&resolved.id)
            .expect("resolved recipe exists");
        let conflicts = self.store().conflicts_for_subject(&recipe.id);
        let status = if conflicts.is_empty() {
            AnswerStatus::Ok
        } else {
            AnswerStatus::Ambiguous
        };
        self.context().answer(
            status,
            Some(recipe.clone()),
            vec![&recipe.provenance],
            conflicts,
        )
    }
}
