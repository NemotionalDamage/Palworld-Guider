use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use game_knowledge::{candidate_output_is_safe, KnowledgeRecord, LocalEvidenceMetadata};

struct Arguments {
    candidate: PathBuf,
    canonical: PathBuf,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments = parse_arguments(std::env::args().skip(1))?;
    if !candidate_output_is_safe(&arguments.candidate) {
        return Err("candidate input must be under .local/research/local-build/candidates".into());
    }
    if arguments.canonical != Path::new("data/reviewed") {
        return Err("promotion target must be data/reviewed".into());
    }
    let content = fs::read_to_string(&arguments.candidate)?;
    let mut maps = Vec::new();
    let mut regions = Vec::new();
    let mut points = Vec::new();
    let mut zones = Vec::new();
    for line in content.lines() {
        match serde_json::from_str::<KnowledgeRecord>(line)? {
            KnowledgeRecord::MapDefinition(record) => maps.push(record),
            KnowledgeRecord::MapRegion(record) => regions.push(record),
            KnowledgeRecord::MapPoint(record) => points.push(record),
            KnowledgeRecord::PalHabitatZone(record) => zones.push(record),
            _ => return Err("map promotion input contains an unsupported record".into()),
        }
    }
    let zone_ids_by_pal = zones.iter().fold(
        BTreeMap::<String, Vec<String>>::new(),
        |mut accumulator, zone| {
            accumulator
                .entry(zone.pal_id.clone())
                .or_default()
                .push(zone.id.clone());
            accumulator
        },
    );
    append_records(
        &arguments.canonical.join("maps.jsonl"),
        &maps,
        KnowledgeRecord::MapDefinition,
    )?;
    append_records(
        &arguments.canonical.join("map_regions.jsonl"),
        &regions,
        KnowledgeRecord::MapRegion,
    )?;
    append_records(
        &arguments.canonical.join("map_points.jsonl"),
        &points,
        KnowledgeRecord::MapPoint,
    )?;
    append_records(
        &arguments.canonical.join("pal_habitat_zones.jsonl"),
        &zones,
        KnowledgeRecord::PalHabitatZone,
    )?;
    update_pal_habitats(&arguments.canonical.join("pals.jsonl"), &zone_ids_by_pal)?;
    update_pal_habitats(&arguments.canonical.join("facts.jsonl"), &zone_ids_by_pal)?;
    println!(
        "{}",
        serde_json::json!({
            "maps": maps.len(),
            "regions": regions.len(),
            "points": points.len(),
            "habitat_zones": zones.len(),
            "pals_with_habitats": zone_ids_by_pal.len(),
        })
    );
    Ok(())
}

fn append_records<T: serde::Serialize + Clone>(
    path: &Path,
    records: &[T],
    wrapper: fn(T) -> KnowledgeRecord,
) -> Result<(), Box<dyn std::error::Error>> {
    if records.is_empty() {
        return Ok(());
    }
    let serialized = records
        .iter()
        .cloned()
        .map(wrapper)
        .map(|record| serde_json::to_string(&record))
        .collect::<Result<Vec<_>, _>>()?;
    fs::write(path, serialized.join("\n") + "\n")?;
    Ok(())
}

fn update_pal_habitats(
    path: &Path,
    zone_ids_by_pal: &BTreeMap<String, Vec<String>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let content = fs::read_to_string(path)?;
    let mut updated = 0;
    let records = content
        .lines()
        .map(|line| {
            let mut record: KnowledgeRecord = match serde_json::from_str(line) {
                Ok(record) => record,
                Err(_) => return Ok(line.to_string()),
            };
            let KnowledgeRecord::Pal(pal) = &mut record else {
                return Ok(line.to_string());
            };
            let Some(zone_ids) = zone_ids_by_pal.get(&pal.id) else {
                return Ok(line.to_string());
            };
            pal.habitat_ids = zone_ids.clone();
            if let Some(evidence) = pal.local_evidence.as_mut() {
                evidence
                    .unresolved_fields
                    .retain(|field| field != "habitat_ids");
                evidence.transformation_notes = evidence.transformation_notes.replace(
                    "habitats remain unresolved.; habitats resolved",
                    "habitats resolved",
                );
                if !evidence
                    .transformation_notes
                    .contains("habitats resolved from target-build spawner volumes")
                {
                    evidence
                        .transformation_notes
                        .push_str("; habitats resolved from target-build spawner volumes.");
                }
            } else {
                pal.local_evidence = Some(LocalEvidenceMetadata {
                    source_table: "DT_PalSpawnerPlacement + DT_PalWildSpawner".to_string(),
                    localization_status: game_knowledge::LocalizationStatus::NotApplicable,
                    unresolved_fields: Vec::new(),
                    reviewed_empty_fields: Vec::new(),
                    transformation_notes: "habitats resolved from target-build spawner volumes."
                        .to_string(),
                });
            }
            let map_source = "SRC-LOCAL-BUILD-MAP-24575825-20260906".to_string();
            if !pal
                .provenance
                .corroborating_source_ids
                .contains(&map_source)
            {
                pal.provenance.corroborating_source_ids.push(map_source);
            }
            if pal.provenance.change_risk.as_deref()
                == Some("habitats remain unresolved; patches can change Pal data.")
            {
                pal.provenance.change_risk = Some("patches can change Pal data.".to_string());
            }
            updated += 1;
            serde_json::to_string(&record).map_err(Box::<dyn std::error::Error>::from)
        })
        .collect::<Result<Vec<_>, _>>()?;
    fs::write(path, records.join("\n") + "\n")?;
    println!("pals_updated={updated}");
    Ok(())
}

fn parse_arguments(arguments: impl Iterator<Item = String>) -> Result<Arguments, String> {
    let mut candidate = None;
    let mut canonical = None;
    let mut values = arguments.peekable();
    while let Some(argument) = values.next() {
        match argument.as_str() {
            "--candidate" => {
                candidate = Some(PathBuf::from(next_value(&mut values, &argument)?));
            }
            "--canonical" => {
                canonical = Some(PathBuf::from(next_value(&mut values, &argument)?));
            }
            _ => return Err(format!("unknown argument: {argument}")),
        }
    }
    Ok(Arguments {
        candidate: candidate.ok_or("--candidate is required")?,
        canonical: canonical.ok_or("--canonical is required")?,
    })
}

fn next_value(
    values: &mut std::iter::Peekable<impl Iterator<Item = String>>,
    flag: &str,
) -> Result<String, String> {
    values
        .next()
        .ok_or_else(|| format!("{flag} requires a value"))
}
