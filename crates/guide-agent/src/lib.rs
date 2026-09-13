//! Bounded, grounded natural-language guide agent.

mod answer;

use answer::{contains_entity_phrase, truncate_characters, AnswerDraft, AnswerStatus, FactSheet};
use guide_core::{ProvenanceSummary, VersionInfo};
use guide_tools::{ToolBudget, ToolEnvelope, ToolRegistry, ToolStatus};
use provider::{ChatMessage, ChatProvider, ChatRequest, ProviderError, ToolRequest, ToolSpec};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use state_snapshot::PlayerStateSnapshot;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

pub struct AgentLimits {
    pub max_tool_calls: usize,
    pub timeout: Duration,
}

pub struct AgentConfig {
    pub limits: AgentLimits,
    pub max_reply_characters: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentHistoryEntry {
    pub question: String,
    pub answer: Option<String>,
}

impl AgentHistoryEntry {
    pub fn new(question: impl Into<String>, answer: Option<String>) -> Self {
        Self {
            question: question.into(),
            answer,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Ok,
    Unknown,
    Ambiguous,
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCallRecord {
    pub round: usize,
    pub id: String,
    pub name: String,
    pub arguments: Value,
    pub status: ToolStatus,
    pub data: Option<Value>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentAnswer {
    pub status: AgentStatus,
    pub answer: Option<String>,
    pub tool_calls: Vec<ToolCallRecord>,
    pub provenance: Vec<ProvenanceSummary>,
    pub version: VersionInfo,
    pub uncertainty: Vec<String>,
    pub errors: Vec<String>,
}

struct GroundingContext {
    records: Vec<ToolCallRecord>,
    provenance: BTreeSet<ProvenanceSummary>,
    uncertainty: Vec<String>,
    version: VersionInfo,
    summary: String,
}

pub struct GuideAgent {
    registry: ToolRegistry,
    provider: Box<dyn ChatProvider>,
    config: AgentConfig,
}

const SYSTEM_PROMPT: &str = "You are the Palworld Guider brain. Answer in the same language as the player's question. Answer the player from the supplied GROUNDING and whitelisted deterministic tools. For calculations, shortages, breeding, coordinates, or runtime observations, call the relevant tool. For factual questions already supported by GROUNDING, do not call another lookup; submit a short natural-language answer with submit_answer.\n\nAbsolute rules:\n1. Every game fact must come from GROUNDING or a tool result. Never guess.\n2. Every number must come from FACT_SHEET, GROUNDING, or the player's own question. Prefer natural sentences and write numbers directly only when they already appear in one of those. For calculator results, prefer slot references from FACT_SHEET.\n3. If you use slot references, only reference IDs listed in FACT_SHEET. Never invent, combine, or calculate values.\n4. Keep sentences short and chat-friendly (1-3 sentences, or up to 5 short steps).\n5. If available evidence does not support an answer, use status \"unknown\" and say you don't know.\n6. Do not echo the player's question. Do not use markdown. Step numbering is added automatically.\n7. Entity names may appear as plain words only when they come from GROUNDING, tool results, or the player's question. When localized names are provided, use the exact name matching the player's language; never translate a name yourself.\n8. Report missing, conflicting, or version-stale information in the uncertainty field.\n9. If the question is small talk or asks for an opinion, do not call tools; answer in one short sentence that you can only help with guide questions about items, Pals, recipes, materials, breeding, and progression, and do not name any specific Pal or item.\n10. Prefer the fewest tool calls that can answer the question; never repeat the same or a similar lookup.";

impl GuideAgent {
    pub fn new(
        registry: ToolRegistry,
        provider: Box<dyn ChatProvider>,
        config: AgentConfig,
    ) -> Self {
        Self {
            registry,
            provider,
            config,
        }
    }

    pub fn set_state_snapshot(&mut self, snapshot: PlayerStateSnapshot) {
        self.registry.set_state_snapshot(snapshot);
    }

    fn ground_question(&self, question: &str, deadline: Instant) -> GroundingContext {
        let mut grounding_budget = ToolBudget::new(24, deadline);
        let mut records = Vec::new();
        let mut provenance = BTreeSet::new();
        let mut uncertainty = Vec::new();
        let mut version = self.registry.base_version();
        let known_entity_names = self.registry.known_entity_names();
        let known_waza_names = self.registry.known_waza_names();

        for candidate in candidate_entities(question, &known_waza_names) {
            let envelope = self.registry.dispatch(
                "get_waza",
                &serde_json::json!({"query": candidate}),
                &mut grounding_budget,
            );
            self.push_grounding_envelope(
                "get_waza",
                &serde_json::json!({"query": candidate}),
                envelope,
                &mut records,
                &mut provenance,
                &mut uncertainty,
                &mut version,
            );
        }

        for candidate in candidate_entities(question, &known_entity_names) {
            let resolution = self.registry.dispatch(
                "resolve_name",
                &serde_json::json!({"query": candidate}),
                &mut grounding_budget,
            );
            let Some(kind) = (resolution.status == ToolStatus::Ok)
                .then_some(resolution.data.as_ref())
                .flatten()
                .and_then(|data| data.get("kind"))
                .and_then(Value::as_str)
            else {
                for message in resolution.uncertainty {
                    push_unique(&mut uncertainty, message);
                }
                continue;
            };
            let tool_names = match kind {
                "item" => vec!["get_item", "get_recipe"],
                "pal" => vec!["get_pal"],
                "technology" => vec!["get_technology"],
                _ => Vec::new(),
            };
            for tool_name in tool_names {
                let envelope = self.registry.dispatch(
                    tool_name,
                    &serde_json::json!({"query": candidate}),
                    &mut grounding_budget,
                );
                self.push_grounding_envelope(
                    tool_name,
                    &serde_json::json!({"query": candidate}),
                    envelope,
                    &mut records,
                    &mut provenance,
                    &mut uncertainty,
                    &mut version,
                );
            }
        }

        if has_recipe_grounding_intent(question) {
            for candidate in candidate_entities(question, &known_entity_names) {
                for tool_name in ["get_recipe", "get_item"] {
                    let already_grounded = records.iter().any(|record| {
                        record.name == tool_name
                            && record.arguments.get("query").and_then(Value::as_str)
                                == Some(candidate.as_str())
                    });
                    if already_grounded {
                        continue;
                    }
                    let envelope = self.registry.dispatch(
                        tool_name,
                        &serde_json::json!({"query": candidate}),
                        &mut grounding_budget,
                    );
                    self.push_grounding_envelope(
                        tool_name,
                        &serde_json::json!({"query": candidate}),
                        envelope,
                        &mut records,
                        &mut provenance,
                        &mut uncertainty,
                        &mut version,
                    );
                }
            }
        }

        if has_map_intent(question) {
            if let Some(arguments) = map_grounding_arguments(question) {
                for tool_name in ["locate_coordinate", "find_nearby_map_points"] {
                    let envelope =
                        self.registry
                            .dispatch(tool_name, &arguments, &mut grounding_budget);
                    self.push_grounding_envelope(
                        tool_name,
                        &arguments,
                        envelope,
                        &mut records,
                        &mut provenance,
                        &mut uncertainty,
                        &mut version,
                    );
                }
            } else if has_player_proximity_intent(question) {
                let arguments = serde_json::json!({
                    "kind": "fast_travel",
                    "limit": 5,
                });
                let envelope = self.registry.dispatch(
                    "find_nearby_map_points",
                    &arguments,
                    &mut grounding_budget,
                );
                self.push_grounding_envelope(
                    "find_nearby_map_points",
                    &arguments,
                    envelope,
                    &mut records,
                    &mut provenance,
                    &mut uncertainty,
                    &mut version,
                );
            } else if has_base_camp_route_intent(question) {
                let arguments = serde_json::json!({"to_query": "据点"});
                let envelope =
                    self.registry
                        .dispatch("plan_travel_route", &arguments, &mut grounding_budget);
                self.push_grounding_envelope(
                    "plan_travel_route",
                    &arguments,
                    envelope,
                    &mut records,
                    &mut provenance,
                    &mut uncertainty,
                    &mut version,
                );
            }
        }

        let search_arguments = serde_json::json!({"query": question, "limit": 5});
        let search = self.registry.dispatch(
            "search_structured_knowledge",
            &search_arguments,
            &mut grounding_budget,
        );
        for summary in search.provenance {
            provenance.insert(summary);
        }
        for message in search.uncertainty {
            push_unique(&mut uncertainty, message);
        }
        version = search.version;
        let search_data = (search.status == ToolStatus::Ok)
            .then_some(search.data)
            .flatten();

        GroundingContext {
            summary: grounding_summary(&records, search_data.as_ref()),
            records,
            provenance,
            uncertainty,
            version,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn push_grounding_envelope(
        &self,
        name: &str,
        arguments: &Value,
        envelope: ToolEnvelope,
        records: &mut Vec<ToolCallRecord>,
        provenance: &mut BTreeSet<ProvenanceSummary>,
        uncertainty: &mut Vec<String>,
        version: &mut VersionInfo,
    ) {
        let ToolEnvelope {
            status,
            data,
            provenance: envelope_provenance,
            version: envelope_version,
            uncertainty: envelope_uncertainty,
            errors,
        } = envelope;
        for summary in envelope_provenance {
            provenance.insert(summary);
        }
        for message in envelope_uncertainty {
            push_unique(uncertainty, message);
        }
        *version = envelope_version;
        if status != ToolStatus::Ok {
            return;
        }
        records.push(ToolCallRecord {
            round: 0,
            id: format!("ground_{}", records.len() + 1),
            name: name.to_string(),
            arguments: arguments.clone(),
            status,
            data,
            errors,
        });
    }

    pub fn ask(&self, question: &str) -> AgentAnswer {
        self.ask_with_cancellation(question, &AtomicBool::new(false))
    }

    pub fn ask_with_history(&self, question: &str, history: &[AgentHistoryEntry]) -> AgentAnswer {
        self.ask_with_history_and_cancellation(question, history, &AtomicBool::new(false))
    }

    pub fn ask_with_cancellation(&self, question: &str, cancelled: &AtomicBool) -> AgentAnswer {
        self.ask_with_history_and_cancellation(question, &[], cancelled)
    }

    pub fn ask_with_history_and_cancellation(
        &self,
        question: &str,
        history: &[AgentHistoryEntry],
        cancelled: &AtomicBool,
    ) -> AgentAnswer {
        let deadline = Instant::now() + self.config.limits.timeout;
        if let Some(request) = DirectMaterialRequest::parse(question) {
            let entity_name_variants_by_id = self.registry.entity_name_variants_by_id();
            if let Some(answer) =
                self.direct_material_answer(&request, deadline, &entity_name_variants_by_id)
            {
                return answer;
            }
        }
        let entity_name_variants_by_id = self.registry.entity_name_variants_by_id();
        let mut budget = ToolBudget::new(self.config.limits.max_tool_calls, deadline);
        let known_entity_names = self.registry.known_entity_names();
        let entity_names_by_id = self.registry.canonical_entity_names();
        let GroundingContext {
            records,
            provenance,
            uncertainty,
            version,
            summary,
        } = self.ground_question(question, deadline);
        let initial_fact_sheet = FactSheet::build(
            &records,
            known_entity_names.clone(),
            entity_names_by_id.clone(),
            &entity_name_variants_by_id,
            &version,
        );
        let mut messages = Vec::new();
        for entry in history {
            messages.push(ChatMessage::new("user", entry.question.clone()));
            messages.push(ChatMessage::new(
                "assistant",
                entry
                    .answer
                    .clone()
                    .unwrap_or_else(|| "[no answer]".to_string()),
            ));
        }
        messages.push(ChatMessage::new(
            "user",
            format!(
                "{question}\n\n{summary}\n\n{}",
                initial_fact_sheet.summary()
            ),
        ));
        let mut records = records;
        let mut provenance = provenance;
        let mut uncertainty = uncertainty;
        let mut version = version;
        let rounds = self.config.limits.max_tool_calls + 2;
        let mut submit_retries_used = 0;
        let mut provider_retries_used = 0;

        for round in 0..rounds {
            if cancelled.load(Ordering::SeqCst) {
                return self.error_answer(
                    records,
                    provenance,
                    uncertainty,
                    version.clone(),
                    "agent run cancelled before completion",
                );
            }
            if Instant::now() > deadline {
                return self.error_answer(
                    records,
                    provenance,
                    uncertainty,
                    version.clone(),
                    "agent timeout exceeded before a final answer",
                );
            }
            let tools = self.available_tools(&records, question);
            let request = ChatRequest {
                system: SYSTEM_PROMPT.to_string(),
                messages: messages.clone(),
                tools: tools.clone(),
            };
            let response = match self.provider.complete(&request) {
                Ok(response) => response,
                Err(error) => match (provider_retries_used == 0, &error) {
                    (true, ProviderError::InvalidResponse(_)) => {
                        provider_retries_used += 1;
                        messages.push(ChatMessage::new(
                            "user",
                            format!(
                                "PROVIDER_RESPONSE_ERROR: {error}\n\
                                 Return one valid tool call or submit_answer. \
                                 Tool arguments must be valid JSON."
                            ),
                        ));
                        continue;
                    }
                    _ => {
                        return self.error_answer(
                            records,
                            provenance,
                            uncertainty,
                            version,
                            format!("provider {}: {error}", self.provider.name()),
                        )
                    }
                },
            };
            if response.tool_requests.is_empty() {
                return self.model_fallback(
                    question,
                    response.content,
                    records,
                    provenance,
                    uncertainty,
                    version,
                    &entity_name_variants_by_id,
                );
            }
            if let Some(tool_request) = response
                .tool_requests
                .iter()
                .find(|request| request.name == "submit_answer")
            {
                match self.try_submit_answer(
                    tool_request,
                    question,
                    records.clone(),
                    provenance.clone(),
                    uncertainty.clone(),
                    version.clone(),
                    &known_entity_names,
                    &entity_names_by_id,
                    &entity_name_variants_by_id,
                ) {
                    Ok(answer) => return answer,
                    Err(error) if submit_retries_used == 0 => {
                        submit_retries_used += 1;
                        messages.push(ChatMessage::new(
                            "assistant",
                            if response.content.trim().is_empty() {
                                "I need to correct my answer.".to_string()
                            } else {
                                response.content.clone()
                            },
                        ));
                        messages.push(ChatMessage::new(
                            "user",
                            format!(
                                "SUBMIT_ANSWER_ERROR: {error}\n\
                                 Call submit_answer again with a corrected draft. \
                                 Numbers must come from FACT_SHEET or GROUNDING. \
                                 Use only slot IDs listed in the current FACT_SHEET."
                            ),
                        ));
                        continue;
                    }
                    Err(error) => {
                        uncertainty.push(format!("model draft invalid: {error}"));
                        let fact_sheet = FactSheet::build(
                            &records,
                            known_entity_names.clone(),
                            entity_names_by_id.clone(),
                            &entity_name_variants_by_id,
                            &version,
                        );
                        return self.rendered_fallback(
                            records,
                            provenance,
                            uncertainty,
                            version,
                            &fact_sheet,
                        );
                    }
                }
            }

            let assistant_content = if response.content.trim().is_empty() {
                "Selecting deterministic tools.".to_string()
            } else {
                response.content.clone()
            };
            messages.push(ChatMessage::new("assistant", assistant_content));
            for tool_request in response.tool_requests {
                self.dispatch_tool(
                    tool_request,
                    round,
                    &mut budget,
                    &mut records,
                    &mut provenance,
                    &mut uncertainty,
                    &mut version,
                    &mut messages,
                    &known_entity_names,
                    &entity_names_by_id,
                    &entity_name_variants_by_id,
                );
            }
        }

        uncertainty.push("tool call budget exhausted before a final answer".to_string());
        self.model_fallback(
            question,
            String::new(),
            records,
            provenance,
            uncertainty,
            version,
            &entity_name_variants_by_id,
        )
    }

    fn available_tools(&self, records: &[ToolCallRecord], question: &str) -> Vec<ToolSpec> {
        let completed_tools = records
            .iter()
            .map(|record| record.name.as_str())
            .collect::<BTreeSet<_>>();
        let successful_tools = records
            .iter()
            .filter(|record| record.status == ToolStatus::Ok)
            .map(|record| record.name.as_str())
            .collect::<BTreeSet<_>>();
        let exact_lookup_succeeded = [
            "get_item",
            "get_pal",
            "get_recipe",
            "get_technology",
            "get_waza",
        ]
        .into_iter()
        .any(|tool| successful_tools.contains(tool));
        let calculation_intent = has_calculation_intent(question);
        let preferred_calculator = preferred_calculation_tool(question);
        let calculation_succeeded = successful_tools
            .iter()
            .any(|tool_name| is_calculation_tool(tool_name));
        let map_grounding_succeeded = successful_tools
            .iter()
            .any(|tool_name| is_map_tool(tool_name));
        let breeding_intent = has_breeding_intent(question);
        let map_intent = has_map_intent(question);
        let state_intent = has_state_intent(question);
        let conflict_intent = question.contains("冲突");
        let pal_unlock_intent = question.contains("技能") || question.contains("解锁");
        let mut tools = self
            .registry
            .definitions()
            .into_iter()
            .filter(|definition| !completed_tools.contains(definition.name.as_str()))
            .filter(|definition| {
                if is_calculation_tool(&definition.name) && !calculation_intent {
                    return false;
                }
                if is_calculation_tool(&definition.name)
                    && map_grounding_succeeded
                    && preferred_calculator.is_none()
                {
                    return false;
                }
                if is_calculation_tool(&definition.name) {
                    if let Some(preferred) = preferred_calculator {
                        if definition.name != preferred {
                            return false;
                        }
                    }
                    if calculation_succeeded {
                        return false;
                    }
                }
                if is_breeding_tool(&definition.name) && !breeding_intent {
                    return false;
                }
                if is_grounding_map_tool(&definition.name) && map_grounding_succeeded {
                    return false;
                }
                if is_map_tool(&definition.name) && !map_intent {
                    return false;
                }
                if is_state_tool(&definition.name) && !state_intent {
                    return false;
                }
                if definition.name == "get_conflicting_records" && !conflict_intent {
                    return false;
                }
                let suppress_exact_lookup = (exact_lookup_succeeded || map_grounding_succeeded)
                    && matches!(
                        definition.name.as_str(),
                        "resolve_name"
                            | "get_item"
                            | "get_pal"
                            | "get_recipe"
                            | "get_technology"
                            | "get_waza"
                    );
                let suppress_passive_lookups = (successful_tools.contains("get_item")
                    || successful_tools.contains("get_pal"))
                    && matches!(
                        definition.name.as_str(),
                        "get_type_effectiveness" | "get_work_kind_descriptions"
                    );
                let suppress_pal_waza_lookup = definition.name == "get_pal_waza_unlocks"
                    && (successful_tools.contains("get_waza")
                        || (successful_tools.contains("get_pal") && !pal_unlock_intent));
                let suppress_search = definition.name == "search_structured_knowledge"
                    && (successful_tools.contains("get_item")
                        || successful_tools.contains("get_pal")
                        || successful_tools.contains("get_waza"));
                let suppress_waza_lookup = definition.name == "get_waza"
                    && successful_tools.contains("get_pal_waza_unlocks");
                !suppress_exact_lookup
                    && !suppress_passive_lookups
                    && !suppress_pal_waza_lookup
                    && !suppress_search
                    && !suppress_waza_lookup
            })
            .map(|definition| ToolSpec {
                name: definition.name,
                description: definition.description,
                parameters_schema: definition.parameters_schema,
            })
            .collect::<Vec<_>>();
        tools.push(answer::submit_answer_tool());
        tools
    }

    #[allow(clippy::too_many_arguments)]
    fn dispatch_tool(
        &self,
        tool_request: ToolRequest,
        round: usize,
        budget: &mut ToolBudget,
        records: &mut Vec<ToolCallRecord>,
        provenance: &mut BTreeSet<ProvenanceSummary>,
        uncertainty: &mut Vec<String>,
        version: &mut VersionInfo,
        messages: &mut Vec<ChatMessage>,
        known_entity_names: &BTreeSet<String>,
        entity_names_by_id: &BTreeMap<String, String>,
        entity_name_variants_by_id: &BTreeMap<String, BTreeSet<String>>,
    ) {
        let envelope = self
            .registry
            .dispatch(&tool_request.name, &tool_request.arguments, budget);
        records.push(ToolCallRecord {
            round,
            id: tool_request.id.clone(),
            name: tool_request.name.clone(),
            arguments: tool_request.arguments.clone(),
            status: envelope.status,
            data: envelope.data.clone(),
            errors: envelope.errors.clone(),
        });
        let envelope_json = serde_json::to_string(&envelope).unwrap_or_else(|_| "{}".to_string());
        for summary in envelope.provenance {
            provenance.insert(summary);
        }
        for message in envelope.uncertainty {
            if !uncertainty.contains(&message) {
                uncertainty.push(message);
            }
        }
        *version = envelope.version;
        let fact_sheet = FactSheet::build(
            records,
            known_entity_names.clone(),
            entity_names_by_id.clone(),
            entity_name_variants_by_id,
            version,
        );
        messages.push(ChatMessage::new(
            "user",
            format!(
                "TOOL_RESULT {} {}\n{envelope_json}\n{}",
                tool_request.id,
                tool_request.name,
                fact_sheet.summary()
            ),
        ));
    }

    #[allow(clippy::too_many_arguments)]
    fn try_submit_answer(
        &self,
        tool_request: &ToolRequest,
        question: &str,
        records: Vec<ToolCallRecord>,
        provenance: BTreeSet<ProvenanceSummary>,
        uncertainty: Vec<String>,
        version: VersionInfo,
        known_entity_names: &BTreeSet<String>,
        entity_names_by_id: &BTreeMap<String, String>,
        entity_name_variants_by_id: &BTreeMap<String, BTreeSet<String>>,
    ) -> Result<AgentAnswer, String> {
        let fact_sheet = FactSheet::build(
            &records,
            known_entity_names.clone(),
            entity_names_by_id.clone(),
            entity_name_variants_by_id,
            &version,
        )
        .with_player_question(question);
        let draft = match serde_json::from_value::<AnswerDraft>(tool_request.arguments.clone()) {
            Ok(draft) => draft,
            Err(error) => {
                return Err(format!("invalid arguments: {error}"));
            }
        };
        match answer::render(&draft, &fact_sheet, self.config.max_reply_characters) {
            Ok(reply) => {
                let mut uncertainty = uncertainty;
                for message in draft.uncertainty.clone().unwrap_or_default() {
                    if !uncertainty.contains(&message) {
                        uncertainty.push(message);
                    }
                }
                if records.is_empty()
                    && !uncertainty
                        .iter()
                        .any(|message| message.contains("no deterministic tool evidence"))
                {
                    uncertainty.push(
                        "no deterministic tool evidence was used for this answer".to_string(),
                    );
                }
                Ok(self.finished_answer(
                    records,
                    provenance,
                    uncertainty,
                    version,
                    reply,
                    Some(draft.status),
                ))
            }
            Err(error) => Err(error.to_string()),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn model_fallback(
        &self,
        question: &str,
        content: String,
        records: Vec<ToolCallRecord>,
        provenance: BTreeSet<ProvenanceSummary>,
        mut uncertainty: Vec<String>,
        version: VersionInfo,
        entity_name_variants_by_id: &BTreeMap<String, BTreeSet<String>>,
    ) -> AgentAnswer {
        let known_entity_names = self.registry.known_entity_names();
        let entity_names_by_id = self.registry.canonical_entity_names();
        let fact_sheet = FactSheet::build(
            &records,
            known_entity_names,
            entity_names_by_id,
            entity_name_variants_by_id,
            &version,
        )
        .with_player_question(question);
        let trimmed = content.trim();
        if !trimmed.is_empty() {
            let status = match answer_status(&records) {
                AgentStatus::Ok => AnswerStatus::Ok,
                _ => AnswerStatus::Unknown,
            };
            let draft = AnswerDraft {
                status,
                sentences: vec![trimmed.to_string()],
                steps: None,
                uncertainty: None,
            };
            if let Ok(reply) = answer::render(&draft, &fact_sheet, self.config.max_reply_characters)
            {
                uncertainty.push(
                    "model did not call submit_answer; strict compatibility render used"
                        .to_string(),
                );
                if records.is_empty()
                    && !uncertainty
                        .iter()
                        .any(|message| message.contains("no deterministic tool evidence"))
                {
                    uncertainty.push(
                        "no deterministic tool evidence was used for this answer".to_string(),
                    );
                }
                return self.finished_answer(
                    records,
                    provenance,
                    uncertainty,
                    version,
                    reply,
                    Some(draft.status),
                );
            }
        }
        uncertainty
            .push("model did not call submit_answer; deterministic fallback used".to_string());
        self.rendered_fallback(records, provenance, uncertainty, version, &fact_sheet)
    }

    fn rendered_fallback(
        &self,
        records: Vec<ToolCallRecord>,
        provenance: BTreeSet<ProvenanceSummary>,
        mut uncertainty: Vec<String>,
        version: VersionInfo,
        fact_sheet: &FactSheet,
    ) -> AgentAnswer {
        if records.is_empty()
            && !uncertainty
                .iter()
                .any(|message| message.contains("no deterministic tool evidence"))
        {
            uncertainty.push("no deterministic tool evidence was used for this answer".to_string());
        }
        let reply =
            answer::fallback_render(fact_sheet, &uncertainty, self.config.max_reply_characters);
        self.finished_answer(records, provenance, uncertainty, version, reply, None)
    }

    fn finished_answer(
        &self,
        records: Vec<ToolCallRecord>,
        provenance: BTreeSet<ProvenanceSummary>,
        uncertainty: Vec<String>,
        version: VersionInfo,
        reply: String,
        draft_status: Option<AnswerStatus>,
    ) -> AgentAnswer {
        let mut status = answer_status(&records);
        match draft_status {
            Some(AnswerStatus::Unknown) => status = AgentStatus::Unknown,
            Some(AnswerStatus::Ambiguous) if status == AgentStatus::Ok => {
                status = AgentStatus::Ambiguous;
            }
            _ => {}
        }
        let mut errors = Vec::new();
        for record in &records {
            for error in &record.errors {
                if !errors.contains(error) {
                    errors.push(error.clone());
                }
            }
        }
        AgentAnswer {
            status,
            answer: Some(reply),
            tool_calls: records
                .iter()
                .filter(|record| !record.id.starts_with("ground_"))
                .cloned()
                .collect(),
            provenance: provenance.into_iter().collect(),
            version,
            uncertainty,
            errors,
        }
    }

    fn error_answer(
        &self,
        records: Vec<ToolCallRecord>,
        provenance: BTreeSet<ProvenanceSummary>,
        uncertainty: Vec<String>,
        version: VersionInfo,
        message: impl Into<String>,
    ) -> AgentAnswer {
        AgentAnswer {
            status: AgentStatus::Error,
            answer: None,
            tool_calls: records
                .iter()
                .filter(|record| !record.id.starts_with("ground_"))
                .cloned()
                .collect(),
            provenance: provenance.into_iter().collect(),
            version,
            uncertainty,
            errors: vec![message.into()],
        }
    }

    fn direct_material_answer(
        &self,
        request: &DirectMaterialRequest,
        deadline: Instant,
        entity_name_variants_by_id: &BTreeMap<String, BTreeSet<String>>,
    ) -> Option<AgentAnswer> {
        let mut budget = ToolBudget::new(1, deadline);
        let envelope = self.registry.dispatch(
            "calculate_materials",
            &json!({
                "query": request.query,
                "quantity": request.quantity,
            }),
            &mut budget,
        );
        if envelope.status != ToolStatus::Ok {
            return None;
        }
        let data = envelope.data.clone().unwrap_or_else(|| json!({}));
        let localized = request.localized;
        let target_name = data
            .get("target_id")
            .and_then(Value::as_str)
            .map(|target_id| {
                localized_entity_name(
                    target_id,
                    data.get("target_name")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown"),
                    entity_name_variants_by_id,
                    localized,
                )
            })
            .unwrap_or_else(|| "unknown".to_string());
        let totals = data
            .get("totals")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let materials = totals
            .iter()
            .map(|total| {
                let quantity = total.get("required_quantity").and_then(Value::as_u64)?;
                let item_id = total.get("item_id").and_then(Value::as_str)?;
                let fallback = total
                    .get("item_name")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown");
                Some(format!(
                    "{} {} 个",
                    localized_entity_name(item_id, fallback, entity_name_variants_by_id, localized),
                    quantity
                ))
            })
            .collect::<Option<Vec<String>>>()?;
        if materials.is_empty() {
            return None;
        }
        let reply = format!(
            "制造 {} 个{}需要：{}。",
            request.quantity,
            target_name,
            materials.join("，")
        );
        let record = ToolCallRecord {
            round: 0,
            id: "direct_calculate_materials".to_string(),
            name: "calculate_materials".to_string(),
            arguments: json!({
                "query": request.query,
                "quantity": request.quantity,
            }),
            status: envelope.status,
            data: Some(data),
            errors: envelope.errors,
        };
        let provenance = envelope.provenance.into_iter().collect::<BTreeSet<_>>();
        Some(self.finished_answer(
            vec![record],
            provenance,
            envelope.uncertainty,
            envelope.version,
            reply,
            Some(AnswerStatus::Ok),
        ))
    }
}

#[derive(Debug)]
struct DirectMaterialRequest {
    query: String,
    quantity: u32,
    localized: bool,
}

impl DirectMaterialRequest {
    fn parse(question: &str) -> Option<Self> {
        let question = question.trim();
        let localized = question.chars().any(is_han_character);
        let unit = question
            .char_indices()
            .find(|(_, character)| *character == '个')?;
        let mut quantity_start = unit.0;
        while quantity_start > 0 {
            let character = question[..quantity_start].chars().next_back()?;
            if character.is_ascii_digit() || is_chinese_numeral(character) {
                quantity_start -= character.len_utf8();
            } else {
                break;
            }
        }
        if quantity_start == unit.0 {
            return None;
        }
        let prefix = question[..quantity_start].trim();
        let crafting_verbs = ["制作", "制造", "合成", "做", "造"];
        let prefix_ends_with_verb = crafting_verbs
            .iter()
            .any(|verb| prefix.is_empty() || prefix.ends_with(verb));
        let query_before_unit = if prefix_ends_with_verb {
            None
        } else {
            let verb = crafting_verbs
                .iter()
                .find(|verb| prefix.starts_with(*verb))?;
            let query = prefix.strip_prefix(verb)?.trim();
            (!query.is_empty()).then_some(query)
        };
        let quantity_text = &question[quantity_start..unit.0];
        let quantity = if quantity_text
            .chars()
            .all(|character| character.is_ascii_digit())
        {
            quantity_text.parse().ok()?
        } else {
            parse_chinese_quantity(quantity_text)?
        };
        if quantity == 0 {
            return None;
        }
        let mut item_start = unit.0 + unit.1.len_utf8();
        while question[item_start..].starts_with('的') {
            item_start += '的'.len_utf8();
        }
        let remainder = &question[item_start..];
        let material_start = remainder.find("材料")?;
        let material_prefix = &remainder[..material_start];
        let suffix_start = ["需要", "要"]
            .iter()
            .filter_map(|marker| material_prefix.find(marker))
            .min()?;
        if suffix_start == 0 && query_before_unit.is_none() {
            return None;
        }
        let query = query_before_unit
            .unwrap_or(remainder[..suffix_start].trim())
            .trim_end_matches(['？', '?', '。', '.', '！', '!', '，', ','])
            .trim();
        if query.is_empty() || query.chars().count() > 120 {
            return None;
        }
        Some(Self {
            query: query.to_string(),
            quantity,
            localized,
        })
    }
}

fn localized_entity_name(
    item_id: &str,
    fallback: &str,
    variants_by_id: &BTreeMap<String, BTreeSet<String>>,
    localized: bool,
) -> String {
    if !localized {
        return fallback.to_string();
    }
    variants_by_id
        .get(item_id)
        .and_then(|variants| {
            variants
                .iter()
                .find(|variant| variant.chars().any(is_han_character))
        })
        .map(String::as_str)
        .unwrap_or(fallback)
        .to_string()
}

fn is_han_character(character: char) -> bool {
    matches!(character, '\u{4E00}'..='\u{9FFF}')
}

fn is_chinese_numeral(character: char) -> bool {
    matches!(
        character,
        '零' | '一'
            | '二'
            | '两'
            | '三'
            | '四'
            | '五'
            | '六'
            | '七'
            | '八'
            | '九'
            | '十'
            | '百'
            | '千'
    )
}

fn parse_chinese_quantity(value: &str) -> Option<u32> {
    let mut total = 0_u32;
    let mut current = 0_u32;
    for character in value.chars() {
        let digit = match character {
            '零' => 0,
            '一' => 1,
            '二' | '两' => 2,
            '三' => 3,
            '四' => 4,
            '五' => 5,
            '六' => 6,
            '七' => 7,
            '八' => 8,
            '九' => 9,
            '十' => {
                total += if current == 0 { 10 } else { current * 10 };
                current = 0;
                continue;
            }
            '百' => {
                total += if current == 0 { 100 } else { current * 100 };
                current = 0;
                continue;
            }
            '千' => {
                total += if current == 0 { 1_000 } else { current * 1_000 };
                current = 0;
                continue;
            }
            _ => return None,
        };
        current = current
            .checked_mul(10)
            .and_then(|value| value.checked_add(digit))
            .filter(|value| *value < 10_000)?;
    }
    total.checked_add(current).filter(|value| *value != 0)
}

fn is_calculation_tool(name: &str) -> bool {
    matches!(
        name,
        "calculate_materials" | "calculate_shortage" | "calculate_craftable_count"
    )
}

fn is_breeding_tool(name: &str) -> bool {
    matches!(
        name,
        "calculate_breeding_result" | "calculate_breeding_chain"
    )
}

fn is_map_tool(name: &str) -> bool {
    matches!(
        name,
        "locate_coordinate"
            | "find_nearby_map_points"
            | "find_pal_spawn_zones"
            | "plan_travel_route"
    )
}

fn is_grounding_map_tool(name: &str) -> bool {
    matches!(
        name,
        "locate_coordinate" | "find_nearby_map_points" | "find_pal_spawn_zones"
    )
}

fn is_state_tool(name: &str) -> bool {
    matches!(
        name,
        "import_player_snapshot" | "analyze_inventory" | "analyze_party" | "suggest_next_goals"
    )
}

fn has_calculation_intent(question: &str) -> bool {
    question.chars().any(|character| character.is_ascii_digit())
        || [
            "多少",
            "需要",
            "几个",
            "几份",
            "还缺",
            "缺什么",
            "要多少",
            "总共",
        ]
        .iter()
        .any(|keyword| question.contains(keyword))
}

fn preferred_calculation_tool(question: &str) -> Option<&'static str> {
    if ["还缺", "缺什么", "短缺", "missing for"]
        .iter()
        .any(|keyword| question.contains(keyword))
    {
        return Some("calculate_shortage");
    }
    if ["能做", "可以做", "可制作", "最多", "can make", "can craft"]
        .iter()
        .any(|keyword| question.contains(keyword))
    {
        return Some("calculate_craftable_count");
    }
    if ["需要多少", "材料", "总共", "materials for"]
        .iter()
        .any(|keyword| question.contains(keyword))
    {
        return Some("calculate_materials");
    }
    None
}

fn has_recipe_grounding_intent(question: &str) -> bool {
    ["怎么做", "怎么制作", "怎么合成", "制作方法", "配方", "合成"]
        .iter()
        .any(|keyword| question.contains(keyword))
}

fn has_breeding_intent(question: &str) -> bool {
    ["配种", "繁殖", "出什么", "生什么", "路线"]
        .iter()
        .any(|keyword| question.contains(keyword))
}

fn has_map_intent(question: &str) -> bool {
    [
        "坐标",
        "地图",
        "附近",
        "位置",
        "传送",
        "去哪",
        "怎么去",
        "前往",
        "多远",
        "距离",
    ]
    .iter()
    .any(|keyword| question.contains(keyword))
}

fn has_player_proximity_intent(question: &str) -> bool {
    [
        "离我最近",
        "最近的传送",
        "nearest fast travel",
        "nearest teleport",
        "nearest waypoint",
    ]
    .iter()
    .any(|keyword| question.to_ascii_lowercase().contains(keyword))
}

fn has_base_camp_route_intent(question: &str) -> bool {
    question.contains("据点")
        && [
            "去",
            "回",
            "返回",
            "最快",
            "路线",
            "go to",
            "return to",
            "fastest",
        ]
        .iter()
        .any(|keyword| question.to_ascii_lowercase().contains(keyword))
}

fn map_grounding_arguments(question: &str) -> Option<Value> {
    let (x, y) = labeled_coordinate(question).or_else(|| coordinate_after_marker(question))?;
    let explicit_world = ["世界坐标", "游戏坐标", "实际坐标", "world coordinate"]
        .iter()
        .any(|keyword| question.to_ascii_lowercase().contains(keyword));
    let explicit_pixel = ["地图坐标", "地图像素", "像素", "pixel"]
        .iter()
        .any(|keyword| question.to_ascii_lowercase().contains(keyword));
    let coordinate_system = if explicit_world {
        "world"
    } else if explicit_pixel {
        "map_pixel"
    } else if question.contains("百分比") || question.contains("归一化") {
        "normalized"
    } else {
        "map_display"
    };
    let mut arguments = serde_json::json!({
        "x": x,
        "y": y,
        "z": 0.0,
        "coordinate_system": coordinate_system,
        "limit": 5,
    });
    if question.contains("传送") {
        arguments["kind"] = serde_json::json!("fast_travel");
    }
    Some(arguments)
}

fn labeled_coordinate(question: &str) -> Option<(f64, f64)> {
    let lowercase = question.to_ascii_lowercase();
    let x = labeled_number_after(&lowercase, "x")?;
    let y = labeled_number_after(&lowercase, "y")?;
    Some((x, y))
}

fn labeled_number_after(text: &str, label: &str) -> Option<f64> {
    let mut search_start = 0;
    while let Some(index) = text[search_start..].find(label) {
        let value_start = search_start + index + label.len();
        let mut cursor = value_start;
        while cursor < text.len()
            && text[cursor..]
                .chars()
                .next()
                .is_some_and(|character| character.is_whitespace() || "=:：为".contains(character))
        {
            cursor += text[cursor..].chars().next()?.len_utf8();
        }
        if let Some((value, _)) = parse_f64_at(text, cursor) {
            if text[..value_start]
                .chars()
                .next_back()
                .is_none_or(|character| !character.is_ascii_alphanumeric())
            {
                return Some(value);
            }
        }
        search_start = value_start;
    }
    None
}

fn coordinate_after_marker(question: &str) -> Option<(f64, f64)> {
    let marker = question.rfind("坐标")?;
    let mut cursor = marker + "坐标".len();
    while cursor < question.len()
        && question[cursor..]
            .chars()
            .next()
            .is_some_and(|character| character.is_whitespace() || "是=：:（(".contains(character))
    {
        cursor += question[cursor..].chars().next()?.len_utf8();
    }
    let (x, after_x) = parse_f64_at(question, cursor)?;
    let mut separator = after_x;
    while separator < question.len()
        && question[separator..]
            .chars()
            .next()
            .is_some_and(|character| character.is_whitespace() || "，,、/".contains(character))
    {
        separator += question[separator..].chars().next()?.len_utf8();
    }
    let (y, _) = parse_f64_at(question, separator)?;
    Some((x, y))
}

fn parse_f64_at(text: &str, start: usize) -> Option<(f64, usize)> {
    let bytes = text.as_bytes();
    let mut end = start;
    if end < bytes.len() && (bytes[end] == b'-' || bytes[end] == b'+') {
        end += 1;
    }
    let digits_start = end;
    while end < bytes.len() && bytes[end].is_ascii_digit() {
        end += 1;
    }
    if end < bytes.len() && bytes[end] == b'.' {
        end += 1;
        while end < bytes.len() && bytes[end].is_ascii_digit() {
            end += 1;
        }
    }
    if end == digits_start || digits_start < start {
        return None;
    }
    let value = text[start..end].parse::<f64>().ok()?;
    value.is_finite().then_some((value, end))
}

fn has_state_intent(question: &str) -> bool {
    ["背包", "队伍", "我的状态", "状态", "下一步"]
        .iter()
        .any(|keyword| question.contains(keyword))
}

fn candidate_entities(question: &str, known_entity_names: &BTreeSet<String>) -> Vec<String> {
    let mut candidates = known_entity_names
        .iter()
        .filter(|name| contains_grounding_entity(question, name))
        .cloned()
        .collect::<Vec<_>>();
    candidates.sort_by_key(|candidate| std::cmp::Reverse(candidate.chars().count()));
    candidates.dedup();
    candidates.truncate(4);
    candidates
}

fn contains_grounding_entity(text: &str, name: &str) -> bool {
    if name.chars().any(is_cjk_character) {
        text.contains(name)
    } else {
        contains_entity_phrase(text, name)
    }
}

fn is_cjk_character(character: char) -> bool {
    matches!(
        character as u32,
        0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xF900..=0xFAFF
            | 0x20000..=0x2FA1F
    )
}

fn push_unique(target: &mut Vec<String>, message: String) {
    if !target.contains(&message) {
        target.push(message);
    }
}

fn grounding_summary(records: &[ToolCallRecord], search_data: Option<&Value>) -> String {
    if records.is_empty() && search_data.is_none() {
        return "GROUNDING: no deterministic reviewed facts matched this question.".to_string();
    }
    let mut lines = vec![
        "GROUNDING (reviewed deterministic evidence; do not invent facts beyond this):".to_string(),
    ];
    for record in records {
        let Some(data) = &record.data else {
            continue;
        };
        let compact = compact_grounding_data(&record.name, data);
        let encoded = serde_json::to_string(&compact).unwrap_or_else(|_| "{}".to_string());
        lines.push(format!(
            "[{}] {}",
            record.name,
            truncate_characters(&encoded, 2000)
        ));
    }
    if let Some(search_data) = search_data {
        let compact = compact_grounding_data("search_structured_knowledge", search_data);
        let encoded = serde_json::to_string(&compact).unwrap_or_else(|_| "{}".to_string());
        lines.push(format!(
            "[search_structured_knowledge] {}",
            truncate_characters(&encoded, 2000)
        ));
    }
    lines.join("\n")
}

fn compact_grounding_data(name: &str, data: &Value) -> Value {
    match name {
        "get_item" => {
            let Some(fields) = data.as_object() else {
                return data.clone();
            };
            let mut compact = serde_json::Map::new();
            for key in ["id", "names", "description", "rarity", "pal_drop_sources"] {
                if let Some(value) = fields.get(key) {
                    compact.insert(key.to_string(), value.clone());
                }
            }
            for relation in ["produced_by", "used_as_ingredient", "byproduct_of"] {
                let Some(values) = fields.get(relation).and_then(Value::as_array) else {
                    continue;
                };
                let trimmed = values
                    .iter()
                    .take(8)
                    .filter_map(|value| value.as_object())
                    .map(|fields| {
                        let mut summary = serde_json::Map::new();
                        for key in [
                            "id",
                            "output_item_id",
                            "output_item_name",
                            "output_quantity",
                            "ingredient_quantity",
                        ] {
                            if let Some(value) = fields.get(key) {
                                summary.insert(key.to_string(), value.clone());
                            }
                        }
                        Value::Object(summary)
                    })
                    .collect::<Vec<_>>();
                compact.insert(format!("{relation}_count"), serde_json::json!(values.len()));
                if values.len() > 8 {
                    compact.insert(format!("{relation}_truncated"), serde_json::json!(true));
                }
                compact.insert(relation.to_string(), Value::Array(trimmed));
            }
            Value::Object(compact)
        }
        "get_pal" => {
            let Some(fields) = data.as_object() else {
                return data.clone();
            };
            let mut compact = fields.clone();
            if let Some(habitat_ids) = compact.remove("habitat_ids") {
                let count = habitat_ids.as_array().map(Vec::len).unwrap_or(0);
                compact.insert("habitat_zone_count".to_string(), serde_json::json!(count));
                if let Some(ids) = habitat_ids.as_array() {
                    let preview = ids.iter().take(20).cloned().collect::<Vec<_>>();
                    compact.insert(
                        "habitat_zone_ids_preview".to_string(),
                        serde_json::json!(preview),
                    );
                }
            }
            Value::Object(compact)
        }
        "search_structured_knowledge" => {
            let Some(fields) = data.as_object() else {
                return data.clone();
            };
            let mut compact = serde_json::Map::new();
            if let Some(results) = fields.get("results").and_then(Value::as_array) {
                let trimmed = results
                    .iter()
                    .take(5)
                    .map(|result| {
                        let mut fields = result.as_object().cloned().unwrap_or_default();
                        if let Some(summary) = fields.get("summary").and_then(Value::as_str) {
                            fields.insert(
                                "summary".to_string(),
                                serde_json::json!(truncate_characters(summary, 300)),
                            );
                        }
                        Value::Object(fields)
                    })
                    .collect::<Vec<_>>();
                compact.insert("results".to_string(), Value::Array(trimmed));
            }
            Value::Object(compact)
        }
        _ => data.clone(),
    }
}

fn answer_status(records: &[ToolCallRecord]) -> AgentStatus {
    if records.is_empty() {
        return AgentStatus::Unknown;
    }
    if records
        .iter()
        .any(|record| record.status == ToolStatus::Ambiguous)
    {
        return AgentStatus::Ambiguous;
    }
    if records
        .iter()
        .any(|record| record.status == ToolStatus::Unknown)
        || !records.iter().any(|record| record.status == ToolStatus::Ok)
    {
        return AgentStatus::Unknown;
    }
    AgentStatus::Ok
}
