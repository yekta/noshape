//! Parameter blocks shared by many features (FeatureScript "predicates").

use crate::params::{Hints, ParamSpec, ParamValues, QueryFilter};

pub const BODY_TYPE: &[(&str, &str)] = &[("SOLID", "Solid"), ("SURFACE", "Surface"), ("THIN", "Thin")];
pub const OPERATION: &[(&str, &str)] = &[("NEW", "New"), ("ADD", "Add"), ("REMOVE", "Remove"), ("INTERSECT", "Intersect")];
pub const SURFACE_OPERATION: &[(&str, &str)] = &[("NEW", "New"), ("ADD", "Add")];
pub const END_BOUND: &[(&str, &str)] = &[
    ("BLIND", "Blind"),
    ("UP_TO_NEXT", "Up to next"),
    ("UP_TO_SURFACE", "Up to face"),
    ("UP_TO_BODY", "Up to part"),
    ("UP_TO_VERTEX", "Up to vertex"),
    ("THROUGH_ALL", "Through all"),
];

pub fn is_up_to(v: &str) -> bool {
    matches!(v, "UP_TO_NEXT" | "UP_TO_SURFACE" | "UP_TO_BODY" | "UP_TO_VERTEX")
}

/// Creation type + result operation (New/Add/Remove/Intersect).
pub fn creation_and_operation(with_thin: bool) -> Vec<ParamSpec> {
    let types: &'static [(&str, &str)] = if with_thin { BODY_TYPE } else { &BODY_TYPE[..2] };
    vec![
        ParamSpec::enum_("bodyType", "Creation type", types).hints(Hints::HORIZONTAL_ENUM | Hints::REMEMBER_PREVIOUS_VALUE),
        ParamSpec::enum_("operationType", "Result body operation type", OPERATION)
            .hints(Hints::HORIZONTAL_ENUM)
            .visible(|v| !v.is("bodyType", "SURFACE")),
        ParamSpec::enum_("surfaceOperationType", "Result body operation type", SURFACE_OPERATION)
            .hints(Hints::HORIZONTAL_ENUM)
            .visible(|v| v.is("bodyType", "SURFACE")),
    ]
}

/// The Thin creation-type block.
pub fn thin_block() -> Vec<ParamSpec> {
    let thin = |v: &ParamValues| v.is("bodyType", "THIN");
    vec![
        ParamSpec::bool("midplane", "Mid plane", false).visible(thin),
        ParamSpec::num("thickness1", "Thickness 1", crate::params::NumSpec::positive_length(5.0, 0.25))
            .hints(Hints::CAN_BE_TOLERANT)
            .visible(move |v| thin(v) && !v.bool("midplane")),
        ParamSpec::flip("flipWall", "Flip wall").visible(move |v| thin(v) && !v.bool("midplane")),
        ParamSpec::num("thickness2", "Thickness 2", crate::params::NumSpec::positive_length(0.0, 0.0))
            .hints(Hints::CAN_BE_TOLERANT)
            .visible(move |v| thin(v) && !v.bool("midplane")),
        ParamSpec::num("thickness", "Thickness", crate::params::NumSpec::positive_length(5.0, 0.25))
            .hints(Hints::CAN_BE_TOLERANT)
            .visible(move |v| thin(v) && v.bool("midplane")),
    ]
}

/// Merge scope block (solid and surface variants).
pub fn merge_scope() -> Vec<ParamSpec> {
    let solid_merge = |v: &ParamValues| !v.is("bodyType", "SURFACE") && !v.is("operationType", "NEW");
    let surf_merge = |v: &ParamValues| v.is("bodyType", "SURFACE") && v.is("surfaceOperationType", "ADD");
    vec![
        ParamSpec::bool("defaultScope", "Merge with all", false).visible(solid_merge),
        ParamSpec::query("booleanScope", "Merge scope", QueryFilter::SOLID_BODY)
            .visible(move |v| solid_merge(v) && !v.bool("defaultScope")),
        ParamSpec::bool("defaultSurfaceScope", "Merge with all", true).visible(surf_merge),
        ParamSpec::query("booleanSurfaceScope", "Merge scope", QueryFilter::SHEET_BODY)
            .visible(move |v| surf_merge(v) && !v.bool("defaultSurfaceScope")),
    ]
}

