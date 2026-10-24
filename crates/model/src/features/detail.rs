//! Detail feature schemas: Chamfer, Face blend, Modify fillet, Draft and
//! Body draft (research/sections/C-features-core.md §8-12).

use super::common::*;
use super::{Category, FeatureType};
use crate::params::{Hints, NumSpec, ParamSpec, ParamValues, QueryFilter};

pub fn register(out: &mut Vec<FeatureType>) {
    out.push(chamfer());
    out.push(face_blend());
    out.push(modify_fillet());
    out.push(draft());
    out.push(body_draft());
}

pub const CHAMFER_METHOD: &[(&str, &str)] = &[("FACE_OFFSET", "Offset"), ("APEX_RANGE", "Tangent")];
pub const CHAMFER_TYPE: &[(&str, &str)] = &[("EQUAL_OFFSETS", "Equal distance"), ("TWO_OFFSETS", "Two distances"), ("OFFSET_ANGLE", "Distance and angle")];

/// Chamfer parameters shared with sheet-metal Corner break (`gate` = when
/// the chamfer options apply).
pub fn chamfer_options(gate: impl Fn(&ParamValues) -> bool + Clone + Send + Sync + 'static) -> Vec<ParamSpec> {
    let rem = Hints::REMEMBER_PREVIOUS_VALUE;
    let g = gate.clone();
    let ty = move |v: &ParamValues, t: &str| g(v) && v.is("chamferType", t);
    vec![
        ParamSpec::enum_("chamferMethod", "Measurement", CHAMFER_METHOD).hints(Hints::SHOW_LABEL | rem).visible(gate.clone()),
        ParamSpec::enum_("chamferType", "Chamfer type", CHAMFER_TYPE).hints(Hints::SHOW_LABEL | rem).visible(gate),
        ParamSpec::num("width", "Distance", BLEND_BOUNDS).hints(rem | Hints::CAN_BE_TOLERANT).visible({
            let ty = ty.clone();
            move |v| ty(v, "EQUAL_OFFSETS") || ty(v, "OFFSET_ANGLE")
        }),
        ParamSpec::num("width1", "Distance 1", BLEND_BOUNDS).hints(Hints::CAN_BE_TOLERANT).visible({
            let ty = ty.clone();
            move |v| ty(v, "TWO_OFFSETS")
        }),
        ParamSpec::flip("oppositeDirection", "Opposite direction").visible({
            let ty = ty.clone();
            move |v| ty(v, "TWO_OFFSETS") || ty(v, "OFFSET_ANGLE")
        }),
        ParamSpec::num("width2", "Distance 2", BLEND_BOUNDS).hints(Hints::CAN_BE_TOLERANT).visible({
            let ty = ty.clone();
            move |v| ty(v, "TWO_OFFSETS")
        }),
        ParamSpec::num("angle", "Angle", NumSpec::angle(45.0, 0.1, 179.9)).hints(Hints::CAN_BE_TOLERANT).visible({
            let ty = ty.clone();
            move |v| ty(v, "OFFSET_ANGLE")
        }),
        ParamSpec::query("directionOverrides", "Direction overrides", QueryFilter::EDGE).visible(move |v| ty(v, "TWO_OFFSETS") || ty(v, "OFFSET_ANGLE")),
    ]
}

pub fn chamfer() -> FeatureType {
    let mut p = vec![ParamSpec::query("entities", "Entities to chamfer", QueryFilter::EDGE | QueryFilter::ANY_FACE).hints(Hints::INITIAL_FOCUS)];
    p.extend(chamfer_options(always));
    p.push(ParamSpec::bool("tangentPropagation", "Tangent propagation", true));
    FeatureType::new("chamfer", "Chamfer", Category::Detail, p).describe("Bevel sharp edges")
}

