//! Solid creation schemas: Revolve, Sweep, Loft, Thicken, Enclose, Rib,
//! Shell, Hole, External thread and the FeatureScript-only primitives.
//! Parameter ids, labels, defaults and visibility follow the FeatureScript
//! preconditions (research/sections/C-features-core.md, 03a).

use super::common::*;
use super::hole_tables;
use super::{Category, FeatureType};
use crate::params::{Hints, NumSpec, ParamSpec, ParamValues, QueryFilter};

pub fn register(out: &mut Vec<FeatureType>) {
    out.push(revolve());
    out.push(sweep());
    out.push(loft());
    out.push(thicken());
    out.push(enclose());
    out.push(rib());
    out.push(shell());
    out.push(hole());
    out.push(external_thread());
    out.push(cube());
    out.push(sphere());
}

/// Revolve end types (extrude end types without Through all).
const REVOLVE_BOUND: &[(&str, &str)] =
    &[("BLIND", "Blind"), ("UP_TO_NEXT", "Up to next"), ("UP_TO_SURFACE", "Up to face"), ("UP_TO_BODY", "Up to part"), ("UP_TO_VERTEX", "Up to vertex")];

const UP_TO_FACE_FILTER: QueryFilter = QueryFilter(QueryFilter::ANY_FACE.0 | QueryFilter::PLANE.0 | QueryFilter::MATE_CONNECTOR.0);

pub fn revolve() -> FeatureType {
    let mut p = creation_and_operation(true);
    p.push(
        ParamSpec::query("entities", "Faces and sketch regions to revolve", QueryFilter::SKETCH_REGION | QueryFilter::PLANAR_FACE)
            .hints(Hints::INITIAL_FOCUS)
            .visible(|v| v.is("bodyType", "SOLID")),
    );
    p.push(
        ParamSpec::query("surfaceEntities", "Edges and sketch curves to revolve", QueryFilter::ANY_EDGE | QueryFilter::WIRE_BODY)
            .visible(|v| v.is("bodyType", "SURFACE")),
    );
    p.push(
        ParamSpec::query("wallShape", "Faces and sketch regions to revolve", QueryFilter::SKETCH_REGION | QueryFilter::PLANAR_FACE | QueryFilter::SKETCH_EDGE)
            .visible(|v| v.is("bodyType", "THIN")),
    );
    p.extend(thin_block());
    p.push(ParamSpec::query1("axis", "Revolve axis", QueryFilter::AXIS | QueryFilter::CIRCULAR_EDGE));
    p.push(ParamSpec::bool("fullRevolve", "Full revolve", true));

    let part = |v: &ParamValues| !v.bool("fullRevolve");
    p.push(ParamSpec::enum_("endBound", "End type", REVOLVE_BOUND).visible(part));
    p.push(ParamSpec::bool("oppositeDirection", "Opposite direction", false).hints(Hints::OPPOSITE_DIRECTION_CIRCULAR).visible(part));
    let blind = move |v: &ParamValues| part(v) && v.is("endBound", "BLIND");
    p.push(ParamSpec::num("angle", "Revolve angle", angle_360(30.0)).hints(Hints::CAN_BE_TOLERANT).visible(blind));
    p.push(ParamSpec::bool("symmetric", "Symmetric", false).visible(blind));
    p.push(ParamSpec::query1("endBoundEntityFace", "Up to face", UP_TO_FACE_FILTER).visible(move |v| part(v) && v.is("endBound", "UP_TO_SURFACE")));
    p.push(ParamSpec::query1("endBoundEntityBody", "Up to surface or part", QueryFilter::BODY).visible(move |v| part(v) && v.is("endBound", "UP_TO_BODY")));
    p.push(
        ParamSpec::query1("endBoundEntityVertex", "Up to vertex or mate connector", QueryFilter::POINT_LIKE)
            .visible(move |v| part(v) && v.is("endBound", "UP_TO_VERTEX")),
    );
    let upto = move |v: &ParamValues| part(v) && !v.is("endBound", "BLIND");
    p.push(ParamSpec::bool("endBoundHasOffset", "Offset", false).hints(Hints::DISPLAY_SHORT | Hints::FIRST_IN_ROW).visible(upto));
    let has_off = move |v: &ParamValues| upto(v) && v.bool("endBoundHasOffset");
    p.push(ParamSpec::num("endBoundOffset", "Offset", angle_360(0.0)).hints(Hints::DISPLAY_SHORT).visible(has_off));
    p.push(ParamSpec::bool("endBoundOffsetFlip", "Flip offset", false).hints(Hints::OPPOSITE_DIRECTION_CIRCULAR).visible(has_off));

    p.push(
        ParamSpec::bool("hasStartBound", "Second end position", false)
            .hints(Hints::GROUP_HEADER)
            .visible(move |v| part(v) && (!v.is("endBound", "BLIND") || !v.bool("symmetric"))),
    );
    let second = move |v: &ParamValues| part(v) && v.bool("hasStartBound") && (!v.is("endBound", "BLIND") || !v.bool("symmetric"));
    p.push(ParamSpec::enum_("startBound", "Start type", REVOLVE_BOUND).visible(second));
    p.push(ParamSpec::num("angleBack", "Start angle", angle_360(330.0)).visible(move |v| second(v) && v.is("startBound", "BLIND")));
    let s_upto = move |v: &ParamValues| second(v) && !v.is("startBound", "BLIND");
    p.push(ParamSpec::bool("startOppositeDirection", "Opposite direction", false).hints(Hints::OPPOSITE_DIRECTION_CIRCULAR).visible(s_upto));
    p.push(ParamSpec::query1("startBoundEntityFace", "Up to face", UP_TO_FACE_FILTER).visible(move |v| second(v) && v.is("startBound", "UP_TO_SURFACE")));
    p.push(
        ParamSpec::query1("startBoundEntityBody", "Up to surface or part", QueryFilter::BODY).visible(move |v| second(v) && v.is("startBound", "UP_TO_BODY")),
    );
    p.push(
        ParamSpec::query1("startBoundEntityVertex", "Up to vertex or mate connector", QueryFilter::POINT_LIKE)
            .visible(move |v| second(v) && v.is("startBound", "UP_TO_VERTEX")),
    );
    p.push(ParamSpec::bool("startBoundHasOffset", "Start offset", false).hints(Hints::DISPLAY_SHORT | Hints::FIRST_IN_ROW).visible(s_upto));
    let s_off = move |v: &ParamValues| s_upto(v) && v.bool("startBoundHasOffset");
    p.push(ParamSpec::num("startBoundOffset", "Start offset", angle_360(0.0)).hints(Hints::DISPLAY_SHORT).visible(s_off));
    p.push(ParamSpec::bool("startBoundOffsetFlip", "Flip offset", false).hints(Hints::OPPOSITE_DIRECTION_CIRCULAR).visible(s_off));
    p.extend(merge_scope());

    FeatureType::new("revolve", "Revolve", Category::Solid, p)
        .shortcut("shift-w")
        .describe("Add or remove material by revolving a sketch region or face about an axis")
}

