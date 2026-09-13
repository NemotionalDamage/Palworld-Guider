use crate::GuideEngine;
use game_knowledge::{ItemRecord, LocaleNames};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

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

pub(crate) fn normalize(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

impl GuideEngine {
    pub fn resolve(&self, query: &str, kind: Option<EntityKind>) -> Resolution {
        self.resolve_with_rarity(query, kind, None)
    }

    pub fn resolve_with_rarity(
        &self,
        query: &str,
        kind: Option<EntityKind>,
        rarity: Option<&str>,
    ) -> Resolution {
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
            let output_matches = self.recipe_output_matches(query, &normalized, rarity);
            if output_matches.len() == 1 {
                return Resolution::Unique(
                    output_matches
                        .into_iter()
                        .next()
                        .expect("one output match is present"),
                );
            }
            if let Some(base) = self.base_tier_match(&output_matches) {
                return Resolution::Unique(base);
            }
            if output_matches.len() > 1 {
                return Resolution::Ambiguous(output_matches);
            }
        }
        if kind.is_none_or(|expected| expected == EntityKind::Item) {
            for record in store.items() {
                if self.rarity_matches_item(&record.id, rarity)
                    && (normalize(&record.id) == normalized
                        || names_match(&record.names, normalized.as_str()))
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
                if normalize(&record.id) == normalized
                    || names_match(&record.names, normalized.as_str())
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
                if normalize(&record.id) == normalized
                    || names_match(&record.names, normalized.as_str())
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
            if !self.rarity_matches_item(&alias.target_id, rarity) {
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
            0 => self.retry_without_leading_rarity(query, kind, rarity),
            1 => Resolution::Unique(matches.into_iter().next().expect("one match is present")),
            _ => match self.base_tier_match(&matches) {
                Some(base) => Resolution::Unique(base),
                None => Resolution::Ambiguous(matches),
            },
        }
    }

    /// Callers often write the tier into the name ("Legendary Metal Armor"). The exact name is
    /// matched first, so an item really named "Legendary Sphere" keeps resolving; otherwise the
    /// leading rarity word is split off and the remainder is retried with that tier.
    fn retry_without_leading_rarity(
        &self,
        query: &str,
        kind: Option<EntityKind>,
        rarity: Option<&str>,
    ) -> Resolution {
        let Some((detected, rest)) = split_leading_rarity(query) else {
            return Resolution::Unknown;
        };
        self.resolve_with_rarity(&rest, kind, rarity.or(Some(detected)))
    }

    /// Rank variants of one item share a localized name, so one name can match several records.
    /// The base (lowest-rarity) tier answers a bare name; another tier is returned only when the
    /// caller asked for it, and a family whose tiers all sit at one rarity stays ambiguous.
    fn base_tier_match(&self, matches: &[ResolvedEntity]) -> Option<ResolvedEntity> {
        if matches.len() < 2 {
            return None;
        }
        let mut families = BTreeSet::new();
        let mut ranked = Vec::with_capacity(matches.len());
        for candidate in matches {
            let item = self.candidate_item(candidate)?;
            families.insert(normalize(&item.names.en));
            ranked.push((rarity_rank(&item.rarity)?, candidate));
        }
        if families.len() != 1 {
            return None;
        }
        let lowest = ranked.iter().map(|(rank, _)| *rank).min()?;
        let mut tiers = ranked.iter().filter(|(rank, _)| *rank == lowest);
        let (_, base) = tiers.next()?;
        if tiers.next().is_some() {
            return None;
        }
        Some((*base).clone())
    }

    fn candidate_item(&self, candidate: &ResolvedEntity) -> Option<&ItemRecord> {
        match candidate.kind {
            EntityKind::Item => self.store().item(&candidate.id),
            EntityKind::Recipe => self
                .store()
                .recipe(&candidate.id)
                .and_then(|recipe| self.store().item(&recipe.output.item_id)),
            _ => None,
        }
    }

    fn recipe_output_matches(
        &self,
        query: &str,
        normalized: &str,
        rarity: Option<&str>,
    ) -> Vec<ResolvedEntity> {
        let item_ids = self
            .store()
            .items()
            .filter(|item| {
                self.rarity_matches_item(&item.id, rarity)
                    && (normalize(&item.id) == normalized
                        || normalize(&item.names.en) == normalized
                        || self.store().aliases().any(|alias| {
                            alias.target_id == item.id && normalize(&alias.alias) == normalized
                        }))
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

    fn rarity_matches_item(&self, item_id: &str, rarity: Option<&str>) -> bool {
        let Some(rarity) = rarity else {
            return true;
        };
        self.store()
            .item(item_id)
            .is_some_and(|item| item.rarity.eq_ignore_ascii_case(rarity))
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

/// Rarity tiers run from the craftable base item up to the schematic-only tiers.
pub(crate) fn rarity_rank(rarity: &str) -> Option<u8> {
    match rarity.to_ascii_lowercase().as_str() {
        "common" => Some(0),
        "uncommon" => Some(1),
        "rare" => Some(2),
        "epic" => Some(3),
        "legendary" => Some(4),
        "mythic" => Some(5),
        _ => None,
    }
}

/// Splits a leading rarity word from a query, so "Legendary Metal Armor" can be read as the
/// legendary tier of "Metal Armor".
fn split_leading_rarity(query: &str) -> Option<(&'static str, String)> {
    let trimmed = query.trim_start();
    let (word, rest) = match trimmed.split_once(char::is_whitespace) {
        Some((word, rest)) => (word, rest),
        None => (trimmed, ""),
    };
    let rarity = match word
        .trim_matches(|character: char| !character.is_alphanumeric())
        .to_ascii_lowercase()
        .as_str()
    {
        "common" => "common",
        "uncommon" => "uncommon",
        "rare" => "rare",
        "epic" => "epic",
        "legendary" => "legendary",
        "mythic" => "mythic",
        _ => return None,
    };
    let rest = rest.trim();
    (!rest.is_empty()).then(|| (rarity, rest.to_string()))
}

fn names_match(names: &LocaleNames, normalized: &str) -> bool {
    normalize(&names.en) == normalized
        || names
            .zh_hans
            .as_ref()
            .is_some_and(|name| normalize(name) == normalized)
}