pub fn face_blend() -> FeatureType {
    let rem = Hints::REMEMBER_PREVIOUS_VALUE;
    let radius = |v: &ParamValues| v.is("blendControlType", "RADIUS");
    let mut p = vec![
        ParamSpec::query("side1", "Side 1", QueryFilter::ANY_FACE).hints(Hints::INITIAL_FOCUS),
        ParamSpec::flip("flipSide1", "Flip side 1"),
        ParamSpec::query("side2", "Side 2", QueryFilter::ANY_FACE),
        ParamSpec::flip("flipSide2", "Flip side 2"),
        ParamSpec::bool("propagation", "Propagation", true).hints(Hints::GROUP_HEADER),
        ParamSpec::enum_("propagationType", "Propagation type", &[("TANGENT", "Tangent"), ("ADJACENT", "Adjacent"), ("CUSTOM", "Custom")])
            .visible(|v| v.bool("propagation")),
        ParamSpec::num("propagationAngle", "Maximum angle", NumSpec::angle(0.0, 0.0, 180.0))
            .visible(|v| v.bool("propagation") && v.is("propagationType", "CUSTOM")),
    ];
    p.extend(group("crossSectionGroup", "Cross section", true, vec![
        ParamSpec::enum_("crossSection", "Cross section", &[("ROLLING_BALL", "Rolling ball"), ("SWEPT_PROFILE", "Swept profile")])
            .hints(Hints::SHOW_LABEL | rem),
        ParamSpec::query1("spine", "Spine", QueryFilter::EDGE).visible(|v| v.is("crossSection", "SWEPT_PROFILE")),
        ParamSpec::enum_("blendControlType", "Measurement", &[("RADIUS", "Radius"), ("WIDTH", "Width")]).hints(Hints::SHOW_LABEL | rem),
        ParamSpec::num("radius", "Radius", BLEND_BOUNDS).hints(rem).visible(radius),
        ParamSpec::num("width", "Width", BLEND_BOUNDS).hints(rem).visible(move |v| !radius(v)),
        ParamSpec::bool("asymmetric", "Asymmetric", false),
        ParamSpec::num("secondRadius", "Second radius", BLEND_BOUNDS).hints(rem).visible(move |v| v.bool("asymmetric") && radius(v)),
        ParamSpec::num("widthRatio", "Ratio", NumSpec::real(1.0, 1e-4, 1e4)).hints(rem).visible(move |v| v.bool("asymmetric") && !radius(v)),
        ParamSpec::flip("switchAsymmetric", "Switch side").visible(|v| v.bool("asymmetric")),
        ParamSpec::enum_("crossSectionShape", "Control", &[("CIRCULAR", "Circular"), ("CONIC", "Conic"), ("CURVATURE", "Curvature"), ("CHAMFER", "Chamfer")])
            .hints(Hints::SHOW_LABEL | rem),
        ParamSpec::num("rho", "Rho", NumSpec::real(0.5, 0.0, 0.99999)).visible(|v| v.is("crossSectionShape", "CONIC")),
        ParamSpec::num("magnitude", "Magnitude", NumSpec::real(0.5, 0.0, 0.99999)).visible(|v| v.is("crossSectionShape", "CURVATURE")),
    ]));
    p.extend(group("constraintsAndLimitsGroup", "Constraints and limits", false, vec![
        ParamSpec::bool("tangentHoldLines", "Tangent hold lines", false),
        ParamSpec::query("tangentEdges", "Tangent edges", QueryFilter::EDGE).visible(|v| v.bool("tangentHoldLines")),
        ParamSpec::query("inverseTangentEdges", "Inverse tangent edges", QueryFilter::EDGE).visible(|v| v.bool("tangentHoldLines")),
        ParamSpec::bool("conicHoldLines", "Conic hold lines", false),
        ParamSpec::query("conicEdges", "Conic edges", QueryFilter::EDGE).visible(|v| v.bool("conicHoldLines")),
        ParamSpec::query("inverseConicEdges", "Inverse conic edges", QueryFilter::EDGE).visible(|v| v.bool("conicHoldLines")),
        ParamSpec::bool("hasCliffEdges", "Cliff edges", false),
        ParamSpec::query("cliffEdges", "Cliff edges", QueryFilter::EDGE).visible(|v| v.bool("hasCliffEdges")),
        ParamSpec::bool("hasCaps", "Caps", false),
        ParamSpec::array(
            "caps",
            "Caps",
            "Cap",
            vec![ParamSpec::query1("entity", "Cap entity", QueryFilter::ANY_FACE | QueryFilter::PLANE), ParamSpec::flip("capFlip", "Flip cap")],
        )
        .visible(|v| v.bool("hasCaps")),
        ParamSpec::bool("hasLimits", "Limits", false),
        ParamSpec::query1("limitPlane1", "Plane limit", QueryFilter::PLANE | QueryFilter::PLANAR_FACE).visible(|v| v.bool("hasLimits")),
        ParamSpec::flip("limitPlane1Flip", "Flip first plane").visible(|v| v.bool("hasLimits")),
        ParamSpec::query1("limitPlane2", "Second plane limit", QueryFilter::PLANE | QueryFilter::PLANAR_FACE).visible(|v| v.bool("hasLimits")),
        ParamSpec::flip("limitPlane2Flip", "Flip second plane").visible(|v| v.bool("hasLimits")),
        ParamSpec::query("faceLimits", "Face limits", QueryFilter::ANY_FACE).visible(|v| v.bool("hasLimits")),
        ParamSpec::array(
            "edgeLimits",
            "Edge limits",
            "Edge limit",
            vec![ParamSpec::query1("edge", "Edge limit", QueryFilter::EDGE), ParamSpec::query1("edgeLimitSide", "Side", QueryFilter::ANY_FACE)],
        )
        .visible(|v| v.bool("hasLimits")),
        ParamSpec::bool("hasHelpPoint", "Help point", false),
        ParamSpec::query1("helpPoint", "Help point", QueryFilter::VERTEX | QueryFilter::MATE_CONNECTOR).visible(|v| v.bool("hasHelpPoint")),
    ]));
    p.extend([
        ParamSpec::enum_("trim", "Trim type", &[("WALLS", "Walls"), ("SHORT", "Short"), ("LONG", "Long"), ("NO_TRIM", "No trim")])
            .hints(Hints::SHOW_LABEL | rem),
        ParamSpec::bool("detach", "Detached", false),
        ParamSpec::bool("showIsocurves", "Show isocurves", false),
        ParamSpec::num("curveCount", "Count", ISO_GRID).visible(|v| v.bool("showIsocurves")),
    ]);
    FeatureType::new("faceBlend", "Face blend", Category::Detail, p).describe("Blend two sets of faces, connected or not")
}

