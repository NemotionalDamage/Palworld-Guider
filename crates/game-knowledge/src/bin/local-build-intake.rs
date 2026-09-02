use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use game_knowledge::{
    audit_canonical_backfill, candidate_output_is_safe, canonical_backfill_output_is_safe,
    generate_candidates, IntakeBatch, KnowledgeStore, LocalBuildTables, LocalizationIndex,
};

struct Arguments {
    root: PathBuf,
    batch: Batch,
    output: PathBuf,
    canonical: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy)]
enum Batch {
    CanonicalBackfill,
    Intake(IntakeBatch),
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
    let tables = LocalBuildTables::load(&arguments.root)?;
    let localization = LocalizationIndex::load(&arguments.root)?;
    let intake_batch = match arguments.batch {
        Batch::CanonicalBackfill => {
            return run_canonical_backfill(&arguments, &tables, &localization)
        }
        Batch::Intake(intake_batch) => intake_batch,
    };
    if !candidate_output_is_safe(&arguments.output) {
        return Err("candidate output must be under .local/research/local-build/candidates".into());
    }
    let candidates = generate_candidates(&tables, &localization, intake_batch)?;
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

fn run_canonical_backfill(
    arguments: &Arguments,
    tables: &LocalBuildTables,
    localization: &LocalizationIndex,
) -> Result<String, Box<dyn std::error::Error>> {
    if !canonical_backfill_output_is_safe(&arguments.output) {
        return Err(
            "backfill output must be .local/research/local-build/reports/canonical-backfill.json"
                .into(),
        );
    }
    let canonical = arguments
        .canonical
        .as_deref()
        .ok_or("--canonical is required for canonical backfill")?;
    let store = KnowledgeStore::load_directory(canonical).map_err(|errors| {
        errors
            .into_iter()
            .map(|error| error.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    let report = audit_canonical_backfill(tables, localization, &store);
    if let Some(parent) = arguments.output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&arguments.output, serde_json::to_vec_pretty(&report)?)?;

    let mut summary = BTreeMap::new();
    summary.insert(
        "total_records".to_string(),
        report.summary.total_records.to_string(),
    );
    summary.insert(
        "audited_facts".to_string(),
        report.summary.total_audited_facts.to_string(),
    );
    summary.insert(
        "unclassified_facts".to_string(),
        report.summary.unclassified_facts.to_string(),
    );
    summary.insert("report".to_string(), arguments.output.display().to_string());
    Ok(serde_json::to_string(&summary)?)
}

fn parse_arguments(arguments: impl Iterator<Item = String>) -> Result<Arguments, String> {
    let mut root = None;
    let mut batch = None;
    let mut output = None;
    let mut canonical = None;
    let mut values = arguments.peekable();
    while let Some(argument) = values.next() {
        match argument.as_str() {
            "--root" => root = Some(path_value(&mut values, &argument)?),
            "--batch" => batch = Some(string_value(&mut values, &argument)?),
            "--output" => output = Some(path_value(&mut values, &argument)?),
            "--canonical" => canonical = Some(path_value(&mut values, &argument)?),
            _ => return Err(format!("unknown argument: {argument}")),
        }
    }
    Ok(Arguments {
        root: root.ok_or("--root is required")?,
        batch: parse_batch(&batch.ok_or("--batch is required")?)?,
        output: output.ok_or("--output is required")?,
        canonical,
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

fn parse_batch(value: &str) -> Result<Batch, String> {
    match value {
        "canonical-backfill" => Ok(Batch::CanonicalBackfill),
        "all" => Ok(Batch::Intake(IntakeBatch::All)),
        "items" => Ok(Batch::Intake(IntakeBatch::Items)),
        "recipes" => Ok(Batch::Intake(IntakeBatch::Recipes)),
        "technologies" => Ok(Batch::Intake(IntakeBatch::Technologies)),
        "pals" => Ok(Batch::Intake(IntakeBatch::Pals)),
        "pal-drops" => Ok(Batch::Intake(IntakeBatch::PalDrops)),
        "work-suitability" => Ok(Batch::Intake(IntakeBatch::WorkSuitability)),
        "localization-aliases" => Ok(Batch::Intake(IntakeBatch::LocalizationAliases)),
        "relationships" => Ok(Batch::Intake(IntakeBatch::Relationships)),
        _ => Err(format!("unsupported batch: {value}")),
    }
}

fn report_path(output: &Path) -> PathBuf {
    let mut path = output.as_os_str().to_os_string();
    path.push(".report.json");
    PathBuf::from(path)
}
