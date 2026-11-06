//! Reference geometry and miscellaneous Part Studio schemas: Mate connector,
//! Point, Name entity, Composite part, Query variable
//! (03b RG-03..06, 03c PSM-24, VAR-13). Plane and Variable are in `core.rs`.

use super::common::*;
use super::pattern::MATE_CONNECTOR_AXIS;
use super::{Category, FeatureType};
use crate::params::{Hints, NumSpec, ParamSpec, ParamValues, QueryFilter};

pub fn register(out: &mut Vec<FeatureType>) {
    out.push(mate_connector());
    out.push(point());
    out.push(name_entity());
    out.push(composite_part());
    out.push(query_variable());
}

pub fn mate_connector() -> FeatureType {
    const INFERENCE: &[(&str, &str)] = &[
        ("PART_ORIGIN", "Part origin"),
        ("POINT", "Point"),
        ("CENTROID", "Centroid"),
        ("CENTER", "Center"),
        ("MID_POINT", "Mid point"),
        ("TOP_AXIS_POINT", "Top axis point"),
        ("MID_AXIS_POINT", "Mid axis point"),
        ("BOTTOM_AXIS_POINT", "Bottom axis point"),
        ("ORIGIN_X", "Origin X"),
        ("ORIGIN_Y", "Origin Y"),
        ("ORIGIN_Z", "Origin Z"),
        ("LOOP_CENTER", "Loop center"),
        ("VIRTUAL_SHARP", "Virtual sharp"),
    ];
    let mv = |v: &ParamValues| v.bool("transform");
    let owner = |v: &ParamValues| v.bool("allowOwnerEntity") && v.bool("requireOwnerPart");
    let p = vec![
        ParamSpec::enum_("originType", "Origin type", &[("ON_ENTITY", "On entity"), ("BETWEEN_ENTITIES", "Between entities")]),
        ParamSpec::query1("originQuery", "Origin entity", QueryFilter::ANY_EDGE | QueryFilter::POINT_LIKE | QueryFilter::ANY_FACE)
            // Live (dom/dialog-mate-connector.txt): a full-width field, no
            // mate connector button.
            .hints(Hints::INITIAL_FOCUS | Hints::UNCONFIGURABLE | Hints::NO_MATE_CONNECTOR_BUTTON),
        hidden(ParamSpec::enum_("entityInferenceType", "Inference type", INFERENCE)),
        hidden(ParamSpec::query("secondaryOriginQuery", "Secondary origin", QueryFilter::ANY_EDGE | QueryFilter::POINT_LIKE | QueryFilter::ANY_FACE)),
        ParamSpec::query1("originAdditionalQuery", "Between entity", QueryFilter::ANY_FACE).hints(Hints::UNCONFIGURABLE).visible(|v| v.is("originType", "BETWEEN_ENTITIES")),
        ParamSpec::bool("realign", "Realign", false),
        ParamSpec::query1("primaryAxisQuery", "Primary axis entity", QueryFilter::ANY_FACE | QueryFilter::ANY_EDGE).visible(|v| v.bool("realign")),
        ParamSpec::query1("secondaryAxisQuery", "Secondary axis entity", QueryFilter::ANY_FACE | QueryFilter::ANY_EDGE).visible(|v| v.bool("realign")),
        // Live (dom/dialog-mate-connector.txt): a plain checkbox like Realign.
        ParamSpec::bool("transform", "Move", false),
        ParamSpec::num("translationX", "X translation", ZERO_DEFAULT_LENGTH).visible(mv),
        ParamSpec::num("translationY", "Y translation", ZERO_DEFAULT_LENGTH).visible(mv),
        ParamSpec::num("translationZ", "Z translation", ZERO_DEFAULT_LENGTH).visible(mv),
        ParamSpec::enum_("rotationType", "Rotation axis", &[("ABOUT_X", "Rotate about X"), ("ABOUT_Y", "Rotate about Y"), ("ABOUT_Z", "Rotate about Z")])
            .default_enum("ABOUT_Z")
            .visible(mv),
        ParamSpec::num("rotation", "Rotation angle", angle_360(0.0)).visible(mv),
        hidden(ParamSpec::bool("allowOwnerEntity", "Allow owner entity", true)),
        ParamSpec::bool("requireOwnerPart", "Owner entity", true).visible(|v| v.bool("allowOwnerEntity")),
        ParamSpec::query1("ownerPart", "Select owner entity", QueryFilter::BODY | QueryFilter::WIRE_BODY | QueryFilter::POINT_BODY).visible(owner),
        ParamSpec::enum_("attachmentOption", "Attachment", &[("NONE", "None"), ("TO_OWNER", "To owner"), ("TO_SELECTION", "To selection")])
            .hints(Hints::SHOW_LABEL)
            .visible(owner),
        ParamSpec::query1("attachTo", "Attach to", QueryFilter::ANY_FACE | QueryFilter::BODY | QueryFilter::MATE_CONNECTOR)
            .visible(move |v| owner(v) && v.is("attachmentOption", "TO_SELECTION")),
        hidden(ParamSpec::bool("specifyNormal", "Specify normal", false)),
        hidden(ParamSpec::num("nx", "Normal x", NumSpec::real(0.0, -1.0, 1.0))),
        hidden(ParamSpec::num("ny", "Normal y", NumSpec::real(0.0, -1.0, 1.0))),
        hidden(ParamSpec::num("nz", "Normal z", NumSpec::real(0.0, -1.0, 1.0))),
        // mateConnector.fs: one row of two icon buttons (live).
        ParamSpec::bool("flipPrimary", "Flip primary axis", false).hints(Hints::PRIMARY_AXIS),
        ParamSpec::enum_("secondaryAxisType", "Reorient secondary axis", MATE_CONNECTOR_AXIS).hints(Hints::MATE_CONNECTOR_AXIS_TYPE),
        hidden(ParamSpec::bool("isForSubFeature", "Is for sub-feature", false)),
    ];
    FeatureType::new("mateConnector", "Mate connector", Category::Reference, p)
        .shortcut("ctrl-m")
        .describe("Create a local coordinate system for mates and feature references")
}

