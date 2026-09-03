//! Bounded, grounded natural-language guide agent.

mod answer;

use answer::{AnswerDraft, AnswerStatus, FactSheet};
use guide_core::{ProvenanceSummary, VersionInfo};
use guide_tools::{ToolBudget, ToolRegistry, ToolStatus};
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

pub struct GuideAgent {
    registry: ToolRegistry,
    provider: Box<dyn ChatProvider>,
    config: AgentConfig,
}

const SYSTEM_PROMPT: &str = "You are the Palworld Guider brain. Understand the player's question, select whitelisted deterministic tools for exact facts, recipes, quantities, shortages, breeding, or runtime observations, then submit the final answer with the submit_answer tool.\n\nAbsolute rules:\n1. Never write numbers. No digits, decimals, number words (one, two, three...), or ordinals (first, second...) in sentences or steps. Every quantity, coordinate, and numeric fact must be a slot reference such as {q1} taken from the FACT_SHEET.\n2. Only reference slot IDs listed in the current FACT_SHEET. Never invent, combine, or calculate slot values.\n3. Keep sentences short and chat-friendly (1-3 sentences, or up to 5 short steps).\n4. If tool results do not support an answer, use status \"unknown\" and say you don't know. Never guess game facts.\n5. Do not echo the player's question. Do not use markdown. Step numbering is added automatically.\n6. Entity names may appear as plain words only when they come from tool results or the player's question.\n7. Report missing, conflicting, or version-stale information from tool results in the uncertainty field.\n8. If the question is small talk or asks for an opinion (for example cuteness or friendliness), do not call tools; answer in one short sentence that you can only help with guide questions about items, Pals, recipes, materials, breeding, and progression, and do not name any specific Pal or item.\n9. Prefer the fewest tool calls that can answer the question; never repeat the same or a similar lookup.";

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

        let mut messages = vec![ChatMessage::new("user", question)];
        let mut records = Vec::new();
        let mut provenance: BTreeSet<ProvenanceSummary> = BTreeSet::new();
        let mut uncertainty = Vec::new();
        let mut version = self.registry.base_version();
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
            tool_calls: records,
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
            tool_calls: records,
            provenance: provenance.into_iter().collect(),
            version,
            uncertainty,
            errors: vec![message.into()],
        }
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
