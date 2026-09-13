use crate::{AnswerStatus, GuideEngine, ResolvedEntity};
use game_knowledge::{
    BreedingRuleRecord, ConflictRecord, ConflictResolution, PalRecord, Provenance,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BreedingResult {
    pub parent_a_id: String,
    pub parent_b_id: String,
    pub child_id: String,
    pub rule_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BreedingStep {
    pub parent_a_id: String,
    pub parent_b_id: String,
    pub child_id: String,
    pub rule_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BreedingChain {
    pub start_id: String,
    pub target_id: String,
    pub steps: Vec<BreedingStep>,
}

#[derive(Debug, Clone)]
struct BreedingChainEdge {
    previous_id: String,
    step: BreedingStep,
    provenance: Provenance,
    subject_id: Option<String>,
}

const BREEDING_FORMULA_RULE_ID: &str = "BREEDING_FORMULA";
const MAX_REPORTED_BREEDING_CHAINS: usize = 8;

struct BreedingIndex<'a> {
    breeding_parents: Vec<&'a PalRecord>,
    formula_children: Vec<&'a PalRecord>,
    all_pals: BTreeMap<String, &'a PalRecord>,
    breeding_parent_ids: BTreeSet<String>,
    special_rules: BTreeMap<(String, String), Vec<&'a BreedingRuleRecord>>,
    special_children: BTreeSet<String>,
}

struct BreedingOutcome<'a> {
    child_record: &'a PalRecord,
    rule: Option<&'a BreedingRuleRecord>,
}

enum PalResolution {
    Unique(ResolvedEntity),
    Ambiguous(Vec<ResolvedEntity>),
    Unknown,
}

impl GuideEngine {
    pub fn calculate_breeding_result(
        &self,
        parent_a_query: &str,
        parent_b_query: &str,
    ) -> crate::GuideAnswer<BreedingResult> {
        let (parent_a_resolution, parent_b_resolution) = (
            self.resolve_pal(parent_a_query),
            self.resolve_pal(parent_b_query),
        );
        let (parent_a, parent_b) = match (parent_a_resolution, parent_b_resolution) {
            (PalResolution::Unique(parent_a), PalResolution::Unique(parent_b)) => {
                (parent_a, parent_b)
            }
            (parent_a_resolution, parent_b_resolution) => {
                let mut candidates = Vec::new();
                if let PalResolution::Ambiguous(matches) = parent_a_resolution {
                    candidates.extend(matches);
                }
                if let PalResolution::Ambiguous(matches) = parent_b_resolution {
                    candidates.extend(matches);
                }
                if candidates.is_empty() {
                    return self
                        .context()
                        .unknown("unknown parent Pal; no reviewed record matches");
                }
                let ids = candidates
                    .into_iter()
                    .map(|candidate| candidate.id)
                    .collect::<Vec<_>>();
                return self.context().ambiguous(format!(
                    "ambiguous parent Pal; candidates: {}; no parent was selected",
                    ids.join(", ")
                ));
            }
        };

        let index = self.breeding_index();
        let parent_a_record = self
            .store()
            .pal(&parent_a.id)
            .expect("resolved parent Pal exists");
        let parent_b_record = self
            .store()
            .pal(&parent_b.id)
            .expect("resolved parent Pal exists");
        let mut outcomes = breeding_outcomes(&index, parent_a_record, parent_b_record);
        if outcomes.is_empty() {
            return self
                .context()
                .unknown("unknown breeding rule; no reviewed result matches this parent pair");
        }
        if outcomes.len() > 1 {
            let described = outcomes
                .iter()
                .map(describe_breeding_outcome)
                .collect::<Vec<_>>()
                .join("; ");
            return self.context().ambiguous(format!(
                "this parent pair has more than one reviewed result: {described}; which child you get depends on the condition recorded for each rule, so no single child was selected"
            ));
        }
        let outcome = outcomes.remove(0);
        let result = match outcome.rule {
            Some(rule) => BreedingResult {
                parent_a_id: rule.parent_a_id.clone(),
                parent_b_id: rule.parent_b_id.clone(),
                child_id: rule.child_id.clone(),
                rule_id: rule.id.clone(),
            },
            None => BreedingResult {
                parent_a_id: parent_a.id.clone(),
                parent_b_id: parent_b.id.clone(),
                child_id: outcome.child_record.id.clone(),
                rule_id: BREEDING_FORMULA_RULE_ID.to_string(),
            },
        };
        let conflicts = self
            .store()
            .conflicts_for_subject(&result.rule_id)
            .into_iter()
            .chain(self.store().conflicts_for_subject(&result.child_id))
            .filter(|conflict| conflict.resolution == ConflictResolution::Unresolved)
            .collect::<Vec<&ConflictRecord>>();
        let status = if conflicts.is_empty() {
            AnswerStatus::Ok
        } else {
            AnswerStatus::Ambiguous
        };
        let provenance = outcome
            .rule
            .map(|rule| &rule.provenance)
            .unwrap_or(&outcome.child_record.provenance);
        self.context()
            .answer(status, Some(result), vec![provenance], conflicts)
    }

    pub fn calculate_breeding_chain(
        &self,
        start_query: &str,
        target_query: &str,
        maximum_depth: usize,
    ) -> crate::GuideAnswer<BreedingChain> {
        let (start_resolution, target_resolution) = (
            self.resolve_pal(start_query),
            self.resolve_pal(target_query),
        );
        let (start, target) = match (start_resolution, target_resolution) {
            (PalResolution::Unique(start), PalResolution::Unique(target)) => (start, target),
            (start_resolution, target_resolution) => {
                let mut candidates = Vec::new();
                if let PalResolution::Ambiguous(matches) = start_resolution {
                    candidates.extend(matches);
                }
                if let PalResolution::Ambiguous(matches) = target_resolution {
                    candidates.extend(matches);
                }
                if candidates.is_empty() {
                    return self
                        .context()
                        .unknown("unknown start or target Pal; no reviewed record matches");
                }
                let ids = candidates
                    .into_iter()
                    .map(|candidate| candidate.id)
                    .collect::<Vec<_>>();
                return self.context().ambiguous(format!(
                    "ambiguous start or target Pal; candidates: {}; no endpoint was selected",
                    ids.join(", ")
                ));
            }
        };
        if maximum_depth == 0 && start.id != target.id {
            return self
                .context()
                .error("breeding depth limit must be greater than zero");
        }

        let start_record = self
            .store()
            .pal(&start.id)
            .expect("resolved start Pal exists");
        let target_record = self
            .store()
            .pal(&target.id)
            .expect("resolved target Pal exists");
        let start_provenance = start_record.provenance.clone();
        let target_provenance = target_record.provenance.clone();
        if start.id == target.id {
            let chain = BreedingChain {
                start_id: start.id.clone(),
                target_id: target.id,
                steps: Vec::new(),
            };
            return self.context().answer(
                AnswerStatus::Ok,
                Some(chain),
                vec![&start_provenance, &target_provenance],
                Vec::new(),
            );
        }

        let index = self.breeding_index();
        let start_id = start.id.clone();
        let target_id = target.id.clone();
        let mut depths = BTreeMap::<String, usize>::new();
        let mut predecessors = BTreeMap::<String, Vec<BreedingChainEdge>>::new();
        depths.insert(start_id.clone(), 0);
        let mut frontier = vec![start_id.clone()];
        let mut level = 0_usize;
        let mut target_depth = None;

        while !frontier.is_empty() && level < maximum_depth {
            let mut next_frontier = Vec::new();
            for current_id in &frontier {
                let current_record = self.store().pal(current_id).expect("path Pal exists");
                for partner in &index.breeding_parents {
                    for outcome in breeding_outcomes(&index, current_record, partner) {
                        let next_id = outcome.child_record.id.clone();
                        if next_id == *current_id {
                            continue;
                        }
                        let edge = match outcome.rule {
                            Some(rule) => BreedingChainEdge {
                                previous_id: current_id.clone(),
                                step: BreedingStep {
                                    parent_a_id: rule.parent_a_id.clone(),
                                    parent_b_id: rule.parent_b_id.clone(),
                                    child_id: next_id.clone(),
                                    rule_id: rule.id.clone(),
                                },
                                provenance: rule.provenance.clone(),
                                subject_id: Some(rule.id.clone()),
                            },
                            None => BreedingChainEdge {
                                previous_id: current_id.clone(),
                                step: BreedingStep {
                                    parent_a_id: current_id.clone(),
                                    parent_b_id: partner.id.clone(),
                                    child_id: next_id.clone(),
                                    rule_id: BREEDING_FORMULA_RULE_ID.to_string(),
                                },
                                provenance: outcome.child_record.provenance.clone(),
                                subject_id: None,
                            },
                        };
                        match depths.get(&next_id) {
                            Some(existing) if *existing < level + 1 => continue,
                            Some(_) => {}
                            None => {
                                depths.insert(next_id.clone(), level + 1);
                                next_frontier.push(next_id.clone());
                            }
                        }
                        predecessors.entry(next_id).or_default().push(edge);
                    }
                }
            }
            level += 1;
            if depths.get(&target_id) == Some(&level) {
                target_depth = Some(level);
                break;
            }
            frontier = next_frontier;
        }

        let Some(target_depth) = target_depth else {
            return self.context().unknown(format!(
                "unknown breeding chain from {} to {} within depth {maximum_depth}",
                start.id, target.id
            ));
        };

        let mut ways = BTreeMap::<String, u64>::new();
        ways.insert(start_id.clone(), 1);
        for node_level in 1..=target_depth {
            for (node_id, node_depth) in &depths {
                if *node_depth != node_level {
                    continue;
                }
                let total = predecessors
                    .get(node_id)
                    .map(|edges| {
                        edges
                            .iter()
                            .filter_map(|edge| ways.get(&edge.previous_id).copied())
                            .fold(0_u64, u64::saturating_add)
                    })
                    .unwrap_or(0);
                ways.insert(node_id.clone(), total);
            }
        }
        let shortest_chain_count = ways.get(&target_id).copied().unwrap_or(0);

        let mut chains = Vec::new();
        collect_breeding_chains(
            &predecessors,
            &target_id,
            &mut Vec::new(),
            &mut chains,
            MAX_REPORTED_BREEDING_CHAINS,
        );

        if shortest_chain_count == 1 && chains.len() == 1 {
            let mut steps = Vec::new();
            let mut provenances = vec![start_provenance];
            let mut subject_ids = BTreeSet::from([start.id.clone()]);
            for edge in chains[0].iter().rev() {
                steps.push(edge.step.clone());
                provenances.push(edge.provenance.clone());
                if let Some(subject_id) = &edge.subject_id {
                    subject_ids.insert(subject_id.clone());
                }
                subject_ids.insert(edge.step.child_id.clone());
            }
            let chain = BreedingChain {
                start_id: start.id,
                target_id: target.id,
                steps,
            };
            let provenance_references = provenances
                .iter()
                .chain(std::iter::once(&target_provenance))
                .collect::<Vec<_>>();
            let conflicts = subject_ids
                .iter()
                .flat_map(|subject_id| self.store().conflicts_for_subject(subject_id))
                .filter(|conflict| conflict.resolution == ConflictResolution::Unresolved)
                .collect::<Vec<&ConflictRecord>>();
            let status = if conflicts.is_empty() {
                AnswerStatus::Ok
            } else {
                AnswerStatus::Ambiguous
            };
            return self
                .context()
                .answer(status, Some(chain), provenance_references, conflicts);
        }

        let descriptions = chains
            .iter()
            .map(|edges| {
                edges
                    .iter()
                    .rev()
                    .map(|edge| edge.step.child_id.as_str())
                    .collect::<Vec<_>>()
                    .join(" -> ")
            })
            .collect::<Vec<_>>()
            .join("; ");
        let remaining = shortest_chain_count.saturating_sub(chains.len() as u64);
        let suffix = if remaining > 0 {
            format!(" (+{remaining} more)")
        } else {
            String::new()
        };
        self.context().ambiguous(format!(
            "ambiguous shortest breeding chains: {descriptions}{suffix}; no chain was selected"
        ))
    }

    fn breeding_index(&self) -> BreedingIndex<'_> {
        let mut special_rules = BTreeMap::<(String, String), Vec<&BreedingRuleRecord>>::new();
        let mut special_children = BTreeSet::<String>::new();
        let mut breeding_parent_ids = BTreeSet::<String>::new();
        for rule in self.store().breeding_rules() {
            special_rules
                .entry(ordered_pair(&rule.parent_a_id, &rule.parent_b_id))
                .or_default()
                .push(rule);
            special_children.insert(rule.child_id.clone());
            breeding_parent_ids.insert(rule.parent_a_id.clone());
            breeding_parent_ids.insert(rule.parent_b_id.clone());
        }

        let mut breeding_parents = Vec::new();
        let mut formula_children = Vec::new();
        let mut all_pals = BTreeMap::<String, &PalRecord>::new();
        for pal in self.store().pals() {
            all_pals.insert(pal.id.clone(), pal);
            if is_breeding_parent(pal, &special_children, &breeding_parent_ids) {
                breeding_parents.push(pal);
            }
            if usable_breeding_rank(pal).is_some()
                && !pal.breeding_ignore_combi
                && !special_children.contains(&pal.id)
            {
                formula_children.push(pal);
            }
        }

        BreedingIndex {
            breeding_parents,
            formula_children,
            all_pals,
            breeding_parent_ids,
            special_rules,
            special_children,
        }
    }

    fn resolve_pal(&self, query: &str) -> PalResolution {
        match self.resolve(query, Some(crate::EntityKind::Pal)) {
            crate::resolver::Resolution::Unique(resolved) => PalResolution::Unique(resolved),
            crate::resolver::Resolution::Ambiguous(candidates) => {
                PalResolution::Ambiguous(candidates)
            }
            crate::resolver::Resolution::Unknown => PalResolution::Unknown,
        }
    }
}

