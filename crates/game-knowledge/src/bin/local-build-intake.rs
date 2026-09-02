use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use game_knowledge::{
    candidate_output_is_safe, generate_candidates, IntakeBatch, LocalBuildTables, LocalizationIndex,
};

struct Arguments {
    root: PathBuf,
    batch: IntakeBatch,
    output: PathBuf,
}

fn main() -> ExitCode {
    match run() {
        Ok(summary) => {
            println!("{summary}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<String, Box<dyn std::error::Error>> {
    let arguments = parse_arguments(std::env::args().skip(1))?;
    if !candidate_output_is_safe(&arguments.output) {
        return Err("candidate output must be under .local/research/local-build/candidates".into());
    }

    let tables = LocalBuildTables::load(&arguments.root)?;
    let localization = LocalizationIndex::load(&arguments.root)?;
    let candidates = generate_candidates(&tables, &localization, arguments.batch)?;
    if let Some(parent) = arguments.output.parent() {
        fs::create_dir_all(parent)?;
    }
    let records = candidates
        .records
        .iter()
        .map(serde_json::to_string)
        .collect::<Result<Vec<_>, _>>()?;
    fs::write(&arguments.output, records.join("\n") + "\n")?;

    let report_path = report_path(&arguments.output);
    fs::write(&report_path, serde_json::to_vec_pretty(&candidates.report)?)?;

    let mut summary = BTreeMap::new();
    summary.insert(
        "candidate_records".to_string(),
        candidates.records.len().to_string(),
    );
    summary.insert(
        "row_outcomes".to_string(),
        candidates.report.row_outcomes.len().to_string(),
    );
    summary.insert("report".to_string(), report_path.display().to_string());
    Ok(serde_json::to_string(&summary)?)
}

fn parse_arguments(arguments: impl Iterator<Item = String>) -> Result<Arguments, String> {
    let mut root = None;
    let mut batch = None;
    let mut output = None;
    let mut values = arguments.peekable();
    while let Some(argument) = values.next() {
        match argument.as_str() {
            "--root" => root = Some(path_value(&mut values, &argument)?),
            "--batch" => batch = Some(string_value(&mut values, &argument)?),
            "--output" => output = Some(path_value(&mut values, &argument)?),
            _ => return Err(format!("unknown argument: {argument}")),
        }
    }
    Ok(Arguments {
        root: root.ok_or("--root is required")?,
        batch: parse_batch(&batch.ok_or("--batch is required")?)?,
        output: output.ok_or("--output is required")?,
    })
}

fn string_value(
    values: &mut std::iter::Peekable<impl Iterator<Item = String>>,
    flag: &str,
) -> Result<String, String> {
    values
        .next()
        .ok_or_else(|| format!("{flag} requires a value"))
}

fn path_value(
    values: &mut std::iter::Peekable<impl Iterator<Item = String>>,
    flag: &str,
) -> Result<PathBuf, String> {
    Ok(PathBuf::from(string_value(values, flag)?))
}

fn parse_batch(value: &str) -> Result<IntakeBatch, String> {
    match value {
        "all" => Ok(IntakeBatch::All),
        "items" => Ok(IntakeBatch::Items),
        "recipes" => Ok(IntakeBatch::Recipes),
        "technologies" => Ok(IntakeBatch::Technologies),
        "pals" => Ok(IntakeBatch::Pals),
        "pal-drops" => Ok(IntakeBatch::PalDrops),
        "work-suitability" => Ok(IntakeBatch::WorkSuitability),
        "localization-aliases" => Ok(IntakeBatch::LocalizationAliases),
        "relationships" => Ok(IntakeBatch::Relationships),
        _ => Err(format!("unsupported batch: {value}")),
    }
}

fn report_path(output: &Path) -> PathBuf {
    let mut path = output.as_os_str().to_os_string();
    path.push(".report.json");
    PathBuf::from(path)
}
