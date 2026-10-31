//! Core feature schemas: Sketch, Extrude, Fillet, Plane, Variable.
//! These are the reference implementations of the schema pattern; parameter
//! ids, labels, defaults and visibility follow Onshape's FeatureScript
//! definitions (see research/sections/C-features-core.md).

use super::common::*;
use super::{Category, FeatureType};
use crate::params::{Hints, NumSpec, ParamSpec, ParamValues, QueryFilter};

pub fn register(out: &mut Vec<FeatureType>) {
    out.push(sketch());
    out.push(extrude());
    out.push(fillet());
    out.push(plane());
    out.push(variable());
}

pub fn sketch() -> FeatureType {
    FeatureType::new(
        "sketch",
        "Sketch",
        Category::Sketch,
        vec![
            ParamSpec::query1("sketchPlane", "Sketch plane", QueryFilter::PLANE_LIKE | QueryFilter::SKETCH_REGION).hints(Hints::INITIAL_FOCUS),
            ParamSpec::bool("disableImprinting", "Disable imprinting", false).hints(Hints::REMEMBER_PREVIOUS_VALUE),
            ParamSpec::sketch("sketch"),
        ],
    )
    .shortcut("shift-s")
    .describe("Create a 2D sketch on a plane or planar face")
}

pub fn extrude() -> FeatureType {
    // extrude.fs: the viewer it is made in (the sheet metal flat view has
    // its own Creation type, Add or Remove).
    let mut p = vec![ParamSpec::enum_("domain", "Domain", &[("MODEL", "MODEL"), ("FLAT", "FLAT")]).hints(Hints::ALWAYS_HIDDEN)];
    p.extend(creation_and_operation(true));
    p.push(
        ParamSpec::enum_("flatOperationType", "Creation type", &[("ADD", "Add"), ("REMOVE", "Remove")])
            .default_enum("REMOVE")
            .hints(Hints::HORIZONTAL_ENUM)
            .visible(|v| v.is("domain", "FLAT")),
    );
    p.push(
        ParamSpec::query("entities", "Faces and sketch regions to extrude", QueryFilter::SKETCH_REGION | QueryFilter::PLANAR_FACE)
            .hints(Hints::INITIAL_FOCUS)
            .visible(|v| v.is("bodyType", "SOLID")),
    );
    p.push(ParamSpec::query("surfaceEntities", "Sketch curves to extrude", QueryFilter::SKETCH_EDGE).visible(|v| v.is("bodyType", "SURFACE")));
    p.push(
        ParamSpec::query("wallShape", "Faces and sketch regions to extrude", QueryFilter::SKETCH_REGION | QueryFilter::PLANAR_FACE | QueryFilter::SKETCH_EDGE)
            .visible(|v| v.is("bodyType", "THIN")),
    );
    p.extend(thin_block());
    p.extend(end_condition("", false));

    p.push(ParamSpec::bool("hasExtrudeDirection", "Direction", false).hints(Hints::GROUP_HEADER).visible(|v| !v.is("bodyType", "THIN")));
    p.push(
        ParamSpec::query1("extrudeDirection", "Extrude direction", QueryFilter::DIRECTION)
            .visible(|v| !v.is("bodyType", "THIN") && v.bool("hasExtrudeDirection")),
    );

    p.push(ParamSpec::bool("startOffset", "Starting offset", false).hints(Hints::GROUP_HEADER));
    p.push(ParamSpec::enum_("startOffsetBound", "Starting offset bound", &[("BLIND", "Blind"), ("ENTITY", "Entity")]).visible(|v| v.bool("startOffset")));
    p.push(ParamSpec::length("startOffsetDistance", "Depth", 25.0, 1.0).visible(|v| v.bool("startOffset") && v.is("startOffsetBound", "BLIND")));
    p.push(ParamSpec::flip("startOffsetOppositeDirection", "Opposite direction").visible(|v| v.bool("startOffset") && v.is("startOffsetBound", "BLIND")));
    p.push(
        ParamSpec::query1("startOffsetEntity", "Entity", QueryFilter::PLANE_LIKE | QueryFilter::POINT_LIKE | QueryFilter::ANY_EDGE)
            .visible(|v| v.bool("startOffset") && v.is("startOffsetBound", "ENTITY")),
    );

    let sym_ok = |v: &ParamValues| v.is("endBound", "BLIND") || v.is("endBound", "THROUGH_ALL");
    p.push(ParamSpec::bool("symmetric", "Symmetric", false).visible(sym_ok));

    let draft_ok = |v: &ParamValues| v.is("bodyType", "SOLID") || v.is("bodyType", "THIN");
    p.push(ParamSpec::bool("hasDraft", "Draft", false).hints(Hints::DISPLAY_SHORT | Hints::FIRST_IN_ROW).visible(draft_ok));
    p.push(
        ParamSpec::num("draftAngle", "Draft angle", NumSpec::angle(3.0, 0.0, 89.9))
            .hints(Hints::DISPLAY_SHORT)
            .visible(move |v| draft_ok(v) && v.bool("hasDraft")),
    );
    p.push(
        ParamSpec::bool("draftPullDirection", "Opposite direction", false)
            .hints(Hints::OPPOSITE_DIRECTION_CIRCULAR)
            .visible(move |v| draft_ok(v) && v.bool("hasDraft")),
    );

    p.push(
        ParamSpec::bool("hasSecondDirection", "Second end position", false)
            .hints(Hints::FIRST_IN_ROW | Hints::GROUP_HEADER)
            .visible(move |v| !(v.bool("symmetric") && sym_ok(v))),
    );
    p.extend(end_condition("secondDirection", true));
    let second_away = |v: &ParamValues| v.bool("secondDirectionOppositeDirection") != v.bool("oppositeDirection");
    p.push(
        ParamSpec::bool("hasSecondDirectionDraft", "Draft", false)
            .hints(Hints::DISPLAY_SHORT | Hints::FIRST_IN_ROW)
            .visible(move |v| second_direction_on(v) && draft_ok(v) && second_away(v)),
    );
    p.push(
        ParamSpec::num("secondDirectionDraftAngle", "Draft angle", NumSpec::angle(3.0, 0.0, 89.9))
            .hints(Hints::DISPLAY_SHORT)
            .visible(move |v| second_direction_on(v) && v.bool("hasSecondDirectionDraft") && draft_ok(v) && second_away(v)),
    );
    p.push(
        ParamSpec::bool("secondDirectionDraftPullDirection", "Opposite direction", false)
            .hints(Hints::OPPOSITE_DIRECTION_CIRCULAR)
            .visible(move |v| second_direction_on(v) && v.bool("hasSecondDirectionDraft") && draft_ok(v) && second_away(v)),
    );
    p.extend(merge_scope());

    FeatureType::new("extrude", "Extrude", Category::Solid, p)
        .shortcut("shift-e")
        .describe("Add or remove material perpendicular to a sketch region or planar face")
}

