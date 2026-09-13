//! Deterministic state-aware advice and progression planning.

use chrono::{DateTime, Utc};
use game_knowledge::{ProgressionRelationKind, WorkKind};
use guide_core::{
    AnswerStatus, EntityKind, GuideAnswer, GuideEngine, InventoryEntry, ProvenanceSummary,
    Resolution, ShortageCalculation, VersionInfo,
};
use serde::{Deserialize, Serialize};
use state_snapshot::{
    PlayerStateSnapshot, SnapshotFreshness, SnapshotSummary, SnapshotSummaryOptions,
    SnapshotValidator,
};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlannerStatus {
    Ok,
    Unknown,
    Ambiguous,
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlannerAnswer<T> {
    pub status: PlannerStatus,
    pub data: Option<T>,
    pub provenance: Vec<ProvenanceSummary>,
    pub version: VersionInfo,
    pub uncertainty: Vec<String>,
    pub errors: Vec<String>,
}

impl<T> PlannerAnswer<T> {
    pub fn map_data<U>(self, transform: impl FnOnce(Option<T>) -> Option<U>) -> PlannerAnswer<U> {
        PlannerAnswer {
            status: self.status,
            data: transform(self.data),
            provenance: self.provenance,
            version: self.version,
            uncertainty: self.uncertainty,
            errors: self.errors,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GoalInventoryGap {
    pub target_id: String,
    pub target_name: String,
    pub requested_quantity: u32,
    pub shortage: ShortageCalculation,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct InventoryGapAnalysis {
    pub goals: Vec<GoalInventoryGap>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartyWorkMember {
    pub pal_id: String,
    pub pal_name: String,
    pub slot: u8,
    pub work_kinds: Vec<WorkKind>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartyWorkAnalysis {
    pub members: Vec<PartyWorkMember>,
    pub covered_kinds: Vec<WorkKind>,
    pub missing_kinds: Vec<WorkKind>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CraftableNowRecipe {
    pub target_id: String,
    pub target_name: String,
    pub recipe_id: Option<String>,
    pub maximum_additional_count: u32,
    pub limiting_material_ids: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CraftableNowAnalysis {
    pub recipes: Vec<CraftableNowRecipe>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoalReadiness {
    pub target_id: String,
    pub target_name: String,
    pub ready: bool,
    pub missing_requirements: Vec<String>,
    pub uncertainties: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GoalReadinessAnalysis {
    pub goals: Vec<GoalReadiness>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlannerAnalysis {
    pub state_summary: SnapshotSummary,
    pub inventory_gap: InventoryGapAnalysis,
    pub party_work: PartyWorkAnalysis,
    pub craftable_now: CraftableNowAnalysis,
    pub goal_readiness: GoalReadinessAnalysis,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RecommendationScore {
    pub relevance: u8,
    pub effort: u8,
    pub benefit: u8,
    pub risk: u8,
    pub uncertainty: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecommendationBasis {
    pub knowledge_record_ids: Vec<String>,
    pub state_fields: Vec<String>,
    pub evidence_kinds: Vec<String>,
    pub assumptions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannerRecommendation {
    pub id: String,
    pub action: String,
    pub reason: String,
    pub requirements: Vec<String>,
    pub alternatives: Vec<String>,
    pub expected_benefit: String,
    pub risk: String,
    pub uncertainty: String,
    pub score: RecommendationScore,
    pub basis: RecommendationBasis,
}

pub struct GuidePlanner {
    engine: GuideEngine,
}

impl GuidePlanner {
    pub fn new(engine: GuideEngine) -> Self {
        Self { engine }
    }

    pub fn analyze(
        &self,
        snapshot: &PlayerStateSnapshot,
        now: DateTime<Utc>,
    ) -> PlannerAnswer<PlannerAnalysis> {
        let validation = SnapshotValidator.validate(snapshot, now);
        if !validation.valid {
            return error_answer(validation.errors);
        }

        let inventory = snapshot_inventory(snapshot);
        let mut provenance = BTreeSet::new();
        let mut uncertainty = Vec::new();
        let mut version = VersionInfo {
            knowledge_version: "unknown".to_string(),
            configured_game_version: None,
            matches: true,
        };
        let mut inventory_goals = Vec::new();
        let mut readiness_goals = Vec::new();
        // Player technology unlocks cannot be read from the game yet: the UE4SS adapter
        // exposes no technology tool, and a snapshot only carries caller-declared names,
        // which are not treated as evidence here. Every technology is therefore assumed
        // locked, so gated goals keep reporting their unlock requirement instead of
        // claiming the player already owns it. Once unlocks can be observed, replace this
        // with a snapshot-driven set (see `unlocked_technology_ids` in git history).
        let unlocked = BTreeSet::<String>::new();

        if let Some(goals) = &snapshot.goals {
            for goal in goals {
                if goal.kind == "progression" {
                    let Resolution::Unique(entity) = self
                        .engine
                        .resolve(&goal.target, Some(EntityKind::Technology))
                    else {
                        continue;
                    };
                    let relationships = self
                        .engine
                        .store()
                        .progression_relationships()
                        .filter(|relationship| {
                            relationship.from_id == entity.id
                                && relationship.relation == ProgressionRelationKind::Unlocks
                        })
                        .collect::<Vec<_>>();
                    for relationship in &relationships {
                        provenance.insert(ProvenanceSummary::new(&relationship.provenance));
                    }
                    let is_unlocked = unlocked.contains(&entity.id);
                    let has_relationships = !relationships.is_empty();
                    let target_id = entity.id;
                    let target_name = entity.matched_name;
                    readiness_goals.push(GoalReadiness {
                        target_id,
                        target_name,
                        ready: is_unlocked && has_relationships,
                        missing_requirements: if is_unlocked && has_relationships {
                            Vec::new()
                        } else {
                            vec![format!("Unlock the reviewed {}", goal.target)]
                        },
                        uncertainties: if relationships.is_empty() {
                            vec![format!(
                                "no reviewed progression relationship for {}",
                                goal.target
                            )]
                        } else {
                            Vec::new()
                        },
                    });
                    continue;
                }
                let Resolution::Unique(entity) =
                    self.engine.resolve(&goal.target, Some(EntityKind::Item))
                else {
                    continue;
                };
                let shortage_answer =
                    self.engine
                        .calculate_shortage(&goal.target, goal.quantity, &inventory);
                merge_answer_metadata(
                    &shortage_answer,
                    &mut provenance,
                    &mut uncertainty,
                    &mut version,
                );
                let AnswerStatus::Ok = shortage_answer.status else {
                    continue;
                };
                let shortage = shortage_answer.data.expect("ok shortage has data");
                let mut missing_requirements = shortage
                    .shortages
                    .iter()
                    .map(|material| {
                        format!(
                            "Collect {} more {}",
                            material.missing_quantity, material.item_name
                        )
                    })
                    .collect::<Vec<_>>();
                if let Some(recipe_id) = &shortage.material_calculation.recipe_id {
                    let recipe = self
                        .engine
                        .store()
                        .recipe(recipe_id)
                        .expect("calculated recipe exists");
                    if let Some(technology_id) = &recipe.technology_id {
                        if !unlocked.contains(technology_id) {
                            let technology = self
                                .engine
                                .store()
                                .technology(technology_id)
                                .expect("referenced technology exists");
                            missing_requirements.push(format!(
                                "Unlock {} at level {}",
                                technology.names.en, technology.level
                            ));
                        }
                    }
                }
                inventory_goals.push(GoalInventoryGap {
                    target_id: entity.id.clone(),
                    target_name: entity.matched_name.clone(),
                    requested_quantity: goal.quantity,
                    shortage,
                });
                readiness_goals.push(GoalReadiness {
                    target_id: entity.id,
                    target_name: entity.matched_name,
                    ready: missing_requirements.is_empty(),
                    missing_requirements,
                    uncertainties: Vec::new(),
                });
            }
        } else {
            uncertainty.push("inventory state is missing".to_string());
        }

        let party_work = self.analyze_party(snapshot, &mut uncertainty);
        let mut craftable_recipes = Vec::new();
        for recipe in self.engine.store().recipes() {
            let Some(output) = self.engine.store().item(&recipe.output.item_id) else {
                continue;
            };
            let answer = self
                .engine
                .calculate_craftable_count(&output.names.en, &inventory);
            merge_answer_metadata(&answer, &mut provenance, &mut uncertainty, &mut version);
            if answer.status != AnswerStatus::Ok {
                continue;
            }
            let calculation = answer.data.expect("ok calculation has data");
            if calculation.maximum_additional_count == 0 {
                continue;
            }
            craftable_recipes.push(CraftableNowRecipe {
                target_id: calculation.target_id,
                target_name: output.names.en.clone(),
                recipe_id: calculation.recipe_id,
                maximum_additional_count: calculation.maximum_additional_count,
                limiting_material_ids: calculation.limiting_material_ids,
            });
        }
        craftable_recipes.sort_by(|left, right| left.target_id.cmp(&right.target_id));

        let analysis = PlannerAnalysis {
            state_summary: snapshot.summarize(
                "plan next goals",
                &SnapshotSummaryOptions::include_all(),
                now,
            ),
            inventory_gap: InventoryGapAnalysis {
                goals: inventory_goals,
            },
            party_work,
            craftable_now: CraftableNowAnalysis {
                recipes: craftable_recipes,
            },
            goal_readiness: GoalReadinessAnalysis {
                goals: readiness_goals,
            },
        };
        let status = if uncertainty.is_empty() {
            PlannerStatus::Ok
        } else {
            PlannerStatus::Unknown
        };
        PlannerAnswer {
            status,
            data: Some(analysis),
            provenance: provenance.into_iter().collect(),
            version,
            uncertainty,
            errors: Vec::new(),
        }
    }

    fn analyze_party(
        &self,
        snapshot: &PlayerStateSnapshot,
        uncertainty: &mut Vec<String>,
    ) -> PartyWorkAnalysis {
        let mut members = Vec::new();
        let mut covered = BTreeSet::new();
        if let Some(party) = &snapshot.party {
            for member in party {
                let Resolution::Unique(entity) =
                    self.engine.resolve(&member.pal, Some(EntityKind::Pal))
                else {
                    push_unique(uncertainty, format!("unknown party Pal {}", member.pal));
                    continue;
                };
                let Some(pal) = self.engine.store().pal(&entity.id) else {
                    continue;
                };
                let work_kind_ids = pal
                    .work_suitability
                    .iter()
                    .map(|suitability| work_kind_id(suitability.kind))
                    .collect::<BTreeSet<_>>();
                covered.extend(work_kind_ids.iter().copied());
                members.push(PartyWorkMember {
                    pal_id: pal.id.clone(),
                    pal_name: pal.names.en.clone(),
                    slot: member.slot,
                    work_kinds: pal
                        .work_suitability
                        .iter()
                        .map(|suitability| suitability.kind)
                        .collect(),
                });
            }
        } else {
            push_unique(uncertainty, "party state is missing".to_string());
        }
        let missing = ALL_WORK_KINDS
            .iter()
            .copied()
            .filter(|kind| !covered.contains(work_kind_id(*kind)))
            .collect();
        members.sort_by_key(|member| member.slot);
        PartyWorkAnalysis {
            members,
            covered_kinds: ALL_WORK_KINDS
                .iter()
                .copied()
                .filter(|kind| covered.contains(work_kind_id(*kind)))
                .collect(),
            missing_kinds: missing,
        }
    }

    pub fn recommend(
        &self,
        snapshot: &PlayerStateSnapshot,
        now: DateTime<Utc>,
    ) -> PlannerAnswer<Vec<PlannerRecommendation>> {
        let analysis_answer = self.analyze(snapshot, now);
        let mut uncertainty = analysis_answer.uncertainty.clone();
        let mut errors = analysis_answer.errors.clone();
        let validation = SnapshotValidator.validate(snapshot, now);
        if !validation.valid {
            errors.extend(validation.errors);
            return error_answer(errors);
        }
        if snapshot.freshness(now) != SnapshotFreshness::Fresh {
            push_unique(&mut uncertainty, "snapshot is stale".to_string());
        }
        if let Some(goals) = &snapshot.goals {
            for goal in goals {
                let expected_kind = match goal.kind.as_str() {
                    "craft" => EntityKind::Item,
                    "progression" => EntityKind::Technology,
                    "pal_work" => EntityKind::Pal,
                    _ => EntityKind::Item,
                };
                if matches!(
                    self.engine.resolve(&goal.target, Some(expected_kind)),
                    Resolution::Unknown
                ) {
                    push_unique(
                        &mut uncertainty,
                        format!("unknown goal target {}", goal.target),
                    );
                    return unknown_recommendations(uncertainty, analysis_answer);
                }
            }
        }
        if !analysis_answer.version.matches {
            push_unique(
                &mut uncertainty,
                format!(
                    "knowledge version {} does not match configured game version {}",
                    analysis_answer.version.knowledge_version,
                    analysis_answer
                        .version
                        .configured_game_version
                        .as_deref()
                        .unwrap_or("unknown")
                ),
            );
        }
        let missing_fields = snapshot.completeness().missing_fields;
        if !missing_fields.is_empty() {
            push_unique(
                &mut uncertainty,
                format!("player state is missing: {}", missing_fields.join(", ")),
            );
        }
        let Some(analysis) = &analysis_answer.data else {
            return unknown_recommendations(uncertainty, analysis_answer);
        };
        let state_fields = present_state_fields(snapshot);
        let evidence_kinds = analysis
            .state_summary
            .evidence_kinds
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        let mut recommendations = Vec::new();
        for goal in &analysis.inventory_gap.goals {
            for material in &goal.shortage.shortages {
                recommendations.push(PlannerRecommendation {
                    id: format!("collect-{}-{}", material.missing_quantity, material.item_id),
                    action: format!(
                        "Collect {} {}",
                        material.missing_quantity, material.item_name
                    ),
                    reason: format!(
                        "{} requires {} more {} for the requested {}",
                        goal.target_name,
                        material.missing_quantity,
                        material.item_name,
                        goal.target_name
                    ),
                    requirements: vec![format!(
                        "Gather {} additional {}",
                        material.missing_quantity, material.item_name
                    )],
                    alternatives: vec![format!("Check drops or trades for {}", material.item_name)],
                    expected_benefit: format!("Moves {} toward readiness", goal.target_name),
                    risk: "Low; normal gathering only".to_string(),
                    uncertainty: String::new(),
                    score: RecommendationScore {
                        relevance: 5,
                        effort: 2,
                        benefit: 5,
                        risk: 1,
                        uncertainty: 1,
                    },
                    basis: RecommendationBasis {
                        knowledge_record_ids: vec![
                            goal.shortage
                                .material_calculation
                                .recipe_id
                                .clone()
                                .unwrap_or_default(),
                            material.item_id.clone(),
                        ],
                        state_fields: state_fields.clone(),
                        evidence_kinds: evidence_kinds.clone(),
                        assumptions: Vec::new(),
                    },
                });
            }
        }
        for recipe in &analysis.craftable_now.recipes {
            recommendations.push(PlannerRecommendation {
                id: format!("craft-now-{}", recipe.target_id),
                action: format!(
                    "Craft {} {} now",
                    recipe.maximum_additional_count, recipe.target_name
                ),
                reason: format!(
                    "The supplied inventory already covers {} {}",
                    recipe.maximum_additional_count, recipe.target_name
                ),
                requirements: vec![format!("Use the reviewed {} recipe", recipe.target_name)],
                alternatives: vec!["Keep materials for a higher-priority goal".to_string()],
                expected_benefit: "Converts surplus materials into useful progress".to_string(),
                risk: "Low; crafting remains player-controlled".to_string(),
                uncertainty: String::new(),
                score: RecommendationScore {
                    relevance: 4,
                    effort: 1,
                    benefit: 4,
                    risk: 1,
                    uncertainty: 1,
                },
                basis: RecommendationBasis {
                    knowledge_record_ids: vec![
                        recipe.recipe_id.clone().unwrap_or_default(),
                        recipe.target_id.clone(),
                    ],
                    state_fields: state_fields.clone(),
                    evidence_kinds: evidence_kinds.clone(),
                    assumptions: Vec::new(),
                },
            });
        }
        if let (true, Some(missing_kind)) = (
            snapshot.party.is_some(),
            analysis.party_work.missing_kinds.first(),
        ) {
            recommendations.push(PlannerRecommendation {
                id: format!("party-work-{}", work_kind_id(*missing_kind)),
                action: format!("Add a Pal with {}", work_kind_label(*missing_kind)),
                reason: "The supplied party does not cover every reviewed work suitability"
                    .to_string(),
                requirements: vec![format!(
                    "Capture or assign a Pal with {}",
                    work_kind_label(*missing_kind)
                )],
                alternatives: vec!["Use a different work goal first".to_string()],
                expected_benefit: "Improves base coverage for relevant work".to_string(),
                risk: "Medium; suitable Pal choice depends on combat and base needs".to_string(),
                uncertainty: "Reviewed seed coverage is intentionally small".to_string(),
                score: RecommendationScore {
                    relevance: 2,
                    effort: 4,
                    benefit: 3,
                    risk: 2,
                    uncertainty: 3,
                },
                basis: RecommendationBasis {
                    knowledge_record_ids: analysis
                        .party_work
                        .members
                        .iter()
                        .map(|member| member.pal_id.clone())
                        .collect(),
                    state_fields: state_fields.clone(),
                    evidence_kinds: evidence_kinds.clone(),
                    assumptions: vec![
                        "Missing work kinds matter only when relevant to the goal".to_string()
                    ],
                },
            });
        }
        if let Some(preferences) = &snapshot.preferences {
            if preferences.long_horizon {
                let action = if matches!(
                    preferences.spoiler_level.as_str(),
                    "mechanics" | "progression"
                ) {
                    "Review the next technology stage"
                } else {
                    "Review only your immediate next unlock"
                };
                recommendations.push(PlannerRecommendation {
                    id: "review-long-horizon".to_string(),
                    action: action.to_string(),
                    reason: "Your preferences allow one stage of lookahead".to_string(),
                    requirements: vec!["Keep later spoilers hidden until requested".to_string()],
                    alternatives: vec!["Stay on the current goal".to_string()],
                    expected_benefit: "Prepares the next unlock without oversharing".to_string(),
                    risk: "Low; lookahead is preference-gated".to_string(),
                    uncertainty: String::new(),
                    score: RecommendationScore {
                        relevance: 2,
                        effort: 1,
                        benefit: 3,
                        risk: 1,
                        uncertainty: 2,
                    },
                    basis: RecommendationBasis {
                        knowledge_record_ids: self
                            .engine
                            .store()
                            .technologies()
                            .map(|technology| technology.id.clone())
                            .collect::<Vec<_>>(),
                        state_fields: state_fields.clone(),
                        evidence_kinds: evidence_kinds.clone(),
                        assumptions: Vec::new(),
                    },
                });
            }
        }
        while recommendations.len() < 3 {
            let index = recommendations.len();
            let (id, action, reason, requirement, benefit) = match index {
                0 => (
                    "enter-inventory",
                    "Enter your current inventory",
                    "Material advice needs current quantities",
                    "Provide item names and quantities",
                    "Enables deterministic shortage and craftable advice",
                ),
                1 => (
                    "set-craft-goal",
                    "Set a specific craft goal",
                    "A target connects state to reviewed requirements",
                    "Choose an item and desired quantity",
                    "Produces ranked next steps",
                ),
                _ => (
                    "review-static-guide",
                    "Ask the static guide for acquisition help",
                    "Public knowledge can guide gathering while state is incomplete",
                    "Name the material or item",
                    "Keeps advice grounded without inventing state",
                ),
            };
            recommendations.push(PlannerRecommendation {
                id: id.to_string(),
                action: action.to_string(),
                reason: reason.to_string(),
                requirements: vec![requirement.to_string()],
                alternatives: vec!["Provide a complete snapshot later".to_string()],
                expected_benefit: benefit.to_string(),
                risk: "Low".to_string(),
                uncertainty: "Player state is incomplete".to_string(),
                score: RecommendationScore {
                    relevance: 1,
                    effort: 1,
                    benefit: 2,
                    risk: 1,
                    uncertainty: 4,
                },
                basis: RecommendationBasis {
                    knowledge_record_ids: first_knowledge_ids(&self.engine),
                    state_fields: state_fields.clone(),
                    evidence_kinds: evidence_kinds.clone(),
                    assumptions: vec!["No dynamic state was observed".to_string()],
                },
            });
        }
        recommendations.sort_by(|left, right| {
            (ReverseScore(left.score), &left.id).cmp(&(ReverseScore(right.score), &right.id))
        });
        recommendations.truncate(5);
        let status = if uncertainty.is_empty() {
            PlannerStatus::Ok
        } else {
            PlannerStatus::Unknown
        };
        PlannerAnswer {
            status,
            data: Some(recommendations),
            provenance: analysis_answer.provenance.clone(),
            version: analysis_answer.version.clone(),
            uncertainty,
            errors,
        }
    }
}

const ALL_WORK_KINDS: [WorkKind; 12] = [
    WorkKind::Kindling,
    WorkKind::Watering,
    WorkKind::Planting,
    WorkKind::GeneratingElectricity,
    WorkKind::Handiwork,
    WorkKind::Gathering,
    WorkKind::Lumbering,
    WorkKind::Mining,
    WorkKind::MedicineProduction,
    WorkKind::Transporting,
    WorkKind::Farming,
    WorkKind::Cooling,
];

struct ReverseScore(RecommendationScore);

impl Eq for ReverseScore {}

impl PartialEq for ReverseScore {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl PartialOrd for ReverseScore {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ReverseScore {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other
            .0
            .relevance
            .cmp(&self.0.relevance)
            .then_with(|| self.0.effort.cmp(&other.0.effort))
            .then_with(|| other.0.benefit.cmp(&self.0.benefit))
            .then_with(|| self.0.risk.cmp(&other.0.risk))
            .then_with(|| self.0.uncertainty.cmp(&other.0.uncertainty))
    }
}

fn snapshot_inventory(snapshot: &PlayerStateSnapshot) -> Vec<InventoryEntry> {
    snapshot
        .inventory
        .as_ref()
        .map(|inventory| {
            inventory
                .iter()
                .map(|entry| InventoryEntry::new(entry.item.clone(), entry.quantity))
                .collect()
        })
        .unwrap_or_default()
}

fn merge_answer_metadata<T>(
    answer: &GuideAnswer<T>,
    provenance: &mut BTreeSet<ProvenanceSummary>,
    uncertainty: &mut Vec<String>,
    version: &mut VersionInfo,
) {
    provenance.extend(answer.provenance.iter().cloned());
    for message in &answer.uncertainty {
        push_unique(uncertainty, message.clone());
    }
    if answer.version.knowledge_version != "unknown" {
        *version = answer.version.clone();
    }
}

fn present_state_fields(snapshot: &PlayerStateSnapshot) -> Vec<String> {
    let mut fields = Vec::new();
    if snapshot.inventory.is_some() {
        fields.push("inventory".to_string());
    }
    if snapshot.party.is_some() {
        fields.push("party".to_string());
    }
    if snapshot.unlocked_technologies.is_some() {
        fields.push("unlocked_technologies".to_string());
    }
    if snapshot.captured_pals.is_some() {
        fields.push("captured_pals".to_string());
    }
    if snapshot.player_level.is_some() {
        fields.push("player_level".to_string());
    }
    if snapshot.goals.is_some() {
        fields.push("goals".to_string());
    }
    if snapshot.preferences.is_some() {
        fields.push("preferences".to_string());
    }
    fields
}

fn first_knowledge_ids(engine: &GuideEngine) -> Vec<String> {
    let mut ids = engine
        .store()
        .items()
        .map(|item| item.id.clone())
        .collect::<Vec<_>>();
    ids.extend(engine.store().recipes().map(|recipe| recipe.id.clone()));
    ids.sort();
    ids.truncate(1);
    ids
}

fn work_kind_id(kind: WorkKind) -> &'static str {
    match kind {
        WorkKind::Kindling => "kindling",
        WorkKind::Watering => "watering",
        WorkKind::Planting => "planting",
        WorkKind::GeneratingElectricity => "generating_electricity",
        WorkKind::Handiwork => "handiwork",
        WorkKind::Gathering => "gathering",
        WorkKind::Lumbering => "lumbering",
        WorkKind::Mining => "mining",
        WorkKind::MedicineProduction => "medicine_production",
        WorkKind::Transporting => "transporting",
        WorkKind::Farming => "farming",
        WorkKind::Cooling => "cooling",
    }
}

fn work_kind_label(kind: WorkKind) -> String {
    work_kind_id(kind).replace('_', " ")
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.contains(&value) {
        values.push(value);
    }
}

fn error_answer<T>(errors: Vec<String>) -> PlannerAnswer<T> {
    PlannerAnswer {
        status: PlannerStatus::Error,
        data: None,
        provenance: Vec::new(),
        version: VersionInfo {
            knowledge_version: "unknown".to_string(),
            configured_game_version: None,
            matches: true,
        },
        uncertainty: Vec::new(),
        errors,
    }
}

fn unknown_recommendations(
    uncertainty: Vec<String>,
    analysis: PlannerAnswer<PlannerAnalysis>,
) -> PlannerAnswer<Vec<PlannerRecommendation>> {
    PlannerAnswer {
        status: PlannerStatus::Unknown,
        data: None,
        provenance: analysis.provenance,
        version: analysis.version,
        uncertainty,
        errors: analysis.errors,
    }
}
