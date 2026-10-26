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

pub const FRAME_CORNER: &[(&str, &str)] = &[("MITER", "Miter"), ("BUTT", "Butt"), ("COPED_BUTT", "Coped butt"), ("NONE", "None")];

fn butt(v: &ParamValues, id: &str) -> bool {
    v.is(id, "BUTT") || v.is(id, "COPED_BUTT")
}

pub fn frame() -> FeatureType {
    let p = vec![
        // PartStudioData reference to a profile sketch (document/library + configuration).
        ParamSpec::text("profileSketch", "Sketch profile", "").hints(Hints::REMEMBER_PREVIOUS_VALUE),
        ParamSpec::query("selections", "Selections", QueryFilter::ANY_FACE | QueryFilter::ANY_EDGE | QueryFilter::VERTEX | QueryFilter::WIRE_BODY)
            .hints(Hints::INITIAL_FOCUS | Hints::ALLOW_QUERY_ORDER),
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
            "Corner",
            vec![
                ParamSpec::query1("vertex", "Vertex", QueryFilter::VERTEX),
                ParamSpec::enum_("cornerType", "Corner type", FRAME_CORNER).hints(Hints::SHOW_LABEL),
                ParamSpec::flip("cornerButtFlip", "Flip corner").visible(|v| butt(v, "cornerType")),
            ],
        ),
        ParamSpec::bool("trim", "Limit frame ends", false).hints(Hints::GROUP_HEADER),
        ParamSpec::query("trimPlanes", "Faces to trim to", QueryFilter::PLANAR_FACE | QueryFilter::PLANE).visible(|v| v.bool("trim")),
        ParamSpec::query("trimBodies", "Parts to trim to", QueryFilter::SOLID_BODY).visible(|v| v.bool("trim")),
        hidden(ParamSpec::num("index", "Index", NumSpec::integer(4, 0, 100))),
    ];
    FeatureType::new("frame", "Frame", Category::Frame, p).describe("Sweep a profile sketch along edges to create frame segments")
}

pub fn frame_trim() -> FeatureType {
    let face = |v: &ParamValues| v.is("trimType", "FACE");
    let p = vec![
        ParamSpec::enum_("trimType", "Trim Type", &[("GROUP", "Ordered groups"), ("FACE", "Face")])
            .hints(Hints::HORIZONTAL_ENUM | Hints::REMEMBER_PREVIOUS_VALUE),
        ParamSpec::array("frameGroups", "Frame groups", "Group", vec![ParamSpec::query("frames", "Frames", QueryFilter::SOLID_BODY)])
            .hints(Hints::COLLAPSE_ARRAY_ITEMS)
            .visible(move |v| !face(v)),
        ParamSpec::query("targets", "Parts to trim", QueryFilter::SOLID_BODY).visible(face),
        ParamSpec::query1("tool", "Face to trim to", QueryFilter::ANY_FACE | QueryFilter::SHEET_BODY | QueryFilter::PLANE | QueryFilter::MATE_CONNECTOR)
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
        ParamSpec::query("edges", "Edges", QueryFilter::LINEAR_EDGE).hints(Hints::INITIAL_FOCUS),
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
            .visible(move |v| rect(v) && v.is("cornersType", "CHAMFER")),
        ParamSpec::num("cornerRadius", "Radius", NumSpec::positive_length(5.0, 0.25).bounded(1e-3, 500.0))
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
            "Column override",
            vec![
                hidden(ParamSpec::bool("isFirst", "Is first", false)),
                ParamSpec::bool("sameFramesAsPrevious", "Same frames as previous", true).visible(|v| !v.bool("isFirst")),
                ParamSpec::bool("overrideAllFrames", "All frames", false).visible(|v| v.bool("isFirst") || !v.bool("sameFramesAsPrevious")),
                ParamSpec::query("overrideFrames", "Frames", QueryFilter::SOLID_BODY)
                    .visible(|v| (v.bool("isFirst") || !v.bool("sameFramesAsPrevious")) && !v.bool("overrideAllFrames")),
                ParamSpec::text("column", "Column", ""),
                ParamSpec::text("value", "Value", ""),
            ],
        ),
    ];
    FeatureType::new("cutlist", "Cut list", Category::Frame, p).icon("cutList").describe("Group frame segments and generate a cut list table")
}

pub fn tag() -> FeatureType {
    let frame = |v: &ParamValues| v.is("tagPurpose", "FRAME");
    let form = |v: &ParamValues| v.is("tagPurpose", "FORM");
    let p = vec![
        ParamSpec::enum_("tagPurpose", "Tag purpose", &[("FRAME", "Frame"), ("FORM", "Form"), ("PCB_HOLES", "PCB holes")])
            .hints(Hints::REMEMBER_PREVIOUS_VALUE),
        ParamSpec::query1("sketch", "Sketch profile", QueryFilter::FEATURE | QueryFilter::SKETCH).hints(Hints::INITIAL_FOCUS).visible(frame),
        ParamSpec::query("alignmentPoints", "Sketch points", QueryFilter::SKETCH_POINT).visible(frame),
        ParamSpec::text("standard", "Standard", "").visible(frame),
        ParamSpec::text("description", "Description", "").visible(frame),
        ParamSpec::array(
            "additionalColumns",
            "Additional columns",
            "Column",
            vec![ParamSpec::text("header", "Header", ""), ParamSpec::text("value", "Value", "")],
        )
        .hints(Hints::UNCONFIGURABLE)
        .visible(frame),
        ParamSpec::bool("showDefaultAlignmentPoints", "Show default points", true).hints(Hints::REMEMBER_PREVIOUS_VALUE).visible(frame),
        // Form tag (sheet metal form tools).
        ParamSpec::query1("positivePart", "Part to add", QueryFilter::SOLID_BODY).visible(form),
        ParamSpec::query1("negativePart", "Part to remove", QueryFilter::SOLID_BODY).visible(form),
        ParamSpec::query1("flatFormSketch", "Sketch for flat view", QueryFilter::FEATURE | QueryFilter::SKETCH).visible(form),
        ParamSpec::query1("cSysMateConnector", "Form origin mate connector", QueryFilter::MATE_CONNECTOR).visible(form),
    ];
    FeatureType::new("tag", "Tag", Category::Frame, p).icon("tagProfile").describe("Tag a profile sketch for frames or a form tool for sheet metal")
}
