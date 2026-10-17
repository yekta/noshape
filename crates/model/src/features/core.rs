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
            ParamSpec::query1("sketchPlane", "Sketch plane", QueryFilter::PLANE_LIKE).hints(Hints::INITIAL_FOCUS),
            ParamSpec::bool("disableImprinting", "Disable imprinting", false).hints(Hints::REMEMBER_PREVIOUS_VALUE),
            ParamSpec::sketch("sketch"),
        ],
    )
    .shortcut("shift-s")
    .describe("Create a 2D sketch on a plane or planar face")
}

pub fn extrude() -> FeatureType {
    let mut p = creation_and_operation(true);
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
            .visible(move |v| v.bool("hasSecondDirection") && draft_ok(v) && second_away(v)),
    );
    p.push(
        ParamSpec::num("secondDirectionDraftAngle", "Draft angle", NumSpec::angle(3.0, 0.0, 89.9))
            .hints(Hints::DISPLAY_SHORT)
            .visible(move |v| v.bool("hasSecondDirection") && v.bool("hasSecondDirectionDraft") && draft_ok(v) && second_away(v)),
    );
    p.push(
        ParamSpec::bool("secondDirectionDraftPullDirection", "Opposite direction", false)
            .hints(Hints::OPPOSITE_DIRECTION_CIRCULAR)
            .visible(move |v| v.bool("hasSecondDirection") && v.bool("hasSecondDirectionDraft") && draft_ok(v) && second_away(v)),
    );
    p.extend(merge_scope());

    FeatureType::new("extrude", "Extrude", Category::Solid, p)
        .shortcut("shift-e")
        .describe("Add or remove material perpendicular to a sketch region or planar face")
}

pub fn fillet() -> FeatureType {
    let p = vec![
        ParamSpec::enum_("filletType", "Fillet type", &[("EDGE", "Edge"), ("FULL_ROUND", "Full round")]).hints(Hints::HORIZONTAL_ENUM),
        ParamSpec::query("entities", "Edges or faces to fillet", QueryFilter::EDGE | QueryFilter::ANY_FACE)
            .hints(Hints::INITIAL_FOCUS)
            .visible(|v| v.is("filletType", "EDGE")),
        ParamSpec::query("side1Face", "Side 1 face", QueryFilter::ANY_FACE).visible(|v| v.is("filletType", "FULL_ROUND")),
        ParamSpec::query("side2Face", "Side 2 face", QueryFilter::ANY_FACE).visible(|v| v.is("filletType", "FULL_ROUND")),
        ParamSpec::query("centerFaces", "Center faces", QueryFilter::ANY_FACE).visible(|v| v.is("filletType", "FULL_ROUND")),
        ParamSpec::bool("tangentPropagation", "Tangent propagation", true).visible(|v| v.is("filletType", "EDGE")),
        ParamSpec::enum_("crossSection", "Cross section", &[("CIRCULAR", "Circular"), ("CONIC", "Conic"), ("CURVATURE", "Curvature")])
            .hints(Hints::REMEMBER_PREVIOUS_VALUE | Hints::SHOW_LABEL)
            .visible(|v| v.is("filletType", "EDGE")),
        ParamSpec::enum_("filletMeasurement", "Measurement", &[("RADIUS", "Radius"), ("WIDTH", "Width")])
            .hints(Hints::HORIZONTAL_ENUM | Hints::REMEMBER_PREVIOUS_VALUE)
            .visible(|v| v.is("filletType", "EDGE")),
        ParamSpec::num("radius", "Radius", NumSpec::positive_length(5.0, 0.2))
            .hints(Hints::REMEMBER_PREVIOUS_VALUE | Hints::CAN_BE_TOLERANT)
            .visible(|v| v.is("filletType", "EDGE") && v.is("filletMeasurement", "RADIUS")),
        ParamSpec::num("width", "Width", NumSpec::positive_length(5.0, 0.2))
            .visible(|v| v.is("filletType", "EDGE") && v.is("filletMeasurement", "WIDTH")),
        ParamSpec::bool("asymmetric", "Asymmetric", false).visible(|v| v.is("filletType", "EDGE") && v.is("crossSection", "CIRCULAR")),
        ParamSpec::num("otherRadius", "Second radius", NumSpec::positive_length(5.0, 0.2))
            .visible(|v| v.is("filletType", "EDGE") && v.bool("asymmetric")),
        ParamSpec::flip("flipAsymmetric", "Flip asymmetric").visible(|v| v.is("filletType", "EDGE") && v.bool("asymmetric")),
        ParamSpec::num("rho", "Rho", NumSpec::real(0.5, 0.0, 0.9999)).visible(|v| v.is("filletType", "EDGE") && v.is("crossSection", "CONIC")),
        ParamSpec::num("magnitude", "Magnitude", NumSpec::real(0.5, 0.0, 0.9999))
            .visible(|v| v.is("filletType", "EDGE") && v.is("crossSection", "CURVATURE")),
        ParamSpec::bool("isVariable", "Partial / variable fillet", false).hints(Hints::GROUP_HEADER).visible(|v| v.is("filletType", "EDGE")),
        ParamSpec::array(
            "vertexSettings",
            "Vertex radii",
            "Vertex",
            vec![
                ParamSpec::query1("vertexPoint", "Vertex", QueryFilter::VERTEX),
                ParamSpec::num("vertexRadius", "Radius", NumSpec::positive_length(5.0, 0.2)).hints(Hints::MATCH_LAST_ARRAY_ITEM),
            ],
        )
        .visible(|v| v.is("filletType", "EDGE") && v.bool("isVariable")),
        ParamSpec::bool("allowEdgeOverflow", "Allow edge overflow", true).visible(|v| v.is("filletType", "EDGE")),
    ];
    FeatureType::new("fillet", "Fillet", Category::Detail, p)
        .shortcut("shift-f")
        .describe("Round sharp interior and exterior edges")
}

