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
    "required": ["status", "sentences"],
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
        "uncertainty": {"type": "array", "items": {"type": "string"}}
    }
    })
}

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
        semantic: QuantitySemantic,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum QuantitySemantic {
    Material,
    Metric,
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
    player_numbers: Vec<f64>,
}

impl FactSheet {
    pub fn build(
        records: &[ToolCallRecord],
        known_entity_names: BTreeSet<String>,
        entity_names_by_id: BTreeMap<String, String>,
        entity_name_variants_by_id: &BTreeMap<String, BTreeSet<String>>,
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
                entity_name_variants_by_id,
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
                    entity_name_variants_by_id,
                    &mut entity_evidence,
                );
                if is_observation_tool(&record.name) {
                    collect_observations(data, &record.name, &mut observations);
                } else {
                    collect_tool_quantities(record, data, &mut quantities, &entity_names_by_id);
                }
            }
        }
        let mut deduped_quantities: Vec<QuantityCandidate> = Vec::with_capacity(quantities.len());
        for quantity in quantities {
            let already_seen = deduped_quantities.iter().any(|existing| {
                existing.value == quantity.value
                    && existing.entity == quantity.entity
                    && existing.unit == quantity.unit
            });
            if !already_seen {
                deduped_quantities.push(quantity);
            }
        }
        let mut quantities = deduped_quantities;
        quantities.sort_by_key(|quantity| calculator_slot_priority(&quantity.source_tool));

        let mut slots = Vec::new();
        for (index, quantity) in quantities.into_iter().enumerate() {
            slots.push(FactSlot::Quantity {
                id: format!("q{}", index + 1),
                value: quantity.value,
                entity: quantity.entity,
                unit: quantity.unit,
                source_tool: quantity.source_tool,
                semantic: quantity.semantic,
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
            player_numbers: Vec::new(),
        }
    }

    /// Numbers the player wrote in the question may be restated without inventing a fact.
    pub fn with_player_question(mut self, question: &str) -> Self {
        self.player_numbers = arabic_numbers(question)
            .into_iter()
            .chain(chinese_numbers(question))
            .collect();
        self
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
                    ..
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

fn calculator_slot_priority(source_tool: &str) -> u8 {
    if matches!(
        source_tool,
        "calculate_materials" | "calculate_shortage" | "calculate_craftable_count"
    ) {
        0
    } else {
        1
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
        description: "Submit the final answer as short natural sentences. Numbers may be written directly only when they appear in FACT_SHEET, GROUNDING, or the player's own question; exact calculator quantities may use slot references from FACT_SHEET."
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

    let allowed_slots = all_slot_ids(fact_sheet);
    let rendered_texts = texts
        .iter()
        .map(|text| replace_slots(text, fact_sheet, &allowed_slots))
        .collect::<Result<Vec<_>, _>>()?;
    let rendered_answer = rendered_texts
        .iter()
        .map(|rendered| rendered.replaced.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    validate_grounded_numbers(&rendered_answer, fact_sheet)?;
    for rendered in &rendered_texts {
        reject_unsupported_entities(
            &rendered.replaced,
            &fact_sheet.entity_evidence,
            &fact_sheet.known_entity_names,
        )?;
    }

    let answer = rendered_texts
        .iter()
        .enumerate()
        .map(|(index, rendered)| {
            if index < draft.sentences.len() {
                rendered.replaced.clone()
            } else {
                format!(
                    "{}. {}",
                    index - draft.sentences.len() + 1,
                    rendered.replaced
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
    let has_calculator_result = fact_sheet.slots.iter().any(|slot| {
        matches!(
            slot,
            FactSlot::Quantity {
                source_tool,
                ..
            } if source_tool.starts_with("calculate_")
        )
    });
    for slot in &fact_sheet.slots {
        if let FactSlot::Quantity {
            value,
            entity,
            unit,
            source_tool,
            semantic,
            ..
        } = slot
        {
            if has_calculator_result && !source_tool.starts_with("calculate_") {
                continue;
            }
            if lines.len() >= 6 {
                break;
            }
            match semantic {
                QuantitySemantic::Material => {
                    let target = unit
                        .as_deref()
                        .filter(|unit| !unit.is_empty())
                        .unwrap_or(entity);
                    lines.push(format!("还需要 {} 个 {target}。", format_number(*value)));
                }
                QuantitySemantic::Metric => {
                    if let Some(target) = entity
                        .strip_suffix(" maximum additional count")
                        .filter(|target| !target.is_empty())
                    {
                        lines.push(format!(
                            "最多还可以制作 {} 个 {target}。",
                            format_number(*value)
                        ));
                    } else if entity == "conflict record count" {
                        lines.push(format!("共有 {} 条已审核冲突记录。", format_number(*value)));
                    } else {
                        lines.push(format!("{entity}: {}.", format_number(*value)));
                    }
                }
            }
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
}

fn contains_bracket_slot_reference(text: &str) -> bool {
    let bytes = text.as_bytes();
    for (index, byte) in bytes.iter().enumerate() {
        if *byte != b'[' {
            continue;
        }
        let mut cursor = index + 1;
        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if cursor >= bytes.len() || !matches!(bytes[cursor], b'q' | b'e' | b'o' | b'v') {
            continue;
        }
        cursor += 1;
        let digits_start = cursor;
        while cursor < bytes.len() && bytes[cursor].is_ascii_digit() {
            cursor += 1;
        }
        if cursor == digits_start {
            continue;
        }
        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if cursor < bytes.len() && bytes[cursor] == b']' {
            return true;
        }
    }
    false
}

fn replace_slots(
    text: &str,
    fact_sheet: &FactSheet,
    allowed_slots: &[String],
) -> Result<ReplacedText, RenderError> {
    if contains_bracket_slot_reference(text) {
        return Err(RenderError::InvalidSlotReference);
    }
    let mut replaced = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find('{') {
        let prefix = &rest[..start];
        replaced.push_str(prefix);
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
        rest = &remainder[relative_end + 1..];
    }
    replaced.push_str(rest);
    if rest.contains('}') {
        return Err(RenderError::InvalidSlotReference);
    }
    Ok(ReplacedText { replaced })
}

fn all_slot_ids(fact_sheet: &FactSheet) -> Vec<String> {
    fact_sheet
        .slots
        .iter()
        .map(FactSlot::id)
        .map(str::to_string)
        .collect()
}

fn validate_grounded_numbers(text: &str, fact_sheet: &FactSheet) -> Result<(), RenderError> {
    let calculator_values = fact_sheet
        .slots
        .iter()
        .filter_map(|slot| match slot {
            FactSlot::Quantity {
                value, source_tool, ..
            } if source_tool.starts_with("calculate_") => Some(*value),
            _ => None,
        })
        .collect::<Vec<_>>();

    let rendered_arabic = arabic_numbers(text);
    let rendered_numbers = rendered_arabic
        .iter()
        .copied()
        .chain(chinese_numbers(text))
        .collect::<Vec<_>>();
    for required in calculator_values {
        if !contains_number(&rendered_numbers, required) {
            return Err(RenderError::NumericLiteral);
        }
    }

    let mut allowed_values = fact_sheet
        .slots
        .iter()
        .filter_map(|slot| match slot {
            FactSlot::Quantity { value, .. } | FactSlot::Observation { value, .. } => Some(*value),
            _ => None,
        })
        .collect::<Vec<_>>();
    allowed_values.extend(fact_sheet.player_numbers.iter().copied());

    // A version label may be quoted as an arabic literal, but it must never whitelist the same
    // quantity written as a word: knowledge version 1.0 must not let a draft say "one".
    let mut allowed_literals = allowed_values.clone();
    for slot in &fact_sheet.slots {
        if let FactSlot::Version { text, .. } = slot {
            allowed_literals.extend(arabic_numbers(text));
        }
    }

    for value in rendered_arabic {
        if !contains_number(&allowed_literals, value) && !contains_number(&allowed_literals, -value)
        {
            return Err(RenderError::NumericLiteral);
        }
    }
    for value in english_number_values(text) {
        if !contains_number(&allowed_values, value) {
            return Err(RenderError::NumericWord);
        }
    }
    Ok(())
}

fn contains_number(allowed: &[f64], value: f64) -> bool {
    allowed.iter().any(|allowed| (allowed - value).abs() < 1e-9)
}

fn arabic_numbers(text: &str) -> Vec<f64> {
    let mut numbers = Vec::new();
    let mut buffer = String::new();
    for character in text.chars() {
        if character.is_ascii_digit() || character == '.' {
            buffer.push(character);
        } else {
            if let Some(number) = parse_arabic_number(&buffer) {
                numbers.push(number);
            }
            buffer.clear();
        }
    }
    if let Some(number) = parse_arabic_number(&buffer) {
        numbers.push(number);
    }
    numbers
}

fn parse_arabic_number(buffer: &str) -> Option<f64> {
    let trimmed = buffer.trim_matches('.');
    if trimmed.is_empty() {
        return None;
    }
    trimmed.parse::<f64>().ok()
}

fn english_number_values(text: &str) -> Vec<f64> {
    text.to_lowercase()
        .split(|character: char| !character.is_alphanumeric())
        .filter_map(english_number_word_value)
        .collect()
}

fn english_number_word_value(token: &str) -> Option<f64> {
    match token {
        "zero" => Some(0.0),
        "one" => Some(1.0),
        "two" => Some(2.0),
        "three" => Some(3.0),
        "four" => Some(4.0),
        "five" => Some(5.0),
        "six" => Some(6.0),
        "seven" => Some(7.0),
        "eight" => Some(8.0),
        "nine" => Some(9.0),
        "ten" => Some(10.0),
        "eleven" => Some(11.0),
        "twelve" => Some(12.0),
        "thirteen" => Some(13.0),
        "fourteen" => Some(14.0),
        "fifteen" => Some(15.0),
        "sixteen" => Some(16.0),
        "seventeen" => Some(17.0),
        "eighteen" => Some(18.0),
        "nineteen" => Some(19.0),
        "twenty" => Some(20.0),
        "thirty" => Some(30.0),
        "forty" => Some(40.0),
        "fifty" => Some(50.0),
        "sixty" => Some(60.0),
        "seventy" => Some(70.0),
        "eighty" => Some(80.0),
        "ninety" => Some(90.0),
        "hundred" => Some(100.0),
        "thousand" => Some(1000.0),
        "million" => Some(1000000.0),
        _ => None,
    }
}

fn chinese_numbers(text: &str) -> Vec<f64> {
    let characters = text.chars().collect::<Vec<_>>();
    let mut numbers = Vec::new();
    let mut index = 0;
    while index < characters.len() {
        if !CHINESE_NUMBER_CHARACTERS.contains(&characters[index]) {
            index += 1;
            continue;
        }
        let start = index;
        while index < characters.len() && CHINESE_NUMBER_CHARACTERS.contains(&characters[index]) {
            index += 1;
        }
        let run = characters[start..index].iter().collect::<String>();
        if run == "一"
            && index < characters.len()
            && matches!(characters[index], '种' | '些' | '般')
        {
            continue;
        }
        if let Some(number) = parse_chinese_number(&run) {
            numbers.push(number);
        }
    }
    numbers
}

fn parse_chinese_number(run: &str) -> Option<f64> {
    let mut total = 0.0;
    let mut section = 0.0;
    let mut number = 0.0;
    for character in run.chars() {
        match character {
            '零' | '〇' => {}
            '一' | '壹' => number = 1.0,
            '二' | '贰' | '两' => number = 2.0,
            '三' | '叁' => number = 3.0,
            '四' | '肆' => number = 4.0,
            '五' | '伍' => number = 5.0,
            '六' | '陆' => number = 6.0,
            '七' | '柒' => number = 7.0,
            '八' | '捌' => number = 8.0,
            '九' | '玖' => number = 9.0,
            '十' | '拾' => {
                if number == 0.0 {
                    number = 1.0;
                }
                section += number * 10.0;
                number = 0.0;
            }
            '百' | '佰' => {
                if number == 0.0 {
                    number = 1.0;
                }
                section += number * 100.0;
                number = 0.0;
            }
            '千' | '仟' => {
                if number == 0.0 {
                    number = 1.0;
                }
                section += number * 1000.0;
                number = 0.0;
            }
            '万' | '萬' => {
                section = (section + number) * 10000.0;
                total += section;
                section = 0.0;
                number = 0.0;
            }
            '亿' | '億' => {
                section = (section + number) * 100000000.0;
                total += section;
                section = 0.0;
                number = 0.0;
            }
            _ => return None,
        }
    }
    total += section + number;
    Some(total)
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
    semantic: QuantitySemantic,
}

struct ObservationCandidate {
    axis: String,
    value: f64,
    source_tool: String,
}

fn collect_tool_quantities(
    record: &ToolCallRecord,
    data: &Value,
    quantities: &mut Vec<QuantityCandidate>,
    entity_names_by_id: &BTreeMap<String, String>,
) {
    match record.name.as_str() {
        "calculate_materials" => {
            for total in data
                .get("totals")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                push_material(
                    total.get("required_quantity"),
                    display_name(
                        total.get("item_name"),
                        total.get("item_id"),
                        entity_names_by_id,
                    ),
                    &record.name,
                    quantities,
                );
            }
        }
        "calculate_shortage" => {
            for shortage in data
                .get("shortages")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                push_material(
                    shortage.get("missing_quantity"),
                    display_name(
                        shortage.get("item_name"),
                        shortage.get("item_id"),
                        entity_names_by_id,
                    ),
                    &record.name,
                    quantities,
                );
            }
        }
        "calculate_craftable_count" => {
            let target = display_name(None, data.get("target_id"), entity_names_by_id);
            push_metric(
                data.get("maximum_additional_count"),
                format!("{target} maximum additional count"),
                &record.name,
                quantities,
            );
        }
        "get_type_effectiveness" => {
            let records = data
                .as_array()
                .map(Vec::as_slice)
                .unwrap_or(std::slice::from_ref(data));
            for record_value in records {
                push_metric(
                    record_value.get("multiplier"),
                    "multiplier".to_string(),
                    &record.name,
                    quantities,
                );
            }
        }
        "get_conflicting_records" => {
            let count = data
                .get("conflicts")
                .and_then(Value::as_array)
                .map(Vec::len)
                .unwrap_or(0) as f64;
            quantities.push(QuantityCandidate {
                value: count,
                entity: "conflict record count".to_string(),
                unit: None,
                source_tool: record.name.clone(),
                semantic: QuantitySemantic::Metric,
            });
        }
        "get_technology" => {
            push_metric(
                data.get("level"),
                "technology level".to_string(),
                &record.name,
                quantities,
            );
        }
        "get_recipe" => {
            for ingredient in data
                .get("ingredients")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                push_material(
                    ingredient.get("quantity"),
                    display_name(None, ingredient.get("item_id"), entity_names_by_id),
                    &record.name,
                    quantities,
                );
            }
        }
        "get_pal" => {
            for work in data
                .get("work_suitability")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let kind = work.get("kind").and_then(Value::as_str).unwrap_or("work");
                push_metric(
                    work.get("level"),
                    format!("{kind} work level"),
                    &record.name,
                    quantities,
                );
            }
            for drop in data
                .get("drops")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let item = display_name(None, drop.get("item_id"), entity_names_by_id);
                for (field, label) in [
                    ("probability_percent", "drop probability"),
                    ("min_quantity", "minimum drop"),
                    ("max_quantity", "maximum drop"),
                ] {
                    push_metric(
                        drop.get(field),
                        format!("{item} {label}"),
                        &record.name,
                        quantities,
                    );
                }
            }
        }
        "get_waza" => collect_waza_metrics(data, &record.name, quantities),
        "get_pal_waza_unlocks" => {
            for unlock in data.as_array().into_iter().flatten() {
                let skill = unlock
                    .pointer("/waza/names/en")
                    .and_then(Value::as_str)
                    .unwrap_or("skill");
                push_metric(
                    unlock.get("unlock_level"),
                    format!("{skill} unlock level"),
                    &record.name,
                    quantities,
                );
                if let Some(waza) = unlock.get("waza") {
                    collect_waza_metrics(waza, &record.name, quantities);
                }
            }
        }
        "locate_coordinate" => {
            for (axis, value) in [("x", "coordinate x"), ("y", "coordinate y")] {
                push_metric(
                    data.pointer(&format!("/location/{axis}")),
                    value.to_string(),
                    &record.name,
                    quantities,
                );
            }
        }
        "find_nearby_map_points" => {
            for point in data.as_array().into_iter().flatten() {
                let name = point
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("map point");
                push_metric(
                    point.get("distance_map_display"),
                    format!("{name} map-coordinate distance"),
                    &record.name,
                    quantities,
                );
            }
        }
        "plan_travel_route" => {
            push_metric(
                data.get("recommended_distance_map_display"),
                "recommended travel map-coordinate distance".to_string(),
                &record.name,
                quantities,
            );
        }
        _ => {}
    }
}

fn collect_waza_metrics(data: &Value, source_tool: &str, quantities: &mut Vec<QuantityCandidate>) {
    push_metric(
        data.get("power"),
        "power".to_string(),
        source_tool,
        quantities,
    );
    push_metric(
        data.get("cool_time"),
        "cool_time".to_string(),
        source_tool,
        quantities,
    );
    for index in 1..=2 {
        let effect_type = data
            .get(format!("effect_type{index}"))
            .and_then(Value::as_str)
            .unwrap_or("none");
        if effect_type != "none" {
            push_metric(
                data.get(format!("effect_value{index}")),
                format!("{effect_type} effect value"),
                source_tool,
                quantities,
            );
        }
    }
}

fn display_name(
    name: Option<&Value>,
    id: Option<&Value>,
    entity_names_by_id: &BTreeMap<String, String>,
) -> String {
    name.and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| id.and_then(Value::as_str).map(str::to_string))
        .map(|value| {
            entity_names_by_id
                .get(value.as_str())
                .cloned()
                .unwrap_or(value)
        })
        .unwrap_or_else(|| "unknown".to_string())
}

fn push_material(
    value: Option<&Value>,
    entity: String,
    source_tool: &str,
    quantities: &mut Vec<QuantityCandidate>,
) {
    let Some(value) = value
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
    else {
        return;
    };
    quantities.push(QuantityCandidate {
        value,
        entity,
        unit: None,
        source_tool: source_tool.to_string(),
        semantic: QuantitySemantic::Material,
    });
}

fn push_metric(
    value: Option<&Value>,
    entity: String,
    source_tool: &str,
    quantities: &mut Vec<QuantityCandidate>,
) {
    let Some(value) = value
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
    else {
        return;
    };
    quantities.push(QuantityCandidate {
        value,
        entity,
        unit: None,
        source_tool: source_tool.to_string(),
        semantic: QuantitySemantic::Metric,
    });
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
    entity_name_variants_by_id: &BTreeMap<String, BTreeSet<String>>,
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
            if let Some(variants) = entity_name_variants_by_id.get(id) {
                entity_names.extend(variants.iter().cloned());
            }
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
    use super::{
        contains_entity_phrase, is_internal_uncertainty, validate_grounded_numbers, FactSheet,
        FactSlot, QuantitySemantic, RenderError,
    };

    fn sheet_with_slots(slots: Vec<FactSlot>) -> FactSheet {
        FactSheet {
            slots,
            entity_evidence: Default::default(),
            known_entity_names: Default::default(),
            player_numbers: Vec::new(),
        }
    }

    #[test]
    fn version_label_does_not_whitelist_the_same_quantity_as_a_word() {
        let sheet = sheet_with_slots(vec![FactSlot::Version {
            id: "v1".to_string(),
            text: "1.0".to_string(),
        }]);

        assert!(validate_grounded_numbers("Knowledge version 1.0.", &sheet).is_ok());
        assert_eq!(
            validate_grounded_numbers("Knowledge version one.", &sheet),
            Err(RenderError::NumericWord)
        );
    }

    #[test]
    fn observed_quantity_still_whitelists_its_english_word() {
        let sheet = sheet_with_slots(vec![FactSlot::Quantity {
            id: "q1".to_string(),
            value: 1.0,
            entity: "Wooden Club".to_string(),
            unit: None,
            source_tool: "lookup_recipe".to_string(),
            semantic: QuantitySemantic::Material,
        }]);

        assert!(validate_grounded_numbers("Wooden Club needs one of them.", &sheet).is_ok());
    }

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
