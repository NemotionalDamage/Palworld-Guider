//! Typed tool registry around the deterministic guide core.

use guide_core::{
    AnswerStatus, EntityKind, GuideAnswer, GuideEngine, InventoryEntry, ProvenanceSummary,
    Resolution, VersionInfo,
};
use guide_planner::{GuidePlanner, PlannerAnswer, PlannerStatus};
use knowledge_index::{IndexSearchAnswer, IndexStatus, KnowledgeIndex, ProvenanceBrief};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use state_snapshot::{
    PlayerStateSnapshot, SnapshotFreshness, SnapshotSummaryOptions, SnapshotValidator,
};
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolStatus {
    Ok,
    Unknown,
    Ambiguous,
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolEnvelope {
    pub status: ToolStatus,
    pub data: Option<Value>,
    pub provenance: Vec<ProvenanceSummary>,
    pub version: VersionInfo,
    pub uncertainty: Vec<String>,
    pub errors: Vec<String>,
}

impl ToolEnvelope {
    fn from_answer<T: Serialize>(answer: GuideAnswer<T>) -> Self {
        let status = match answer.status {
            AnswerStatus::Ok => ToolStatus::Ok,
            AnswerStatus::Unknown => ToolStatus::Unknown,
            AnswerStatus::Ambiguous => ToolStatus::Ambiguous,
            AnswerStatus::Error => ToolStatus::Error,
        };
        Self {
            status,
            data: answer
                .data
                .as_ref()
                .map(|data| serde_json::to_value(data).unwrap_or(Value::Null)),
            provenance: answer.provenance,
            version: answer.version,
            uncertainty: answer.uncertainty,
            errors: answer.errors,
        }
    }

    fn from_search(answer: IndexSearchAnswer) -> Self {
        let status = match answer.status {
            IndexStatus::Ok => ToolStatus::Ok,
            IndexStatus::Unknown => ToolStatus::Unknown,
        };
        let provenance = answer
            .results
            .iter()
            .map(|result| provenance_summary(&result.provenance))
            .collect::<Vec<_>>();
        Self {
            status,
            data: Some(json!({"results": answer.results})),
            provenance,
            version: VersionInfo {
                knowledge_version: answer.version.knowledge_version,
                configured_game_version: answer.version.configured_game_version,
                matches: answer.version.matches,
            },
            uncertainty: answer.uncertainty,
            errors: Vec::new(),
        }
    }

    fn error(message: impl Into<String>, version: VersionInfo) -> Self {
        Self {
            status: ToolStatus::Error,
            data: None,
            provenance: Vec::new(),
            version,
            uncertainty: Vec::new(),
            errors: vec![message.into()],
        }
    }

    fn unknown(message: impl Into<String>, version: VersionInfo) -> Self {
        Self {
            status: ToolStatus::Unknown,
            data: None,
            provenance: Vec::new(),
            version,
            uncertainty: vec![message.into()],
            errors: Vec::new(),
        }
    }

    fn ok(data: Value, version: VersionInfo) -> Self {
        Self {
            status: ToolStatus::Ok,
            data: Some(data),
            provenance: Vec::new(),
            version,
            uncertainty: Vec::new(),
            errors: Vec::new(),
        }
    }

    fn from_planner<T: Serialize>(answer: PlannerAnswer<T>) -> Self {
        let status = match answer.status {
            PlannerStatus::Ok => ToolStatus::Ok,
            PlannerStatus::Unknown => ToolStatus::Unknown,
            PlannerStatus::Ambiguous => ToolStatus::Ambiguous,
            PlannerStatus::Error => ToolStatus::Error,
        };
        Self {
            status,
            data: answer
                .data
                .as_ref()
                .map(|data| serde_json::to_value(data).unwrap_or(Value::Null)),
            provenance: answer.provenance,
            version: answer.version,
            uncertainty: answer.uncertainty,
            errors: answer.errors,
        }
    }
}

fn provenance_summary(brief: &ProvenanceBrief) -> ProvenanceSummary {
    ProvenanceSummary {
        source_id: brief.source_id.clone(),
        applicable_game_version: brief.applicable_game_version.clone(),
        retrieved_on: brief.retrieved_on.clone(),
        reviewer: brief.reviewer.clone(),
        review_status: brief.review_status.clone(),
        confidence: brief.confidence.clone(),
        change_risk: brief.change_risk.clone(),
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters_schema: Value,
}

pub struct ToolBudget {
    max_calls: usize,
    deadline: Instant,
    used: usize,
}

impl ToolBudget {
    pub fn new(max_calls: usize, deadline: Instant) -> Self {
        Self {
            max_calls,
            deadline,
            used: 0,
        }
    }

    fn consume(&mut self) -> Result<(), String> {
        if Instant::now() > self.deadline {
            return Err("tool deadline exceeded".to_string());
        }
        if self.used >= self.max_calls {
            return Err("tool call budget exhausted".to_string());
        }
        self.used += 1;
        Ok(())
    }
}

pub struct ToolRegistry {
    engine: GuideEngine,
    index: KnowledgeIndex,
    state_snapshot: Option<PlayerStateSnapshot>,
    definitions: Vec<ToolDefinition>,
}

impl ToolRegistry {
    pub fn new(engine: GuideEngine, index: KnowledgeIndex) -> Self {
        Self {
            engine,
            index,
            state_snapshot: None,
            definitions: build_definitions(),
        }
    }

    pub fn with_state_snapshot(mut self, snapshot: PlayerStateSnapshot) -> Self {
        self.state_snapshot = Some(snapshot);
        self
    }

    pub fn with_state_snapshot_json(mut self, value: &Value) -> Result<Self, String> {
        let snapshot = PlayerStateSnapshot::from_json(value)?;
        let validation = SnapshotValidator.validate(&snapshot, chrono::Utc::now());
        if !validation.valid {
            return Err(validation.errors.join("; "));
        }
        self.state_snapshot = Some(snapshot);
        Ok(self)
    }

    pub fn definitions(&self) -> &[ToolDefinition] {
        &self.definitions
    }

    pub fn dispatch(&self, name: &str, arguments: &Value, budget: &mut ToolBudget) -> ToolEnvelope {
        let version = self.base_version();
        if !self
            .definitions
            .iter()
            .any(|definition| definition.name == name)
        {
            return ToolEnvelope::error(format!("unknown tool: {name}"), version);
        }
        if let Err(message) = budget.consume() {
            return ToolEnvelope::error(message, version);
        }
        self.execute(name, arguments, version)
    }

    pub fn base_version(&self) -> VersionInfo {
        let version = self.index.version();
        VersionInfo {
            knowledge_version: version.knowledge_version,
            configured_game_version: version.configured_game_version,
            matches: version.matches,
        }
    }

    pub fn known_entity_names(&self) -> std::collections::BTreeSet<String> {
        let mut names = std::collections::BTreeSet::new();
        for item in self.engine.store().items() {
            names.insert(item.names.en.clone());
            if let Some(name) = &item.names.zh_hans {
                names.insert(name.clone());
            }
        }
        for pal in self.engine.store().pals() {
            names.insert(pal.names.en.clone());
            if let Some(name) = &pal.names.zh_hans {
                names.insert(name.clone());
            }
        }
        for technology in self.engine.store().technologies() {
            names.insert(technology.names.en.clone());
            if let Some(name) = &technology.names.zh_hans {
                names.insert(name.clone());
            }
        }
        for alias in self.engine.store().aliases() {
            names.insert(alias.alias.clone());
        }
        names.retain(|name| name.chars().count() >= 3);
        names
    }

    pub fn canonical_entity_names(&self) -> std::collections::BTreeMap<String, String> {
        let mut names = std::collections::BTreeMap::new();
        for item in self.engine.store().items() {
            names.insert(item.id.clone(), item.names.en.clone());
        }
        for pal in self.engine.store().pals() {
            names.insert(pal.id.clone(), pal.names.en.clone());
        }
        for technology in self.engine.store().technologies() {
            names.insert(technology.id.clone(), technology.names.en.clone());
        }
        names
    }

    fn execute(&self, name: &str, arguments: &Value, version: VersionInfo) -> ToolEnvelope {
        match name {
            "resolve_name" => self.resolve_name(arguments, version),
            "get_item" => match require_string(arguments, "query") {
                Ok(query) => ToolEnvelope::from_answer(self.engine.lookup_item(&query)),
                Err(message) => ToolEnvelope::error(message, version),
            },
            "get_pal" => match require_string(arguments, "query") {
                Ok(query) => ToolEnvelope::from_answer(self.engine.lookup_pal(&query)),
                Err(message) => ToolEnvelope::error(message, version),
            },
            "get_recipe" => match require_string(arguments, "query") {
                Ok(query) => ToolEnvelope::from_answer(self.engine.lookup_recipe(&query)),
                Err(message) => ToolEnvelope::error(message, version),
            },
            "get_technology" => match require_string(arguments, "query") {
                Ok(query) => ToolEnvelope::from_answer(self.engine.lookup_technology(&query)),
                Err(message) => ToolEnvelope::error(message, version),
            },
            "search_structured_knowledge" => self.search(arguments, version),
            "calculate_materials" => match self.material_arguments(arguments) {
                Ok((query, quantity)) => {
                    ToolEnvelope::from_answer(self.engine.calculate_materials(&query, quantity))
                }
                Err(message) => ToolEnvelope::error(message, version),
            },
            "calculate_shortage" => self.shortage(arguments, version),
            "calculate_craftable_count" => self.craftable(arguments, version),
            "calculate_breeding_result" => match (
                require_string(arguments, "parent_a"),
                require_string(arguments, "parent_b"),
            ) {
                (Ok(parent_a), Ok(parent_b)) => ToolEnvelope::from_answer(
                    self.engine.calculate_breeding_result(&parent_a, &parent_b),
                ),
                (first, second) => ToolEnvelope::error(
                    first
                        .err()
                        .or(second.err())
                        .unwrap_or_else(|| "invalid breeding arguments".to_string()),
                    version,
                ),
            },
            "calculate_breeding_chain" => match (
                require_string(arguments, "parent_a"),
                require_string(arguments, "parent_b"),
                require_bounded_integer(arguments, "maximum_depth", 1, 10),
            ) {
                (Ok(parent_a), Ok(parent_b), Ok(maximum_depth)) => {
                    ToolEnvelope::from_answer(self.engine.calculate_breeding_chain(
                        &parent_a,
                        &parent_b,
                        maximum_depth as usize,
                    ))
                }
                (first, second, third) => ToolEnvelope::error(
                    first
                        .err()
                        .or(second.err())
                        .or(third.err())
                        .unwrap_or_else(|| "invalid breeding arguments".to_string()),
                    version,
                ),
            },
            "get_conflicting_records" => self.conflicts(version),
            "import_player_snapshot" => self.import_player_snapshot(arguments, version),
            "analyze_inventory" => self.analyze_inventory(version),
            "analyze_party" => self.analyze_party(version),
            "suggest_next_goals" => self.suggest_next_goals(version),
            _ => ToolEnvelope::error(format!("unknown tool: {name}"), version),
        }
    }

    fn import_player_snapshot(&self, arguments: &Value, version: VersionInfo) -> ToolEnvelope {
        if let Err(message) = require_string(arguments, "confirmation") {
            return ToolEnvelope::error(message, version);
        }
        let Some(snapshot) = &self.state_snapshot else {
            return ToolEnvelope::unknown("no player state snapshot is configured", version);
        };
        let summary = snapshot.summarize(
            "player state summary",
            &SnapshotSummaryOptions::include_all(),
            chrono::Utc::now(),
        );
        let mut envelope = ToolEnvelope::ok(
            serde_json::to_value(summary).unwrap_or(Value::Null),
            version,
        );
        if snapshot.freshness(chrono::Utc::now()) != SnapshotFreshness::Fresh {
            envelope.status = ToolStatus::Unknown;
            envelope.uncertainty.push("snapshot is stale".to_string());
        }
        envelope
    }

    fn analyze_inventory(&self, version: VersionInfo) -> ToolEnvelope {
        let Some(snapshot) = &self.state_snapshot else {
            return ToolEnvelope::unknown("no player state snapshot is configured", version);
        };
        let answer = GuidePlanner::new(self.engine.clone())
            .analyze(snapshot, chrono::Utc::now())
            .map_data(|analysis| analysis.map(|analysis| analysis.inventory_gap));
        ToolEnvelope::from_planner(answer)
    }

    fn analyze_party(&self, version: VersionInfo) -> ToolEnvelope {
        let Some(snapshot) = &self.state_snapshot else {
            return ToolEnvelope::unknown("no player state snapshot is configured", version);
        };
        let answer = GuidePlanner::new(self.engine.clone())
            .analyze(snapshot, chrono::Utc::now())
            .map_data(|analysis| analysis.map(|analysis| analysis.party_work));
        ToolEnvelope::from_planner(answer)
    }

    fn suggest_next_goals(&self, version: VersionInfo) -> ToolEnvelope {
        let Some(snapshot) = &self.state_snapshot else {
            return ToolEnvelope::unknown("no player state snapshot is configured", version);
        };
        ToolEnvelope::from_planner(
            GuidePlanner::new(self.engine.clone()).recommend(snapshot, chrono::Utc::now()),
        )
    }

    fn resolve_name(&self, arguments: &Value, version: VersionInfo) -> ToolEnvelope {
        let query = match require_string(arguments, "query") {
            Ok(query) => query,
            Err(message) => return ToolEnvelope::error(message, version),
        };
        let kind = match optional_entity_kind(arguments) {
            Ok(kind) => kind,
            Err(message) => return ToolEnvelope::error(message, version),
        };
        match self.engine.resolve(&query, kind) {
            Resolution::Unique(resolved) => ToolEnvelope::ok(
                json!({
                    "id": resolved.id,
                    "kind": entity_kind_value(resolved.kind),
                    "matched_name": resolved.matched_name,
                }),
                version,
            ),
            Resolution::Ambiguous(candidates) => {
                let mut envelope = ToolEnvelope::ok(json!({"candidates": candidates}), version);
                envelope.status = ToolStatus::Ambiguous;
                envelope.uncertainty.push(format!(
                    "ambiguous name; candidates: {}",
                    candidates
                        .iter()
                        .map(|candidate| candidate.id.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
                envelope
            }
            Resolution::Unknown => {
                ToolEnvelope::unknown("unknown name; no reviewed record matches", version)
            }
        }
    }

    fn search(&self, arguments: &Value, version: VersionInfo) -> ToolEnvelope {
        let query = match require_string(arguments, "query") {
            Ok(query) => query,
            Err(message) => return ToolEnvelope::error(message, version),
        };
        let limit = match optional_bounded_integer(arguments, "limit", 1, 20) {
            Ok(limit) => limit.unwrap_or(5) as usize,
            Err(message) => return ToolEnvelope::error(message, version),
        };
        ToolEnvelope::from_search(self.index.search(&query, limit))
    }

    fn material_arguments(&self, arguments: &Value) -> Result<(String, u32), String> {
        let query = require_string(arguments, "query")?;
        let quantity = require_bounded_integer(arguments, "quantity", 1, u32::MAX as u64)? as u32;
        Ok((query, quantity))
    }

    fn shortage(&self, arguments: &Value, version: VersionInfo) -> ToolEnvelope {
        match self
            .material_arguments(arguments)
            .and_then(|(query, quantity)| {
                parse_inventory(arguments).map(|inventory| (query, quantity, inventory))
            }) {
            Ok((query, quantity, inventory)) => ToolEnvelope::from_answer(
                self.engine.calculate_shortage(&query, quantity, &inventory),
            ),
            Err(message) => ToolEnvelope::error(message, version),
        }
    }

    fn craftable(&self, arguments: &Value, version: VersionInfo) -> ToolEnvelope {
        match require_string(arguments, "query")
            .and_then(|query| parse_inventory(arguments).map(|inventory| (query, inventory)))
        {
            Ok((query, inventory)) => {
                ToolEnvelope::from_answer(self.engine.calculate_craftable_count(&query, &inventory))
            }
            Err(message) => ToolEnvelope::error(message, version),
        }
    }

    fn conflicts(&self, version: VersionInfo) -> ToolEnvelope {
        let conflicts = self.engine.store().conflicts();
        if conflicts.is_empty() {
            return ToolEnvelope::unknown(
                "no conflicting records in the reviewed knowledge base",
                version,
            );
        }
        let provenance = conflicts
            .iter()
            .map(|conflict| ProvenanceSummary::new(&conflict.provenance))
            .collect::<Vec<_>>();
        let mut envelope = ToolEnvelope::ok(json!({"conflicts": conflicts}), version);
        envelope.provenance = provenance;
        envelope
    }
}

fn build_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "resolve_name".to_string(),
            description: "Resolve an exact ID, English name, or reviewed alias to one entity."
                .to_string(),
            parameters_schema: json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string"},
                    "kind": {"type": "string", "enum": ["item", "pal", "technology", "recipe", "habitat", "breeding_rule", "alias", "progression_relationship", "conflict"]}
                },
                "required": ["query"]
            }),
        },
        ToolDefinition {
            name: "get_item".to_string(),
            description: "Look up an item with acquisition, crafting, and Pal-drop relationships."
                .to_string(),
            parameters_schema: object_query(),
        },
        ToolDefinition {
            name: "get_pal".to_string(),
            description: "Look up a Pal with stats, work suitability, drops, and habitats."
                .to_string(),
            parameters_schema: object_query(),
        },
        ToolDefinition {
            name: "get_recipe".to_string(),
            description: "Look up a reviewed recipe by its output item.".to_string(),
            parameters_schema: object_query(),
        },
        ToolDefinition {
            name: "get_technology".to_string(),
            description: "Look up a technology and the recipes it unlocks.".to_string(),
            parameters_schema: object_query(),
        },
        ToolDefinition {
            name: "search_structured_knowledge".to_string(),
            description: "Lexically search deterministic structured summaries with provenance."
                .to_string(),
            parameters_schema: json!({
                "type": "object",
                "properties": {"query": {"type": "string"}, "limit": {"type": "integer", "minimum": 1, "maximum": 20}},
                "required": ["query"]
            }),
        },
        ToolDefinition {
            name: "calculate_materials".to_string(),
            description: "Calculate a deterministic recursive material tree and raw totals."
                .to_string(),
            parameters_schema: json!({
                "type": "object",
                "properties": {"query": {"type": "string"}, "quantity": {"type": "integer", "minimum": 1}},
                "required": ["query", "quantity"]
            }),
        },
        ToolDefinition {
            name: "calculate_shortage".to_string(),
            description: "Calculate deterministic material shortages for an inventory.".to_string(),
            parameters_schema: json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string"},
                    "quantity": {"type": "integer", "minimum": 1},
                    "inventory": inventory_schema()
                },
                "required": ["query", "quantity"]
            }),
        },
        ToolDefinition {
            name: "calculate_craftable_count".to_string(),
            description: "Calculate the maximum additional craftable count for an inventory."
                .to_string(),
            parameters_schema: json!({
                "type": "object",
                "properties": {"query": {"type": "string"}, "inventory": inventory_schema()},
                "required": ["query"]
            }),
        },
        ToolDefinition {
            name: "calculate_breeding_result".to_string(),
            description: "Look up the reviewed breeding result for two parents.".to_string(),
            parameters_schema: json!({
                "type": "object",
                "properties": {"parent_a": {"type": "string"}, "parent_b": {"type": "string"}},
                "required": ["parent_a", "parent_b"]
            }),
        },
        ToolDefinition {
            name: "calculate_breeding_chain".to_string(),
            description: "Find a bounded deterministic breeding chain between two Pals."
                .to_string(),
            parameters_schema: json!({
                "type": "object",
                "properties": {
                    "parent_a": {"type": "string"},
                    "parent_b": {"type": "string"},
                    "maximum_depth": {"type": "integer", "minimum": 1, "maximum": 10}
                },
                "required": ["parent_a", "parent_b", "maximum_depth"]
            }),
        },
        ToolDefinition {
            name: "get_conflicting_records".to_string(),
            description: "Return all unresolved or resolved knowledge conflicts.".to_string(),
            parameters_schema: json!({"type": "object", "properties": {}, "required": []}),
        },
        ToolDefinition {
            name: "import_player_snapshot".to_string(),
            description: "Confirm use of an explicitly supplied, validated player-state snapshot and return its redacted summary.".to_string(),
            parameters_schema: json!({
                "type": "object",
                "properties": {"confirmation": {"type": "string", "enum": ["user_entered"]}},
                "required": ["confirmation"]
            }),
        },
        ToolDefinition {
            name: "analyze_inventory".to_string(),
            description: "Analyze the attached player snapshot for inventory gaps against reviewed goals.".to_string(),
            parameters_schema: json!({"type": "object", "properties": {}, "required": []}),
        },
        ToolDefinition {
            name: "analyze_party".to_string(),
            description: "Analyze the attached player snapshot for party work-suitability coverage and gaps.".to_string(),
            parameters_schema: json!({"type": "object", "properties": {}, "required": []}),
        },
        ToolDefinition {
            name: "suggest_next_goals".to_string(),
            description: "Return three to five deterministic, explained recommendations from the attached snapshot.".to_string(),
            parameters_schema: json!({"type": "object", "properties": {}, "required": []}),
        },
    ]
}

