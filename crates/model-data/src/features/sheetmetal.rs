//! Sheet metal schemas (03c SM-02..20): Sheet metal model, Flange, Tab,
//! Hem, Bend, Jog, Form, Make joint, Modify joint, Corner, Corner break,
//! Bend relief, Rip, Sheet metal loft, Finish sheet metal model, Recognize.

use super::common::*;
use super::detail::chamfer_options;
use super::{Category, FeatureType};
use crate::params::{Hints, NumSpec, ParamSpec, ParamValues, QueryFilter};

pub fn register(out: &mut Vec<FeatureType>) {
    out.push(sheet_metal_start());
    out.push(flange());
    out.push(hem());
    out.push(tab());
    out.push(bend());
    out.push(jog());
    out.push(form());
    out.push(form_tool());
    out.push(sheet_metal_loft());
    out.push(make_joint());
    out.push(corner());
    out.push(bend_relief());
    out.push(corner_break());
    out.push(modify_joint());
    out.push(sheet_metal_end());
    out.push(pcb_start());
    out.push(pcb_end());
    out.push(rip());
    out.push(unfold());
    out.push(refold());
    out.push(recognize());
}

/// SM_THICKNESS_BOUNDS: 2e-5 .. 500 m, default 1.6 mm / 0.0625 in.
pub const SM_THICKNESS: NumSpec = NumSpec::positive_length(1.6, 0.0625).bounded(2e-5, 500.0);
/// SM_BEND_RADIUS_BOUNDS: 1e-5 .. 500 m, default 2.3 mm / 0.09 in.
pub const SM_BEND_RADIUS: NumSpec = NumSpec::positive_length(2.3, 0.09).bounded(1e-5, 500.0);
/// SM_MINIMAL_CLEARANCE_BOUNDS: 2e-5 .. 1 m, default 0.02 mm / 0.001 in.
pub const SM_MINIMAL_CLEARANCE: NumSpec = NumSpec::positive_length(0.02, 0.001).bounded(2e-5, 1.0);
/// SM_RELIEF_SIZE_BOUNDS: 2e-5 .. 500 m, default 5 mm / 0.25 in.
pub const SM_RELIEF_SIZE: NumSpec = NumSpec::positive_length(5.0, 0.25).bounded(2e-5, 500.0);
pub const K_FACTOR: NumSpec = NumSpec::real(0.45, 0.0, 1.0);

pub const CORNER_RELIEF: &[(&str, &str)] = &[
    ("SIZED_RECTANGLE", "Square - Sized"),
    ("RECTANGLE", "Rectangle - Scaled"),
    ("SIZED_ROUND", "Round - Sized"),
    ("ROUND", "Round - Scaled"),
    ("CLOSED", "Closed"),
    ("SIMPLE", "Simple"),
];
pub const JOINT_STYLE: &[(&str, &str)] = &[("EDGE", "Edge joint"), ("BUTT", "Butt joint - Direction 1"), ("BUTT2", "Butt joint - Direction 2")];
pub const ANGLE_CONTROL: &[(&str, &str)] =
    &[("BEND_ANGLE", "Bend angle"), ("ALIGN_GEOMETRY", "Align to geometry"), ("ANGLE_FROM_DIRECTION", "Angle from direction")];

/// Side faces or linear edges of an active sheet metal model.
const SM_EDGES: QueryFilter = QueryFilter(QueryFilter::EDGE.0 | QueryFilter::PLANAR_FACE.0);
const SM_CORNER: QueryFilter = QueryFilter(QueryFilter::VERTEX.0 | QueryFilter::EDGE.0);
const SM_BOUNDS: &[(&str, &str)] =
    &[("BLIND", "Blind"), ("UP_TO_NEXT", "Up to next"), ("UP_TO_SURFACE", "Up to face"), ("UP_TO_BODY", "Up to part"), ("UP_TO_VERTEX", "Up to vertex")];

/// Model parameter groups (sheetMetalModelParameters): General, Material,
/// Relief. Shown when `gate` holds.
pub fn model_parameters(gate: impl Fn(&ParamValues) -> bool + Copy + Send + Sync + 'static) -> Vec<ParamSpec> {
    let rem = Hints::REMEMBER_PREVIOUS_VALUE;
    // sheetMetalModelParameters: groups "General" (open), "Material" and
    // "Relief" (collapsed by default).
    let group = |id: &'static str, label: &'static str, open: bool| ParamSpec::bool(id, label, open).hints(Hints::GROUP_HEADER | Hints::GROUP_LABEL | Hints::UNCONFIGURABLE).visible(gate);
    let general = move |v: &ParamValues| gate(v) && v.bool("generalGroup");
    let material = move |v: &ParamValues| gate(v) && v.bool("materialGroup");
    let relief = move |v: &ParamValues| gate(v) && v.bool("reliefGroup");
    let corner = move |v: &ParamValues, s: &str| relief(v) && v.is("defaultCornerStyle", s);
    let bend_scaled = move |v: &ParamValues| relief(v) && !v.is("defaultBendReliefStyle", "TEAR");
    vec![
        group("generalGroup", "General", true),
        ParamSpec::num("thickness", "Thickness", SM_THICKNESS).hints(rem).visible(general),
        ParamSpec::flip("oppositeDirection", "Opposite direction").visible(general),
        ParamSpec::num("radius", "Bend radius", SM_BEND_RADIUS).hints(rem).visible(general),
        ParamSpec::bool("flipDirectionUp", "Flip direction up", false).hints(rem).visible(general),
        group("materialGroup", "Material", false),
        ParamSpec::enum_(
            "bendCalculationType",
            "Bend calculation",
            &[("K_FACTOR", "K Factor"), ("BEND_ALLOWANCE", "Bend allowance"), ("BEND_DEDUCTION", "Bend deduction")],
        )
        .hints(rem | Hints::SHOW_LABEL)
        .visible(material),
        ParamSpec::num("kFactor", "Default bend K Factor", K_FACTOR).hints(rem).visible(material),
        ParamSpec::num("kFactorRolled", "Rolled K Factor", NumSpec::real(0.5, 0.0, 1.0)).hints(rem).visible(material),
        group("reliefGroup", "Relief", false),
        ParamSpec::num("minimalClearance", "Minimal gap", SM_MINIMAL_CLEARANCE).hints(rem).visible(relief),
        ParamSpec::enum_("defaultCornerStyle", "Corner relief type", CORNER_RELIEF).default_enum("SIMPLE").hints(rem | Hints::SHOW_LABEL).visible(relief),
        ParamSpec::num("defaultCornerReliefScale", "Corner relief scale", NumSpec::real(1.5, 1.0, 2.0))
            .hints(rem)
            .visible(move |v| corner(v, "RECTANGLE") || corner(v, "ROUND")),
        ParamSpec::num("defaultRoundReliefDiameter", "Corner relief diameter", SM_RELIEF_SIZE).hints(rem).visible(move |v| corner(v, "SIZED_ROUND")),
        ParamSpec::num("defaultSquareReliefWidth", "Corner relief width", SM_RELIEF_SIZE).hints(rem).visible(move |v| corner(v, "SIZED_RECTANGLE")),
        ParamSpec::enum_(
            "defaultBendReliefStyle",
            "Bend relief type",
            &[("RECTANGLE", "Rectangle - Scaled"), ("OBROUND", "Obround - Scaled"), ("TEAR", "Tear")],
        )
        .default_enum("OBROUND")
        .hints(rem | Hints::SHOW_LABEL)
        .visible(relief),
        ParamSpec::num("defaultBendReliefDepthScale", "Bend relief depth scale", NumSpec::real(2.0, 1.0, 5.0)).hints(rem).visible(bend_scaled),
        ParamSpec::num("defaultBendReliefScale", "Bend relief width scale", NumSpec::real(1.0625, 0.0625, 2.0)).hints(rem).visible(bend_scaled),
    ]
}