/// Fillet radii and widths (valueBounds.fs `BLEND_BOUNDS`: above 0.00001 m).
const BLEND: NumSpec = NumSpec::positive_length(5.0, 0.2).bounded(1e-5, 500.0);

/// Variable fillet radii may be zero (fillet.fs `VR_BLEND_BOUNDS`).
const VR_BLEND: NumSpec = NumSpec { kind: crate::params::NumKind::Length, default_metric: 5.0e-3, default_imperial: 0.2 * 0.0254, min: 0.0, max: 500.0 };

pub fn fillet() -> FeatureType {
    // Onshape's fillet.fs (docs-raw/fs-std): ids, labels, conditions and
    // defaults as there; layout checked against live dialogs/fillet-01..14.
    let edge = |v: &ParamValues| v.is("filletType", "EDGE");
    let radius_ctl = move |v: &ParamValues| edge(v) && v.is("blendControlType", "RADIUS");
    let partial = move |v: &ParamValues| radius_ctl(v) && v.bool("isPartial");
    let second = move |v: &ParamValues| partial(v) && v.bool("secondBound");
    let variable = move |v: &ParamValues| radius_ctl(v) && v.bool("isVariable");
    // fillet.fs: the overflow and corner options sit inside the Radius block.
    let not_curvature = move |v: &ParamValues| radius_ctl(v) && !v.is("crossSection", "CURVATURE");
    const END_TYPES: &[(&str, &str)] = &[("PERCENTAGE", "Position"), ("ENTITY", "Entity"), ("OFFSET", "Offset")];
    let p = vec![
        ParamSpec::enum_("filletType", "Fillet type", &[("EDGE", "Edge"), ("FULL_ROUND", "Full round")]).hints(Hints::HORIZONTAL_ENUM),
        ParamSpec::query("entities", "Entities to fillet", QueryFilter::EDGE | QueryFilter::ANY_FACE).hints(Hints::INITIAL_FOCUS).visible(edge),
        ParamSpec::query1("side1Face", "First side face", QueryFilter::ANY_FACE).visible(|v| v.is("filletType", "FULL_ROUND")),
        ParamSpec::query1("side2Face", "Second side face", QueryFilter::ANY_FACE).visible(|v| v.is("filletType", "FULL_ROUND")),
        ParamSpec::query("centerFaces", "Faces to round", QueryFilter::ANY_FACE).visible(|v| v.is("filletType", "FULL_ROUND")),
        ParamSpec::bool("tangentPropagation", "Tangent propagation", true),
        ParamSpec::enum_("blendControlType", "Measurement", &[("RADIUS", "Radius"), ("WIDTH", "Width")]).hints(Hints::REMEMBER_PREVIOUS_VALUE | Hints::SHOW_LABEL).visible(edge),
        ParamSpec::enum_("crossSection", "Control", &[("CIRCULAR", "Distance"), ("CONIC", "Conic"), ("CURVATURE", "Curvature")])
            .hints(Hints::REMEMBER_PREVIOUS_VALUE | Hints::SHOW_LABEL)
            .describe("Cross sectional control")
            .visible(edge),
        ParamSpec::num("radius", "Radius", BLEND)
            .hints(Hints::REMEMBER_PREVIOUS_VALUE | Hints::CAN_BE_TOLERANT)
            .visible(move |v| radius_ctl(v) && v.is("crossSection", "CIRCULAR")),
        ParamSpec::num("nonCircularRadius", "Radius", BLEND)
            .hints(Hints::REMEMBER_PREVIOUS_VALUE)
            .visible(move |v| radius_ctl(v) && !v.is("crossSection", "CIRCULAR")),
        ParamSpec::num("width", "Width", BLEND).hints(Hints::REMEMBER_PREVIOUS_VALUE).visible(move |v| edge(v) && v.is("blendControlType", "WIDTH")),
        ParamSpec::num("rho", "Rho", NumSpec::real(0.5, 0.0, 0.99999)).visible(move |v| edge(v) && v.is("crossSection", "CONIC")),
        ParamSpec::num("magnitude", "Magnitude", NumSpec::real(0.5, 0.0, 0.99999)).visible(move |v| edge(v) && v.is("crossSection", "CURVATURE")),
        ParamSpec::bool("isAsymmetric", "Asymmetric", false).visible(radius_ctl),
        ParamSpec::num("otherRadius", "Second radius", BLEND).hints(Hints::REMEMBER_PREVIOUS_VALUE).visible(move |v| radius_ctl(v) && v.bool("isAsymmetric")),
        ParamSpec::flip("flipAsymmetric", "Flip asymmetric").visible(move |v| radius_ctl(v) && v.bool("isAsymmetric")),
        ParamSpec::bool("isPartial", "Partial fillet", false).hints(Hints::GROUP_HEADER).visible(radius_ctl),
        ParamSpec::enum_("startPartialType", "End type", END_TYPES).hints(Hints::SHOW_LABEL | Hints::REMEMBER_PREVIOUS_VALUE).visible(partial),
        ParamSpec::num("startPartialOffset", "Start offset", VR_BLEND).visible(move |v| partial(v) && v.is("startPartialType", "OFFSET")),
        ParamSpec::query1("startPartialEntity", "Select an entity or mate connector", QueryFilter::ANY_FACE | QueryFilter::POINT_LIKE).visible(move |v| partial(v) && v.is("startPartialType", "ENTITY")),
        ParamSpec::num("partialFirstEdgeTotalParameter", "Start position", NumSpec::real(0.01, 0.0, 1.0)).visible(move |v| partial(v) && v.is("startPartialType", "PERCENTAGE")),
        ParamSpec::bool("partialOppositeParameter", "Opposite direction", true).hints(Hints::OPPOSITE_DIRECTION | Hints::DISPLAY_SHORT).visible(partial),
        ParamSpec::bool("useTrimmedFirstBound", "Trim to face boundaries", false).visible(move |v| partial(v) && v.is("startPartialType", "ENTITY")),
        ParamSpec::bool("secondBound", "Second bound", false).visible(partial),
        ParamSpec::enum_("endPartialType", "End type", END_TYPES).hints(Hints::SHOW_LABEL | Hints::REMEMBER_PREVIOUS_VALUE).visible(second),
        ParamSpec::num("endPartialOffset", "End offset", VR_BLEND).visible(move |v| second(v) && v.is("endPartialType", "OFFSET")),
        ParamSpec::query1("endPartialEntity", "Select an entity or mate connector", QueryFilter::ANY_FACE | QueryFilter::POINT_LIKE).visible(move |v| second(v) && v.is("endPartialType", "ENTITY")),
        ParamSpec::bool("useTrimmedSecondBound", "Trim to face boundaries", false).visible(move |v| second(v) && v.is("endPartialType", "ENTITY")),
        ParamSpec::num("partialSecondEdgeTotalParameter", "End position", NumSpec::real(0.99, 0.0, 1.0)).visible(move |v| second(v) && v.is("endPartialType", "PERCENTAGE")),
        ParamSpec::bool("isVariable", "Variable fillet", false).hints(Hints::GROUP_HEADER).visible(radius_ctl),
        // Items see the feature's values too (isAsymmetric, crossSection), as
        // in FeatureScript's loops over `definition.vertexSettings`.
        ParamSpec::array(
            "vertexSettings",
            "Vertices",
            "vertex",
            vec![
                // fillet.fs: the vertex is picked in the array's field only.
                ParamSpec::query1("vertex", "Vertex", QueryFilter::VERTEX).hints(Hints::ALWAYS_HIDDEN),
                ParamSpec::num("vertexRadius", "Radius", VR_BLEND).hints(Hints::MATCH_LAST_ARRAY_ITEM),
                ParamSpec::num("vertexOtherRadius", "Second radius", BLEND).hints(Hints::MATCH_LAST_ARRAY_ITEM).visible(|v| v.bool("isAsymmetric")),
                ParamSpec::flip("vertexFlipAsymmetric", "Flip asymmetric").hints(Hints::MATCH_LAST_ARRAY_ITEM).visible(|v| v.bool("isAsymmetric")),
                ParamSpec::num("variableRho", "Rho", NumSpec::real(0.5, 0.0, 0.99999)).hints(Hints::MATCH_LAST_ARRAY_ITEM).visible(|v| v.is("crossSection", "CONIC")),
                ParamSpec::num("variableMagnitude", "Magnitude", NumSpec::real(0.5, 0.0, 0.99999)).hints(Hints::MATCH_LAST_ARRAY_ITEM).visible(|v| v.is("crossSection", "CURVATURE")),
            ],
        )
        .hints(Hints::DRIVEN_QUERY | Hints::PREVENT_ARRAY_REORDER)
        .item_template("[#vertexRadius] #vertex")
        .visible(variable),
        ParamSpec::array(
            "pointOnEdgeSettings",
            "Points on edges",
            "point on edge",
            vec![
                ParamSpec::query1("edge", "Edge", QueryFilter::EDGE),
                // EDGE_INTERIOR_PARAMETER_BOUNDS_SPEC
                ParamSpec::num("edgeParameter", "Location", NumSpec::real(0.5, 0.001, 0.999)),
                ParamSpec::num("pointOnEdgeRadius", "Radius", VR_BLEND).hints(Hints::MATCH_LAST_ARRAY_ITEM),
                ParamSpec::num("pointOnEdgeOtherRadius", "Second radius", BLEND).hints(Hints::MATCH_LAST_ARRAY_ITEM).visible(|v| v.bool("isAsymmetric")),
                ParamSpec::flip("pointOnEdgeFlipAsymmetric", "Flip asymmetric").hints(Hints::MATCH_LAST_ARRAY_ITEM).visible(|v| v.bool("isAsymmetric")),
                ParamSpec::num("pointOnEdgeVariableRho", "Rho", NumSpec::real(0.5, 0.0, 0.99999)).hints(Hints::MATCH_LAST_ARRAY_ITEM).visible(|v| v.is("crossSection", "CONIC")),
                ParamSpec::num("pointOnEdgeVariableMagnitude", "Magnitude", NumSpec::real(0.5, 0.0, 0.99999)).hints(Hints::MATCH_LAST_ARRAY_ITEM).visible(|v| v.is("crossSection", "CURVATURE")),
            ],
        )
        .item_template("[#pointOnEdgeRadius] #edge")
        .hints(Hints::PREVENT_ARRAY_REORDER)
        .visible(variable),
        ParamSpec::bool("smoothTransition", "Smooth transition", false).visible(variable),
        ParamSpec::bool("allowEdgeOverflow", "Allow edge overflow", true).hints(Hints::GROUP_HEADER).visible(not_curvature),
        ParamSpec::query("keepEdges", "Edges to keep", QueryFilter::EDGE).visible(move |v| not_curvature(v) && v.bool("allowEdgeOverflow")),
        ParamSpec::bool("smoothCorners", "Smooth fillet corners", false).hints(Hints::GROUP_HEADER).visible(not_curvature),
        ParamSpec::query("smoothCornerExceptions", "Corners to exclude", QueryFilter::VERTEX).visible(move |v| not_curvature(v) && v.bool("smoothCorners")),
    ];
    FeatureType::new("fillet", "Fillet", Category::Detail, p)
        .shortcut("shift-f")
        .describe("Round sharp interior and exterior edges")
}