/// Internal construction point (`cPoint`), not exposed in Onshape's UI.
pub fn point() -> FeatureType {
    let edge = |v: &ParamValues| v.is("pointType", "EDGE_POINT");
    let p = vec![
        ParamSpec::enum_("pointType", "Point type", &[("EDGE_POINT", "Edge point"), ("MESH_POINT", "Mesh point")]),
        ParamSpec::query1("entities", "Edge", QueryFilter::ANY_EDGE).visible(edge),
        ParamSpec::num("parameter", "Parameter", EDGE_PARAMETER).visible(edge),
        ParamSpec::query1("mesh", "Mesh", QueryFilter::ANY_FACE).visible(move |v| !edge(v)),
        ParamSpec::length("x", "X", 25.0, 1.0).visible(move |v| !edge(v)),
        ParamSpec::length("y", "Y", 25.0, 1.0).visible(move |v| !edge(v)),
        ParamSpec::length("z", "Z", 25.0, 1.0).visible(move |v| !edge(v)),
    ];
    FeatureType::new("cPoint", "Point", Category::Reference, p).hidden().describe("Construction point on an edge or mesh")
}

/// Internal: assign a persistent name to an entity (`nameEntity`).
pub fn name_entity() -> FeatureType {
    let p = vec![
        ParamSpec::query1("entity", "Entity", QueryFilter::ANY_FACE | QueryFilter::ANY_EDGE | QueryFilter::BODY),
        ParamSpec::text("entityName", "Name", ""),
    ];
    FeatureType::new("nameEntity", "Name entity", Category::Reference, p).hidden().describe("Give an entity a persistent name")
}

pub fn composite_part() -> FeatureType {
    let p = vec![
        ParamSpec::query("bodies", "Entities", QueryFilter::BODY | QueryFilter::WIRE_BODY | QueryFilter::POINT_BODY).hints(Hints::INITIAL_FOCUS),
        ParamSpec::bool("closed", "Closed", false),
    ];
    FeatureType::new("compositePart", "Composite part", Category::Solid, p).describe("Group bodies to act as a single part")
}

