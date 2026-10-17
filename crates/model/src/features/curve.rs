//! Curve schemas: Helix, Projected curve, Bridging curve, Composite curve,
//! Intersection curve, Isocline, Isoparametric curve, 3D fit spline, Routing
//! curve, Offset curve, Edit curve, Trim curve (03b CV-01..13).

use super::common::*;
use super::{Category, FeatureType};
use crate::params::{Hints, NumSpec, ParamSpec, ParamValues, QueryFilter};

pub fn register(out: &mut Vec<FeatureType>) {
    out.push(helix());
    out.push(fit_spline());
    out.push(projected_curve());
    out.push(bridging_curve());
    out.push(composite_curve());
    out.push(intersection_curve());
    out.push(trim_curve());
    out.push(isocline());
    out.push(offset_curve());
    out.push(isoparametric_curve());
    out.push(edit_curve());
    out.push(routing_curve());
}

const CURVES: QueryFilter = QueryFilter(QueryFilter::ANY_EDGE.0 | QueryFilter::WIRE_BODY.0);
const FACES_OR_BODIES: QueryFilter = QueryFilter(QueryFilter::ANY_FACE.0 | QueryFilter::PLANE.0 | QueryFilter::BODY.0);
const UP_TO_ANY: QueryFilter = QueryFilter(QueryFilter::BODY.0 | QueryFilter::WIRE_BODY.0 | QueryFilter::ANY_FACE.0 | QueryFilter::PLANE.0 | QueryFilter::ANY_EDGE.0 | QueryFilter::POINT_LIKE.0);

pub fn helix() -> FeatureType {
    let axis_ty = |v: &ParamValues, t: &str| v.is("axisType", t);
    let path = |v: &ParamValues, t: &str| v.is("pathType", t);
    let start_angle = |v: &ParamValues| v.is("startType", "START_ANGLE");
    let has_end = move |v: &ParamValues| !path(v, "TURNS_PITCH");
    let height_end = move |v: &ParamValues| has_end(v) && v.is("endType", "HEIGHT");
    let end_point = move |v: &ParamValues| has_end(v) && v.is("endType", "END_POINT");
    let p = vec![
        hidden(ParamSpec::query("initEntities", "Initial entities", QueryFilter::AXIS | QueryFilter::CIRCULAR_EDGE)),
        ParamSpec::enum_("axisType", "Helix axis type", &[("SURFACE", "Cylinder/Cone"), ("AXIS", "Axis"), ("CIRCLE", "Circle")]).hints(Hints::HORIZONTAL_ENUM),
        ParamSpec::query1("entities", "Conical or cylindrical face", QueryFilter::CYLINDRICAL_FACE | QueryFilter::FACE)
            .hints(Hints::INITIAL_FOCUS)
            .visible(move |v| axis_ty(v, "SURFACE")),
        ParamSpec::query1("axis", "Helix axis", QueryFilter::AXIS).visible(move |v| axis_ty(v, "AXIS")),
        ParamSpec::query1("edge", "Circular edge", QueryFilter::CIRCULAR_EDGE).visible(move |v| axis_ty(v, "CIRCLE")),
        ParamSpec::enum_("pathType", "Input type", &[("TURNS", "Turns"), ("PITCH", "Pitch"), ("TURNS_PITCH", "Turns and pitch")]),
        ParamSpec::flip("oppositeDirection", "Opposite direction"),
        ParamSpec::enum_("startType", "Start condition", &[("START_ANGLE", "Start angle"), ("START_POINT", "Start point")]),
        ParamSpec::flip("oppositeOrigin", "Opposite origin").visible(move |v| axis_ty(v, "AXIS")),
        ParamSpec::num("startAngle", "Start angle", angle_360(0.0)).visible(start_angle),
        ParamSpec::num("startRadius", "Start radius", NumSpec::positive_length(25.0, 1.0)).visible(move |v| start_angle(v) && axis_ty(v, "AXIS")),
        ParamSpec::query1("startPoint", "Start point", QueryFilter::VERTEX | QueryFilter::SKETCH_POINT).visible(move |v| !start_angle(v)),
        ParamSpec::enum_("endType", "End condition", &[("HEIGHT", "Height"), ("END_POINT", "End point")]).visible(has_end),
        ParamSpec::num("height", "Height", NONNEGATIVE_LENGTH).visible(move |v| height_end(v) && !axis_ty(v, "SURFACE")),
        ParamSpec::bool("endRadToggle", "End radius", false)
            .hints(Hints::DISPLAY_SHORT | Hints::FIRST_IN_ROW)
            .visible(move |v| height_end(v) && axis_ty(v, "AXIS")),
        ParamSpec::num("endRadius", "End radius", NumSpec::positive_length(25.0, 1.0))
            .hints(Hints::DISPLAY_SHORT)
            .visible(move |v| height_end(v) && axis_ty(v, "AXIS") && v.bool("endRadToggle")),
        ParamSpec::query1("endPoint", "End point", QueryFilter::VERTEX | QueryFilter::SKETCH_POINT).visible(end_point),
        ParamSpec::num("revolutions", "Target revolutions", NumSpec::real(4.0, 1e-5, 1000.0)).visible(move |v| path(v, "TURNS") || path(v, "TURNS_PITCH")),
        ParamSpec::num("actualRevolutions", "Computed revolutions", NumSpec::real(4.0, 0.0, 1e9))
            .hints(Hints::READ_ONLY | Hints::UNCONFIGURABLE)
            .visible(move |v| path(v, "TURNS") && end_point(v)),
        ParamSpec::flip("roundUpTurns", "Flip rounding").visible(move |v| path(v, "TURNS") && end_point(v)),
        ParamSpec::num("helicalPitch", "Target pitch", NONNEGATIVE_LENGTH).visible(move |v| path(v, "PITCH") || path(v, "TURNS_PITCH")),
        ParamSpec::num("actualPitch", "Computed pitch", NumSpec::positive_length(25.0, 1.0))
            .hints(Hints::READ_ONLY | Hints::UNCONFIGURABLE)
            .visible(move |v| path(v, "PITCH") && end_point(v)),
        ParamSpec::flip("roundUpPitch", "Flip rounding").visible(move |v| path(v, "PITCH") && end_point(v)),
        ParamSpec::enum_("handedness", "Handedness", &[("CW", "Clockwise"), ("CCW", "Counterclockwise")]),
        ParamSpec::bool("showStartEndProfiles", "Show start and end profiles", false),
    ];
    FeatureType::new("helix", "Helix", Category::Curve, p).describe("Create a helix around a cylinder, cone, axis or circle")
}