pub fn plane() -> FeatureType {
    const TYPES: &[(&str, &str)] = &[
        ("OFFSET", "Offset"),
        ("PLANE_POINT", "Plane point"),
        ("LINE_ANGLE", "Line angle"),
        ("POINT_NORMAL", "Point normal"),
        ("THREE_POINT", "Three point"),
        ("MID_PLANE", "Mid plane"),
        ("CURVE_POINT", "Curve point"),
        ("TANGENT_CYLINDER", "Tangent to cylinder"),
    ];
    let p = vec![
        ParamSpec::query("entities", "Entities", QueryFilter::PLANE_LIKE | QueryFilter::POINT_LIKE | QueryFilter::ANY_EDGE | QueryFilter::CYLINDRICAL_FACE)
            .hints(Hints::INITIAL_FOCUS),
        ParamSpec::enum_("cplaneType", "Plane type", TYPES).hints(Hints::REMEMBER_PREVIOUS_VALUE | Hints::SHOW_LABEL),
        ParamSpec::length("offset", "Offset distance", 25.0, 1.0).visible(|v| v.is("cplaneType", "OFFSET")),
        ParamSpec::angle("angle", "Angle", 30.0).visible(|v| v.is("cplaneType", "LINE_ANGLE")),
        ParamSpec::flip("oppositeDirection", "Flip alignment").visible(|v| v.is("cplaneType", "OFFSET") || v.is("cplaneType", "LINE_ANGLE")),
        ParamSpec::bool("flipAlignment", "Flip normal", false),
        ParamSpec::length("width", "Display width", 150.0, 6.0).hints(Hints::UNCONFIGURABLE),
        ParamSpec::length("height", "Display height", 150.0, 6.0).hints(Hints::UNCONFIGURABLE),
    ];
    FeatureType::new("cPlane", "Plane", Category::Reference, p).shortcut("p").icon("plane").describe("Create a construction plane")
}

pub fn variable() -> FeatureType {
    const KINDS: &[(&str, &str)] = &[("LENGTH", "Length"), ("ANGLE", "Angle"), ("NUMBER", "Number"), ("ANY", "Any")];
    let p = vec![
        ParamSpec::enum_("variableType", "Variable type", KINDS).hints(Hints::HORIZONTAL_ENUM),
        ParamSpec::text("name", "Name", "x"),
        ParamSpec::length("lengthValue", "Value", 25.0, 1.0).visible(|v| v.is("variableType", "LENGTH")),
        ParamSpec::angle("angleValue", "Value", 0.0).visible(|v| v.is("variableType", "ANGLE")),
        ParamSpec::num("numberValue", "Value", NumSpec::real(0.0, -1e9, 1e9)).visible(|v| v.is("variableType", "NUMBER")),
        ParamSpec::text("anyValue", "Value", "0").visible(|v| v.is("variableType", "ANY")),
        ParamSpec::text("description", "Description", ""),
    ];
    FeatureType::new("assignVariable", "Variable", Category::Variable, p).icon("variable").describe("Define a variable usable in expressions as #name")
}
