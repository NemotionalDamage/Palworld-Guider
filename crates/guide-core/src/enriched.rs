use crate::{AnswerStatus, GuideAnswer, GuideEngine};
use game_knowledge::{
    ElementType, Provenance, TypeEffectivenessRecord, WazaRecord, WorkKindDescriptionRecord,
};

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PalWazaUnlockDetail {
    pub id: String,
    pub pal_id: String,
    pub waza_id: String,
    pub unlock_level: u32,
    pub waza: WazaRecord,
    pub provenance: Provenance,
}

fn parse_element_type(value: &str) -> Option<ElementType> {
    let normalized = value.trim().replace(['_', '-'], "").to_ascii_lowercase();
    ElementType::from_native(&normalized).or(match normalized.as_str() {
        "normal" => Some(ElementType::Normal),
        "fire" => Some(ElementType::Fire),
        "water" => Some(ElementType::Water),
        "leaf" | "grass" => Some(ElementType::Leaf),
        "earth" | "ground" => Some(ElementType::Earth),
        "ice" => Some(ElementType::Ice),
        "electricity" | "electric" | "electrictype" => Some(ElementType::Electricity),
        "dark" => Some(ElementType::Dark),
        "dragon" => Some(ElementType::Dragon),
        _ => None,
    })
}

impl GuideEngine {
    pub fn get_type_effectiveness(
        &self,
        attacking_type: Option<&str>,
        defending_type: Option<&str>,
    ) -> GuideAnswer<Vec<TypeEffectivenessRecord>> {
        let attacking = attacking_type.and_then(parse_element_type);
        let defending = defending_type.and_then(parse_element_type);
        if attacking_type.is_some() && attacking.is_none() {
            return self
                .context()
                .error("attacking_type must be a supported element type");
        }
        if defending_type.is_some() && defending.is_none() {
            return self
                .context()
                .error("defending_type must be a supported element type");
        }

        let records = self
            .store()
            .type_effectiveness()
            .filter(|record| attacking.is_none_or(|kind| record.attacking_type == kind))
            .filter(|record| defending.is_none_or(|kind| record.defending_type == kind))
            .cloned()
            .collect::<Vec<_>>();
        let status = if records.is_empty() {
            AnswerStatus::Unknown
        } else {
            AnswerStatus::Ok
        };
        let provenances = records
            .iter()
            .map(|record| record.provenance.clone())
            .collect::<Vec<_>>();
        let provenance_refs = provenances.iter().collect::<Vec<_>>();
        let mut answer = self
            .context()
            .answer(status, Some(records), provenance_refs, Vec::new());
        if status == AnswerStatus::Unknown {
            answer
                .uncertainty
                .push("no reviewed type-effectiveness record matched".into());
        }
        answer
    }