fn collect_breeding_chains(
    predecessors: &BTreeMap<String, Vec<BreedingChainEdge>>,
    current_id: &str,
    reversed: &mut Vec<BreedingChainEdge>,
    chains: &mut Vec<Vec<BreedingChainEdge>>,
    limit: usize,
) {
    if chains.len() >= limit {
        return;
    }
    let Some(edges) = predecessors.get(current_id) else {
        chains.push(reversed.clone());
        return;
    };
    for edge in edges {
        reversed.push(edge.clone());
        collect_breeding_chains(predecessors, &edge.previous_id, reversed, chains, limit);
        reversed.pop();
        if chains.len() >= limit {
            return;
        }
    }
}

fn breeding_outcomes<'a>(
    index: &'a BreedingIndex<'a>,
    parent_a: &'a PalRecord,
    parent_b: &'a PalRecord,
) -> Vec<BreedingOutcome<'a>> {
    let pair_key = ordered_pair(&parent_a.id, &parent_b.id);
    if let Some(rules) = index.special_rules.get(&pair_key) {
        return rules
            .iter()
            .filter(|rule| index.special_children.contains(&rule.child_id))
            .map(|rule| BreedingOutcome {
                child_record: self_lookup(index, &rule.child_id),
                rule: Some(*rule),
            })
            .collect();
    }

    if !is_breeding_parent(
        parent_a,
        &index.special_children,
        &index.breeding_parent_ids,
    ) || !is_breeding_parent(
        parent_b,
        &index.special_children,
        &index.breeding_parent_ids,
    ) {
        return Vec::new();
    }
    if parent_a.id == parent_b.id {
        return vec![BreedingOutcome {
            child_record: parent_a,
            rule: None,
        }];
    }

    let Some(parent_a_rank) = usable_breeding_rank(parent_a) else {
        return Vec::new();
    };
    let Some(parent_b_rank) = usable_breeding_rank(parent_b) else {
        return Vec::new();
    };
    let target_rank = (parent_a_rank + parent_b_rank).div_ceil(2);
    let child_record = index
        .formula_children
        .iter()
        .copied()
        .min_by(|left, right| {
            let left_distance = left
                .breeding_combi_rank
                .unwrap_or_default()
                .abs_diff(target_rank);
            let right_distance = right
                .breeding_combi_rank
                .unwrap_or_default()
                .abs_diff(target_rank);
            left_distance
                .cmp(&right_distance)
                .then_with(|| {
                    right
                        .breeding_combi_priority
                        .unwrap_or_default()
                        .cmp(&left.breeding_combi_priority.unwrap_or_default())
                })
                .then_with(|| left.id.cmp(&right.id))
        });
    match child_record {
        Some(child_record) => vec![BreedingOutcome {
            child_record,
            rule: None,
        }],
        None => Vec::new(),
    }
}