pub fn fit_spline() -> FeatureType {
    let verts = |v: &ParamValues| v.is("fitType", "VERTICES");
    let open = move |v: &ParamValues| verts(v) && !v.bool("closed");
    let mut p = vec![
        hidden(ParamSpec::query("initEntities", "Entities", QueryFilter::VERTEX | QueryFilter::EDGE)),
        ParamSpec::enum_("fitType", "Approximation type", &[("VERTICES", "Vertices"), ("EDGES", "Edges")]).hints(Hints::HORIZONTAL_ENUM),
        ParamSpec::query("vertices", "Vertices", QueryFilter::VERTEX | QueryFilter::SKETCH_POINT)
            .hints(Hints::ALLOW_QUERY_ORDER | Hints::INITIAL_FOCUS)
            .visible(verts),
        ParamSpec::bool("closed", "Closed spline", false).visible(verts),
    ];
    for (dir, mag, flip, curv, labels) in [
        ("startDirection", "startMagnitude", "oppositeDirectionStart", "matchStartCurvature", ["Start direction", "Start magnitude", "Match curvature at start"]),
        ("endDirection", "endMagnitude", "oppositeDirectionEnd", "matchEndCurvature", ["End direction", "End magnitude", "Match curvature at end"]),
    ] {
        p.push(ParamSpec::query1(dir, labels[0], QueryFilter::DIRECTION | QueryFilter::EDGE).visible(open));
        p.push(ParamSpec::num(mag, labels[1], CLAMP_MAGNITUDE).visible(open));
        p.push(ParamSpec::flip(flip, "Opposite direction").visible(open));
        p.push(ParamSpec::bool(curv, labels[2], false).visible(open));
    }
    p.push(hidden(ParamSpec::bool("hasStartDirection", "Has start direction", false)));
    p.push(hidden(ParamSpec::bool("hasEndDirection", "Has end direction", false)));
    p.push(ParamSpec::query("edges", "Edges", QueryFilter::EDGE).visible(move |v| !verts(v)));
    p.extend(curve_approximation(always));
    FeatureType::new("fitSpline", "3D fit spline", Category::Curve, p).describe("Create a 3D spline through vertices or along a tangent edge chain")
}

