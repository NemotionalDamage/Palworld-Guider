use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::models::WorkKind;
use crate::KnowledgeStore;
use crate::{LocalBuildLocale, LocalBuildTables, LocalizationIndex};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BackfillFact {
    pub canonical_record_id: String,
    pub canonical_field: String,
    pub current_value: String,
    pub local_table: String,
    pub local_native_row_id: String,
    pub local_value: String,
    pub mapping_strategy: String,
    pub classification: String,
    pub difference_explanation: String,
    pub proposed_action: String,
    pub review_status: String,
    pub stable_hash: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BackfillSummary {
    pub total_records: usize,
    pub total_audited_facts: usize,
    pub classification_counts: BTreeMap<String, usize>,
    pub mapping_success_rate: f64,
    pub ambiguous_mappings: usize,
    pub conflicts: usize,
    pub missing_localization: usize,
    pub schema_failures: usize,
    pub provenance_failures: usize,
    pub unclassified_facts: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CanonicalBackfillReport {
    pub summary: BackfillSummary,
    pub facts: Vec<BackfillFact>,
}

pub fn canonical_backfill_output_is_safe(path: &Path) -> bool {
    let components = path
        .components()
        .map(|component| component.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>();
    if components.iter().any(|component| component == "..") {
        return false;
    }
    path.file_name()
        .is_some_and(|filename| filename == "canonical-backfill.json")
        && components.windows(4).any(|window| {
            window
                == [
                    ".local".to_string(),
                    "research".to_string(),
                    "local-build".to_string(),
                    "reports".to_string(),
                ]
        })
}

pub fn audit_canonical_backfill(
    tables: &LocalBuildTables,
    localization: &LocalizationIndex,
    store: &KnowledgeStore,
) -> CanonicalBackfillReport {
    let mut auditor = Auditor {
        tables,
        localization,
        store,
        facts: Vec::new(),
        record_ids: BTreeSet::new(),
    };

    for item in store.items() {
        auditor.record_ids.insert(item.id.clone());
        auditor.audit_item(item);
    }
    for recipe in store.recipes() {
        auditor.record_ids.insert(recipe.id.clone());
        auditor.audit_recipe(recipe);
    }
    for technology in store.technologies() {
        auditor.record_ids.insert(technology.id.clone());
        auditor.audit_technology(technology);
    }
    for pal in store.pals() {
        auditor.record_ids.insert(pal.id.clone());
        auditor.audit_pal(pal);
    }
    for alias in store.aliases() {
        auditor.record_ids.insert(alias.id.clone());
        auditor.audit_alias(alias);
    }
    for relationship in store.progression_relationships() {
        auditor.record_ids.insert(relationship.id.clone());
        auditor.audit_relationship(relationship);
    }
    for conflict in store.conflicts() {
        auditor.record_ids.insert(conflict.id.clone());
        auditor.audit_conflict(conflict);
    }

    let mut classification_counts = BTreeMap::new();
    for fact in &auditor.facts {
        *classification_counts
            .entry(fact.classification.clone())
            .or_insert(0) += 1;
    }
    let mapped_count = auditor
        .facts
        .iter()
        .filter(|fact| !fact.local_native_row_id.is_empty())
        .count();
    let total_facts = auditor.facts.len();
    let mapping_success_rate = if total_facts == 0 {
        0.0
    } else {
        mapped_count as f64 / total_facts as f64 * 100.0
    };
    CanonicalBackfillReport {
        summary: BackfillSummary {
            total_records: auditor.record_ids.len(),
            total_audited_facts: total_facts,
            classification_counts,
            mapping_success_rate,
            ambiguous_mappings: auditor
                .facts
                .iter()
                .filter(|fact| fact.classification == "ambiguous_mapping")
                .count(),
            conflicts: auditor
                .facts
                .iter()
                .filter(|fact| fact.classification == "conflicting")
                .count(),
            missing_localization: auditor
                .facts
                .iter()
                .filter(|fact| {
                    fact.classification == "unresolved_mapping"
                        && fact.difference_explanation.contains("missing localization")
                })
                .count(),
            schema_failures: 0,
            provenance_failures: 0,
            unclassified_facts: auditor
                .facts
                .iter()
                .filter(|fact| fact.classification.is_empty())
                .count(),
        },
        facts: auditor.facts,
    }
}

struct Auditor<'a> {
    tables: &'a LocalBuildTables,
    localization: &'a LocalizationIndex,
    store: &'a KnowledgeStore,
    facts: Vec<BackfillFact>,
    record_ids: BTreeSet<String>,
}

impl Auditor<'_> {
    fn audit_item(&mut self, item: &crate::models::ItemRecord) {
        let native_id = item.native_row_id.as_deref().unwrap_or_default();
        let Some(local_rarity) = self
            .tables
            .item(native_id)
            .map(|row| format!("{:?}", row.rarity))
        else {
            self.push_unmapped(
                item.id.as_str(),
                "record",
                "item row is not represented locally",
            );
            return;
        };

        let english = self
            .localization
            .item_name(native_id, LocalBuildLocale::English)
            .ok()
            .flatten()
            .unwrap_or_default();
        if english == item.names.en {
            self.push(
                &item.id,
                "names.en",
                &item.names.en,
                "DT_ItemNameText_Common",
                native_id,
                &english,
                "native_row_id",
                "corroborated_exact",
                "",
                "retain corroboration",
            );
        } else {
            self.push(
                &item.id,
                "names.en",
                &item.names.en,
                "DT_ItemNameText_Common",
                native_id,
                &english,
                "native_row_id",
                "conflicting",
                "localized English names differ",
                "keep conflict explicit",
            );
        }

        if let Some(chinese) = item.names.zh_hans.as_deref() {
            let local_chinese = self
                .localization
                .item_name(native_id, LocalBuildLocale::SimplifiedChinese)
                .ok()
                .flatten();
            match local_chinese {
                Some(value) if value == chinese => self.push(
                    &item.id,
                    "names.zh_hans",
                    chinese,
                    "DT_ItemNameText_Common",
                    native_id,
                    &value,
                    "native_row_id",
                    "corroborated_exact",
                    "",
                    "retain corroboration",
                ),
                Some(value) => self.push(
                    &item.id,
                    "names.zh_hans",
                    chinese,
                    "DT_ItemNameText_Common",
                    native_id,
                    &value,
                    "native_row_id",
                    "conflicting",
                    "Simplified Chinese names differ",
                    "keep conflict explicit",
                ),
                None => self.push(
                    &item.id,
                    "names.zh_hans",
                    chinese,
                    "DT_ItemNameText_Common",
                    native_id,
                    "",
                    "native_row_id",
                    "unresolved_mapping",
                    "missing localization for the mapped row",
                    "retain reviewed alias; do not fabricate localization",
                ),
            }
        }

        if let Some(description) = item.description.as_deref() {
            let local_description = self
                .localization
                .item_description(native_id, LocalBuildLocale::English)
                .ok()
                .flatten()
                .unwrap_or_default();
            let normalized_description = local_description.replace("\r\n", " ");
            let differs_only_by_line_endings = normalized_description == description;
            let classification = if local_description == description {
                "corroborated_exact"
            } else if differs_only_by_line_endings || local_description.starts_with(description) {
                "corroborated_partial"
            } else {
                "conflicting"
            };
            self.push(
                &item.id,
                "description",
                description,
                "DT_ItemDescriptionText_Common",
                native_id,
                &local_description,
                "native_row_id",
                classification,
                if classification == "corroborated_partial" {
                    if differs_only_by_line_endings {
                        "the reviewed text differs only by target-build line endings"
                    } else {
                        "the reviewed description is a concise prefix of the target-build text"
                    }
                } else {
                    ""
                },
                "retain reviewed summary; promote fuller text only after review",
            );
        }

        self.push(
            &item.id,
            "rarity",
            &item.rarity,
            "DT_ItemDataTable",
            native_id,
            &local_rarity,
            "native_row_id",
            "unresolved_mapping",
            "the numeric target-build rarity representation is not yet semantically confirmed",
            "do not add local corroboration until rarity semantics are reviewed",
        );
        for (index, lead) in item.acquisition_leads.iter().enumerate() {
            self.push(
                &item.id,
                &format!("acquisition_leads[{index}].action"),
                &lead.action,
                "",
                "",
                "",
                "not represented in selected structural table",
                "not_represented_locally",
                "selected local rows do not encode this action relationship",
                "retain the original provenance",
            );
        }
    }

    fn audit_recipe(&mut self, recipe: &crate::models::RecipeRecord) {
        let native_id = recipe.native_row_id.as_deref().unwrap_or_default();
        let Some(local) = self.tables.recipe(native_id) else {
            self.push_unmapped(
                recipe.id.as_str(),
                "record",
                "recipe row is not represented locally",
            );
            return;
        };
        let output_native = self.item_native(&recipe.output.item_id).to_string();
        let output_name = self
            .store
            .item(&recipe.output.item_id)
            .map(|item| item.names.en.clone())
            .unwrap_or_default();
        let local_output_name = local
            .product_id
            .as_deref()
            .and_then(|id| {
                self.localization
                    .item_name(id, LocalBuildLocale::English)
                    .ok()
                    .flatten()
            })
            .unwrap_or_default();
        if output_name != local_output_name {
            self.push(
                &recipe.id,
                "output.product_identity",
                &output_name,
                "DT_ItemNameText_Common",
                native_id,
                &local_output_name,
                "native_row_id",
                "conflicting",
                "the local recipe output has a different localized identity",
                "keep explicit conflict",
            );
        } else {
            self.push(
                &recipe.id,
                "output.item_id",
                &output_native,
                "DT_ItemRecipeDataTable",
                native_id,
                local.product_id.as_deref().unwrap_or_default(),
                "native_row_id",
                "corroborated_exact",
                "",
                "retain corroboration",
            );
        }
        self.push(
            &recipe.id,
            "output.quantity",
            &recipe.output.quantity.to_string(),
            "DT_ItemRecipeDataTable",
            native_id,
            &local
                .product_count
                .map(|count| count.to_string())
                .unwrap_or_default(),
            "native_row_id",
            if local.product_count == Some(recipe.output.quantity as i64)
                && output_name == local_output_name
            {
                "corroborated_exact"
            } else if local.product_count == Some(recipe.output.quantity as i64) {
                "corroborated_partial"
            } else {
                "conflicting"
            },
            "",
            "retain reviewed value unless a reviewed non-conflicting correction is approved",
        );

        let mut expected = recipe
            .ingredients
            .iter()
            .map(|ingredient| {
                (
                    self.item_native(&ingredient.item_id).to_string(),
                    ingredient.quantity,
                )
            })
            .collect::<Vec<_>>();
        expected.sort();
        let mut actual = local
            .materials
            .iter()
            .map(|material| {
                (
                    material.item_id.clone(),
                    u32::try_from(material.quantity).unwrap_or_default(),
                )
            })
            .collect::<Vec<_>>();
        actual.sort();
        self.push(
            &recipe.id,
            "ingredients",
            &format!("{expected:?}"),
            "DT_ItemRecipeDataTable",
            native_id,
            &format!("{actual:?}"),
            "native_row_id",
            if expected == actual {
                "corroborated_exact"
            } else {
                "conflicting"
            },
            if expected != actual {
                "the ingredient sets or quantities differ"
            } else {
                ""
            },
            if expected == actual {
                "retain corroboration"
            } else {
                "keep explicit conflict"
            },
        );

        for (index, station) in recipe.crafting_stations.iter().enumerate() {
            self.push(
                &recipe.id,
                &format!("crafting_stations[{index}]"),
                station,
                "",
                "",
                "",
                "not represented in selected structural table",
                "not_represented_locally",
                "WorkableAttribute alone does not prove a crafting-station relationship",
                "retain original provenance; do not add local corroboration",
            );
        }
        if let Some(technology_id) = recipe.technology_id.as_deref() {
            let level = self.store.technology(technology_id).map(|tech| tech.level);
            let unlock_rows = self
                .tables
                .technology_rows()
                .into_iter()
                .filter(|row| row.unlock_item_recipes.iter().any(|id| id == native_id))
                .collect::<Vec<_>>();
            let local_level = unlock_rows.first().and_then(|row| row.level_cap);
            let classification = if unlock_rows.len() > 1 {
                "ambiguous_mapping"
            } else if local_level.map(|value| value as u32) == level
                && output_name == local_output_name
            {
                "corroborated_exact"
            } else if local_level.map(|value| value as u32) == level {
                "corroborated_partial"
            } else {
                "conflicting"
            };
            self.push(
                &recipe.id,
                "technology_id",
                technology_id,
                "DT_TechnologyRecipeUnlock",
                &unlock_rows
                    .first()
                    .map(|row| row.native_row_id.clone())
                    .unwrap_or_default(),
                &local_level
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
                "exact structural relationship",
                classification,
                if output_name != local_output_name {
                    "the unlock row exists, but its recipe product identity remains conflicted"
                } else {
                    ""
                },
                "review before adding relation corroboration",
            );
        }
    }

    fn audit_technology(&mut self, technology: &crate::models::TechnologyRecord) {
        self.push(
            &technology.id,
            "names.en",
            &technology.names.en,
            "",
            "",
            "",
            "aggregate canonical level has no unique native row",
            "not_represented_locally",
            "the selected technology table contains unlock rows, not an aggregate level entity",
            "retain existing provenance",
        );
        self.push(
            &technology.id,
            "level",
            &technology.level.to_string(),
            "DT_TechnologyRecipeUnlock",
            "",
            "",
            "aggregate canonical level has no unique native row",
            "not_represented_locally",
            "level values occur on unlock rows but no row maps to the aggregate entity",
            "audit relations structurally instead of claiming row-level corroboration",
        );
    }

    fn audit_pal(&mut self, pal: &crate::models::PalRecord) {
        let native_id = pal.native_row_id.as_deref().unwrap_or_default();
        if self.tables.pal(native_id).is_none() {
            self.push_unmapped(
                pal.id.as_str(),
                "record",
                "Pal row is not represented locally",
            );
            return;
        }
        let english = self
            .localization
            .pal_name(native_id, LocalBuildLocale::English)
            .ok()
            .flatten()
            .unwrap_or_default();
        self.push(
            &pal.id,
            "names.en",
            &pal.names.en,
            "DT_PalNameText_Common",
            native_id,
            &english,
            "native_row_id",
            if english == pal.names.en {
                "corroborated_exact"
            } else {
                "conflicting"
            },
            "",
            "retain the reviewed value",
        );
        let local_work = self
            .tables
            .pal(native_id)
            .expect("row was checked")
            .work_suitability
            .clone();
        for work in &pal.work_suitability {
            let native_kind = work_native_kind(work.kind);
            let local_level = local_work.get(native_kind).copied();
            self.push(
                &pal.id,
                &format!(
                    "work_suitability.{}",
                    format!("{:?}", work.kind).to_lowercase()
                ),
                &work.level.to_string(),
                "DT_PalMonsterParameter",
                native_id,
                &local_level
                    .map(|level| level.to_string())
                    .unwrap_or_default(),
                "native_row_id",
                if local_level == Some(i64::from(work.level)) {
                    "corroborated_exact"
                } else {
                    "conflicting"
                },
                "",
                "retain the reviewed level",
            );
        }
        for (index, drop) in pal.drops.iter().enumerate() {
            let item_native = self.item_native(&drop.item_id).to_string();
            let local_drop = self
                .tables
                .drops_for(native_id)
                .into_iter()
                .find(|row| row.item_id == item_native);
            let row_id = local_drop
                .as_ref()
                .map(|row| row.native_row_id.clone())
                .unwrap_or_default();
            self.push(
                &pal.id,
                &format!("drops[{index}].item_id"),
                &item_native,
                "DT_PalDropItem_Common",
                &row_id,
                local_drop
                    .as_ref()
                    .map(|row| row.item_id.as_str())
                    .unwrap_or_default(),
                "native_row_id plus exact character/item relationship",
                if local_drop.is_some() {
                    "corroborated_exact"
                } else {
                    "conflicting"
                },
                "",
                "retain the reviewed reference",
            );
            self.push(
                &pal.id,
                &format!("drops[{index}].min_quantity"),
                &drop.min_quantity.to_string(),
                "DT_PalDropItem_Common",
                &row_id,
                &local_drop
                    .map(|row| row.min_quantity.to_string())
                    .unwrap_or_default(),
                "native_row_id plus exact character/item relationship",
                if local_drop.map(|row| row.min_quantity as u32) == Some(drop.min_quantity) {
                    "corroborated_exact"
                } else {
                    "conflicting"
                },
                "",
                "retain the reviewed bound",
            );
            self.push(
                &pal.id,
                &format!("drops[{index}].max_quantity"),
                &drop.max_quantity.to_string(),
                "DT_PalDropItem_Common",
                &row_id,
                &local_drop
                    .map(|row| row.max_quantity.to_string())
                    .unwrap_or_default(),
                "native_row_id plus exact character/item relationship",
                if local_drop.map(|row| row.max_quantity as u32) == Some(drop.max_quantity) {
                    "corroborated_exact"
                } else {
                    "conflicting"
                },
                "",
                "retain the reviewed bound",
            );
            self.push(
                &pal.id,
                &format!("drops[{index}].probability_percent"),
                &drop.probability_percent.to_string(),
                "DT_PalDropItem_Common",
                &row_id,
                &local_drop
                    .map(|row| row.rate.to_string())
                    .unwrap_or_default(),
                "native_row_id plus exact character/item relationship",
                "unresolved_mapping",
                "the target-build probability representation and unit are not yet confirmed",
                "do not add local corroboration until rate semantics are reviewed",
            );
        }
    }

    fn audit_alias(&mut self, alias: &crate::models::AliasRecord) {
        let native_id = self.item_native(&alias.target_id).to_string();
        let locale = if alias.locale == "zh_hans" {
            LocalBuildLocale::SimplifiedChinese
        } else {
            LocalBuildLocale::English
        };
        let local_value = self
            .localization
            .item_name(native_id.as_str(), locale)
            .ok()
            .flatten();
        match local_value {
            Some(value) if value == alias.alias => self.push(
                &alias.id,
                "alias",
                &alias.alias,
                "DT_ItemNameText_Common",
                native_id.as_str(),
                &value,
                "target native_row_id plus localization key",
                "corroborated_exact",
                "",
                "retain corroboration",
            ),
            Some(value) => self.push(
                &alias.id,
                "alias",
                &alias.alias,
                "DT_ItemNameText_Common",
                native_id.as_str(),
                &value,
                "target native_row_id plus localization key",
                "not_represented_locally",
                "the reviewed colloquial alias is not the target-build localized name",
                "retain the reviewed alias and original provenance",
            ),
            None => self.push(
                &alias.id,
                "alias",
                &alias.alias,
                "DT_ItemNameText_Common",
                native_id.as_str(),
                "",
                "target native_row_id plus localization key",
                "unresolved_mapping",
                "missing localization for the mapped target row",
                "retain the alias; do not fabricate localization",
            ),
        }
    }

    fn audit_relationship(&mut self, relationship: &crate::models::ProgressionRelationshipRecord) {
        let recipe_native = self
            .store
            .recipe(&relationship.to_id)
            .and_then(|recipe| recipe.native_row_id.clone())
            .unwrap_or_default();
        let unlock_rows = self
            .tables
            .technology_rows()
            .into_iter()
            .filter(|row| row.unlock_item_recipes.contains(&recipe_native))
            .collect::<Vec<_>>();
        let output_conflicted = self
            .store
            .conflicts_for_subject(&relationship.to_id)
            .iter()
            .any(|conflict| {
                matches!(
                    conflict.resolution,
                    crate::models::ConflictResolution::Unresolved
                )
            });
        let classification = if unlock_rows.len() > 1 {
            "ambiguous_mapping"
        } else if unlock_rows.is_empty() {
            "not_represented_locally"
        } else if output_conflicted {
            "conflicting"
        } else {
            "corroborated_exact"
        };
        self.push(
            &relationship.id,
            "relation",
            &format!("{:?}", relationship.relation),
            "DT_TechnologyRecipeUnlock",
            &unlock_rows
                .first()
                .map(|row| row.native_row_id.clone())
                .unwrap_or_default(),
            &unlock_rows
                .first()
                .map(|row| row.unlock_item_recipes.join(","))
                .unwrap_or_default(),
            "exact structural recipe-unlock relationship",
            classification,
            if output_conflicted {
                "the unlock exists locally, but the mapped recipe product identity remains conflicting"
            } else {
                ""
            },
            if classification == "corroborated_exact" { "retain corroboration" } else { "review before corroboration" },
        );
    }

    fn audit_conflict(&mut self, conflict: &crate::models::ConflictRecord) {
        let recipe_subject = self.store.recipe(&conflict.subject_id);
        let local_table = if recipe_subject.is_some() {
            "DT_ItemRecipeDataTable"
        } else {
            "DT_ItemNameText_Common"
        };
        let subject_native = self
            .store
            .item(&conflict.subject_id)
            .and_then(|item| item.native_row_id.clone())
            .or_else(|| recipe_subject.and_then(|recipe| recipe.native_row_id.clone()))
            .unwrap_or_default();
        self.push(
            &conflict.id,
            "values",
            &conflict.values.join(" || "),
            local_table,
            &subject_native,
            &conflict.values.join(" || "),
            "conflict subject native_row_id",
            "conflicting",
            "both evidence values remain intentionally visible",
            "keep unresolved; require product review",
        );
    }

    fn item_native(&self, item_id: &str) -> &str {
        self.store
            .item(item_id)
            .and_then(|item| item.native_row_id.as_deref())
            .unwrap_or_default()
    }

    #[allow(clippy::too_many_arguments)]
    fn push(
        &mut self,
        record_id: &str,
        field: &str,
        current_value: &str,
        local_table: &str,
        local_native_row_id: &str,
        local_value: &str,
        mapping_strategy: &str,
        classification: &str,
        difference_explanation: &str,
        proposed_action: &str,
    ) {
        self.facts.push(BackfillFact {
            canonical_record_id: record_id.to_string(),
            canonical_field: field.to_string(),
            current_value: current_value.to_string(),
            local_table: local_table.to_string(),
            local_native_row_id: local_native_row_id.to_string(),
            local_value: local_value.to_string(),
            mapping_strategy: mapping_strategy.to_string(),
            classification: classification.to_string(),
            difference_explanation: difference_explanation.to_string(),
            proposed_action: proposed_action.to_string(),
            review_status: "reviewed".to_string(),
            stable_hash: stable_hash(&format!("{record_id}:{field}:{local_native_row_id}")),
        });
    }

    fn push_unmapped(&mut self, record_id: &str, field: &str, explanation: &str) {
        self.push(
            record_id,
            field,
            "",
            "",
            "",
            "",
            "native_row_id, stable internal ID, reviewed alias, localization key, structural relation",
            "not_represented_locally",
            explanation,
            "retain the reviewed canonical record and original provenance",
        );
    }
}

fn work_native_kind(kind: WorkKind) -> &'static str {
    match kind {
        WorkKind::Kindling => "EmitFlame",
        WorkKind::Watering => "Watering",
        WorkKind::Planting => "Seeding",
        WorkKind::GeneratingElectricity => "GenerateElectricity",
        WorkKind::Handiwork => "Handcraft",
        WorkKind::Gathering => "Collection",
        WorkKind::Lumbering => "Deforest",
        WorkKind::Mining => "Mining",
        WorkKind::MedicineProduction => "ProductMedicine",
        WorkKind::Transporting => "Transport",
        WorkKind::Farming => "MonsterFarm",
        WorkKind::Cooling => "Cool",
    }
}

fn stable_hash(value: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}