pub fn sweep() -> FeatureType {
    const PROFILE_CONTROL: &[(&str, &str)] = &[
        ("NONE", "None"),
        ("KEEP_ORIENTATION", "Keep profile orientation"),
        ("LOCK_FACES", "Lock profile faces"),
        ("LOCK_DIRECTION", "Lock profile direction"),
    ];
    const TWIST: &[(&str, &str)] = &[("TURNS", "Turns"), ("ANGLE", "Angle"), ("PITCH", "Pitch")];
    let mut p = creation_and_operation(true);
    p.push(
        ParamSpec::query("profiles", "Faces and sketch regions to sweep", QueryFilter::SKETCH_REGION | QueryFilter::PLANAR_FACE)
            .hints(Hints::INITIAL_FOCUS)
            .visible(|v| v.is("bodyType", "SOLID")),
    );
    p.push(
        ParamSpec::query("surfaceProfiles", "Edges and sketch curves to sweep", QueryFilter::ANY_EDGE | QueryFilter::WIRE_BODY)
            .visible(|v| v.is("bodyType", "SURFACE")),
    );
    p.push(
        ParamSpec::query(
            "wallShape",
            "Faces and sketch regions to sweep",
            QueryFilter::ANY_EDGE | QueryFilter::ANY_FACE | QueryFilter::SKETCH_REGION | QueryFilter::WIRE_BODY,
        )
        .visible(|v| v.is("bodyType", "THIN")),
    );
    p.extend(without_hint(thin_block(), Hints::CAN_BE_TOLERANT));
    p.push(ParamSpec::query("path", "Sweep path", QueryFilter::ANY_EDGE | QueryFilter::WIRE_BODY));
    p.push(ParamSpec::flip("pathOppositeDirection", "Opposite direction"));
    p.push(ParamSpec::bool("extendToFullPath", "Extend to full path", false));
    p.push(ParamSpec::enum_("profileControl", "Profile control", PROFILE_CONTROL).hints(Hints::SHOW_LABEL | Hints::REMEMBER_PREVIOUS_VALUE));
    p.push(ParamSpec::query("lockFaces", "Faces to lock", QueryFilter::ANY_FACE).visible(|v| v.is("profileControl", "LOCK_FACES")));
    p.push(ParamSpec::query1("lockDirectionQuery", "Direction to lock", QueryFilter::DIRECTION).visible(|v| v.is("profileControl", "LOCK_DIRECTION")));
    let twist_ok = |v: &ParamValues| v.is("profileControl", "NONE") || v.is("profileControl", "KEEP_ORIENTATION");
    p.push(ParamSpec::bool("hasTwist", "Twist", false).hints(Hints::GROUP_HEADER).visible(twist_ok));
    let twist = move |v: &ParamValues| twist_ok(v) && v.bool("hasTwist");
    p.push(ParamSpec::enum_("twistType", "Twist type", TWIST).hints(Hints::SHOW_LABEL | Hints::REMEMBER_PREVIOUS_VALUE).visible(twist));
    p.push(ParamSpec::num("turns", "Revolutions", NumSpec::real(1.0, 0.0, 1000.0)).visible(move |v| twist(v) && v.is("twistType", "TURNS")));
    p.push(ParamSpec::num("angle", "Rotation angle", NumSpec::angle(0.0, 0.0, 360_000.0)).visible(move |v| twist(v) && v.is("twistType", "ANGLE")));
    p.push(
        ParamSpec::num("pitch", "Pitch length", NumSpec::positive_length(25.0, 1.0).bounded(1e-6, 500.0))
            .visible(move |v| twist(v) && v.is("twistType", "PITCH")),
    );
    p.push(ParamSpec::bool("ccw", "Twist opposite direction", false).hints(Hints::OPPOSITE_DIRECTION_CIRCULAR).visible(twist));
    p.push(ParamSpec::bool("hasScale", "Scale", false).hints(Hints::DISPLAY_SHORT | Hints::FIRST_IN_ROW));
    p.push(ParamSpec::num("scaleFactor", "Scale factor", NumSpec::real(1.0, 1e-5, 1e5)).hints(Hints::DISPLAY_SHORT).visible(|v| v.bool("hasScale")));
    p.push(ParamSpec::bool("trimEnds", "Trim ends", true).visible(|v| v.is("bodyType", "THIN")));
    p.extend(merge_scope());
    FeatureType::new("sweep", "Sweep", Category::Solid, p).describe("Sweep a profile along a path")
}