/// One extrude direction of Sheet metal model (Extrude process).
fn sm_extrude_direction(second: bool) -> Vec<ParamSpec> {
    let ids: [&'static str; 9] = if second {
        [
            "secondDirectionBound",
            "secondDirectionOppositeExtrudeDirection",
            "secondDirectionDepth",
            "secondDirectionBoundEntityFace",
            "secondDirectionBoundEntityBody",
            "secondDirectionBoundEntityVertex",
            "hasSecondDirectionOffset",
            "secondDirectionOffsetDistance",
            "secondDirectionOffsetOppositeDirection",
        ]
    } else {
        [
            "endBound",
            "oppositeExtrudeDirection",
            "depth",
            "endBoundEntityFace",
            "endBoundEntityBody",
            "endBoundEntityVertex",
            "hasOffset",
            "offsetDistance",
            "offsetOppositeDirection",
        ]
    };
    let [bound, flip, depth, face, body, vertex, has_off, off, off_flip] = ids;
    let gate =
        move |v: &ParamValues| v.is("process", "EXTRUDE") && (!second || (v.bool("hasSecondDirection") && !(v.bool("symmetric") && v.is("endBound", "BLIND"))));
    let up_to = move |v: &ParamValues| gate(v) && !v.is(bound, "BLIND");
    let mut flip_spec = ParamSpec::flip(flip, "Opposite direction").visible(gate);
    if second {
        flip_spec = default_true(flip_spec);
    }
    vec![
        ParamSpec::enum_(bound, "End type", SM_BOUNDS).visible(gate),
        flip_spec,
        ParamSpec::length(depth, "Depth", 25.0, 1.0).hints(Hints::CAN_BE_TOLERANT).visible(move |v| gate(v) && v.is(bound, "BLIND")),
        ParamSpec::query1(face, "Up to face", QueryFilter::ANY_FACE | QueryFilter::PLANE | QueryFilter::MATE_CONNECTOR)
            .visible(move |v| gate(v) && v.is(bound, "UP_TO_SURFACE")),
        ParamSpec::query1(body, "Up to surface or part", QueryFilter::BODY).visible(move |v| gate(v) && v.is(bound, "UP_TO_BODY")),
        ParamSpec::query1(vertex, "Up to vertex or mate connector", QueryFilter::POINT_LIKE).visible(move |v| gate(v) && v.is(bound, "UP_TO_VERTEX")),
        ParamSpec::bool(has_off, "Offset distance", false).hints(Hints::DISPLAY_SHORT | Hints::FIRST_IN_ROW).visible(up_to),
        ParamSpec::length(off, "Offset distance", 25.0, 1.0).hints(Hints::DISPLAY_SHORT).visible(move |v| up_to(v) && v.bool(has_off)),
        ParamSpec::flip(off_flip, "Opposite direction").visible(move |v| up_to(v) && v.bool(has_off)),
    ]
}

pub fn sheet_metal_start() -> FeatureType {
    let mut p = start_selections();
    p.extend(model_parameters(always));
    p.push(hidden(ParamSpec::text("parameterLibraryPurposeId", "Parameter library purpose", "")));
    FeatureType::new("sheetMetalStart", "Sheet metal model", Category::SheetMetal, p)
        .icon("sheetMetalModel")
        .describe("Create an active sheet metal model by converting, extruding or thickening")
}

/// pcbStart.fs: the Sheet metal model selections, then smGeneralParameters
/// (ungrouped) and the K factor (PCB_K_FACTOR_BOUNDS: 0..1, default 0).
pub fn pcb_start() -> FeatureType {
    let rem = Hints::REMEMBER_PREVIOUS_VALUE;
    let mut p = start_selections();
    p.extend([
        ParamSpec::num("thickness", "Thickness", SM_THICKNESS).hints(rem),
        ParamSpec::flip("oppositeDirection", "Opposite direction"),
        ParamSpec::num("radius", "Bend radius", SM_BEND_RADIUS).hints(rem),
        ParamSpec::bool("flipDirectionUp", "Flip direction up", false).hints(rem),
        ParamSpec::num("kFactor", "Default bend K Factor", NumSpec::real(0.0, 0.0, 1.0)).hints(rem),
    ]);
    FeatureType::new("pcbStart", "Flex PCB model", Category::SheetMetal, p)
        .icon("pcbStart")
        .describe("Create an active flex PCB model by converting, extruding or thickening")
}

