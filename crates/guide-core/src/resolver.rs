use crate::GuideEngine;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    Item,
    Pal,
    Technology,
    Recipe,
    Habitat,
    BreedingRule,
    Alias,
    ProgressionRelationship,
    Conflict,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedEntity {
    pub id: String,
    pub kind: EntityKind,
    pub matched_name: String,
}

pub enum Resolution {
    Unique(ResolvedEntity),
    Ambiguous(Vec<ResolvedEntity>),
    Unknown,
}

fn normalize(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

impl GuideEngine {
    pub fn resolve(&self, query: &str, kind: Option<EntityKind>) -> Resolution {
        let store = self.store();

        let exact_id = |kind: EntityKind| {
            let resolved = match kind {
                EntityKind::Item => store
                    .item(query)
                    .map(|record| (record.id.clone(), record.names.en.clone(), EntityKind::Item)),
                EntityKind::Pal => store
                    .pal(query)
                    .map(|record| (record.id.clone(), record.names.en.clone(), EntityKind::Pal)),
                EntityKind::Technology => store.technology(query).map(|record| {
                    (
                        record.id.clone(),
                        record.names.en.clone(),
                        EntityKind::Technology,
                    )
                }),
                EntityKind::Recipe => store.recipe(query).map(|record| {
                    (
                        record.id.clone(),
                        record.output.item_id.clone(),
                        EntityKind::Recipe,
                    )
                }),
                _ => None,
            };
            resolved.map(|(id, matched_name, kind)| ResolvedEntity {
                id,
                kind,
                matched_name,
            })
        };

        if let Some(kind) = kind {
            if let Some(resolved) = exact_id(kind) {
                return Resolution::Unique(resolved);
            }
        }

        let normalized = normalize(query);
        if normalized.is_empty() {
            return Resolution::Unknown;
        }
        let mut matches = Vec::new();
        if kind == Some(EntityKind::Recipe) {
            let output_matches = self.recipe_output_matches(query, &normalized);
            if output_matches.len() == 1 {
                return Resolution::Unique(
                    output_matches
                        .into_iter()
                        .next()
                        .expect("one output match is present"),
                );
            }
            if output_matches.len() > 1 {
                return Resolution::Ambiguous(output_matches);
            }
        }
        if kind.is_none_or(|expected| expected == EntityKind::Item) {
            for record in store.items() {
                if normalize(&record.id) == normalized || normalize(&record.names.en) == normalized
                {
                    matches.push(ResolvedEntity {
                        id: record.id.clone(),
                        kind: EntityKind::Item,
                        matched_name: record.names.en.clone(),
                    });
                }
            }
        }
        if kind.is_none_or(|expected| expected == EntityKind::Pal) {
            for record in store.pals() {
                if normalize(&record.id) == normalized || normalize(&record.names.en) == normalized
                {
                    matches.push(ResolvedEntity {
                        id: record.id.clone(),
                        kind: EntityKind::Pal,
                        matched_name: record.names.en.clone(),
                    });
                }
            }
        }
        if kind.is_none_or(|expected| expected == EntityKind::Technology) {
            for record in store.technologies() {
                if normalize(&record.id) == normalized || normalize(&record.names.en) == normalized
                {
                    matches.push(ResolvedEntity {
                        id: record.id.clone(),
                        kind: EntityKind::Technology,
                        matched_name: record.names.en.clone(),
                    });
                }
            }
        }

        let mut alias_matches = Vec::new();
        for alias in store.aliases() {
            if normalize(&alias.alias) != normalized {
                continue;
            }
            if let Some(expected) = kind {
                if self.entity_kind(&alias.target_id) != Some(expected) {
                    continue;
                }
            }
            alias_matches.push(ResolvedEntity {
                id: alias.target_id.clone(),
                kind: self
                    .entity_kind(&alias.target_id)
                    .unwrap_or(EntityKind::Alias),
                matched_name: alias.alias.clone(),
            });
        }
        matches.extend(alias_matches);

        matches.sort_by(|left, right| (left.kind, &left.id).cmp(&(right.kind, &right.id)));
        matches.dedup_by(|left, right| left.id == right.id && left.kind == right.kind);

        match matches.len() {
            0 => Resolution::Unknown,
            1 => Resolution::Unique(matches.into_iter().next().expect("one match is present")),
            _ => Resolution::Ambiguous(matches),
        }
    }

    fn recipe_output_matches(&self, query: &str, normalized: &str) -> Vec<ResolvedEntity> {
        let item_ids = self
            .store()
            .items()
            .filter(|item| {
                normalize(&item.id) == normalized
                    || normalize(&item.names.en) == normalized
                    || self.store().aliases().any(|alias| {
                        alias.target_id == item.id && normalize(&alias.alias) == normalized
                    })
            })
            .map(|item| item.id.clone())
            .collect::<Vec<_>>();
        self.store()
            .recipes()
            .filter(|recipe| item_ids.contains(&recipe.output.item_id))
            .map(|recipe| ResolvedEntity {
                id: recipe.id.clone(),
                kind: EntityKind::Recipe,
                matched_name: query.to_string(),
            })
            .collect()
    }

    pub(crate) fn entity_kind(&self, id: &str) -> Option<EntityKind> {
        let store = self.store();
        if store.item(id).is_some() {
            Some(EntityKind::Item)
        } else if store.pal(id).is_some() {
            Some(EntityKind::Pal)
        } else if store.technology(id).is_some() {
            Some(EntityKind::Technology)
        } else if store.recipe(id).is_some() {
            Some(EntityKind::Recipe)
        } else {
            None
        }
    }
}