pub fn projected_curve() -> FeatureType {
    let two = |v: &ParamValues| v.is("curveProjectionType", "TWO_SKETCHES");
    let dir = move |v: &ParamValues| !two(v) && v.is("projectionType", "DIRECTION");
    let mut p = vec![
        hidden(ParamSpec::query("preselectedEntities", "Preselection", QueryFilter::ANY_EDGE)),
        ParamSpec::enum_("curveProjectionType", "Curve projection type", &[("TWO_SKETCHES", "Two sketches"), ("CURVE_TO_FACE", "Curve to face")])
            .hints(Hints::HORIZONTAL_ENUM | Hints::REMEMBER_PREVIOUS_VALUE),
        ParamSpec::query("sketchEdges1", "First sketch", QueryFilter::SKETCH_EDGE).hints(Hints::INITIAL_FOCUS).visible(two),
        ParamSpec::query("sketchEdges2", "Second sketch", QueryFilter::SKETCH_EDGE).visible(two),
        ParamSpec::query("dropTools", "Edges", QueryFilter::ANY_EDGE).visible(move |v| !two(v)),
        ParamSpec::enum_("projectionType", "Projection direction type", &[("DIRECTION", "Direction"), ("NORMAL_TO_TARGET", "Normal to target")])
            .visible(move |v| !two(v)),
        ParamSpec::query1("directionQuery", "Direction", QueryFilter::DIRECTION).visible(dir),
        ParamSpec::flip("oppositeDirection", "Opposite direction").visible(dir),
        ParamSpec::query("targets", "Targets", FACES_OR_BODIES).visible(move |v| !two(v)),
    ];
    p.extend(curve_approximation(always));
    FeatureType::new("projectCurves", "Projected curve", Category::Curve, p)
        .icon("projectedCurve")
        .describe("Create a curve from two sketch projections or by projecting edges onto faces")
}

pub const BRIDGING_MATCH: &[(&str, &str)] =
    &[("POSITION", "Match position"), ("TANGENCY", "Match tangent"), ("CURVATURE", "Match curvature"), ("G3", "Match flow")];