/// pcbEnd.fs: unfold, then finish the flex PCB model.
pub fn pcb_end() -> FeatureType {
    let ty = |v: &ParamValues, t: &str| v.is("unfoldType", t);
    let p = vec![
        hidden(ParamSpec::query("initEntities", "Entities", QueryFilter::ANY_FACE | QueryFilter::EDGE)),
        ParamSpec::enum_("unfoldType", "Unfold type", &[("UNFOLD_ALL", "Unfold all"), ("UNFOLD_SELECTED", "Unfold selected"), ("UNFOLD_NONE", "Unfold none")]),
        ParamSpec::query("entitiesToUnfold", "Bend or non-planar faces to unfold", QueryFilter::ANY_FACE).visible(move |v| ty(v, "UNFOLD_SELECTED")),
        ParamSpec::query1("holdEntity", "Wall face or bounding edge to hold", QueryFilter::ANY_FACE | QueryFilter::EDGE).visible(move |v| !ty(v, "UNFOLD_NONE")),
        ParamSpec::query1("pcbPart", "PCB part to finish", QueryFilter::SOLID_BODY),
    ];
    FeatureType::new("pcbEnd", "Finish flex PCB model", Category::SheetMetal, p)
        .icon("pcbEnd")
        .describe("Unfold and finish flex PCB models")
}

/// sheetMetalStart.fs / pcbStart.fs: initial entities, Process and the
/// "Selections" group.
fn start_selections() -> Vec<ParamSpec> {
    let pr = |v: &ParamValues, t: &str| v.is("process", t);
    let extrude = move |v: &ParamValues| pr(v, "EXTRUDE");
    let conv_or_thick = move |v: &ParamValues| pr(v, "CONVERT") || pr(v, "THICKEN");
    let mut p = vec![
        hidden(ParamSpec::query("initEntities", "Initial entities", QueryFilter::BODY | QueryFilter::ANY_FACE | QueryFilter::SKETCH_EDGE)),
        ParamSpec::enum_("process", "Process", &[("CONVERT", "Convert"), ("EXTRUDE", "Extrude"), ("THICKEN", "Thicken")]).hints(Hints::HORIZONTAL_ENUM),
    ];
    // sheetMetalStart.fs: the selections sit in the "Selections" group.
    let mut sel = vec![
        ParamSpec::query("partToConvert", "Parts and surfaces to convert", QueryFilter::BODY).hints(Hints::INITIAL_FOCUS).visible(move |v| pr(v, "CONVERT")),
        ParamSpec::query("facesToExclude", "Faces to exclude", QueryFilter::ANY_FACE).visible(move |v| pr(v, "CONVERT")),
        ParamSpec::query("sketchCurves", "Sketch curves to extrude", QueryFilter::SKETCH_EDGE).visible(extrude),
        ParamSpec::query("bendArcs", "Arcs to extrude as bends", QueryFilter::SKETCH_EDGE).visible(extrude),
    ];
    sel.extend(sm_extrude_direction(false));
    sel.push(ParamSpec::bool("symmetric", "Symmetric", false).visible(move |v| extrude(v) && v.is("endBound", "BLIND")));
    sel.push(
        ParamSpec::bool("hasSecondDirection", "Second end position", false)
            .hints(Hints::GROUP_HEADER)
            .visible(move |v| extrude(v) && !(v.bool("symmetric") && v.is("endBound", "BLIND"))),
    );
    sel.extend(sm_extrude_direction(true));
    sel.extend([
        ParamSpec::query("regions", "Faces or sketch regions to thicken", QueryFilter::ANY_FACE | QueryFilter::SKETCH_REGION)
            .visible(move |v| pr(v, "THICKEN")),
        ParamSpec::bool("tangentPropagation", "Tangent propagation", false).visible(move |v| pr(v, "THICKEN")),
        ParamSpec::query("bends", "Edges or cylinders to bend", QueryFilter::LINEAR_EDGE | QueryFilter::CYLINDRICAL_FACE).visible(conv_or_thick),
        ParamSpec::num("clearance", "Clearance from input", NONNEGATIVE_ZERO_DEFAULT).visible(conv_or_thick),
        ParamSpec::bool("bendsIncluded", "Include bends", false).visible(conv_or_thick),
        ParamSpec::bool("keepInputParts", "Keep input parts", false).visible(move |v| pr(v, "CONVERT")),
    ]);
    p.extend(group("selectionsGroup", "Selections", true, sel));
    p
}

/// "Use model bend radius" + "Bend radius".
fn bend_radius_override(gate: impl Fn(&ParamValues) -> bool + Copy + Send + Sync + 'static, radius_id: &'static str) -> Vec<ParamSpec> {
    vec![
        ParamSpec::bool("useDefaultRadius", "Use model bend radius", true).visible(gate),
        ParamSpec::num(radius_id, "Bend radius", SM_BEND_RADIUS).visible(move |v| gate(v) && !v.bool("useDefaultRadius")),
    ]
}

/// "Use model K Factor" + "K Factor".
fn k_factor_override(gate: impl Fn(&ParamValues) -> bool + Copy + Send + Sync + 'static) -> Vec<ParamSpec> {
    vec![
        ParamSpec::bool("useDefaultKFactor", "Use model K Factor", true).visible(gate),
        ParamSpec::num("kFactor", "K Factor", K_FACTOR).visible(move |v| gate(v) && !v.bool("useDefaultKFactor")),
    ]
}