    pub fn lookup_waza(&self, query: &str) -> GuideAnswer<WazaRecord> {
        let normalized = normalize_waza(query);
        let mut matches = self
            .store()
            .waza()
            .filter(|record| {
                normalize_waza(&record.id) == normalized
                    || normalize_waza(&record.native_waza_id) == normalized
                    || normalize_waza(&record.names.en) == normalized
                    || record
                        .names
                        .zh_hans
                        .as_ref()
                        .is_some_and(|name| normalize_waza(name) == normalized)
            })
            .cloned()
            .collect::<Vec<_>>();
        matches.sort_by(|left, right| left.id.cmp(&right.id));
        matches.dedup_by(|left, right| left.id == right.id);
        match matches.len() {
            0 => self
                .context()
                .unknown("unknown Waza; no reviewed record matches"),
            1 => {
                let record = matches
                    .into_iter()
                    .next()
                    .expect("one Waza match is present");
                let provenance = record.provenance.clone();
                self.context().answer(
                    AnswerStatus::Ok,
                    Some(record),
                    vec![&provenance],
                    Vec::new(),
                )
            }
            _ => self.context().ambiguous(format!(
                "ambiguous Waza name; candidates: {}",
                matches
                    .iter()
                    .map(|record| record.id.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        }
    }

    pub fn get_pal_waza_unlocks(&self, pal_query: &str) -> GuideAnswer<Vec<PalWazaUnlockDetail>> {
        let resolved = match self.resolve(pal_query, Some(crate::EntityKind::Pal)) {
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
        let Some(pal) = self.store().pal(&resolved.id) else {
            return self
                .context()
                .unknown("resolved Pal is absent from the store");
        };
        let mut unlocks = self
            .store()
            .pal_waza_unlocks()
            .filter(|unlock| unlock.pal_id == pal.id)
            .cloned()
            .collect::<Vec<_>>();
        unlocks.sort_by(|left, right| {
            left.unlock_level
                .cmp(&right.unlock_level)
                .then_with(|| left.waza_id.cmp(&right.waza_id))
        });
        let mut missing_waza_ids = Vec::new();
        let details = unlocks
            .iter()
            .map(|unlock| {
                let waza = self
                    .store()
                    .waza()
                    .find(|record| record.id == unlock.waza_id);
                if waza.is_none() {
                    missing_waza_ids.push(unlock.waza_id.clone());
                }
                waza.map(|waza| PalWazaUnlockDetail {
                    id: unlock.id.clone(),
                    pal_id: unlock.pal_id.clone(),
                    waza_id: unlock.waza_id.clone(),
                    unlock_level: unlock.unlock_level,
                    waza: waza.clone(),
                    provenance: unlock.provenance.clone(),
                })
            })
            .collect::<Vec<_>>();
        let status = if unlocks.is_empty() {
            AnswerStatus::Unknown
        } else if !missing_waza_ids.is_empty() {
            AnswerStatus::Ambiguous
        } else {
            AnswerStatus::Ok
        };
        let mut provenances = unlocks
            .iter()
            .map(|unlock| unlock.provenance.clone())
            .collect::<Vec<_>>();
        for detail in details.iter().flatten() {
            provenances.push(detail.waza.provenance.clone());
        }
        let mut provenance_refs = provenances.iter().collect::<Vec<_>>();
        provenance_refs.push(&pal.provenance);
        let mut answer = self.context().answer(
            status,
            Some(details.into_iter().flatten().collect::<Vec<_>>()),
            provenance_refs,
            Vec::new(),
        );
        if unlocks.is_empty() {
            answer
                .uncertainty
                .push("no reviewed active-skill unlock data for this Pal".into());
        } else if !missing_waza_ids.is_empty() {
            answer.uncertainty.push(format!(
                "reviewed active-skill details are missing for: {}",
                missing_waza_ids.join(", ")
            ));
        }
        answer
    }

    pub fn get_work_kind_descriptions(
        &self,
        query: Option<&str>,
    ) -> GuideAnswer<Vec<WorkKindDescriptionRecord>> {
        let normalized = query.map(normalize_waza);
        let mut records = self
            .store()
            .work_kind_descriptions()
            .filter(|record| {
                normalized.as_ref().is_none_or(|normalized| {
                    normalize_waza(&record.id) == *normalized
                        || normalize_waza(&record.names.en) == *normalized
                        || record
                            .names
                            .zh_hans
                            .as_ref()
                            .is_some_and(|name| normalize_waza(name) == *normalized)
                })
            })
            .cloned()
            .collect::<Vec<_>>();
        records.sort_by(|left, right| left.id.cmp(&right.id));
        let status = if records.is_empty() {
            AnswerStatus::Unknown
        } else {
            AnswerStatus::Ok
        };
        let provenances = records
            .iter()
            .map(|record| record.provenance.clone())
            .collect::<Vec<_>>();
        let provenance_refs = provenances.iter().collect::<Vec<_>>();
        let mut answer = self
            .context()
            .answer(status, Some(records), provenance_refs, Vec::new());
        if status == AnswerStatus::Unknown {
            answer
                .uncertainty
                .push("no reviewed work-kind description matched".into());
        }
        answer
    }
}

fn normalize_waza(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}
