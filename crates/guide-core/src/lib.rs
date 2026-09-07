//! Phase G2 deterministic guide core.

mod answers;
mod breeding;
mod calculators;
mod enriched;
mod lookup;
mod map;
mod resolver;

pub use answers::{AnswerStatus, GuideAnswer, ProvenanceSummary, VersionInfo};
pub use breeding::{BreedingChain, BreedingResult, BreedingStep};
pub use calculators::{
    ByproductTotal, CraftableCalculation, InventoryAmount, InventoryEntry, MaterialAcquisition,
    MaterialCalculation, MaterialNode, MaterialTotal, ShortageCalculation, ShortageMaterial,
};
pub use lookup::{ByproductRecipeSummary, ItemLookup, PalLookup, RecipeLookup, TechnologyLookup};
pub use map::{
    CoordinateLocation, MapPointDistance, NearbyHabitatZone, NearbyMapPoint,
    NormalizedMapCoordinate,
};
pub use resolver::{EntityKind, Resolution, ResolvedEntity};

use game_knowledge::KnowledgeStore;
use std::path::Path;

pub use game_knowledge::{MapPointKind, WorldCoordinate};

#[derive(Debug, Clone)]
pub struct GuideEngine {
    store: KnowledgeStore,
    configured_game_version: Option<String>,
    calculation_depth_limit: usize,
}

impl GuideEngine {
    pub fn new(store: KnowledgeStore, configured_game_version: Option<String>) -> Self {
        Self {
            store,
            configured_game_version,
            calculation_depth_limit: 8,
        }
    }

    pub fn load_directory(
        path: impl AsRef<Path>,
        configured_game_version: Option<String>,
    ) -> Result<Self, String> {
        let store = KnowledgeStore::load_directory(path).map_err(|errors| {
            errors
                .into_iter()
                .map(|error| error.to_string())
                .collect::<Vec<_>>()
                .join("; ")
        })?;
        Ok(Self::new(store, configured_game_version))
    }

    pub fn store(&self) -> &KnowledgeStore {
        &self.store
    }
}