pub fn flange() -> FeatureType {
    const BOUND: &[(&str, &str)] = &[("BLIND", "Blind"), ("UP_TO_ENTITY", "Up to entity"), ("UP_TO_ENTITY_OFFSET", "Up to entity with offset")];
    let lim = |v: &ParamValues, t: &str| v.is("limitType", t);
    let ac = |v: &ParamValues, t: &str| v.is("angleControlType", t);
    let partial = |v: &ParamValues| v.bool("isPartialFlange");
    let flange_len = NumSpec::positive_length(25.0, 1.0).bounded(1e-5, 500.0);
    let mut p = vec![
        ParamSpec::query("edges", "Edges or side faces to flange", SM_EDGES).hints(Hints::INITIAL_FOCUS),
        ParamSpec::enum_("flangeAlignment", "Flange alignment", &[("INNER", "Inner"), ("OUTER", "Outer"), ("MIDDLE", "Middle"), ("BEND", "Hold line")]),
        ParamSpec::enum_("limitType", "End type", BOUND),
        ParamSpec::num("distance", "Distance", flange_len).visible(move |v| lim(v, "BLIND")),
        ParamSpec::query1("limitEntity", "Up to entity", QueryFilter::ANY_FACE | QueryFilter::PLANE | QueryFilter::POINT_LIKE)
            .visible(move |v| !lim(v, "BLIND")),
        ParamSpec::num("offset", "Offset", flange_len).visible(move |v| lim(v, "UP_TO_ENTITY_OFFSET")),
        ParamSpec::flip("oppositeOffsetDirection", "Opposite offset direction").visible(move |v| lim(v, "UP_TO_ENTITY_OFFSET")),
        ParamSpec::enum_("angleControlType", "Angle control type", ANGLE_CONTROL),
        ParamSpec::flip("oppositeDirection", "Opposite side"),
        ParamSpec::num("bendAngle", "Bend angle", NumSpec::angle(90.0, 1.0, 179.0)).visible(move |v| ac(v, "BEND_ANGLE")),
        ParamSpec::query1("parallelEntity", "Parallel to", QueryFilter::DIRECTION).visible(move |v| ac(v, "ALIGN_GEOMETRY")),
        ParamSpec::query1("directionEntity", "Direction", QueryFilter::DIRECTION).visible(move |v| ac(v, "ANGLE_FROM_DIRECTION")),
        ParamSpec::num("angleFromDirection", "Angle", NumSpec::angle(90.0, 0.0, 180.0)).visible(move |v| ac(v, "ANGLE_FROM_DIRECTION")),
        ParamSpec::bool("angleFromDirectionOppositeAngle", "Opposite angle", false)
            .hints(Hints::OPPOSITE_DIRECTION_CIRCULAR)
            .visible(move |v| ac(v, "ANGLE_FROM_DIRECTION")),
        ParamSpec::bool("autoMiter", "Automatic miter", true),
        ParamSpec::num("miterAngle", "Miter angle", NumSpec::angle(45.0, 0.0, 179.0)).visible(|v| !v.bool("autoMiter")),
    ];
    p.extend(bend_radius_override(always, "bendRadius"));
    p.extend([
        ParamSpec::bool("isPartialFlange", "Partial flange", false).hints(Hints::GROUP_HEADER),
        ParamSpec::enum_("chainType", "Chain type", &[("PER_EDGE", "Per edge"), ("PER_CHAIN", "Per chain")]).visible(partial),
        ParamSpec::flip("flipFlangeBounds", "Flip sides").visible(partial),
        ParamSpec::bool("holdAdjacentEdges", "Hold adjacent edges", true).visible(partial),
    ]);
    for (gate_id, ty, dist, ent, ent_off, ent_flip, labels) in [
        (
            "",
            "firstBoundType",
            "firstBoundOffset",
            "firstBoundingEntity",
            "firstBoundingEntityOffset",
            "firstBoundingEntityOppositeOffsetDirection",
            ["Bound type", "Distance"],
        ),
        (
            "hasSecondBound",
            "secondBoundType",
            "secondBoundOffset",
            "secondBoundingEntity",
            "secondBoundingEntityOffset",
            "secondBoundingEntityOppositeOffsetDirection",
            ["Second bound type", "Second distance"],
        ),
    ] {
        if !gate_id.is_empty() {
            p.push(ParamSpec::bool(gate_id, "Second bound", false).visible(partial));
        }
        let on = move |v: &ParamValues| partial(v) && (gate_id.is_empty() || v.bool(gate_id));
        p.push(ParamSpec::enum_(ty, labels[0], BOUND).visible(on));
        p.push(ParamSpec::num(dist, labels[1], NumSpec::positive_length(25.0, 1.0)).visible(move |v| on(v) && v.is(ty, "BLIND")));
        p.push(
            ParamSpec::query1(ent, "Up to entity", QueryFilter::PLANE | QueryFilter::PLANAR_FACE | QueryFilter::POINT_LIKE)
                .visible(move |v| on(v) && !v.is(ty, "BLIND")),
        );
        p.push(ParamSpec::num(ent_off, "Distance", NumSpec::positive_length(25.0, 1.0)).visible(move |v| on(v) && v.is(ty, "UP_TO_ENTITY_OFFSET")));
        p.push(ParamSpec::flip(ent_flip, "Opposite offset direction").visible(move |v| on(v) && v.is(ty, "UP_TO_ENTITY_OFFSET")));
    }
    FeatureType::new("sheetMetalFlange", "Flange", Category::SheetMetal, p).icon("flange").describe("Add a wall joined by a bend to each selected edge")
}

