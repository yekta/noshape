//! Frame (weldment) schemas: Frame, Frame trim, Gusset, End cap, Cut list,
//! Tag (03b FR-01..07, D-features-other.md §7, §3.9).

use super::common::*;
use super::{Category, FeatureType};
use crate::params::{Hints, NumSpec, ParamSpec, ParamValues, QueryFilter};

pub fn register(out: &mut Vec<FeatureType>) {
    out.push(frame());
    out.push(frame_trim());
    out.push(gusset());
    out.push(end_cap());
    out.push(cut_list());
    out.push(tag());
}

/// A Frame's `profileSketch` naming a sketch of another Part Studio of the document (Select sketch › Current
/// document, live D134): `<tab>#<sketch feature>.<index>` as the select panel writes a studio's sketch, the studio's
/// configuration after a tab (`input=value` pairs joined by `;`). The tab and the sketch's feature.
pub fn studio_sketch(value: &str) -> Option<(&str, crate::FeatureId, crate::configuration::ConfigValues)> {
    let (base, config) = value.split_once('\t').unwrap_or((value, ""));
    let (tab, body) = base.trim().split_once('#')?;
    let feature = body.split('.').next()?.strip_prefix('F')?;
    let feature = crate::FeatureId(u64::from_str_radix(feature, 16).ok()?);
    let mut values = crate::configuration::ConfigValues::default();
    for kv in config.split(';').filter(|s| !s.is_empty()) {
        if let Some((k, v)) = kv.split_once('=') {
            values.0.insert(k.to_string(), v.to_string());
        }
    }
    (!tab.is_empty()).then_some((tab, feature, values))
}

pub const FRAME_CORNER: &[(&str, &str)] = &[("MITER", "Miter"), ("BUTT", "Butt"), ("COPED_BUTT", "Coped butt"), ("NONE", "None")];

fn butt(v: &ParamValues, id: &str) -> bool {
    v.is(id, "BUTT") || v.is(id, "COPED_BUTT")
}

pub fn frame() -> FeatureType {
    let p = vec![
        // PartStudioData reference to a profile sketch (document/library + configuration).
        // A fresh account's first Frame opens with 8020 › 10 Series › 1001 (live 2216; then the last pick, remembered).
        ParamSpec::text("profileSketch", "Sketch profile", "frame-library:8020/1001")
            .hints(Hints::REMEMBER_PREVIOUS_VALUE)
            .describe("A library profile (feature_frame_profiles gives its value: frame-library:<standard>/<profile>, a tab, Size=<size>), a sketch of this Part Studio by name, or a profile by kind and size (\"SHS 40x40x3\")"),
        ParamSpec::query("selections", "Selections", QueryFilter::ANY_FACE | QueryFilter::ANY_EDGE | QueryFilter::VERTEX | QueryFilter::WIRE_BODY).describe("Faces, edges, and vertices that define sweep paths")
            .hints(Hints::INITIAL_FOCUS),
        ParamSpec::bool("lockProfile", "Lock profile faces", false).hints(Hints::GROUP_HEADER),
        ParamSpec::query("profileLockFaces", "Faces to lock", QueryFilter::ANY_FACE).visible(|v| v.bool("lockProfile")),
        ParamSpec::bool("mergeTangentSegments", "Merge tangent segments", true),
        ParamSpec::num("angle", "Angle", angle_360(0.0)),
        ParamSpec::flip("mirrorProfile", "Mirror across Y axis"),
        ParamSpec::query1("angleReference", "Angle reference", QueryFilter::DIRECTION),
        ParamSpec::enum_("defaultCornerType", "Default corner type", FRAME_CORNER).hints(Hints::SHOW_LABEL),
        ParamSpec::flip("defaultButtFlip", "Flip corner").visible(|v| butt(v, "defaultCornerType")),
        ParamSpec::array(
            "cornerOverrides",
            "Corner overrides",
            "vertex",
            vec![
                ParamSpec::query1("vertex", "Vertex", QueryFilter::VERTEX).hints(Hints::ALWAYS_HIDDEN),
                ParamSpec::enum_("cornerType", "Corner type", FRAME_CORNER).hints(Hints::SHOW_LABEL),
                ParamSpec::flip("cornerButtFlip", "Flip corner").visible(|v| butt(v, "cornerType")),
            ],
        )
        // frame.fs: a driven query, picked vertices add the overrides.
        .item_template("#vertex [#cornerType]")
        .hints(Hints::DRIVEN_QUERY),
        ParamSpec::bool("trim", "Limit frame ends", false).hints(Hints::GROUP_HEADER),
        ParamSpec::query("trimPlanes", "Faces to trim to", QueryFilter::PLANAR_FACE | QueryFilter::PLANE).describe("Planes and planar faces to use as trim tools").visible(|v| v.bool("trim")),
        ParamSpec::query("trimBodies", "Parts to trim to", QueryFilter::SOLID_BODY).describe("Parts to use as trim tools").visible(|v| v.bool("trim")),
        hidden(ParamSpec::num("index", "Index", NumSpec::integer(4, 0, 100))),
    ];
    FeatureType::new("frame", "Frame", Category::Frame, p).describe("Sweep a profile sketch along edges to create frame segments")
}