fn object_query() -> Value {
    json!({
        "type": "object",
        "properties": {"query": {"type": "string"}},
        "required": ["query"]
    })
}

fn inventory_schema() -> Value {
    json!({
        "type": "array",
        "items": {
            "type": "object",
            "properties": {"item": {"type": "string"}, "quantity": {"type": "integer", "minimum": 0}},
            "required": ["item", "quantity"]
        }
    })
}

fn require_string(arguments: &Value, name: &str) -> Result<String, String> {
    let value = arguments
        .get(name)
        .ok_or_else(|| format!("missing required argument \"{name}\""))?;
    let string = value
        .as_str()
        .ok_or_else(|| format!("argument \"{name}\" must be a string"))?;
    if string.trim().is_empty() {
        return Err(format!("argument \"{name}\" must be a non-empty string"));
    }
    Ok(string.trim().to_string())
}

fn require_bounded_integer(
    arguments: &Value,
    name: &str,
    minimum: u64,
    maximum: u64,
) -> Result<u64, String> {
    let value = arguments
        .get(name)
        .ok_or_else(|| format!("missing required argument \"{name}\""))?;
    let integer = value
        .as_u64()
        .ok_or_else(|| format!("argument \"{name}\" must be a non-negative integer"))?;
    if integer < minimum || integer > maximum {
        return Err(format!(
            "argument \"{name}\" must be between {minimum} and {maximum}"
        ));
    }
    Ok(integer)
}

