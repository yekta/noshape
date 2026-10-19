//! Pattern and transform schemas: Linear / Circular / Curve pattern, Mirror,
//! Transform (incl. copy) and the deprecated Copy part
//! (D-features-other.md §1, 03a PS-19..24).

use super::common::*;
use super::{Category, FeatureType};
use crate::params::{Hints, NumSpec, ParamSpec, ParamValues, QueryFilter};

pub fn register(out: &mut Vec<FeatureType>) {
    out.push(linear_pattern());
    out.push(circular_pattern());
    out.push(curve_pattern());
    out.push(mirror());
    out.push(transform());
    out.push(copy_part());
}

pub const PATTERN_TYPE: &[(&str, &str)] = &[("PART", "Part pattern"), ("FEATURE", "Feature pattern"), ("FACE", "Face pattern")];
pub const MIRROR_TYPE: &[(&str, &str)] = &[("PART", "Part mirror"), ("FEATURE", "Feature mirror"), ("FACE", "Face mirror")];

/// Bodies (and explicit mate connectors) a part pattern / transform accepts.
const PATTERN_BODIES: QueryFilter = QueryFilter(QueryFilter::BODY.0 | QueryFilter::WIRE_BODY.0 | QueryFilter::POINT_BODY.0 | QueryFilter::MATE_CONNECTOR.0);

fn is_part(v: &ParamValues) -> bool {
    v.is("patternType", "PART")
}

/// Pattern type selector, Part-pattern operation tabs and the three seed
/// selections (patternTypePredicate); `mirror` switches to the Mirror labels.
fn pattern_type_block(types: &'static [(&'static str, &'static str)], label: &'static str, mirror: bool) -> Vec<ParamSpec> {
    let (ent, faces, feats) = if mirror {
        ("Entities to mirror", "Faces to mirror", "Features to mirror")
    } else {
        ("Entities to pattern", "Faces to pattern", "Features to pattern")
    };
    vec![
        ParamSpec::enum_("patternType", label, types),
        operation_type(is_part),
        ParamSpec::query("entities", ent, PATTERN_BODIES).hints(Hints::INITIAL_FOCUS).visible(is_part),
        ParamSpec::query("faces", faces, QueryFilter::ANY_FACE).hints(Hints::SHOW_CREATE_SELECTION).visible(|v| v.is("patternType", "FACE")),
        ParamSpec::query("instanceFunction", feats, QueryFilter::FEATURE).visible(|v| v.is("patternType", "FEATURE")),
    ]
}

/// Merge scope + Reapply features, common tail of every pattern.
fn pattern_tail(reapply_default: bool) -> Vec<ParamSpec> {
    let mut out = solid_merge_scope(is_part, QueryFilter::BODY);
    out.push(ParamSpec::bool("fullFeaturePattern", "Reapply features", reapply_default).visible(|v| v.is("patternType", "FEATURE")));
    out
}

pub fn linear_pattern() -> FeatureType {
    let second = |v: &ParamValues| v.bool("hasSecondDir");
    let mut p = pattern_type_block(PATTERN_TYPE, "Pattern type", false);
    p.extend([
        ParamSpec::query1("directionOne", "Direction", QueryFilter::DIRECTION).hints(Hints::NO_MATE_CONNECTOR_BUTTON),
        ParamSpec::num("distance", "Distance", NumSpec::positive_length(25.0, 1.0)),
        ParamSpec::num("instanceCount", "Instance count", NumSpec::integer(2, 1, 2500)),
        ParamSpec::flip("oppositeDirection", "Opposite direction"),
        ParamSpec::bool("isCentered", "Centered", false),
        ParamSpec::bool("hasSecondDir", "Second direction", false).hints(Hints::GROUP_HEADER),
        ParamSpec::query1("directionTwo", "Direction", QueryFilter::DIRECTION).hints(Hints::NO_MATE_CONNECTOR_BUTTON).visible(second),
        ParamSpec::num("distanceTwo", "Distance", NumSpec::positive_length(25.0, 1.0)).visible(second),
        ParamSpec::num("instanceCountTwo", "Instance count", NumSpec::integer(1, 1, 2500)).visible(second),
        ParamSpec::flip("oppositeDirectionTwo", "Opposite direction").visible(second),
        ParamSpec::bool("isCenteredTwo", "Centered", false).visible(second),
    ]);
    p.extend(pattern_tail(false));
    p.extend(skip_instances(true));
    FeatureType::new("linearPattern", "Linear pattern", Category::Pattern, p).describe("Pattern parts, features or faces along one or two directions")
}

