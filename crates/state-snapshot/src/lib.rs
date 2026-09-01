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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotFreshness {
    Fresh,
    Stale,
    UnknownTtl,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotCompleteness {
    pub missing_fields: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InventorySummary {
    pub item: String,
    pub quantity: u32,
    pub evidence: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartyMemberSummary {
    pub slot: u8,
    pub pal: String,
    pub evidence: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnlockedTechnologySummary {
    pub technology: String,
    pub evidence: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapturedPalSummary {
    pub pal: String,
    pub level: u8,
    pub evidence: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerLevelSummary {
    pub value: u8,
    pub evidence: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoalSummary {
    pub kind: String,
    pub target: String,
    pub quantity: u32,
    pub priority: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreferenceSummary {
    pub spoiler_level: String,
    pub long_horizon: bool,
    pub preferred_activities: Vec<String>,
    pub avoided_activities: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotSummary {
    pub schema_version: String,
    pub source_kind: String,
    pub game_version: String,
    pub evidence_kinds: BTreeSet<String>,
    pub freshness: SnapshotFreshness,
    pub missing_fields: Vec<String>,
    pub inventory: Option<Vec<InventorySummary>>,
    pub party: Option<Vec<PartyMemberSummary>>,
    pub unlocked_technologies: Option<Vec<UnlockedTechnologySummary>>,
    pub captured_pals: Option<Vec<CapturedPalSummary>>,
    pub player_level: Option<PlayerLevelSummary>,
    pub goals: Option<Vec<GoalSummary>>,
    pub preferences: Option<PreferenceSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotSummaryOptions {
    question: String,
    include_all: bool,
}

impl SnapshotSummaryOptions {
    pub fn for_question(question: &str) -> Self {
        Self {
            question: question.trim().to_lowercase(),
            include_all: false,
        }
    }

    pub fn include_all() -> Self {
        Self {
            question: String::new(),
            include_all: true,
        }
    }
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

    pub fn freshness(&self, now: DateTime<Utc>) -> SnapshotFreshness {
        let Some(ttl) = self.source.time_to_live_seconds else {
            return SnapshotFreshness::UnknownTtl;
        };
        let age = now
            .signed_duration_since(self.source.captured_at)
            .num_seconds();
        if age >= 0 && age <= ttl as i64 {
            SnapshotFreshness::Fresh
        } else {
            SnapshotFreshness::Stale
        }
    }

    pub fn completeness(&self) -> SnapshotCompleteness {
        SnapshotCompleteness {
            missing_fields: missing_state_fields(self),
        }
    }

    pub fn summarize(
        &self,
        _question: &str,
        options: &SnapshotSummaryOptions,
        now: DateTime<Utc>,
    ) -> SnapshotSummary {
        let mut evidence_kinds = BTreeSet::new();
        let inventory = self.inventory.as_ref().and_then(|inventory| {
            if !options.include_all && !question_needs_inventory(options) {
                return None;
            }
            let mut summaries = inventory
                .iter()
                .map(|entry| {
                    evidence_kinds.insert(entry.evidence.clone());
                    InventorySummary {
                        item: entry.item.clone(),
                        quantity: entry.quantity,
                        evidence: entry.evidence.clone(),
                    }
                })
                .collect::<Vec<_>>();
            summaries.sort_by(|left, right| left.item.cmp(&right.item));
            Some(summaries)
        });
        let party = self.party.as_ref().and_then(|party| {
            if !options.include_all && !question_needs_party(options, party) {
                return None;
            }
            let mut summaries = party
                .iter()
                .map(|member| {
                    evidence_kinds.insert(member.evidence.clone());
                    PartyMemberSummary {
                        slot: member.slot,
                        pal: member.pal.clone(),
                        evidence: member.evidence.clone(),
                    }
                })
                .collect::<Vec<_>>();
            summaries.sort_by_key(|member| member.slot);
            Some(summaries)
        });
        let unlocked_technologies = self.unlocked_technologies.as_ref().and_then(|entries| {
            if !options.include_all && !question_needs_technologies(options) {
                return None;
            }
            let mut summaries = entries
                .iter()
                .map(|entry| {
                    evidence_kinds.insert(entry.evidence.clone());
                    UnlockedTechnologySummary {
                        technology: entry.technology.clone(),
                        evidence: entry.evidence.clone(),
                    }
                })
                .collect::<Vec<_>>();
            summaries.sort_by(|left, right| left.technology.cmp(&right.technology));
            Some(summaries)
        });
        let captured_pals = self.captured_pals.as_ref().and_then(|pals| {
            if !options.include_all && !question_needs_captured_pals(options, pals) {
                return None;
            }
            let mut summaries = pals
                .iter()
                .map(|pal| {
                    evidence_kinds.insert(pal.evidence.clone());
                    CapturedPalSummary {
                        pal: pal.pal.clone(),
                        level: pal.level,
                        evidence: pal.evidence.clone(),
                    }
                })
                .collect::<Vec<_>>();
            summaries.sort_by(|left, right| left.pal.cmp(&right.pal));
            Some(summaries)
        });
        let player_level = self.player_level.as_ref().and_then(|level| {
            if !options.include_all && !question_needs_player_level(options) {
                return None;
            }
            evidence_kinds.insert(level.evidence.clone());
            Some(PlayerLevelSummary {
                value: level.value,
                evidence: level.evidence.clone(),
            })
        });
        let goals = self.goals.as_ref().and_then(|goals| {
            if !options.include_all && !question_needs_goals(options) {
                return None;
            }
            Some(
                goals
                    .iter()
                    .map(|goal| GoalSummary {
                        kind: goal.kind.clone(),
                        target: goal.target.clone(),
                        quantity: goal.quantity,
                        priority: goal.priority,
                    })
                    .collect(),
            )
        });
        let preferences = self.preferences.as_ref().and_then(|preferences| {
            if !options.include_all && !question_needs_preferences(options) {
                return None;
            }
            Some(PreferenceSummary {
                spoiler_level: preferences.spoiler_level.clone(),
                long_horizon: preferences.long_horizon,
                preferred_activities: preferences.preferred_activities.clone(),
                avoided_activities: preferences.avoided_activities.clone(),
            })
        });

        SnapshotSummary {
            schema_version: self.schema_version.clone(),
            source_kind: self.source.kind.clone(),
            game_version: self.source.game_version.clone(),
            evidence_kinds,
            freshness: self.freshness(now),
            missing_fields: missing_state_fields(self),
            inventory,
            party,
            unlocked_technologies,
            captured_pals,
            player_level,
            goals,
            preferences,
        }
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

fn missing_state_fields(snapshot: &PlayerStateSnapshot) -> Vec<String> {
    let mut fields = Vec::new();
    if snapshot.inventory.is_none() {
        fields.push("inventory".to_string());
    }
    if snapshot.party.is_none() {
        fields.push("party".to_string());
    }
    if snapshot.unlocked_technologies.is_none() {
        fields.push("unlocked_technologies".to_string());
    }
    if snapshot.captured_pals.is_none() {
        fields.push("captured_pals".to_string());
    }
    if snapshot.player_level.is_none() {
        fields.push("player_level".to_string());
    }
    if snapshot.goals.is_none() {
        fields.push("goals".to_string());
    }
    if snapshot.preferences.is_none() {
        fields.push("preferences".to_string());
    }
    fields.sort_unstable();
    fields
}

fn question_needs_inventory(options: &SnapshotSummaryOptions) -> bool {
    options.question.contains("inventory")
        || options.question.contains("have")
        || options.question.contains("craft")
        || options.question.contains("material")
        || options.question.contains("wood")
}

fn question_needs_party(options: &SnapshotSummaryOptions, party: &[PartyMemberState]) -> bool {
    options.question.contains("party")
        || options.question.contains("work")
        || options.question.contains("pal")
        || party
            .iter()
            .any(|member| options.question.contains(&member.pal.to_lowercase()))
}

fn question_needs_technologies(options: &SnapshotSummaryOptions) -> bool {
    options.question.contains("unlock") || options.question.contains("technology")
}

fn question_needs_captured_pals(
    options: &SnapshotSummaryOptions,
    pals: &[CapturedPalState],
) -> bool {
    options.question.contains("roster")
        || options.question.contains("captured")
        || pals
            .iter()
            .any(|pal| options.question.contains(&pal.pal.to_lowercase()))
}

fn question_needs_player_level(options: &SnapshotSummaryOptions) -> bool {
    options.question.contains("player level") || options.question.contains("my level")
}

fn question_needs_goals(options: &SnapshotSummaryOptions) -> bool {
    options.question.contains("goal") || options.question.contains("next")
}

fn question_needs_preferences(options: &SnapshotSummaryOptions) -> bool {
    options.question.contains("preference") || options.question.contains("spoiler")
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
