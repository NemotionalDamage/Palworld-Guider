use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewStatus {
    Candidate,
    Reviewed,
    Retired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    VerifiedTarget,
    Official,
    ReviewedSecondary,
    Community,
    Conflicted,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    pub source_id: String,
    pub applicable_game_version: String,
    pub retrieved_on: String,
    pub reviewer: String,
    pub review_status: ReviewStatus,
    pub confidence: Confidence,
    pub change_risk: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub corroborating_source_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalizationStatus {
    Resolved,
    Partial,
    Missing,
    NotApplicable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalEvidenceMetadata {
    pub source_table: String,
    pub localization_status: LocalizationStatus,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unresolved_fields: Vec<String>,
    pub transformation_notes: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRecord {
    pub id: String,
    pub title: String,
    pub supplier: String,
    pub retrieved_on: String,
    pub evidence_urls: Vec<String>,
    pub applicable_game_version: String,
    pub reviewer: String,
    pub review_status: ReviewStatus,
    pub confidence: Confidence,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocaleNames {
    pub en: String,
    pub zh_hans: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AcquisitionLead {
    pub action: String,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemRecord {
    pub id: String,
    pub names: LocaleNames,
    pub description: Option<String>,
    pub rarity: String,
    pub acquisition_leads: Vec<AcquisitionLead>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_row_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_evidence: Option<LocalEvidenceMetadata>,
    pub provenance: Provenance,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TechnologyRecord {
    pub id: String,
    pub names: LocaleNames,
    pub level: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_row_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_evidence: Option<LocalEvidenceMetadata>,
    pub provenance: Provenance,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecipeIngredient {
    pub item_id: String,
    pub quantity: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecipeItem {
    pub item_id: String,
    pub quantity: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecipeRecord {
    pub id: String,
    pub output: RecipeItem,
    pub ingredients: Vec<RecipeIngredient>,
    pub crafting_stations: Vec<String>,
    pub technology_id: Option<String>,
    pub crafting_seconds: Option<f32>,
    #[serde(default)]
    pub byproducts: Vec<RecipeItem>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_row_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_evidence: Option<LocalEvidenceMetadata>,
    pub provenance: Provenance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkKind {
    Kindling,
    Watering,
    Planting,
    GeneratingElectricity,
    Handiwork,
    Gathering,
    Lumbering,
    Mining,
    MedicineProduction,
    Transporting,
    Farming,
    Cooling,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkSuitability {
    pub kind: WorkKind,
    pub level: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PalStats {
    pub hp: u32,
    pub attack: u32,
    pub defense: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DropSource {
    pub item_id: String,
    pub min_quantity: u32,
    pub max_quantity: u32,
    pub probability_percent: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PalRecord {
    pub id: String,
    pub names: LocaleNames,
    pub stats: Option<PalStats>,
    pub work_suitability: Vec<WorkSuitability>,
    pub drops: Vec<DropSource>,
    pub habitat_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_row_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_evidence: Option<LocalEvidenceMetadata>,
    pub provenance: Provenance,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HabitatRecord {
    pub id: String,
    pub names: LocaleNames,
    pub pal_ids: Vec<String>,
    pub provenance: Provenance,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BreedingRuleRecord {
    pub id: String,
    pub parent_a_id: String,
    pub parent_b_id: String,
    pub child_id: String,
    pub notes: Option<String>,
    pub provenance: Provenance,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AliasRecord {
    pub id: String,
    pub alias: String,
    pub target_id: String,
    pub locale: String,
    pub provenance: Provenance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProgressionRelationKind {
    Unlocks,
    Requires,
    Improves,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProgressionRelationshipRecord {
    pub id: String,
    pub from_id: String,
    pub to_id: String,
    pub relation: ProgressionRelationKind,
    pub requirement: Option<String>,
    pub provenance: Provenance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictResolution {
    Unresolved,
    Resolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConflictRecord {
    pub id: String,
    pub subject_id: String,
    pub field: String,
    pub values: Vec<String>,
    pub source_ids: Vec<String>,
    pub resolution: ConflictResolution,
    pub provenance: Provenance,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "record_type", rename_all = "snake_case")]
pub enum KnowledgeRecord {
    Source(SourceRecord),
    Item(ItemRecord),
    Pal(PalRecord),
    Technology(TechnologyRecord),
    Recipe(RecipeRecord),
    Habitat(HabitatRecord),
    BreedingRule(BreedingRuleRecord),
    Alias(AliasRecord),
    ProgressionRelationship(ProgressionRelationshipRecord),
    Conflict(ConflictRecord),
}
