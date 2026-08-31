//! Bounded, grounded natural-language guide agent.

use guide_core::{ProvenanceSummary, VersionInfo};
use guide_tools::{ToolBudget, ToolRegistry, ToolStatus};
use provider::{ChatMessage, ChatProvider, ChatRequest, ToolSpec};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;
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

const SYSTEM_PROMPT: &str = "You are the Palworld Guider brain. Understand the player's question, select whitelisted tools when exact facts, recipes, quantities, shortages, or breeding results are needed, and phrase a concise grounded answer. Never invent game facts. Never calculate material totals, recipe trees, shortages, or breeding results without calling the corresponding deterministic tool. Never change a value returned by a tool. Keep answers short and suitable for in-game chat. Report missing, conflicting, or version-stale information from tool results clearly.";

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

    pub fn ask(&self, question: &str) -> AgentAnswer {
        self.ask_with_cancellation(question, &AtomicBool::new(false))
    }

    pub fn ask_with_cancellation(&self, question: &str, cancelled: &AtomicBool) -> AgentAnswer {
        let deadline = Instant::now() + self.config.limits.timeout;
        let mut budget = ToolBudget::new(self.config.limits.max_tool_calls, deadline);
        let tools = self
            .registry
            .definitions()
            .iter()
            .map(|definition| ToolSpec {
                name: definition.name.clone(),
                description: definition.description.clone(),
                parameters_schema: definition.parameters_schema.clone(),
            })
            .collect::<Vec<_>>();
        let mut messages = vec![ChatMessage::new("user", question)];
        let mut records = Vec::new();
        let mut provenance: BTreeSet<ProvenanceSummary> = BTreeSet::new();
        let mut uncertainty: Vec<String> = Vec::new();
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
                return self.finalize(response.content, records, provenance, uncertainty, version);
            }
            let assistant_content = if response.content.trim().is_empty() {
                "Selecting deterministic tools.".to_string()
            } else {
                response.content.clone()
            };
            messages.push(ChatMessage::new("assistant", assistant_content));
            for tool_request in response.tool_requests {
                let envelope = self.registry.dispatch(
                    &tool_request.name,
                    &tool_request.arguments,
                    &mut budget,
                );
                records.push(ToolCallRecord {
                    round,
                    id: tool_request.id.clone(),
                    name: tool_request.name.clone(),
                    arguments: tool_request.arguments.clone(),
                    status: envelope.status,
                    data: envelope.data.clone(),
                    errors: envelope.errors.clone(),
                });
                let envelope_json =
                    serde_json::to_string(&envelope).unwrap_or_else(|_| "{}".to_string());
                for summary in envelope.provenance {
                    provenance.insert(summary);
                }
                for message in envelope.uncertainty {
                    if !uncertainty.contains(&message) {
                        uncertainty.push(message);
                    }
                }
                version = envelope.version;
                messages.push(ChatMessage::new(
                    "user",
                    format!(
                        "TOOL_RESULT {} {}\n{envelope_json}",
                        tool_request.id, tool_request.name
                    ),
                ));
            }
        }

        self.error_answer(
            records,
            provenance,
            uncertainty,
            version,
            "tool call budget exhausted before a final answer",
        )
    }

    fn finalize(
        &self,
        content: String,
        records: Vec<ToolCallRecord>,
        provenance: BTreeSet<ProvenanceSummary>,
        mut uncertainty: Vec<String>,
        version: VersionInfo,
    ) -> AgentAnswer {
        let trimmed = content.trim();
        if trimmed.is_empty() {
            return self.error_answer(
                records,
                provenance,
                uncertainty,
                version,
                "provider returned an empty answer",
            );
        }
        if records.is_empty()
            && !uncertainty
                .iter()
                .any(|message| message.contains("no deterministic tool evidence"))
        {
            uncertainty.push("no deterministic tool evidence was used for this answer".to_string());
        }
        if let Err(violations) =
            grounding_gate(trimmed, &records, &self.registry.known_entity_names())
        {
            let message = format!(
                "grounding gate rejected the model answer: {}",
                violations.join("; ")
            );
            return self.error_answer(records, provenance, uncertainty, version, message);
        }
        let character_count = trimmed.chars().count();
        let answer = if character_count > self.config.max_reply_characters {
            uncertainty.push("reply truncated to the configured limit".to_string());
            trimmed
                .chars()
                .take(self.config.max_reply_characters)
                .collect::<String>()
        } else {
            trimmed.to_string()
        };
        let status = if records
            .iter()
            .any(|record| record.status == ToolStatus::Error)
            && !records.iter().any(|record| record.status == ToolStatus::Ok)
        {
            AgentStatus::Error
        } else if records
            .iter()
            .any(|record| record.status == ToolStatus::Ambiguous)
        {
            AgentStatus::Ambiguous
        } else if records
            .iter()
            .any(|record| record.status == ToolStatus::Unknown)
            || records.is_empty()
        {
            AgentStatus::Unknown
        } else {
            AgentStatus::Ok
        };
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
            answer: Some(answer),
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

fn grounding_gate(
    answer: &str,
    records: &[ToolCallRecord],
    known_entity_names: &BTreeSet<String>,
) -> Result<(), Vec<String>> {
    let calculator_records = records
        .iter()
        .filter(|record| is_calculator_tool(&record.name))
        .collect::<Vec<_>>();
    let breeding_records = records
        .iter()
        .filter(|record| is_breeding_tool(&record.name))
        .collect::<Vec<_>>();
    let successful_calculator_data = successful_data_text(&calculator_records);
    let successful_breeding_data = successful_data_text(&breeding_records);
    let successful_any_data = successful_data_text(&records.iter().collect::<Vec<_>>());
    let mut argument_text = String::new();
    for record in records {
        argument_text.push_str(&serde_json::to_string(&record.arguments).unwrap_or_default());
    }
    let mut breeding_argument_text = String::new();
    for record in &breeding_records {
        breeding_argument_text
            .push_str(&serde_json::to_string(&record.arguments).unwrap_or_default());
    }

    let numeric_evidence = if calculator_records.is_empty() {
        digit_runs(&successful_any_data)
    } else {
        digit_runs(&successful_calculator_data)
    };
    let numeric_claims = numeric_claims(answer);
    let mut violations = Vec::new();
    for (display, value) in &numeric_claims {
        if !numeric_evidence.contains(value) {
            violations.push(format!("unsupported numeric claim \"{display}\""));
        }
    }

    let answer_lower = answer.to_lowercase();
    let entity_evidence = if breeding_records.is_empty() {
        format!("{successful_any_data}{argument_text}")
    } else {
        format!("{successful_breeding_data}{breeding_argument_text}")
    };
    let evidence_lower = entity_evidence.to_lowercase();
    for name in known_entity_names {
        let name_lower = name.to_lowercase();
        if answer_lower.contains(&name_lower) && !evidence_lower.contains(&name_lower) {
            violations.push(format!("unsupported entity claim \"{name}\""));
        }
    }

    let has_successful_calculator = calculator_records
        .iter()
        .any(|record| record.status == ToolStatus::Ok);
    if !has_successful_calculator
        && contains_any_word(&answer_lower, CALCULATION_CONTEXT_WORDS)
        && !numeric_claims.is_empty()
    {
        push_violation(
            &mut violations,
            "unsupported calculation claim; quantities require a successful calculator tool"
                .to_string(),
        );
    }

    let has_successful_breeding = breeding_records
        .iter()
        .any(|record| record.status == ToolStatus::Ok);
    if !has_successful_breeding && contains_any_word(&answer_lower, BREEDING_CONTEXT_WORDS) {
        for name in known_entity_names {
            let name_lower = name.to_lowercase();
            let breeding_arguments_lower = breeding_argument_text.to_lowercase();
            if answer_lower.contains(&name_lower) && !breeding_arguments_lower.contains(&name_lower)
            {
                push_violation(
                    &mut violations,
                    format!("unsupported breeding claim \"{name}\""),
                );
            }
        }
    }

    if violations.is_empty() {
        Ok(())
    } else {
        Err(violations)
    }
}

fn successful_data_text(records: &[&ToolCallRecord]) -> String {
    let mut text = String::new();
    for record in records {
        if record.status == ToolStatus::Ok {
            if let Some(data) = &record.data {
                text.push_str(&serde_json::to_string(data).unwrap_or_default());
            }
        }
    }
    text
}

fn is_calculator_tool(name: &str) -> bool {
    name.starts_with("calculate_")
}

fn is_breeding_tool(name: &str) -> bool {
    name.starts_with("calculate_breeding")
}

const CALCULATION_CONTEXT_WORDS: &[&str] = &[
    "need",
    "needs",
    "needed",
    "require",
    "requires",
    "required",
    "cost",
    "costs",
    "total",
    "totals",
    "material",
    "materials",
    "ingredient",
    "ingredients",
    "craft",
    "crafts",
];

const BREEDING_CONTEXT_WORDS: &[&str] = &[
    "breeding",
    "breed",
    "breeds",
    "produce",
    "produces",
    "produced",
    "hatch",
    "hatches",
    "hatched",
    "offspring",
    "child",
];

const NUMBER_WORDS: &[(&str, &str)] = &[
    ("zero", "0"),
    ("one", "1"),
    ("two", "2"),
    ("three", "3"),
    ("four", "4"),
    ("five", "5"),
    ("six", "6"),
    ("seven", "7"),
    ("eight", "8"),
    ("nine", "9"),
    ("ten", "10"),
    ("eleven", "11"),
    ("twelve", "12"),
    ("thirteen", "13"),
    ("fourteen", "14"),
    ("fifteen", "15"),
    ("sixteen", "16"),
    ("seventeen", "17"),
    ("eighteen", "18"),
    ("nineteen", "19"),
    ("twenty", "20"),
    ("thirty", "30"),
    ("forty", "40"),
    ("fifty", "50"),
    ("sixty", "60"),
    ("seventy", "70"),
    ("eighty", "80"),
    ("ninety", "90"),
];

fn numeric_claims(answer: &str) -> Vec<(String, String)> {
    let normalized = normalize_numerals(answer);
    let mut claims = digit_runs(&normalized)
        .into_iter()
        .map(|run| (run.clone(), run))
        .collect::<Vec<_>>();
    for word in normalized.split(|character: char| !character.is_alphanumeric()) {
        let word_lower = word.to_lowercase();
        if let Some((_, value)) = NUMBER_WORDS.iter().find(|(name, _)| *name == word_lower) {
            claims.push((word.to_string(), (*value).to_string()));
        }
    }
    claims
}

fn normalize_numerals(text: &str) -> String {
    text.chars()
        .map(|character| {
            let code = character as u32;
            match code {
                0xFF10..=0xFF19 => char::from_digit(code - 0xFF10, 10).unwrap_or(character),
                _ => character,
            }
        })
        .collect()
}

fn contains_any_word(text: &str, words: &[&str]) -> bool {
    words.iter().any(|word| {
        text.split(|character: char| !character.is_alphanumeric())
            .any(|token| token == *word)
    })
}

fn push_violation(violations: &mut Vec<String>, violation: String) {
    if !violations.contains(&violation) {
        violations.push(violation);
    }
}

fn digit_runs(text: &str) -> BTreeSet<String> {
    text.split(|character: char| !character.is_ascii_digit())
        .filter(|run| !run.is_empty())
        .map(str::to_string)
        .collect()
}