pub fn hem() -> FeatureType {
    let ty = |v: &ParamValues, t: &str| v.is("hemType", t);
    let p = vec![
        ParamSpec::query("edges", "Edges or side faces to hem", SM_EDGES).hints(Hints::INITIAL_FOCUS),
        ParamSpec::enum_("hemType", "Hem type", &[("STRAIGHT", "Straight"), ("ROLLED", "Rolled"), ("TEAR_DROP", "Tear drop")])
            .hints(Hints::REMEMBER_PREVIOUS_VALUE),
        ParamSpec::flip("oppositeDirection", "Opposite direction"),
        ParamSpec::bool("useMinimalGapForStraight", "Flattened", true).visible(move |v| ty(v, "STRAIGHT")),
        ParamSpec::num("innerRadius", "Inner radius", SM_BEND_RADIUS).visible(move |v| !ty(v, "STRAIGHT") || !v.bool("useMinimalGapForStraight")),
        ParamSpec::bool("useMinimalGapForTipGap", "Use minimal gap", true).visible(move |v| ty(v, "TEAR_DROP")),
        ParamSpec::num("tipGap", "Gap", SM_MINIMAL_CLEARANCE).visible(move |v| ty(v, "TEAR_DROP") && !v.bool("useMinimalGapForTipGap")),
        ParamSpec::num("angle", "Angle", NumSpec::angle(270.0, 180.0, 359.9)).visible(move |v| ty(v, "ROLLED")),
        ParamSpec::num("length", "Total length", NumSpec::positive_length(12.5, 0.5).bounded(1e-5, 500.0)).visible(move |v| !ty(v, "ROLLED")),
        ParamSpec::enum_("hemAlignment", "Hem alignment", &[("OUTER", "Outer"), ("IN_PLACE", "In place")]),
        ParamSpec::enum_("hemCornerType", "Corner type", &[("SIMPLE", "Simple"), ("CLOSED", "Closed")]),
    ];
    FeatureType::new("sheetMetalHem", "Hem", Category::SheetMetal, p).icon("hem").describe("Fold an edge back on itself")
}

pub fn tab() -> FeatureType {
    let p = vec![
        ParamSpec::query("tabFaces", "Tab profile", QueryFilter::PLANAR_FACE | QueryFilter::SKETCH_REGION).hints(Hints::INITIAL_FOCUS),
        ParamSpec::query("booleanUnionScope", "Flange to merge", QueryFilter::ANY_FACE),
        ParamSpec::num("booleanOffset", "Subtraction offset", NONNEGATIVE_ZERO_DEFAULT),
        ParamSpec::query("booleanSubtractScope", "Subtraction scope", QueryFilter::ANY_FACE | QueryFilter::SOLID_BODY),
    ];
    FeatureType::new("sheetMetalTab", "Tab", Category::SheetMetal, p).icon("tab").describe("Add a planar tab to a sheet metal wall")
}

/// Bend-line, face and angle fields shared by Bend and Jog
/// (sheetMetalBendTopPredicate).
fn bend_top(max_angle: f64) -> Vec<ParamSpec> {
    let ac = |v: &ParamValues, t: &str| v.is("angleControlType", t);
    vec![
        // std: `GeometryType.LINE` only, so no "Select mate connector" button
        // beside the field (live qa5 BEND2, dom/qa5-sm-dialog-bend.html).
        ParamSpec::query1("bendReference", "Bend line", QueryFilter::LINEAR_EDGE | QueryFilter::SKETCH_EDGE).hints(Hints::INITIAL_FOCUS),
        ParamSpec::flip("holdOtherSide", "Hold opposite side"),
        ParamSpec::query1("face", "Sheet metal face to bend", QueryFilter::PLANAR_FACE),
        // std `UIHint.SHOW_LABEL`: "Bend alignment [Bend line ▾]" (live).
        ParamSpec::enum_(
            "bendAlignment",
            "Bend alignment",
            &[
                ("BEND_LINE", "Bend line"),
                ("HELD_EDGE", "Hold line"),
                ("BENT_EDGE", "Hold other line"),
                ("BENT_INSIDE", "Inner"),
                ("BENT_OUTSIDE", "Outer"),
                ("BENT_MIDPLANE", "Middle"),
            ],
        )
        .hints(Hints::SHOW_LABEL),
        ParamSpec::enum_("angleControlType", "Angle control type", ANGLE_CONTROL),
        ParamSpec::bool("oppositeAngle", "Opposite angle", false).hints(Hints::OPPOSITE_DIRECTION_CIRCULAR),
        ParamSpec::num("bendAngle", "Bend angle", NumSpec::angle(90.0, 1.0, max_angle)).visible(move |v| ac(v, "BEND_ANGLE")),
        ParamSpec::query1("parallelEntity", "Parallel to", QueryFilter::DIRECTION).visible(move |v| ac(v, "ALIGN_GEOMETRY")),
        ParamSpec::query1("directionEntity", "Direction", QueryFilter::DIRECTION).visible(move |v| ac(v, "ANGLE_FROM_DIRECTION")),
        ParamSpec::num("angleFromDirection", "Angle", NumSpec::angle(90.0, 0.0, 180.0)).visible(move |v| ac(v, "ANGLE_FROM_DIRECTION")),
        ParamSpec::bool("oppositeAngleFromDirection", "Opposite angle", false)
            .hints(Hints::OPPOSITE_DIRECTION_CIRCULAR)
            .visible(move |v| ac(v, "ANGLE_FROM_DIRECTION")),
    ]
}

pub fn bend() -> FeatureType {
    let mut p = bend_top(359.9);
    p.extend(bend_radius_override(always, "bendRadius"));
    p.extend(k_factor_override(always));
    FeatureType::new("sheetMetalBend", "Bend", Category::SheetMetal, p).icon("bend").describe("Bend a sheet metal wall along a line")
}

