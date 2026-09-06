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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ElementType {
    Normal,
    Fire,
    Water,
    Leaf,
    Earth,
    Ice,
    Electricity,
    Dark,
    Dragon,
}

impl ElementType {
    pub fn from_native(value: &str) -> Option<Self> {
        let stripped = value.strip_prefix("EPalElementType::").unwrap_or(value);
        match stripped {
            "Normal" => Some(Self::Normal),
            "Fire" => Some(Self::Fire),
            "Water" => Some(Self::Water),
            "Leaf" => Some(Self::Leaf),
            "Earth" => Some(Self::Earth),
            "Ice" => Some(Self::Ice),
            "Electricity" => Some(Self::Electricity),
            "Dark" => Some(Self::Dark),
            "Dragon" => Some(Self::Dragon),
            _ => None,
        }
    }
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
    pub work_suitability: Vec<WorkSuitability>,
    pub drops: Vec<DropSource>,
    pub habitat_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub element_type1: Option<ElementType>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub element_type2: Option<ElementType>,
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
pub struct TypeEffectivenessRecord {
    pub id: String,
    pub attacking_type: ElementType,
    pub defending_type: ElementType,
    pub multiplier: f32,
    pub notes: Option<String>,
    pub provenance: Provenance,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkKindDescriptionRecord {
    pub id: String,
    pub work_kind: WorkKind,
    pub names: LocaleNames,
    pub description: String,
    pub provenance: Provenance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WazaCategory {
    Shot,
    Melee,
}

impl WazaCategory {
    pub fn from_native(value: &str) -> Option<Self> {
        let stripped = value.strip_prefix("EPalWazaCategory::").unwrap_or(value);
        match stripped {
            "Shot" => Some(Self::Shot),
            "Melee" => Some(Self::Melee),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WazaStrength {
    None,
    Weak,
    Medium,
    Strong,
}

impl WazaStrength {
    pub fn from_native(value: &str) -> Option<Self> {
        let stripped = value.strip_prefix("EPalWazaStrength::").unwrap_or(value);
        match stripped {
            "None" => Some(Self::None),
            "Weak" => Some(Self::Weak),
            "Medium" => Some(Self::Medium),
            "Strong" => Some(Self::Strong),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdditionalEffectType {
    None,
    Burn,
    Wetness,
    Electrical,
    Freeze,
    Darkness,
    IvyCling,
    Muddy,
    Poison,
    Stun,
}

impl AdditionalEffectType {
    pub fn from_native(value: &str) -> Option<Self> {
        let stripped = value
            .strip_prefix("EPalAdditionalEffectType::")
            .unwrap_or(value);
        match stripped {
            "None" => Some(Self::None),
            "Burn" => Some(Self::Burn),
            "Wetness" => Some(Self::Wetness),
            "Electrical" => Some(Self::Electrical),
            "Freeze" => Some(Self::Freeze),
            "Darkness" => Some(Self::Darkness),
            "IvyCling" => Some(Self::IvyCling),
            "Muddy" => Some(Self::Muddy),
            "Poison" => Some(Self::Poison),
            "Stun" => Some(Self::Stun),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WazaRecord {
    pub id: String,
    pub native_waza_id: String,
    pub names: LocaleNames,
    pub element: Option<ElementType>,
    pub category: Option<WazaCategory>,
    pub power: u32,
    pub cool_time: f32,
    pub strength: WazaStrength,
    pub effect_type1: AdditionalEffectType,
    pub effect_value1: u32,
    pub effect_type2: AdditionalEffectType,
    pub effect_value2: u32,
    pub provenance: Provenance,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PalWazaUnlock {
    pub id: String,
    pub pal_id: String,
    pub waza_id: String,
    pub unlock_level: u32,
    pub provenance: Provenance,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WorldCoordinate {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MapBounds {
    pub min_x: f64,
    pub max_x: f64,
    pub min_y: f64,
    pub max_y: f64,
    pub min_z: f64,
    pub max_z: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MapDefinitionRecord {
    pub id: String,
    pub native_name: String,
    pub names: LocaleNames,
    pub bounds: MapBounds,
    pub logical_size: u32,
    pub priority: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub texture_asset_path: Option<String>,
    pub provenance: Provenance,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MapShape {
    Box {
        center: WorldCoordinate,
        extent_x: f64,
        extent_y: f64,
        extent_z: f64,
    },
    Sphere {
        center: WorldCoordinate,
        radius: f64,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MapRegionRecord {
    pub id: String,
    pub map_id: String,
    pub native_row_id: String,
    pub message_id: String,
    pub names: LocaleNames,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geometry: Option<MapShape>,
    #[serde(default)]
    pub boundary_is_reviewed: bool,
    pub provenance: Provenance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MapPointKind {
    FastTravel,
    BossTower,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MapPointRecord {
    pub id: String,
    pub map_id: String,
    pub native_id: String,
    pub kind: MapPointKind,
    pub names: LocaleNames,
    pub location: WorldCoordinate,
    pub local_evidence: LocalEvidenceMetadata,
    pub provenance: Provenance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PalSpawnPlacementKind {
    Field,
    Dungeon,
    DungeonBoss,
    FieldBoss,
    ImprisonmentBoss,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PalHabitatZoneRecord {
    pub id: String,
    pub map_id: String,
    pub native_placement_id: String,
    pub native_spawner_name: String,
    pub placement_kind: PalSpawnPlacementKind,
    pub location: WorldCoordinate,
    pub radius: f64,
    pub raw_pal_id: String,
    pub pal_id: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variant_labels: Vec<String>,
    pub level_min: u32,
    pub level_max: u32,
    pub count_min: u32,
    pub count_max: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_of_day: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weather: Option<String>,
    pub allows_randomizer: bool,
    pub respawn_cool_time_seconds: f64,
    pub local_evidence: LocalEvidenceMetadata,
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
    TypeEffectiveness(TypeEffectivenessRecord),
    WorkKindDescription(WorkKindDescriptionRecord),
    Waza(WazaRecord),
    PalWazaUnlock(PalWazaUnlock),
    MapDefinition(MapDefinitionRecord),
    MapRegion(MapRegionRecord),
    MapPoint(MapPointRecord),
    PalHabitatZone(PalHabitatZoneRecord),
}
