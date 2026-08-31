use crate::{AnswerStatus, GuideEngine};
use game_knowledge::{ConflictRecord, ItemRecord, Provenance, RecipeRecord};
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
    AmbiguousRecipe {
        item_id: String,
        recipe_ids: Vec<String>,
    },
    Cycle {
        path: Vec<String>,
    },
    DepthLimit {
        item_id: String,
        limit: usize,
    },
    QuantityOverflow,
}

#[derive(Debug, Clone)]
struct Expansion {
    node: MaterialNode,
    totals: Vec<MaterialTotal>,
    byproducts: Vec<ByproductTotal>,
    provenances: Vec<Provenance>,
    subject_ids: BTreeSet<String>,
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
        if requested_quantity == 0 {
            return self.context().error("quantity must be greater than zero");
        }
        let resolved = match self.resolve(item_query, Some(crate::EntityKind::Item)) {
            crate::resolver::Resolution::Unique(resolved) => resolved,
            crate::resolver::Resolution::Ambiguous(candidates) => {
                let ids = candidate_ids(&candidates);
                return self
                    .context()
                    .ambiguous(format!("ambiguous item name; candidates: {ids}"));
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
                Err(CalculationError::AmbiguousRecipe {
                    item_id,
                    recipe_ids,
                }) => {
                    return self.context().ambiguous(format!(
                        "ambiguous recipe for {item_id}; candidates: {}; no recipe was selected",
                        recipe_ids.join(", ")
                    ))
                }
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
            .collect::<Vec<&ConflictRecord>>();
        let status = if conflicts.is_empty() {
            AnswerStatus::Ok
        } else {
            AnswerStatus::Ambiguous
        };
        self.context()
            .answer(status, Some(calculation), provenance_references, conflicts)
    }

    pub fn calculate_shortage(
        &self,
        item_query: &str,
        requested_quantity: u32,
        inventory: &[InventoryEntry],
    ) -> crate::GuideAnswer<ShortageCalculation> {
        let parsed_inventory = match self.parse_inventory(inventory) {
            Ok(parsed_inventory) => parsed_inventory,
            Err(errors) => {
                let mut answer = self.context().error("invalid inventory");
                answer.errors.extend(errors);
                return answer;
            }
        };
        self.calculate_materials(item_query, requested_quantity)
            .map_data(|data| {
                data.map(|material_calculation| {
                    let shortages = material_calculation
                        .totals
                        .iter()
                        .filter_map(|total| {
                            let available_quantity =
                                parsed_inventory.get(&total.item_id).copied().unwrap_or(0);
                            (available_quantity < total.required_quantity).then(|| {
                                ShortageMaterial {
                                    item_id: total.item_id.clone(),
                                    item_name: total.item_name.clone(),
                                    required_quantity: total.required_quantity,
                                    available_quantity,
                                    missing_quantity: total.required_quantity - available_quantity,
                                }
                            })
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
                    ShortageCalculation {
                        material_calculation,
                        inventory,
                        shortages,
                    }
                })
            })
    }

    pub fn calculate_craftable_count(
        &self,
        item_query: &str,
        inventory: &[InventoryEntry],
    ) -> crate::GuideAnswer<CraftableCalculation> {
        let parsed_inventory = match self.parse_inventory(inventory) {
            Ok(parsed_inventory) => parsed_inventory,
            Err(errors) => {
                let mut answer = self.context().error("invalid inventory");
                answer.errors.extend(errors);
                return answer;
            }
        };
        let mut material_answer = self.calculate_materials(item_query, 1);
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
        let craftable = {
            let mut maximum_additional_count = u32::MAX;
            for total in &material_calculation.totals {
                let available_quantity = parsed_inventory.get(&total.item_id).copied().unwrap_or(0);
                maximum_additional_count =
                    maximum_additional_count.min(available_quantity / total.required_quantity);
            }
            let limiting_material_ids = material_calculation
                .totals
                .iter()
                .filter(|total| {
                    let available_quantity =
                        parsed_inventory.get(&total.item_id).copied().unwrap_or(0);
                    available_quantity / total.required_quantity == maximum_additional_count
                })
                .map(|total| total.item_id.clone())
                .collect();
            CraftableCalculation {
                target_id: material_calculation.target_id.clone(),
                recipe_id: material_calculation.recipe_id.clone(),
                maximum_additional_count,
                limiting_material_ids,
                material_calculation,
            }
        };
        material_answer.map_data(|_| Some(craftable))
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
            return Err(CalculationError::AmbiguousRecipe {
                item_id: item.id.clone(),
                recipe_ids: recipes.iter().map(|recipe| recipe.id.clone()).collect(),
            });
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
        let mut provenances = vec![item.provenance.clone(), recipe.provenance.clone()];
        let mut subject_ids = BTreeSet::new();
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
            merge_totals(&mut totals, expansion.totals)?;
            merge_byproducts(&mut byproducts, expansion.byproducts)?;
        }
        for byproduct in &recipe.byproducts {
            let quantity = byproduct
                .quantity
                .checked_mul(batches)
                .ok_or(CalculationError::QuantityOverflow)?;
            let total = byproducts
                .entry(byproduct.item_id.clone())
                .or_insert_with(|| ByproductTotal {
                    item_id: byproduct.item_id.clone(),
                    item_name: self
                        .store()
                        .item(&byproduct.item_id)
                        .expect("byproduct item reference is validated")
                        .names
                        .en
                        .clone(),
                    quantity: 0,
                });
            total.quantity = total
                .quantity
                .checked_add(quantity)
                .ok_or(CalculationError::QuantityOverflow)?;
        }

        let all_byproducts = byproducts.into_values().collect::<Vec<_>>();
        let node = MaterialNode {
            item_id: item.id.clone(),
            item_name: item.names.en.clone(),
            required_quantity,
            acquisition: MaterialAcquisition::Recipe {
                recipe_id: recipe.id.clone(),
                output_quantity: recipe.output.quantity,
                batches,
                children,
                byproducts: all_byproducts.clone(),
            },
        };
        Ok(Expansion {
            node,
            totals: totals.into_values().collect(),
            byproducts: all_byproducts,
            provenances,
            subject_ids,
        })
    }
}

fn candidate_ids(candidates: &[crate::ResolvedEntity]) -> String {
    candidates
        .iter()
        .map(|candidate| candidate.id.as_str())
        .collect::<Vec<_>>()
        .join(", ")
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
