use guide_core::{AnswerStatus, GuideAnswer, GuideEngine, InventoryEntry};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "usage: guide-core [--data DIR] [--game-version VERSION] COMMAND...\n\
commands:\n\
  lookup item|pal|technology|recipe NAME...\n\
  recipe NAME...\n\
  materials QUANTITY NAME...\n\
  shortage [--inventory NAME=QTY,...] QUANTITY NAME...\n\
  craftable [--inventory NAME=QTY,...] NAME...\n\
  breeding PARENT_A PARENT_B\n\
  chain MAX_DEPTH START TARGET";

fn main() -> ExitCode {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let (answer, code) = run(arguments);
    println!(
        "{}",
        serde_json::to_string_pretty(&answer).expect("answer value is serializable")
    );
    ExitCode::from(code)
}

fn run(arguments: Vec<String>) -> (Value, u8) {
    match run_result(arguments) {
        Ok((answer, status)) => {
            let code = match status {
                AnswerStatus::Ok => 0,
                AnswerStatus::Unknown | AnswerStatus::Ambiguous => 1,
                AnswerStatus::Error => 2,
            };
            (answer, code)
        }
        Err(message) => (error_value(message), 2),
    }
}

fn run_result(arguments: Vec<String>) -> Result<(Value, AnswerStatus), String> {
    let (data_directory, configured_game_version, mut command) = parse_global_arguments(arguments)?;
    if command.is_empty() {
        return Err(USAGE.to_string());
    }
    let engine = GuideEngine::load_directory(&data_directory, configured_game_version).map_err(
        |errors| format!("cannot load reviewed knowledge from {data_directory:?}: {errors}"),
    )?;

    let operation = command.remove(0);
    match operation.as_str() {
        "lookup" => {
            if command.len() < 2 {
                return Err(USAGE.to_string());
            }
            let kind = command.remove(0);
            let query = command.join(" ");
            match kind.as_str() {
                "item" => Ok(answer_value(engine.lookup_item(&query))),
                "pal" => Ok(answer_value(engine.lookup_pal(&query))),
                "technology" => Ok(answer_value(engine.lookup_technology(&query))),
                "recipe" => Ok(answer_value(engine.lookup_recipe(&query))),
                _ => Err(USAGE.to_string()),
            }
        }
        "recipe" => {
            if command.is_empty() {
                return Err(USAGE.to_string());
            }
            Ok(answer_value(engine.lookup_recipe(&command.join(" "))))
        }
        "materials" => {
            if command.len() < 2 {
                return Err(USAGE.to_string());
            }
            let quantity = parse_quantity(&command[0])?;
            let query = command[1..].join(" ");
            Ok(answer_value(engine.calculate_materials(&query, quantity)))
        }
        "shortage" => {
            let inventory_entries = extract_inventory(&mut command)?;
            if command.len() < 2 {
                return Err(USAGE.to_string());
            }
            let quantity = parse_quantity(&command[0])?;
            let query = command[1..].join(" ");
            Ok(answer_value(engine.calculate_shortage(
                &query,
                quantity,
                &inventory_entries,
            )))
        }
        "craftable" => {
            let inventory_entries = extract_inventory(&mut command)?;
            if command.is_empty() {
                return Err(USAGE.to_string());
            }
            let query = command.join(" ");
            Ok(answer_value(
                engine.calculate_craftable_count(&query, &inventory_entries),
            ))
        }
        "breeding" => {
            if command.len() != 2 {
                return Err(USAGE.to_string());
            }
            Ok(answer_value(
                engine.calculate_breeding_result(&command[0], &command[1]),
            ))
        }
        "chain" => {
            if command.len() != 3 {
                return Err(USAGE.to_string());
            }
            let maximum_depth = command[0]
                .parse::<usize>()
                .map_err(|_| format!("invalid maximum depth: {}", command[0]))?;
            Ok(answer_value(engine.calculate_breeding_chain(
                &command[1],
                &command[2],
                maximum_depth,
            )))
        }
        _ => Err(USAGE.to_string()),
    }
}

fn parse_global_arguments(
    arguments: Vec<String>,
) -> Result<(PathBuf, Option<String>, Vec<String>), String> {
    let mut data_directory = PathBuf::from("data/reviewed");
    let mut configured_game_version = None;
    let mut command = Vec::new();
    let mut index = 0;

    while index < arguments.len() {
        match arguments[index].as_str() {
            "--data" => {
                index += 1;
                let value = arguments.get(index).ok_or("--data requires a path")?;
                data_directory = PathBuf::from(value);
            }
            "--game-version" => {
                index += 1;
                let value = arguments
                    .get(index)
                    .ok_or("--game-version requires a value")?;
                configured_game_version = Some(value.clone());
            }
            _ => command.push(arguments[index].clone()),
        }
        index += 1;
    }
    Ok((data_directory, configured_game_version, command))
}

fn extract_inventory(command: &mut Vec<String>) -> Result<Vec<InventoryEntry>, String> {
    let Some(flag_position) = command
        .iter()
        .position(|argument| argument == "--inventory")
    else {
        return Ok(Vec::new());
    };
    let value_position = flag_position + 1;
    let value = command
        .get(value_position)
        .ok_or_else(|| "--inventory requires NAME=QTY,...".to_string())?
        .clone();
    command.drain(flag_position..=value_position);

    value
        .split(',')
        .filter(|entry| !entry.is_empty())
        .map(|entry| {
            let (name, quantity) = entry
                .split_once('=')
                .ok_or_else(|| format!("invalid inventory entry: {entry}"))?;
            let quantity = parse_quantity(quantity.trim())?;
            Ok(InventoryEntry::new(name.trim(), quantity))
        })
        .collect()
}

fn parse_quantity(value: &str) -> Result<u32, String> {
    value
        .parse::<u32>()
        .map_err(|_| format!("invalid quantity: {value}"))
}

fn answer_value<T: serde::Serialize>(answer: GuideAnswer<T>) -> (Value, AnswerStatus) {
    let status = answer.status;
    (
        serde_json::to_value(answer).expect("guide answer is serializable"),
        status,
    )
}

fn error_value(message: impl Into<String>) -> Value {
    json!({
        "status": "error",
        "data": null,
        "provenance": [],
        "version": {
            "knowledge_version": "unknown",
            "configured_game_version": null,
            "matches": false
        },
        "uncertainty": [],
        "errors": [message.into()]
    })
}
