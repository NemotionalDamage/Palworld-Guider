//! Bounded, grounded natural-language guide agent.

mod answer;

use answer::{contains_entity_phrase, truncate_characters, AnswerDraft, AnswerStatus, FactSheet};
use guide_core::{ProvenanceSummary, VersionInfo};
use guide_tools::{ToolBudget, ToolEnvelope, ToolRegistry, ToolStatus};
use provider::{ChatMessage, ChatProvider, ChatRequest, ToolRequest, ToolSpec};
use serde::{Deserialize, Serialize};
use serde_json::Value;
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

const SYSTEM_PROMPT: &str = "You are the Palworld Guider brain. Answer the player from the supplied GROUNDING and whitelisted deterministic tools. For calculations, shortages, breeding, coordinates, or runtime observations, call the relevant tool. For factual questions already supported by GROUNDING, do not call another lookup; submit a short natural-language answer with submit_answer.\n\nAbsolute rules:\n1. Every game fact must come from GROUNDING or a tool result. Never guess.\n2. Numbers and exact quantities must be slot references such as {q1} from FACT_SHEET; do not write digits or number words in sentences or steps. For non-numeric facts, slots may be empty.\n3. Only reference slot IDs listed in the current FACT_SHEET. Never invent, combine, or calculate slot values.\n4. Keep sentences short and chat-friendly (1-3 sentences, or up to 5 short steps).\n5. If available evidence does not support an answer, use status \"unknown\" and say you don't know.\n6. Do not echo the player's question. Do not use markdown. Step numbering is added automatically.\n7. Entity names may appear as plain words only when they come from GROUNDING, tool results, or the player's question.\n8. Report missing, conflicting, or version-stale information in the uncertainty field.\n9. If the question is small talk or asks for an opinion, do not call tools; answer in one short sentence that you can only help with guide questions about items, Pals, recipes, materials, breeding, and progression, and do not name any specific Pal or item.\n10. Prefer the fewest tool calls that can answer the question; never repeat the same or a similar lookup.";

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
            let tool_name = match kind {
                "item" => Some("get_item"),
                "pal" => Some("get_pal"),
                "technology" => Some("get_technology"),
                _ => None,
            };
            let Some(tool_name) = tool_name else {
                continue;
            };
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

    pub fn ask_with_cancellation(&self, question: &str, cancelled: &AtomicBool) -> AgentAnswer {
        let deadline = Instant::now() + self.config.limits.timeout;
        let mut budget = ToolBudget::new(self.config.limits.max_tool_calls, deadline);
        let known_entity_names = self.registry.known_entity_names();
        let entity_names_by_id = self.registry.canonical_entity_names();
        let mut tools = self
            .registry
            .definitions()
            .into_iter()
            .map(|definition| ToolSpec {
                name: definition.name,
                description: definition.description,
                parameters_schema: definition.parameters_schema,
            })
            .collect::<Vec<_>>();
        tools.push(answer::submit_answer_tool());

        let GroundingContext {
            records,
            provenance,
            uncertainty,
            version,
            summary,
        } = self.ground_question(question, deadline);
        let mut messages = vec![ChatMessage::new("user", format!("{question}\n\n{summary}"))];
        let mut records = records;
        let mut provenance = provenance;
        let mut uncertainty = uncertainty;
        let mut version = version;
        let rounds = self.config.limits.max_tool_calls + 1;

        for round in 0..rounds {
            if cancelled.load(Ordering::SeqCst) {
                return self.error_answer(
                    records,
                    provenance,
                    uncertainty,
                    version,
                    "agent run cancelled before completion",
                );
            }
            if Instant::now() > deadline {
                return self.error_answer(
                    records,
                    provenance,
                    uncertainty,
                    version,
                    "agent timeout exceeded before a final answer",
                );
            }
            let request = ChatRequest {
                system: SYSTEM_PROMPT.to_string(),
                messages: messages.clone(),
                tools: tools.clone(),
            };
            let response = match self.provider.complete(&request) {
                Ok(response) => response,
                Err(error) => {
                    return self.error_answer(
                        records,
                        provenance,
                        uncertainty,
                        version,
                        format!("provider {}: {error}", self.provider.name()),
                    )
                }
            };
            if response.tool_requests.is_empty() {
                return self.model_fallback(
                    response.content,
                    records,
                    provenance,
                    uncertainty,
                    version,
                );
            }
            if let Some(tool_request) = response
                .tool_requests
                .iter()
                .find(|request| request.name == "submit_answer")
            {
                return self.submit_answer(
                    tool_request,
                    records,
                    provenance,
                    uncertainty,
                    version,
                    &known_entity_names,
                    &entity_names_by_id,
                );
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
                );
            }
        }

        uncertainty.push("tool call budget exhausted before a final answer".to_string());
        self.model_fallback(String::new(), records, provenance, uncertainty, version)
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
    fn submit_answer(
        &self,
        tool_request: &ToolRequest,
        records: Vec<ToolCallRecord>,
        provenance: BTreeSet<ProvenanceSummary>,
        mut uncertainty: Vec<String>,
        version: VersionInfo,
        known_entity_names: &BTreeSet<String>,
        entity_names_by_id: &BTreeMap<String, String>,
    ) -> AgentAnswer {
        let fact_sheet = FactSheet::build(
            &records,
            known_entity_names.clone(),
            entity_names_by_id.clone(),
            &version,
        );
        let draft = match serde_json::from_value::<AnswerDraft>(tool_request.arguments.clone()) {
            Ok(draft) => draft,
            Err(error) => {
                uncertainty.push(format!("model draft invalid: invalid arguments: {error}"));
                return self.rendered_fallback(
                    records,
                    provenance,
                    uncertainty,
                    version,
                    &fact_sheet,
                );
            }
        };
        for message in draft.uncertainty.clone().unwrap_or_default() {
            if !uncertainty.contains(&message) {
                uncertainty.push(message);
            }
        }
        match answer::render(&draft, &fact_sheet, self.config.max_reply_characters) {
            Ok(reply) => {
                let mut uncertainty = uncertainty;
                if records.is_empty()
                    && !uncertainty
                        .iter()
                        .any(|message| message.contains("no deterministic tool evidence"))
                {
                    uncertainty.push(
                        "no deterministic tool evidence was used for this answer".to_string(),
                    );
                }
                self.finished_answer(
                    records,
                    provenance,
                    uncertainty,
                    version,
                    reply,
                    Some(draft.status),
                )
            }
            Err(error) => {
                uncertainty.push(format!("model draft invalid: {error}"));
                self.rendered_fallback(records, provenance, uncertainty, version, &fact_sheet)
            }
        }
    }

    fn model_fallback(
        &self,
        content: String,
        records: Vec<ToolCallRecord>,
        provenance: BTreeSet<ProvenanceSummary>,
        mut uncertainty: Vec<String>,
        version: VersionInfo,
    ) -> AgentAnswer {
        let known_entity_names = self.registry.known_entity_names();
        let entity_names_by_id = self.registry.canonical_entity_names();
        let fact_sheet =
            FactSheet::build(&records, known_entity_names, entity_names_by_id, &version);
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
                slots: Vec::new(),
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
