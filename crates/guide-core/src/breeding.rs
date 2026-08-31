use crate::{AnswerStatus, GuideEngine, ResolvedEntity};
use game_knowledge::{BreedingRuleRecord, ConflictRecord, Provenance};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

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
struct SearchPath {
    current_id: String,
    visited: BTreeSet<String>,
    steps: Vec<BreedingStep>,
    provenances: Vec<Provenance>,
    subject_ids: BTreeSet<String>,
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
        let rules = self
            .store()
            .breeding_rules()
            .filter(|rule| pair_matches(rule, &parent_a.id, &parent_b.id))
            .cloned()
            .collect::<Vec<BreedingRuleRecord>>();

        if rules.is_empty() {
            return self
                .context()
                .unknown("unknown breeding rule; no reviewed result matches this parent pair");
        }
        if rules.len() > 1 {
            let children = rules
                .iter()
                .map(|rule| rule.child_id.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            return self.context().ambiguous(format!(
                "ambiguous breeding rules; possible children: {children}; no result was selected"
            ));
        }

        let rule = &rules[0];
        let result = BreedingResult {
            parent_a_id: rule.parent_a_id.clone(),
            parent_b_id: rule.parent_b_id.clone(),
            child_id: rule.child_id.clone(),
            rule_id: rule.id.clone(),
        };
        let conflicts = self
            .store()
            .conflicts_for_subject(&rule.id)
            .into_iter()
            .chain(self.store().conflicts_for_subject(&result.child_id))
            .collect::<Vec<&ConflictRecord>>();
        let status = if conflicts.is_empty() {
            AnswerStatus::Ok
        } else {
            AnswerStatus::Ambiguous
        };
        self.context()
            .answer(status, Some(result), vec![&rule.provenance], conflicts)
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

        let start_provenance = self
            .store()
            .pal(&start.id)
            .expect("resolved start Pal exists")
            .provenance
            .clone();
        let target_provenance = self
            .store()
            .pal(&target.id)
            .expect("resolved target Pal exists")
            .provenance
            .clone();
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

        let initial = SearchPath {
            current_id: start.id.clone(),
            visited: BTreeSet::from([start.id.clone()]),
            steps: Vec::new(),
            provenances: vec![start_provenance],
            subject_ids: BTreeSet::from([start.id.clone()]),
        };
        let mut queue = vec![initial];
        let mut candidates = Vec::new();

        while let Some(path) = queue.first().cloned() {
            queue.remove(0);
            if let Some(shortest_depth) = candidates
                .first()
                .map(|candidate: &SearchPath| candidate.steps.len())
            {
                if path.steps.len() > shortest_depth {
                    break;
                }
            }
            if path.steps.len() >= maximum_depth {
                continue;
            }

            for rule in self.store().breeding_rules() {
                let next_id =
                    if rule.parent_a_id == path.current_id || rule.parent_b_id == path.current_id {
                        rule.child_id.clone()
                    } else {
                        continue;
                    };
                if path.visited.contains(&next_id) {
                    continue;
                }
                let step = BreedingStep {
                    parent_a_id: rule.parent_a_id.clone(),
                    parent_b_id: rule.parent_b_id.clone(),
                    child_id: rule.child_id.clone(),
                    rule_id: rule.id.clone(),
                };
                let mut visited = path.visited.clone();
                visited.insert(next_id.clone());
                let mut steps = path.steps.clone();
                steps.push(step);
                let mut provenances = path.provenances.clone();
                provenances.push(rule.provenance.clone());
                let mut subject_ids = path.subject_ids.clone();
                subject_ids.insert(rule.id.clone());
                subject_ids.insert(next_id.clone());
                let next_path = SearchPath {
                    current_id: next_id.clone(),
                    visited,
                    steps,
                    provenances,
                    subject_ids,
                };
                if next_id == target.id {
                    candidates.push(next_path);
                } else {
                    queue.push(next_path);
                }
            }
        }

        match candidates.len() {
            0 => self.context().unknown(format!(
                "unknown breeding chain from {} to {} within depth {maximum_depth}",
                start.id, target.id
            )),
            1 => {
                let path = candidates.remove(0);
                let chain = BreedingChain {
                    start_id: start.id,
                    target_id: target.id,
                    steps: path.steps,
                };
                let provenance_references = path
                    .provenances
                    .iter()
                    .chain(std::iter::once(&target_provenance))
                    .collect::<Vec<_>>();
                let conflicts = path
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
                    .answer(status, Some(chain), provenance_references, conflicts)
            }
            _ => {
                let descriptions = candidates
                    .iter()
                    .map(|path| {
                        path.steps
                            .iter()
                            .map(|step| step.child_id.as_str())
                            .collect::<Vec<_>>()
                            .join(" -> ")
                    })
                    .collect::<Vec<_>>()
                    .join("; ");
                self.context().ambiguous(format!(
                    "ambiguous shortest breeding chains: {descriptions}; no chain was selected"
                ))
            }
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

fn pair_matches(rule: &BreedingRuleRecord, parent_a: &str, parent_b: &str) -> bool {
    (rule.parent_a_id == parent_a && rule.parent_b_id == parent_b)
        || (rule.parent_a_id == parent_b && rule.parent_b_id == parent_a)
}