/// Loft start/end conditions (LoftEndDerivativeType).
pub const LOFT_END_CONDITION: &[(&str, &str)] = &[
    ("DEFAULT", "None"),
    ("NORMAL_TO_PROFILE", "Normal to profile"),
    ("TANGENT_TO_PROFILE", "Tangent to profile"),
    ("MATCH_TANGENT", "Match tangent"),
    ("MATCH_CURVATURE", "Match curvature"),
    ("NORMAL_TO_DIRECTION", "Normal direction"),
    ("TANGENT_TO_DIRECTION", "Tangent direction"),
];

pub fn is_match_condition(c: &str) -> bool {
    matches!(c, "MATCH_TANGENT" | "MATCH_CURVATURE")
}

pub fn is_direction_condition(c: &str) -> bool {
    matches!(c, "NORMAL_TO_DIRECTION" | "TANGENT_TO_DIRECTION")
}

pub fn loft() -> FeatureType {
    const GUIDE_CONTINUITY: &[(&str, &str)] = &[
        ("DEFAULT", "None"),
        ("NORMAL_TO_GUIDE", "Normal to guide"),
        ("TANGENT_TO_GUIDE", "Tangent to guide"),
        ("MATCH_TANGENT", "Match tangent"),
        ("MATCH_CURVATURE", "Match curvature"),
    ];
    let mut p = creation_and_operation(true);
    p.push(
        ParamSpec::array(
            "sheetProfilesArray",
            "Profiles",
            "Profile",
            vec![ParamSpec::query(
                "sheetProfileEntities",
                "Faces and sketch regions",
                QueryFilter::ANY_FACE | QueryFilter::SKETCH_REGION | QueryFilter::SHEET_BODY | QueryFilter::VERTEX,
            )],
        )
        .hints(Hints::COLLAPSE_ARRAY_ITEMS | Hints::INITIAL_FOCUS)
        .visible(|v| v.is("bodyType", "SOLID")),
    );
    p.push(
        ParamSpec::array(
            "wireProfilesArray",
            "Profiles",
            "Profile",
            vec![ParamSpec::query(
                "wireProfileEntities",
                "Edges, curves and sketches",
                QueryFilter::ANY_EDGE | QueryFilter::ANY_FACE | QueryFilter::WIRE_BODY | QueryFilter::SHEET_BODY | QueryFilter::VERTEX,
            )],
        )
        .hints(Hints::COLLAPSE_ARRAY_ITEMS)
        .visible(|v| !v.is("bodyType", "SOLID")),
    );
    p.extend(without_hint(thin_block(), Hints::CAN_BE_TOLERANT));
    for (cond, faces, mag, dir, label) in [
        ("startCondition", "adjacentFacesStart", "startMagnitude", "startDirection", ["Start profile condition", "Start magnitude", "Start direction"]),
        ("endCondition", "adjacentFacesEnd", "endMagnitude", "endDirection", ["End profile condition", "End magnitude", "End direction"]),
    ] {
        p.push(ParamSpec::enum_(cond, label[0], LOFT_END_CONDITION).hints(Hints::SHOW_LABEL));
        p.push(ParamSpec::query(faces, "Adjacent faces", QueryFilter::ANY_FACE).visible(move |v| is_match_condition(v.enum_(cond))));
        p.push(ParamSpec::num(mag, label[1], CLAMP_MAGNITUDE).visible(move |v| !v.is(cond, "DEFAULT")));
        p.push(ParamSpec::query1(dir, label[2], QueryFilter::DIRECTION).visible(move |v| is_direction_condition(v.enum_(cond))));
    }
    p.push(ParamSpec::bool("trimProfiles", "Trim profiles", false).visible(|v| !v.is("bodyType", "SOLID")));
    p.push(ParamSpec::bool("addGuides", "Guides and continuity", false).hints(Hints::GROUP_HEADER));
    p.push(
        ParamSpec::array(
            "guidesArray",
            "Guides",
            "Guide",
            vec![
                ParamSpec::query("guideEntities", "Edges, curves and sketches", QueryFilter::ANY_EDGE | QueryFilter::WIRE_BODY),
                ParamSpec::enum_("guideDerivativeType", "Continuity", GUIDE_CONTINUITY).hints(Hints::SHOW_LABEL | Hints::MATCH_LAST_ARRAY_ITEM),
            ],
        )
        .hints(Hints::COLLAPSE_ARRAY_ITEMS)
        .visible(|v| v.bool("addGuides")),
    );
    p.push(ParamSpec::bool("trimGuidesByProfiles", "Trim guides", true).visible(|v| v.bool("addGuides") && !v.is("bodyType", "SOLID")));
    p.push(ParamSpec::bool("addSections", "Path", false).hints(Hints::GROUP_HEADER));
    p.push(ParamSpec::query("spine", "Edges, curves and sketches", QueryFilter::ANY_EDGE | QueryFilter::WIRE_BODY).visible(|v| v.bool("addSections")));
    p.push(ParamSpec::num("sectionCount", "Section count", NumSpec::integer(5, 1, 50)).visible(|v| v.bool("addSections")));
    p.push(ParamSpec::bool("matchConnections", "Connections", false).hints(Hints::GROUP_HEADER));
    p.push(
        ParamSpec::array(
            "connections",
            "Match connections",
            "Connection",
            vec![ParamSpec::query("connectionEntities", "Vertices or edges", QueryFilter::EDGE | QueryFilter::VERTEX)],
        )
        .visible(|v| v.bool("matchConnections")),
    );
    p.push(ParamSpec::bool("showIsocurves", "Show isocurves", false));
    p.push(ParamSpec::num("curveCount", "Count", ISO_GRID).visible(|v| v.bool("showIsocurves")));
    p.push(ParamSpec::bool("trimEnds", "Trim ends", true).visible(|v| v.is("bodyType", "THIN")));
    p.extend(merge_scope());
    p.push(hidden(ParamSpec::bool("makePeriodic", "Make periodic", false)));
    FeatureType::new("loft", "Loft", Category::Solid, p).describe("Create a solid or surface transitioning between profiles")
}