pub fn bridging_curve() -> FeatureType {
    let rem = Hints::REMEMBER_PREVIOUS_VALUE;
    let non_pos = |v: &ParamValues, side: &str| !v.is(side, "POSITION");
    let any_non_pos = move |v: &ParamValues| non_pos(v, "match1") || non_pos(v, "match2");
    let any_g3 = |v: &ParamValues| v.is("match1", "G3") || v.is("match2", "G3");
    let cp_method = move |v: &ParamValues| v.is("method", "CONTROL_POINTS") || any_g3(v);
    let edit_cp = move |v: &ParamValues| cp_method(v) && any_non_pos(v) && v.bool("editControlPoints");
    let curv = |v: &ParamValues, side: &str| v.is(side, "CURVATURE") || v.is(side, "G3");
    let match_opt = |id: &'static str, label: &'static str| ParamSpec::enum_(id, label, BRIDGING_MATCH).default_enum("TANGENCY").hints(rem);
    let mut p = vec![
        hidden(ParamSpec::query("preselectedEntities", "Preselection", QueryFilter::ANY_EDGE | QueryFilter::VERTEX)),
        hidden(ParamSpec::bool("side1HasVertex", "Start has vertex", false)),
        hidden(ParamSpec::bool("side2HasVertex", "End has vertex", false)),
        hidden(ParamSpec::bool("side1HasFace", "Start has face", false)),
        hidden(ParamSpec::bool("side2HasFace", "End has face", false)),
        query_n("side1", "Start", QueryFilter::EDGE | QueryFilter::POINT_LIKE | QueryFilter::ANY_FACE, 2).hints(Hints::INITIAL_FOCUS),
        match_opt("match1", "Match"),
        ParamSpec::flip("flip1", "Opposite direction").visible(move |v| non_pos(v, "match1")),
        query_n("side2", "End", QueryFilter::EDGE | QueryFilter::POINT_LIKE | QueryFilter::ANY_FACE, 2),
        match_opt("match2", "Match"),
        ParamSpec::flip("flip2", "Opposite direction").visible(move |v| non_pos(v, "match2")),
        ParamSpec::enum_("method", "Method", &[("MAGNITUDE_BIAS", "Magnitude/bias"), ("CONTROL_POINTS", "Control points")])
            .default_enum("CONTROL_POINTS")
            .hints(rem)
            .visible(move |v| any_non_pos(v) && !any_g3(v)),
        ParamSpec::bool("editControlPoints", "Edit control points", false)
            .hints(rem | Hints::GROUP_HEADER)
            .visible(move |v| cp_method(v) && any_non_pos(v)),
    ];
    for (side, mag, curv_off, g3, labels) in [
        ("match1", "side1Magnitude", "side1CurvatureOffset", "side1G3Offset", ["Start magnitude", "Start curvature offset", "Start flow offset"]),
        ("match2", "side2Magnitude", "side2CurvatureOffset", "side2G3Offset", ["End magnitude", "End curvature offset", "End flow offset"]),
    ] {
        p.push(ParamSpec::num(mag, labels[0], NumSpec::real(1.0, 0.0, 1e5)).visible(move |v| edit_cp(v) && non_pos(v, side)));
        p.push(ParamSpec::num(curv_off, labels[1], CLAMP_MAGNITUDE).visible(move |v| edit_cp(v) && curv(v, side)));
        p.push(ParamSpec::num(g3, labels[2], CLAMP_MAGNITUDE).visible(move |v| edit_cp(v) && v.is(side, "G3")));
    }
    p.extend([
        ParamSpec::num("magnitude", "Magnitude", NumSpec::real(1.0, 0.0, 1e5)).visible(move |v| !cp_method(v) && any_non_pos(v)),
        ParamSpec::num("bias", "Bias", NumSpec::real(0.5, 1e-4, 0.9999))
            .visible(move |v| !cp_method(v) && non_pos(v, "match1") && non_pos(v, "match2")),
        ParamSpec::bool("editEdgePositions", "Edit edge positions", false)
            .hints(rem | Hints::GROUP_HEADER)
            .visible(|v| !v.bool("side1HasVertex") || !v.bool("side2HasVertex")),
        ParamSpec::num("startEdgeParameter", "Start edge position", EDGE_PARAMETER)
            .visible(|v| v.bool("editEdgePositions") && !v.bool("side1HasVertex")),
        ParamSpec::num("endEdgeParameter", "End edge position", EDGE_PARAMETER)
            .visible(|v| v.bool("editEdgePositions") && !v.bool("side2HasVertex")),
        ParamSpec::bool("editTangencyAngles", "Edit tangency angles", false)
            .hints(rem | Hints::GROUP_HEADER)
            .visible(move |v| (v.bool("side1HasFace") || v.bool("side2HasFace")) && any_non_pos(v)),
        ParamSpec::num("startAngle", "Start angle", NumSpec::angle(0.0, -180.0, 180.0))
            .visible(move |v| v.bool("editTangencyAngles") && v.bool("side1HasFace") && non_pos(v, "match1")),
        ParamSpec::num("endAngle", "End angle", NumSpec::angle(0.0, -180.0, 180.0))
            .visible(move |v| v.bool("editTangencyAngles") && v.bool("side2HasFace") && non_pos(v, "match2")),
    ]);
    FeatureType::new("bridgingCurve", "Bridging curve", Category::Curve, p).describe("Connect two locations with a G0-G3 continuous curve")
}

pub fn composite_curve() -> FeatureType {
    let mut p = vec![ParamSpec::query("edges", "Edges", CURVES).hints(Hints::INITIAL_FOCUS)];
    p.extend(curve_approximation(always));
    FeatureType::new("compositeCurve", "Composite curve", Category::Curve, p).describe("Combine connected edges into one curve")
}

pub fn intersection_curve() -> FeatureType {
    let mut p = vec![
        ParamSpec::query("group1", "Group 1", FACES_OR_BODIES).hints(Hints::INITIAL_FOCUS),
        ParamSpec::query("group2", "Group 2", FACES_OR_BODIES),
    ];
    p.extend(curve_approximation(always));
    FeatureType::new("intersectionCurve", "Intersection curve", Category::Curve, p).describe("Create curves where two groups of faces or bodies intersect")
}

