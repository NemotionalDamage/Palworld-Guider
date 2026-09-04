//! Version compatibility checks across game, knowledge, index, and embedding dimensions.

use std::collections::BTreeSet;

use game_knowledge::KnowledgeStore;
use serde::{Deserialize, Serialize};

/// Embedding version metadata. Always `None` while semantic vector search is deferred.
pub type EmbeddingVersion = Option<String>;

/// One dimension of the version compatibility report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionDimension {
    pub label: String,
    pub value: String,
    pub matches: bool,
}

/// Aggregated version compatibility report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionReport {
    pub knowledge_version: String,
    pub configured_game_version: Option<String>,
    pub index_version: Option<String>,
    pub embedding_version: EmbeddingVersion,
    pub game_version_matches: bool,
    pub warnings: Vec<String>,
}

impl VersionReport {
    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }

    pub fn all_match(&self) -> bool {
        self.game_version_matches && !self.has_warnings()
    }
}

/// Collect all unique `applicable_game_version` values from the store's records.
fn collect_knowledge_versions(store: &KnowledgeStore) -> BTreeSet<String> {
    let mut versions = BTreeSet::new();
    for record in store.items() {
        versions.insert(record.provenance.applicable_game_version.clone());
    }
    for record in store.pals() {
        versions.insert(record.provenance.applicable_game_version.clone());
    }
    for record in store.technologies() {
        versions.insert(record.provenance.applicable_game_version.clone());
    }
    for record in store.recipes() {
        versions.insert(record.provenance.applicable_game_version.clone());
    }
    for record in store.habitats() {
        versions.insert(record.provenance.applicable_game_version.clone());
    }
    for record in store.breeding_rules() {
        versions.insert(record.provenance.applicable_game_version.clone());
    }
    for record in store.aliases() {
        versions.insert(record.provenance.applicable_game_version.clone());
    }
    for record in store.progression_relationships() {
        versions.insert(record.provenance.applicable_game_version.clone());
    }
    for record in store.conflicts() {
        versions.insert(record.provenance.applicable_game_version.clone());
    }
    versions
}

/// Determine the knowledge version string from a set of versions.
fn knowledge_version_string(versions: &BTreeSet<String>) -> String {
    match versions.len() {
        0 => "unknown".to_string(),
        1 => versions
            .iter()
            .next()
            .expect("one version is present")
            .clone(),
        _ => "mixed".to_string(),
    }
}

/// Check version compatibility across all dimensions.
///
/// * `store` - The loaded knowledge store.
/// * `configured_game_version` - The game version the user has configured, if any.
/// * `index_version` - The knowledge version reported by the Tantivy index, if built.
pub fn version_check(
    store: &KnowledgeStore,
    configured_game_version: Option<&str>,
    index_version: Option<&str>,
) -> VersionReport {
    let versions = collect_knowledge_versions(store);
    let knowledge_version = knowledge_version_string(&versions);
    let embedding_version: EmbeddingVersion = None;

    let game_version_matches = match configured_game_version {
        Some(configured) => {
            knowledge_version != "mixed"
                && knowledge_version != "unknown"
                && knowledge_version == configured
        }
        None => true,
    };

    let mut warnings = Vec::new();

    if !game_version_matches {
        warnings.push(format!(
            "knowledge version {} does not match configured game version {}",
            knowledge_version,
            configured_game_version.unwrap_or("unknown")
        ));
    }

    if knowledge_version == "mixed" {
        let all_versions: Vec<String> = versions.iter().cloned().collect();
        warnings.push(format!(
            "knowledge base contains mixed game versions: {}",
            all_versions.join(", ")
        ));
    }

    if knowledge_version == "unknown" {
        warnings.push("knowledge base contains no reviewed records".to_string());
    }

    if let Some(idx_ver) = index_version {
        if idx_ver != knowledge_version && knowledge_version != "unknown" {
            warnings.push(format!(
                "index version {} does not match knowledge version {}",
                idx_ver, knowledge_version
            ));
        }
    }

    if let Some(emb_ver) = &embedding_version {
        if emb_ver != &knowledge_version && knowledge_version != "unknown" {
            warnings.push(format!(
                "embedding version {} does not match knowledge version {}",
                emb_ver, knowledge_version
            ));
        }
    }

    VersionReport {
        knowledge_version,
        configured_game_version: configured_game_version.map(str::to_string),
        index_version: index_version.map(str::to_string),
        embedding_version,
        game_version_matches,
        warnings,
    }
}