pub fn frame_trim() -> FeatureType {
    let face = |v: &ParamValues| v.is("trimType", "FACE");
    let p = vec![
        ParamSpec::enum_("trimType", "Trim Type", &[("GROUP", "Ordered groups"), ("FACE", "Face")])
            .hints(Hints::HORIZONTAL_ENUM | Hints::REMEMBER_PREVIOUS_VALUE),
        ParamSpec::array("frameGroups", "Frame groups", "group", vec![ParamSpec::query("frames", "Frames", QueryFilter::SOLID_BODY).describe("Frame segments are trimmed against preceding groups and are used as tools for later groups")]).describe("Frame segments in earlier groups are used as tools for later groups")
        .item_template("#frames")
            .hints(Hints::COLLAPSE_ARRAY_ITEMS | Hints::DRIVEN_QUERY)
            .visible(move |v| !face(v)),
        ParamSpec::query("targets", "Parts to trim", QueryFilter::SOLID_BODY).describe("Parts to trim using trim tool").visible(face),
        ParamSpec::query1("tool", "Face to trim to", QueryFilter::ANY_FACE | QueryFilter::SHEET_BODY | QueryFilter::PLANE | QueryFilter::MATE_CONNECTOR).describe("Face, plane, mate connector, or planar face to use as trim tool")
            .visible(face),
        ParamSpec::flip("flipTrim", "Opposite side").visible(face),
        ParamSpec::bool("useToolExtents", "Trim to face boundaries", false).visible(face),
    ];
    FeatureType::new("frameTrim", "Frame trim", Category::Frame, p).describe("Trim frame segments against each other or a face")
}

pub fn gusset() -> FeatureType {
    let rem = Hints::REMEMBER_PREVIOUS_VALUE;
    let rect = |v: &ParamValues| v.is("gussetType", "RECTANGLE");
    let aligned = |v: &ParamValues| v.is("gussetPosition", "ALIGNED");
    let p = vec![
        ParamSpec::query("edges", "Edges", QueryFilter::LINEAR_EDGE).describe("Edges that define the bases of the gussets").hints(Hints::INITIAL_FOCUS),
        ParamSpec::enum_("gussetType", "Gusset type", &[("TRIANGLE", "Triangle"), ("RECTANGLE", "Rectangle")]).hints(rem | Hints::SHOW_LABEL),
        ParamSpec::num("length", "Length", NONNEGATIVE_LENGTH).hints(rem),
        ParamSpec::flip("shouldFlipBaseSides", "Flip base sides").visible(rect),
        ParamSpec::num("thickness", "Thickness", NONNEGATIVE_LENGTH).hints(rem),
        ParamSpec::bool("chamfer", "Chamfer", false).hints(rem | Hints::DISPLAY_SHORT | Hints::FIRST_IN_ROW).visible(rect),
        ParamSpec::num("chamferSize", "Chamfer size", NumSpec::positive_length(1.0, 0.04).bounded(1e-5, 500.0))
            .hints(rem | Hints::DISPLAY_SHORT)
            .visible(move |v| rect(v) && v.bool("chamfer")),
        ParamSpec::enum_("gussetPosition", "Gusset position", &[("CENTERED", "Centered"), ("ALIGNED", "Aligned")]).hints(rem | Hints::SHOW_LABEL),
        ParamSpec::query1("alignedReference", "Alignment entity", QueryFilter::LINEAR_EDGE).visible(aligned),
        ParamSpec::flip("shouldFlipAlignment", "Opposite alignment").visible(aligned),
        ParamSpec::num("offset", "Offset distance", NONNEGATIVE_ZERO_DEFAULT).hints(rem),
        ParamSpec::flip("shouldFlipOffset", "Opposite direction"),
        hidden(ParamSpec::query("baseSweptFaces", "Base faces", QueryFilter::PLANAR_FACE)),
    ];
    FeatureType::new("gusset", "Gusset", Category::Frame, p).describe("Add gusset plates between adjacent frame segments")
}