pub fn plane() -> FeatureType {
    // Onshape's cplane.fs (CPlaneType and the cPlane precondition).
    const TYPES: &[(&str, &str)] = &[
        ("OFFSET", "Offset"),
        ("PLANE_POINT", "Plane point"),
        ("LINE_ANGLE", "Line angle"),
        ("LINE_POINT", "Point normal"),
        ("THREE_POINT", "Three point"),
        ("MID_PLANE", "Mid plane"),
        ("CURVE_POINT", "Curve point"),
        ("TANGENT_PLANE", "Tangent"),
    ];
    let p = vec![
        ParamSpec::query("entities", "Entities", QueryFilter::PLANE_LIKE | QueryFilter::SKETCH_REGION | QueryFilter::POINT_LIKE | QueryFilter::ANY_EDGE | QueryFilter::CYLINDRICAL_FACE)
            .hints(Hints::INITIAL_FOCUS | Hints::NO_MATE_CONNECTOR_BUTTON),
        // No SHOW_LABEL: a full-width select with "Plane type" as its tooltip (live).
        ParamSpec::enum_("cplaneType", "Plane type", TYPES).hints(Hints::REMEMBER_PREVIOUS_VALUE),
        ParamSpec::num("offset", "Offset distance", NumSpec::positive_length(25.0, 1.0)).visible(|v| v.is("cplaneType", "OFFSET")),
        ParamSpec::num("angle", "Angle", NumSpec::angle(0.0, -1e5, 1e5)).visible(|v| v.is("cplaneType", "LINE_ANGLE")),
        ParamSpec::flip("oppositeDirection", "Opposite direction").visible(|v| v.is("cplaneType", "OFFSET") || v.is("cplaneType", "LINE_ANGLE")),
        ParamSpec::bool("flipAlignment", "Flip alignment", false).visible(|v| v.is("cplaneType", "MID_PLANE") || v.is("cplaneType", "TANGENT_PLANE")),
        ParamSpec::bool("flipNormal", "Flip normal", false),
        ParamSpec::num("width", "Starting width", NumSpec::length(150.0, 6.0).bounded(1e-5, 500.0)).hints(Hints::UNCONFIGURABLE | Hints::ALWAYS_HIDDEN),
        ParamSpec::num("height", "Starting height", NumSpec::length(150.0, 6.0).bounded(1e-5, 500.0)).hints(Hints::UNCONFIGURABLE | Hints::ALWAYS_HIDDEN),
    ];
    FeatureType::new("cPlane", "Plane", Category::Reference, p).icon("plane").describe("Create a construction plane")
}