pub fn modify_fillet() -> FeatureType {
    let p = vec![
        ParamSpec::query("faces", "Fillet faces to modify", QueryFilter::ANY_FACE).hints(Hints::INITIAL_FOCUS),
        ParamSpec::enum_("modifyFilletType", "Modification type", &[("CHANGE_RADIUS", "Change radius"), ("REMOVE_FILLET", "Remove fillet")]),
        ParamSpec::num("radius", "Fillet radius", BLEND_BOUNDS).visible(|v| v.is("modifyFilletType", "CHANGE_RADIUS")),
        ParamSpec::bool("reFillet", "Reapply fillet", true).visible(|v| v.is("modifyFilletType", "CHANGE_RADIUS")),
    ];
    FeatureType::new("modifyFillet", "Modify fillet", Category::Direct, p).describe("Change the radius of, or remove, existing fillets")
}

pub fn draft() -> FeatureType {
    let rem = Hints::REMEMBER_PREVIOUS_VALUE;
    let neutral = |v: &ParamValues| v.is("draftFeatureType", "NEUTRAL_PLANE");
    let parting = |v: &ParamValues| v.is("draftFeatureType", "PARTING_LINE");
    let two_sided = move |v: &ParamValues| parting(v) && v.is("partingLineSides", "TWO_SIDED");
    let p = vec![
        ParamSpec::enum_("draftFeatureType", "Draft type", &[("NEUTRAL_PLANE", "Neutral plane"), ("PARTING_LINE", "Parting line")])
            .hints(Hints::HORIZONTAL_ENUM),
        ParamSpec::query1("neutralPlane", "Neutral plane", QueryFilter::PLANE_LIKE).hints(Hints::INITIAL_FOCUS).visible(neutral),
        ParamSpec::query("draftFaces", "Entities to draft", QueryFilter::ANY_FACE).hints(Hints::INITIAL_FOCUS_ON_EDIT).visible(neutral),
        hidden(ParamSpec::query("hintFaces", "Hint faces", QueryFilter::ANY_FACE)),
        ParamSpec::query1("pullDirectionEntity", "Pull direction", QueryFilter::DIRECTION).visible(parting),
        ParamSpec::query("partingEdges", "Parting edges", QueryFilter::EDGE).hints(Hints::SHOW_CREATE_SELECTION | Hints::INITIAL_FOCUS_ON_EDIT).visible(parting),
        ParamSpec::enum_("partingLineSides", "Sides", &[("ONE_SIDED", "One sided"), ("SYMMETRIC", "Symmetric"), ("TWO_SIDED", "Two sided")]).visible(parting),
        default_true(ParamSpec::flip("alongPull", "Switch face")).visible(move |v| parting(v) && v.is("partingLineSides", "ONE_SIDED")),
        ParamSpec::num("angle", "Draft angle", NumSpec::angle(3.0, 0.0, 89.9)).hints(rem),
        ParamSpec::bool("pullDirection", "Opposite direction", false).hints(Hints::OPPOSITE_DIRECTION_CIRCULAR),
        ParamSpec::num("secondAngle", "Second draft angle", NumSpec::angle(3.0, 0.0, 89.9)).hints(rem).visible(two_sided),
        ParamSpec::bool("secondPullDirection", "Opposite direction", true).hints(Hints::OPPOSITE_DIRECTION_CIRCULAR).visible(two_sided),
        ParamSpec::bool("tangentPropagation", "Tangent propagation", true).visible(neutral),
        ParamSpec::bool("referenceEdgePropagation", "Parting line propagation", true).visible(parting),
        ParamSpec::bool("reFillet", "Reapply fillet", false),
    ];
    FeatureType::new("draft", "Draft", Category::Detail, p).describe("Taper faces relative to a neutral plane or parting line")
}