/// Thickness block shared by Thicken (midplane / thickness 1 & 2).
fn thickness_pair(flip_id: &'static str) -> Vec<ParamSpec> {
    vec![
        ParamSpec::bool("midplane", "Mid plane", false),
        ParamSpec::num("thickness1", "Thickness 1", NumSpec::positive_length(5.0, 0.25)).visible(|v| !v.bool("midplane")),
        ParamSpec::flip(flip_id, "Opposite direction").visible(|v| !v.bool("midplane")),
        ParamSpec::num("thickness2", "Thickness 2", NumSpec::positive_length(0.0, 0.0)).visible(|v| !v.bool("midplane")),
        ParamSpec::num("thickness", "Thickness", NumSpec::positive_length(5.0, 0.25)).visible(|v| v.bool("midplane")),
    ]
}

pub fn thicken() -> FeatureType {
    let mut p = vec![
        operation_type(always),
        ParamSpec::query("entities", "Faces and surfaces to thicken", QueryFilter::ANY_FACE | QueryFilter::SKETCH_REGION | QueryFilter::SHEET_BODY)
            .hints(Hints::INITIAL_FOCUS),
    ];
    p.extend(thickness_pair("oppositeDirection"));
    p.push(ParamSpec::bool("keepTools", "Keep tools", false));
    p.extend(solid_merge_scope(always, QueryFilter::SOLID_BODY));
    FeatureType::new("thicken", "Thicken", Category::Solid, p).describe("Add depth to a surface or face to create a solid")
}

