//! Bounded, grounded natural-language guide agent.

use guide_core::{ProvenanceSummary, VersionInfo};
use guide_tools::{ToolBudget, ToolRegistry, ToolStatus};
use provider::{ChatMessage, ChatProvider, ChatRequest, ToolSpec};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use state_snapshot::PlayerStateSnapshot;
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

    pub fn set_state_snapshot(&mut self, snapshot: PlayerStateSnapshot) {
        self.registry.set_state_snapshot(snapshot);
    }

    pub fn ask(&self, question: &str) -> AgentAnswer {
        self.ask_with_cancellation(question, &AtomicBool::new(false))
    }

    pub fn ask_with_cancellation(&self, question: &str, cancelled: &AtomicBool) -> AgentAnswer {
        let deadline = Instant::now() + self.config.limits.timeout;
        let mut budget = ToolBudget::new(self.config.limits.max_tool_calls, deadline);
        let definitions = self.registry.definitions();
        let tools = definitions
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
                return self.finalize(
                    question,
                    response.content,
                    records,
                    provenance,
                    uncertainty,
                    version,
                );
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
        question: &str,
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
        if let Err(violations) = grounding_gate(
            trimmed,
            question,
            &records,
            &self.registry.known_entity_names(),
            &self.registry.canonical_entity_names(),
        ) {
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

#[derive(Debug, Clone)]
struct NumericClaim {
    display: String,
    value: f64,
    start: usize,
    end: usize,
}

#[derive(Debug, Clone)]
struct QuantityEvidence {
    value: f64,
    entity: String,
}

#[derive(Debug, Clone)]
struct WordToken {
    text: String,
    start: usize,
    end: usize,
}

fn grounding_gate(
    answer: &str,
    question: &str,
    records: &[ToolCallRecord],
    known_entity_names: &BTreeSet<String>,
    canonical_entity_names: &std::collections::BTreeMap<String, String>,
) -> Result<(), Vec<String>> {
    let calculator_records = records
        .iter()
        .filter(|record| is_calculator_tool(&record.name))
        .collect::<Vec<_>>();
    let breeding_records = records
        .iter()
        .filter(|record| is_breeding_tool(&record.name))
        .collect::<Vec<_>>();
    let successful_any_data = successful_data_text(&records.iter().collect::<Vec<_>>());
    let mut argument_text = String::new();
    for record in records {
        argument_text.push_str(&serde_json::to_string(&record.arguments).unwrap_or_default());
    }
    let matching_breeding_records = breeding_records
        .iter()
        .filter(|record| breeding_record_matches_question(record, question))
        .copied()
        .collect::<Vec<_>>();
    let matching_breeding_entities =
        breeding_permitted_entities(&matching_breeding_records, canonical_entity_names);
    let quantity_evidence = if calculator_records.is_empty() {
        quantity_evidence_records(&records.iter().collect::<Vec<_>>(), false)
    } else {
        quantity_evidence_records(&calculator_records, true)
    };
    let normalized_answer = normalize_numerals(answer);
    let numeric_claims = numeric_claims(&normalized_answer);
    let mut violations = Vec::new();
    for claim in &numeric_claims {
        let claim_entities =
            numeric_claim_context_entities(&normalized_answer, claim, known_entity_names);
        let locale_matches = !contains_cjk(&claim.display) || contains_cjk(question);
        let supported = locale_matches
            && quantity_evidence.iter().any(|evidence| {
                same_number(evidence.value, claim.value)
                    && claim_entities
                        .iter()
                        .any(|name| normalized_words(name) == normalized_words(&evidence.entity))
            });
        if !supported {
            violations.push(format!("unsupported numeric claim \"{}\"", claim.display));
        }
    }

    let answer_lower = normalized_answer.to_lowercase();
    let entity_evidence = if breeding_records.is_empty() {
        format!("{successful_any_data}{argument_text}")
    } else {
        matching_breeding_entities
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join("\n")
    };
    let evidence_lower = entity_evidence.to_lowercase();
    for name in known_entity_names {
        if contains_entity_phrase(&answer_lower, name)
            && !contains_entity_phrase(&evidence_lower, name)
        {
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
    let has_successful_matching_breeding = matching_breeding_records
        .iter()
        .any(|record| record.status == ToolStatus::Ok);
    let question_indicates_breeding =
        contains_any_word(&question.to_lowercase(), BREEDING_CONTEXT_WORDS);
    if contains_any_word(&answer_lower, BREEDING_CONTEXT_WORDS)
        || question_indicates_breeding
        || !breeding_records.is_empty()
    {
        let permitted_entities = matching_breeding_entities.clone();
        let uses_permitted_language =
            breeding_answer_uses_permitted_language(&answer_lower, &permitted_entities);
        if !has_successful_breeding && !has_successful_matching_breeding {
            if !is_explicit_unknown(&answer_lower) {
                push_violation(
                    &mut violations,
                    "unsupported breeding claim; a successful breeding result is required"
                        .to_string(),
                );
            } else if !breeding_unknown_uses_permitted_language(&answer_lower, &permitted_entities)
                || !breeding_unknown_entities_are_context_only(&answer_lower, &permitted_entities)
            {
                push_violation(
                    &mut violations,
                    "unsupported breeding claim after an explicit unknown statement".to_string(),
                );
            }
        } else if !has_successful_matching_breeding {
            if !is_explicit_unknown(&answer_lower) {
                push_violation(
                    &mut violations,
                    "unsupported breeding claim; no successful result matches the requested parent pair"
                        .to_string(),
                );
            } else if !breeding_unknown_uses_permitted_language(&answer_lower, &permitted_entities)
                || !breeding_unknown_entities_are_context_only(&answer_lower, &permitted_entities)
            {
                push_violation(
                    &mut violations,
                    "unsupported breeding claim after an explicit unknown statement".to_string(),
                );
            }
        } else if !uses_permitted_language {
            push_violation(
                &mut violations,
                "unsupported breeding claim beyond successful tool facts".to_string(),
            );
        }

        for name in known_entity_names {
            let name_permitted = permitted_entities.iter().any(|permitted| {
                permitted.eq_ignore_ascii_case(name) || contains_entity_phrase(permitted, name)
            });
            if contains_entity_phrase(&answer_lower, name) && !name_permitted {
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

const BREEDING_ALLOWED_WORDS: &[&str] = &[
    "a",
    "an",
    "the",
    "and",
    "plus",
    "for",
    "is",
    "are",
    "was",
    "were",
    "produces",
    "produce",
    "produced",
    "offspring",
    "child",
    "result",
    "results",
    "breeding",
    "breed",
    "breeds",
    "no",
    "reviewed",
    "available",
    "unknown",
    "not",
    "unavailable",
    "cannot",
    "determine",
    "requested",
    "pair",
    "combination",
    "matches",
    "with",
    "未知",
    "未知结果",
    "育种",
    "结果",
    "后代",
    "孩子",
    "无法",
    "确定",
    "没有",
    "可用",
    "不可用",
    "是",
    "和",
    "与",
    "对于",
];

const UNKNOWN_BREEDING_ALLOWED_WORDS: &[&str] = &[
    "a",
    "an",
    "the",
    "and",
    "plus",
    "for",
    "is",
    "are",
    "was",
    "were",
    "result",
    "results",
    "breeding",
    "breed",
    "breeds",
    "no",
    "reviewed",
    "available",
    "unknown",
    "not",
    "unavailable",
    "cannot",
    "determine",
    "requested",
    "pair",
    "combination",
    "matches",
    "with",
    "未知",
    "未知结果",
    "育种",
    "结果",
    "无法",
    "确定",
    "没有",
    "可用",
    "不可用",
    "是",
    "和",
    "与",
    "对于",
];

const BREEDING_CONTEXT_PRECEDERS: &[&str] = &[
    "for",
    "of",
    "between",
    "with",
    "and",
    "plus",
    "requested",
    "pair",
    "combination",
    "matches",
    "breeding",
    "breed",
    "breeds",
    "from",
    "to",
    "parent",
    "parents",
    "using",
    "和",
    "与",
    "对于",
    "之间",
    "父母",
    "从",
    "到",
];

const NUMBER_WORDS: &[(&str, f64)] = &[
    ("zero", 0.0),
    ("one", 1.0),
    ("two", 2.0),
    ("three", 3.0),
    ("four", 4.0),
    ("five", 5.0),
    ("six", 6.0),
    ("seven", 7.0),
    ("eight", 8.0),
    ("nine", 9.0),
    ("ten", 10.0),
    ("eleven", 11.0),
    ("twelve", 12.0),
    ("thirteen", 13.0),
    ("fourteen", 14.0),
    ("fifteen", 15.0),
    ("sixteen", 16.0),
    ("seventeen", 17.0),
    ("eighteen", 18.0),
    ("nineteen", 19.0),
    ("twenty", 20.0),
    ("thirty", 30.0),
    ("forty", 40.0),
    ("fifty", 50.0),
    ("sixty", 60.0),
    ("seventy", 70.0),
    ("eighty", 80.0),
    ("ninety", 90.0),
    ("hundred", 100.0),
    ("thousand", 1_000.0),
    ("million", 1_000_000.0),
    ("零", 0.0),
    ("〇", 0.0),
    ("一", 1.0),
    ("壹", 1.0),
    ("二", 2.0),
    ("贰", 2.0),
    ("两", 2.0),
    ("三", 3.0),
    ("叁", 3.0),
    ("四", 4.0),
    ("肆", 4.0),
    ("五", 5.0),
    ("伍", 5.0),
    ("六", 6.0),
    ("陆", 6.0),
    ("七", 7.0),
    ("柒", 7.0),
    ("八", 8.0),
    ("捌", 8.0),
    ("九", 9.0),
    ("玖", 9.0),
    ("十", 10.0),
    ("拾", 10.0),
    ("百", 100.0),
    ("佰", 100.0),
    ("千", 1_000.0),
    ("仟", 1_000.0),
    ("万", 10_000.0),
    ("萬", 10_000.0),
    ("亿", 100_000_000.0),
    ("億", 100_000_000.0),
];

fn numeric_claims(answer: &str) -> Vec<NumericClaim> {
    let tokens = word_tokens(answer);
    let mut claims = numeric_literal_claims(answer);
    claims.extend(word_number_claims(answer, &tokens, &claims));
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

fn successful_calculator_records<'a>(records: &'a [&'a ToolCallRecord]) -> Vec<&'a ToolCallRecord> {
    records
        .iter()
        .copied()
        .filter(|record| record.status == ToolStatus::Ok)
        .collect()
}

fn quantity_evidence_records(
    records: &[&ToolCallRecord],
    include_arguments: bool,
) -> Vec<QuantityEvidence> {
    let mut evidence = Vec::new();
    for record in successful_calculator_records(records) {
        if let Some(data) = &record.data {
            collect_quantity_evidence(data, &Vec::new(), &mut evidence);
        }
        if include_arguments {
            collect_quantity_evidence(&record.arguments, &Vec::new(), &mut evidence);
        }
        if record.name == "calculate_craftable_count" {
            if let (Some(data), Some(query)) = (
                record.data.as_ref(),
                record.arguments.get("query").and_then(Value::as_str),
            ) {
                if let Some(maximum) = data.get("maximum_additional_count").and_then(Value::as_f64)
                {
                    evidence.push(QuantityEvidence {
                        value: maximum,
                        entity: query.to_string(),
                    });
                }
            }
        }
    }
    evidence
}

fn collect_quantity_evidence(
    value: &Value,
    inherited_context: &[String],
    evidence: &mut Vec<QuantityEvidence>,
) {
    match value {
        Value::Object(fields) => {
            let local_context = fields
                .iter()
                .filter_map(|(key, value)| {
                    if matches!(
                        key.as_str(),
                        "item_name" | "target_name" | "query" | "parent_a" | "parent_b"
                    ) {
                        value.as_str().map(str::to_string)
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            let context = if local_context.is_empty() {
                inherited_context
            } else {
                &local_context
            };
            for (key, field) in fields {
                if let Some(number) = field.as_f64() {
                    if !matches!(key.as_str(), "confidence" | "latitude" | "longitude") {
                        for entity in context {
                            evidence.push(QuantityEvidence {
                                value: number,
                                entity: entity.clone(),
                            });
                        }
                    }
                }
                collect_quantity_evidence(field, context, evidence);
            }
        }
        Value::Array(values) => {
            for value in values {
                collect_quantity_evidence(value, inherited_context, evidence);
            }
        }
        _ => {}
    }
}

fn sentence_bounds(text: &str, start: usize, end: usize) -> (usize, usize) {
    let sentence_start = text[..start]
        .rfind(['.', '!', '?', ';'])
        .map(|index| index + 1)
        .unwrap_or(0);
    let sentence_end = text[end..]
        .find(['.', '!', '?', ';'])
        .map(|offset| end + offset)
        .unwrap_or(text.len());
    (sentence_start, sentence_end)
}

fn numeric_claim_context_entities(
    text: &str,
    claim: &NumericClaim,
    known_entity_names: &BTreeSet<String>,
) -> BTreeSet<String> {
    let (_, sentence_end) = sentence_bounds(text, claim.start, claim.end);
    let tokens = word_tokens(text);
    let after_start = tokens
        .iter()
        .find(|token| token.start >= claim.end)
        .map(|token| token.start)
        .unwrap_or(claim.end);
    let entity_phrases = known_entity_names
        .iter()
        .map(|name| normalized_words(name))
        .filter(|phrase| !phrase.is_empty())
        .collect::<Vec<_>>();
    let mut after_end = sentence_end;
    let mut phrase_words = Vec::new();
    for token in tokens.iter().filter(|token| token.start >= after_start) {
        if token.start >= sentence_end {
            break;
        }
        phrase_words.push(token.text.clone());
        if !entity_phrases.iter().any(|phrase| {
            phrase_words.len() <= phrase.len()
                && phrase_words
                    .iter()
                    .zip(phrase.iter())
                    .all(|(word, expected)| same_word(word, expected))
        }) {
            after_end = token.start;
            break;
        }
    }
    let mut entities = BTreeSet::new();
    let after = &text[after_start..after_end];
    for name in known_entity_names {
        if contains_entity_phrase(after, name) {
            entities.insert(name.clone());
        }
    }
    entities
}

fn contains_entity_phrase(text: &str, entity: &str) -> bool {
    let text_words = normalized_words(text);
    let entity_words = normalized_words(entity);
    if entity_words.is_empty() || text_words.len() < entity_words.len() {
        return false;
    }
    text_words.windows(entity_words.len()).any(|window| {
        window
            .iter()
            .zip(&entity_words)
            .all(|(text, expected)| same_word(text, expected))
    })
}

fn normalized_words(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|character: char| !character.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_string)
        .collect()
}

fn contains_cjk(text: &str) -> bool {
    text.chars().any(|character| {
        matches!(
            character as u32,
            0x4E00..=0x9FFF | 0x3400..=0x4DBF | 0xF900..=0xFAFF
        )
    })
}

fn same_word(left: &str, right: &str) -> bool {
    left == right
        || singular_word(left) == singular_word(right)
        || left == singular_word(right)
        || singular_word(left) == right
}

fn singular_word(word: &str) -> &str {
    word.strip_suffix('s').unwrap_or(word)
}

fn same_number(left: f64, right: f64) -> bool {
    (left - right).abs() <= f64::EPSILON * left.abs().max(right.abs()).max(1.0)
}

fn word_tokens(text: &str) -> Vec<WordToken> {
    let mut tokens = Vec::new();
    let mut iterator = text.char_indices().peekable();
    while let Some((start, character)) = iterator.next() {
        if is_chinese_number_character(character) {
            tokens.push(WordToken {
                text: character.to_string(),
                start,
                end: start + character.len_utf8(),
            });
            continue;
        }
        if !character.is_alphanumeric() {
            continue;
        }
        let mut end = start + character.len_utf8();
        while let Some(&(next_start, next_character)) = iterator.peek() {
            if !next_character.is_alphanumeric() {
                break;
            }
            iterator.next();
            end = next_start + next_character.len_utf8();
        }
        tokens.push(WordToken {
            text: text[start..end].to_lowercase(),
            start,
            end,
        });
    }
    tokens
}

fn is_chinese_number_character(character: char) -> bool {
    matches!(
        character,
        '零' | '〇'
            | '一'
            | '壹'
            | '二'
            | '贰'
            | '两'
            | '三'
            | '叁'
            | '四'
            | '肆'
            | '五'
            | '伍'
            | '六'
            | '陆'
            | '七'
            | '柒'
            | '八'
            | '捌'
            | '九'
            | '玖'
            | '十'
            | '拾'
            | '百'
            | '佰'
            | '千'
            | '仟'
            | '万'
            | '萬'
            | '亿'
            | '億'
            | '点'
            | '點'
    )
}

fn numeric_literal_claims(text: &str) -> Vec<NumericClaim> {
    let mut claims = Vec::new();
    let characters = text.char_indices().collect::<Vec<_>>();
    let mut index = 0;
    while index < characters.len() {
        let next_is_number = characters.get(index + 1).is_some_and(|(_, character)| {
            character.is_ascii_digit()
                || (*character == '.'
                    && characters
                        .get(index + 2)
                        .is_some_and(|(_, character)| character.is_ascii_digit()))
        });
        let starts_number = characters[index].1.is_ascii_digit()
            || (characters[index].1 == '.' && next_is_number)
            || (matches!(characters[index].1, '+' | '-') && next_is_number);
        if !starts_number {
            index += 1;
            continue;
        }
        let start = characters[index].0;
        if matches!(characters[index].1, '+' | '-') {
            index += 1;
        }
        let mut end_index = index;
        let mut seen_decimal_point = false;
        while end_index + 1 < characters.len() {
            let character = characters[end_index + 1].1;
            if character.is_ascii_digit() {
                end_index += 1;
            } else if character == '.' && !seen_decimal_point {
                seen_decimal_point = true;
                end_index += 1;
            } else {
                break;
            }
        }
        if end_index + 2 < characters.len()
            && matches!(characters[end_index + 1].1, 'e' | 'E')
            && (characters[end_index + 2].1.is_ascii_digit()
                || (matches!(characters[end_index + 2].1, '+' | '-')
                    && characters
                        .get(end_index + 3)
                        .is_some_and(|(_, character)| character.is_ascii_digit())))
        {
            end_index += 2;
            if matches!(characters[end_index].1, '+' | '-') {
                end_index += 1;
            }
            while end_index + 1 < characters.len() && characters[end_index + 1].1.is_ascii_digit() {
                end_index += 1;
            }
        }
        let end = characters[end_index].0 + characters[end_index].1.len_utf8();
        let mut value = text[start..end].parse::<f64>().unwrap_or_default();
        let mut display_end = end;
        let tokens = word_tokens(text);
        if let Some(token) = tokens.iter().find(|token| {
            token.start >= end && matches!(token.text.as_str(), "hundred" | "thousand" | "million")
        }) {
            if let Some((_, scale)) = NUMBER_WORDS.iter().find(|(name, _)| *name == token.text) {
                value *= scale;
                display_end = token.end;
            }
        }
        claims.push(NumericClaim {
            display: text[start..display_end].to_string(),
            value,
            start,
            end: display_end,
        });
        index = end_index + 1;
    }
    claims
}

fn word_number_claims(
    text: &str,
    tokens: &[WordToken],
    literal_claims: &[NumericClaim],
) -> Vec<NumericClaim> {
    let mut claims = Vec::new();
    let mut index = 0;
    while index < tokens.len() {
        let token = &tokens[index];
        if literal_claims
            .iter()
            .any(|claim| token.start < claim.end && token.end > claim.start)
        {
            index += 1;
            continue;
        }
        if let Some((value, length)) = parse_fraction_at(tokens, index) {
            claims.push(NumericClaim {
                display: text[token.start..tokens[index + length - 1].end].to_string(),
                value,
                start: token.start,
                end: tokens[index + length - 1].end,
            });
            index += length;
        } else if let Some((value, length)) = parse_cardinal_at(tokens, index) {
            claims.push(NumericClaim {
                display: text[token.start..tokens[index + length - 1].end].to_string(),
                value,
                start: token.start,
                end: tokens[index + length - 1].end,
            });
            index += length;
        } else {
            index += 1;
        }
    }
    claims
}

fn parse_fraction_at(tokens: &[WordToken], index: usize) -> Option<(f64, usize)> {
    if let Some(decimal) = parse_chinese_decimal_at(tokens, index) {
        return Some(decimal);
    }
    if let Some(denominator) = fraction_denominator(&tokens.get(index)?.text) {
        return Some((1.0 / denominator, 1));
    }

    let (whole, mut length) = parse_cardinal_at(tokens, index)?;
    if tokens.get(index + length)?.text == "and" {
        let numerator_index = index + length + 1;
        let numerator_token = tokens.get(numerator_index)?;
        let (numerator, numerator_length) = if numerator_token.text == "a" {
            (1.0, 1)
        } else {
            let value = NUMBER_WORDS
                .iter()
                .find(|(name, _)| *name == numerator_token.text)
                .filter(|(_, value)| *value < 100.0)
                .map(|(_, value)| *value)?;
            (value, 1)
        };
        let denominator_index = numerator_index + numerator_length;
        let denominator = fraction_denominator(&tokens.get(denominator_index)?.text)?;
        length = denominator_index + 1 - index;
        return Some((whole + numerator / denominator, length));
    }

    let denominator = fraction_denominator(&tokens.get(index + length)?.text)?;
    Some((whole / denominator, length + 1))
}

fn parse_cardinal_at(tokens: &[WordToken], index: usize) -> Option<(f64, usize)> {
    number_word_value(&tokens.get(index)?.text)?;
    let mut end = index;
    while end < tokens.len() {
        if tokens[end].text == "and" {
            let next = tokens.get(end + 1)?;
            if number_word_value(&next.text).is_none() {
                break;
            }
            end += 1;
            continue;
        }
        if number_word_value(&tokens[end].text).is_none() {
            break;
        }
        end += 1;
    }

    let mut total = 0.0;
    let mut current = 0.0;
    for token in &tokens[index..end] {
        let value = number_word_value(&token.text)?;
        if matches!(token.text.as_str(), "hundred" | "百" | "佰") {
            current = if current == 0.0 {
                value
            } else {
                current * value
            };
        } else if matches!(
            token.text.as_str(),
            "thousand" | "千" | "仟" | "万" | "萬" | "million" | "亿" | "億"
        ) {
            total += if current == 0.0 {
                value
            } else {
                current * value
            };
            current = 0.0;
        } else if token.text != "and" {
            current += value;
        }
    }
    Some((total + current, end - index))
}

fn parse_chinese_decimal_at(tokens: &[WordToken], index: usize) -> Option<(f64, usize)> {
    let (whole, length) = parse_cardinal_at(tokens, index)?;
    if !matches!(tokens.get(index + length)?.text.as_str(), "点" | "點") {
        return None;
    }
    let mut decimal_index = index + length + 1;
    let mut decimal = 0.0;
    let mut place = 10.0;
    let mut decimal_length = 0;
    while let Some(token) = tokens.get(decimal_index) {
        let Some(digit) =
            number_word_value(&token.text).filter(|value| *value >= 0.0 && *value < 10.0)
        else {
            break;
        };
        decimal += digit / place;
        place *= 10.0;
        decimal_index += 1;
        decimal_length += 1;
    }
    if decimal_length == 0 {
        return None;
    }
    Some((whole + decimal, length + decimal_length + 1))
}

fn number_word_value(word: &str) -> Option<f64> {
    NUMBER_WORDS
        .iter()
        .find(|(name, _)| *name == word)
        .map(|(_, value)| *value)
}

fn fraction_denominator(word: &str) -> Option<f64> {
    match word {
        "half" | "halves" => Some(2.0),
        "third" | "thirds" => Some(3.0),
        "quarter" | "quarters" | "fourth" | "fourths" => Some(4.0),
        "fifth" | "fifths" => Some(5.0),
        "eighth" | "eighths" => Some(8.0),
        _ => None,
    }
}

fn breeding_record_matches_question(record: &ToolCallRecord, question: &str) -> bool {
    let parent_a = record.arguments.get("parent_a").and_then(Value::as_str);
    let parent_b = record.arguments.get("parent_b").and_then(Value::as_str);
    match (parent_a, parent_b) {
        (Some(parent_a), Some(parent_b)) => {
            if normalized_words(parent_a) == normalized_words(parent_b) {
                count_entity_phrase(question, parent_a) >= 2
            } else {
                contains_entity_phrase(question, parent_a)
                    && contains_entity_phrase(question, parent_b)
            }
        }
        _ => false,
    }
}

fn count_entity_phrase(text: &str, entity: &str) -> usize {
    let text_words = normalized_words(text);
    let entity_words = normalized_words(entity);
    if entity_words.is_empty() || text_words.len() < entity_words.len() {
        return 0;
    }
    text_words
        .windows(entity_words.len())
        .filter(|window| {
            window
                .iter()
                .zip(&entity_words)
                .all(|(text, expected)| same_word(text, expected))
        })
        .count()
}

fn breeding_permitted_entities(
    records: &[&ToolCallRecord],
    canonical_entity_names: &std::collections::BTreeMap<String, String>,
) -> BTreeSet<String> {
    let mut entities = BTreeSet::new();
    for record in records {
        for key in ["parent_a", "parent_b"] {
            if let Some(parent) = record.arguments.get(key).and_then(Value::as_str) {
                entities.insert(parent.to_string());
            }
        }
        if record.status != ToolStatus::Ok {
            continue;
        }
        if let Some(data) = &record.data {
            collect_breeding_entities(data, canonical_entity_names, &mut entities);
        }
    }
    entities
}

fn collect_breeding_entities(
    value: &Value,
    canonical_entity_names: &std::collections::BTreeMap<String, String>,
    entities: &mut BTreeSet<String>,
) {
    match value {
        Value::Object(fields) => {
            for (key, field) in fields {
                if let Some(identifier) = field.as_str() {
                    if matches!(
                        key.as_str(),
                        "parent_a_id" | "parent_b_id" | "child_id" | "start_id" | "target_id"
                    ) {
                        if let Some(name) = canonical_entity_names.get(identifier) {
                            entities.insert(name.clone());
                        }
                    }
                }
                collect_breeding_entities(field, canonical_entity_names, entities);
            }
        }
        Value::Array(values) => {
            for value in values {
                collect_breeding_entities(value, canonical_entity_names, entities);
            }
        }
        _ => {}
    }
}

fn breeding_answer_uses_permitted_language(
    answer_lower: &str,
    permitted_entities: &BTreeSet<String>,
) -> bool {
    breeding_retained_words(answer_lower, permitted_entities)
        .iter()
        .all(|word| BREEDING_ALLOWED_WORDS.contains(&word.as_str()))
}

fn breeding_unknown_uses_permitted_language(
    answer_lower: &str,
    permitted_entities: &BTreeSet<String>,
) -> bool {
    breeding_retained_words(answer_lower, permitted_entities)
        .iter()
        .all(|word| UNKNOWN_BREEDING_ALLOWED_WORDS.contains(&word.as_str()))
}

fn breeding_retained_words(
    answer_lower: &str,
    permitted_entities: &BTreeSet<String>,
) -> Vec<String> {
    let words = normalized_words(answer_lower);
    let permitted_phrases = permitted_entities
        .iter()
        .map(|entity| normalized_words(entity))
        .filter(|phrase| !phrase.is_empty())
        .collect::<Vec<_>>();
    let mut retained = Vec::new();
    let mut index = 0;
    while index < words.len() {
        let matched_length = permitted_phrases
            .iter()
            .filter(|phrase| index + phrase.len() <= words.len())
            .filter(|phrase| {
                words[index..index + phrase.len()]
                    .iter()
                    .zip(phrase.iter())
                    .all(|(word, expected)| same_word(word, expected))
            })
            .map(|phrase| phrase.len())
            .max()
            .unwrap_or(0);
        if matched_length > 0 {
            index += matched_length;
        } else {
            retained.push(words[index].clone());
            index += 1;
        }
    }
    retained
}

fn breeding_unknown_entities_are_context_only(
    answer_lower: &str,
    permitted_entities: &BTreeSet<String>,
) -> bool {
    let words = normalized_words(answer_lower);
    let permitted_phrases = permitted_entities
        .iter()
        .map(|entity| normalized_words(entity))
        .filter(|phrase| !phrase.is_empty())
        .collect::<Vec<_>>();
    let mut index = 0;
    while index < words.len() {
        let matched_length = permitted_phrases
            .iter()
            .filter(|phrase| index + phrase.len() <= words.len())
            .filter(|phrase| {
                words[index..index + phrase.len()]
                    .iter()
                    .zip(phrase.iter())
                    .all(|(word, expected)| same_word(word, expected))
            })
            .map(|phrase| phrase.len())
            .max()
            .unwrap_or(0);
        if matched_length > 0 {
            if index > 0 && !BREEDING_CONTEXT_PRECEDERS.contains(&words[index - 1].as_str()) {
                return false;
            }
            index += matched_length;
        } else {
            index += 1;
        }
    }
    true
}

fn is_explicit_unknown(answer_lower: &str) -> bool {
    [
        "unknown",
        "no reviewed",
        "no result",
        "not available",
        "unavailable",
        "cannot determine",
    ]
    .iter()
    .any(|phrase| answer_lower.contains(phrase))
}

fn push_violation(violations: &mut Vec<String>, violation: String) {
    if !violations.contains(&violation) {
        violations.push(violation);
    }
}