pub fn circular_pattern() -> FeatureType {
    let mut p = pattern_type_block(PATTERN_TYPE, "Pattern type", false);
    p.extend([
        ParamSpec::query1("axis", "Axis of pattern", QueryFilter::AXIS | QueryFilter::CIRCULAR_EDGE),
        ParamSpec::num("angle", "Angle", angle_360(360.0)),
        ParamSpec::num("instanceCount", "Instance count", NumSpec::integer(4, 1, 2500)),
        ParamSpec::bool("oppositeDirection", "Opposite direction", false).hints(Hints::OPPOSITE_DIRECTION_CIRCULAR),
        ParamSpec::bool("equalSpace", "Equal spacing", true),
        ParamSpec::bool("isCentered", "Centered", false),
    ]);
    p.extend(pattern_tail(false));
    p.extend(skip_instances(false));
    FeatureType::new("circularPattern", "Circular pattern", Category::Pattern, p).describe("Pattern parts, features or faces about an axis")
}

pub fn curve_pattern() -> FeatureType {
    let mut p = pattern_type_block(PATTERN_TYPE, "Pattern type", false);
    p.extend([
        ParamSpec::query("edges", "Path to pattern along", QueryFilter::ANY_EDGE | QueryFilter::WIRE_BODY).hints(Hints::ALLOW_QUERY_ORDER),
        ParamSpec::enum_("spacingType", "Spacing type", &[("EQUAL", "Equal spacing"), ("DISTANCE", "Distance")]),
        ParamSpec::num("distance", "Distance", NumSpec::positive_length(25.0, 1.0)).visible(|v| v.is("spacingType", "DISTANCE")),
        ParamSpec::num("instanceCount", "Instance count", NumSpec::integer(2, 1, 2500)),
        ParamSpec::enum_(
            "orientationType",
            "Orientation",
            &[("DEFAULT", "Tangent to curve"), ("LOCK_FACES", "Normal to face"), ("KEEP_ORIENTATION", "Locked")],
        )
        .hints(Hints::SHOW_LABEL),
        ParamSpec::query("lockedFaces", "Faces to be normal to", QueryFilter::ANY_FACE).visible(|v| v.is("orientationType", "LOCK_FACES")),
    ]);
    p.extend(pattern_tail(false));
    p.extend(skip_instances(false));
    FeatureType::new("curvePattern", "Curve pattern", Category::Pattern, p).describe("Pattern parts, features or faces along a path")
}

pub fn mirror() -> FeatureType {
    let mut p = pattern_type_block(MIRROR_TYPE, "Mirror type", true);
    p.push(ParamSpec::query1("mirrorPlane", "Mirror plane", QueryFilter::PLANE_LIKE));
    // FS UI default is false (the FS defaults map says true for legacy calls).
    p.extend(pattern_tail(false));
    FeatureType::new("mirror", "Mirror", Category::Pattern, p).describe("Mirror parts, features or faces across a plane")
}

pub const TRANSFORM_TYPE: &[(&str, &str)] = &[
    ("TRANSLATION_ENTITY", "Translate by line"),
    ("TRANSLATION_DISTANCE", "Translate by distance"),
    ("TRANSLATION_3D", "Translate by XYZ"),
    ("TRANSFORM_MATE_CONNECTORS", "Transform by mate connectors"),
    ("ROTATION", "Rotate"),
    ("COPY", "Copy in place"),
    ("SCALE_UNIFORMLY", "Scale"),
];

pub const MATE_CONNECTOR_AXIS: &[(&str, &str)] = &[("PLUS_X", "+X"), ("PLUS_Y", "+Y"), ("MINUS_X", "-X"), ("MINUS_Y", "-Y")];