pub fn trim_curve() -> FeatureType {
    let rem = Hints::REMEMBER_PREVIOUS_VALUE;
    let extend = |v: &ParamValues| v.is("moveBoundaryType", "EXTEND");
    let p = vec![
        ParamSpec::enum_("moveBoundaryType", "Move curve boundary type", &[("TRIM", "Trim"), ("EXTEND", "Extend")]).hints(Hints::HORIZONTAL_ENUM | rem),
        ParamSpec::query("wires", "Curves to adjust", QueryFilter::WIRE_BODY).hints(Hints::INITIAL_FOCUS),
        ParamSpec::enum_("endCondition", "End condition", &[("BLIND", "Blind"), ("UP_TO_ENTITY", "Up to entity")]).hints(rem).visible(extend),
        ParamSpec::num("extensionDistance", "Distance", NONNEGATIVE_LENGTH).visible(move |v| extend(v) && v.is("endCondition", "BLIND")),
        ParamSpec::query("extendTo", "Up to entity", UP_TO_ANY).visible(move |v| extend(v) && v.is("endCondition", "UP_TO_ENTITY")),
        ParamSpec::enum_("extensionShape", "Extension shape", &[("CURVATURE", "Curvature"), ("TANGENT", "Tangent")]).hints(rem).visible(extend),
        ParamSpec::query("trimTo", "Up to entity", UP_TO_ANY).visible(move |v| !extend(v)),
        ParamSpec::query1("helpPoint", "Help point", QueryFilter::VERTEX | QueryFilter::SKETCH_POINT | QueryFilter::MATE_CONNECTOR),
        ParamSpec::flip("flipHeuristics", "Opposite direction"),
    ];
    FeatureType::new("trimCurve", "Trim curve", Category::Curve, p).describe("Trim or extend curves")
}

pub fn isocline() -> FeatureType {
    let p = vec![
        ParamSpec::query("faces", "Faces", QueryFilter::ANY_FACE).hints(Hints::INITIAL_FOCUS),
        ParamSpec::query1("direction", "Direction", QueryFilter::DIRECTION),
        ParamSpec::flip("oppositeDirection", "Opposite direction"),
        ParamSpec::num("angle", "Angle", NumSpec::angle(3.0, 0.0, 89.9)),
        ParamSpec::bool("split", "Split faces", false),
    ];
    FeatureType::new("isocline", "Isocline", Category::Curve, p).describe("Create curves where faces reach a given angle to a direction")
}

pub fn offset_curve() -> FeatureType {
    let extend = |v: &ParamValues| v.is("scope", "OFFSET_EXTEND");
    let trim = move |v: &ParamValues| extend(v) && v.bool("trim");
    let mut p = vec![
        ParamSpec::query("edges", "Edges", QueryFilter::EDGE).hints(Hints::INITIAL_FOCUS),
        ParamSpec::num("distance", "Distance", NONNEGATIVE_LENGTH),
        ParamSpec::enum_("offsetType", "Offset type", &[("GEODESIC", "Geodesic"), ("EUCLIDEAN", "Euclidean")]),
        ParamSpec::flip("oppositeDirection", "Flip"),
        ParamSpec::enum_("gapFill", "Gap fill", &[("LINEAR", "Linear"), ("ROUND", "Round")]),
        ParamSpec::enum_(
            "scope",
            "Scope",
            &[("OFFSET", "Offset"), ("OFFSET_EXTEND", "Offset and extend"), ("OFFSET_EXTEND_SPLIT", "Offset, extend, and split")],
        ),
        ParamSpec::bool("trim", "Trim", false).hints(Hints::GROUP_HEADER).visible(extend),
        ParamSpec::num("startTrim", "Start trim", NONNEGATIVE_ZERO_DEFAULT).visible(trim),
        ParamSpec::bool("equalTrim", "Equal trim", false).visible(trim),
        ParamSpec::num("endTrim", "End trim", NONNEGATIVE_ZERO_DEFAULT).visible(move |v| trim(v) && !v.bool("equalTrim")),
        ParamSpec::query("targets", "Targets", QueryFilter::ANY_FACE),
    ];
    p.extend(curve_approximation(|v: &ParamValues| !v.is("scope", "OFFSET_EXTEND_SPLIT")));
    FeatureType::new("offsetCurveOnFace", "Offset curve", Category::Curve, p)
        .icon("offsetCurve")
        .describe("Create curves on faces offset from edges")
}

pub fn isoparametric_curve() -> FeatureType {
    let p = vec![
        ParamSpec::query1("face", "Select face", QueryFilter::ANY_FACE).hints(Hints::INITIAL_FOCUS),
        ParamSpec::enum_("directionType", "Direction", &[("U", "U direction"), ("V", "V direction")]),
        ParamSpec::bool("equalSpacing", "Equal spacing", false),
        ParamSpec::num("nCurves", "Instance count", NumSpec::integer(10, 2, 50)).visible(|v| v.bool("equalSpacing")),
        ParamSpec::array(
            "uvParamList",
            "Position",
            "Curve",
            vec![
                ParamSpec::bool("selectPoint", "Select point", false),
                ParamSpec::query1("point", "Select point", QueryFilter::VERTEX | QueryFilter::SKETCH_POINT | QueryFilter::MATE_CONNECTOR)
                    .visible(|v| v.bool("selectPoint")),
                ParamSpec::num("uvValue", "Location", EDGE_PARAMETER).visible(|v| !v.bool("selectPoint")),
            ],
        )
        .visible(|v| !v.bool("equalSpacing")),
    ];
    FeatureType::new("isoparametricCurve", "Isoparametric curve", Category::Curve, p).describe("Create curves along a face's U or V direction")
}

