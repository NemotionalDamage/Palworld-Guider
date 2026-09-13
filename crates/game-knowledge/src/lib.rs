//! Reviewed, versioned Palworld knowledge records.

mod backfill;
mod error;
mod local_build;
mod local_map;
mod models;
mod store;

pub use backfill::{
    audit_canonical_backfill, canonical_backfill_output_is_safe, BackfillFact, BackfillSummary,
    CanonicalBackfillReport,
};
pub use error::ValidationError;
pub use local_build::{
    candidate_output_is_safe, generate_candidates, AntiBiasAudit, IntakeBatch, IntakeCandidateSet,
    IntakeReport, IntakeRowOutcome, LocalBuildError, LocalBuildLocale, LocalBuildTables,
    LocalDropRow, LocalItemRow, LocalPalRow, LocalRecipeMaterial, LocalRecipeRow,
    LocalTechnologyRow, LocalizationIndex, RepresentativeError, TableCoverage,
};
pub use local_map::{generate_map_candidates, MapCandidateReport, MapIntakeBatch, MapIntakeError};
pub use models::{
    AcquisitionLead, AdditionalEffectType, AliasRecord, BreedingRuleRecord, Confidence,
    ConflictRecord, ConflictResolution, DropSource, ElementType, HabitatRecord, ItemRecord,
    KnowledgeRecord, LocalEvidenceMetadata, LocaleNames, LocalizationStatus, MapBounds,
    MapDefinitionRecord, MapPointKind, MapPointRecord, MapRegionRecord, MapShape,
    PalHabitatZoneRecord, PalRecord, PalSpawnPlacementKind, PalWazaUnlock, ProgressionRelationKind,
    ProgressionRelationshipRecord, Provenance, RecipeIngredient, RecipeItem, RecipeRecord,
    ReviewStatus, SourceRecord, TechnologyRecord, TypeEffectivenessRecord, WazaCategory,
    WazaRecord, WazaStrength, WildSpawnReview, WorkKind, WorkKindDescriptionRecord,
    WorkSuitability, WorldCoordinate,
};
pub use store::KnowledgeStore;