/// Extrude's second end position is shown: ticked, and not hidden by a
/// symmetric extrude (std extrude.fs `isSymmetricExtrude`).
pub fn second_direction_on(v: &ParamValues) -> bool {
    v.bool("hasSecondDirection") && !(v.bool("symmetric") && (v.is("endBound", "BLIND") || v.is("endBound", "THROUGH_ALL")))
}

/// End condition block used by Extrude (first or second direction).
pub fn end_condition(prefix: &'static str, second: bool) -> Vec<ParamSpec> {
    // Parameter ids follow FeatureScript names.
    let ids: [&'static str; 8] = if second {
        [
            "secondDirectionBound",
            "secondDirectionOppositeDirection",
            "secondDirectionDepth",
            "secondDirectionBoundEntityFace",
            "secondDirectionBoundEntityBody",
            "secondDirectionBoundEntityVertex",
            "hasSecondDirectionOffset",
            "secondDirectionOffsetDistance",
        ]
    } else {
        ["endBound", "oppositeDirection", "depth", "endBoundEntityFace", "endBoundEntityBody", "endBoundEntityVertex", "hasOffset", "offsetDistance"]
    };
    let _ = prefix;
    let [bound, flip, depth, face, body, vertex, has_off, off] = ids;
    let gate = move |v: &ParamValues| !second || second_direction_on(v);
    let mut out = vec![
        ParamSpec::enum_(bound, "End type", END_BOUND).visible(gate),
        ParamSpec::flip(flip, "Opposite direction").visible(gate),
        // extrudeCommon.fs: only the first depth can be tolerant.
        ParamSpec::length(depth, "Depth", 25.0, 1.0)
            .hints(if second { Hints::NONE } else { Hints::CAN_BE_TOLERANT })
            .visible(move |v| gate(v) && v.is(bound, "BLIND")),
        ParamSpec::query1(face, "Up to face", QueryFilter::ANY_FACE | QueryFilter::PLANE | QueryFilter::MATE_CONNECTOR)
            .visible(move |v| gate(v) && v.is(bound, "UP_TO_SURFACE")),
        ParamSpec::query1(body, "Up to surface or part", QueryFilter::BODY).visible(move |v| gate(v) && v.is(bound, "UP_TO_BODY")),
        ParamSpec::query1(vertex, "Up to vertex or mate connector", QueryFilter::POINT_LIKE)
            .visible(move |v| gate(v) && v.is(bound, "UP_TO_VERTEX")),
        ParamSpec::bool(has_off, "Offset distance", false)
            .hints(Hints::DISPLAY_SHORT | Hints::FIRST_IN_ROW)
            .visible(move |v| gate(v) && is_up_to(v.enum_(bound))),
        ParamSpec::length(off, "Offset distance", 25.0, 1.0)
            .hints(Hints::DISPLAY_SHORT | Hints::CAN_BE_TOLERANT)
            .visible(move |v| gate(v) && is_up_to(v.enum_(bound)) && v.bool(has_off)),
    ];
    if second {
        // Onshape defaults the second direction to the opposite side.
        if let crate::params::ParamKind::Bool { default } = &mut out[1].kind {
            *default = true;
        }
        out.push(
            ParamSpec::flip("secondDirectionOffsetOppositeDirection", "Opposite direction")
                .visible(move |v| gate(v) && is_up_to(v.enum_(bound)) && v.bool(has_off)),
        );
    } else {
        out.push(
            ParamSpec::flip("offsetOppositeDirection", "Opposite direction")
                .visible(move |v| is_up_to(v.enum_(bound)) && v.bool(has_off)),
        );
    }
    out
}

// ---------------------------------------------------------------------------
// Shared blocks for features beyond the core set.
// ---------------------------------------------------------------------------

use crate::params::{NumSpec, ParamKind};