pub fn edit_curve() -> FeatureType {
    let appr = |v: &ParamValues| v.bool("approximate");
    let cps = |v: &ParamValues| v.bool("editControlPoints");
    let p = vec![
        ParamSpec::query("wire", "Curves", CURVES).hints(Hints::INITIAL_FOCUS),
        ParamSpec::bool("approximate", "Approximate", false).hints(Hints::GROUP_HEADER),
        ParamSpec::num("approximationDegree", "Target degree", NumSpec::integer(3, 2, 15)).visible(appr),
        ParamSpec::num("approximationMaxCPs", "Maximum control points", NumSpec::integer(15, 4, 100)).visible(appr),
        ParamSpec::num("approximationTolerance", "Tolerance", TOLERANCE_BOUND).visible(appr),
        ParamSpec::bool("keepStartDerivative", "Keep start derivative", false).visible(appr),
        ParamSpec::bool("keepEndDerivative", "Keep end derivative", false).visible(appr),
        ParamSpec::bool("elevate", "Elevate", false).hints(Hints::GROUP_HEADER),
        ParamSpec::num("elevationDegree", "Target degree", NumSpec::integer(3, 2, 15)).visible(|v| v.bool("elevate")),
        ParamSpec::bool("planarize", "Planarize", false).hints(Hints::GROUP_HEADER),
        ParamSpec::enum_(
            "planeReference",
            "Reference plane",
            &[("BEST", "Best fit"), ("YZ", "YZ plane"), ("XZ", "XZ plane"), ("XY", "XY plane"), ("CUSTOM", "Custom")],
        )
        .visible(|v| v.bool("planarize")),
        ParamSpec::query1("referencePlane", "Entity or mate connector", QueryFilter::PLANE_LIKE)
            .visible(|v| v.bool("planarize") && v.is("planeReference", "CUSTOM")),
        ParamSpec::bool("lockEnds", "Lock ends", false).visible(|v| v.bool("planarize") && v.is("planeReference", "BEST")),
        ParamSpec::bool("editControlPoints", "Edit control points", false).hints(Hints::GROUP_HEADER),
        ParamSpec::enum_(
            "editPointMode",
            "Edit mode",
            &[("XYZ", "XYZ"), ("UVN_TANGENT", "UVN (Tangent)"), ("UVN_POLYGON", "UVN (Control polygon)")],
        )
        .visible(cps),
        ParamSpec::array(
            "selectedIndices",
            "Control point indices",
            "Index",
            vec![ParamSpec::num("indexValue", "Control point index", NumSpec::integer(0, 0, 99))],
        )
        .visible(move |v| cps(v) && v.is("editPointMode", "XYZ")),
        ParamSpec::num("selectedIndex", "Control point index", NumSpec::integer(0, 0, 99)).visible(move |v| cps(v) && !v.is("editPointMode", "XYZ")),
        ParamSpec::array(
            "controlPointEdits",
            "Points overrides",
            "Point override",
            vec![
                ParamSpec::num("index", "Control point index", NumSpec::integer(0, 0, 99)),
                ParamSpec::query1("reference", "Reference", QueryFilter::POINT_LIKE),
                ParamSpec::num("x", "X offset", ZERO_DEFAULT_LENGTH),
                ParamSpec::num("y", "Y offset", ZERO_DEFAULT_LENGTH),
                ParamSpec::num("z", "Z offset", ZERO_DEFAULT_LENGTH),
                ParamSpec::num("weight", "Weight", NumSpec::real(1.0, 1e-5, 1e5)),
            ],
        )
        .hints(Hints::COLLAPSE_ARRAY_ITEMS)
        .visible(cps),
        ParamSpec::bool("showDetails", "Show details", true),
        ParamSpec::num("curveDegree", "Degree", NumSpec::integer(3, 0, 1000)).hints(Hints::READ_ONLY | Hints::UNCONFIGURABLE).visible(|v| v.bool("showDetails")),
        ParamSpec::num("curveNumCPs", "Control points", NumSpec::integer(4, 0, 100_000))
            .hints(Hints::READ_ONLY | Hints::UNCONFIGURABLE)
            .visible(|v| v.bool("showDetails")),
        ParamSpec::num("curveNumSpans", "Spans", NumSpec::integer(1, 0, 100_000))
            .hints(Hints::READ_ONLY | Hints::UNCONFIGURABLE)
            .visible(|v| v.bool("showDetails")),
        ParamSpec::bool("approximationShowDeviation", "Show deviation", false),
        ParamSpec::num("maxDeviation", "Maximum deviation", NumSpec::positive_length(0.0, 0.0))
            .hints(Hints::READ_ONLY | Hints::UNCONFIGURABLE)
            .visible(|v| v.bool("approximationShowDeviation")),
    ];
    FeatureType::new("editCurve", "Edit curve", Category::Curve, p).describe("Approximate, elevate, planarize or edit control points of a curve")
}

