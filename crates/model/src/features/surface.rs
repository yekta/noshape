//! Surface schemas: Offset surface, Fill, Move boundary, Boundary surface,
//! Ruled surface, Mutual trim, Constrained surface
//! (03b-surfaces-curves-ref.md SF-03..10). Face blend is in `detail.rs`,
//! Thicken / Enclose in `solid.rs`.

use super::common::*;
use super::solid::{LOFT_END_CONDITION, is_direction_condition, is_match_condition};
use super::{Category, FeatureType};
use crate::params::{Hints, NumSpec, ParamSpec, ParamValues, QueryFilter};

pub fn register(out: &mut Vec<FeatureType>) {
    out.push(offset_surface());
    out.push(fill());
    out.push(move_boundary());
    out.push(boundary_surface());
    out.push(ruled_surface());
    out.push(mutual_trim());
    out.push(constrained_surface());
}

const CURVES: QueryFilter = QueryFilter(QueryFilter::ANY_EDGE.0 | QueryFilter::WIRE_BODY.0);

pub fn offset_surface() -> FeatureType {
    let p = vec![
        ParamSpec::query(
            "surfacesAndFaces",
            "Faces, surfaces, and sketch regions to offset",
            QueryFilter::ANY_FACE | QueryFilter::SKETCH_REGION | QueryFilter::SHEET_BODY,
        )
        .hints(Hints::INITIAL_FOCUS),
        ParamSpec::num("offset", "Offset", ZERO_INCLUSIVE_OFFSET).hints(Hints::REMEMBER_PREVIOUS_VALUE),
        ParamSpec::flip("oppositeDirection", "Opposite offset direction"),
    ];
    FeatureType::new("offsetSurface", "Offset surface", Category::Surface, p).describe("Create a surface offset from faces, surfaces or sketch regions")
}

pub fn fill() -> FeatureType {
    let mut p = vec![
        hidden(ParamSpec::query("preselectedEntities", "Preselection", CURVES)),
        surface_operation_type(),
        ParamSpec::array(
            "edges",
            "Edges",
            "Edge",
            vec![
                ParamSpec::query("entities", "Edges", CURVES),
                ParamSpec::enum_("continuity", "Continuity", &[("G0", "Position"), ("G1", "Tangency"), ("G2", "Curvature")])
                    .hints(Hints::MATCH_LAST_ARRAY_ITEM),
                ParamSpec::query("adjacentFaces", "Adjacent faces", QueryFilter::ANY_FACE).visible(|v| !v.is("continuity", "G0")),
            ],
        )
        .hints(Hints::INITIAL_FOCUS),
        ParamSpec::bool("addGuides", "Guides", false).hints(Hints::GROUP_HEADER),
        ParamSpec::query("guideEntities", "Vertices or curves", QueryFilter::VERTEX | CURVES).visible(|v| v.bool("addGuides")),
        ParamSpec::enum_("constraintMode", "Mode", &[("PRECISE", "Precise"), ("SAMPLED", "Sampled")]).visible(|v| v.bool("addGuides")),
        ParamSpec::num("sampleSize", "Sample size", NumSpec::integer(5, 1, 50)).visible(|v| v.bool("addGuides") && v.is("constraintMode", "SAMPLED")),
        ParamSpec::bool("showIsocurves", "Show isocurves", false),
        ParamSpec::num("curveCount", "Count", ISO_GRID).visible(|v| v.bool("showIsocurves")),
    ];
    p.extend(surface_merge_scope());
    FeatureType::new("fill", "Fill", Category::Surface, p).describe("Create a surface bounded by a closed loop of edges")
}

pub fn move_boundary() -> FeatureType {
    let cond = |v: &ParamValues, c: &str| v.is("endCondition", c);
    let up_to = move |v: &ParamValues| !cond(v, "BLIND");
    let p = vec![
        ParamSpec::query("entities", "Surface or boundary edges", QueryFilter::SHEET_BODY | QueryFilter::EDGE).hints(Hints::INITIAL_FOCUS),
        ParamSpec::bool("tangentPropagation", "Tangent propagation", true),
        ParamSpec::enum_(
            "endCondition",
            "Move end condition",
            &[("BLIND", "Blind"), ("UP_TO_FACE", "Up to face"), ("UP_TO_BODY", "Up to part/surface"), ("UP_TO_VERTEX", "Up to vertex")],
        ),
        ParamSpec::flip("oppositeDirection", "Opposite direction").visible(move |v| cond(v, "BLIND")),
        ParamSpec::length("extendDistance", "Distance", 25.0, 1.0).visible(move |v| cond(v, "BLIND")),
        ParamSpec::query1("targetPart", "Target", QueryFilter::BODY).visible(move |v| cond(v, "UP_TO_BODY")),
        ParamSpec::query1("targetFace", "Target", QueryFilter::ANY_FACE | QueryFilter::PLANE | QueryFilter::MATE_CONNECTOR)
            .visible(move |v| cond(v, "UP_TO_FACE")),
        ParamSpec::query1("targetVertex", "Target", QueryFilter::POINT_LIKE).visible(move |v| cond(v, "UP_TO_VERTEX")),
        ParamSpec::bool("hasOffset", "Offset distance", false).hints(Hints::DISPLAY_SHORT | Hints::FIRST_IN_ROW).visible(up_to),
        ParamSpec::length("offset", "Offset distance", 25.0, 1.0).hints(Hints::DISPLAY_SHORT).visible(move |v| up_to(v) && v.bool("hasOffset")),
        ParamSpec::flip("offsetOppositeDirection", "Opposite direction").visible(move |v| up_to(v) && v.bool("hasOffset")),
        ParamSpec::bool("maintainCurvature", "Maintain curvature", false),
    ];
    FeatureType::new("extendSurface", "Move boundary", Category::Surface, p).icon("moveBoundary").describe("Extend or trim surface boundary edges")
}

