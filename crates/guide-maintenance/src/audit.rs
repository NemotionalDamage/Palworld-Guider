//! Knowledge auditing: sources, conflicts, stale records, and batch validation.

use std::collections::BTreeMap;

use game_knowledge::{
    Confidence, ConflictResolution, KnowledgeRecord, KnowledgeStore, ReviewStatus, ValidationError,
};
use serde::{Deserialize, Serialize};

/// Summary of a registered source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceSummary {
    pub id: String,
    pub title: String,
    pub applicable_game_version: String,
    pub retrieved_on: String,
    pub review_status: String,
    pub confidence: String,
    pub fact_count: usize,
}

/// Summary of a conflict record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConflictSummary {
    pub id: String,
    pub subject_id: String,
    pub field: String,
    pub values: Vec<String>,
    pub source_ids: Vec<String>,
    pub resolution: String,
}

/// A record whose game version does not match the configured version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StaleRecord {
    pub record_id: String,
    pub record_type: String,
    pub applicable_game_version: String,
}

/// Result of validating a batch of JSONL records.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchValidation {
    pub valid: bool,
    pub total_records: usize,
    pub errors: Vec<String>,
}

/// Knowledge auditor that reports on the current state of the store.
pub struct KnowledgeAudit;

fn review_status_value(status: &ReviewStatus) -> &'static str {
    match status {
        ReviewStatus::Candidate => "candidate",
        ReviewStatus::Reviewed => "reviewed",
        ReviewStatus::Retired => "retired",
    }
}

fn confidence_value(confidence: &Confidence) -> &'static str {
    match confidence {
        Confidence::VerifiedTarget => "verified_target",
        Confidence::Official => "official",
        Confidence::ReviewedSecondary => "reviewed_secondary",
        Confidence::Community => "community",
        Confidence::Conflicted => "conflicted",
        Confidence::Unknown => "unknown",
    }
}

impl KnowledgeAudit {
    /// Audit all registered sources and count facts per source.
    pub fn audit_sources(store: &KnowledgeStore) -> Vec<SourceSummary> {
        let mut fact_counts: BTreeMap<String, usize> = BTreeMap::new();

        for record in store.items() {
            *fact_counts
                .entry(record.provenance.source_id.clone())
                .or_insert(0) += 1;
        }
        for record in store.pals() {
            *fact_counts
                .entry(record.provenance.source_id.clone())
                .or_insert(0) += 1;
        }
        for record in store.technologies() {
            *fact_counts
                .entry(record.provenance.source_id.clone())
                .or_insert(0) += 1;
        }
        for record in store.recipes() {
            *fact_counts
                .entry(record.provenance.source_id.clone())
                .or_insert(0) += 1;
        }
        for record in store.habitats() {
            *fact_counts
                .entry(record.provenance.source_id.clone())
                .or_insert(0) += 1;
        }
        for record in store.breeding_rules() {
            *fact_counts
                .entry(record.provenance.source_id.clone())
                .or_insert(0) += 1;
        }
        for record in store.aliases() {
            *fact_counts
                .entry(record.provenance.source_id.clone())
                .or_insert(0) += 1;
        }
        for record in store.progression_relationships() {
            *fact_counts
                .entry(record.provenance.source_id.clone())
                .or_insert(0) += 1;
        }
        for record in store.conflicts() {
            *fact_counts
                .entry(record.provenance.source_id.clone())
                .or_insert(0) += 1;
        }

        store
            .sources()
            .map(|source| SourceSummary {
                id: source.id.clone(),
                title: source.title.clone(),
                applicable_game_version: source.applicable_game_version.clone(),
                retrieved_on: source.retrieved_on.clone(),
                review_status: review_status_value(&source.review_status).to_string(),
                confidence: confidence_value(&source.confidence).to_string(),
                fact_count: *fact_counts.get(&source.id).unwrap_or(&0),
            })
            .collect()
    }

    /// Audit all conflict records and their resolution status.
    pub fn audit_conflicts(store: &KnowledgeStore) -> Vec<ConflictSummary> {
        store
            .conflicts()
            .iter()
            .map(|conflict| ConflictSummary {
                id: conflict.id.clone(),
                subject_id: conflict.subject_id.clone(),
                field: conflict.field.clone(),
                values: conflict.values.clone(),
                source_ids: conflict.source_ids.clone(),
                resolution: match conflict.resolution {
                    ConflictResolution::Unresolved => "unresolved".to_string(),
                    ConflictResolution::Resolved => "resolved".to_string(),
                },
            })
            .collect()
    }