pub const LENGTH_UNIT_NAMES: &[(&str, &str)] = &[
    ("Meter", "Meter"),
    ("Centimeter", "Centimeter"),
    ("Millimeter", "Millimeter"),
    ("Inch", "Inch"),
    ("Foot", "Foot"),
    ("Yard", "Yard"),
];

pub fn routing_curve() -> FeatureType {
    let spline = |v: &ParamValues| v.is("curveType", "INTERPOLATED_SPLINE");
    let points = |v: &ParamValues| v.is("step", "POINTS");
    let input = move |v: &ParamValues, t: &str| points(v) && v.is("inputType", t);
    let single = move |v: &ParamValues| points(v) && v.is("pointsEditType", "SINGLE");
    let segments = |v: &ParamValues| v.is("step", "SEGMENTS");
    let ortho = move |v: &ParamValues| segments(v) && v.is("segmentEditType", "ORTHOGONAL_PATH");
    let reference = |v: &ParamValues, t: &str| v.is("referenceType", t);
    let p = vec![
        hidden(ParamSpec::query("preselectedEntities", "Preselection", CURVES | QueryFilter::POINT_LIKE)),
        hidden(ParamSpec::num("curveLength", "Curve length", NumSpec::positive_length(0.0, 0.0))),
        ParamSpec::enum_("curveType", "Curve type", &[("INTERPOLATED_SPLINE", "Interpolated spline"), ("POLYLINE", "Polyline")]),
        ParamSpec::num("bendRadius", "Bend radius", NONNEGATIVE_ZERO_DEFAULT).visible(move |v| !spline(v)),
        ParamSpec::bool("hasTargetLength", "Target length", false).hints(Hints::DISPLAY_SHORT | Hints::FIRST_IN_ROW).visible(spline),
        ParamSpec::num("targetLength", "Target length", NONNEGATIVE_LENGTH)
            .hints(Hints::DISPLAY_SHORT)
            .visible(move |v| spline(v) && v.bool("hasTargetLength")),
        ParamSpec::enum_("step", "Step", &[("POINTS", "Points"), ("SEGMENTS", "Segments")]).hints(Hints::HORIZONTAL_ENUM | Hints::UNCONFIGURABLE),
        // Points step: inputs.
        ParamSpec::enum_("inputType", "Input type", &[("VERTEX", "Vertex"), ("CURVE", "Curve"), ("CSV", "CSV file")]).visible(points),
        ParamSpec::query("inputVertices", "Vertices", QueryFilter::POINT_LIKE).visible(move |v| input(v, "VERTEX")),
        ParamSpec::query("inputCurve", "Edges", CURVES).visible(move |v| input(v, "CURVE")),
        ParamSpec::num("samples", "Number of samples", NumSpec::integer(5, 2, 50)).visible(move |v| input(v, "CURVE")),
        ParamSpec::text("inputData", "Table", "").hints(Hints::UNCONFIGURABLE).visible(move |v| input(v, "CSV")),
        ParamSpec::num("startColumn", "Data start column (0,1,2,.)", NumSpec::integer(0, 0, 100_000)).visible(move |v| input(v, "CSV")),
        ParamSpec::enum_("csvUnit", "Unit", LENGTH_UNIT_NAMES).visible(move |v| input(v, "CSV")),
        ParamSpec::bool("overwritePoints", "Overwrite points", true).visible(move |v| input(v, "CURVE") || input(v, "CSV")),
        ParamSpec::bool("isPeriodic", "Closed", false).visible(points),
        ParamSpec::query1("curveBaseCSys", "Curve reference", QueryFilter::POINT_LIKE).visible(points),
        ParamSpec::enum_("pointsEditType", "Point edit type", &[("SINGLE", "Single"), ("MULTIPLE", "Multiple")]).hints(Hints::HORIZONTAL_ENUM).visible(points),
        ParamSpec::query1("multiEditReference", "Edit reference", QueryFilter::POINT_LIKE).visible(move |v| points(v) && !single(v)),
        ParamSpec::array("selectedIndices", "Selected points", "Point", vec![ParamSpec::num("indexValue", "Point index", NumSpec::integer(0, 0, 10_000))])
            .visible(move |v| points(v) && !single(v)),
        hidden(ParamSpec::num("pointIndex", "Selected point", NumSpec::integer(0, 0, 10_000))),
        // Item visibility below only sees the item's own values; the
        // polyline/spline-only fields are therefore always shown in items.
        ParamSpec::array(
            "points",
            "Points",
            "Point",
            vec![
                ParamSpec::enum_("referenceType", "Reference", &[("ORIGIN", "Origin"), ("VERTEX", "Vertex"), ("CURVE", "Curve"), ("RELATIVE", "Relative")]),
                ParamSpec::query1("vertex", "Vertex", QueryFilter::POINT_LIKE).visible(move |v| reference(v, "VERTEX")),
                ParamSpec::query("curve", "Edges", CURVES).visible(move |v| reference(v, "CURVE")),
                ParamSpec::num("curveParameter", "Parameter", EDGE_PARAMETER).visible(move |v| reference(v, "CURVE")),
                ParamSpec::bool("alignCSys", "Align coordinate system", false).visible(move |v| reference(v, "CURVE")),
                ParamSpec::flip("flipCSys", "Flip coordinate system").visible(move |v| reference(v, "CURVE") && v.bool("alignCSys")),
                ParamSpec::num("dx", "X offset", ZERO_DEFAULT_LENGTH),
                ParamSpec::num("dy", "Y offset", ZERO_DEFAULT_LENGTH),
                ParamSpec::num("dz", "Z offset", ZERO_DEFAULT_LENGTH),
                ParamSpec::bool("overrideBendRadius", "Override bend radius", false),
                ParamSpec::num("pointBendRadius", "Bend radius", NONNEGATIVE_ZERO_DEFAULT).visible(|v| v.bool("overrideBendRadius")),
                ParamSpec::bool("hasDerivative", "Derivative", false),
                ParamSpec::enum_(
                    "derivativeAlignment",
                    "Derivative alignment",
                    &[("X", "Manipulator X axis"), ("Y", "Manipulator Y axis"), ("Z", "Manipulator Z axis"), ("DIRECTION", "Direction")],
                )
                .visible(|v| v.bool("hasDerivative")),
                ParamSpec::query1("derivativeDirection", "Derivative direction", QueryFilter::DIRECTION)
                    .visible(|v| v.bool("hasDerivative") && v.is("derivativeAlignment", "DIRECTION")),
                ParamSpec::num("magnitude", "Magnitude", CLAMP_MAGNITUDE).visible(|v| v.bool("hasDerivative")),
                ParamSpec::flip("oppositeDirection", "Opposite direction").visible(|v| v.bool("hasDerivative")),
            ],
        )
        .visible(single),
        ParamSpec::bool("addPointOnAxisDrag", "Add point on axis drag", false).visible(single),
        ParamSpec::bool("showCSys", "Show reference coordinate system", true).visible(points),
        ParamSpec::bool("keepPoints", "Keep points", false).visible(points),
        // Segments step.
        hidden(ParamSpec::num("segmentIndex", "Segment index", NumSpec::integer(-1, -1, 10_000))),
        ParamSpec::enum_("segmentEditType", "Segment edit type", &[("ADD_POINTS", "Add points"), ("ORTHOGONAL_PATH", "Orthogonal path")]).visible(segments),
        ParamSpec::num("numPointsToAdd", "Number of points to add", NumSpec::integer(1, 0, 10_000))
            .visible(move |v| segments(v) && v.is("segmentEditType", "ADD_POINTS")),
        ParamSpec::enum_("orthoCsys", "Coordinate system", &[("WORLD", "World"), ("CURVE", "Curve"), ("OTHER", "Other")]).visible(ortho),
        ParamSpec::query1("orthoCsysMate", "Orthogonal path coordinate system", QueryFilter::POINT_LIKE).visible(move |v| ortho(v) && v.is("orthoCsys", "OTHER")),
        hidden(ParamSpec::num("solutionIndex", "Solution index", NumSpec::integer(0, 0, 5))),
    ];
    FeatureType::new("routingCurve", "Routing curve", Category::Curve, p).describe("Build a 3D spline or polyline path from placed points")
}