pub fn transform() -> FeatureType {
    let ty = |v: &ParamValues, t: &str| v.is("transformType", t);
    let scale = move |v: &ParamValues| ty(v, "SCALE_UNIFORMLY");
    let mcs = move |v: &ParamValues| ty(v, "TRANSFORM_MATE_CONNECTORS");
    let xyz = move |v: &ParamValues| ty(v, "TRANSLATION_3D");
    let p = vec![
        ParamSpec::query("entities", "Entities to transform or copy", PATTERN_BODIES).hints(Hints::INITIAL_FOCUS),
        ParamSpec::enum_("transformType", "Transform type", TRANSFORM_TYPE),
        ParamSpec::flip("oppositeDirectionEntity", "Opposite direction").visible(move |v| ty(v, "TRANSLATION_ENTITY")),
        query_n("transformLine", "Line or points", QueryFilter::VERTEX | QueryFilter::SKETCH_POINT | QueryFilter::ANY_EDGE, 2)
            .hints(Hints::ALLOW_QUERY_ORDER)
            .visible(move |v| ty(v, "TRANSLATION_ENTITY")),
        ParamSpec::query1("transformAxis", "Axis", QueryFilter::AXIS | QueryFilter::CIRCULAR_EDGE).visible(move |v| ty(v, "ROTATION")),
        query_n("transformDirection", "Direction", QueryFilter::DIRECTION | QueryFilter::VERTEX, 2).visible(move |v| ty(v, "TRANSLATION_DISTANCE")),
        ParamSpec::num("distance", "Distance", NONNEGATIVE_LENGTH).visible(move |v| ty(v, "TRANSLATION_DISTANCE")),
        ParamSpec::bool("uniform", "Scale uniformly", true).visible(scale),
        ParamSpec::num("scale", "Scale", NumSpec::real(1.0, 1e-5, 1e5)).visible(move |v| scale(v) && v.bool("uniform")),
        ParamSpec::num("scaleX", "X scale", NumSpec::real(1.0, 1e-5, 1e5)).visible(move |v| scale(v) && !v.bool("uniform")),
        ParamSpec::num("scaleY", "Y scale", NumSpec::real(1.0, 1e-5, 1e5)).visible(move |v| scale(v) && !v.bool("uniform")),
        ParamSpec::num("scaleZ", "Z scale", NumSpec::real(1.0, 1e-5, 1e5)).visible(move |v| scale(v) && !v.bool("uniform")),
        ParamSpec::query1("baseConnector", "From mate connector", QueryFilter::MATE_CONNECTOR).visible(mcs),
        ParamSpec::query1("destinationConnector", "To mate connector", QueryFilter::MATE_CONNECTOR).visible(mcs),
        ParamSpec::bool("oppositeDirectionMateAxis", "Flip primary axis", false).hints(Hints::FIRST_IN_ROW).visible(mcs),
        ParamSpec::enum_("secondaryAxisType", "Reorient secondary axis", MATE_CONNECTOR_AXIS).visible(mcs),
        ParamSpec::num("angle", "Angle", angle_360(30.0)).visible(move |v| ty(v, "ROTATION")),
        ParamSpec::flip("oppositeDirection", "Opposite direction").visible(move |v| ty(v, "ROTATION") || ty(v, "TRANSLATION_DISTANCE")),
        ParamSpec::num("dx", "X translation", ZERO_DEFAULT_LENGTH).visible(xyz),
        ParamSpec::num("dy", "Y translation", ZERO_DEFAULT_LENGTH).visible(xyz),
        ParamSpec::num("dz", "Z translation", ZERO_DEFAULT_LENGTH).visible(xyz),
        ParamSpec::query1("scalePoint", "Point or mate connector", QueryFilter::POINT_LIKE).visible(scale),
        ParamSpec::bool("makeCopy", "Copy part", false).visible(move |v| !ty(v, "COPY")),
    ];
    FeatureType::new("transform", "Transform", Category::Transform, p).describe("Move, rotate, scale or copy parts and mate connectors")
}

/// Deprecated FeatureScript feature, superseded by Transform > Copy in place.
pub fn copy_part() -> FeatureType {
    let p = vec![ParamSpec::query("entities", "Entities to copy", QueryFilter::BODY | QueryFilter::WIRE_BODY | QueryFilter::POINT_BODY)];
    FeatureType::new("copyPart", "Copy part", Category::Transform, p).hidden().describe("Copy bodies in place (deprecated)")
}