/// Jog (sheetMetalJog): two opposite bends offsetting part of a wall.
pub fn jog() -> FeatureType {
    let bt = |v: &ParamValues, t: &str| v.is("offsetType", t);
    let mut p = bend_top(179.0);
    p.push(hidden(ParamSpec::bool("bendAngleSetProgrammatically", "Bend angle set programmatically", false)));
    p.extend(bend_radius_override(always, "bendRadius"));
    p.extend(k_factor_override(always));
    p.extend([
        // sheetMetalJog.fs: both selects SHOW_LABEL (live qa5-sm-dialog-jog).
        ParamSpec::enum_("offsetType", "Bounding type", &[("BLIND", "Blind"), ("UP_TO_ENTITY", "Up to entity"), ("THICKNESS", "Thickness")]).hints(Hints::SHOW_LABEL),
        ParamSpec::num("bendOffset", "Jog offset", ZERO_INCLUSIVE_OFFSET.bounded(0.0, 500.0)).visible(move |v| bt(v, "BLIND")),
        ParamSpec::query1("jogLimit", "Up to entity", QueryFilter::ANY_FACE | QueryFilter::PLANE | QueryFilter::POINT_LIKE | QueryFilter::EDGE)
            .visible(move |v| bt(v, "UP_TO_ENTITY")),
        ParamSpec::bool("jogLimitOffset", "Offset distance", false)
            .hints(Hints::DISPLAY_SHORT | Hints::FIRST_IN_ROW)
            .visible(move |v| bt(v, "UP_TO_ENTITY")),
        ParamSpec::num("jogLimitDistance", "Offset distance", NONNEGATIVE_ZERO_DEFAULT)
            .hints(Hints::DISPLAY_SHORT)
            .visible(move |v| bt(v, "UP_TO_ENTITY") && v.bool("jogLimitOffset")),
        ParamSpec::flip("jogLimitOppositeDirection", "Opposite direction").visible(move |v| bt(v, "UP_TO_ENTITY") && v.bool("jogLimitOffset")),
        ParamSpec::num("thicknessFactor", "Thickness factor", NumSpec::real(3.0, 1e-5, 1e5)).visible(move |v| bt(v, "THICKNESS")),
        ParamSpec::enum_("bendOffsetAnchor", "Jog offset anchor", &[("INSIDE", "Inside"), ("NOMINAL", "Nominal"), ("OUTSIDE", "Outside")]).hints(Hints::SHOW_LABEL),
        ParamSpec::bool("preserveMaterial", "Preserve material", true),
    ]);
    FeatureType::new("sheetMetalJog", "Jog", Category::SheetMetal, p).icon("jog").describe("Offset part of a wall with two opposite bends")
}

pub fn form() -> FeatureType {
    let p = vec![
        // PartStudioData reference (document/element/version/configuration).
        ParamSpec::text("formPartStudio", "Form Part Studio", "").hints(Hints::REMEMBER_PREVIOUS_VALUE),
        ParamSpec::query("locations", "Location(s)", QueryFilter::MATE_CONNECTOR | QueryFilter::SKETCH_POINT).hints(Hints::INITIAL_FOCUS),
        ParamSpec::flip("flipDirection", "Flip direction"),
        ParamSpec::query("targetFaces", "Target face(s)", QueryFilter::PLANAR_FACE),
    ];
    FeatureType::new("sheetMetalFormed", "Form", Category::SheetMetal, p).icon("form").describe("Stamp a form tool onto sheet metal walls")
}

/// A form of the sheet metal form library: its tools from the form's
/// configuration (the built-in form Part Studios, noshape-model-core's
/// `sheetmetal::forms`). Not in the toolbar.
pub fn form_tool() -> FeatureType {
    let p = vec![
        ParamSpec::text("form", "Form", ""),
        ParamSpec::enum_("part", "Part", &[("ADD", "Add material"), ("SUBTRACT", "Subtract material")]),
        ParamSpec::num("thickness", "Thickness", NumSpec::positive_length(1.0, 0.04)),
        ParamSpec::num("diameter", "Diameter", NumSpec::positive_length(5.0, 0.2)),
        ParamSpec::num("height", "Height", NumSpec::positive_length(5.0, 0.2)),
        ParamSpec::angle("draft", "Angle", 90.0),
        ParamSpec::enum_("radiusControl", "Radius control", &[("Default", "Double thickness"), ("Thickness", "Thickness"), ("Half_thickness", "Half thickness"), ("Custom", "Custom")]),
        ParamSpec::num("intRadius", "Internal radius", NumSpec::positive_length(1.0, 0.04)),
        ParamSpec::num("extRadius", "External radius", NumSpec::positive_length(1.0, 0.04)),
    ];
    FeatureType::new("smFormTool", "Form tool", Category::SheetMetal, p).hidden().icon("form").describe("A form of the sheet metal form library")
}

pub fn sheet_metal_loft() -> FeatureType {
    let new = |v: &ParamValues| v.is("surfaceOperationType", "NEW");
    let mut p = vec![
        surface_operation_type(),
        ParamSpec::query(
            "profile1",
            "Profile 1",
            QueryFilter::ANY_EDGE | QueryFilter::ANY_FACE | QueryFilter::WIRE_BODY | QueryFilter::SHEET_BODY | QueryFilter::VERTEX,
        )
        .hints(Hints::INITIAL_FOCUS),
        ParamSpec::query(
            "profile2",
            "Profile 2",
            QueryFilter::ANY_EDGE | QueryFilter::ANY_FACE | QueryFilter::WIRE_BODY | QueryFilter::SHEET_BODY | QueryFilter::VERTEX,
        ),
        ParamSpec::bool("matchConnections", "Connections", false).hints(Hints::GROUP_HEADER),
        ParamSpec::array(
            "connections",
            "Match connections",
            "Connection",
            vec![ParamSpec::query("connectionEntities", "Vertices or edges", QueryFilter::EDGE | QueryFilter::VERTEX), ParamSpec::bool("isRip", "Rip", false)],
        )
        .visible(|v| v.bool("matchConnections")),
        ParamSpec::num("chordalTolerance", "Chordal tolerance", NumSpec::positive_length(1.0, 0.04).bounded(1e-5, 0.1)),
        ParamSpec::query("booleanScope", "Merge scope", QueryFilter::SOLID_BODY).visible(move |v| !new(v)),
    ];
    p.extend(model_parameters(new));
    FeatureType::new("sheetMetalLoft", "Sheet metal loft", Category::SheetMetal, p).describe("Create a faceted sheet metal transition between two profiles")
}

pub fn make_joint() -> FeatureType {
    let bend = |v: &ParamValues| v.is("joint", "BEND");
    let mut p = vec![
        query_n("entities", "Edges or side faces to join", SM_EDGES, 2).hints(Hints::INITIAL_FOCUS),
        ParamSpec::enum_("joint", "Joint type", &[("BEND", "Bend"), ("RIP", "Rip")]).default_enum("RIP"),
    ];
    p.extend(bend_radius_override(bend, "radius"));
    p.push(ParamSpec::enum_("jointType", "Joint style", JOINT_STYLE).visible(move |v| !bend(v)));
    FeatureType::new("sheetMetalMakeJoint", "Make joint", Category::SheetMetal, p).icon("makeJoint").describe("Join two walls with a bend or rip")
}

