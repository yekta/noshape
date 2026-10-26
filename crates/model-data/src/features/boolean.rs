//! Body-level schemas: Boolean, Split, Delete part (C-features-core.md §17-19).

use super::common::*;
use super::{Category, FeatureType};
use crate::params::{Hints, ParamSpec, ParamValues, QueryFilter};

pub fn register(out: &mut Vec<FeatureType>) {
    out.push(boolean());
    out.push(split());
    out.push(delete_part());
}

pub fn boolean() -> FeatureType {
    let sub = |v: &ParamValues| v.is("operationType", "SUBTRACTION");
    let off = move |v: &ParamValues| sub(v) && v.bool("offset");
    let p = vec![
        ParamSpec::enum_("operationType", "Operation type", &[("UNION", "Union"), ("SUBTRACTION", "Subtract"), ("INTERSECTION", "Intersect")])
            .hints(Hints::HORIZONTAL_ENUM),
        ParamSpec::query("tools", "Tools", QueryFilter::BODY).hints(Hints::ALLOW_QUERY_ORDER | Hints::INITIAL_FOCUS),
        ParamSpec::query("targets", "Targets", QueryFilter::BODY).visible(sub),
        ParamSpec::bool("offset", "Offset", false).visible(sub),
        ParamSpec::bool("offsetAll", "Offset all", false).visible(off),
        ParamSpec::query("entitiesToOffset", "Faces to offset", QueryFilter::ANY_FACE).visible(move |v| off(v) && !v.bool("offsetAll")),
        ParamSpec::num("offsetDistance", "Offset distance", ZERO_INCLUSIVE_OFFSET).visible(off),
        ParamSpec::flip("oppositeDirection", "Opposite direction").visible(off),
        ParamSpec::bool("reFillet", "Reapply fillet", false).visible(off),
        ParamSpec::bool("keepTools", "Keep tools", false).hints(Hints::REMEMBER_PREVIOUS_VALUE),
    ];
    FeatureType::new("booleanBodies", "Boolean", Category::Solid, p).icon("boolean").describe("Union, subtract or intersect parts and surfaces")
}

pub fn split() -> FeatureType {
    let part = |v: &ParamValues| v.is("splitType", "PART");
    let face = |v: &ParamValues| v.is("splitType", "FACE");
    let dir = move |v: &ParamValues| face(v) && v.is("projectionType", "DIRECTION");
    let mut p = vec![
        ParamSpec::enum_("splitType", "Split type", &[("PART", "Part"), ("FACE", "Face")]).hints(Hints::HORIZONTAL_ENUM),
        ParamSpec::query("targets", "Parts, surfaces, or curves to split", QueryFilter::BODY | QueryFilter::WIRE_BODY)
            .hints(Hints::INITIAL_FOCUS)
            .visible(part),
        ParamSpec::query1("tool", "Entity to split with", QueryFilter::SHEET_BODY | QueryFilter::ANY_FACE | QueryFilter::PLANE | QueryFilter::MATE_CONNECTOR)
            .visible(part),
        ParamSpec::bool("keepTools", "Keep tools", false).visible(part),
        ParamSpec::bool("useTrimmed", "Trim to face boundaries", false).visible(part),
        ParamSpec::bool("keepBothSides", "Keep both sides", true).visible(part),
        default_true(ParamSpec::flip("keepFront", "Opposite direction")).visible(move |v| part(v) && !v.bool("keepBothSides")),
        ParamSpec::query("faceTargets", "Faces to split", QueryFilter::ANY_FACE).visible(face),
        ParamSpec::query(
            "faceTools",
            "Entities to split with",
            QueryFilter::ANY_EDGE | QueryFilter::SHEET_BODY | QueryFilter::WIRE_BODY | QueryFilter::ANY_FACE | QueryFilter::PLANE | QueryFilter::MATE_CONNECTOR,
        )
        .visible(face),
    ];
    let mut projection = group("edgeProjectionOptionsGroup", "Edge projection options", true, vec![
        ParamSpec::enum_("projectionType", "Projection direction type", &[("DIRECTION", "Direction"), ("NORMAL_TO_TARGET", "Normal to target")]).visible(face),
        ParamSpec::bool("useSketchPlaneDirection", "Use sketch plane direction", true).visible(dir),
        ParamSpec::query1("directionQuery", "Direction", QueryFilter::DIRECTION).visible(move |v| dir(v) && !v.bool("useSketchPlaneDirection")),
    ]);
    // Its title shows in Face mode only.
    let head = projection.remove(0).visible(face);
    projection.insert(0, head);
    p.extend(projection);
    p.push(ParamSpec::bool("keepToolSurfaces", "Keep tool surfaces and curves", true).visible(face));
    FeatureType::new("splitPart", "Split", Category::Solid, p).icon("split").describe("Split parts with a plane, face or surface, or split faces with curves")
}

pub fn delete_part() -> FeatureType {
    let ignore = |v: &ParamValues| v.is("compositePartOption", "IGNORE");
    let p = vec![
        ParamSpec::enum_(
            "compositePartOption",
            "Composite part options",
            &[("DELETE", "Delete composite parts"), ("DISSOLVE", "Dissolve composite parts"), ("IGNORE", "Ignore composite parts")],
        ),
        ParamSpec::query("entities", "Entities to delete", QueryFilter::BODY | QueryFilter::WIRE_BODY | QueryFilter::POINT_BODY)
            .hints(Hints::INITIAL_FOCUS)
            .visible(move |v| !ignore(v)),
        ParamSpec::query("nonCompositeEntities", "Entities to delete", QueryFilter::BODY | QueryFilter::WIRE_BODY | QueryFilter::POINT_BODY).visible(ignore),
    ];
    FeatureType::new("deleteBodies", "Delete part", Category::Solid, p).icon("deletePart").describe("Delete parts, surfaces, curves or composite parts")
}
