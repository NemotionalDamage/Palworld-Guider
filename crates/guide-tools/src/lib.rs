//! Typed tool registry around the deterministic guide core.

use game_knowledge::LocaleNames;
use guide_core::{
    describe_acquisition_leads, map_display_to_world, world_to_map_display, AnswerStatus,
    EntityKind, GuideAnswer, GuideEngine, InventoryEntry, MapDisplayCoordinate, MapPointKind,
    NearbyMapPoint, ProvenanceSummary, Resolution, TravelAnchorSeed, VersionInfo, WorldCoordinate,
};
use guide_planner::{GuidePlanner, PlannerAnswer, PlannerStatus};
use knowledge_index::{IndexSearchAnswer, IndexStatus, KnowledgeIndex, ProvenanceBrief};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use state_snapshot::{PlayerStateSnapshot, SnapshotFreshness, SnapshotValidator};
use std::sync::Arc;
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

    fn from_runtime(result: RuntimeToolResult, version: VersionInfo) -> Self {
        Self {
            status: result.status,
            data: result.data,
            provenance: Vec::new(),
            version,
            uncertainty: result.uncertainty,
            errors: result.errors,
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

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeToolResult {
    pub status: ToolStatus,
    pub data: Option<Value>,
    pub uncertainty: Vec<String>,
    pub errors: Vec<String>,
}

pub trait RuntimeToolSource: Send + Sync {
    fn definitions(&self) -> Vec<ToolDefinition>;
    fn dispatch(&self, name: &str, arguments: &Value) -> RuntimeToolResult;
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
    runtime_tools: Option<Arc<dyn RuntimeToolSource>>,
}

impl ToolRegistry {
    pub fn new(engine: GuideEngine, index: KnowledgeIndex) -> Self {
        Self {
            engine,
            index,
            state_snapshot: None,
            definitions: build_definitions(),
            runtime_tools: None,
        }
    }

    pub fn with_state_snapshot(mut self, snapshot: PlayerStateSnapshot) -> Self {
        self.state_snapshot = Some(snapshot);
        self
    }

    pub fn set_state_snapshot(&mut self, snapshot: PlayerStateSnapshot) {
        self.state_snapshot = Some(snapshot);
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

    pub fn definitions(&self) -> Vec<ToolDefinition> {
        let mut definitions = self.definitions.clone();
        if let Some(runtime) = &self.runtime_tools {
            definitions.extend(runtime.definitions());
        }
        definitions
    }

    pub fn try_with_runtime_tools(
        mut self,
        runtime: Arc<dyn RuntimeToolSource>,
    ) -> Result<Self, String> {
        for definition in runtime.definitions() {
            if self
                .definitions
                .iter()
                .any(|static_definition| static_definition.name == definition.name)
            {
                return Err(format!("duplicate runtime tool: {}", definition.name));
            }
        }
        self.runtime_tools = Some(runtime);
        Ok(self)
    }

    pub fn dispatch(&self, name: &str, arguments: &Value, budget: &mut ToolBudget) -> ToolEnvelope {
        let version = self.base_version();
        if let Some(runtime) = &self.runtime_tools {
            if runtime
                .definitions()
                .iter()
                .any(|definition| definition.name == name)
            {
                if let Err(message) = budget.consume() {
                    return ToolEnvelope::error(message, version);
                }
                return ToolEnvelope::from_runtime(runtime.dispatch(name, arguments), version);
            }
        }
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

    pub fn known_waza_names(&self) -> std::collections::BTreeSet<String> {
        let mut names = std::collections::BTreeSet::new();
        for waza in self.engine.store().waza() {
            names.insert(waza.names.en.clone());
            if let Some(name) = &waza.names.zh_hans {
                names.insert(name.clone());
            }
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

    pub fn entity_name_variants_by_id(
        &self,
    ) -> std::collections::BTreeMap<String, std::collections::BTreeSet<String>> {
        let mut variants = std::collections::BTreeMap::new();
        let mut insert = |id: &str, name: &str| {
            if name.chars().count() >= 2 {
                variants
                    .entry(id.to_string())
                    .or_insert_with(std::collections::BTreeSet::new)
                    .insert(name.to_string());
            }
        };
        for item in self.engine.store().items() {
            insert(&item.id, &item.names.en);
            if let Some(name) = &item.names.zh_hans {
                insert(&item.id, name);
            }
        }
        for pal in self.engine.store().pals() {
            insert(&pal.id, &pal.names.en);
            if let Some(name) = &pal.names.zh_hans {
                insert(&pal.id, name);
            }
        }
        for technology in self.engine.store().technologies() {
            insert(&technology.id, &technology.names.en);
            if let Some(name) = &technology.names.zh_hans {
                insert(&technology.id, name);
            }
        }
        for alias in self.engine.store().aliases() {
            insert(&alias.target_id, &alias.alias);
        }
        variants
    }

    fn execute(&self, name: &str, arguments: &Value, version: VersionInfo) -> ToolEnvelope {
        match name {
            "resolve_name" => self.resolve_name(arguments, version),
            "get_item" => match require_string(arguments, "query")
                .and_then(|query| optional_rarity(arguments).map(|rarity| (query, rarity)))
            {
                Ok((query, rarity)) => ToolEnvelope::from_answer(
                    self.engine.lookup_item_filtered(&query, rarity.as_deref()),
                ),
                Err(message) => ToolEnvelope::error(message, version),
            },
            "get_pal" => match require_string(arguments, "query") {
                Ok(query) => ToolEnvelope::from_answer(self.engine.lookup_pal(&query)),
                Err(message) => ToolEnvelope::error(message, version),
            },
            "get_recipe" => match require_string(arguments, "query")
                .and_then(|query| optional_rarity(arguments).map(|rarity| (query, rarity)))
            {
                Ok((query, rarity)) => ToolEnvelope::from_answer(
                    self.engine
                        .lookup_recipe_filtered(&query, rarity.as_deref()),
                ),
                Err(message) => ToolEnvelope::error(message, version),
            },
            "get_technology" => match require_string(arguments, "query") {
                Ok(query) => ToolEnvelope::from_answer(self.engine.lookup_technology(&query)),
                Err(message) => ToolEnvelope::error(message, version),
            },
            "get_type_effectiveness" => {
                match optional_string(arguments, "attacking_type").and_then(|attacking| {
                    optional_string(arguments, "defending_type")
                        .map(|defending| (attacking, defending))
                }) {
                    Ok((attacking, defending)) => ToolEnvelope::from_answer(
                        self.engine
                            .get_type_effectiveness(attacking.as_deref(), defending.as_deref()),
                    ),
                    Err(message) => ToolEnvelope::error(message, version),
                }
            }
            "get_waza" => match require_string(arguments, "query") {
                Ok(query) => ToolEnvelope::from_answer(self.engine.lookup_waza(&query)),
                Err(message) => ToolEnvelope::error(message, version),
            },
            "get_pal_waza_unlocks" => match require_string(arguments, "pal") {
                Ok(pal) => ToolEnvelope::from_answer(self.engine.get_pal_waza_unlocks(&pal)),
                Err(message) => ToolEnvelope::error(message, version),
            },
            "get_work_kind_descriptions" => match optional_string(arguments, "query") {
                Ok(query) => ToolEnvelope::from_answer(
                    self.engine.get_work_kind_descriptions(query.as_deref()),
                ),
                Err(message) => ToolEnvelope::error(message, version),
            },
            "locate_coordinate" => match self.coordinate(arguments) {
                Ok(location) => ToolEnvelope::from_answer(self.engine.locate_coordinate(location)),
                Err(message) => ToolEnvelope::error(message, version),
            },
            "find_nearby_map_points" => self.nearby_map_points(arguments, version),
            "find_pal_spawn_zones" => self.pal_spawn_zones(arguments, version),
            "plan_travel_route" => self.plan_travel_route(arguments, version),
            "search_structured_knowledge" => self.search(arguments, version),
            "calculate_materials" => match self.material_arguments(arguments) {
                Ok((query, quantity, rarity)) => ToolEnvelope::from_answer(
                    self.engine
                        .calculate_materials_filtered(&query, quantity, rarity.as_deref()),
                ),
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
        let now = chrono::Utc::now();
        let freshness = snapshot.freshness(now);
        let mut envelope = ToolEnvelope::ok(
            json!({
                "source_kind": snapshot.source.kind,
                "game_version": snapshot.source.game_version,
                "freshness": freshness,
                "missing_fields": snapshot.completeness().missing_fields
            }),
            version,
        );
        if freshness != SnapshotFreshness::Fresh {
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

    fn material_arguments(
        &self,
        arguments: &Value,
    ) -> Result<(String, u32, Option<String>), String> {
        let query = require_string(arguments, "query")?;
        let quantity = require_bounded_integer(arguments, "quantity", 1, u32::MAX as u64)? as u32;
        let rarity = optional_rarity(arguments)?;
        Ok((query, quantity, rarity))
    }

    fn shortage(&self, arguments: &Value, version: VersionInfo) -> ToolEnvelope {
        match self
            .material_arguments(arguments)
            .and_then(|(query, quantity, rarity)| {
                parse_inventory(arguments).map(|inventory| (query, quantity, rarity, inventory))
            }) {
            Ok((query, quantity, rarity, inventory)) => {
                ToolEnvelope::from_answer(self.engine.calculate_shortage_filtered(
                    &query,
                    quantity,
                    &inventory,
                    rarity.as_deref(),
                ))
            }
            Err(message) => ToolEnvelope::error(message, version),
        }
    }

    fn craftable(&self, arguments: &Value, version: VersionInfo) -> ToolEnvelope {
        match require_string(arguments, "query").and_then(|query| {
            optional_rarity(arguments).and_then(|rarity| {
                parse_inventory(arguments).map(|inventory| (query, rarity, inventory))
            })
        }) {
            Ok((query, rarity, inventory)) => {
                ToolEnvelope::from_answer(self.engine.calculate_craftable_count_filtered(
                    &query,
                    &inventory,
                    rarity.as_deref(),
                ))
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

    fn coordinate(&self, arguments: &Value) -> Result<WorldCoordinate, String> {
        let location = coordinate(arguments)?;
        let system = arguments
            .get("coordinate_system")
            .and_then(Value::as_str)
            .unwrap_or("world");
        if !matches!(system, "world" | "map_pixel" | "normalized" | "map_display") {
            return Err(
                "argument \"coordinate_system\" must be world, map_pixel, normalized, or map_display"
                    .to_string(),
            );
        }
        if system == "world" {
            return Ok(location);
        }
        if system == "map_display" {
            let mut world = map_display_to_world(MapDisplayCoordinate {
                x: location.x,
                y: location.y,
            });
            world.z = location.z;
            return Ok(world);
        }

        let requested_map_id = arguments.get("map_id").and_then(Value::as_str);
        let mut candidates = self
            .engine
            .store()
            .maps()
            .filter(|map| requested_map_id.is_none_or(|map_id| map.id == map_id))
            .peekable();
        if candidates.peek().is_none() {
            return Err("argument \"map_id\" does not match a reviewed map".to_string());
        }

        let selected = candidates.min_by_key(|map| (map.priority, map.id.clone()));
        let Some(map) = selected else {
            return Err("no reviewed map matches the coordinate".to_string());
        };
        let width = map.bounds.max_x - map.bounds.min_x;
        let height = map.bounds.max_y - map.bounds.min_y;
        let (normalized_x, normalized_y) = if system == "map_pixel" {
            let logical_size = f64::from(map.logical_size);
            if !(0.0..=logical_size).contains(&location.x)
                || !(0.0..=logical_size).contains(&location.y)
            {
                return Err(format!(
                    "map_pixel coordinates must be within 0..{logical_size}"
                ));
            }
            (location.x / logical_size, location.y / logical_size)
        } else if !(0.0..=1.0).contains(&location.x) || !(0.0..=1.0).contains(&location.y) {
            return Err("normalized coordinates must be within 0..1".to_string());
        } else {
            (location.x, location.y)
        };

        Ok(WorldCoordinate {
            x: map.bounds.min_x + normalized_x * width,
            y: map.bounds.min_y + normalized_y * height,
            z: location.z,
        })
    }

    fn nearby_map_points(&self, arguments: &Value, version: VersionInfo) -> ToolEnvelope {
        let result: Result<_, String> = (|| {
            let location = self.coordinate_or_player(arguments)?;
            let limit = optional_bounded_integer(arguments, "limit", 1, 20)?.unwrap_or(5) as usize;
            Ok((location, optional_map_point_kind(arguments)?, limit))
        })();
        match result {
            Ok((location, kind, limit)) => {
                let mut answer = self
                    .engine
                    .find_nearby_map_points(location, kind, 20.max(limit));
                if answer.status == AnswerStatus::Ok {
                    if let Some(points) = answer.data.as_mut() {
                        if kind.is_none_or(|filter| filter == MapPointKind::FastTravel) {
                            points.extend(
                                self.available_base_camp_anchors()
                                    .into_iter()
                                    .map(|anchor| nearby_base_camp(&location, &anchor)),
                            );
                        }
                        points.sort_by(|left, right| {
                            left.distance_map_display
                                .total_cmp(&right.distance_map_display)
                                .then_with(|| left.id.cmp(&right.id))
                        });
                        points.truncate(limit);
                    }
                }
                ToolEnvelope::from_answer(answer)
            }
            Err(message) => ToolEnvelope::error(message, version),
        }
    }

    fn pal_spawn_zones(&self, arguments: &Value, version: VersionInfo) -> ToolEnvelope {
        let result: Result<_, String> = (|| {
            let query = require_string(arguments, "pal")?;
            let location = self.coordinate(arguments)?;
            let limit = optional_bounded_integer(arguments, "limit", 1, 20)?.unwrap_or(5) as usize;
            Ok((query, location, limit))
        })();
        match result {
            Ok((pal, location, limit)) => {
                ToolEnvelope::from_answer(self.engine.find_pal_spawn_zones(&pal, location, limit))
            }
            Err(message) => ToolEnvelope::error(message, version),
        }
    }

    fn plan_travel_route(&self, arguments: &Value, version: VersionInfo) -> ToolEnvelope {
        let result: Result<_, String> = (|| {
            let from = match arguments.get("from") {
                Some(from) => self.coordinate(from)?,
                None => self.runtime_player_position()?,
            };
            let (to, destination_name) = self.travel_destination(arguments, from)?;
            let extra_anchors = self.travel_extra_anchors(arguments)?;
            Ok((from, to, destination_name, extra_anchors))
        })();
        match result {
            Ok((from, to, destination_name, extra_anchors)) => ToolEnvelope::from_answer(
                self.engine
                    .plan_travel_route(from, to, destination_name, &extra_anchors),
            ),
            Err(message) => ToolEnvelope::error(message, version),
        }
    }

    fn travel_destination(
        &self,
        arguments: &Value,
        from: WorldCoordinate,
    ) -> Result<(WorldCoordinate, Option<String>), String> {
        if arguments.get("to").is_some() {
            return self
                .coordinate(require_object(arguments, "to")?)
                .map(|to| (to, None));
        }
        let query = require_string(arguments, "to_query")?;
        if let Some(anchor) = self.resolve_base_camp_destination(&query, from) {
            return Ok((anchor.location, Some(anchor.name)));
        }
        self.resolve_travel_destination(&query, from)
            .map(|(to, name)| (to, Some(name)))
    }

    fn resolve_base_camp_destination(
        &self,
        query: &str,
        from: WorldCoordinate,
    ) -> Option<TravelAnchorSeed> {
        let anchors = self.available_base_camp_anchors();
        if anchors.is_empty() {
            return None;
        }
        let normalize = |value: &str| {
            value
                .chars()
                .filter(|character| character.is_alphanumeric())
                .flat_map(char::to_lowercase)
                .collect::<String>()
        };
        let normalized_query = normalize(query);
        if matches!(
            normalized_query.as_str(),
            "据点" | "玩家据点" | "我的据点" | "basecamp" | "playerbasecamp" | "mybasecamp"
        ) {
            return anchors.into_iter().min_by(|left, right| {
                distance_map_display(&from, &left.location)
                    .total_cmp(&distance_map_display(&from, &right.location))
                    .then_with(|| left.id.cmp(&right.id))
            });
        }
        anchors.into_iter().find(|anchor| {
            normalize(&anchor.id) == normalized_query || normalize(&anchor.name) == normalized_query
        })
    }

    fn resolve_travel_destination(
        &self,
        query: &str,
        from: WorldCoordinate,
    ) -> Result<(WorldCoordinate, String), String> {
        let normalize = |value: &str| {
            value
                .chars()
                .filter(|character| character.is_alphanumeric())
                .flat_map(char::to_lowercase)
                .collect::<String>()
        };
        let normalized_query = normalize(query);
        let map_points = self
            .engine
            .store()
            .map_points()
            .filter(|point| {
                normalize(&point.id) == normalized_query
                    || normalize(&point.native_id) == normalized_query
                    || normalize(&point.names.en) == normalized_query
                    || point
                        .names
                        .zh_hans
                        .as_deref()
                        .is_some_and(|name| normalize(name) == normalized_query)
            })
            .cloned()
            .collect::<Vec<_>>();
        if map_points.len() == 1 {
            let point = &map_points[0];
            return Ok((point.location, point.names.en.clone()));
        }
        if map_points.len() > 1 {
            let ids = map_points
                .iter()
                .map(|point| point.id.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(format!("ambiguous map point; candidates: {ids}"));
        }

        match self.engine.resolve(query, Some(EntityKind::Pal)) {
            Resolution::Unique(resolved) => {
                let Some(pal) = self.engine.store().pal(&resolved.id) else {
                    return Err("resolved Pal is absent from the store".to_string());
                };
                let zones = pal
                    .habitat_ids
                    .iter()
                    .filter_map(|zone_id| self.engine.store().pal_habitat_zone(zone_id))
                    .min_by(|left, right| {
                        let left_distance =
                            (left.location.x - from.x).hypot(left.location.y - from.y);
                        let right_distance =
                            (right.location.x - from.x).hypot(right.location.y - from.y);
                        left_distance
                            .total_cmp(&right_distance)
                            .then_with(|| left.id.cmp(&right.id))
                    });
                match zones {
                    Some(zone) => Ok((zone.location, resolved.matched_name)),
                    None => {
                        let leads = describe_acquisition_leads(&pal.habitat_leads);
                        if leads.is_empty() {
                            Err(format!("Pal \"{query}\" has no reviewed habitat coverage"))
                        } else {
                            Err(format!(
                                "Pal \"{query}\" has no fixed field habitat; habitat leads: {leads}"
                            ))
                        }
                    }
                }
            }
            Resolution::Ambiguous(candidates) => {
                let ids = candidates
                    .iter()
                    .map(|candidate| candidate.id.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                Err(format!("ambiguous Pal; candidates: {ids}"))
            }
            Resolution::Unknown => {
                Err("unknown destination; no reviewed map point or Pal matches".to_string())
            }
        }
    }

    fn travel_extra_anchors(&self, arguments: &Value) -> Result<Vec<TravelAnchorSeed>, String> {
        let mut anchors = Vec::new();
        if let Some(extra_anchors) = arguments.get("extra_anchors") {
            let items = extra_anchors.as_array().ok_or_else(|| {
                "argument \"extra_anchors\" must be an array of world-coordinate anchors"
                    .to_string()
            })?;
            for item in items {
                anchors.push(travel_anchor_seed(item)?);
            }
        }
        let include_base_camps = arguments
            .get("via_base_camps")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        if include_base_camps {
            anchors.extend(self.available_base_camp_anchors());
        }
        Ok(anchors)
    }

    fn coordinate_or_player(&self, arguments: &Value) -> Result<WorldCoordinate, String> {
        if arguments.get("x").is_some() || arguments.get("y").is_some() {
            return self.coordinate(arguments);
        }
        if arguments.get("coordinate_system").is_some() {
            return Err(
                "arguments \"x\" and \"y\" are required with coordinate_system".to_string(),
            );
        }
        self.runtime_player_position()
    }

    fn runtime_player_position(&self) -> Result<WorldCoordinate, String> {
        let runtime = self
            .runtime_tools
            .as_ref()
            .filter(|runtime| {
                runtime
                    .definitions()
                    .iter()
                    .any(|definition| definition.name == "get_player_status")
            })
            .ok_or_else(|| "observed player position is unavailable".to_string())?;
        let result = runtime.dispatch("get_player_status", &json!({}));
        if result.status != ToolStatus::Ok {
            return Err(result
                .errors
                .first()
                .cloned()
                .unwrap_or_else(|| "observed player position is unavailable".to_string()));
        }
        let position = result
            .data
            .and_then(|data| data.get("position").cloned())
            .ok_or_else(|| "get_player_status returned no position".to_string())?;
        world_coordinate(&position)
    }

    fn available_base_camp_anchors(&self) -> Vec<TravelAnchorSeed> {
        let Some(runtime) = self.runtime_tools.as_ref() else {
            return Vec::new();
        };
        if !runtime
            .definitions()
            .iter()
            .any(|definition| definition.name == "get_base_camps")
        {
            return Vec::new();
        }
        let result = runtime.dispatch("get_base_camps", &json!({}));
        if result.status != ToolStatus::Ok {
            return Vec::new();
        }
        result
            .data
            .and_then(|data| base_camp_anchors(&data).ok())
            .unwrap_or_default()
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
            description: "Look up an item with acquisition, crafting, and Pal-drop relationships. Pass rarity to choose one of several tiers that share a display name; a bare name answers the base tier, and acquisition_leads names any schematic the tier needs."
                .to_string(),
            parameters_schema: json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string"},
                    "rarity": {"type": "string", "enum": ["common", "uncommon", "rare", "epic", "legendary"]}
                },
                "required": ["query"]
            }),
        },
        ToolDefinition {
            name: "get_pal".to_string(),
            description: "Look up a Pal with stats, work suitability, drops, and habitats."
                .to_string(),
            parameters_schema: object_query(),
        },
        ToolDefinition {
            name: "get_recipe".to_string(),
            description: "Look up a reviewed recipe by its output item. Pass rarity for a higher tier, and read schematic_leads for the schematic that must be in inventory before that tier can be crafted. When an item has several reviewed recipes, alternative_recipes lists the rest, and every route must be reported."
                .to_string(),
            parameters_schema: json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string"},
                    "rarity": {"type": "string", "enum": ["common", "uncommon", "rare", "epic", "legendary"]}
                },
                "required": ["query"]
            }),
        },
        ToolDefinition {
            name: "get_technology".to_string(),
            description: "Look up a technology and the recipes it unlocks.".to_string(),
            parameters_schema: object_query(),
        },
        ToolDefinition {
            name: "get_type_effectiveness".to_string(),
            description: "Look up reviewed element type-effectiveness multipliers."
                .to_string(),
            parameters_schema: json!({
                "type": "object",
                "properties": {
                    "attacking_type": {"type": "string"},
                    "defending_type": {"type": "string"}
                },
                "required": []
            }),
        },
        ToolDefinition {
            name: "get_waza".to_string(),
            description: "Look up a reviewed Pal active skill by name or ID.".to_string(),
            parameters_schema: object_query(),
        },
        ToolDefinition {
            name: "get_pal_waza_unlocks".to_string(),
            description: "List reviewed active skills a Pal unlocks by level.".to_string(),
            parameters_schema: json!({
                "type": "object",
                "properties": {"pal": {"type": "string"}},
                "required": ["pal"]
            }),
        },
        ToolDefinition {
            name: "get_work_kind_descriptions".to_string(),
            description: "Explain a reviewed work-suitability kind or list all kinds."
                .to_string(),
            parameters_schema: json!({
                "type": "object",
                "properties": {"query": {"type": "string"}},
                "required": []
            }),
        },
        ToolDefinition {
            name: "locate_coordinate".to_string(),
            description: "Map a world, logical-map-pixel, normalized, or in-game map display coordinate to a reviewed logical map and, when the coordinate falls inside one, its reviewed region.".to_string(),
            parameters_schema: coordinate_schema(),
        },
        ToolDefinition {
            name: "find_nearby_map_points".to_string(),
            description: "Find reviewed fast-travel points and player base camps near a coordinate, or near the observed player when x/y are omitted. Use distance_map_display for player-facing map-coordinate distances.".to_string(),
            parameters_schema: json!({
                "type": "object",
                "properties": {
                    "x": {"type": "number"},
                    "y": {"type": "number"},
                    "z": {"type": "number"},
                    "coordinate_system": {"type": "string", "enum": ["world", "map_pixel", "normalized", "map_display"]},
                    "kind": {"type": "string", "enum": ["fast_travel", "boss_tower"]},
                    "limit": {"type": "integer", "minimum": 1, "maximum": 20}
                },
                "required": []
            }),
        },
        ToolDefinition {
            name: "plan_travel_route".to_string(),
            description: "Compare direct travel with fast-travel legs to a destination. Omit from to use the observed player position. to_query supports reviewed map points, Pal habitats, and player base camps (including the generic query base camp). Player base camps are included automatically when the read-only adapter provides them; set via_base_camps to false to exclude them.".to_string(),
            parameters_schema: json!({
                "type": "object",
                "properties": {
                    "from": coordinate_schema(),
                    "to": coordinate_schema(),
                    "to_query": {"type": "string"},
                    "via_base_camps": {"type": "boolean"},
                    "extra_anchors": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "id": {"type": "string"},
                                "name": {"type": "string"},
                                "x": {"type": "number"},
                                "y": {"type": "number"},
                                "z": {"type": "number"}
                            },
                            "required": ["x", "y"]
                        }
                    }
                },
                "required": ["from"]
            }),
        },
        ToolDefinition {
            name: "find_pal_spawn_zones".to_string(),
            description: "Rank reviewed target-build spawn zones for a Pal relative to a coordinate.".to_string(),
            parameters_schema: json!({
                "type": "object",
                "properties": {
                    "pal": {"type": "string"},
                    "x": {"type": "number"},
                    "y": {"type": "number"},
                    "z": {"type": "number"},
                    "coordinate_system": {"type": "string", "enum": ["world", "map_pixel", "normalized", "map_display"]},
                    "limit": {"type": "integer", "minimum": 1, "maximum": 20}
                },
                "required": ["pal", "x", "y"]
            }),
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
                "properties": {
                    "query": {"type": "string"},
                    "quantity": {"type": "integer", "minimum": 1},
                    "rarity": {"type": "string", "enum": ["common", "uncommon", "rare", "epic", "legendary"]}
                },
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
                    "rarity": {"type": "string", "enum": ["common", "uncommon", "rare", "epic", "legendary"]},
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
                "properties": {
                    "query": {"type": "string"},
                    "rarity": {"type": "string", "enum": ["common", "uncommon", "rare", "epic", "legendary"]},
                    "inventory": inventory_schema()
                },
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

fn coordinate_schema() -> Value {
    json!({
        "type": "object",
                "properties": {
                    "x": {"type": "number"},
                    "y": {"type": "number"},
                    "z": {"type": "number"},
                    "coordinate_system": {
                        "type": "string",
                        "enum": ["world", "map_pixel", "normalized", "map_display"],
                        "description": "Use world for Unreal world units, map_pixel for 0..logical_size pixels, normalized for 0..1 coordinates, or map_display for the in-game map display coordinates."
                    },
                    "map_id": {"type": "string"}
                },
        "required": ["x", "y"]
    })
}

fn coordinate(arguments: &Value) -> Result<WorldCoordinate, String> {
    let read_axis = |name: &str| -> Result<f64, String> {
        arguments
            .get(name)
            .and_then(Value::as_f64)
            .filter(|value| value.is_finite())
            .ok_or_else(|| format!("argument \"{name}\" must be a finite number"))
    };
    Ok(WorldCoordinate {
        x: read_axis("x")?,
        y: read_axis("y")?,
        z: arguments.get("z").and_then(Value::as_f64).unwrap_or(0.0),
    })
}

fn optional_map_point_kind(arguments: &Value) -> Result<Option<MapPointKind>, String> {
    let Some(value) = arguments.get("kind") else {
        return Ok(None);
    };
    match value.as_str() {
        Some("fast_travel") => Ok(Some(MapPointKind::FastTravel)),
        Some("boss_tower") => Ok(Some(MapPointKind::BossTower)),
        _ => Err("argument \"kind\" must be fast_travel or boss_tower".to_string()),
    }
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

fn require_object<'a>(arguments: &'a Value, name: &str) -> Result<&'a Value, String> {
    let value = arguments
        .get(name)
        .ok_or_else(|| format!("missing required argument \"{name}\""))?;
    value
        .as_object()
        .map(|_| value)
        .ok_or_else(|| format!("argument \"{name}\" must be an object"))
}

fn travel_anchor_seed(value: &Value) -> Result<TravelAnchorSeed, String> {
    let name = value
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("base_camp")
        .to_string();
    let id = value
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or(&name)
        .to_string();
    let location_value = value.get("location").unwrap_or(value);
    let read_axis = |axis: &str| -> Result<f64, String> {
        location_value
            .get(axis)
            .and_then(Value::as_f64)
            .filter(|number| number.is_finite())
            .ok_or_else(|| format!("anchor \"{axis}\" must be a finite number"))
    };
    Ok(TravelAnchorSeed {
        id,
        name,
        location: WorldCoordinate {
            x: read_axis("x")?,
            y: read_axis("y")?,
            z: location_value
                .get("z")
                .and_then(Value::as_f64)
                .unwrap_or(0.0),
        },
    })
}

fn base_camp_anchors(data: &Value) -> Result<Vec<TravelAnchorSeed>, String> {
    let items = data
        .get("base_camps")
        .and_then(Value::as_array)
        .or_else(|| data.as_array())
        .ok_or_else(|| "get_base_camps returned an unsupported shape".to_string())?;
    items.iter().map(travel_anchor_seed).collect()
}

fn nearby_base_camp(origin: &WorldCoordinate, anchor: &TravelAnchorSeed) -> NearbyMapPoint {
    NearbyMapPoint {
        id: anchor.id.clone(),
        name: anchor.name.clone(),
        names: LocaleNames {
            en: anchor.name.clone(),
            zh_hans: Some(anchor.name.clone()),
        },
        kind: MapPointKind::FastTravel,
        location: anchor.location,
        map_display: world_to_map_display(anchor.location),
        distance_map_display: distance_map_display(origin, &anchor.location),
        distance_meters: distance_meters(origin, &anchor.location),
        bearing_degrees: bearing_degrees(origin, &anchor.location),
    }
}

fn world_coordinate(value: &Value) -> Result<WorldCoordinate, String> {
    let read_axis = |axis: &str| -> Result<f64, String> {
        value
            .get(axis)
            .and_then(Value::as_f64)
            .filter(|number| number.is_finite())
            .ok_or_else(|| format!("position \"{axis}\" must be a finite number"))
    };
    Ok(WorldCoordinate {
        x: read_axis("x")?,
        y: read_axis("y")?,
        z: value.get("z").and_then(Value::as_f64).unwrap_or(0.0),
    })
}

fn distance_map_display(origin: &WorldCoordinate, target: &WorldCoordinate) -> f64 {
    let origin_display = world_to_map_display(*origin);
    let target_display = world_to_map_display(*target);
    (target_display.x - origin_display.x).hypot(target_display.y - origin_display.y)
}

fn distance_meters(origin: &WorldCoordinate, target: &WorldCoordinate) -> f64 {
    (target.x - origin.x).hypot(target.y - origin.y) / 100.0
}

fn bearing_degrees(origin: &WorldCoordinate, target: &WorldCoordinate) -> f64 {
    (target.x - origin.x)
        .atan2(target.y - origin.y)
        .to_degrees()
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

fn optional_string(arguments: &Value, name: &str) -> Result<Option<String>, String> {
    let Some(value) = arguments.get(name) else {
        return Ok(None);
    };
    let string = value
        .as_str()
        .ok_or_else(|| format!("argument \"{name}\" must be a string"))?;
    if string.trim().is_empty() {
        return Err(format!("argument \"{name}\" must be a non-empty string"));
    }
    Ok(Some(string.trim().to_string()))
}

fn optional_rarity(arguments: &Value) -> Result<Option<String>, String> {
    let Some(value) = arguments.get("rarity") else {
        return Ok(None);
    };
    let rarity = value
        .as_str()
        .ok_or_else(|| "argument \"rarity\" must be a string".to_string())?;
    match rarity.to_ascii_lowercase().as_str() {
        "common" | "uncommon" | "rare" | "epic" | "legendary" => Ok(Some(rarity.to_string())),
        _ => Err("argument \"rarity\" must be common, uncommon, rare, epic, or legendary".into()),
    }
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
