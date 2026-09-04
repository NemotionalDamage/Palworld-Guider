use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::models::{
    AliasRecord, Confidence, ElementType, ItemRecord, KnowledgeRecord, LocalEvidenceMetadata,
    LocaleNames, LocalizationStatus, PalRecord, ProgressionRelationKind,
    ProgressionRelationshipRecord, Provenance, RecipeIngredient, RecipeItem, RecipeRecord,
    ReviewStatus, WorkKind, WorkSuitability,
};

const ITEM_FIELDS: &[&str] = &[
    "TypeA",
    "TypeB",
    "Rank",
    "Rarity",
    "MaxStackCount",
    "Weight",
    "Price",
    "bLegalInGame",
    "TechnologyTreeLock",
];
const RECIPE_FIELDS: &[&str] = &[
    "Product_Id",
    "Product_Count",
    "WorkAmount",
    "WorkableAttribute",
    "UnlockItemID",
    "Material1_Id",
    "Material1_Count",
    "Material2_Id",
    "Material2_Count",
    "Material3_Id",
    "Material3_Count",
    "Material4_Id",
    "Material4_Count",
    "Material5_Id",
    "Material5_Count",
];
const TECHNOLOGY_FIELDS: &[&str] = &[
    "UnlockBuildObjects",
    "UnlockItemRecipes",
    "RequireTechnology",
    "RequireResearchId",
    "LevelCap",
    "Tier",
    "Cost",
];
const PAL_FIELDS: &[&str] = &[
    "IsPal",
    "Hp",
    "MeleeAttack",
    "ShotAttack",
    "Defense",
    "Support",
    "CraftSpeed",
    "ElementType1",
    "ElementType2",
    "WorkSuitability_EmitFlame",
    "WorkSuitability_Watering",
    "WorkSuitability_Seeding",
    "WorkSuitability_GenerateElectricity",
    "WorkSuitability_Handcraft",
    "WorkSuitability_Collection",
    "WorkSuitability_Deforest",
    "WorkSuitability_Mining",
    "WorkSuitability_OilExtraction",
    "WorkSuitability_ProductMedicine",
    "WorkSuitability_Cool",
    "WorkSuitability_Transport",
    "WorkSuitability_MonsterFarm",
];
const DROP_FIELDS: &[&str] = &["CharacterID", "Level"];

#[derive(Debug)]
pub enum LocalBuildError {
    MissingTable {
        path: String,
    },
    InvalidTable {
        path: String,
        message: String,
    },
    LocalizationConflict {
        table: String,
        row_id: String,
        localized_value: String,
        source_value: String,
    },
    InvalidLocalization {
        table: String,
        row_id: String,
        reason: String,
    },
    Io {
        path: String,
        message: String,
    },
}

impl fmt::Display for LocalBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingTable { path } => write!(formatter, "required local table is missing: {path}"),
            Self::InvalidTable { path, message } => {
                write!(formatter, "invalid local table {path}: {message}")
            }
            Self::LocalizationConflict {
                table,
                row_id,
                localized_value,
                source_value,
            } => write!(
                formatter,
                "localized values conflict in {table} for {row_id}: {localized_value} versus {source_value}"
            ),
            Self::InvalidLocalization {
                table,
                row_id,
                reason,
            } => write!(formatter, "invalid localization in {table} for {row_id}: {reason}"),
            Self::Io { path, message } => write!(formatter, "cannot read {path}: {message}"),
        }
    }
}

impl std::error::Error for LocalBuildError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalBuildLocale {
    English,
    SimplifiedChinese,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepresentativeError {
    pub stable_hash: u64,
    pub native_row_id: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TableCoverage {
    pub name: String,
    pub total_rows: usize,
    pub parsed_rows: usize,
    pub skipped_rows: usize,
    pub failed_rows: usize,
    pub skip_reasons: BTreeMap<String, usize>,
    pub failure_reasons: BTreeMap<String, usize>,
    pub fields_observed: BTreeMap<String, usize>,
    pub fields_missing: BTreeMap<String, usize>,
    pub fields_null: BTreeMap<String, usize>,
    pub fields_none_sentinel: BTreeMap<String, usize>,
    pub fields_empty_string: BTreeMap<String, usize>,
    pub fields_zero: BTreeMap<String, usize>,
    pub fields_false: BTreeMap<String, usize>,
    pub unsupported_enum_forms: BTreeMap<String, usize>,
    pub localization_hits: BTreeMap<String, usize>,
    pub localization_misses: BTreeMap<String, usize>,
    pub localization_rejections: BTreeMap<String, usize>,
    pub localization_conflicts: usize,
    pub duplicate_native_ids: usize,
    pub invalid_references: usize,
    pub representative_errors: Vec<RepresentativeError>,
    pub parse_failure_rows: Vec<RepresentativeError>,
}

impl TableCoverage {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            ..Self::default()
        }
    }

