use std::fs;
use std::path::{Path, PathBuf};

use game_knowledge::{
    candidate_output_is_safe, generate_map_candidates, KnowledgeStore, MapIntakeBatch,
};

struct Arguments {
    root: PathBuf,
    canonical: PathBuf,
    batch: MapIntakeBatch,
    output: PathBuf,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments = parse_arguments(std::env::args().skip(1))?;
    if !candidate_output_is_safe(&arguments.output) {
        return Err("candidate output must be under .local/research/local-build/candidates".into());
    }
    let store = KnowledgeStore::load_directory(&arguments.canonical).map_err(|errors| {
        errors
            .into_iter()
            .map(|error| error.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    let candidates = generate_map_candidates(&arguments.root, &store, arguments.batch)?;
    if let Some(parent) = arguments.output.parent() {
        fs::create_dir_all(parent)?;
    }
    let records = candidates
        .records
        .iter()
        .map(serde_json::to_string)
        .collect::<Result<Vec<_>, _>>()?;
    fs::write(&arguments.output, records.join("\n") + "\n")?;

    let mut report_path = arguments.output.clone().into_os_string();
    report_path.push(".report.json");
    let report_path = PathBuf::from(report_path);
    fs::write(&report_path, serde_json::to_vec_pretty(&candidates.report)?)?;
    println!("{}", serde_json::to_string(&candidates.report)?);
    Ok(())
}

fn parse_arguments(arguments: impl Iterator<Item = String>) -> Result<Arguments, String> {
    let mut root = None;
    let mut canonical = None;
    let mut batch = None;
    let mut output = None;
    let mut values = arguments.peekable();
    while let Some(argument) = values.next() {
        match argument.as_str() {
            "--root" => root = Some(path_value(&mut values, &argument)?),
            "--canonical" => canonical = Some(path_value(&mut values, &argument)?),
            "--batch" => batch = Some(string_value(&mut values, &argument)?),
            "--output" => output = Some(path_value(&mut values, &argument)?),
            _ => return Err(format!("unknown argument: {argument}")),
        }
    }
    Ok(Arguments {
        root: root.ok_or("--root is required")?,
        canonical: canonical.ok_or("--canonical is required")?,
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

fn parse_batch(value: &str) -> Result<MapIntakeBatch, String> {
    match value {
        "all" => Ok(MapIntakeBatch::All),
        "map" => Ok(MapIntakeBatch::MapFoundation),
        "pal-habitats" => Ok(MapIntakeBatch::PalHabitats),
        _ => Err(format!("unsupported map batch: {value}")),
    }
}

fn _path_is_relative(path: &Path) -> bool {
    path.is_relative()
}
