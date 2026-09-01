//! Validated, read-only player-state snapshots for the Palworld guide.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;

pub const SNAPSHOT_SCHEMA_VERSION: &str = "state_snapshot_v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotSourceKind {
    UserEntered,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotEvidenceKind {
    Observed,
    UserEntered,
    SaveDerived,
    Assumed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotConsent {
    pub id: String,
    pub scope: Vec<String>,
    pub granted_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotSource {
    pub kind: String,
    pub captured_at: DateTime<Utc>,
    pub game_version: String,
    pub time_to_live_seconds: Option<u64>,
    pub consent: SnapshotConsent,
}

impl SnapshotSource {
    pub fn kind(&self) -> Option<SnapshotSourceKind> {
        match self.kind.as_str() {
            "user_entered" => Some(SnapshotSourceKind::UserEntered),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InventoryState {
    pub item: String,
    pub quantity: u32,
    pub evidence: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PartyMemberState {
    pub slot: u8,
    pub pal: String,
    pub evidence: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnlockedTechnologyState {
    pub technology: String,
    pub evidence: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapturedPalState {
    pub pal: String,
    pub level: u8,
    pub evidence: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlayerLevelState {
    pub value: u8,
    pub evidence: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlayerGoal {
    pub kind: String,
    pub target: String,
    pub quantity: u32,
    pub priority: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlayerPreferences {
    pub spoiler_level: String,
    pub long_horizon: bool,
    pub preferred_activities: Vec<String>,
    pub avoided_activities: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlayerStateSnapshot {
    pub schema_version: String,
    pub source: SnapshotSource,
    pub inventory: Option<Vec<InventoryState>>,
    pub party: Option<Vec<PartyMemberState>>,
    pub unlocked_technologies: Option<Vec<UnlockedTechnologyState>>,
    pub captured_pals: Option<Vec<CapturedPalState>>,
    pub player_level: Option<PlayerLevelState>,
    pub goals: Option<Vec<PlayerGoal>>,
    pub preferences: Option<PlayerPreferences>,
}

impl PlayerStateSnapshot {
    pub fn from_json(value: &Value) -> Result<Self, String> {
        let mut snapshot: Self = serde_json::from_value(value.clone())
            .map_err(|error| error.to_string().replace(char::from(96), ""))?;
        if snapshot.schema_version != SNAPSHOT_SCHEMA_VERSION {
            return Err(format!(
                "unsupported snapshot schema version {}",
                snapshot.schema_version
            ));
        }
        snapshot.normalize();
        Ok(snapshot)
    }

    fn normalize(&mut self) {
        self.schema_version = self.schema_version.trim().to_string();
        self.source.kind = self.source.kind.trim().to_string();
        self.source.game_version = self.source.game_version.trim().to_string();
        self.source.consent.id = self.source.consent.id.trim().to_string();
        normalize_strings(self.source.consent.scope.iter_mut());
        if let Some(inventory) = &mut self.inventory {
            for entry in inventory {
                entry.item = entry.item.trim().to_string();
                entry.evidence = entry.evidence.trim().to_string();
            }
        }
        if let Some(party) = &mut self.party {
            for member in party {
                member.pal = member.pal.trim().to_string();
                member.evidence = member.evidence.trim().to_string();
            }
        }
        if let Some(technologies) = &mut self.unlocked_technologies {
            for technology in technologies {
                technology.technology = technology.technology.trim().to_string();
                technology.evidence = technology.evidence.trim().to_string();
            }
        }
        if let Some(pals) = &mut self.captured_pals {
            for pal in pals {
                pal.pal = pal.pal.trim().to_string();
                pal.evidence = pal.evidence.trim().to_string();
            }
        }
        if let Some(level) = &mut self.player_level {
            level.evidence = level.evidence.trim().to_string();
        }
        if let Some(goals) = &mut self.goals {
            for goal in goals {
                goal.kind = goal.kind.trim().to_string();
                goal.target = goal.target.trim().to_string();
            }
        }
        if let Some(preferences) = &mut self.preferences {
            preferences.spoiler_level = preferences.spoiler_level.trim().to_string();
            normalize_strings(preferences.preferred_activities.iter_mut());
            normalize_strings(preferences.avoided_activities.iter_mut());
        }
    }
}

fn normalize_strings<'a>(values: impl Iterator<Item = &'a mut String>) {
    for value in values {
        *value = value.trim().to_lowercase();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotValidation {
    pub valid: bool,
    pub errors: Vec<String>,
    pub missing_fields: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SnapshotValidator;

impl SnapshotValidator {
    pub fn validate(
        &self,
        snapshot: &PlayerStateSnapshot,
        now: DateTime<Utc>,
    ) -> SnapshotValidation {
        let mut errors = Vec::new();
        let mut missing_fields = Vec::<String>::new();

        if snapshot.source.kind().is_none() {
            errors.push(format!(
                "unsupported source kind {}; only user_entered is enabled",
                snapshot.source.kind
            ));
        }
        if snapshot.source.captured_at > now {
            errors.push("capture time is in the future".to_string());
        }
        if snapshot.source.consent.granted_at > now {
            errors.push("consent grant time is in the future".to_string());
        }
        if snapshot.source.consent.id.is_empty() {
            errors.push("consent id must be a non-empty local identifier".to_string());
        }
        if snapshot.source.consent.scope.is_empty() {
            errors.push("consent scope must contain at least one activity".to_string());
        }
        if snapshot.source.game_version.is_empty() {
            errors.push("source game version must not be empty".to_string());
        }
        if let Some(ttl) = snapshot.source.time_to_live_seconds {
            if ttl == 0 || ttl > 2_592_000 {
                errors.push("time to live must be between 1 and 2592000 seconds".to_string());
            }
        }

        let mut inventory_items = BTreeSet::new();
        if let Some(inventory) = &snapshot.inventory {
            for entry in inventory {
                if entry.item.is_empty() {
                    errors.push("inventory item must not be empty".to_string());
                }
                if entry.quantity == 0 {
                    errors.push(format!(
                        "inventory quantity must be greater than zero for {}",
                        entry.item
                    ));
                }
                validate_evidence(&entry.evidence, &mut errors);
                if !inventory_items.insert(normalized_key(&entry.item)) {
                    errors.push(format!("duplicate inventory item {}", entry.item));
                }
            }
        } else {
            missing_fields.push("inventory".to_string());
        }

        let mut party_slots = BTreeSet::new();
        if let Some(party) = &snapshot.party {
            for member in party {
                if member.pal.is_empty() {
                    errors.push("party Pal name must not be empty".to_string());
                }
                if member.slot > 4 {
                    errors.push("party slot must be between 0 and 4".to_string());
                }
                validate_evidence(&member.evidence, &mut errors);
                if !party_slots.insert(member.slot) {
                    errors.push(format!("duplicate party slot {}", member.slot));
                }
            }
        } else {
            missing_fields.push("party".to_string());
        }

        if let Some(technologies) = &snapshot.unlocked_technologies {
            let mut names = BTreeSet::new();
            for technology in technologies {
                if technology.technology.is_empty() {
                    errors.push("unlocked technology name must not be empty".to_string());
                }
                validate_evidence(&technology.evidence, &mut errors);
                if !names.insert(normalized_key(&technology.technology)) {
                    errors.push(format!(
                        "duplicate unlocked technology {}",
                        technology.technology
                    ));
                }
            }
        } else {
            missing_fields.push("unlocked_technologies".to_string());
        }

        if let Some(pals) = &snapshot.captured_pals {
            for pal in pals {
                if pal.pal.is_empty() {
                    errors.push("captured Pal name must not be empty".to_string());
                }
                if pal.level == 0 || pal.level > 60 {
                    errors.push(format!(
                        "Pal level for {} must be between 1 and 60",
                        pal.pal
                    ));
                }
                validate_evidence(&pal.evidence, &mut errors);
            }
        } else {
            missing_fields.push("captured_pals".to_string());
        }

        if let Some(level) = &snapshot.player_level {
            if level.value == 0 || level.value > 60 {
                errors.push("player level must be between 1 and 60".to_string());
            }
            validate_evidence(&level.evidence, &mut errors);
        } else {
            missing_fields.push("player_level".to_string());
        }

        if let Some(goals) = &snapshot.goals {
            let mut goal_keys = BTreeSet::new();
            for goal in goals {
                if !matches!(goal.kind.as_str(), "craft" | "progression" | "pal_work") {
                    errors.push(format!("unsupported goal kind {}", goal.kind));
                }
                if goal.target.is_empty() {
                    errors.push("goal target must not be empty".to_string());
                }
                if goal.quantity == 0 {
                    errors.push(format!(
                        "goal quantity for {} must be greater than zero",
                        goal.target
                    ));
                }
                if goal.priority == 0 || goal.priority > 5 {
                    errors.push("goal priority must be between 1 and 5".to_string());
                }
                if !goal_keys.insert((goal.kind.clone(), normalized_key(&goal.target))) {
                    errors.push(format!("duplicate goal {} {}", goal.kind, goal.target));
                }
            }
        } else {
            missing_fields.push("goals".to_string());
        }

        if let Some(preferences) = &snapshot.preferences {
            if !matches!(
                preferences.spoiler_level.as_str(),
                "none" | "minimal" | "mechanics" | "progression"
            ) {
                errors.push(format!(
                    "unsupported spoiler level {}",
                    preferences.spoiler_level
                ));
            }
            validate_activities(&preferences.preferred_activities, "preferred", &mut errors);
            validate_activities(&preferences.avoided_activities, "avoided", &mut errors);
            let preferred = preferences
                .preferred_activities
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>();
            let avoided = preferences
                .avoided_activities
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>();
            let conflicts = preferred.intersection(&avoided).collect::<Vec<_>>();
            if !conflicts.is_empty() {
                errors.push(format!(
                    "conflicting preference activities: {}",
                    conflicts
                        .into_iter()
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
        } else {
            missing_fields.push("preferences".to_string());
        }

        missing_fields.sort_unstable();
        let valid = errors.is_empty();
        SnapshotValidation {
            valid,
            errors,
            missing_fields,
        }
    }
}

fn validate_evidence(evidence: &str, errors: &mut Vec<String>) {
    if !matches!(
        evidence,
        "observed" | "user_entered" | "save_derived" | "assumed"
    ) {
        errors.push(format!("unsupported evidence kind {evidence}"));
    }
}

fn validate_activities(activities: &[String], label: &str, errors: &mut Vec<String>) {
    let mut unique = BTreeSet::new();
    for activity in activities {
        if !matches!(
            activity.as_str(),
            "combat"
                | "gathering"
                | "crafting"
                | "building"
                | "automation"
                | "breeding"
                | "exploration"
        ) {
            errors.push(format!("unsupported {label} activity {activity}"));
        }
        if !unique.insert(activity.clone()) {
            errors.push(format!("duplicate {label} activity {activity}"));
        }
    }
}

fn normalized_key(value: &str) -> String {
    value.trim().to_lowercase()
}