pub fn end_cap() -> FeatureType {
    let rem = Hints::REMEMBER_PREVIOUS_VALUE;
    let internal = |v: &ParamValues| v.is("profileType", "INTERNAL");
    let rect = |v: &ParamValues| v.is("profileType", "RECTANGLE");
    let p = vec![
        ParamSpec::query("faces", "Faces", QueryFilter::PLANAR_FACE).hints(Hints::INITIAL_FOCUS),
        ParamSpec::enum_(
            "profileType",
            "Profile type",
            &[("MATCH_PROFILE", "Match profile"), ("RECTANGLE", "Rectangle"), ("CIRCLE", "Circle"), ("INTERNAL", "Internal")],
        ),
        ParamSpec::num("thickness", "Thickness", NumSpec::positive_length(5.0, 0.25).bounded(1e-3, 500.0)),
        ParamSpec::flip("thicknessDirection", "Opposite direction").visible(move |v| !internal(v)),
        ParamSpec::num("offsetDistance", "Offset", NONNEGATIVE_ZERO_DEFAULT).hints(rem).visible(move |v| !internal(v)),
        ParamSpec::flip("offsetDirection", "Opposite direction").visible(move |v| !internal(v)),
        ParamSpec::num("internalOffsetDistance", "Offset", NONNEGATIVE_ZERO_DEFAULT).hints(rem).visible(internal),
        ParamSpec::num("internalShiftDistance", "Inset", NONNEGATIVE_ZERO_DEFAULT).hints(rem).visible(internal),
        ParamSpec::enum_("cornersType", "Treatment type", &[("NONE", "None"), ("CHAMFER", "Chamfer"), ("FILLET", "Fillet")]).hints(rem | Hints::SHOW_LABEL).visible(rect),
        ParamSpec::num("cornerDistance", "Length", NumSpec::positive_length(5.0, 0.25).bounded(1e-3, 500.0))
            .hints(rem)
            .visible(move |v| rect(v) && v.is("cornersType", "CHAMFER")),
        ParamSpec::num("cornerRadius", "Radius", NumSpec::positive_length(5.0, 0.25).bounded(1e-3, 500.0))
            .hints(rem)
            .visible(move |v| rect(v) && v.is("cornersType", "FILLET")),
    ];
    FeatureType::new("endcap", "End cap", Category::Frame, p).icon("endCap").describe("Cap open frame segment ends")
}

pub fn cut_list() -> FeatureType {
    let p = vec![
        ParamSpec::bool("selectAllFrames", "Select all frames", false),
        ParamSpec::query("frames", "Frames", QueryFilter::SOLID_BODY).hints(Hints::INITIAL_FOCUS).visible(|v| !v.bool("selectAllFrames")),
        ParamSpec::array(
            "columnOverrides",
            "Overrides",
            "Column Override",
            vec![
                hidden(ParamSpec::bool("isFirst", "Is first", false)),
                ParamSpec::bool("sameFramesAsPrevious", "Same frames as previous", true).visible(|v| !v.bool("isFirst")),
                ParamSpec::bool("overrideAllFrames", "All frames", false).visible(|v| v.bool("isFirst") || !v.bool("sameFramesAsPrevious")),
                ParamSpec::query("overrideFrames", "Frames", QueryFilter::SOLID_BODY)
                    .visible(|v| (v.bool("isFirst") || !v.bool("sameFramesAsPrevious")) && !v.bool("overrideAllFrames")),
                ParamSpec::text("column", "Column", ""),
                ParamSpec::text("value", "Value", ""),
            ],
        )
        .item_template("#column = #value")
        .hints(Hints::PREVENT_ARRAY_REORDER),
    ];
    FeatureType::new("cutlist", "Cut list", Category::Frame, p).icon("cutList").describe("Group frame segments and generate a cut list table")
}

