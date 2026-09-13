use crate::{AnswerStatus, GuideEngine};
use game_knowledge::{ConflictRecord, ConflictResolution, ItemRecord, Provenance, RecipeRecord};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InventoryEntry {
    pub item: String,
    pub quantity: u32,
}

impl InventoryEntry {
    pub fn new(item: impl Into<String>, quantity: u32) -> Self {
        Self {
            item: item.into(),
            quantity,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaterialTotal {
    pub item_id: String,
    pub item_name: String,
    pub required_quantity: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ByproductTotal {
    pub item_id: String,
    pub item_name: String,
    pub quantity: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MaterialNode {
    pub item_id: String,
    pub item_name: String,
    pub required_quantity: u32,
    pub acquisition: MaterialAcquisition,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MaterialAcquisition {
    Raw,
    Recipe {
        recipe_id: String,
        output_quantity: u32,
        batches: u32,
        children: Vec<MaterialNode>,
        byproducts: Vec<ByproductTotal>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MaterialCalculation {
    pub target_id: String,
    pub target_name: String,
    pub requested_quantity: u32,
    pub recipe_id: Option<String>,
    pub tree: MaterialNode,
    pub totals: Vec<MaterialTotal>,
    pub byproducts: Vec<ByproductTotal>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InventoryAmount {
    pub item_id: String,
    pub item_name: String,
    pub available_quantity: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShortageMaterial {
    pub item_id: String,
    pub item_name: String,
    pub required_quantity: u32,
    pub available_quantity: u32,
    pub missing_quantity: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShortageCalculation {
    pub material_calculation: MaterialCalculation,
    pub inventory: Vec<InventoryAmount>,
    pub shortages: Vec<ShortageMaterial>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CraftableCalculation {
    pub target_id: String,
    pub recipe_id: Option<String>,
    pub maximum_additional_count: u32,
    pub limiting_material_ids: Vec<String>,
    pub material_calculation: MaterialCalculation,
}

#[derive(Debug, Clone)]
enum CalculationError {
    Cycle { path: Vec<String> },
    DepthLimit { item_id: String, limit: usize },
    QuantityOverflow,
}

#[derive(Debug, Clone)]
struct Expansion {
    node: MaterialNode,
    totals: Vec<MaterialTotal>,
    byproducts: Vec<ByproductTotal>,
    provenances: Vec<Provenance>,
    subject_ids: BTreeSet<String>,
    uncertainty: Vec<String>,
}

#[derive(Debug, Clone)]
struct ShortageAccumulator {
    item_id: String,
    item_name: String,
    required_quantity: u32,
    initial_available_quantity: u32,
    missing_quantity: u32,
}

impl GuideEngine {
    pub fn with_calculation_depth(mut self, limit: usize) -> Self {
        self.calculation_depth_limit = limit;
        self
    }

    pub fn calculate_materials(
        &self,
        item_query: &str,
        requested_quantity: u32,
    ) -> crate::GuideAnswer<MaterialCalculation> {
        self.calculate_materials_filtered(item_query, requested_quantity, None)
    }

    pub fn calculate_materials_filtered(
        &self,
        item_query: &str,
        requested_quantity: u32,
        rarity: Option<&str>,
    ) -> crate::GuideAnswer<MaterialCalculation> {
        if requested_quantity == 0 {
            return self.context().error("quantity must be greater than zero");
        }
        let resolved =
            match self.resolve_with_rarity(item_query, Some(crate::EntityKind::Item), rarity) {
                crate::resolver::Resolution::Unique(resolved) => resolved,
                crate::resolver::Resolution::Ambiguous(candidates) => {
                    return self
                        .context()
                        .ambiguous(self.ambiguous_message("item", &candidates));
                }
                crate::resolver::Resolution::Unknown => {
                    return self
                        .context()
                        .unknown("unknown item; no reviewed record matches")
                }
            };

        let expansion =
            match self.expand_materials(&resolved.id, requested_quantity, &mut Vec::new(), 0) {
                Ok(expansion) => expansion,
                Err(CalculationError::Cycle { path }) => {
                    return self
                        .context()
                        .error(format!("recipe cycle detected: {}", path.join(" -> ")))
                }
                Err(CalculationError::DepthLimit { item_id, limit }) => {
                    return self
                        .context()
                        .error(format!("recipe depth limit {limit} exceeded at {item_id}"))
                }
                Err(CalculationError::QuantityOverflow) => {
                    return self.context().error("quantity calculation overflowed u32")
                }
            };

        let recipe_id = match &expansion.node.acquisition {
            MaterialAcquisition::Recipe { recipe_id, .. } => Some(recipe_id.clone()),
            MaterialAcquisition::Raw => None,
        };
        let provenance_references = expansion.provenances.iter().collect::<Vec<_>>();
        let calculation_uncertainty = expansion.uncertainty.clone();
        let calculation = MaterialCalculation {
            target_id: resolved.id.clone(),
            target_name: resolved.matched_name.clone(),
            requested_quantity,
            recipe_id,
            tree: expansion.node,
            totals: expansion.totals,
            byproducts: expansion.byproducts,
        };
        let conflicts = expansion
            .subject_ids
            .iter()
            .flat_map(|subject_id| self.store().conflicts_for_subject(subject_id))
            .filter(|conflict| conflict.resolution == ConflictResolution::Unresolved)
            .collect::<Vec<&ConflictRecord>>();
        let status = if conflicts.is_empty() {
            AnswerStatus::Ok
        } else {
            AnswerStatus::Ambiguous
        };
        let mut answer =
            self.context()
                .answer(status, Some(calculation), provenance_references, conflicts);
        for message in calculation_uncertainty {
            if !answer.uncertainty.contains(&message) {
                answer.uncertainty.push(message);
            }
        }
        answer
    }

    pub fn calculate_shortage(
        &self,
        item_query: &str,
        requested_quantity: u32,
        inventory: &[InventoryEntry],
    ) -> crate::GuideAnswer<ShortageCalculation> {
        self.calculate_shortage_filtered(item_query, requested_quantity, inventory, None)
    }

    pub fn calculate_shortage_filtered(
        &self,
        item_query: &str,
        requested_quantity: u32,
        inventory: &[InventoryEntry],
        rarity: Option<&str>,
    ) -> crate::GuideAnswer<ShortageCalculation> {
        let parsed_inventory = match self.parse_inventory(inventory) {
            Ok(parsed_inventory) => parsed_inventory,
            Err(errors) => {
                let mut answer = self.context().error("invalid inventory");
                answer.errors.extend(errors);
                return answer;
            }
        };
        let mut material_answer =
            self.calculate_materials_filtered(item_query, requested_quantity, rarity);
        if material_answer.status != AnswerStatus::Ok {
            return material_answer.map_data(|_| None);
        }
        let material_calculation = material_answer
            .data
            .take()
            .expect("ok material calculation has data");
        let mut remaining_inventory = parsed_inventory.clone();
        let mut shortage_totals = BTreeMap::new();
        if let Err(CalculationError::QuantityOverflow) = self.collect_shortages(
            &material_calculation.tree,
            true,
            &mut remaining_inventory,
            &mut shortage_totals,
        ) {
            return self.context().error("shortage calculation overflowed u32");
        }
        let shortages = shortage_totals
            .into_values()
            .filter(|total| total.missing_quantity > 0)
            .map(|total| ShortageMaterial {
                item_id: total.item_id,
                item_name: total.item_name,
                required_quantity: total.required_quantity,
                available_quantity: total.initial_available_quantity,
                missing_quantity: total.missing_quantity,
            })
            .collect();
        let inventory = parsed_inventory
            .iter()
            .map(|(item_id, available_quantity)| InventoryAmount {
                item_id: item_id.clone(),
                item_name: self
                    .store()
                    .item(item_id)
                    .expect("validated inventory item exists")
                    .names
                    .en
                    .clone(),
                available_quantity: *available_quantity,
            })
            .collect();
        let shortage = ShortageCalculation {
            material_calculation,
            inventory,
            shortages,
        };
        material_answer.map_data(|_| Some(shortage))
    }

    pub fn calculate_craftable_count(
        &self,
        item_query: &str,
        inventory: &[InventoryEntry],
    ) -> crate::GuideAnswer<CraftableCalculation> {
        self.calculate_craftable_count_filtered(item_query, inventory, None)
    }

    pub fn calculate_craftable_count_filtered(
        &self,
        item_query: &str,
        inventory: &[InventoryEntry],
        rarity: Option<&str>,
    ) -> crate::GuideAnswer<CraftableCalculation> {
        let parsed_inventory = match self.parse_inventory(inventory) {
            Ok(parsed_inventory) => parsed_inventory,
            Err(errors) => {
                let mut answer = self.context().error("invalid inventory");
                answer.errors.extend(errors);
                return answer;
            }
        };
        let mut material_answer = self.calculate_materials_filtered(item_query, 1, rarity);
        if material_answer.status != AnswerStatus::Ok {
            return material_answer.map_data(|_| None);
        }
        let material_calculation = material_answer
            .data
            .take()
            .expect("ok material calculation has data");
        if material_calculation.recipe_id.is_none() {
            return self
                .context()
                .unknown("unknown crafting recipe; raw acquisition is not reported as craftable");
        }
        let mut maximum_additional_count = 0_u32;
        let mut high = u32::MAX;
        while maximum_additional_count < high {
            let mut candidate = maximum_additional_count + (high - maximum_additional_count) / 2;
            if candidate == maximum_additional_count {
                candidate = high;
            }
            let mut inventory = parsed_inventory.clone();
            if self.can_fulfill_material(
                &material_calculation.tree,
                candidate,
                &mut inventory,
                false,
            ) {
                maximum_additional_count = candidate;
            } else {
                high = candidate - 1;
            }
        }

        if maximum_additional_count > 0 {
            let mut inventory = parsed_inventory.clone();
            let fulfilled = self.can_fulfill_material(
                &material_calculation.tree,
                maximum_additional_count,
                &mut inventory,
                false,
            );
            debug_assert!(fulfilled, "maximum craftable count must be fulfillable");
            if apply_byproducts_to_inventory(
                &material_calculation.tree.item_id,
                &material_calculation.tree.acquisition,
                1,
                0,
                &mut inventory,
            )
            .is_ok()
                && self.fulfill_recipe_batches(
                    &material_calculation.tree.acquisition,
                    1,
                    &mut inventory,
                )
            {
                return self
                    .context()
                    .error("craftable count overflow; no result was calculated");
            }
        }

        let limiting_material_ids = if maximum_additional_count < u32::MAX {
            maximum_additional_count
                .checked_add(1)
                .and_then(|boundary_quantity| {
                    let boundary_node = MaterialNode {
                        required_quantity: boundary_quantity,
                        ..material_calculation.tree.clone()
                    };
                    let mut inventory = parsed_inventory.clone();
                    let mut shortages = BTreeMap::new();
                    self.collect_shortages(&boundary_node, false, &mut inventory, &mut shortages)
                        .ok()?;
                    Some(
                        shortages
                            .into_values()
                            .filter(|total| total.missing_quantity > 0)
                            .map(|total| total.item_id)
                            .collect::<Vec<_>>(),
                    )
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        let craftable = CraftableCalculation {
            target_id: material_calculation.target_id.clone(),
            recipe_id: material_calculation.recipe_id.clone(),
            maximum_additional_count,
            limiting_material_ids,
            material_calculation,
        };
        material_answer.map_data(|_| Some(craftable))
    }

    fn collect_shortages(
        &self,
        node: &MaterialNode,
        consume_inventory: bool,
        inventory: &mut BTreeMap<String, u32>,
        shortages: &mut BTreeMap<String, ShortageAccumulator>,
    ) -> Result<(), CalculationError> {
        let available_before = if consume_inventory {
            inventory.get(&node.item_id).copied().unwrap_or(0)
        } else {
            0
        };
        let consumed = available_before.min(node.required_quantity);
        let missing_quantity = node.required_quantity - consumed;
        if consume_inventory {
            if let Some(quantity) = inventory.get_mut(&node.item_id) {
                *quantity -= consumed;
            }
        }

        match &node.acquisition {
            MaterialAcquisition::Raw => {
                let total =
                    shortages
                        .entry(node.item_id.clone())
                        .or_insert_with(|| ShortageAccumulator {
                            item_id: node.item_id.clone(),
                            item_name: node.item_name.clone(),
                            required_quantity: 0,
                            initial_available_quantity: available_before,
                            missing_quantity: 0,
                        });
                total.required_quantity = total
                    .required_quantity
                    .checked_add(node.required_quantity)
                    .ok_or(CalculationError::QuantityOverflow)?;
                total.missing_quantity = total
                    .missing_quantity
                    .checked_add(missing_quantity)
                    .ok_or(CalculationError::QuantityOverflow)?;
            }
            MaterialAcquisition::Recipe {
                output_quantity,
                batches,
                children,
                ..
            } => {
                if missing_quantity == 0 {
                    return Ok(());
                }
                let self_byproduct_per_batch =
                    byproducts_for_batches(&node.acquisition, &node.item_id, 1)?
                        .first()
                        .map(|(_, quantity)| *quantity)
                        .unwrap_or(0);
                let effective_output_quantity = output_quantity
                    .checked_add(self_byproduct_per_batch)
                    .ok_or(CalculationError::QuantityOverflow)?;
                let required_batches = batches_for(missing_quantity, effective_output_quantity)?;
                for &child_index in ordered_child_indices(children).iter() {
                    let child = &children[child_index];
                    let child_quantity = recipe_child_quantity(
                        child,
                        &node.acquisition,
                        *batches,
                        required_batches,
                    )?;
                    self.collect_shortages(
                        &(MaterialNode {
                            required_quantity: child_quantity,
                            ..child.clone()
                        }),
                        true,
                        inventory,
                        shortages,
                    )?;
                }
                apply_byproducts_to_inventory(
                    &node.item_id,
                    &node.acquisition,
                    required_batches,
                    missing_quantity,
                    inventory,
                )?;
            }
        }
        Ok(())
    }

    fn can_fulfill_material(
        &self,
        node: &MaterialNode,
        required_quantity: u32,
        inventory: &mut BTreeMap<String, u32>,
        consume_inventory: bool,
    ) -> bool {
        let available_before = if consume_inventory {
            inventory.get(&node.item_id).copied().unwrap_or(0)
        } else {
            0
        };
        let consumed = available_before.min(required_quantity);
        let remaining_quantity = required_quantity - consumed;
        if consume_inventory {
            if let Some(quantity) = inventory.get_mut(&node.item_id) {
                *quantity -= consumed;
            }
        }
        if remaining_quantity == 0 {
            return true;
        }

        match &node.acquisition {
            MaterialAcquisition::Raw => false,
            MaterialAcquisition::Recipe {
                output_quantity, ..
            } => {
                let Ok(byproducts) = byproducts_for_batches(&node.acquisition, &node.item_id, 1)
                else {
                    return false;
                };
                let self_byproduct_per_batch = byproducts
                    .first()
                    .map(|(_, quantity)| *quantity)
                    .unwrap_or(0);
                let Some(effective_output_quantity) =
                    output_quantity.checked_add(self_byproduct_per_batch)
                else {
                    return false;
                };
                let Some(required_batches) =
                    batches_for(remaining_quantity, effective_output_quantity).ok()
                else {
                    return false;
                };
                if !self.fulfill_recipe_batches(&node.acquisition, required_batches, inventory) {
                    return false;
                }
                apply_byproducts_to_inventory(
                    &node.item_id,
                    &node.acquisition,
                    required_batches,
                    remaining_quantity,
                    inventory,
                )
                .is_ok()
            }
        }
    }

    fn fulfill_recipe_batches(
        &self,
        acquisition: &MaterialAcquisition,
        required_batches: u32,
        inventory: &mut BTreeMap<String, u32>,
    ) -> bool {
        let MaterialAcquisition::Recipe {
            batches, children, ..
        } = acquisition
        else {
            return false;
        };
        ordered_child_indices(children).iter().all(|&child_index| {
            let child = &children[child_index];
            let Some(child_quantity) =
                recipe_child_quantity(child, acquisition, *batches, required_batches).ok()
            else {
                return false;
            };
            self.can_fulfill_material(
                &(MaterialNode {
                    required_quantity: child_quantity,
                    ..child.clone()
                }),
                child_quantity,
                inventory,
                true,
            )
        })
    }

    fn parse_inventory(
        &self,
        inventory: &[InventoryEntry],
    ) -> Result<BTreeMap<String, u32>, Vec<String>> {
        let mut parsed = BTreeMap::new();
        let mut errors = Vec::new();
        for entry in inventory {
            if entry.quantity == 0 {
                errors.push(format!(
                    "{}: quantity must be greater than zero",
                    entry.item
                ));
                continue;
            }
            match self.resolve(&entry.item, Some(crate::EntityKind::Item)) {
                crate::resolver::Resolution::Unique(resolved) => {
                    let current: u32 = parsed.get(&resolved.id).copied().unwrap_or(0);
                    match current.checked_add(entry.quantity) {
                        Some(quantity) => {
                            parsed.insert(resolved.id, quantity);
                        }
                        None => errors.push(format!("{}: inventory total overflow", entry.item)),
                    }
                }
                crate::resolver::Resolution::Ambiguous(_) => {
                    errors.push(format!("{}: ambiguous inventory item", entry.item));
                }
                crate::resolver::Resolution::Unknown => {
                    errors.push(format!("{}: unknown inventory item", entry.item));
                }
            }
        }
        if errors.is_empty() {
            Ok(parsed)
        } else {
            Err(errors)
        }
    }

    fn expand_materials(
        &self,
        item_id: &str,
        required_quantity: u32,
        path: &mut Vec<String>,
        depth: usize,
    ) -> Result<Expansion, CalculationError> {
        if depth > self.calculation_depth_limit {
            return Err(CalculationError::DepthLimit {
                item_id: item_id.to_string(),
                limit: self.calculation_depth_limit,
            });
        }
        if path.iter().any(|visited| visited == item_id) {
            let mut cycle = path.clone();
            cycle.push(item_id.to_string());
            return Err(CalculationError::Cycle { path: cycle });
        }
        path.push(item_id.to_string());

        let item = self
            .store()
            .item(item_id)
            .expect("material item reference is validated");
        let recipes = self
            .store()
            .recipes()
            .filter(|recipe| recipe.output.item_id == item_id)
            .collect::<Vec<&RecipeRecord>>();
        let expansion = if recipes.is_empty() {
            raw_expansion(item, required_quantity)
        } else if recipes.len() == 1 {
            self.expand_recipe(item, required_quantity, recipes[0], path, depth)?
        } else {
            let mut expansion = raw_expansion(item, required_quantity);
            expansion.uncertainty.push(format!(
                "{} has multiple reviewed recipes ({}); it was treated as a raw material in this tree",
                item.names.en,
                recipes.iter().map(|recipe| recipe.id.as_str()).collect::<Vec<_>>().join(", ")
            ));
            expansion
        };
        path.pop();
        Ok(expansion)
    }

    fn expand_recipe(
        &self,
        item: &ItemRecord,
        required_quantity: u32,
        recipe: &RecipeRecord,
        path: &mut Vec<String>,
        depth: usize,
    ) -> Result<Expansion, CalculationError> {
        let batches = batches_for(required_quantity, recipe.output.quantity)?;
        let mut children = Vec::new();
        let mut totals = BTreeMap::new();
        let mut byproducts = BTreeMap::new();
        let mut node_byproducts = BTreeMap::new();
        let mut provenances = vec![item.provenance.clone(), recipe.provenance.clone()];
        let mut subject_ids = BTreeSet::new();
        let mut uncertainty = Vec::new();
        subject_ids.insert(item.id.clone());
        subject_ids.insert(recipe.id.clone());

        for ingredient in &recipe.ingredients {
            let ingredient_quantity = ingredient
                .quantity
                .checked_mul(batches)
                .ok_or(CalculationError::QuantityOverflow)?;
            let expansion =
                self.expand_materials(&ingredient.item_id, ingredient_quantity, path, depth + 1)?;
            children.push(expansion.node);
            provenances.extend(expansion.provenances);
            subject_ids.extend(expansion.subject_ids);
            uncertainty.extend(expansion.uncertainty);
            merge_totals(&mut totals, expansion.totals)?;
            merge_byproducts(&mut byproducts, expansion.byproducts)?;
        }
        for byproduct in &recipe.byproducts {
            let quantity = byproduct
                .quantity
                .checked_mul(batches)
                .ok_or(CalculationError::QuantityOverflow)?;
            let byproduct_item = self
                .store()
                .item(&byproduct.item_id)
                .expect("byproduct item reference is validated");
            provenances.push(byproduct_item.provenance.clone());
            subject_ids.insert(byproduct_item.id.clone());
            let total = byproducts
                .entry(byproduct.item_id.clone())
                .or_insert_with(|| ByproductTotal {
                    item_id: byproduct.item_id.clone(),
                    item_name: byproduct_item.names.en.clone(),
                    quantity: 0,
                });
            total.quantity = total
                .quantity
                .checked_add(quantity)
                .ok_or(CalculationError::QuantityOverflow)?;
            let node_total = node_byproducts
                .entry(byproduct.item_id.clone())
                .or_insert_with(|| ByproductTotal {
                    item_id: byproduct.item_id.clone(),
                    item_name: byproduct_item.names.en.clone(),
                    quantity: 0,
                });
            node_total.quantity = node_total
                .quantity
                .checked_add(quantity)
                .ok_or(CalculationError::QuantityOverflow)?;
        }

        let all_byproducts = byproducts.into_values().collect::<Vec<_>>();
        let node_byproducts = node_byproducts.into_values().collect::<Vec<_>>();
        let node = MaterialNode {
            item_id: item.id.clone(),
            item_name: item.names.en.clone(),
            required_quantity,
            acquisition: MaterialAcquisition::Recipe {
                recipe_id: recipe.id.clone(),
                output_quantity: recipe.output.quantity,
                batches,
                children,
                byproducts: node_byproducts,
            },
        };
        Ok(Expansion {
            node,
            totals: totals.into_values().collect(),
            byproducts: all_byproducts,
            provenances,
            subject_ids,
            uncertainty,
        })
    }
}

fn batches_for(required_quantity: u32, output_quantity: u32) -> Result<u32, CalculationError> {
    let whole_batches = required_quantity / output_quantity;
    if required_quantity.is_multiple_of(output_quantity) {
        Ok(whole_batches)
    } else {
        whole_batches
            .checked_add(1)
            .ok_or(CalculationError::QuantityOverflow)
    }
}

fn scaled_child_quantity(
    child: &MaterialNode,
    original_batches: u32,
    required_batches: u32,
) -> Result<u32, CalculationError> {
    if original_batches == 0 {
        return Err(CalculationError::QuantityOverflow);
    }
    let quantity_per_batch = child.required_quantity / original_batches;
    if quantity_per_batch
        .checked_mul(original_batches)
        .is_none_or(|quantity| quantity != child.required_quantity)
    {
        return Err(CalculationError::QuantityOverflow);
    }
    quantity_per_batch
        .checked_mul(required_batches)
        .ok_or(CalculationError::QuantityOverflow)
}

fn recipe_child_quantity(
    child: &MaterialNode,
    acquisition: &MaterialAcquisition,
    original_batches: u32,
    required_batches: u32,
) -> Result<u32, CalculationError> {
    let gross_quantity = scaled_child_quantity(child, original_batches, required_batches)?;
    let MaterialAcquisition::Recipe {
        batches,
        byproducts,
        ..
    } = acquisition
    else {
        return Ok(gross_quantity);
    };
    let Some(byproduct) = byproducts
        .iter()
        .find(|byproduct| byproduct.item_id == child.item_id)
    else {
        return Ok(gross_quantity);
    };
    let ingredient_per_batch = child.required_quantity / original_batches;
    let byproduct_per_batch = byproduct.quantity / *batches;
    if byproduct_per_batch
        .checked_mul(*batches)
        .is_none_or(|quantity| quantity != byproduct.quantity)
    {
        return Err(CalculationError::QuantityOverflow);
    }
    let reuse_per_batch = ingredient_per_batch.saturating_sub(byproduct_per_batch);
    ingredient_per_batch
        .checked_add(
            required_batches
                .saturating_sub(1)
                .checked_mul(reuse_per_batch)
                .ok_or(CalculationError::QuantityOverflow)?,
        )
        .ok_or(CalculationError::QuantityOverflow)
}

fn subtree_byproduct_items(node: &MaterialNode) -> BTreeSet<String> {
    let mut items = BTreeSet::new();
    if let MaterialAcquisition::Recipe {
        byproducts,
        children,
        ..
    } = &node.acquisition
    {
        for byproduct in byproducts {
            items.insert(byproduct.item_id.clone());
        }
        for child in children {
            items.extend(subtree_byproduct_items(child));
        }
    }
    items
}

fn subtree_consumed_items(node: &MaterialNode) -> BTreeSet<String> {
    let mut items = BTreeSet::new();
    if let MaterialAcquisition::Recipe { children, .. } = &node.acquisition {
        for child in children {
            items.insert(child.item_id.clone());
            items.extend(subtree_consumed_items(child));
        }
    }
    items
}

fn ordered_child_indices(children: &[MaterialNode]) -> Vec<usize> {
    let produced = children
        .iter()
        .map(subtree_byproduct_items)
        .collect::<Vec<_>>();
    let consumed = children
        .iter()
        .map(subtree_consumed_items)
        .collect::<Vec<_>>();
    let count = children.len();
    let mut adjacency = vec![Vec::new(); count];
    let mut indegree = vec![0_usize; count];
    for producer in 0..count {
        for consumer in 0..count {
            if producer == consumer {
                continue;
            }
            let supplies = produced[producer].contains(&children[consumer].item_id)
                || produced[producer]
                    .intersection(&consumed[consumer])
                    .next()
                    .is_some();
            if supplies {
                adjacency[producer].push(consumer);
                indegree[consumer] += 1;
            }
        }
    }
    let mut remaining = (0..count).collect::<BTreeSet<_>>();
    let mut order = Vec::with_capacity(count);
    while !remaining.is_empty() {
        let next = remaining
            .iter()
            .copied()
            .find(|&index| indegree[index] == 0)
            .unwrap_or_else(|| *remaining.iter().next().expect("remaining is not empty"));
        remaining.remove(&next);
        order.push(next);
        for &successor in &adjacency[next] {
            indegree[successor] = indegree[successor].saturating_sub(1);
        }
    }
    order
}

fn byproducts_for_batches(
    acquisition: &MaterialAcquisition,
    item_id: &str,
    required_batches: u32,
) -> Result<Vec<(String, u32)>, CalculationError> {
    let MaterialAcquisition::Recipe {
        batches,
        byproducts,
        ..
    } = acquisition
    else {
        return Ok(Vec::new());
    };
    if *batches == 0 {
        return Err(CalculationError::QuantityOverflow);
    }
    byproducts
        .iter()
        .filter(|byproduct| byproduct.item_id == item_id)
        .map(|byproduct| {
            let quantity_per_batch = byproduct.quantity / *batches;
            if quantity_per_batch
                .checked_mul(*batches)
                .is_none_or(|quantity| quantity != byproduct.quantity)
            {
                return Err(CalculationError::QuantityOverflow);
            }
            Ok((
                byproduct.item_id.clone(),
                quantity_per_batch
                    .checked_mul(required_batches)
                    .ok_or(CalculationError::QuantityOverflow)?,
            ))
        })
        .collect()
}

fn byproduct_net_availability(
    acquisition: &MaterialAcquisition,
    required_batches: u32,
) -> Result<Vec<(String, u32)>, CalculationError> {
    let MaterialAcquisition::Recipe {
        batches,
        byproducts,
        children,
        ..
    } = acquisition
    else {
        return Ok(Vec::new());
    };
    if *batches == 0 {
        return Err(CalculationError::QuantityOverflow);
    }
    byproducts
        .iter()
        .map(|byproduct| {
            let byproduct_per_batch = byproduct.quantity / *batches;
            if byproduct_per_batch
                .checked_mul(*batches)
                .is_none_or(|quantity| quantity != byproduct.quantity)
            {
                return Err(CalculationError::QuantityOverflow);
            }
            let ingredient_per_batch = children
                .iter()
                .find(|child| child.item_id == byproduct.item_id)
                .map(|child| child.required_quantity / *batches)
                .unwrap_or(0);
            let seed_quantity = if ingredient_per_batch == 0 {
                0
            } else {
                ingredient_per_batch
                    .checked_add(
                        required_batches
                            .saturating_sub(1)
                            .checked_mul(ingredient_per_batch.saturating_sub(byproduct_per_batch))
                            .ok_or(CalculationError::QuantityOverflow)?,
                    )
                    .ok_or(CalculationError::QuantityOverflow)?
            };
            let internally_consumed = ingredient_per_batch
                .checked_mul(required_batches)
                .ok_or(CalculationError::QuantityOverflow)?
                .saturating_sub(seed_quantity);
            let available_quantity = byproduct_per_batch
                .checked_mul(required_batches)
                .ok_or(CalculationError::QuantityOverflow)?
                .saturating_sub(internally_consumed);
            Ok((byproduct.item_id.clone(), available_quantity))
        })
        .collect()
}

fn apply_byproducts_to_inventory(
    item_id: &str,
    acquisition: &MaterialAcquisition,
    required_batches: u32,
    missing_same_item: u32,
    inventory: &mut BTreeMap<String, u32>,
) -> Result<(), CalculationError> {
    let MaterialAcquisition::Recipe {
        output_quantity, ..
    } = acquisition
    else {
        return Ok(());
    };
    let availability = byproduct_net_availability(acquisition, required_batches)?;
    let self_byproduct_quantity = availability
        .iter()
        .find(|(byproduct_item_id, _)| byproduct_item_id == item_id)
        .map(|(_, quantity)| *quantity)
        .unwrap_or(0);
    let self_produced_quantity = output_quantity
        .checked_mul(required_batches)
        .ok_or(CalculationError::QuantityOverflow)?
        .checked_add(self_byproduct_quantity)
        .ok_or(CalculationError::QuantityOverflow)?;
    let self_surplus = self_produced_quantity.saturating_sub(missing_same_item);
    if self_surplus > 0 {
        let quantity = inventory.entry(item_id.to_string()).or_insert(0);
        *quantity = quantity
            .checked_add(self_surplus)
            .ok_or(CalculationError::QuantityOverflow)?;
    }
    for (byproduct_item_id, quantity) in availability {
        if byproduct_item_id == item_id {
            continue;
        }
        let total = inventory.entry(byproduct_item_id).or_insert(0);
        *total = total
            .checked_add(quantity)
            .ok_or(CalculationError::QuantityOverflow)?;
    }
    Ok(())
}

fn raw_expansion(item: &ItemRecord, required_quantity: u32) -> Expansion {
    Expansion {
        node: MaterialNode {
            item_id: item.id.clone(),
            item_name: item.names.en.clone(),
            required_quantity,
            acquisition: MaterialAcquisition::Raw,
        },
        totals: vec![MaterialTotal {
            item_id: item.id.clone(),
            item_name: item.names.en.clone(),
            required_quantity,
        }],
        byproducts: Vec::new(),
        provenances: vec![item.provenance.clone()],
        subject_ids: BTreeSet::from([item.id.clone()]),
        uncertainty: Vec::new(),
    }
}

fn merge_totals(
    target: &mut BTreeMap<String, MaterialTotal>,
    additions: Vec<MaterialTotal>,
) -> Result<(), CalculationError> {
    for addition in additions {
        let total = target
            .entry(addition.item_id.clone())
            .or_insert_with(|| MaterialTotal {
                item_id: addition.item_id.clone(),
                item_name: addition.item_name.clone(),
                required_quantity: 0,
            });
        total.required_quantity = total
            .required_quantity
            .checked_add(addition.required_quantity)
            .ok_or(CalculationError::QuantityOverflow)?;
    }
    Ok(())
}

fn merge_byproducts(
    target: &mut BTreeMap<String, ByproductTotal>,
    additions: Vec<ByproductTotal>,
) -> Result<(), CalculationError> {
    for addition in additions {
        let total = target
            .entry(addition.item_id.clone())
            .or_insert_with(|| ByproductTotal {
                item_id: addition.item_id.clone(),
                item_name: addition.item_name.clone(),
                quantity: 0,
            });
        total.quantity = total
            .quantity
            .checked_add(addition.quantity)
            .ok_or(CalculationError::QuantityOverflow)?;
    }
    Ok(())
}
