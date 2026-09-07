use crate::ToolCallRecord;
use guide_core::VersionInfo;
use guide_tools::ToolStatus;
use provider::ToolSpec;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

fn submit_answer_parameters() -> Value {
    json!({
    "type": "object",
    "additionalProperties": false,
    "required": ["status", "sentences", "slots"],
    "properties": {
        "status": {"type": "string", "enum": ["ok", "unknown", "ambiguous"]},
        "sentences": {
            "type": "array",
            "items": {"type": "string"},
            "minItems": 1,
            "maxItems": 4
        },
        "steps": {
            "type": "array",
            "items": {"type": "string"},
            "maxItems": 5
        },
        "slots": {"type": "array", "items": {"type": "string"}},
        "uncertainty": {"type": "array", "items": {"type": "string"}}
    }
    })
}

const ENGLISH_NUMBER_WORDS: &[&str] = &[
    "zero",
    "one",
    "two",
    "three",
    "four",
    "five",
    "six",
    "seven",
    "eight",
    "nine",
    "ten",
    "eleven",
    "twelve",
    "thirteen",
    "fourteen",
    "fifteen",
    "sixteen",
    "seventeen",
    "eighteen",
    "nineteen",
    "twenty",
    "thirty",
    "forty",
    "fifty",
    "sixty",
    "seventy",
    "eighty",
    "ninety",
    "hundred",
    "thousand",
    "million",
    "first",
    "second",
    "third",
    "fourth",
    "fifth",
    "sixth",
    "seventh",
    "eighth",
    "ninth",
    "tenth",
    "eleventh",
    "twelfth",
    "thirteenth",
    "fourteenth",
    "fifteenth",
    "sixteenth",
    "seventeenth",
    "eighteenth",
    "nineteenth",
    "twentieth",
    "thirtieth",
    "fortieth",
    "fiftieth",
    "sixtieth",
    "seventieth",
    "eightieth",
    "ninetieth",
];

const CHINESE_NUMBER_CHARACTERS: &[char] = &[
    '零', '〇', '一', '壹', '二', '贰', '两', '三', '叁', '四', '肆', '五', '伍', '六', '陆', '七',
    '柒', '八', '捌', '九', '玖', '十', '拾', '百', '佰', '千', '仟', '万', '萬', '亿', '億',
];

#[derive(Debug, Clone, PartialEq)]
pub enum FactSlot {
    Quantity {
        id: String,
        value: f64,
        entity: String,
        unit: Option<String>,
        source_tool: String,
    },
    Entity {
        id: String,
        name: String,
        source_tool: String,
    },
    Observation {
        id: String,
        axis: String,
        value: f64,
        source_tool: String,
    },
    Version {
        id: String,
        text: String,
    },
}

impl FactSlot {
    fn id(&self) -> &str {
        match self {
            Self::Quantity { id, .. }
            | Self::Entity { id, .. }
            | Self::Observation { id, .. }
            | Self::Version { id, .. } => id,
        }
    }