pub fn modify_joint() -> FeatureType {
    let bend = |v: &ParamValues| v.is("jointType", "BEND");
    let mut p = vec![
        ParamSpec::query1("entity", "Joint", QueryFilter::ANY_FACE | QueryFilter::EDGE).hints(Hints::INITIAL_FOCUS),
        ParamSpec::enum_("jointType", "Joint type", &[("BEND", "Bend"), ("RIP", "Rip"), ("TANGENT", "Tangent")]),
    ];
    p.extend(bend_radius_override(bend, "radius"));
    // sheetMetalJoint.fs: Use model K Factor, else Bend calculation and its value.
    let own = move |v: &ParamValues| bend(v) && !v.bool("useDefaultKFactor");
    let calc = move |v: &ParamValues, t: &str| own(v) && v.is("bendCalculationType", t);
    p.extend([
        ParamSpec::bool("useDefaultKFactor", "Use model K Factor", true).visible(bend),
        ParamSpec::enum_("bendCalculationType", "Bend calculation", &[("K_FACTOR", "K Factor"), ("BEND_ALLOWANCE", "Bend allowance"), ("BEND_DEDUCTION", "Bend deduction")])
            .hints(Hints::SHOW_LABEL)
            .visible(own),
        ParamSpec::num("kFactor", "K Factor", NumSpec::real(0.45, -1.5, 1.0)).visible(move |v| calc(v, "K_FACTOR")),
        ParamSpec::length("bendAllowance", "Bend allowance", 25.0, 1.0).visible(move |v| calc(v, "BEND_ALLOWANCE")),
        ParamSpec::length("bendDeduction", "Bend deduction", 25.0, 1.0).visible(move |v| calc(v, "BEND_DEDUCTION")),
    ]);
    p.push(hidden(ParamSpec::bool("hasStyle", "Has style", true)));
    p.push(ParamSpec::enum_("jointStyle", "Joint style", JOINT_STYLE).visible(|v| v.is("jointType", "RIP") && v.bool("hasStyle")));
    FeatureType::new("sheetMetalJoint", "Modify joint", Category::SheetMetal, p).icon("modifyJoint").describe("Change the type or style of a sheet metal joint")
}

pub fn corner() -> FeatureType {
    let st = |v: &ParamValues, s: &str| v.is("cornerStyle", s);
    let rem = Hints::REMEMBER_PREVIOUS_VALUE;
    let p = vec![
        ParamSpec::query1("corner", "Corner", SM_CORNER).hints(Hints::INITIAL_FOCUS),
        ParamSpec::enum_("cornerStyle", "Corner relief type", CORNER_RELIEF),
        ParamSpec::num("cornerReliefScale", "Corner relief scale", NumSpec::real(1.5, 1.0, 2.0))
            .hints(rem)
            .visible(move |v| st(v, "RECTANGLE") || st(v, "ROUND")),
        ParamSpec::num("roundReliefDiameter", "Corner relief diameter", SM_RELIEF_SIZE).hints(rem).visible(move |v| st(v, "SIZED_ROUND")),
        ParamSpec::num("squareReliefWidth", "Corner relief width", SM_RELIEF_SIZE).hints(rem).visible(move |v| st(v, "SIZED_RECTANGLE")),
    ];
    FeatureType::new("sheetMetalCorner", "Corner", Category::SheetMetal, p).icon("corner").describe("Override the corner relief at one corner")
}

pub fn bend_relief() -> FeatureType {
    let st = |v: &ParamValues, s: &str| v.is("bendReliefStyle", s);
    let scaled = move |v: &ParamValues| st(v, "RECTANGLE") || st(v, "OBROUND");
    let p = vec![
        ParamSpec::query1("bendRelief", "Bend relief", SM_CORNER).hints(Hints::INITIAL_FOCUS),
        ParamSpec::enum_(
            "bendReliefStyle",
            "Bend relief type",
            &[
                ("SIZED_RECTANGLE", "Square - Sized"),
                ("RECTANGLE", "Rectangle - Scaled"),
                ("SIZED_OBROUND", "Obround - Sized"),
                ("OBROUND", "Obround - Scaled"),
                ("TEAR", "Tear"),
            ],
        )
        .default_enum("SIZED_OBROUND"),
        ParamSpec::num("bendReliefDepthScale", "Bend relief depth scale", NumSpec::real(2.0, 1.0, 5.0)).visible(scaled),
        ParamSpec::num("bendReliefWidthScale", "Bend relief width scale", NumSpec::real(1.0625, 0.0625, 2.0)).visible(scaled),
        ParamSpec::num("bendReliefDepth", "Bend relief depth", SM_RELIEF_SIZE).visible(move |v| st(v, "SIZED_RECTANGLE") || st(v, "SIZED_OBROUND")),
        ParamSpec::bool("extendBendRelief", "Extend bend relief", false).visible(move |v| !st(v, "TEAR")),
    ];
    FeatureType::new("sheetMetalBendRelief", "Bend relief", Category::SheetMetal, p)
        .icon("bendRelief")
        .describe("Override the relief where a bend meets a free edge")
}