pub fn body_draft() -> FeatureType {
    let scope = |v: &ParamValues, s: &str| v.is("selectionType", s);
    let matching = |v: &ParamValues| v.bool("bothSides") && !v.bool("draftOnSelf") && v.bool("matchFacesAtParting");
    let p = vec![
        hidden(ParamSpec::query("preselection", "Preselection", QueryFilter::SOLID_BODY | QueryFilter::ANY_FACE | QueryFilter::EDGE)),
        ParamSpec::enum_("selectionType", "Draft scope", &[("PARTS", "Parts to draft"), ("FACES", "Faces to draft"), ("EDGES", "Edges to hold")])
            .hints(Hints::SHOW_LABEL | Hints::REMEMBER_PREVIOUS_VALUE),
        ParamSpec::query("topEdges", "Edges", QueryFilter::EDGE).visible(move |v| scope(v, "EDGES")),
        ParamSpec::query("faces", "Faces", QueryFilter::ANY_FACE).visible(move |v| scope(v, "FACES")),
        ParamSpec::query("bodies", "Parts", QueryFilter::SOLID_BODY).visible(move |v| scope(v, "PARTS")),
        ParamSpec::query("excludeFaces", "Faces to exclude", QueryFilter::ANY_FACE).visible(move |v| scope(v, "PARTS")),
        ParamSpec::bool("draftOnSelf", "Draft on self", false),
        ParamSpec::query1("pullDirection", "Pull direction", QueryFilter::DIRECTION),
        ParamSpec::flip("flipPullDirection", "Flip pull direction"),
        ParamSpec::query1(
            "partingObject",
            "Parting entity",
            QueryFilter::SHEET_BODY | QueryFilter::ANY_FACE | QueryFilter::PLANE | QueryFilter::MATE_CONNECTOR,
        )
        .visible(|v| !v.bool("draftOnSelf")),
        ParamSpec::num("angle", "Angle", NumSpec::angle(3.0, 0.0, 89.9)),
        ParamSpec::bool("bothSides", "Symmetric", false),
        ParamSpec::query("bottomEdges", "Bottom edges", QueryFilter::EDGE).visible(move |v| v.bool("bothSides") && scope(v, "EDGES")),
        ParamSpec::bool("matchFacesAtParting", "Match faces at parting", true)
            .hints(Hints::GROUP_HEADER)
            .visible(|v| v.bool("bothSides") && !v.bool("draftOnSelf")),
        ParamSpec::enum_("matchFaceType", "Match type", &[("TANGENT_TO_FACE", "Tangent to face"), ("REFERENCE_EDGE", "Reference edge")])
            .hints(Hints::SHOW_LABEL)
            .visible(matching),
        ParamSpec::enum_("concaveRepair", "Concave repair", &[("NONE", "None"), ("RADIUS", "Radius"), ("MIX", "Mix")])
            .hints(Hints::SHOW_LABEL)
            .visible(matching),
        ParamSpec::num("concaveRepairRadius", "Radius", BLEND_BOUNDS).visible(move |v| matching(v) && !v.is("concaveRepair", "NONE")),
        ParamSpec::enum_("cornerType", "Corner type", &[("EXTEND", "Extend"), ("PLANE", "Plane")]).hints(Hints::SHOW_LABEL),
        ParamSpec::bool("keepMaterial", "Keep material", false),
    ];
    FeatureType::new("bodyDraft", "Body draft", Category::Detail, p).describe("Draft whole parts, faces or held edges, adding material")
}