pub const SELECTION_TYPE: &[(&str, &str)] = &[
    ("SELECTION", "Selection"),
    ("CREATED_BY", "Created by"),
    ("OWNED_BY", "Owned by"),
    ("PROTRUSION", "Protrusion"),
    ("POCKET", "Pocket"),
    ("HOLE", "Hole"),
    ("FILLETS", "Fillets"),
    ("BOUNDED_FACES", "Bounded faces"),
    ("LOOP_CHAIN_CONNECTED", "Loop/chain connected"),
    ("PARALLEL", "Parallel"),
    ("TANGENT_CONNECTED", "Tangent connected"),
    ("MATCHING", "Matching"),
    ("ALL_SOLID_BODIES", "All solid bodies"),
    ("EDGE_CONVEXITY", "Edge convexity"),
    ("ADJACENT", "Adjacent"),
];

const ANY_ENTITY: QueryFilter = QueryFilter(
    QueryFilter::ANY_FACE.0 | QueryFilter::ANY_EDGE.0 | QueryFilter::POINT_LIKE.0 | QueryFilter::BODY.0 | QueryFilter::WIRE_BODY.0 | QueryFilter::PLANE.0,
);

pub fn query_variable() -> FeatureType {
    let st = |v: &ParamValues, t: &str| v.is("selectionType", t);
    let any = move |v: &ParamValues, ts: &[&str]| ts.iter().any(|t| st(v, t));
    let seeded = move |v: &ParamValues| any(v, &["TANGENT_CONNECTED", "MATCHING"]);
    let face_seed = move |v: &ParamValues| any(v, &["PROTRUSION", "POCKET", "HOLE", "FILLETS", "BOUNDED_FACES"]) || (seeded(v) && v.is("seedType", "FACE"));
    let edge_seed = move |v: &ParamValues| st(v, "PARALLEL") || (seeded(v) && v.is("seedType", "EDGE"));
    let created = move |v: &ParamValues| st(v, "CREATED_BY");
    let adjacent = move |v: &ParamValues| st(v, "ADJACENT");
    let p = vec![
        // QUERY_VARIABLE_NAME: the Name has the keyboard on opening, Selections stays the active field (live ui-parity
        // query-variable-tool), as Variable's.
        ParamSpec::text("name", "Name", "").hints(Hints::UNCONFIGURABLE | Hints::INITIAL_FOCUS),
        ParamSpec::text("description", "Description", ""),
        ParamSpec::enum_("selectionType", "Selection type", SELECTION_TYPE),
        ParamSpec::enum_("seedType", "Entity type", &[("FACE", "Face"), ("EDGE", "Edge")]).visible(seeded),
        ParamSpec::query("selectionQuery", "Selections", ANY_ENTITY).visible(move |v| st(v, "SELECTION")),
        ParamSpec::query("createdByFeatures", "Created by features", QueryFilter::FEATURE).visible(created),
        ParamSpec::query("seedBodies", "Entities", QueryFilter::BODY | QueryFilter::WIRE_BODY).visible(move |v| any(v, &["OWNED_BY", "EDGE_CONVEXITY"])),
        ParamSpec::query("seedFaces", "Faces", QueryFilter::ANY_FACE).visible(face_seed),
        ParamSpec::query("seedEdgesOrFaces", "Edges or faces", QueryFilter::ANY_EDGE | QueryFilter::ANY_FACE).visible(move |v| st(v, "LOOP_CHAIN_CONNECTED")),
        ParamSpec::query("seedEdges", "Edges", QueryFilter::ANY_EDGE).visible(edge_seed),
        ParamSpec::enum_("entityType", "Entity type", &[("VERTEX", "Vertex"), ("EDGE", "Edge"), ("FACE", "Face"), ("BODY", "Body")])
            .visible(move |v| any(v, &["CREATED_BY", "OWNED_BY"])),
        ParamSpec::enum_(
            "filletCompareType",
            "Fillet compare type",
            &[("EQUAL", "Equal"), ("LESS_EQUAL", "Less or equal"), ("GREATER_EQUAL", "Greater or equal")],
        )
        .visible(move |v| st(v, "FILLETS")),
        ParamSpec::query("boundedFacesBounds", "Bounds", QueryFilter::ANY_EDGE | QueryFilter::ANY_FACE).visible(move |v| st(v, "BOUNDED_FACES")),
        ParamSpec::enum_(
            "edgeConvexityType",
            "Edge convexity type",
            &[("CONVEX", "Convex"), ("CONCAVE", "Concave"), ("SMOOTH", "Smooth"), ("VARIABLE", "Variable")],
        )
        .visible(move |v| st(v, "EDGE_CONVEXITY")),
        ParamSpec::bool("filterConstruction", "Filter construction entities", true).visible(created),
        ParamSpec::bool("filterByBodyType", "Filter by body type", false).visible(created),
        ParamSpec::enum_(
            "createdByBodyType",
            "Body type",
            &[
                ("SOLID", "Part"),
                ("SHEET", "Surface"),
                ("WIRE", "Curve"),
                ("POINT", "Point"),
                ("MATE_CONNECTOR", "Mate connector"),
                ("COMPOSITE", "Composite part"),
            ],
        )
        .visible(move |v| created(v) && v.bool("filterByBodyType")),
        ParamSpec::query("seedQuery", "Entities", QueryFilter::ANY_FACE | QueryFilter::EDGE | QueryFilter::VERTEX).visible(adjacent),
        ParamSpec::enum_("adjacencyType", "Shared", &[("VERTEX", "Vertex"), ("EDGE", "Edge")]).hints(Hints::SHOW_LABEL).visible(adjacent),
        ParamSpec::enum_("edgeAdjacentEntityType", "Select", &[("ALL", "Edges and faces"), ("EDGE", "Only edges"), ("FACE", "Only faces")]).hints(Hints::SHOW_LABEL)
            .visible(move |v| adjacent(v) && v.is("adjacencyType", "EDGE")),
        ParamSpec::enum_(
            "vertexAdjacentEntityType",
            "Select",
            &[("ALL", "Edges, faces, and vertices"), ("EDGE", "Only edges"), ("VERTEX", "Only vertices"), ("FACE", "Only faces")],
        ).hints(Hints::SHOW_LABEL)
        .visible(move |v| adjacent(v) && v.is("adjacencyType", "VERTEX")),
        ParamSpec::bool("addAdditionalQueries", "Add additional queries", false).hints(Hints::GROUP_HEADER),
        ParamSpec::array(
            "additionalQueries",
            "Additional queries",
            "additional query",
            // std queryVariable.fs additionalQueryPredicate: the main query's
            // fields again, "addQ"-prefixed.
            {
                let st = |v: &ParamValues, t: &str| v.is("addQselectionType", t);
                let any = move |v: &ParamValues, ts: &[&str]| ts.iter().any(|t| st(v, t));
                let seeded = move |v: &ParamValues| any(v, &["TANGENT_CONNECTED", "MATCHING"]);
                let face_seed = move |v: &ParamValues| any(v, &["PROTRUSION", "POCKET", "HOLE", "FILLETS", "BOUNDED_FACES"]) || (seeded(v) && v.is("addQseedType", "FACE"));
                let edge_seed = move |v: &ParamValues| st(v, "PARALLEL") || (seeded(v) && v.is("addQseedType", "EDGE"));
                let created = move |v: &ParamValues| st(v, "CREATED_BY");
                let adjacent = move |v: &ParamValues| st(v, "ADJACENT");
                vec![
                    ParamSpec::enum_("booleanOperation", "Boolean operation", &[("UNION", "Union"), ("SUBTRACTION", "Subtract"), ("INTERSECTION", "Intersect")])
                        .hints(Hints::HORIZONTAL_ENUM),
                    ParamSpec::enum_("addQselectionType", "Selection type", SELECTION_TYPE),
                    ParamSpec::text("addQlowercaseSelectionType", "Selection type", "").hints(Hints::ALWAYS_HIDDEN),
                    ParamSpec::enum_("addQseedType", "Entity type", &[("FACE", "Face"), ("EDGE", "Edge")]).visible(seeded),
                    ParamSpec::query("addQselectionQuery", "Selections", ANY_ENTITY).visible(move |v| st(v, "SELECTION")),
                    ParamSpec::query("addQcreatedByFeatures", "Created by features", QueryFilter::FEATURE).visible(created),
                    ParamSpec::query("addQseedBodies", "Entities", QueryFilter::BODY | QueryFilter::WIRE_BODY).visible(move |v| any(v, &["OWNED_BY", "EDGE_CONVEXITY"])),
                    ParamSpec::query("addQseedFaces", "Face", QueryFilter::ANY_FACE).visible(face_seed),
                    ParamSpec::query("addQseedEdgesOrFaces", "Edge or face", QueryFilter::ANY_EDGE | QueryFilter::ANY_FACE).visible(move |v| st(v, "LOOP_CHAIN_CONNECTED")),
                    ParamSpec::query("addQseedEdges", "Edge", QueryFilter::ANY_EDGE).visible(edge_seed),
                    ParamSpec::enum_("addQentityType", "Entity type", &[("VERTEX", "Vertex"), ("EDGE", "Edge"), ("FACE", "Face"), ("BODY", "Body")])
                        .visible(move |v| any(v, &["CREATED_BY", "OWNED_BY"])),
                    ParamSpec::enum_("addQfilletCompareType", "Fillet compare type", &[("EQUAL", "Equal"), ("LESS_EQUAL", "Less or equal"), ("GREATER_EQUAL", "Greater or equal")])
                        .visible(move |v| st(v, "FILLETS")),
                    ParamSpec::query("addQboundedFacesBounds", "Bounds", QueryFilter::ANY_EDGE | QueryFilter::ANY_FACE).visible(move |v| st(v, "BOUNDED_FACES")),
                    ParamSpec::enum_("addQedgeConvexityType", "Edge convexity type", &[("CONVEX", "Convex"), ("CONCAVE", "Concave"), ("SMOOTH", "Smooth"), ("VARIABLE", "Variable")])
                        .visible(move |v| st(v, "EDGE_CONVEXITY")),
                    ParamSpec::bool("addQfilterConstruction", "Filter construction entities", true).visible(created),
                    ParamSpec::bool("addQfilterByBodyType", "Filter by body type", false).visible(created),
                    ParamSpec::enum_(
                        "addQcreatedByBodyType",
                        "Body type",
                        &[("SOLID", "Part"), ("SHEET", "Surface"), ("WIRE", "Curve"), ("POINT", "Point"), ("MATE_CONNECTOR", "Mate connector"), ("COMPOSITE", "Composite part")],
                    )
                    .visible(move |v| created(v) && v.bool("addQfilterByBodyType")),
                    ParamSpec::query("addQseedQuery", "Entities", QueryFilter::ANY_FACE | QueryFilter::EDGE | QueryFilter::VERTEX).visible(adjacent),
                    ParamSpec::enum_("addQadjacencyType", "Shared", &[("VERTEX", "Vertex"), ("EDGE", "Edge")]).hints(Hints::SHOW_LABEL).visible(adjacent),
                    ParamSpec::enum_("addQedgeAdjacentEntityType", "Select", &[("ALL", "Edges and faces"), ("EDGE", "Only edges"), ("FACE", "Only faces")])
                        .hints(Hints::SHOW_LABEL)
                        .visible(move |v| adjacent(v) && v.is("addQadjacencyType", "EDGE")),
                    ParamSpec::enum_(
                        "addQvertexAdjacentEntityType",
                        "Select",
                        &[("ALL", "Edges, faces, and vertices"), ("EDGE", "Only edges"), ("VERTEX", "Only vertices"), ("FACE", "Only faces")],
                    )
                    .hints(Hints::SHOW_LABEL)
                    .visible(move |v| adjacent(v) && v.is("addQadjacencyType", "VERTEX")),
                ]
            },
        )
        // addQlowercaseSelectionType: the regeneration's computed value.
        .item_template("#booleanOperation of #addQlowercaseSelectionType")
        .visible(|v| v.bool("addAdditionalQueries")),
        ParamSpec::bool("evaluateOnUse", "Evaluate on use", false),
        ParamSpec::bool("showSelection", "Show selection", true),
    ];
    FeatureType::new("queryVariable", "Query variable", Category::Variable, p).describe("Named, parametric selection reusable in other features")
}