fn optional_bounded_integer(
    arguments: &Value,
    name: &str,
    minimum: u64,
    maximum: u64,
) -> Result<Option<u64>, String> {
    if arguments.get(name).is_none() {
        return Ok(None);
    }
    require_bounded_integer(arguments, name, minimum, maximum).map(Some)
}

fn optional_entity_kind(arguments: &Value) -> Result<Option<EntityKind>, String> {
    let Some(value) = arguments.get("kind") else {
        return Ok(None);
    };
    let kind = value
        .as_str()
        .ok_or_else(|| "argument \"kind\" must be a string".to_string())?;
    match kind {
        "item" => Ok(Some(EntityKind::Item)),
        "pal" => Ok(Some(EntityKind::Pal)),
        "technology" => Ok(Some(EntityKind::Technology)),
        "recipe" => Ok(Some(EntityKind::Recipe)),
        "habitat" => Ok(Some(EntityKind::Habitat)),
        "breeding_rule" => Ok(Some(EntityKind::BreedingRule)),
        "alias" => Ok(Some(EntityKind::Alias)),
        "progression_relationship" => Ok(Some(EntityKind::ProgressionRelationship)),
        "conflict" => Ok(Some(EntityKind::Conflict)),
        _ => Err("argument \"kind\" must be a supported entity kind".to_string()),
    }
}

fn entity_kind_value(kind: EntityKind) -> Value {
    serde_json::to_value(kind).unwrap_or(Value::Null)
}

fn parse_inventory(arguments: &Value) -> Result<Vec<InventoryEntry>, String> {
    let Some(value) = arguments.get("inventory") else {
        return Ok(Vec::new());
    };
    let entries = value
        .as_array()
        .ok_or_else(|| "argument \"inventory\" must be an array".to_string())?;
    let mut inventory = Vec::new();
    for entry in entries {
        let item = entry
            .get("item")
            .and_then(Value::as_str)
            .filter(|item| !item.trim().is_empty())
            .ok_or_else(|| "inventory entries require a non-empty \"item\"".to_string())?;
        let quantity = entry
            .get("quantity")
            .and_then(Value::as_u64)
            .filter(|quantity| *quantity <= u32::MAX as u64)
            .ok_or_else(|| {
                "inventory quantities must be integers from 0 to 4294967295".to_string()
            })?;
        inventory.push(InventoryEntry::new(item.trim(), quantity as u32));
    }
    Ok(inventory)
}
