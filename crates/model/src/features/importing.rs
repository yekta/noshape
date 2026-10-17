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
        ParamSpec::query("location", "Locations", QueryFilter::MATE_CONNECTOR).visible(new_ui),
        ParamSpec::enum_("placement", "Placement", &[("AT_ORIGIN", "Base origin"), ("AT_MATE_CONNECTOR", "Base mate connector")]).visible(new_ui),
        hidden(ParamSpec::num("mateConnectorIndex", "Mate connector index", NumSpec::integer(0, 0, 100_000))),
        hidden(ParamSpec::text("mateConnectorId", "Mate connector id", "")),
        hidden(ParamSpec::num("mateConnectorIndexInFeature", "Mate connector index in feature", NumSpec::integer(0, 0, 100_000))),
        ParamSpec::bool("includeMateConnectors", "Include mate connectors", true).visible(new_ui),
        ParamSpec::bool("includeProperties", "Include properties", true).hints(Hints::REMEMBER_PREVIOUS_VALUE).visible(new_ui),
        hidden(ParamSpec::query("parts", "Parts to import", QueryFilter::BODY)),
    ];
    FeatureType::new("importDerived", "Derived", Category::Import, p)
        .icon("derived")
        .describe("Insert parts, sketches or a whole Part Studio from another Part Studio")
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
        hidden(ParamSpec::text("compositeName", "Composite name", "")),
        hidden(ParamSpec::bool("fixSingleCompositeEmptyName", "Fix single composite empty name", false)),
    ];
    FeatureType::new("importForeign", "Import", Category::Import, p).hidden().describe("Geometry imported from a CAD file")
}