/// FeatureScript BLEND_BOUNDS: 1e-5 .. 500 m, default 5 mm / 0.2 in.
pub const BLEND_BOUNDS: NumSpec = NumSpec::positive_length(5.0, 0.2).bounded(1e-5, 500.0);
/// NONNEGATIVE_LENGTH_BOUNDS: 1e-5 .. 500 m, default 25 mm / 1 in.
pub const NONNEGATIVE_LENGTH: NumSpec = NumSpec::positive_length(25.0, 1.0).bounded(1e-5, 500.0);
/// ZERO_INCLUSIVE_OFFSET_BOUNDS: 0 .. 500 m, default 5 mm / 0.25 in.
pub const ZERO_INCLUSIVE_OFFSET: NumSpec = NumSpec::positive_length(5.0, 0.25);
/// NONNEGATIVE_ZERO_DEFAULT_LENGTH_BOUNDS: 0 .. 500 m, default 0.
pub const NONNEGATIVE_ZERO_DEFAULT: NumSpec = NumSpec::positive_length(0.0, 0.0);
/// ZERO_DEFAULT_LENGTH_BOUNDS: -500 .. 500 m, default 0.
pub const ZERO_DEFAULT_LENGTH: NumSpec = NumSpec::length(0.0, 0.0);
/// SHELL_OFFSET_BOUNDS: 1e-5 .. 500 m, default 2.5 mm / 0.1 in.
pub const SHELL_OFFSET: NumSpec = NumSpec::positive_length(2.5, 0.1).bounded(1e-5, 500.0);
/// TOLERANCE_BOUND: 1e-8 .. 1 m, default 0.01 mm / 0.002 in.
pub const TOLERANCE_BOUND: NumSpec = NumSpec::positive_length(0.01, 0.002).bounded(1e-8, 1.0);
/// ANGLE_360_BOUNDS-style angle: -1e5 .. 1e5 deg with the given default.
pub const fn angle_360(default_deg: f64) -> NumSpec {
    NumSpec::angle(default_deg, -1e5, 1e5)
}
/// CLAMP_MAGNITUDE_REAL_BOUNDS: -1e5 .. 1e5, default 1.
pub const CLAMP_MAGNITUDE: NumSpec = NumSpec::real(1.0, -1e5, 1e5);
/// ISO_GRID_BOUNDS (isocurve count): 1 .. 50, default 10.
pub const ISO_GRID: NumSpec = NumSpec::integer(10, 1, 50);
/// EDGE_PARAMETER_BOUNDS: 0 .. 1, default 0.5.
pub const EDGE_PARAMETER: NumSpec = NumSpec::real(0.5, 0.0, 1.0);

/// Visibility gate that is always true.
pub fn always(_: &ParamValues) -> bool {
    true
}

/// Query parameter accepting at most `n` picks.
pub fn query_n(id: &'static str, label: &'static str, filter: QueryFilter, n: usize) -> ParamSpec {
    let mut p = ParamSpec::query(id, label, filter);
    if let ParamKind::Query { max_picks, .. } = &mut p.kind {
        *max_picks = Some(n);
    }
    p
}

/// Hidden parameter (FeatureScript `UIHint.ALWAYS_HIDDEN`), kept so stored
/// definitions and edit logic have a slot for it.
pub fn hidden(p: ParamSpec) -> ParamSpec {
    p.hints(Hints::ALWAYS_HIDDEN)
}

/// Strip a hint from every spec in a block (e.g. Sweep/Loft thin thickness is
/// not tolerance-capable).
pub fn without_hint(mut specs: Vec<ParamSpec>, h: Hints) -> Vec<ParamSpec> {
    for s in &mut specs {
        s.hints = Hints(s.hints.0 & !h.0);
    }
    specs
}

/// A plain group title ("Group Name" in FeatureScript, e.g. "Selections"):
/// a GROUP_LABEL header whose value is "unfolded", and its params, shown
/// only while it is unfolded.
pub fn group(id: &'static str, label: &'static str, open: bool, specs: Vec<ParamSpec>) -> Vec<ParamSpec> {
    let mut out = vec![ParamSpec::bool(id, label, open).hints(Hints::GROUP_HEADER | Hints::GROUP_LABEL | Hints::UNCONFIGURABLE)];
    out.extend(specs.into_iter().map(|mut s| {
        let prev = s.visible.take();
        s.visible(move |v| v.bool(id) && prev.as_ref().is_none_or(|f| f(v)))
    }));
    out
}

/// Change the default of a Bool spec.
pub fn default_true(mut p: ParamSpec) -> ParamSpec {
    if let ParamKind::Bool { default } = &mut p.kind {
        *default = true;
    }
    p
}