fn describe_breeding_outcome(outcome: &BreedingOutcome<'_>) -> String {
    let names = &outcome.child_record.names;
    let label = match names.zh_hans.as_deref().map(str::trim) {
        Some(chinese) if !chinese.is_empty() => format!("{} ({chinese})", names.en),
        _ => names.en.clone(),
    };
    match outcome.rule {
        Some(rule) => match rule.notes.as_deref().map(str::trim) {
            Some(notes) if !notes.is_empty() => format!("{label} [{}] - {notes}", rule.child_id),
            _ => format!("{label} [{}]", rule.child_id),
        },
        None => format!("{label} [{}]", outcome.child_record.id),
    }
}

fn self_lookup<'a>(index: &'a BreedingIndex<'a>, pal_id: &str) -> &'a PalRecord {
    index
        .all_pals
        .get(pal_id)
        .copied()
        .expect("breeding child Pal exists")
}

fn usable_breeding_rank(pal: &PalRecord) -> Option<u32> {
    pal.breeding_combi_rank
        .filter(|rank| *rank != 0 && *rank != 9999)
}

fn is_breeding_parent(
    pal: &PalRecord,
    special_children: &BTreeSet<String>,
    explicit_parent_ids: &BTreeSet<String>,
) -> bool {
    explicit_parent_ids.contains(&pal.id)
        || (usable_breeding_rank(pal).is_some()
            && (!pal.breeding_ignore_combi
                || pal.breeding_self_only
                || special_children.contains(&pal.id)))
}

fn ordered_pair(first: &str, second: &str) -> (String, String) {
    if first <= second {
        (first.to_string(), second.to_string())
    } else {
        (second.to_string(), first.to_string())
    }
}