/// One U or V profile row of Boundary surface (fields prefixed `u` / `v`).
fn bsurf_profiles(id: &'static str, label: &'static str, ids: [&'static str; 5]) -> ParamSpec {
    let [entities, cond, mag, reference, direction] = ids;
    ParamSpec::array(
        id,
        label,
        "Profile",
        vec![
            ParamSpec::query(entities, "Edges, curves and sketches", CURVES),
            ParamSpec::enum_(cond, "Boundary condition", LOFT_END_CONDITION),
            ParamSpec::num(mag, "Magnitude", CLAMP_MAGNITUDE).visible(move |v| !v.is(cond, "DEFAULT")),
            ParamSpec::query(reference, "Faces", QueryFilter::ANY_FACE).visible(move |v| is_match_condition(v.enum_(cond))),
            ParamSpec::query1(direction, "Direction", QueryFilter::DIRECTION).visible(move |v| is_direction_condition(v.enum_(cond))),
        ],
    )
    .hints(Hints::COLLAPSE_ARRAY_ITEMS)
}

pub fn boundary_surface() -> FeatureType {
    let mut p = vec![
        surface_operation_type(),
        bsurf_profiles("uProfilesArray", "U profiles", ["uProfileEntities", "uCondition", "uMagnitude", "uReference", "uDirection"])
            .hints(Hints::INITIAL_FOCUS),
        bsurf_profiles("vProfilesArray", "V profiles", ["vProfileEntities", "vCondition", "vMagnitude", "vReference", "vDirection"]),
        ParamSpec::bool("showIsocurves", "Show isocurves", false),
        ParamSpec::num("curveCount", "Count", ISO_GRID).visible(|v| v.bool("showIsocurves")),
    ];
    p.extend(surface_merge_scope());
    FeatureType::new("boundarySurface", "Boundary surface", Category::Surface, p).describe("Create a surface from up to two U and two V profiles")
}

pub fn ruled_surface() -> FeatureType {
    let ty = |v: &ParamValues, t: &str| v.is("ruledType", t);
    let face_based = move |v: &ParamValues| ty(v, "NORMAL") || ty(v, "TANGENT");
    let ovr = |v: &ParamValues, t: &str| v.is("overrideType", t);
    let mut p = vec![
        surface_operation_type(),
        ParamSpec::query("edges", "Edges and curves for ruled surface path", CURVES).hints(Hints::INITIAL_FOCUS),
        ParamSpec::enum_(
            "ruledType",
            "Ruled surface type",
            &[("NORMAL", "Normal"), ("TANGENT", "Tangent"), ("ALIGNED_WITH_VECTOR", "Aligned with direction"), ("ANGLE_FROM_VECTOR", "Angle from direction")],
        ),
        ParamSpec::query1("axis", "Direction", QueryFilter::DIRECTION).visible(move |v| ty(v, "ALIGNED_WITH_VECTOR") || ty(v, "ANGLE_FROM_VECTOR")),
        ParamSpec::num("angleFromVector", "Angle", angle_360(90.0)).visible(move |v| ty(v, "ANGLE_FROM_VECTOR")),
        ParamSpec::bool("oppositeAngleFromVector", "Opposite direction", false)
            .hints(Hints::OPPOSITE_DIRECTION_CIRCULAR)
            .visible(move |v| ty(v, "ANGLE_FROM_VECTOR")),
        ParamSpec::num("angle", "Angle", angle_360(0.0)).visible(face_based),
        ParamSpec::bool("oppositeAngle", "Opposite direction", false).hints(Hints::OPPOSITE_DIRECTION_CIRCULAR).visible(face_based),
        ParamSpec::length("distance", "Distance", 25.0, 1.0),
        ParamSpec::flip("oppositeDirection", "Opposite direction"),
        ParamSpec::query("referenceFaces", "Reference faces", QueryFilter::ANY_FACE).visible(face_based),
        ParamSpec::array(
            "vertexOverrides",
            "Vertex overrides",
            "Vertex",
            vec![
                ParamSpec::query1("vertex", "Vertex", QueryFilter::VERTEX),
                ParamSpec::enum_(
                    "overrideType",
                    "Ruled surface type",
                    &[("NORMAL", "Normal"), ("TANGENT", "Tangent"), ("ALIGNED_WITH_VECTOR", "Aligned with direction"), ("UP_TO_VERTEX", "Up to vertex")],
                )
                .hints(Hints::MATCH_LAST_ARRAY_ITEM),
                ParamSpec::num("angleOverride", "Angle", angle_360(0.0)).visible(move |v| ovr(v, "NORMAL") || ovr(v, "TANGENT")),
                ParamSpec::bool("oppositeAngleOverride", "Opposite direction", false)
                    .hints(Hints::OPPOSITE_DIRECTION_CIRCULAR)
                    .visible(move |v| ovr(v, "NORMAL") || ovr(v, "TANGENT")),
                ParamSpec::query1("axisOverride", "Direction", QueryFilter::DIRECTION).visible(move |v| ovr(v, "ALIGNED_WITH_VECTOR")),
                ParamSpec::query1("boundaryOverride", "Up to vertex or mate connector", QueryFilter::POINT_LIKE).visible(move |v| ovr(v, "UP_TO_VERTEX")),
                ParamSpec::length("distanceOverride", "Distance", 25.0, 1.0).visible(move |v| !ovr(v, "UP_TO_VERTEX")),
                ParamSpec::flip("oppositeDirectionOverride", "Opposite direction").visible(move |v| !ovr(v, "UP_TO_VERTEX")),
            ],
        ),
        ParamSpec::bool("useCubicInterpolation", "Use cubic interpolation", true),
        ParamSpec::bool("showRuledLines", "Show ruled lines", false),
        ParamSpec::num("ruledLineCount", "Count", NumSpec::integer(40, 1, 100)).visible(|v| v.bool("showRuledLines")),
        ParamSpec::enum_(
            "cornerType",
            "Corner type",
            &[("SPUN", "Spun corner"), ("EXTENDED", "Extended corner"), ("LOFTED", "Lofted corner"), ("NONE", "No corner")],
        ),
    ];
    p.extend(surface_merge_scope());
    FeatureType::new("ruledSurface", "Ruled surface", Category::Surface, p).describe("Create a ruled surface from edges")
}

pub fn mutual_trim() -> FeatureType {
    let p = vec![
        ParamSpec::query1("body1", "First surface", QueryFilter::SHEET_BODY).hints(Hints::INITIAL_FOCUS),
        ParamSpec::flip("keepOtherSide1", "Keep opposite side"),
        ParamSpec::query1("body2", "Second surface", QueryFilter::SHEET_BODY),
        ParamSpec::flip("keepOtherSide2", "Keep opposite side"),
        ParamSpec::bool("merge", "Merge", true).hints(Hints::REMEMBER_PREVIOUS_VALUE),
    ];
    FeatureType::new("mutualTrim", "Mutual trim", Category::Surface, p).describe("Trim two intersecting surfaces against each other")
}

pub fn constrained_surface() -> FeatureType {
    let p = vec![
        ParamSpec::enum_("csType", "Type", &[("POINTS", "Points"), ("MESH", "Mesh")]).hints(Hints::HORIZONTAL_ENUM),
        ParamSpec::array(
            "vertices",
            "Vertices",
            "Vertex",
            vec![
                ParamSpec::query1("vertex", "Vertex", QueryFilter::VERTEX),
                ParamSpec::query1("normal", "Normal", QueryFilter::DIRECTION),
                ParamSpec::flip("flipNormal", "Flip normal"),
            ],
        )
        .hints(Hints::COLLAPSE_ARRAY_ITEMS | Hints::INITIAL_FOCUS)
        .visible(|v| v.is("csType", "POINTS")),
        ParamSpec::query("meshes", "Mesh bodies or faces", QueryFilter::ANY_FACE | QueryFilter::BODY).visible(|v| v.is("csType", "MESH")),
        ParamSpec::num("tolerance", "Tolerance", TOLERANCE_BOUND),
        ParamSpec::enum_("optimize", "Optimize", &[("PERF", "Performance"), ("SMOOTH", "Smoothness")]),
        ParamSpec::bool("computeDeviation", "Deviation", false).hints(Hints::GROUP_HEADER),
        ParamSpec::num("maxDeviation", "Maximum deviation", NumSpec::positive_length(0.0, 0.0))
            .hints(Hints::READ_ONLY | Hints::UNCONFIGURABLE)
            .visible(|v| v.bool("computeDeviation")),
        ParamSpec::bool("showDeviation", "Show deviation", false).visible(|v| v.bool("computeDeviation")),
        ParamSpec::enum_("deviationType", "Deviation type", &[("MAX", "Max deviation"), ("ALL", "All deviations")])
            .visible(|v| v.bool("computeDeviation") && v.bool("showDeviation")),
        ParamSpec::num("limit", "Limit", NONNEGATIVE_ZERO_DEFAULT).visible(|v| v.bool("computeDeviation") && v.bool("showDeviation")),
    ];
    FeatureType::new("constrainedSurface", "Constrained surface", Category::Surface, p).describe("Fit a surface through points or mesh data")
}