    /// Find records whose `applicable_game_version` does not match the configured version.
    pub fn audit_stale(store: &KnowledgeStore, configured_game_version: &str) -> Vec<StaleRecord> {
        let mut stale = Vec::new();

        for record in store.items() {
            if record.provenance.applicable_game_version != configured_game_version {
                stale.push(StaleRecord {
                    record_id: record.id.clone(),
                    record_type: "item".to_string(),
                    applicable_game_version: record.provenance.applicable_game_version.clone(),
                });
            }
        }
        for record in store.pals() {
            if record.provenance.applicable_game_version != configured_game_version {
                stale.push(StaleRecord {
                    record_id: record.id.clone(),
                    record_type: "pal".to_string(),
                    applicable_game_version: record.provenance.applicable_game_version.clone(),
                });
            }
        }
        for record in store.technologies() {
            if record.provenance.applicable_game_version != configured_game_version {
                stale.push(StaleRecord {
                    record_id: record.id.clone(),
                    record_type: "technology".to_string(),
                    applicable_game_version: record.provenance.applicable_game_version.clone(),
                });
            }
        }
        for record in store.recipes() {
            if record.provenance.applicable_game_version != configured_game_version {
                stale.push(StaleRecord {
                    record_id: record.id.clone(),
                    record_type: "recipe".to_string(),
                    applicable_game_version: record.provenance.applicable_game_version.clone(),
                });
            }
        }
        for record in store.habitats() {
            if record.provenance.applicable_game_version != configured_game_version {
                stale.push(StaleRecord {
                    record_id: record.id.clone(),
                    record_type: "habitat".to_string(),
                    applicable_game_version: record.provenance.applicable_game_version.clone(),
                });
            }
        }
        for record in store.breeding_rules() {
            if record.provenance.applicable_game_version != configured_game_version {
                stale.push(StaleRecord {
                    record_id: record.id.clone(),
                    record_type: "breeding_rule".to_string(),
                    applicable_game_version: record.provenance.applicable_game_version.clone(),
                });
            }
        }
        for record in store.aliases() {
            if record.provenance.applicable_game_version != configured_game_version {
                stale.push(StaleRecord {
                    record_id: record.id.clone(),
                    record_type: "alias".to_string(),
                    applicable_game_version: record.provenance.applicable_game_version.clone(),
                });
            }
        }
        for record in store.progression_relationships() {
            if record.provenance.applicable_game_version != configured_game_version {
                stale.push(StaleRecord {
                    record_id: record.id.clone(),
                    record_type: "progression_relationship".to_string(),
                    applicable_game_version: record.provenance.applicable_game_version.clone(),
                });
            }
        }
        for record in store.conflicts() {
            if record.provenance.applicable_game_version != configured_game_version {
                stale.push(StaleRecord {
                    record_id: record.id.clone(),
                    record_type: "conflict".to_string(),
                    applicable_game_version: record.provenance.applicable_game_version.clone(),
                });
            }
        }

        stale
    }

    /// Validate a batch of JSONL lines without persisting to the canonical store.
    pub fn validate_batch(lines: &[&str]) -> BatchValidation {
        let mut records = Vec::new();
        let mut parse_errors = Vec::new();

        for (index, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            match serde_json::from_str::<KnowledgeRecord>(trimmed) {
                Ok(record) => records.push(record),
                Err(error) => {
                    parse_errors.push(format!("line {}: {}", index + 1, error));
                }
            }
        }

        if !parse_errors.is_empty() {
            return BatchValidation {
                valid: false,
                total_records: records.len(),
                errors: parse_errors,
            };
        }

        let total = records.len();
        match KnowledgeStore::from_records(records) {
            Ok(_) => BatchValidation {
                valid: true,
                total_records: total,
                errors: Vec::new(),
            },
            Err(validation_errors) => BatchValidation {
                valid: false,
                total_records: total,
                errors: validation_errors
                    .iter()
                    .map(|e: &ValidationError| e.to_string())
                    .collect(),
            },
        }
    }
}
