//! Import schemas: Derived (`importDerived`), Derived mirror (internal) and
//! the Import feature created for translated CAD files (`importForeign`)
//! (03c PSM-25, 03a PS-32, 06-08 IO-04).
//!
//! Cross-element references (PartStudioData, CADImportData) have no
//! dedicated parameter kind yet; they are stored as text references.

use super::common::*;
use super::curve::LENGTH_UNIT_NAMES;
use super::{Category, FeatureType};
use crate::params::{Hints, NumSpec, ParamSpec, QueryFilter};

pub fn register(out: &mut Vec<FeatureType>) {
    out.push(derived());
    out.push(derived_mirror());
    out.push(import_foreign());
}

pub fn derived() -> FeatureType {
    let new_ui = |v: &crate::params::ParamValues| v.bool("newUI");
    let p = vec![
        hidden(ParamSpec::bool("newUI", "New UI", true)),
        ParamSpec::text("partStudio", "Part Studio", "").visible(new_ui),
        ParamSpec::bool("preserveActiveSheetMetal", "Preserve active sheet metal models", false).visible(new_ui),
        // Live (qa5 DER1, part-studio/400): Locations is full width (no
        // mate connector button); Placement keeps its label.
        ParamSpec::query("location", "Locations", QueryFilter::MATE_CONNECTOR | QueryFilter::POINT_LIKE).hints(Hints::NO_MATE_CONNECTOR_BUTTON).describe("Select or create mate connectors in this Part Studio to position derived instances").visible(new_ui),
        ParamSpec::enum_("placement", "Placement", &[("AT_ORIGIN", "Base origin"), ("AT_MATE_CONNECTOR", "Base mate connector")]).hints(Hints::SHOW_LABEL).visible(new_ui),
        hidden(ParamSpec::num("mateConnectorIndex", "Mate connector index", NumSpec::integer(0, 0, 100_000))),
        // Name of the source Part Studio's mate connector aligned with the locations.
        ParamSpec::text("mateConnectorId", "Base mate connector", "").visible(move |v| new_ui(v) && v.is("placement", "AT_MATE_CONNECTOR")),
        hidden(ParamSpec::num("mateConnectorIndexInFeature", "Mate connector index in feature", NumSpec::integer(0, 0, 100_000))),
        ParamSpec::bool("includeMateConnectors", "Include mate connectors", true).visible(new_ui),
        ParamSpec::bool("includeProperties", "Include properties", true).hints(Hints::REMEMBER_PREVIOUS_VALUE).visible(new_ui),
        hidden(ParamSpec::query("parts", "Parts to import", QueryFilter::BODY)),
    ];
    FeatureType::new("importDerived", "Derived", Category::Import, p)
        .icon("derived")
        .describe("Insert parts, sketches or a whole Part Studio from another Part Studio")
}

/// Features bringing another Part Studio's parts (they depend on its state):
/// Derived and the assembly Mirror's "Derived mirror".
pub fn is_derived(type_id: &str) -> bool {
    type_id == "importDerived" || type_id == "derivedMirror"
}

/// Internal feature used by Assembly mirror.
pub fn derived_mirror() -> FeatureType {
    let p = vec![ParamSpec::text("partStudio", "Part Studio", "").hints(Hints::READ_ONLY | Hints::UNCONFIGURABLE)];
    FeatureType::new("derivedMirror", "Derived mirror", Category::Import, p).hidden().describe("Mirrored copy of parts from another Part Studio")
}

/// Created when a CAD file is imported; not a toolbar tool.
pub fn import_foreign() -> FeatureType {
    let p = vec![
        hidden(ParamSpec::bool("dependsOnBlob", "Depends on blob", false)),
        hidden(ParamSpec::text("blobData", "Blob data", "")),
        // The file's parts (JSON list of [`ImportPartInfo`], one per child of
        // the compound in `blobData`): names and appearances.
        hidden(ParamSpec::text("partInfo", "Part info", "")),
        hidden(ParamSpec::text("foreignId", "Foreign id", "")),
        ParamSpec::bool("importAppearances", "Import appearances", true),
        hidden(ParamSpec::bool("specifyMaterialData", "Specify material data", false)),
        ParamSpec::bool("importMaterialDensity", "Import material density", false).visible(|v| v.bool("specifyMaterialData")),
        ParamSpec::bool("yAxisIsUp", "Source is 'Y Axis Up'", false),
        ParamSpec::bool("allowFaultyParts", "Allow faulty parts", false),
        hidden(ParamSpec::bool("specifyUnits", "Specify units", false)),
        ParamSpec::enum_("unit", "Unit", LENGTH_UNIT_NAMES).visible(|v| v.bool("specifyUnits")),
        hidden(ParamSpec::enum_("originalUnit", "Original unit", LENGTH_UNIT_NAMES)),
        hidden(ParamSpec::bool("flatten", "Flatten assembly", false)),
        hidden(ParamSpec::num("maxAssembliesToCreate", "Maximum number of assemblies created", NumSpec::integer(10, 1, 100_000))),
        hidden(ParamSpec::bool("isInContext", "Is in context", false)),
        ParamSpec::bool("createComposite", "Create composite", false),
        // Import modal › "Join adjacent surfaces": sew surfaces sharing edges.
        hidden(ParamSpec::bool("joinAdjacentSurfaces", "Join adjacent surfaces", false)),
        hidden(ParamSpec::text("compositeName", "Composite name", "")),
        hidden(ParamSpec::bool("fixSingleCompositeEmptyName", "Fix single composite empty name", false)),
    ];
    FeatureType::new("importForeign", "Import", Category::Import, p).hidden().describe("Geometry imported from a CAD file")
}

/// What an imported file says about one of its parts: its name, its colour
/// and its faces' own colours (sRGB 0..1; face index in the part shape's
/// face order). Stored as JSON in the Import feature's `partInfo`.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ImportPartInfo {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<[f32; 3]>,
    /// Opacity 0..1 when translucent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opacity: Option<f32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub faces: Vec<(usize, [f32; 3])>,
}
