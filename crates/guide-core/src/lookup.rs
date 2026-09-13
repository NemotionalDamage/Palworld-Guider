use crate::answers::AnswerContext;
use crate::resolver::{normalize, rarity_rank};
use crate::{AnswerStatus, GuideEngine};
use game_knowledge::{
    AcquisitionLead, ConflictResolution, DropSource, ElementType, ItemRecord, LocaleNames,
    Provenance, RecipeRecord, WorkSuitability,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecipeSummary {
    pub id: String,
    pub output_item_id: String,
    pub output_item_name: String,
    pub output_quantity: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ingredient_quantity: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ByproductRecipeSummary {
    pub id: String,
    pub source_output_item_id: String,
    pub source_output_item_name: String,
    pub byproduct_quantity: u32,
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
    pub byproduct_of: Vec<ByproductRecipeSummary>,
    pub pal_drop_sources: Vec<String>,
    pub provenance: Provenance,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PalLookup {
    pub id: String,
    pub names: LocaleNames,
    pub work_suitability: Vec<WorkSuitability>,
    pub drops: Vec<DropSource>,
    pub habitat_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub habitat_leads: Vec<AcquisitionLead>,
    pub element_type1: Option<ElementType>,
    pub element_type2: Option<ElementType>,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecipeLookup {
    #[serde(flatten)]
    pub recipe: RecipeRecord,
    /// Player-facing advice that the output item's recipe is locked behind its matching schematic.
    /// Empty when the item family is not schematic-driven (world-found items such as Pal eggs,
    /// treasure maps, and technology-unlocked Grappling Gun tiers) or when the resolved tier is
    /// crafted from the recipe itself and only its higher tiers need schematics. A
    /// schematic-locked item that ships as a single tier still names its own schematic.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub schematic_leads: Vec<AcquisitionLead>,
    /// Every other reviewed recipe that produces the same output item. An item can have several
    /// reviewed recipes, such as Carbon Fiber from Coal or from Charcoal, and the whole set is
    /// answered instead of refusing the query. Empty when the item has a single reviewed recipe.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alternative_recipes: Vec<RecipeRecord>,
}

pub fn describe_acquisition_leads(leads: &[AcquisitionLead]) -> String {
    leads
        .iter()
        .map(|lead| match lead.notes.as_deref() {
            Some(notes) => format!("{} ({notes})", lead.action),
            None => lead.action.clone(),
        })
        .collect::<Vec<_>>()
        .join("; ")
}

impl GuideEngine {
    /// Rank variants of one item share a single localized name, so one name can
    /// match several records. Each candidate is labelled with the detail that
    /// tells it apart, and the caller is told how to narrow the next attempt.
    pub(crate) fn ambiguous_message(
        &self,
        entity: &str,
        candidates: &[crate::ResolvedEntity],
    ) -> String {
        let labels = candidates
            .iter()
            .map(|candidate| self.candidate_label(candidate))
            .collect::<Vec<_>>()
            .join(", ");
        let distinct_rarities = candidates
            .iter()
            .filter_map(|candidate| self.candidate_rarity(candidate))
            .collect::<BTreeSet<_>>();
        let rarity_hint = if distinct_rarities.len() > 1 {
            "; retry with a rarity to choose one"
        } else {
            ""
        };
        format!("ambiguous {entity} name; candidates: {labels}{rarity_hint}; no fact was selected")
    }

    fn candidate_label(&self, candidate: &crate::ResolvedEntity) -> String {
        let name = if candidate.matched_name.is_empty() {
            candidate.id.as_str()
        } else {
            candidate.matched_name.as_str()
        };
        match self.candidate_rarity(candidate) {
            Some(rarity) => format!("{name} ({})", rarity.to_ascii_lowercase()),
            None => format!("{name} [{}]", candidate.id),
        }
    }

    fn candidate_rarity(&self, candidate: &crate::ResolvedEntity) -> Option<&str> {
        let item_id = match candidate.kind {
            crate::EntityKind::Item => candidate.id.as_str(),
            crate::EntityKind::Recipe => {
                self.store().recipe(&candidate.id)?.output.item_id.as_str()
            }
            _ => return None,
        };
        self.store().item(item_id).map(|item| item.rarity.as_str())
    }

    pub(crate) fn context(&self) -> AnswerContext<'_> {
        AnswerContext {
            configured_game_version: self.configured_game_version.as_deref(),
        }
    }

    pub fn lookup_item(&self, query: &str) -> crate::GuideAnswer<ItemLookup> {
        self.lookup_item_filtered(query, None)
    }

    pub fn lookup_item_filtered(
        &self,
        query: &str,
        rarity: Option<&str>,
    ) -> crate::GuideAnswer<ItemLookup> {
        let resolved = match self.resolve_with_rarity(query, Some(crate::EntityKind::Item), rarity)
        {
            crate::resolver::Resolution::Unique(resolved) => resolved,
            crate::resolver::Resolution::Ambiguous(candidates) => {
                return self
                    .context()
                    .ambiguous(self.ambiguous_message("item", &candidates))
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
        let mut related_subject_ids = BTreeSet::from([item.id.clone()]);

        let mut produced_by = Vec::new();
        let mut used_as_ingredient = Vec::new();
        let mut byproduct_of = Vec::new();
        for recipe in self.store().recipes() {
            if recipe.output.item_id == item.id {
                produced_by.push(RecipeSummary {
                    id: recipe.id.clone(),
                    output_item_id: recipe.output.item_id.clone(),
                    output_item_name: self.item_name(&recipe.output.item_id),
                    output_quantity: recipe.output.quantity,
                    ingredient_quantity: None,
                });
                provenances.push(&recipe.provenance);
                related_subject_ids.insert(recipe.id.clone());
            }
            if let Some(ingredient) = recipe
                .ingredients
                .iter()
                .find(|ingredient| ingredient.item_id == item.id)
            {
                used_as_ingredient.push(RecipeSummary {
                    id: recipe.id.clone(),
                    output_item_id: recipe.output.item_id.clone(),
                    output_item_name: self.item_name(&recipe.output.item_id),
                    output_quantity: recipe.output.quantity,
                    ingredient_quantity: Some(ingredient.quantity),
                });
                provenances.push(&recipe.provenance);
                related_subject_ids.insert(recipe.id.clone());
            }
            if let Some(byproduct) = recipe
                .byproducts
                .iter()
                .find(|byproduct| byproduct.item_id == item.id)
            {
                byproduct_of.push(ByproductRecipeSummary {
                    id: recipe.id.clone(),
                    source_output_item_id: recipe.output.item_id.clone(),
                    source_output_item_name: self.item_name(&recipe.output.item_id),
                    byproduct_quantity: byproduct.quantity,
                });
                provenances.push(&recipe.provenance);
                related_subject_ids.insert(recipe.id.clone());
            }
        }

        let mut pal_drop_sources = Vec::new();
        for pal in self.store().pals() {
            if pal.drops.iter().any(|drop| drop.item_id == item.id) {
                pal_drop_sources.push(pal.names.en.clone());
                provenances.push(&pal.provenance);
                related_subject_ids.insert(pal.id.clone());
            }
        }

        let mut acquisition_leads = if produced_by.is_empty() {
            item.acquisition_leads.clone()
        } else {
            self.recipe_acquisition_leads(&produced_by)
        };
        acquisition_leads.extend(self.rank_variant_leads(item));
        let lookup = ItemLookup {
            id: item.id.clone(),
            names: item.names.clone(),
            description: item.description.clone(),
            rarity: item.rarity.clone(),
            acquisition_leads,
            produced_by,
            used_as_ingredient,
            byproduct_of,
            pal_drop_sources,
            provenance: item.provenance.clone(),
        };
        let conflicts = related_subject_ids
            .iter()
            .flat_map(|subject_id| self.store().conflicts_for_subject(subject_id))
            .filter(|conflict| conflict.resolution == ConflictResolution::Unresolved)
            .collect::<Vec<_>>();
        let status = if conflicts.is_empty() {
            AnswerStatus::Ok
        } else {
            AnswerStatus::Ambiguous
        };
        self.context()
            .answer(status, Some(lookup), provenances, conflicts)
    }

    /// Rank variants of one item share a localized name and differ only by rarity. Gear tiers are
    /// unlocked by their matching schematic instead of the base recipe, which the recipes state as
    /// their `unlock_item_id`; the lead is only emitted when the data names such a schematic item.
    /// The schematic check comes first because a single-tier item can be schematic-locked on its
    /// own, without any sibling tier to compare against.
    fn rank_variant_leads(&self, item: &ItemRecord) -> Vec<AcquisitionLead> {
        // The producing recipe names the schematic item that unlocks this tier.
        if let Some(schematic) = self.schematic_for(item) {
            return vec![AcquisitionLead {
                action: "Requires a schematic".to_string(),
                notes: Some(format!(
                    "{} must be in inventory to unlock the {} recipe, so the item cannot be crafted without it",
                    schematic.names.en,
                    item.rarity.to_ascii_lowercase()
                )),
            }];
        }
        let family = self.rank_family(item);
        if family.len() < 2 {
            return Vec::new();
        }
        let Some(base_rank) = family
            .iter()
            .filter_map(|sibling| rarity_rank(&sibling.rarity))
            .min()
        else {
            return Vec::new();
        };
        if rarity_rank(&item.rarity).is_none_or(|rank| rank > base_rank) {
            return Vec::new();
        }
        let higher = family
            .iter()
            .filter(|sibling| rarity_rank(&sibling.rarity).is_some_and(|rank| rank > base_rank))
            .collect::<Vec<_>>();
        if higher.is_empty()
            || higher
                .iter()
                .any(|sibling| self.schematic_for(sibling).is_none())
        {
            return Vec::new();
        }
        vec![AcquisitionLead {
            action: "Higher tiers need schematics".to_string(),
            notes: Some(format!(
                "{} is also shipped as {} tiers, and each tier needs its own schematic",
                item.names.en,
                higher
                    .iter()
                    .map(|sibling| sibling.rarity.to_ascii_lowercase())
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        }]
    }

    fn rank_family<'a>(&'a self, item: &ItemRecord) -> Vec<&'a ItemRecord> {
        let normalized = normalize(&item.names.en);
        self.store()
            .items()
            .filter(|sibling| normalize(&sibling.names.en) == normalized)
            .collect()
    }

    fn schematic_for<'a>(&'a self, item: &ItemRecord) -> Option<&'a ItemRecord> {
        let mut has_recipe = false;
        for recipe in self.store().recipes() {
            if recipe.output.item_id != item.id {
                continue;
            }
            has_recipe = true;
            if let Some(schematic) = recipe
                .unlock_item_id
                .as_deref()
                .and_then(|unlock_item_id| self.store().item(unlock_item_id))
            {
                return Some(schematic);
            }
        }
        // A recipe that produces the item without naming an unlock item means the recipe is usable on
        // its own, so no schematic claim is made. The name-based lookup below only covers items whose
        // recipe row is absent from the reviewed dataset.
        if has_recipe {
            return None;
        }
        let row = item.native_row_id.as_deref()?;
        let blueprint_row = format!("Blueprint_{row}");
        self.store()
            .items()
            .find(|candidate| candidate.native_row_id.as_deref() == Some(blueprint_row.as_str()))
    }

    fn recipe_acquisition_leads(&self, produced_by: &[RecipeSummary]) -> Vec<AcquisitionLead> {
        let mut leads = Vec::new();
        for summary in produced_by {
            let Some(recipe) = self.store().recipe(&summary.id) else {
                continue;
            };
            let ingredients = recipe
                .ingredients
                .iter()
                .map(|ingredient| {
                    let name = self
                        .store()
                        .item(&ingredient.item_id)
                        .map(|item| item.names.en.clone())
                        .unwrap_or_else(|| ingredient.item_id.clone());
                    format!("{name} x{}", ingredient.quantity)
                })
                .collect::<Vec<_>>()
                .join(", ");
            leads.push(AcquisitionLead {
                action: format!("Craft at {}", recipe.crafting_stations.join(", ")),
                notes: Some(format!("Materials: {ingredients}")),
            });
        }
        leads
    }

    fn item_name(&self, item_id: &str) -> String {
        self.store()
            .item(item_id)
            .map(|item| item.names.en.clone())
            .unwrap_or_else(|| item_id.to_string())
    }

    pub fn lookup_pal(&self, query: &str) -> crate::GuideAnswer<PalLookup> {
        let resolved = match self.resolve(query, Some(crate::EntityKind::Pal)) {
            crate::resolver::Resolution::Unique(resolved) => resolved,
            crate::resolver::Resolution::Ambiguous(candidates) => {
                return self
                    .context()
                    .ambiguous(self.ambiguous_message("Pal", &candidates))
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
            work_suitability: pal.work_suitability.clone(),
            drops: pal.drops.clone(),
            habitat_ids: pal.habitat_ids.clone(),
            habitat_leads: pal.habitat_leads.clone(),
            element_type1: pal.element_type1,
            element_type2: pal.element_type2,
            provenance: pal.provenance.clone(),
        };
        let related_subject_ids = pal
            .drops
            .iter()
            .map(|drop| drop.item_id.clone())
            .chain(pal.habitat_ids.iter().cloned())
            .collect::<BTreeSet<_>>();
        let conflicts = std::iter::once(&pal.id)
            .chain(related_subject_ids.iter())
            .flat_map(|subject_id| self.store().conflicts_for_subject(subject_id))
            .filter(|conflict| conflict.resolution == ConflictResolution::Unresolved)
            .collect::<Vec<_>>();
        let status = if conflicts.is_empty() {
            AnswerStatus::Ok
        } else {
            AnswerStatus::Ambiguous
        };
        let mut answer =
            self.context()
                .answer(status, Some(lookup), vec![&pal.provenance], conflicts);
        if let Some(evidence) = pal.local_evidence.as_ref() {
            let mut unknown_fields = BTreeSet::new();
            for field in &evidence.unresolved_fields {
                match field.as_str() {
                    "drops" => {
                        unknown_fields.insert("drops are unknown");
                    }
                    "habitat_ids" => {
                        unknown_fields.insert("habitats are unknown");
                    }
                    _ => {}
                }
            }
            for field in unknown_fields {
                answer
                    .uncertainty
                    .push(format!("{field} for this local-build Pal record"));
            }
        }
        answer
    }

    pub fn lookup_technology(&self, query: &str) -> crate::GuideAnswer<TechnologyLookup> {
        let resolved = match self.resolve(query, Some(crate::EntityKind::Technology)) {
            crate::resolver::Resolution::Unique(resolved) => resolved,
            crate::resolver::Resolution::Ambiguous(candidates) => {
                return self
                    .context()
                    .ambiguous(self.ambiguous_message("technology", &candidates))
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
        let unlocked_recipe_ids: Vec<String> = self
            .store()
            .recipes()
            .filter(|recipe| recipe.technology_id.as_deref() == Some(technology.id.as_str()))
            .map(|recipe| recipe.id.clone())
            .collect();
        let related_subject_ids = unlocked_recipe_ids.iter().cloned().collect::<BTreeSet<_>>();
        let lookup = TechnologyLookup {
            id: technology.id.clone(),
            names: technology.names.clone(),
            level: technology.level,
            unlocked_recipe_ids,
            provenance: technology.provenance.clone(),
        };
        let conflicts = std::iter::once(&technology.id)
            .chain(related_subject_ids.iter())
            .flat_map(|subject_id| self.store().conflicts_for_subject(subject_id))
            .filter(|conflict| conflict.resolution == ConflictResolution::Unresolved)
            .collect::<Vec<_>>();
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

    /// Returns the first candidate when every candidate is a recipe for the same output item.
    /// Several such matches are recipe variants rather than an ambiguous name, so the caller
    /// answers with the whole set instead of asking the player to disambiguate.
    fn shared_output_recipe(
        &self,
        candidates: &[crate::ResolvedEntity],
    ) -> Option<crate::ResolvedEntity> {
        let mut outputs = BTreeSet::new();
        for candidate in candidates {
            let recipe = self.store().recipe(&candidate.id)?;
            outputs.insert(recipe.output.item_id.clone());
        }
        if outputs.len() != 1 {
            return None;
        }
        candidates.first().cloned()
    }

    pub fn lookup_recipe(&self, query: &str) -> crate::GuideAnswer<RecipeLookup> {
        self.lookup_recipe_filtered(query, None)
    }

    pub fn lookup_recipe_filtered(
        &self,
        query: &str,
        rarity: Option<&str>,
    ) -> crate::GuideAnswer<RecipeLookup> {
        let resolved =
            match self.resolve_with_rarity(query, Some(crate::EntityKind::Recipe), rarity) {
                crate::resolver::Resolution::Unique(resolved) => resolved,
                crate::resolver::Resolution::Ambiguous(candidates) => {
                    match self.shared_output_recipe(&candidates) {
                        Some(resolved) => resolved,
                        None => {
                            return self
                                .context()
                                .ambiguous(self.ambiguous_message("recipe", &candidates))
                        }
                    }
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
        let related_subject_ids = recipe
            .ingredients
            .iter()
            .map(|ingredient| ingredient.item_id.clone())
            .chain(
                recipe
                    .byproducts
                    .iter()
                    .map(|byproduct| byproduct.item_id.clone()),
            )
            .chain(std::iter::once(recipe.output.item_id.clone()))
            .chain(recipe.technology_id.iter().cloned())
            .collect::<BTreeSet<_>>();
        let conflicts = std::iter::once(&recipe.id)
            .chain(related_subject_ids.iter())
            .flat_map(|subject_id| self.store().conflicts_for_subject(subject_id))
            .filter(|conflict| conflict.resolution == ConflictResolution::Unresolved)
            .collect::<Vec<_>>();
        let status = if conflicts.is_empty() {
            AnswerStatus::Ok
        } else {
            AnswerStatus::Ambiguous
        };
        let schematic_leads = self
            .store()
            .item(&recipe.output.item_id)
            .map(|item| self.rank_variant_leads(item))
            .unwrap_or_default();
        let alternative_recipes = self
            .store()
            .recipes()
            .filter(|candidate| {
                candidate.id != recipe.id && candidate.output.item_id == recipe.output.item_id
            })
            .cloned()
            .collect::<Vec<_>>();
        let lookup = RecipeLookup {
            recipe: recipe.clone(),
            schematic_leads,
            alternative_recipes,
        };
        self.context()
            .answer(status, Some(lookup), vec![&recipe.provenance], conflicts)
    }
}