    fn increment(map: &mut BTreeMap<String, usize>, key: &str) {
        *map.entry(key.to_string()).or_insert(0) += 1;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LocalItemRow {
    pub native_row_id: String,
    pub type_a: Option<String>,
    pub type_b: Option<String>,
    pub rank: Option<i64>,
    pub rarity: Option<i64>,
    pub max_stack_count: Option<i64>,
    pub weight: Option<f64>,
    pub price: Option<i64>,
    pub is_legal_in_game: Option<bool>,
    pub technology_tree_lock: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocalRecipeMaterial {
    pub item_id: String,
    pub quantity: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LocalRecipeRow {
    pub native_row_id: String,
    pub product_id: Option<String>,
    pub product_count: Option<i64>,
    pub materials: Vec<LocalRecipeMaterial>,
    pub work_amount: Option<f64>,
    pub workable_attribute: Option<i64>,
    pub unlock_item_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocalTechnologyRow {
    pub native_row_id: String,
    pub unlock_build_objects: Vec<String>,
    pub unlock_item_recipes: Vec<String>,
    pub required_technology: Option<String>,
    pub required_research_id: Option<String>,
    pub level_cap: Option<i64>,
    pub tier: Option<i64>,
    pub cost: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocalPalStats {
    pub hp: i64,
    pub melee_attack: i64,
    pub shot_attack: i64,
    pub defense: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocalPalRow {
    pub native_row_id: String,
    pub is_pal: Option<bool>,
    pub stats: Option<LocalPalStats>,
    pub element_type1: Option<String>,
    pub element_type2: Option<String>,
    pub work_suitability: BTreeMap<String, i64>,
}

impl LocalPalRow {
    pub fn work_suitability(&self, native_kind: &str) -> Option<i64> {
        self.work_suitability.get(native_kind).copied()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LocalDropRow {
    pub native_row_id: String,
    pub character_id: String,
    pub level: Option<i64>,
    pub item_id: String,
    pub rate: f64,
    pub min_quantity: i64,
    pub max_quantity: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LocalBuildTables {
    items: BTreeMap<String, LocalItemRow>,
    recipes: BTreeMap<String, LocalRecipeRow>,
    technologies: BTreeMap<String, LocalTechnologyRow>,
    pals: BTreeMap<String, LocalPalRow>,
    drops: BTreeMap<String, Vec<LocalDropRow>>,
    coverage: Vec<TableCoverage>,
}

impl LocalBuildTables {
    pub fn load(root: &Path) -> Result<Self, LocalBuildError> {
        let (items, item_coverage) = load_table(
            root,
            "Pal/Content/Pal/DataTable/Item/DT_ItemDataTable.json",
            ITEM_FIELDS,
            &[("TypeA", "EPalItemTypeA::"), ("TypeB", "EPalItemTypeB::")],
            parse_item,
        )?;
        let (recipes, recipe_coverage) = load_table(
            root,
            "Pal/Content/Pal/DataTable/Item/DT_ItemRecipeDataTable.json",
            RECIPE_FIELDS,
            &[],
            parse_recipe,
        )?;
        let (technologies, technology_coverage) = load_table(
            root,
            "Pal/Content/Pal/DataTable/Technology/DT_TechnologyRecipeUnlock.json",
            TECHNOLOGY_FIELDS,
            &[],
            parse_technology,
        )?;
        let (pals, pal_coverage) = load_table(
            root,
            "Pal/Content/Pal/DataTable/Character/DT_PalMonsterParameter.json",
            PAL_FIELDS,
            &[],
            parse_pal,
        )?;
        let (drop_rows, drop_coverage) = load_table(
            root,
            "Pal/Content/Pal/DataTable/Character/DT_PalDropItem_Common.json",
            DROP_FIELDS,
            &[],
            parse_drop,
        )?;
        let mut drops = BTreeMap::<String, Vec<LocalDropRow>>::new();
        for (_, expanded_rows) in drop_rows {
            for row in expanded_rows {
                drops.entry(row.character_id.clone()).or_default().push(row);
            }
        }

        Ok(Self {
            items,
            recipes,
            technologies,
            pals,
            drops,
            coverage: vec![
                item_coverage,
                recipe_coverage,
                technology_coverage,
                pal_coverage,
                drop_coverage,
            ],
        })
    }

    pub fn item(&self, row_id: &str) -> Option<&LocalItemRow> {
        self.items.get(row_id)
    }

    pub fn recipe(&self, row_id: &str) -> Option<&LocalRecipeRow> {
        self.recipes.get(row_id)
    }

    pub fn technology(&self, row_id: &str) -> Option<&LocalTechnologyRow> {
        self.technologies.get(row_id)
    }

    pub fn pal(&self, row_id: &str) -> Option<&LocalPalRow> {
        self.pals.get(row_id)
    }

    pub fn drops_for(&self, character_id: &str) -> Vec<&LocalDropRow> {
        self.drops
            .get(character_id)
            .map(|rows| rows.iter().collect())
            .unwrap_or_default()
    }

    pub fn item_rows(&self) -> Vec<&LocalItemRow> {
        self.items.values().collect()
    }

    pub fn recipe_rows(&self) -> Vec<&LocalRecipeRow> {
        self.recipes.values().collect()
    }

    pub fn technology_rows(&self) -> Vec<&LocalTechnologyRow> {
        self.technologies.values().collect()
    }

    pub fn pal_rows(&self) -> Vec<&LocalPalRow> {
        self.pals.values().collect()
    }

    pub fn drop_rows(&self) -> Vec<&LocalDropRow> {
        self.drops.values().flatten().collect()
    }

    pub fn coverage(&self) -> Vec<TableCoverage> {
        self.coverage.clone()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct LocalizationEntry {
    localized_value: String,
    source_value: String,
    valid: bool,
    rejection_reason: Option<String>,
    values_conflict: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LocalizationIndex {
    item_names: BTreeMap<(LocalBuildLocale, String), LocalizationEntry>,
    item_descriptions: BTreeMap<(LocalBuildLocale, String), LocalizationEntry>,
    pal_names: BTreeMap<(LocalBuildLocale, String), LocalizationEntry>,
    map_object_names: BTreeMap<(LocalBuildLocale, String), LocalizationEntry>,
    skill_names: BTreeMap<(LocalBuildLocale, String), LocalizationEntry>,
    ui_common_text: BTreeMap<(LocalBuildLocale, String), LocalizationEntry>,
    coverage: Vec<TableCoverage>,
}

impl LocalizationIndex {
    pub fn load(root: &Path) -> Result<Self, LocalBuildError> {
        let mut index = Self {
            item_names: BTreeMap::new(),
            item_descriptions: BTreeMap::new(),
            pal_names: BTreeMap::new(),
            map_object_names: BTreeMap::new(),
            skill_names: BTreeMap::new(),
            ui_common_text: BTreeMap::new(),
            coverage: Vec::new(),
        };
        for (locale, locale_path) in [
            (
                LocalBuildLocale::English,
                "Pal/Content/L10N/en/Pal/DataTable/Text",
            ),
            (
                LocalBuildLocale::SimplifiedChinese,
                "Pal/Content/L10N/zh-Hans/Pal/DataTable/Text",
            ),
        ] {
            index.load_table(
                root,
                &format!("{locale_path}/DT_ItemNameText_Common.json"),
                locale,
                "ITEM_NAME_",
                "item_name",
                |index, row_id, entry| {
                    index.item_names.insert((locale, row_id.to_string()), entry);
                },
            )?;
            index.load_table(
                root,
                &format!("{locale_path}/DT_ItemDescriptionText_Common.json"),
                locale,
                "ITEM_DESC_",
                "item_description",
                |index, row_id, entry| {
                    index
                        .item_descriptions
                        .insert((locale, row_id.to_string()), entry);
                },
            )?;
            index.load_table(
                root,
                &format!("{locale_path}/DT_PalNameText_Common.json"),
                locale,
                "PAL_NAME_",
                "pal_name",
                |index, row_id, entry| {
                    index.pal_names.insert((locale, row_id.to_string()), entry);
                },
            )?;
            let map_object_path = format!("{locale_path}/DT_MapObjectNameText_Common.json");
            if root.join(&map_object_path).exists() {
                index.load_table(
                    root,
                    &map_object_path,
                    locale,
                    "MAPOBJECT_NAME_",
                    "map_object_name",
                    |index, row_id, entry| {
                        index
                            .map_object_names
                            .insert((locale, row_id.to_string()), entry);
                    },
                )?;
            }
            let skill_path = format!("{locale_path}/DT_SkillNameText_Common.json");
            if root.join(&skill_path).exists() {
                index.load_table(
                    root,
                    &skill_path,
                    locale,
                    "",
                    "skill_name",
                    |index, row_id, entry| {
                        index
                            .skill_names
                            .insert((locale, row_id.to_string()), entry);
                    },
                )?;
            }
            let ui_common_path = format!("{locale_path}/DT_UI_Common_Text_Common.json");
            if root.join(&ui_common_path).exists() {
                index.load_table(
                    root,
                    &ui_common_path,
                    locale,
                    "",
                    "ui_common_text",
                    |index, row_id, entry| {
                        index
                            .ui_common_text
                            .insert((locale, row_id.to_string()), entry);
                    },
                )?;
            }
        }
        Ok(index)
    }

    #[allow(clippy::too_many_lines)]
    fn load_table(
        &mut self,
        root: &Path,
        relative_path: &str,
        _locale: LocalBuildLocale,
        prefix: &str,
        field_name: &str,
        insert: impl Fn(&mut Self, &str, LocalizationEntry),
    ) -> Result<(), LocalBuildError> {
        let rows = read_export(root, relative_path)?;
        let mut coverage = TableCoverage::new(relative_path);
        coverage.total_rows = rows.len();
        for field in [
            "TextData",
            "Namespace",
            "Key",
            "SourceString",
            "LocalizedString",
        ] {
            coverage.fields_observed.insert(field.to_string(), 0);
        }

        for (native_key, row) in rows {
            let row_id = native_key
                .strip_prefix(prefix)
                .unwrap_or(&native_key)
                .trim_end_matches("_TextData")
                .to_string();
            let text = row.get("TextData");
            for field in [
                "TextData",
                "Namespace",
                "Key",
                "SourceString",
                "LocalizedString",
            ] {
                if text.and_then(|value| value.get(field)).is_some() {
                    TableCoverage::increment(&mut coverage.fields_observed, field);
                } else {
                    TableCoverage::increment(&mut coverage.fields_missing, field);
                }
            }
            let Some(text) = text else {
                TableCoverage::increment(&mut coverage.failure_reasons, "missing_text_data");
                coverage.failed_rows += 1;
                continue;
            };
            let (localized_value, source_value) = match (
                optional_string(text, "LocalizedString"),
                optional_string(text, "SourceString"),
            ) {
                (Ok(localized_value), Ok(source_value)) => (localized_value, source_value),
                (Err(error), _) | (_, Err(error)) => {
                    TableCoverage::increment(&mut coverage.failure_reasons, &error.reason);
                    coverage.failed_rows += 1;
                    continue;
                }
            };
            let Some(localized_value) = localized_value else {
                TableCoverage::increment(&mut coverage.failure_reasons, "missing_localized_string");
                coverage.failed_rows += 1;
                continue;
            };
            let source_value = source_value.unwrap_or_default();
            let mut entry = LocalizationEntry {
                localized_value: localized_value.clone(),
                source_value: source_value.clone(),
                valid: true,
                rejection_reason: None,
                values_conflict: false,
            };
            if localized_value.contains('\u{fffd}') {
                entry.valid = false;
                entry.rejection_reason = Some("invalid_unicode".to_string());
                TableCoverage::increment(&mut coverage.localization_rejections, "invalid_unicode");
            } else if is_placeholder(&localized_value) {
                entry.valid = false;
                entry.rejection_reason = Some("placeholder".to_string());
                TableCoverage::increment(&mut coverage.localization_rejections, "placeholder");
            }
            if !source_value.is_empty()
                && !localized_value.is_empty()
                && source_value != localized_value
            {
                entry.values_conflict = true;
                coverage.localization_conflicts += 1;
            }
            if entry.valid {
                coverage.parsed_rows += 1;
                TableCoverage::increment(&mut coverage.localization_hits, field_name);
            } else {
                coverage.failed_rows += 1;
                TableCoverage::increment(&mut coverage.localization_misses, field_name);
            }
            insert(self, &row_id, entry);
        }
        self.coverage.push(coverage);
        Ok(())
    }

    pub fn item_name(
        &self,
        item_row_id: &str,
        locale: LocalBuildLocale,
    ) -> Result<Option<String>, LocalBuildError> {
        self.lookup(
            &self.item_names,
            "DT_ItemNameText_Common",
            item_row_id,
            locale,
        )
    }

    pub fn item_description(
        &self,
        item_row_id: &str,
        locale: LocalBuildLocale,
    ) -> Result<Option<String>, LocalBuildError> {
        self.lookup(
            &self.item_descriptions,
            "DT_ItemDescriptionText_Common",
            item_row_id,
            locale,
        )
    }

    pub fn pal_name(
        &self,
        pal_row_id: &str,
        locale: LocalBuildLocale,
    ) -> Result<Option<String>, LocalBuildError> {
        self.lookup(&self.pal_names, "DT_PalNameText_Common", pal_row_id, locale)
    }

    /// Returns the target-build item description with its inline rich-text tags
    /// expanded into reviewed display text. Tag families resolved: itemName
    /// (exact, then rank-suffix fallback to the base row), characterName,
    /// mapObjectName/MapObjectName, activeSkillName (whole key then
    /// ACTION_SKILL_/PASSIVE_ prefixed), uiCommon, and element icon (`img`
    /// `ElemIcon_*`) which carries no textual content and is dropped when it
    /// immediately precedes the matching element-name tag. Raw description
    /// keys are matched case-insensitively because localization export keys can
    /// differ in case from DataTable row ids (for example `ITEM_DESC_HEAD001`
    /// versus row `Head001`). Whitespace is normalized to single spaces.
    pub fn expanded_item_description(
        &self,
        item_row_id: &str,
        locale: LocalBuildLocale,
    ) -> Result<Option<String>, LocalBuildError> {
        let raw = self
            .localized_entry_ci(
                &self.item_descriptions,
                "DT_ItemDescriptionText_Common",
                item_row_id,
                locale,
            )?
            .map(|entry| entry.localized_value.clone());
        let Some(raw) = raw else {
            return Ok(None);
        };
        Ok(Some(self.expand_rich_text(&raw, locale)))
    }

    fn localized_entry_ci<'a>(
        &'a self,
        table: &'a BTreeMap<(LocalBuildLocale, String), LocalizationEntry>,
        table_name: &str,
        row_id: &str,
        locale: LocalBuildLocale,
    ) -> Result<Option<&'a LocalizationEntry>, LocalBuildError> {
        let entry = table.get(&(locale, row_id.to_string())).or_else(|| {
            table
                .iter()
                .find(|((entry_locale, key), _)| {
                    *entry_locale == locale && key.eq_ignore_ascii_case(row_id)
                })
                .map(|(_, entry)| entry)
        });
        let Some(entry) = entry else {
            return Ok(None);
        };
        if let Some(reason) = &entry.rejection_reason {
            return Err(LocalBuildError::InvalidLocalization {
                table: table_name.to_string(),
                row_id: row_id.to_string(),
                reason: reason.clone(),
            });
        }
        if entry.values_conflict {
            return Err(LocalBuildError::LocalizationConflict {
                table: table_name.to_string(),
                row_id: row_id.to_string(),
                localized_value: entry.localized_value.clone(),
                source_value: entry.source_value.clone(),
            });
        }
        Ok(Some(entry))
    }

    fn rich_text_lookup(
        &self,
        table: &BTreeMap<(LocalBuildLocale, String), LocalizationEntry>,
        row_id: &str,
        locale: LocalBuildLocale,
    ) -> Option<String> {
        self.localized_entry_ci(table, "", row_id, locale)
            .ok()
            .flatten()
            .map(|entry| entry.localized_value.clone())
    }

    fn expand_rich_text(&self, raw: &str, locale: LocalBuildLocale) -> String {
        let mut output = String::with_capacity(raw.len());
        let mut remaining = raw;
        while let Some(open) = remaining.find('<') {
            output.push_str(&remaining[..open]);
            let after_open = &remaining[open + 1..];
            let Some(close) = after_open.find('>') else {
                output.push_str(&remaining[open..]);
                break;
            };
            let tag_text = &after_open[..close];
            let (resolved, drop_tag) = self.resolve_tag(tag_text, locale);
            if drop_tag {
                // Element icon tags carry no textual content.
            } else if let Some(text) = resolved {
                output.push_str(&text);
            } else {
                output.push_str(&remaining[open..open + close + 2]);
            }
            remaining = &after_open[close + 1..];
        }
        output.push_str(remaining);
        output.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    fn resolve_tag(&self, tag_text: &str, locale: LocalBuildLocale) -> (Option<String>, bool) {
        let mut parts = tag_text.split_whitespace();
        let Some(kind) = parts.next() else {
            return (None, false);
        };
        let id = parts.find_map(|part| {
            let rest = part.strip_prefix("id=|")?;
            let end = rest.find('|')?;
            Some(rest[..end].to_string())
        });
        let Some(id) = id else {
            return (None, false);
        };
        match kind {
            "img" => {
                let is_element_icon = id.starts_with("ElemIcon_");
                (None, is_element_icon)
            }
            "itemName" => {
                let exact = self.rich_text_lookup(&self.item_names, &id, locale);
                if exact.is_some() {
                    (exact, false)
                } else {
                    let base = strip_item_rank_suffix(&id);
                    (
                        base.and_then(|base| self.rich_text_lookup(&self.item_names, base, locale)),
                        false,
                    )
                }
            }
            "characterName" => (self.rich_text_lookup(&self.pal_names, &id, locale), false),
            "mapObjectName" | "MapObjectName" => (
                self.rich_text_lookup(&self.map_object_names, &id, locale),
                false,
            ),
            "activeSkillName" => {
                let direct = self.rich_text_lookup(&self.skill_names, &id, locale);
                if direct.is_some() {
                    (direct, false)
                } else {
                    let action = format!("ACTION_SKILL_{id}");
                    let action_value = self.rich_text_lookup(&self.skill_names, &action, locale);
                    if action_value.is_some() {
                        (action_value, false)
                    } else {
                        let passive = format!("PASSIVE_{id}");
                        (
                            self.rich_text_lookup(&self.skill_names, &passive, locale),
                            false,
                        )
                    }
                }
            }
            "uiCommon" => (
                self.rich_text_lookup(&self.ui_common_text, &id, locale),
                false,
            ),
            _ => (None, false),
        }
    }

    pub fn coverage(&self) -> Vec<TableCoverage> {
        self.coverage.clone()
    }

    pub fn item_name_keys(&self) -> Vec<(LocalBuildLocale, String)> {
        self.item_names
            .keys()
            .map(|(locale, row_id)| (*locale, row_id.clone()))
            .collect()
    }

    pub fn item_description_keys(&self) -> Vec<(LocalBuildLocale, String)> {
        self.item_descriptions
            .keys()
            .map(|(locale, row_id)| (*locale, row_id.clone()))
            .collect()
    }

    pub fn pal_name_keys(&self) -> Vec<(LocalBuildLocale, String)> {
        self.pal_names
            .keys()
            .map(|(locale, row_id)| (*locale, row_id.clone()))
            .collect()
    }

    fn lookup(
        &self,
        table: &BTreeMap<(LocalBuildLocale, String), LocalizationEntry>,
        table_name: &str,
        row_id: &str,
        locale: LocalBuildLocale,
    ) -> Result<Option<String>, LocalBuildError> {
        let Some(entry) = table.get(&(locale, row_id.to_string())) else {
            return Ok(None);
        };
        if let Some(reason) = &entry.rejection_reason {
            return Err(LocalBuildError::InvalidLocalization {
                table: table_name.to_string(),
                row_id: row_id.to_string(),
                reason: reason.clone(),
            });
        }
        if entry.values_conflict {
            return Err(LocalBuildError::LocalizationConflict {
                table: table_name.to_string(),
                row_id: row_id.to_string(),
                localized_value: entry.localized_value.clone(),
                source_value: entry.source_value.clone(),
            });
        }
        Ok(Some(entry.localized_value.clone()))
    }
}

fn strip_item_rank_suffix(row_id: &str) -> Option<&str> {
    let (base, suffix) = row_id.rsplit_once('_')?;
    if matches!(suffix, "2" | "3" | "4" | "5") {
        Some(base)
    } else {
        None
    }
}

fn parse_item(native_row_id: &str, row: &Value) -> Result<LocalItemRow, RowParseError> {
    Ok(LocalItemRow {
        native_row_id: native_row_id.to_string(),
        type_a: optional_enum(row, "TypeA")?,
        type_b: optional_enum(row, "TypeB")?,
        rank: optional_integer(row, "Rank")?,
        rarity: optional_integer(row, "Rarity")?,
        max_stack_count: optional_integer(row, "MaxStackCount")?,
        weight: optional_number(row, "Weight")?,
        price: optional_integer(row, "Price")?,
        is_legal_in_game: optional_bool(row, "bLegalInGame")?,
        technology_tree_lock: optional_integer(row, "TechnologyTreeLock")?,
    })
}

fn parse_recipe(native_row_id: &str, row: &Value) -> Result<LocalRecipeRow, RowParseError> {
    let mut materials = Vec::new();
    for index in 1..=5 {
        let item_id = optional_identifier(row, &format!("Material{index}_Id"))?;
        let Some(item_id) = item_id else {
            continue;
        };
        let quantity = required_integer(row, &format!("Material{index}_Count"))?;
        materials.push(LocalRecipeMaterial { item_id, quantity });
    }
    Ok(LocalRecipeRow {
        native_row_id: native_row_id.to_string(),
        product_id: optional_identifier(row, "Product_Id")?,
        product_count: optional_integer(row, "Product_Count")?,
        materials,
        work_amount: optional_number(row, "WorkAmount")?,
        workable_attribute: optional_integer(row, "WorkableAttribute")?,
        unlock_item_id: optional_identifier(row, "UnlockItemID")?,
    })
}

fn parse_technology(native_row_id: &str, row: &Value) -> Result<LocalTechnologyRow, RowParseError> {
    Ok(LocalTechnologyRow {
        native_row_id: native_row_id.to_string(),
        unlock_build_objects: optional_string_array(row, "UnlockBuildObjects")?,
        unlock_item_recipes: optional_string_array(row, "UnlockItemRecipes")?,
        required_technology: optional_identifier(row, "RequireTechnology")?,
        required_research_id: optional_identifier(row, "RequireResearchId")?,
        level_cap: optional_integer(row, "LevelCap")?,
        tier: optional_integer(row, "Tier")?,
        cost: optional_integer(row, "Cost")?,
    })
}

fn parse_pal(native_row_id: &str, row: &Value) -> Result<LocalPalRow, RowParseError> {
    let is_pal = optional_bool(row, "IsPal")?;
    let stats = if [
        optional_integer(row, "Hp")?,
        optional_integer(row, "MeleeAttack")?,
        optional_integer(row, "ShotAttack")?,
        optional_integer(row, "Defense")?,
    ]
    .iter()
    .all(|value| value.is_some())
    {
        Some(LocalPalStats {
            hp: optional_integer(row, "Hp")?.unwrap_or_default(),
            melee_attack: optional_integer(row, "MeleeAttack")?.unwrap_or_default(),
            shot_attack: optional_integer(row, "ShotAttack")?.unwrap_or_default(),
            defense: optional_integer(row, "Defense")?.unwrap_or_default(),
        })
    } else {
        None
    };
    let mut work_suitability = BTreeMap::new();
    for field in [
        "EmitFlame",
        "Watering",
        "Seeding",
        "GenerateElectricity",
        "Handcraft",
        "Collection",
        "Deforest",
        "Mining",
        "OilExtraction",
        "ProductMedicine",
        "Cool",
        "Transport",
        "MonsterFarm",
    ] {
        if let Some(level) = optional_integer(row, &format!("WorkSuitability_{field}"))? {
            work_suitability.insert(field.to_string(), level);
        }
    }
    Ok(LocalPalRow {
        native_row_id: native_row_id.to_string(),
        is_pal,
        stats,
        element_type1: optional_enum(row, "ElementType1")?,
        element_type2: optional_enum(row, "ElementType2")?,
        work_suitability,
    })
}

fn parse_drop(native_row_id: &str, row: &Value) -> Result<Vec<LocalDropRow>, RowParseError> {
    let character_id = required_identifier(row, "CharacterID")?;
    let level = optional_integer(row, "Level")?;
    let mut result = Vec::new();
    for index in 1..=10 {
        let Some(item_id) = optional_identifier(row, &format!("ItemId{index}"))? else {
            continue;
        };
        result.push(LocalDropRow {
            native_row_id: native_row_id.to_string(),
            character_id: character_id.clone(),
            level,
            item_id,
            rate: required_number(row, &format!("Rate{index}"))?,
            min_quantity: required_integer(row, &format!("min{index}"))?,
            max_quantity: required_integer(row, &format!("Max{index}"))?,
        });
    }
    Ok(result)
}

fn load_table<T>(
    root: &Path,
    relative_path: &str,
    fields: &[&str],
    enum_fields: &[(&str, &str)],
    parser: fn(&str, &Value) -> Result<T, RowParseError>,
) -> Result<(BTreeMap<String, T>, TableCoverage), LocalBuildError> {
    let rows = read_export(root, relative_path)?;
    let mut coverage = TableCoverage::new(relative_path);
    coverage.total_rows = rows.len();
    let mut parsed = BTreeMap::new();
    let mut errors = Vec::new();
    for (native_row_id, row) in rows {
        record_field_states(&mut coverage, &native_row_id, &row, fields, enum_fields);
        match parser(&native_row_id, &row) {
            Ok(value) => {
                if parsed.insert(native_row_id, value).is_some() {
                    coverage.duplicate_native_ids += 1;
                }
                coverage.parsed_rows += 1;
            }
            Err(error) => {
                TableCoverage::increment(&mut coverage.failure_reasons, &error.reason);
                coverage.failed_rows += 1;
                errors.push(RepresentativeError {
                    stable_hash: stable_hash(&native_row_id),
                    native_row_id: native_row_id.clone(),
                    message: error.message.clone(),
                });
                coverage.parse_failure_rows.push(RepresentativeError {
                    stable_hash: stable_hash(&native_row_id),
                    native_row_id,
                    message: error.message,
                });
            }
        }
    }
    errors.sort_by_key(|error| (error.stable_hash, error.native_row_id.clone()));
    errors.truncate(5);
    coverage.representative_errors = errors;
    Ok((parsed, coverage))
}

fn read_export(
    root: &Path,
    relative_path: &str,
) -> Result<BTreeMap<String, Value>, LocalBuildError> {
    let path: PathBuf = root.join(relative_path);
    if !path.exists() {
        return Err(LocalBuildError::MissingTable {
            path: relative_path.to_string(),
        });
    }
    let content = fs::read_to_string(&path).map_err(|error| LocalBuildError::Io {
        path: relative_path.to_string(),
        message: error.to_string(),
    })?;
    let value: Value =
        serde_json::from_str(&content).map_err(|error| LocalBuildError::InvalidTable {
            path: relative_path.to_string(),
            message: error.to_string(),
        })?;
    let Some(exports) = value.as_array() else {
        return Err(LocalBuildError::InvalidTable {
            path: relative_path.to_string(),
            message: "export root must be an array".to_string(),
        });
    };
    if exports.len() != 1 {
        return Err(LocalBuildError::InvalidTable {
            path: relative_path.to_string(),
            message: format!("expected one export object, found {}", exports.len()),
        });
    }
    let Some(rows) = exports[0].get("Rows").and_then(Value::as_object) else {
        return Err(LocalBuildError::InvalidTable {
            path: relative_path.to_string(),
            message: "export must contain a Rows object".to_string(),
        });
    };
    Ok(rows
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect())
}

fn record_field_states(
    coverage: &mut TableCoverage,
    _native_row_id: &str,
    row: &Value,
    fields: &[&str],
    enum_fields: &[(&str, &str)],
) {
    let row = row.as_object().cloned().unwrap_or_default();
    for field in fields {
        if let Some(value) = row.get(*field) {
            TableCoverage::increment(&mut coverage.fields_observed, field);
            match value {
                Value::Null => TableCoverage::increment(&mut coverage.fields_null, field),
                Value::String(value) => {
                    if value == "None" {
                        TableCoverage::increment(&mut coverage.fields_none_sentinel, field);
                    }
                    if value.is_empty() {
                        TableCoverage::increment(&mut coverage.fields_empty_string, field);
                    }
                }
                Value::Number(value) => {
                    if value.as_i64() == Some(0) || value.as_u64() == Some(0) {
                        TableCoverage::increment(&mut coverage.fields_zero, field);
                    }
                }
                Value::Bool(value) if !value => {
                    TableCoverage::increment(&mut coverage.fields_false, field);
                }
                _ => {}
            }
        } else {
            TableCoverage::increment(&mut coverage.fields_missing, field);
        }
    }
    for (field, expected_prefix) in enum_fields {
        if let Some(Value::String(value)) = row.get(*field) {
            if value != "None" && !value.starts_with(expected_prefix) {
                TableCoverage::increment(&mut coverage.unsupported_enum_forms, field);
            }
        }
    }
}

struct RowParseError {
    reason: String,
    message: String,
}

impl RowParseError {
    fn malformed(field: &str) -> Self {
        Self {
            reason: "malformed_number".to_string(),
            message: format!("{field} is not a number"),
        }
    }
}

fn optional_string(row: &Value, field: &str) -> Result<Option<String>, RowParseError> {
    match row.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => {
            if value == "None" {
                Ok(None)
            } else {
                Ok(Some(value.clone()))
            }
        }
        Some(_) => Err(RowParseError::malformed(field)),
    }
}

fn optional_enum(row: &Value, field: &str) -> Result<Option<String>, RowParseError> {
    match row.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => {
            if value == "None" {
                Ok(None)
            } else {
                Ok(Some(value.clone()))
            }
        }
        Some(_) => Err(RowParseError {
            reason: "malformed_enum".to_string(),
            message: format!("{field} is not an enum string"),
        }),
    }
}

fn optional_identifier(row: &Value, field: &str) -> Result<Option<String>, RowParseError> {
    optional_string(row, field)
}

fn optional_bool(row: &Value, field: &str) -> Result<Option<bool>, RowParseError> {
    match row.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(value)) => Ok(Some(*value)),
        Some(_) => Err(RowParseError {
            reason: "malformed_bool".to_string(),
            message: format!("{field} is not a boolean"),
        }),
    }
}

fn optional_number(row: &Value, field: &str) -> Result<Option<f64>, RowParseError> {
    match row.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(_)) => Ok(Some(required_number(row, field)?)),
        Some(Value::String(value)) if value == "None" => Ok(None),
        Some(_) => Err(RowParseError::malformed(field)),
    }
}

fn optional_integer(row: &Value, field: &str) -> Result<Option<i64>, RowParseError> {
    match row.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if value == "None" => Ok(None),
        Some(Value::Number(value)) => value.as_i64().map(Some).ok_or_else(|| RowParseError {
            reason: "non_integer".to_string(),
            message: format!("{field} must be an integer"),
        }),
        Some(_) => Err(RowParseError::malformed(field)),
    }
}

fn required_integer(row: &Value, field: &str) -> Result<i64, RowParseError> {
    row.get(field)
        .and_then(Value::as_i64)
        .ok_or_else(|| RowParseError {
            reason: "non_integer".to_string(),
            message: format!("{field} must be an integer"),
        })
}

fn required_number(row: &Value, field: &str) -> Result<f64, RowParseError> {
    row.get(field)
        .and_then(Value::as_f64)
        .ok_or_else(|| RowParseError::malformed(field))
}

fn required_identifier(row: &Value, field: &str) -> Result<String, RowParseError> {
    optional_identifier(row, field)?.ok_or_else(|| RowParseError {
        reason: "missing_identifier".to_string(),
        message: format!("{field} is required"),
    })
}

fn optional_string_array(row: &Value, field: &str) -> Result<Vec<String>, RowParseError> {
    match row.get(field) {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(values)) => values
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .map(str::to_string)
                    .ok_or_else(|| RowParseError {
                        reason: "malformed_string_array".to_string(),
                        message: format!("{field} contains a non-string entry"),
                    })
            })
            .collect(),
        Some(_) => Err(RowParseError {
            reason: "malformed_string_array".to_string(),
            message: format!("{field} is not an array"),
        }),
    }
}

fn is_placeholder(value: &str) -> bool {
    let normalized = value.trim().to_ascii_lowercase();
    matches!(
        normalized.as_str(),
        "en text" | "zh-hans text" | "english text"
    ) || normalized.starts_with("placeholder")
}

fn stable_hash(value: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn local_candidate_provenance() -> Provenance {
    Provenance {
        source_id: "SRC-LOCAL-BUILD-24575825-20260902".to_string(),
        applicable_game_version: "1.0.3".to_string(),
        retrieved_on: "2026-09-02".to_string(),
        reviewer: "Codex".to_string(),
        review_status: ReviewStatus::Candidate,
        confidence: Confidence::VerifiedTarget,
        change_risk: Some(
            "Unreviewed local-build candidate; semantic review is required before promotion."
                .to_string(),
        ),
        corroborating_source_ids: Vec::new(),
    }
}

fn stable_candidate_id(
    prefix: &str,
    native_row_id: &str,
    used_ids: &mut BTreeSet<String>,
) -> String {
    let sanitized = native_row_id
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' || character == '-' {
                character.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect::<String>();
    let mut id = format!("{prefix}_{sanitized}");
    if !used_ids.insert(id.clone()) {
        id = format!("{id}_{:016x}", stable_hash(native_row_id));
        used_ids.insert(id.clone());
    }
    id
}

fn includes(batch: IntakeBatch, component: IntakeBatch) -> bool {
    batch == IntakeBatch::All || batch == component
}

fn push_outcome(
    outcomes: &mut Vec<IntakeRowOutcome>,
    table: &str,
    native_row_id: &str,
    outcome: &str,
    reason: &str,
) {
    outcomes.push(IntakeRowOutcome {
        table: table.to_string(),
        native_row_id: native_row_id.to_string(),
        outcome: outcome.to_string(),
        reason: reason.to_string(),
        stable_hash: stable_hash(&format!("{table}:{native_row_id}:{reason}")),
    });
}

fn push_localization_outcome(
    outcomes: &mut Vec<IntakeRowOutcome>,
    table: &str,
    locale: LocalBuildLocale,
    native_row_id: &str,
    outcome: &str,
    reason: &str,
) {
    let locale_name = match locale {
        LocalBuildLocale::English => "en",
        LocalBuildLocale::SimplifiedChinese => "zh-Hans",
    };
    let table = format!("{table}:{locale_name}");
    push_outcome(outcomes, &table, native_row_id, outcome, reason);
}

fn table_parse_failures(coverage: &[TableCoverage], suffix: &str) -> Vec<RepresentativeError> {
    coverage
        .iter()
        .find(|table| table.name.ends_with(suffix))
        .map(|table| table.parse_failure_rows.clone())
        .unwrap_or_default()
}

fn coverage_total(coverage: &[TableCoverage], suffix: &str) -> usize {
    coverage
        .iter()
        .find(|table| table.name.ends_with(suffix))
        .map(|table| table.total_rows)
        .unwrap_or_default()
}

fn candidate_records(records: &[KnowledgeRecord]) -> usize {
    records.len()
}

fn work_kind(native_kind: &str) -> Option<WorkKind> {
    match native_kind {
        "EmitFlame" => Some(WorkKind::Kindling),
        "Watering" => Some(WorkKind::Watering),
        "Seeding" => Some(WorkKind::Planting),
        "GenerateElectricity" => Some(WorkKind::GeneratingElectricity),
        "Handcraft" => Some(WorkKind::Handiwork),
        "Collection" => Some(WorkKind::Gathering),
        "Deforest" => Some(WorkKind::Lumbering),
        "Mining" => Some(WorkKind::Mining),
        "ProductMedicine" => Some(WorkKind::MedicineProduction),
        "Transport" => Some(WorkKind::Transporting),
        "MonsterFarm" => Some(WorkKind::Farming),
        "Cool" => Some(WorkKind::Cooling),
        _ => None,
    }
}

fn anti_bias_audit(outcomes: &[IntakeRowOutcome]) -> AntiBiasAudit {
    let mut fixture_counts = BTreeMap::new();
    for outcome in outcomes {
        let category = outcome.table.split(':').next().unwrap_or(&outcome.table);
        *fixture_counts.entry(category.to_string()).or_insert(0) += 1;
    }
    AntiBiasAudit {
        seed: 0,
        algorithm: "full-population row outcomes ordered by table and stable FNV-1a native row ID"
            .to_string(),
        fixture_counts,
        production_entity_special_cases: 0,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IntakeRowOutcome {
    pub table: String,
    pub native_row_id: String,
    pub outcome: String,
    pub reason: String,
    pub stable_hash: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IntakeBatch {
    All,
    Items,
    Recipes,
    Technologies,
    Pals,
    PalDrops,
    WorkSuitability,
    LocalizationAliases,
    Relationships,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AntiBiasAudit {
    pub seed: u64,
    pub algorithm: String,
    pub fixture_counts: BTreeMap<String, usize>,
    pub production_entity_special_cases: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct IntakeReport {
    pub input_items: usize,
    pub candidate_items: usize,
    pub rejected_items: usize,
    pub input_recipes: usize,
    pub candidate_recipes: usize,
    pub skipped_recipes: usize,
    pub unresolved_recipe_references: usize,
    pub input_technologies: usize,
    pub candidate_technologies: usize,
    pub skipped_technologies: usize,
    pub input_pals: usize,
    pub candidate_pals: usize,
    pub skipped_pals: usize,
    pub localization_rejections: usize,
    pub unresolved_probability_units: usize,
    pub relationship_candidates: usize,
    pub canonical_matching: usize,
    pub canonical_corroborated: usize,
    pub canonical_conflicting: usize,
    pub canonical_local_only: usize,
    pub canonical_unresolved: usize,
    pub missing_localization: usize,
    pub invalid_references: usize,
    pub duplicate_native_ids: usize,
    pub schema_failures: usize,
    pub table_coverage: Vec<TableCoverage>,
    pub row_outcomes: Vec<IntakeRowOutcome>,
    pub anti_bias: AntiBiasAudit,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IntakeCandidateSet {
    pub records: Vec<KnowledgeRecord>,
    pub report: IntakeReport,
}

pub fn generate_candidates(
    tables: &LocalBuildTables,
    localization: &LocalizationIndex,
    batch: IntakeBatch,
) -> Result<IntakeCandidateSet, LocalBuildError> {
    let mut records = Vec::new();
    let mut outcomes = Vec::new();
    let mut used_ids = BTreeSet::new();
    let mut item_ids = BTreeMap::<String, String>::new();
    let mut recipe_ids = BTreeMap::<String, String>::new();
    let mut candidate_items = 0;
    let mut candidate_recipes = 0;
    let mut candidate_pals = 0;
    let mut unresolved_recipe_references = 0;
    let mut relationship_candidates = 0;
    let mut unresolved_probability_units = 0;

    if includes(batch, IntakeBatch::Items) {
        for error in table_parse_failures(&tables.coverage, "DT_ItemDataTable.json") {
            push_outcome(
                &mut outcomes,
                "DT_ItemDataTable",
                &error.native_row_id,
                "failure",
                "row_parse_failure",
            );
        }
        for row in tables.item_rows() {
            let Some(true) = row.is_legal_in_game else {
                push_outcome(
                    &mut outcomes,
                    "DT_ItemDataTable",
                    &row.native_row_id,
                    "skip",
                    "illegal_in_game",
                );
                continue;
            };
            let english_name =
                match localization.item_name(&row.native_row_id, LocalBuildLocale::English) {
                    Ok(Some(value)) => value,
                    Ok(None) => {
                        push_outcome(
                            &mut outcomes,
                            "DT_ItemDataTable",
                            &row.native_row_id,
                            "skip",
                            "missing_english_name",
                        );
                        continue;
                    }
                    Err(_) => {
                        push_outcome(
                            &mut outcomes,
                            "DT_ItemDataTable",
                            &row.native_row_id,
                            "failure",
                            "invalid_english_name",
                        );
                        continue;
                    }
                };
            let chinese_name = match localization
                .item_name(&row.native_row_id, LocalBuildLocale::SimplifiedChinese)
            {
                Ok(value) => value,
                Err(_) => {
                    push_outcome(
                        &mut outcomes,
                        "DT_ItemDataTable",
                        &row.native_row_id,
                        "failure",
                        "invalid_chinese_name",
                    );
                    continue;
                }
            };
            let description = match localization
                .item_description(&row.native_row_id, LocalBuildLocale::English)
            {
                Ok(value) => value,
                Err(_) => {
                    push_outcome(
                        &mut outcomes,
                        "DT_ItemDataTable",
                        &row.native_row_id,
                        "failure",
                        "invalid_description",
                    );
                    continue;
                }
            };
            let id = stable_candidate_id("ITEM", &row.native_row_id, &mut used_ids);
            let localization_status = if chinese_name.is_some() {
                LocalizationStatus::Resolved
            } else {
                LocalizationStatus::Partial
            };
            let unresolved_fields = vec![
                "rarity_numeric_semantics".to_string(),
                "type_a_semantics".to_string(),
                "type_b_semantics".to_string(),
            ];
            records.push(KnowledgeRecord::Item(ItemRecord {
                id: id.clone(),
                names: LocaleNames {
                    en: english_name.clone(),
                    zh_hans: chinese_name.clone(),
                },
                description,
                rarity: "unknown".to_string(),
                acquisition_leads: Vec::new(),
                native_row_id: Some(row.native_row_id.clone()),
                local_evidence: Some(LocalEvidenceMetadata {
                    source_table: "DT_ItemDataTable".to_string(),
                    localization_status: localization_status.clone(),
                    unresolved_fields,
                    transformation_notes:
                        "Direct typed field extraction; unresolved semantics remain explicit."
                            .to_string(),
                }),
                provenance: local_candidate_provenance(),
            }));
            item_ids.insert(row.native_row_id.clone(), id);
            candidate_items += 1;
            push_outcome(
                &mut outcomes,
                "DT_ItemDataTable",
                &row.native_row_id,
                "candidate",
                "item",
            );
        }
    }

    if includes(batch, IntakeBatch::Recipes) {
        for error in table_parse_failures(&tables.coverage, "DT_ItemRecipeDataTable.json") {
            push_outcome(
                &mut outcomes,
                "DT_ItemRecipeDataTable",
                &error.native_row_id,
                "failure",
                "row_parse_failure",
            );
        }
        for row in tables.recipe_rows() {
            let Some(product_native_id) = &row.product_id else {
                push_outcome(
                    &mut outcomes,
                    "DT_ItemRecipeDataTable",
                    &row.native_row_id,
                    "skip",
                    "missing_product",
                );
                unresolved_recipe_references += 1;
                continue;
            };
            let Some(output_id) = item_ids.get(product_native_id).cloned() else {
                push_outcome(
                    &mut outcomes,
                    "DT_ItemRecipeDataTable",
                    &row.native_row_id,
                    "skip",
                    "unresolved_output_reference",
                );
                unresolved_recipe_references += 1;
                continue;
            };
            let Some(output_quantity) = row.product_count.filter(|count| *count > 0) else {
                push_outcome(
                    &mut outcomes,
                    "DT_ItemRecipeDataTable",
                    &row.native_row_id,
                    "skip",
                    "invalid_output_quantity",
                );
                continue;
            };
            if row.materials.is_empty()
                || row.materials.iter().any(|material| material.quantity <= 0)
            {
                push_outcome(
                    &mut outcomes,
                    "DT_ItemRecipeDataTable",
                    &row.native_row_id,
                    "skip",
                    "invalid_ingredient_quantity",
                );
                continue;
            }
            let mut unresolved = false;
            for material in &row.materials {
                if !item_ids.contains_key(&material.item_id) {
                    unresolved = true;
                }
            }
            if unresolved {
                push_outcome(
                    &mut outcomes,
                    "DT_ItemRecipeDataTable",
                    &row.native_row_id,
                    "skip",
                    "unresolved_ingredient_reference",
                );
                unresolved_recipe_references += 1;
                continue;
            }
            let ingredients = row
                .materials
                .iter()
                .map(|material| RecipeIngredient {
                    item_id: item_ids.get(&material.item_id).cloned().unwrap_or_default(),
                    quantity: u32::try_from(material.quantity).unwrap_or_default(),
                })
                .collect::<Vec<_>>();
            let id = stable_candidate_id("RECIPE", &row.native_row_id, &mut used_ids);
            records.push(KnowledgeRecord::Recipe(RecipeRecord {
                id: id.clone(),
                output: RecipeItem {
                    item_id: output_id,
                    quantity: u32::try_from(output_quantity).unwrap_or_default(),
                },
                ingredients,
                crafting_stations: vec!["unresolved".to_string()],
                technology_id: None,
                crafting_seconds: None,
                byproducts: Vec::new(),
                native_row_id: Some(row.native_row_id.clone()),
                local_evidence: Some(LocalEvidenceMetadata {
                    source_table: "DT_ItemRecipeDataTable".to_string(),
                    localization_status: LocalizationStatus::NotApplicable,
                    unresolved_fields: vec![
                        "crafting_station_relationship".to_string(),
                        "work_amount_unit".to_string(),
                        "workable_attribute_semantics".to_string(),
                    ],
                    transformation_notes: "Direct product and material extraction; station and work units are unresolved."
                        .to_string(),
                }),
                provenance: local_candidate_provenance(),
            }));
            recipe_ids.insert(row.native_row_id.clone(), id);
            candidate_recipes += 1;
            push_outcome(
                &mut outcomes,
                "DT_ItemRecipeDataTable",
                &row.native_row_id,
                "candidate",
                "recipe",
            );
        }
    }

    let mut skipped_technologies = 0;
    if includes(batch, IntakeBatch::Technologies) {
        for row in tables.technology_rows() {
            let reason = if row.unlock_build_objects.is_empty()
                && row.unlock_item_recipes.is_empty()
            {
                "no_provable_relationship"
            } else if !row.unlock_build_objects.is_empty() && row.unlock_item_recipes.is_empty() {
                "map_object_identity_unverified"
            } else {
                "technology_name_localization_unverified"
            };
            push_outcome(
                &mut outcomes,
                "DT_TechnologyRecipeUnlock",
                &row.native_row_id,
                "skip",
                reason,
            );
            skipped_technologies += 1;
        }
    }

    if includes(batch, IntakeBatch::Pals) {
        for row in tables.pal_rows() {
            let Some(true) = row.is_pal else {
                push_outcome(
                    &mut outcomes,
                    "DT_PalMonsterParameter",
                    &row.native_row_id,
                    "skip",
                    "not_pal",
                );
                continue;
            };
            let english_name =
                match localization.pal_name(&row.native_row_id, LocalBuildLocale::English) {
                    Ok(Some(value)) => value,
                    Ok(None) => {
                        push_outcome(
                            &mut outcomes,
                            "DT_PalMonsterParameter",
                            &row.native_row_id,
                            "skip",
                            "missing_english_pal_name",
                        );
                        continue;
                    }
                    Err(_) => {
                        push_outcome(
                            &mut outcomes,
                            "DT_PalMonsterParameter",
                            &row.native_row_id,
                            "failure",
                            "invalid_english_pal_name",
                        );
                        continue;
                    }
                };
            let chinese_name = match localization
                .pal_name(&row.native_row_id, LocalBuildLocale::SimplifiedChinese)
            {
                Ok(value) => value,
                Err(_) => {
                    push_outcome(
                        &mut outcomes,
                        "DT_PalMonsterParameter",
                        &row.native_row_id,
                        "failure",
                        "invalid_chinese_pal_name",
                    );
                    continue;
                }
            };
            let work_suitability = row
                .work_suitability
                .iter()
                .filter_map(|(native_kind, level)| {
                    work_kind(native_kind).map(|kind| WorkSuitability {
                        kind,
                        level: (*level).clamp(1, 5) as u8,
                    })
                })
                .filter(|work| work.level > 0)
                .collect::<Vec<_>>();
            let id = stable_candidate_id("PAL", &row.native_row_id, &mut used_ids);
            let element_type1 = row
                .element_type1
                .as_deref()
                .and_then(ElementType::from_native);
            let element_type2 = row
                .element_type2
                .as_deref()
                .and_then(ElementType::from_native);
            records.push(KnowledgeRecord::Pal(PalRecord {
                id: id.clone(),
                names: LocaleNames {
                    en: english_name,
                    zh_hans: chinese_name,
                },
                stats: None,
                work_suitability,
                drops: Vec::new(),
                habitat_ids: Vec::new(),
                element_type1,
                element_type2,
                native_row_id: Some(row.native_row_id.clone()),
                local_evidence: Some(LocalEvidenceMetadata {
                    source_table: "DT_PalMonsterParameter".to_string(),
                    localization_status: LocalizationStatus::Resolved,
                    unresolved_fields: vec![
                        "stats_scale_semantics".to_string(),
                        "drop_probability_representation".to_string(),
                        "oil_extraction_work_kind".to_string(),
                    ],
                    transformation_notes: "Direct legal Pal, name, element type, and confirmed work-kind extraction; numeric stats and drop rates remain unresolved."
                        .to_string(),
                }),
                provenance: local_candidate_provenance(),
            }));
            candidate_pals += 1;
            push_outcome(
                &mut outcomes,
                "DT_PalMonsterParameter",
                &row.native_row_id,
                "candidate",
                "pal",
            );
            for (native_kind, level) in &row.work_suitability {
                if *level > 0 && work_kind(native_kind).is_some() {
                    let outcome_id = format!("{}:{native_kind}", row.native_row_id);
                    push_outcome(
                        &mut outcomes,
                        "DT_PalMonsterParameter.WorkSuitability",
                        &outcome_id,
                        "embedded_candidate",
                        "work_suitability",
                    );
                }
            }
        }
    }

    if includes(batch, IntakeBatch::PalDrops) {
        for row in tables.drop_rows() {
            push_outcome(
                &mut outcomes,
                "DT_PalDropItem_Common",
                &row.native_row_id,
                "skip",
                "drop_probability_representation_unresolved",
            );
            unresolved_probability_units += 1;
        }
    }

    if includes(batch, IntakeBatch::Relationships) {
        for technology in tables.technology_rows() {
            if technology
                .unlock_item_recipes
                .iter()
                .all(|native_id| recipe_ids.contains_key(native_id))
                && !technology.unlock_item_recipes.is_empty()
            {
                for recipe_native_id in &technology.unlock_item_recipes {
                    let Some(recipe_id) = recipe_ids.get(recipe_native_id) else {
                        continue;
                    };
                    let technology_id =
                        stable_candidate_id("TECHNOLOGY", &technology.native_row_id, &mut used_ids);
                    let id = stable_candidate_id(
                        "REL",
                        &format!("{}:{recipe_native_id}", technology.native_row_id),
                        &mut used_ids,
                    );
                    records.push(KnowledgeRecord::ProgressionRelationship(
                        ProgressionRelationshipRecord {
                            id,
                            from_id: technology_id,
                            to_id: recipe_id.clone(),
                            relation: ProgressionRelationKind::Unlocks,
                            requirement: None,
                            provenance: local_candidate_provenance(),
                        },
                    ));
                    relationship_candidates += 1;
                }
            }
        }
    }

    if includes(batch, IntakeBatch::LocalizationAliases) {
        for (locale, row_id) in localization.item_name_keys() {
            let result = localization.item_name(&row_id, locale);
            let (outcome, reason) = match (&result, item_ids.contains_key(&row_id)) {
                (Err(_), _) => ("failure", "invalid_localization"),
                (Ok(_), false) => ("skip", "unmatched_localization_key"),
                (Ok(None), _) => ("skip", "missing_localization"),
                (Ok(Some(_)), true) => ("embedded_candidate", "item_name"),
            };
            push_localization_outcome(
                &mut outcomes,
                "DT_ItemNameText_Common",
                locale,
                &row_id,
                outcome,
                reason,
            );
            if let (Ok(Some(alias)), true) = (&result, item_ids.contains_key(&row_id)) {
                if matches!(locale, LocalBuildLocale::SimplifiedChinese) {
                    let target_id = &item_ids[&row_id];
                    let id = stable_candidate_id(
                        "ALIAS_ITEM",
                        &format!("{row_id}:zh_hans"),
                        &mut used_ids,
                    );
                    records.push(KnowledgeRecord::Alias(AliasRecord {
                        id,
                        alias: alias.clone(),
                        target_id: target_id.clone(),
                        locale: "zh_hans".to_string(),
                        provenance: local_candidate_provenance(),
                    }));
                }
            }
        }
        for (locale, row_id) in localization.item_description_keys() {
            let outcome = if localization.item_description(&row_id, locale).is_err() {
                "failure"
            } else {
                "skip"
            };
            push_localization_outcome(
                &mut outcomes,
                "DT_ItemDescriptionText_Common",
                locale,
                &row_id,
                outcome,
                "description_requires_review",
            );
        }
        for (locale, row_id) in localization.pal_name_keys() {
            let result = localization.pal_name(&row_id, locale);
            let (outcome, reason) = match &result {
                Err(_) => ("failure", "invalid_localization"),
                Ok(None) => ("skip", "missing_localization"),
                Ok(Some(_)) => ("embedded_candidate", "pal_name"),
            };
            push_localization_outcome(
                &mut outcomes,
                "DT_PalNameText_Common",
                locale,
                &row_id,
                outcome,
                reason,
            );
        }
    }

    let mut table_coverage = tables.coverage();
    table_coverage.extend(localization.coverage());
    let input_items = coverage_total(&table_coverage, "DT_ItemDataTable.json");
    let input_recipes = coverage_total(&table_coverage, "DT_ItemRecipeDataTable.json");
    let input_technologies = coverage_total(&table_coverage, "DT_TechnologyRecipeUnlock.json");
    let input_pals = coverage_total(&table_coverage, "DT_PalMonsterParameter.json");
    let localization_rejections = outcomes
        .iter()
        .filter(|outcome| outcome.table.contains("Text_Common:") && outcome.outcome == "failure")
        .count();
    let anti_bias = anti_bias_audit(&outcomes);
    let report = IntakeReport {
        input_items,
        candidate_items,
        rejected_items: input_items.saturating_sub(candidate_items),
        input_recipes,
        candidate_recipes,
        skipped_recipes: input_recipes.saturating_sub(candidate_recipes),
        unresolved_recipe_references,
        input_technologies,
        candidate_technologies: 0,
        skipped_technologies,
        input_pals,
        candidate_pals,
        skipped_pals: input_pals.saturating_sub(candidate_pals),
        localization_rejections,
        unresolved_probability_units,
        relationship_candidates,
        canonical_matching: 0,
        canonical_corroborated: 0,
        canonical_conflicting: 0,
        canonical_local_only: candidate_records(&records),
        canonical_unresolved: 0,
        missing_localization: 0,
        invalid_references: unresolved_recipe_references,
        duplicate_native_ids: table_coverage
            .iter()
            .map(|table| table.duplicate_native_ids)
            .sum(),
        schema_failures: 0,
        table_coverage,
        row_outcomes: outcomes,
        anti_bias,
    };
    Ok(IntakeCandidateSet { records, report })
}

pub fn candidate_output_is_safe(path: &Path) -> bool {
    let components = path
        .components()
        .map(|component| component.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>();
    if components.iter().any(|component| component == "..") {
        return false;
    }
    components.windows(4).any(|window| {
        window
            == [
                ".local".to_string(),
                "research".to_string(),
                "local-build".to_string(),
                "candidates".to_string(),
            ]
    })
}