/// "Result body operation type" tabs (New/Add/Remove/Intersect) for features
/// without a creation type (Thicken, Enclose, patterns, Wrap...).
pub fn operation_type(gate: impl Fn(&ParamValues) -> bool + Send + Sync + 'static) -> ParamSpec {
    ParamSpec::enum_("operationType", "Result body operation type", OPERATION).hints(Hints::HORIZONTAL_ENUM).visible(gate)
}

/// Solid merge scope (booleanStepScopePredicate / booleanPatternScopePredicate)
/// keyed on `operationType`, shown when `gate` holds and the operation is not New.
pub fn solid_merge_scope(gate: impl Fn(&ParamValues) -> bool + Clone + Send + Sync + 'static, filter: QueryFilter) -> Vec<ParamSpec> {
    let g2 = gate.clone();
    vec![
        ParamSpec::bool("defaultScope", "Merge with all", false).visible(move |v| gate(v) && !v.is("operationType", "NEW")),
        ParamSpec::query("booleanScope", "Merge scope", filter)
            .visible(move |v| g2(v) && !v.is("operationType", "NEW") && !v.bool("defaultScope")),
    ]
}

/// Surface feature "Result body operation type" tabs (New/Add).
pub fn surface_operation_type() -> ParamSpec {
    ParamSpec::enum_("surfaceOperationType", "Result body operation type", SURFACE_OPERATION).hints(Hints::HORIZONTAL_ENUM)
}

/// Surface merge scope (surfaceJoinStepScopePredicate), shown for Add.
pub fn surface_merge_scope() -> Vec<ParamSpec> {
    vec![
        ParamSpec::bool("defaultSurfaceScope", "Merge with all", true).visible(|v| v.is("surfaceOperationType", "ADD")),
        ParamSpec::query("booleanSurfaceScope", "Merge scope", QueryFilter::SHEET_BODY)
            .visible(|v| v.is("surfaceOperationType", "ADD") && !v.bool("defaultSurfaceScope")),
    ]
}

/// Curve re-approximation group (curveApproximationPredicate).
pub fn curve_approximation(gate: impl Fn(&ParamValues) -> bool + Clone + Send + Sync + 'static) -> Vec<ParamSpec> {
    let g = gate.clone();
    let on = move |v: &ParamValues| g(v) && v.bool("approximate");
    let on2 = on.clone();
    vec![
        ParamSpec::bool("approximate", "Approximate", false).hints(Hints::GROUP_HEADER).visible(gate),
        ParamSpec::num("approximationDegree", "Target degree", NumSpec::integer(3, 2, 15)).visible(on.clone()),
        ParamSpec::num("approximationMaxCPs", "Maximum control points", NumSpec::integer(15, 4, 100)).visible(on.clone()),
        ParamSpec::num("approximationTolerance", "Tolerance", TOLERANCE_BOUND).visible(on.clone()),
        ParamSpec::bool("keepStartDerivative", "Keep start derivative", false).visible(on.clone()),
        ParamSpec::bool("keepEndDerivative", "Keep end derivative", false).visible(on.clone()),
        ParamSpec::bool("approximationShowDeviation", "Show deviation", false).visible(on),
        ParamSpec::num("maxDeviation", "Maximum deviation", NumSpec::positive_length(0.0, 0.0))
            .hints(Hints::READ_ONLY | Hints::UNCONFIGURABLE)
            .visible(move |v| on2(v) && v.bool("approximationShowDeviation")),
    ]
}

/// "Skip instances" group of linear (two indices) and circular/curve (one
/// index) patterns.
pub fn skip_instances(two_directions: bool) -> Vec<ParamSpec> {
    // std: "Item name" instance, rows labelled by "Item label template".
    let (item, template) = if two_directions {
        (
            vec![
                ParamSpec::num("index1", "First direction", NumSpec::integer(1, -100_000, 100_000)),
                ParamSpec::num("index2", "Second direction", NumSpec::integer(0, -100_000, 100_000)),
            ],
            "(#index1, #index2)",
        )
    } else {
        (vec![ParamSpec::num("index", "Index", NumSpec::integer(1, -100_000, 100_000))], "#index")
    };
    vec![
        ParamSpec::bool("skipInstances", "Skip instances", false).hints(Hints::GROUP_HEADER),
        ParamSpec::array("skippedInstances", "Instances to skip", "instance", item)
            .item_template(template)
            .hints(Hints::INITIAL_FOCUS)
            .visible(|v| v.bool("skipInstances")),
    ]
}