/// Upgrade Plane parameters saved before the cplane.fs port (no
/// `flipNormal`): `flipAlignment` meant "Flip normal", the types
/// POINT_NORMAL and TANGENT_CYLINDER are now LINE_POINT and TANGENT_PLANE
/// (whose other tangent side was `oppositeDirection`, now `flipAlignment`).
pub fn upgrade_plane_params(params: &mut ParamValues) {
    use crate::params::ParamValue;
    if params.get("flipNormal").is_some() {
        return;
    }
    params.set("flipNormal", ParamValue::Bool(params.bool("flipAlignment")));
    let mut flip_alignment = false;
    match params.enum_("cplaneType") {
        "POINT_NORMAL" => params.set("cplaneType", ParamValue::Enum("LINE_POINT".into())),
        "TANGENT_CYLINDER" => {
            flip_alignment = params.bool("oppositeDirection");
            params.set("oppositeDirection", ParamValue::Bool(false));
            params.set("cplaneType", ParamValue::Enum("TANGENT_PLANE".into()));
        }
        _ => {}
    }
    params.set("flipAlignment", ParamValue::Bool(flip_alignment));
}

pub fn variable() -> FeatureType {
    // Onshape's variable.fs: Assigned | Measured | From import; ids, labels
    // and defaults as there (values default to 0).
    const MODES: &[(&str, &str)] = &[("ASSIGNED", "Assigned"), ("MEASURED", "Measured"), ("TABLE", "From import")];
    const KINDS: &[(&str, &str)] = &[("LENGTH", "Length"), ("ANGLE", "Angle"), ("NUMBER", "Number"), ("ANY", "Any")];
    const MEASURE: &[(&str, &str)] = &[("DISTANCE", "Distance"), ("LENGTH", "Length"), ("DIAMETER", "Diameter")];
    const MINMAX: &[(&str, &str)] = &[("MINIMUM", "Minimum"), ("MAXIMUM", "Maximum")];
    const AXIS: &[(&str, &str)] = &[("DISTANCE", "Distance"), ("X", "Along X"), ("Y", "Along Y"), ("Z", "Along Z"), ("CUSTOM", "Along custom")];
    const ACCESS: &[(&str, &str)] = &[("INDEX", "By index"), ("INDEX_RANGE", "By index range"), ("LABEL", "By label"), ("ALL", "All values")];
    const RESULT: &[(&str, &str)] = &[("SINGLE", "Single value"), ("ARRAY", "Array"), ("MAP", "Map")];
    let assigned = |v: &ParamValues| v.is("mode", "ASSIGNED");
    let measured = |v: &ParamValues, m: &str| v.is("mode", "MEASURED") && v.is("measurementMode", m);
    let distance = move |v: &ParamValues| measured(v, "DISTANCE");
    let table = |v: &ParamValues| v.is("mode", "TABLE");
    let zero = NumSpec::length(0.0, 0.0);
    let p = vec![
        ParamSpec::query("initEntities", "Entities", QueryFilter::ANY_FACE | QueryFilter::ANY_EDGE | QueryFilter::VERTEX | QueryFilter::BODY | QueryFilter::MATE_CONNECTOR).hints(Hints::ALWAYS_HIDDEN),
        ParamSpec::enum_("mode", "Mode", MODES).hints(Hints::HORIZONTAL_ENUM),
        ParamSpec::enum_("variableType", "Variable type", KINDS).hints(Hints::HORIZONTAL_ENUM | Hints::UNCONFIGURABLE).visible(assigned),
        ParamSpec::enum_("measurementMode", "Measurement", MEASURE).hints(Hints::HORIZONTAL_ENUM).visible(|v| v.is("mode", "MEASURED")),
        ParamSpec::text("name", "Name", "").hints(Hints::UNCONFIGURABLE | Hints::INITIAL_FOCUS),
        ParamSpec::num("lengthValue", "Value", zero).visible(move |v| assigned(v) && v.is("variableType", "LENGTH")),
        ParamSpec::angle("angleValue", "Value", 0.0).visible(move |v| assigned(v) && v.is("variableType", "ANGLE")),
        ParamSpec::num("numberValue", "Value", NumSpec::real(0.0, -1e12, 1e12)).visible(move |v| assigned(v) && v.is("variableType", "NUMBER")),
        ParamSpec::text("anyValue", "Value", "0").visible(move |v| assigned(v) && v.is("variableType", "ANY")),
        // From import: a CSV file tab and which cells to read.
        ParamSpec::text("csv", "CSV file", "").visible(table),
        ParamSpec::enum_("rowAccessType", "Access type", ACCESS).visible(table),
        ParamSpec::num("rowIndex", "Index (0-based)", NumSpec::integer(0, 0, 1_000_000)).visible(move |v| table(v) && v.is("rowAccessType", "INDEX")),
        ParamSpec::text("rowValue", "Value", "").visible(move |v| table(v) && v.is("rowAccessType", "LABEL")),
        ParamSpec::bool("rowRegexp", "Regular expression", false).visible(move |v| table(v) && v.is("rowAccessType", "LABEL")),
        ParamSpec::num("rowMinIndex", "Min index (0-based)", NumSpec::integer(0, 0, 1_000_000)).visible(move |v| table(v) && v.is("rowAccessType", "INDEX_RANGE")),
        ParamSpec::num("rowMaxIndex", "Max index (0-based)", NumSpec::integer(0, 0, 1_000_000)).visible(move |v| table(v) && v.is("rowAccessType", "INDEX_RANGE")),
        ParamSpec::enum_("rowResult", "Result type", RESULT).hints(Hints::SHOW_LABEL).visible(table),
        ParamSpec::num("rowLabelIndex", "Label column index", NumSpec::integer(0, 0, 1_000_000)).visible(move |v| table(v) && (v.is("rowAccessType", "LABEL") || v.is("rowResult", "MAP"))),
        ParamSpec::enum_("columnAccessType", "Access type", ACCESS).visible(table),
        ParamSpec::num("columnIndex", "Index (0-based)", NumSpec::integer(0, 0, 1_000_000)).visible(move |v| table(v) && v.is("columnAccessType", "INDEX")),
        ParamSpec::text("columnValue", "Value", "").visible(move |v| table(v) && v.is("columnAccessType", "LABEL")),
        ParamSpec::bool("columnRegexp", "Regular expression", false).visible(move |v| table(v) && v.is("columnAccessType", "LABEL")),
        ParamSpec::num("columnMinIndex", "Min index (0-based)", NumSpec::integer(0, 0, 1_000_000)).visible(move |v| table(v) && v.is("columnAccessType", "INDEX_RANGE")),
        ParamSpec::num("columnMaxIndex", "Max index (0-based)", NumSpec::integer(0, 0, 1_000_000)).visible(move |v| table(v) && v.is("columnAccessType", "INDEX_RANGE")),
        ParamSpec::enum_("columnResult", "Result type", RESULT).hints(Hints::SHOW_LABEL).visible(table),
        ParamSpec::num("columnLabelIndex", "Label row index", NumSpec::integer(0, 0, 1_000_000)).visible(move |v| table(v) && (v.is("columnAccessType", "LABEL") || v.is("columnResult", "MAP"))),
        ParamSpec::bool("useConversionFactor", "Conversion factor", false).hints(Hints::DISPLAY_SHORT | Hints::FIRST_IN_ROW).visible(table),
        ParamSpec::text("conversionFactor", "Conversion factor", "1").hints(Hints::DISPLAY_SHORT).visible(move |v| table(v) && v.bool("useConversionFactor")),
        // Measured.
        // std variable.fs: two picks (then the focus moves on).
        query_n("entityCouple", "Entities to measure between", QueryFilter::ANY_FACE | QueryFilter::ANY_EDGE | QueryFilter::POINT_LIKE | QueryFilter::BODY | QueryFilter::PLANE, 2).visible(distance),
        ParamSpec::enum_("minmax", "Minimum/Maximum", MINMAX).hints(Hints::HORIZONTAL_ENUM).visible(distance),
        ParamSpec::bool("extendEntities", "Extend entities", false).visible(move |v| distance(v) && !v.is("minmax", "MAXIMUM")),
        ParamSpec::bool("measureFromAxis", "Use axis", false).visible(move |v| distance(v) && !v.is("minmax", "MAXIMUM")),
        ParamSpec::bool("axisMeasurements", "Axis measurements", false).hints(Hints::GROUP_HEADER | Hints::GROUP_LABEL | Hints::UNCONFIGURABLE).visible(distance),
        ParamSpec::num("distance", "Distance", zero).hints(Hints::READ_ONLY).visible(move |v| distance(v) && v.bool("axisMeasurements")),
        ParamSpec::num("xOffset", "Along X", zero).hints(Hints::READ_ONLY).visible(move |v| distance(v) && v.bool("axisMeasurements")),
        ParamSpec::num("yOffset", "Along Y", zero).hints(Hints::READ_ONLY).visible(move |v| distance(v) && v.bool("axisMeasurements")),
        ParamSpec::num("zOffset", "Along Z", zero).hints(Hints::READ_ONLY).visible(move |v| distance(v) && v.bool("axisMeasurements")),
        ParamSpec::enum_("componentSelector", "Value to record", AXIS).hints(Hints::SHOW_LABEL).visible(move |v| distance(v) && v.bool("axisMeasurements")),
        ParamSpec::query1("customDirection", "Custom direction", QueryFilter::DIRECTION).visible(move |v| distance(v) && v.bool("axisMeasurements") && v.is("componentSelector", "CUSTOM")),
        ParamSpec::num("customOffset", "Along custom", zero).hints(Hints::READ_ONLY).visible(move |v| distance(v) && v.bool("axisMeasurements") && v.is("componentSelector", "CUSTOM")),
        ParamSpec::query("lengthEntities", "Entities", QueryFilter::ANY_EDGE | QueryFilter::WIRE_BODY).visible(move |v| measured(v, "LENGTH")),
        ParamSpec::bool("radius", "Radius", false).visible(move |v| measured(v, "DIAMETER")),
        ParamSpec::query1("diameterEntity", "Entity", QueryFilter::CIRCULAR_EDGE | QueryFilter::SKETCH_EDGE | QueryFilter::CYLINDRICAL_FACE | QueryFilter::ANY_FACE).visible(move |v| measured(v, "DIAMETER")),
        ParamSpec::text("description", "Description", ""),
    ];
    FeatureType::new("assignVariable", "Variable", Category::Variable, p).icon("variable").describe("Define a variable usable in expressions as #name")
}