pub fn tag() -> FeatureType {
    let frame = |v: &ParamValues| v.is("tagPurpose", "FRAME");
    let form = |v: &ParamValues| v.is("tagPurpose", "FORM");
    let pcb = |v: &ParamValues| v.is("tagPurpose", "PCB_HOLES");
    let hole = move |v: &ParamValues| pcb(v) && v.is("pcbEntityType", "HOLE");
    let mut p = vec![
        ParamSpec::enum_("tagPurpose", "Tag purpose", &[("FRAME", "Frame"), ("FORM", "Form"), ("PCB_HOLES", "PCB holes")])
            .hints(Hints::REMEMBER_PREVIOUS_VALUE),
        ParamSpec::query1("sketch", "Sketch profile", QueryFilter::FEATURE | QueryFilter::SKETCH).hints(Hints::INITIAL_FOCUS).visible(frame),
    ];
    // std tag.fs defineFrameTagParams: the points sit in a collapsed group.
    p.extend(group("additionalAlignmentPointsGroup", "Additional alignment points", false, vec![ParamSpec::query("alignmentPoints", "Sketch points", QueryFilter::SKETCH_POINT).visible(frame)]).into_iter().map(|s| if s.id == "additionalAlignmentPointsGroup" { s.visible(frame) } else { s }));
    p.extend([
        ParamSpec::text("standard", "Standard", "").visible(frame),
        ParamSpec::text("description", "Description", "").visible(frame),
        ParamSpec::array(
            "additionalColumns",
            "Additional columns",
            "Column",
            vec![ParamSpec::text("header", "Header", ""), ParamSpec::text("value", "Value", "")],
        )
        .item_template("#header = #value")
        .hints(Hints::PREVENT_ARRAY_REORDER)
        .visible(frame),
        ParamSpec::bool("showDefaultAlignmentPoints", "Show default points", true).hints(Hints::REMEMBER_PREVIOUS_VALUE).visible(frame),
        // Form tag (sheet metal form tools).
        ParamSpec::query1("positivePart", "Part to add", QueryFilter::SOLID_BODY).visible(form),
        ParamSpec::query1("negativePart", "Part to remove", QueryFilter::SOLID_BODY).visible(form),
        ParamSpec::query1("flatFormSketch", "Sketch for flat view", QueryFilter::FEATURE | QueryFilter::SKETCH).visible(form),
        ParamSpec::query1("cSysMateConnector", "Form origin mate connector", QueryFilter::MATE_CONNECTOR).describe("If none selected, this feature will create one at Origin").visible(form),
        // PCB holes (std definePcbHolesTagParams; std runs Tag).
        ParamSpec::enum_("pcbEntityType", "PCB entity type", &[("HOLE", "Holes"), ("CUTOUT", "Cutouts")]).hints(Hints::SHOW_LABEL).visible(pcb),
        ParamSpec::bool("plated", "Plated", false).visible(pcb),
        ParamSpec::query1("basePlane", "Base plane", QueryFilter::PLANE_LIKE).visible(pcb),
        ParamSpec::array(
            "holes",
            "Holes",
            "Hole",
            vec![
                ParamSpec::query("interiorFaces", "Interior faces", QueryFilter::FACE),
                ParamSpec::text("holeId", "Hole ID", "").hints(Hints::ALWAYS_HIDDEN),
                ParamSpec::text("designator", "Designator", ""),
                ParamSpec::enum_("platedPurpose", "Purpose", &[("NONE", "None"), ("VIA", "Via"), ("THERMAL_VIA", "Thermal via"), ("PIN", "Pin"), ("CUSTOM", "Custom")])
                    .default_enum("VIA")
                    .visible(move |v| hole(v) && v.bool("plated")),
                ParamSpec::enum_("nonPlatedPurpose", "Purpose", &[("NONE", "None"), ("MOUNTING", "Mounting"), ("TOOLING", "Tooling"), ("CUSTOM", "Custom")])
                    .visible(move |v| hole(v) && !v.bool("plated")),
                ParamSpec::text("customPurpose", "Purpose", "").visible(move |v| {
                    v.is("pcbEntityType", "CUTOUT") || (hole(v) && if v.bool("plated") { v.is("platedPurpose", "CUSTOM") } else { v.is("nonPlatedPurpose", "CUSTOM") })
                }),
                ParamSpec::text("componentDesignator", "Associated component instance", "").visible(hole),
                ParamSpec::enum_("holeOwner", "Owner", &[("UNOWNED", "Unowned"), ("ECAD", "ECAD"), ("MCAD", "MCAD")]).hints(Hints::SHOW_LABEL),
            ],
        )
        .item_template("[#designator] #interiorFaces")
        .hints(Hints::COLLAPSE_ARRAY_ITEMS | Hints::INITIAL_FOCUS_ON_EDIT | Hints::DRIVEN_QUERY)
        .visible(pcb),
    ]);
    FeatureType::new("tag", "Tag", Category::Frame, p).icon("tagProfile").describe("Tag a profile sketch for frames or a form tool for sheet metal")
}