pub fn enclose() -> FeatureType {
    let mut p = vec![
        operation_type(always),
        ParamSpec::query("entities", "Entities", QueryFilter::BODY | QueryFilter::ANY_FACE | QueryFilter::PLANE).hints(Hints::INITIAL_FOCUS),
    ];
    p.extend(solid_merge_scope(always, QueryFilter::SOLID_BODY));
    p.push(ParamSpec::bool("keepTools", "Keep tools", false));
    FeatureType::new("enclose", "Enclose", Category::Solid, p).describe("Create a part from the volume bounded by surfaces, faces and planes")
}

pub fn rib() -> FeatureType {
    let p = vec![
        ParamSpec::query("profiles", "Sketch profiles", QueryFilter::SKETCH_EDGE).hints(Hints::INITIAL_FOCUS),
        ParamSpec::query("parts", "Parts", QueryFilter::SOLID_BODY),
        ParamSpec::num("thickness", "Thickness", SHELL_OFFSET),
        ParamSpec::enum_(
            "ribExtrusionDirection",
            "Rib extrusion direction",
            &[("PARALLEL_TO_SKETCH_PLANE", "Parallel to sketch plane"), ("NORMAL_TO_SKETCH_PLANE", "Normal to sketch plane")],
        ),
        default_true(ParamSpec::flip("oppositeDirection", "Opposite direction")),
        ParamSpec::bool("hasDraft", "Draft", false).hints(Hints::DISPLAY_SHORT | Hints::FIRST_IN_ROW),
        ParamSpec::num("draftAngle", "Draft angle", NumSpec::angle(3.0, 0.0, 89.9)).hints(Hints::DISPLAY_SHORT).visible(|v| v.bool("hasDraft")),
        ParamSpec::bool("draftOpposite", "Opposite direction", false).hints(Hints::OPPOSITE_DIRECTION_CIRCULAR).visible(|v| v.bool("hasDraft")),
        ParamSpec::bool("extendProfilesUpToPart", "Extend profiles to part", false),
        ParamSpec::bool("mergeRibs", "Merge ribs", true),
    ];
    FeatureType::new("rib", "Rib", Category::Solid, p).describe("Create ribs from sketch profiles inside parts")
}

pub fn shell() -> FeatureType {
    let p = vec![
        ParamSpec::bool("isHollow", "Hollow", false),
        ParamSpec::query("entities", "Faces to remove", QueryFilter::ANY_FACE).hints(Hints::INITIAL_FOCUS).visible(|v| !v.bool("isHollow")),
        ParamSpec::query("parts", "Parts to hollow", QueryFilter::SOLID_BODY).visible(|v| v.bool("isHollow")),
        ParamSpec::num("thickness", "Shell thickness", SHELL_OFFSET),
        ParamSpec::flip("oppositeDirection", "Opposite direction"),
    ];
    FeatureType::new("shell", "Shell", Category::Solid, p).describe("Remove faces and hollow a part to a constant wall thickness")
}