    fn display(&self) -> String {
        match self {
            Self::Quantity { value, .. } => format_number(*value),
            Self::Entity { name, .. } => name.clone(),
            Self::Observation { axis, value, .. } => {
                format!("{axis}={}", format_number(*value))
            }
            Self::Version { text, .. } => text.clone(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct FactSheet {
    slots: Vec<FactSlot>,
    entity_evidence: BTreeSet<String>,
    known_entity_names: BTreeSet<String>,
}

impl FactSheet {
    pub fn build(
        records: &[ToolCallRecord],
        known_entity_names: BTreeSet<String>,
        entity_names_by_id: BTreeMap<String, String>,
        version: &VersionInfo,
    ) -> Self {
        let mut quantities = Vec::new();
        let mut observations = Vec::new();
        let mut entity_evidence = BTreeSet::new();

        for record in records {
            collect_entity_names(
                &record.arguments,
                &known_entity_names,
                &entity_names_by_id,
                &mut entity_evidence,
            );
            if record.status != ToolStatus::Ok {
                continue;
            }
            if let Some(data) = &record.data {
                collect_entity_names(
                    data,
                    &known_entity_names,
                    &entity_names_by_id,
                    &mut entity_evidence,
                );
                if is_observation_tool(&record.name) {
                    collect_observations(data, &record.name, &mut observations);
                } else if let Some(summary) = calculator_summary(record, data) {
                    let summary_value = match summary {
                        CalculatorSummary::Totals(value) | CalculatorSummary::Quantity(value) => {
                            value
                        }
                    };
                    collect_quantities(
                        &summary_value,
                        &Vec::new(),
                        &record.name,
                        &mut quantities,
                        &entity_names_by_id,
                    );
                } else if record.name == "get_recipe" {
                    collect_quantities(
                        data,
                        &Vec::new(),
                        &record.name,
                        &mut quantities,
                        &entity_names_by_id,
                    );
                }
            }
        }
        quantities.dedup_by(|left, right| {
            left.value == right.value && left.entity == right.entity && left.unit == right.unit
        });

        let mut slots = Vec::new();
        for (index, quantity) in quantities.into_iter().enumerate() {
            slots.push(FactSlot::Quantity {
                id: format!("q{}", index + 1),
                value: quantity.value,
                entity: quantity.entity,
                unit: quantity.unit,
                source_tool: quantity.source_tool,
            });
        }
        for (index, name) in entity_evidence.iter().enumerate() {
            slots.push(FactSlot::Entity {
                id: format!("e{}", index + 1),
                name: name.clone(),
                source_tool: records
                    .iter()
                    .find(|record| {
                        record.status == ToolStatus::Ok
                            && record.arguments.to_string().contains(name.as_str())
                    })
                    .map(|record| record.name.clone())
                    .unwrap_or_else(|| "structured_tool".to_string()),
            });
        }
        for (index, observation) in observations.into_iter().enumerate() {
            slots.push(FactSlot::Observation {
                id: format!("o{}", index + 1),
                axis: observation.axis,
                value: observation.value,
                source_tool: observation.source_tool,
            });
        }

        let mut version_values = vec![version.knowledge_version.clone()];
        if let Some(configured) = &version.configured_game_version {
            version_values.push(configured.clone());
        }
        for (index, text) in version_values.into_iter().enumerate() {
            slots.push(FactSlot::Version {
                id: format!("v{}", index + 1),
                text,
            });
        }

        Self {
            slots,
            entity_evidence,
            known_entity_names,
        }
    }

    pub fn summary(&self) -> String {
        if self.slots.is_empty() {
            return "FACT_SHEET: none".to_string();
        }
        let lines = self
            .slots
            .iter()
            .map(|slot| match slot {
                FactSlot::Quantity {
                    id,
                    value,
                    entity,
                    unit,
                    source_tool,
                } => format!(
                    "[{id}] quantity value={} entity=\"{entity}\" unit=\"{}\" source={source_tool}",
                    format_number(*value),
                    unit.as_deref().unwrap_or("")
                ),
                FactSlot::Entity {
                    id,
                    name,
                    source_tool,
                } => format!("[{id}] entity name=\"{name}\" source={source_tool}"),
                FactSlot::Observation {
                    id,
                    axis,
                    value,
                    source_tool,
                } => format!(
                    "[{id}] observation axis={axis} value={} source={source_tool}",
                    format_number(*value)
                ),
                FactSlot::Version { id, text } => {
                    format!("[{id}] version text=\"{text}\"")
                }
            })
            .collect::<Vec<_>>();
        format!("FACT_SHEET:\n{}", lines.join("\n"))
    }

    fn slot(&self, id: &str) -> Option<&FactSlot> {
        self.slots.iter().find(|slot| slot.id() == id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum AnswerStatus {
    Ok,
    Unknown,
    Ambiguous,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnswerDraft {
    pub status: AnswerStatus,
    pub sentences: Vec<String>,
    pub steps: Option<Vec<String>>,
    pub slots: Vec<String>,
    pub uncertainty: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderError {
    UnknownSlot(String),
    InvalidSlotReference,
    NumericLiteral,
    NumericWord,
    UnsupportedEntity(String),
    EmptyDraft,
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownSlot(id) => write!(formatter, "unknown slot {id}"),
            Self::InvalidSlotReference => write!(formatter, "invalid slot reference"),
            Self::NumericLiteral => write!(formatter, "numeric literal in model draft"),
            Self::NumericWord => write!(formatter, "numeric word in model draft"),
            Self::UnsupportedEntity(entity) => write!(formatter, "unsupported entity {entity}"),
            Self::EmptyDraft => write!(formatter, "empty draft"),
        }
    }
}

pub fn submit_answer_tool() -> ToolSpec {
    ToolSpec {
        name: "submit_answer".to_string(),
        description: "Submit the final answer. Use natural sentences for non-numeric facts from GROUNDING or tool results; numbers and exact quantities must use slot references from FACT_SHEET."
            .to_string(),
        parameters_schema: submit_answer_parameters(),
    }
}

pub fn render(
    draft: &AnswerDraft,
    fact_sheet: &FactSheet,
    max_reply_characters: usize,
) -> Result<String, RenderError> {
    if draft.sentences.is_empty() || draft.sentences.len() > 4 {
        return Err(RenderError::EmptyDraft);
    }
    if let Some(steps) = &draft.steps {
        if steps.len() > 5 {
            return Err(RenderError::EmptyDraft);
        }
    }
    let mut texts = draft.sentences.clone();
    if let Some(steps) = &draft.steps {
        texts.extend(steps.iter().cloned());
    }
    if let Some(uncertainty) = &draft.uncertainty {
        texts.extend(uncertainty.iter().cloned());
    }

    for text in &texts {
        let rendered = replace_slots(text, fact_sheet, &draft.slots)?;
        reject_numbers(&rendered.without_slots)?;
        reject_number_words(&rendered.without_slots)?;
        reject_unsupported_entities(
            &rendered.replaced,
            &fact_sheet.entity_evidence,
            &fact_sheet.known_entity_names,
        )?;
    }

    let mut output = draft
        .sentences
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    if let Some(steps) = &draft.steps {
        output.extend(steps.iter().map(String::as_str));
    }
    let answer = output
        .iter()
        .enumerate()
        .map(|(index, text)| {
            if index < draft.sentences.len() {
                render_text(text, fact_sheet)
            } else {
                format!(
                    "{}. {}",
                    index - draft.sentences.len() + 1,
                    render_text(text, fact_sheet)
                )
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    if answer.trim().is_empty() {
        return Err(RenderError::EmptyDraft);
    }
    Ok(truncate_characters(&answer, max_reply_characters))
}

pub fn fallback_render(
    fact_sheet: &FactSheet,
    uncertainty: &[String],
    max_reply_characters: usize,
) -> String {
    let mut lines = Vec::new();
    for slot in &fact_sheet.slots {
        if let FactSlot::Quantity {
            value,
            entity,
            unit,
            ..
        } = slot
        {
            let unit = unit.as_deref().filter(|unit| !unit.is_empty());
            let target = unit.unwrap_or(entity);
            lines.push(format!("Need {} {target}.", format_number(*value)));
        }
    }
    let observations = fact_sheet
        .slots
        .iter()
        .filter_map(|slot| match slot {
            FactSlot::Observation { axis, value, .. } => {
                Some(format!("{axis}={}", format_number(*value)))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    if !observations.is_empty() {
        lines.push(format!("Position: {}", observations.join(", ")));
    }
    if lines.is_empty() {
        let entities = fact_sheet
            .slots
            .iter()
            .filter_map(|slot| match slot {
                FactSlot::Entity { name, .. } => Some(name.as_str()),
                _ => None,
            })
            .filter(|name| name.is_ascii())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .take(6)
            .collect::<Vec<_>>();
        if entities.is_empty() {
            lines.push("I don't have enough reviewed data to answer that.".to_string());
        } else {
            lines.push(format!(
                "Related reviewed records: {}.",
                entities.join(", ")
            ));
        }
    }
    if has_player_facing_uncertainty(uncertainty) {
        lines.push(
            "Note: some reviewed records are missing, conflicting, or out of date.".to_string(),
        );
    }
    truncate_characters(&lines.join("\n"), max_reply_characters)
}

fn has_player_facing_uncertainty(uncertainty: &[String]) -> bool {
    uncertainty
        .iter()
        .any(|message| !is_internal_uncertainty(message))
}

fn is_internal_uncertainty(message: &str) -> bool {
    const INTERNAL_MARKERS: &[&str] = &[
        "model draft",
        "model did not call submit_answer",
        "no deterministic tool evidence",
        "tool call budget exhausted",
        "unknown slot",
        "unsupported entity",
        "invalid slot reference",
        "empty draft",
        "numeric literal",
        "numeric word",
        "invalid arguments",
    ];
    INTERNAL_MARKERS
        .iter()
        .any(|marker| message.contains(marker))
        || message.contains('{')
        || message.contains('}')
}

struct ReplacedText {
    replaced: String,
    without_slots: String,
}

fn render_text(text: &str, fact_sheet: &FactSheet) -> String {
    replace_slots(text, fact_sheet, &all_slot_ids(fact_sheet))
        .map(|result| result.replaced)
        .unwrap_or_else(|_| text.to_string())
}

fn replace_slots(
    text: &str,
    fact_sheet: &FactSheet,
    allowed_slots: &[String],
) -> Result<ReplacedText, RenderError> {
    let mut replaced = String::with_capacity(text.len());
    let mut without_slots = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find('{') {
        let prefix = &rest[..start];
        replaced.push_str(prefix);
        without_slots.push_str(prefix);
        let remainder = &rest[start + 1..];
        let Some(relative_end) = remainder.find('}') else {
            return Err(RenderError::InvalidSlotReference);
        };
        let id = &remainder[..relative_end];
        if id.contains('{') || id.is_empty() {
            return Err(RenderError::InvalidSlotReference);
        }
        if !allowed_slots.iter().any(|allowed| allowed == id) {
            return Err(RenderError::UnknownSlot(id.to_string()));
        }
        let Some(slot) = fact_sheet.slot(id) else {
            return Err(RenderError::UnknownSlot(id.to_string()));
        };
        let display = slot.display();
        replaced.push_str(&display);
        without_slots.push('\u{e000}');
        rest = &remainder[relative_end + 1..];
    }
    replaced.push_str(rest);
    without_slots.push_str(rest);
    if rest.contains('}') {
        return Err(RenderError::InvalidSlotReference);
    }
    Ok(ReplacedText {
        replaced,
        without_slots,
    })
}

fn all_slot_ids(fact_sheet: &FactSheet) -> Vec<String> {
    fact_sheet
        .slots
        .iter()
        .map(FactSlot::id)
        .map(str::to_string)
        .collect()
}

fn reject_numbers(text: &str) -> Result<(), RenderError> {
    if text
        .chars()
        .any(|character| character.is_ascii_digit() || ('０'..='９').contains(&character))
    {
        return Err(RenderError::NumericLiteral);
    }
    Ok(())
}

fn reject_number_words(text: &str) -> Result<(), RenderError> {
    let lowered = text.to_lowercase();
    let token_contains_number = lowered
        .split(|character: char| !character.is_alphanumeric())
        .any(|token| ENGLISH_NUMBER_WORDS.contains(&token));
    if token_contains_number
        || lowered
            .chars()
            .any(|character| CHINESE_NUMBER_CHARACTERS.contains(&character))
    {
        return Err(RenderError::NumericWord);
    }
    Ok(())
}

fn reject_unsupported_entities(
    text: &str,
    entity_evidence: &BTreeSet<String>,
    known_entity_names: &BTreeSet<String>,
) -> Result<(), RenderError> {
    for name in known_entity_names {
        if contains_entity_phrase(text, name) && !entity_evidence.contains(name) {
            return Err(RenderError::UnsupportedEntity(name.clone()));
        }
    }
    Ok(())
}

pub(crate) fn contains_entity_phrase(text: &str, name: &str) -> bool {
    let lowered_text = text.to_lowercase();
    let lowered_name = name.to_lowercase();
    if lowered_text.len() != text.len() || lowered_name.len() != name.len() {
        return text.contains(name);
    }
    let mut search_start = 0;
    while let Some(offset) = lowered_text[search_start..].find(&lowered_name) {
        let start = search_start + offset;
        let end = start + name.len();
        let before_supported = start == 0
            || !text[..start]
                .chars()
                .next_back()
                .is_some_and(|character| character.is_alphanumeric());
        let after_supported = end == text.len()
            || !text[end..]
                .chars()
                .next()
                .is_some_and(|character| character.is_alphanumeric());
        if before_supported && after_supported {
            return true;
        }
        search_start = start
            + lowered_text[start..]
                .chars()
                .next()
                .map(char::len_utf8)
                .unwrap_or(1);
    }
    false
}

struct QuantityCandidate {
    value: f64,
    entity: String,
    unit: Option<String>,
    source_tool: String,
}

struct ObservationCandidate {
    axis: String,
    value: f64,
    source_tool: String,
}

enum CalculatorSummary {
    Totals(Value),
    Quantity(Value),
}

fn calculator_summary(record: &ToolCallRecord, data: &Value) -> Option<CalculatorSummary> {
    match record.name.as_str() {
        "calculate_materials" => data
            .get("totals")
            .cloned()
            .map(CalculatorSummary::Totals)
            .or_else(|| Some(CalculatorSummary::Totals(data.clone()))),
        "calculate_shortage" => data
            .get("shortages")
            .cloned()
            .map(CalculatorSummary::Totals)
            .or_else(|| Some(CalculatorSummary::Totals(data.clone()))),
        "calculate_craftable_count" => data
            .get("maximum_additional_count")
            .cloned()
            .map(CalculatorSummary::Quantity),
        _ => None,
    }
}

fn collect_quantities(
    value: &Value,
    inherited_context: &[String],
    source_tool: &str,
    quantities: &mut Vec<QuantityCandidate>,
    entity_names_by_id: &BTreeMap<String, String>,
) {
    match value {
        Value::Object(fields) => {
            let local_context = fields
                .iter()
                .filter_map(|(key, field)| {
                    if matches!(
                        key.as_str(),
                        "item_name" | "target_name" | "query" | "parent_a" | "parent_b"
                    ) {
                        field.as_str().map(str::to_string)
                    } else if matches!(key.as_str(), "item_id" | "output_item_id") {
                        field.as_str().map(|id| {
                            entity_names_by_id
                                .get(id)
                                .cloned()
                                .unwrap_or_else(|| id.to_string())
                        })
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
            let unit = fields
                .iter()
                .find(|(key, field)| key.as_str() == "unit" && field.as_str().is_some())
                .and_then(|(_, field)| field.as_str());
            for (key, field) in fields {
                if let Some(number) = field.as_f64() {
                    if !matches!(key.as_str(), "confidence" | "latitude" | "longitude") {
                        for entity in context {
                            quantities.push(QuantityCandidate {
                                value: number,
                                entity: entity.clone(),
                                unit: unit.map(str::to_string),
                                source_tool: source_tool.to_string(),
                            });
                        }
                    }
                }
                collect_quantities(field, context, source_tool, quantities, entity_names_by_id);
            }
        }
        Value::Array(values) => {
            for value in values {
                collect_quantities(
                    value,
                    inherited_context,
                    source_tool,
                    quantities,
                    entity_names_by_id,
                );
            }
        }
        _ => {}
    }
}

fn collect_observations(
    value: &Value,
    source_tool: &str,
    observations: &mut Vec<ObservationCandidate>,
) {
    if let Value::Object(fields) = value {
        for (key, field) in fields {
            if matches!(key.as_str(), "x" | "y" | "z") {
                if let Some(number) = field.as_f64() {
                    observations.push(ObservationCandidate {
                        axis: key.clone(),
                        value: number,
                        source_tool: source_tool.to_string(),
                    });
                }
            }
            collect_observations(field, source_tool, observations);
        }
    } else if let Value::Array(values) = value {
        for value in values {
            collect_observations(value, source_tool, observations);
        }
    }
}

fn collect_entity_names(
    value: &Value,
    known_entity_names: &BTreeSet<String>,
    entity_names_by_id: &BTreeMap<String, String>,
    entity_names: &mut BTreeSet<String>,
) {
    let text = value.to_string();
    for name in known_entity_names {
        if contains_entity_phrase(&text, name) {
            entity_names.insert(name.clone());
        }
    }
    for (id, name) in entity_names_by_id {
        if text.contains(id.as_str()) {
            entity_names.insert(name.clone());
        }
    }
}

fn is_observation_tool(name: &str) -> bool {
    matches!(name, "get_player_status" | "get_active_pal_status")
}

fn format_number(value: f64) -> String {
    if value == value.trunc() && value.is_finite() {
        format!("{}", value as i64)
    } else {
        value.to_string()
    }
}

pub(crate) fn truncate_characters(text: &str, max_characters: usize) -> String {
    if text.chars().count() <= max_characters {
        return text.to_string();
    }
    text.chars().take(max_characters).collect()
}

#[cfg(test)]
mod tests {
    use super::{contains_entity_phrase, is_internal_uncertainty};

    #[test]
    fn chinese_phrase_scan_never_slices_inside_a_code_point() {
        assert!(contains_entity_phrase("疾旋鼬很可爱，疾旋鼬。", "疾旋鼬"));
        assert!(!contains_entity_phrase("小疾旋鼬", "疾旋鼬"));
    }

    #[test]
    fn ascii_phrase_boundaries_keep_previous_behavior() {
        assert!(contains_entity_phrase(
            "The Wooden Club recipe",
            "Wooden Club"
        ));
        assert!(contains_entity_phrase("wooden club", "Wooden Club"));
        assert!(!contains_entity_phrase("XWooden Club", "Wooden Club"));
    }

    #[test]
    fn internal_uncertainty_is_hidden_from_player_text() {
        assert!(is_internal_uncertainty(
            "model draft invalid: numeric word in model draft"
        ));
        assert!(is_internal_uncertainty("Version {v1} facts shown"));
        assert!(is_internal_uncertainty(
            "no deterministic tool evidence was used"
        ));
        assert!(!is_internal_uncertainty(
            "conflicting values remain for RECIPE_WOODEN_CLUB"
        ));
    }
}
