use guide_maintenance::{version_check, KnowledgeAudit, VersionReport};
use knowledge_index::KnowledgeIndex;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "usage: guide-maintenance [--data DIR] [--game-version VERSION] COMMAND...\n\
commands:\n\
  version-check          Report version compatibility across all dimensions\n\
  audit-sources          List all registered sources with fact counts\n\
  audit-conflicts        List all conflicts with resolution status\n\
  audit-stale            List records whose game version does not match\n\
  validate-batch FILE    Validate JSONL records without persisting";

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
        Ok((answer, code)) => (answer, code),
        Err(message) => (error_value(message), 2),
    }
}

fn run_result(arguments: Vec<String>) -> Result<(Value, u8), String> {
    let (data_directory, configured_game_version, mut command) = parse_global_arguments(arguments)?;
    if command.is_empty() {
        return Err(USAGE.to_string());
    }

    let operation = command.remove(0);
    match operation.as_str() {
        "version-check" => {
            let store = game_knowledge::KnowledgeStore::load_directory(&data_directory).map_err(
                |errors| {
                    format!("cannot load reviewed knowledge from {data_directory:?}: {errors:?}")
                },
            )?;

            let index = KnowledgeIndex::from_store(
                &store,
                configured_game_version.as_deref().map(str::to_string),
            )
            .map_err(|e| format!("cannot build index: {e}"))?;

            let report: VersionReport = version_check(
                &store,
                configured_game_version.as_deref(),
                Some(&index.version().knowledge_version),
            );

            let code = if report.has_warnings() { 1 } else { 0 };
            Ok((serde_json::to_value(&report).unwrap(), code))
        }
        "audit-sources" => {
            let store = game_knowledge::KnowledgeStore::load_directory(&data_directory).map_err(
                |errors| {
                    format!("cannot load reviewed knowledge from {data_directory:?}: {errors:?}")
                },
            )?;

            let summaries = KnowledgeAudit::audit_sources(&store);
            Ok((json!({ "sources": summaries, "total": summaries.len() }), 0))
        }
        "audit-conflicts" => {
            let store = game_knowledge::KnowledgeStore::load_directory(&data_directory).map_err(
                |errors| {
                    format!("cannot load reviewed knowledge from {data_directory:?}: {errors:?}")
                },
            )?;

            let summaries = KnowledgeAudit::audit_conflicts(&store);
            let unresolved = summaries
                .iter()
                .filter(|c| c.resolution == "unresolved")
                .count();
            Ok((
                json!({
                    "conflicts": summaries,
                    "total": summaries.len(),
                    "unresolved": unresolved,
                    "resolved": summaries.len() - unresolved
                }),
                0,
            ))
        }
        "audit-stale" => {
            let configured = configured_game_version
                .as_deref()
                .ok_or("--game-version is required for audit-stale")?;
            let store = game_knowledge::KnowledgeStore::load_directory(&data_directory).map_err(
                |errors| {
                    format!("cannot load reviewed knowledge from {data_directory:?}: {errors:?}")
                },
            )?;

            let stale = KnowledgeAudit::audit_stale(&store, configured);
            let code = if stale.is_empty() { 0 } else { 1 };
            Ok((
                json!({ "stale_records": stale, "total": stale.len() }),
                code,
            ))
        }
        "validate-batch" => {
            let file_path = command
                .first()
                .ok_or("validate-batch requires a file path")?;
            let content = std::fs::read_to_string(file_path)
                .map_err(|e| format!("cannot read {file_path}: {e}"))?;
            let mut lines = read_reviewed_jsonl(&data_directory)?;
            lines.extend(content.lines().map(str::to_string));
            let line_references = lines.iter().map(String::as_str).collect::<Vec<_>>();
            let validation = KnowledgeAudit::validate_batch(&line_references);
            let code = if validation.valid { 0 } else { 1 };
            Ok((serde_json::to_value(&validation).unwrap(), code))
        }
        _ => Err(USAGE.to_string()),
    }
}

fn read_reviewed_jsonl(data_directory: &PathBuf) -> Result<Vec<String>, String> {
    let mut paths = std::fs::read_dir(data_directory)
        .map_err(|e| format!("cannot read reviewed context {data_directory:?}: {e}"))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("jsonl"))
        .collect::<Vec<_>>();
    paths.sort();
    let mut lines = Vec::new();
    for path in paths {
        let content = std::fs::read_to_string(&path)
            .map_err(|e| format!("cannot read reviewed context {path:?}: {e}"))?;
        lines.extend(content.lines().map(str::to_string));
    }
    Ok(lines)
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

fn error_value(message: String) -> Value {
    json!({ "error": message })
}