/// Hole style values (HoleStyle).
pub const HOLE_STYLE: &[(&str, &str)] = &[("SIMPLE", "Simple"), ("C_BORE", "Counterbore"), ("C_SINK", "Countersink")];

pub fn hole() -> FeatureType {
    let rem = Hints::REMEMBER_PREVIOUS_VALUE;
    let inch = |v: &ParamValues| v.is("unitsSystem", "INCH");
    let simple = |v: &ParamValues| v.is("styleV2", "SIMPLE");
    let threaded = |v: &ParamValues| v.bool("showTappedDepth");
    let through = |v: &ParamValues| v.is("endStyleV2", "THROUGH");
    let up_to = |v: &ParamValues| v.is("endStyleV2", "UP_TO_NEXT") || v.is("endStyleV2", "UP_TO_ENTITY");
    let p = vec![
        // Hidden state (set by edit logic / the table).
        hidden(ParamSpec::bool("isV2", "Feature version", true)),
        hidden(ParamSpec::enum_("holeVersion", "Hole version", &[("LEGACY", "Legacy"), ("V3", "V3")]).default_enum("V3")),
        hidden(ParamSpec::query("initEntities", "Initial entities", QueryFilter::SKETCH_POINT | QueryFilter::MATE_CONNECTOR)),
        hidden(ParamSpec::text("featureName", "Feature name template", "")),
        hidden(ParamSpec::enum_("threadStandard", "Thread standard", &[("UNSET", "Unset"), ("ANSI", "ANSI"), ("ISO", "ISO")])),
        hidden(ParamSpec::bool("hasClearance", "Has clearance", false)),
        hidden(ParamSpec::bool("showTappedDepth", "Tapped details", false)),
        hidden(ParamSpec::num("majorDiameter", "Tap major diameter", NumSpec::positive_length(5.0, 0.25))),
        hidden(ParamSpec::bool("isMultiple", "Multiple", false)),
        hidden(ParamSpec::num("tappedAngle", "Tapped angle", NumSpec::angle(0.0, 0.0, 9.9))),
        hidden(ParamSpec::enum_("style", "Style", HOLE_STYLE)),
        // Dialog.
        ParamSpec::enum_("unitsSystem", "Units", &[("INCH", "Inch"), ("METRIC", "Metric")]).hints(Hints::HORIZONTAL_ENUM | rem | Hints::UNCONFIGURABLE),
        ParamSpec::enum_("styleV2", "Style", HOLE_STYLE).hints(Hints::HORIZONTAL_ENUM | rem | Hints::UNCONFIGURABLE),
        ParamSpec::query("locations", "Sketch points to place holes", QueryFilter::SKETCH_POINT | QueryFilter::MATE_CONNECTOR).hints(Hints::INITIAL_FOCUS),
        ParamSpec::query("scope", "Merge scope", QueryFilter::SOLID_BODY),
        ParamSpec::lookup("ansiHoleTableEx", "Standard", hole_tables::TABLE_ANSI_HOLE_EX)
            .hints(rem | Hints::UNCONFIGURABLE)
            .visible(move |v| inch(v) && simple(v)),
        ParamSpec::lookup("ansiHoleTable", "Standard", hole_tables::TABLE_ANSI_HOLE).hints(rem | Hints::UNCONFIGURABLE).visible(move |v| inch(v) && !simple(v)),
        ParamSpec::lookup("isoHoleTableEx", "Standard", hole_tables::TABLE_ISO_HOLE_EX)
            .hints(rem | Hints::UNCONFIGURABLE)
            .visible(move |v| !inch(v) && simple(v)),
        ParamSpec::lookup("isoHoleTable", "Standard", hole_tables::TABLE_ISO_HOLE).hints(rem | Hints::UNCONFIGURABLE).visible(move |v| !inch(v) && !simple(v)),
        ParamSpec::bool("showThreadClassV2", "Thread class", false).hints(rem | Hints::GROUP_HEADER).visible(|v| !v.is("threadStandard", "UNSET")),
        ParamSpec::lookup("ansiThreadClassV2", "Thread class", hole_tables::TABLE_ANSI_THREAD_CLASS)
            .hints(rem | Hints::UNCONFIGURABLE)
            .visible(|v| v.bool("showThreadClassV2") && v.is("threadStandard", "ANSI")),
        ParamSpec::lookup("isoThreadClassV2", "Thread class", hole_tables::TABLE_ISO_THREAD_CLASS)
            .hints(rem | Hints::UNCONFIGURABLE)
            .visible(|v| v.bool("showThreadClassV2") && v.is("threadStandard", "ISO")),
        ParamSpec::num("holeDiameterV3", "Diameter", NumSpec::positive_length(5.0, 0.25).bounded(1e-5, 500.0)).hints(rem | Hints::CAN_BE_TOLERANT),
        ParamSpec::num("tapDrillDiameterV3", "Tap drill diameter", NumSpec::positive_length(4.2, 0.201).bounded(1e-5, 500.0))
            .hints(rem | Hints::CAN_BE_TOLERANT)
            .visible(|v| v.bool("hasClearance")),
        ParamSpec::enum_(
            "startStyle",
            "Start plane",
            &[("PART", "Start from part"), ("SKETCH", "Start from sketch plane"), ("PLANE", "Start from selected plane")],
        )
        .hints(rem | Hints::SHOW_LABEL),
        ParamSpec::query1(
            "startBoundEntity",
            "Hole start plane or mate connector",
            QueryFilter::PLANAR_FACE | QueryFilter::PLANE | QueryFilter::MATE_CONNECTOR,
        )
        .visible(|v| v.is("startStyle", "PLANE")),
        ParamSpec::enum_(
            "endStyleV2",
            "Termination",
            &[("BLIND", "Blind"), ("UP_TO_NEXT", "Up to next"), ("UP_TO_ENTITY", "Up to entity"), ("THROUGH", "Through all")],
        )
        .hints(rem | Hints::SHOW_LABEL),
        ParamSpec::flip("oppositeDirection", "Opposite direction"),
        ParamSpec::query1("endBoundEntity", "Up to entity or mate connector", QueryFilter::ANY_FACE | QueryFilter::POINT_LIKE)
            .visible(|v| v.is("endStyleV2", "UP_TO_ENTITY")),
        ParamSpec::bool("offset", "Offset from tip", false).hints(Hints::DISPLAY_SHORT | Hints::FIRST_IN_ROW).visible(up_to),
        ParamSpec::num("offsetDistance", "Offset from tip", ZERO_INCLUSIVE_OFFSET).hints(Hints::DISPLAY_SHORT).visible(move |v| up_to(v) && v.bool("offset")),
        ParamSpec::flip("oppositeOffsetDirection", "Opposite direction").visible(move |v| up_to(v) && v.bool("offset")),
        ParamSpec::num("cBoreDiameterV3", "Counterbore diameter", NumSpec::positive_length(10.0, 0.5).bounded(1e-5, 500.0))
            .hints(rem | Hints::CAN_BE_TOLERANT)
            .visible(|v| v.is("styleV2", "C_BORE")),
        ParamSpec::num("cBoreDepthV3", "Counterbore depth", NumSpec::positive_length(5.0, 0.25))
            .hints(rem | Hints::CAN_BE_TOLERANT)
            .visible(|v| v.is("styleV2", "C_BORE")),
        ParamSpec::num("cSinkDiameterV3", "Countersink diameter", NumSpec::positive_length(10.0, 0.5).bounded(1e-5, 500.0))
            .hints(rem | Hints::CAN_BE_TOLERANT)
            .visible(|v| v.is("styleV2", "C_SINK")),
        ParamSpec::num("cSinkAngleV3", "Countersink angle", NumSpec::angle(90.0, 0.1, 179.9))
            .hints(rem | Hints::CAN_BE_TOLERANT)
            .visible(|v| v.is("styleV2", "C_SINK")),
        ParamSpec::num("holeDepthV3", "Depth", NumSpec::positive_length(12.0, 0.5).bounded(1e-5, 500.0))
            .hints(rem | Hints::CAN_BE_TOLERANT)
            .visible(|v| v.is("endStyleV2", "BLIND")),
        ParamSpec::num("holeDepthComputedV3", "Depth", NumSpec::positive_length(12.0, 0.5))
            .hints(Hints::READ_ONLY | Hints::CAN_BE_TOLERANT | Hints::UNCONFIGURABLE)
            .visible(up_to),
        ParamSpec::enum_("tipAngleStyle", "Tip angle style", &[("DEGREE118", "118°"), ("DEGREE135", "135°"), ("FLAT", "Flat"), ("CUSTOM", "Custom")])
            .hints(Hints::SHOW_LABEL)
            .visible(move |v| !through(v)),
        ParamSpec::num("tipAngleV3", "Tip angle", NumSpec::angle(118.0, 0.1, 180.0))
            .hints(rem | Hints::CAN_BE_TOLERANT)
            .visible(move |v| !through(v) && v.is("tipAngleStyle", "CUSTOM")),
        ParamSpec::bool("isTappedThrough", "Tap through all", true).hints(rem).visible(move |v| threaded(v) && through(v)),
        ParamSpec::num("tappedDepthV3", "Tapped depth", NumSpec::positive_length(12.0, 0.5).bounded(1e-5, 500.0))
            .hints(rem | Hints::CAN_BE_TOLERANT)
            .visible(move |v| threaded(v) && (!through(v) || !v.bool("isTappedThrough"))),
        ParamSpec::num("tapClearance", "Tap clearance (number of thread pitch lengths)", NumSpec::real(3.0, 0.0, 500.0))
            .hints(rem)
            .visible(move |v| threaded(v) && v.is("endStyleV2", "BLIND")),
    ];
    FeatureType::new("hole", "Hole", Category::Solid, p).describe("Create standard or custom simple, counterbore and countersink holes at sketch points")
}