pub fn corner_break() -> FeatureType {
    let fillet = |v: &ParamValues| v.is("blendType", "FILLET");
    let chamfer = |v: &ParamValues| v.is("blendType", "CHAMFER");
    let radius = move |v: &ParamValues| fillet(v) && v.is("blendControlType", "RADIUS");
    let cs = move |v: &ParamValues, c: &str| fillet(v) && v.is("crossSection", c);
    let mut p = vec![
        ParamSpec::enum_("blendType", "Type", &[("FILLET", "Fillet"), ("CHAMFER", "Chamfer")]).hints(Hints::HORIZONTAL_ENUM | Hints::REMEMBER_PREVIOUS_VALUE),
        ParamSpec::query("entities", "Entities to fillet or chamfer", SM_CORNER).hints(Hints::INITIAL_FOCUS),
    ];
    p.extend(chamfer_options(chamfer));
    p.extend([
        ParamSpec::enum_("blendControlType", "Measurement", &[("RADIUS", "Radius"), ("WIDTH", "Width")])
            .hints(Hints::SHOW_LABEL | Hints::REMEMBER_PREVIOUS_VALUE)
            .visible(fillet),
        ParamSpec::enum_("crossSection", "Control", &[("CIRCULAR", "Distance"), ("CONIC", "Conic"), ("CURVATURE", "Curvature")])
            .hints(Hints::SHOW_LABEL | Hints::REMEMBER_PREVIOUS_VALUE)
            .visible(fillet),
        ParamSpec::num("radius", "Radius", BLEND_BOUNDS).hints(Hints::REMEMBER_PREVIOUS_VALUE).visible(radius),
        ParamSpec::num("filletWidth", "Width", BLEND_BOUNDS).visible(move |v| fillet(v) && !radius(v)),
        ParamSpec::num("rho", "Rho", NumSpec::real(0.5, 0.0, 0.99999)).visible(move |v| cs(v, "CONIC")),
        ParamSpec::num("magnitude", "Magnitude", NumSpec::real(0.5, 0.0, 0.99999)).visible(move |v| cs(v, "CURVATURE")),
        ParamSpec::bool("isAsymmetric", "Asymmetric", false).visible(radius),
        ParamSpec::num("otherRadius", "Second radius", BLEND_BOUNDS).visible(move |v| radius(v) && v.bool("isAsymmetric")),
        ParamSpec::flip("flipAsymmetric", "Flip asymmetric").visible(move |v| radius(v) && v.bool("isAsymmetric")),
        ParamSpec::bool("allowEdgeOverflow", "Allow edge overflow", false).visible(move |v| fillet(v) && !cs(v, "CURVATURE")),
        ParamSpec::query("keepEdges", "Edges to keep", QueryFilter::EDGE).visible(move |v| fillet(v) && !cs(v, "CURVATURE") && v.bool("allowEdgeOverflow")),
    ]);
    FeatureType::new("sheetMetalCornerBreak", "Corner break", Category::SheetMetal, p)
        .icon("cornerBreak")
        .describe("Fillet or chamfer sheet metal corners in the flat and folded model")
}

pub fn sheet_metal_end() -> FeatureType {
    let p = vec![ParamSpec::query("sheetMetalParts", "Sheet metal parts", QueryFilter::SOLID_BODY).hints(Hints::INITIAL_FOCUS)];
    FeatureType::new("sheetMetalEnd", "Finish sheet metal model", Category::SheetMetal, p)
        .icon("finishSheetMetal")
        .describe("Deactivate sheet metal models so later features treat them as solids")
}

/// Internal: rip a wall between vertex pairs.
pub fn rip() -> FeatureType {
    let p = vec![
        ParamSpec::query("vertices", "Vertex pairs to split with", QueryFilter::VERTEX).hints(Hints::ALLOW_QUERY_ORDER),
        ParamSpec::bool("useDefaultGap", "Use default minimal gap", true),
        ParamSpec::num("minimalClearance", "Minimal gap", SM_MINIMAL_CLEARANCE).visible(|v| !v.bool("useDefaultGap")),
    ];
    FeatureType::new("sheetMetalRip", "Rip", Category::SheetMetal, p).hidden().describe("Split a sheet metal wall between vertex pairs")
}

/// Unfold (sheetMetalUnfold): flatten bends in the model.
pub fn unfold() -> FeatureType {
    let all = |v: &ParamValues| v.bool("unfoldAll");
    let p = vec![
        ParamSpec::bool("unfoldAll", "Unfold all", true),
        ParamSpec::query1("partToUnfold", "Sheet metal part", QueryFilter::SOLID_BODY | QueryFilter::ANY_FACE).hints(Hints::INITIAL_FOCUS).visible(all),
        ParamSpec::query("entitiesToUnfold", "Bend or non-planar faces", QueryFilter::FACE | QueryFilter::CYLINDRICAL_FACE).visible(move |v| !all(v)),
        ParamSpec::query1("holdEntity", "Wall face or bounding edge to hold", QueryFilter::PLANAR_FACE | QueryFilter::EDGE),
    ];
    FeatureType::new("sheetMetalUnfold", "Unfold", Category::SheetMetal, p).icon("unfold").describe("Flatten bends of a sheet metal part in place")
}

/// Refold (sheetMetalRefold): fold unfolded bends back.
pub fn refold() -> FeatureType {
    let p = vec![ParamSpec::query("toRefold", "Parts or bends", QueryFilter::SOLID_BODY | QueryFilter::FACE | QueryFilter::CYLINDRICAL_FACE | QueryFilter::PLANAR_FACE | QueryFilter::EDGE)
        .hints(Hints::INITIAL_FOCUS)];
    FeatureType::new("sheetMetalRefold", "Refold", Category::SheetMetal, p).icon("refold").describe("Fold unfolded bends back")
}

/// Internal: recognize sheet metal from constant-thickness bodies.
pub fn recognize() -> FeatureType {
    let p = vec![
        ParamSpec::query("bodies", "Bodies", QueryFilter::BODY),
        ParamSpec::num("radius", "Radius", NONNEGATIVE_LENGTH),
        ParamSpec::bool("changeThickness", "Change thickness", false),
        ParamSpec::num("surfaceThickness", "Thickness", NumSpec::positive_length(1.6, 0.0625).bounded(1e-5, 500.0)).visible(|v| v.bool("changeThickness")),
        ParamSpec::bool("keepInputParts", "Keep input parts", false),
        ParamSpec::num("kFactor", "K Factor", NumSpec::real(1.0, 0.0, 1e5)),
        ParamSpec::num("minimalClearance", "Minimal gap", NumSpec::positive_length(0.02, 0.001).bounded(1e-5, 500.0)),
    ];
    FeatureType::new("sheetMetalRecognize", "Recognize", Category::SheetMetal, p).hidden().describe("Convert a constant-thickness solid into sheet metal")
}