pub fn external_thread() -> FeatureType {
    let rem = Hints::REMEMBER_PREVIOUS_VALUE;
    let p = vec![
        ParamSpec::query("entities", "Cylinder edges", QueryFilter::CIRCULAR_EDGE).hints(Hints::INITIAL_FOCUS | Hints::UNCONFIGURABLE),
        ParamSpec::lookup("branchOfStandard", "Standard", hole_tables::TABLE_EXTERNAL_THREAD).hints(rem | Hints::UNCONFIGURABLE),
        ParamSpec::enum_("depthType", "End type", &[("Blind", "Blind"), ("UpToNext", "Up to next")]).hints(rem),
        ParamSpec::num("threadDepth", "Length", NumSpec::positive_length(10.0, 0.5).bounded(1e-4, 200.0)).hints(rem).visible(|v| v.is("depthType", "Blind")),
        ParamSpec::bool("addChamfer", "Add chamfer", false).hints(Hints::GROUP_HEADER),
        ParamSpec::num("chamferWidth", "Length", BLEND_BOUNDS).hints(rem).visible(|v| v.bool("addChamfer")),
        ParamSpec::num("chamferAngle", "Angle", NumSpec::angle(30.0, 0.1, 89.0)).hints(rem).visible(|v| v.bool("addChamfer")),
        ParamSpec::bool("addUndercut", "Add undercut", false).hints(Hints::GROUP_HEADER),
        ParamSpec::num("undercutDiameter", "Diameter", NONNEGATIVE_LENGTH).hints(rem).visible(|v| v.bool("addUndercut")),
        ParamSpec::num("undercutLength", "Length", NONNEGATIVE_LENGTH).hints(rem).visible(|v| v.bool("addUndercut")),
        ParamSpec::bool("splitFace", "Split face", true).hints(rem).visible(|v| !v.bool("addUndercut") && v.is("depthType", "Blind")),
    ];
    FeatureType::new("externalThread", "External thread", Category::Solid, p).describe("Add a cosmetic ANSI or ISO thread to a cylindrical face")
}

/// FeatureScript-only primitive (not in the Onshape toolbar).
pub fn cube() -> FeatureType {
    let p = vec![ParamSpec::num("sideLength", "Side length", NONNEGATIVE_LENGTH)];
    FeatureType::new("cube", "Cube", Category::Solid, p).hidden().describe("Cube with a corner at the origin (FeatureScript primitive)")
}

/// FeatureScript-only primitive (not in the Onshape toolbar).
pub fn sphere() -> FeatureType {
    let p = vec![ParamSpec::query1("center", "Center", QueryFilter::POINT_LIKE), ParamSpec::num("radius", "Radius", NONNEGATIVE_LENGTH)];
    FeatureType::new("fSphere", "Sphere", Category::Solid, p).hidden().describe("Sphere about a vertex (FeatureScript primitive)")
}
