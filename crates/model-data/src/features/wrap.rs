//! Wrap and Decal schemas (03a PS-16, PS-31).

use super::common::*;
use super::{Category, FeatureType};
use crate::params::{Hints, NumSpec, ParamSpec, ParamValues, QueryFilter};

pub fn register(out: &mut Vec<FeatureType>) {
    out.push(wrap());
    out.push(decal());
}

pub fn wrap() -> FeatureType {
    let solid = |v: &ParamValues| v.is("resultType", "SOLID");
    let anchors = |v: &ParamValues| v.bool("customAnchors");
    let mut p = vec![
        ParamSpec::enum_("resultType", "Creation type", &[("SOLID", "Solid"), ("SURFACE", "Surface"), ("IMPRINT", "Split")])
            .default_enum("SURFACE")
            .hints(Hints::HORIZONTAL_ENUM),
        operation_type(solid),
        // wrap.fs / live L20: Tools planar only, Target a cylinder or cone.
        ParamSpec::query("source", "Tools", QueryFilter::PLANAR_FACE | QueryFilter::SKETCH_REGION).hints(Hints::INITIAL_FOCUS),
        ParamSpec::query1("destination", "Target", QueryFilter::CYLINDRICAL_FACE | QueryFilter::FACE),
        ParamSpec::flip("flipAlignment", "Flip alignment"),
        ParamSpec::bool("trim", "Trim to target", false).visible(|v| !v.is("resultType", "IMPRINT")),
        ParamSpec::bool("customAnchors", "Specify anchor points", false),
        ParamSpec::query1("sourceAnchor", "Tools anchor point", QueryFilter::POINT_LIKE).visible(anchors),
        ParamSpec::query1("destinationAnchor", "Target anchor point", QueryFilter::POINT_LIKE).visible(anchors),
    ];
    p.extend(group("positionGroup", "Position", false, vec![
        ParamSpec::num("angle", "Angle", angle_360(0.0)),
        ParamSpec::bool("angleOppositeDirection", "Angle opposite direction", false).hints(Hints::OPPOSITE_DIRECTION_CIRCULAR),
        ParamSpec::num("uShift", "U shift", ZERO_DEFAULT_LENGTH),
        ParamSpec::flip("uShiftOppositeDirection", "U shift opposite direction"),
        ParamSpec::num("vShift", "V shift", ZERO_DEFAULT_LENGTH),
        ParamSpec::flip("vShiftOppositeDirection", "V shift opposite direction"),
    ]));
    p.extend([
        ParamSpec::num("thickness", "Thickness", SHELL_OFFSET).visible(solid),
        ParamSpec::flip("oppositeDirection", "Thickness opposite direction").visible(solid),
    ]);
    p.extend(solid_merge_scope(solid, QueryFilter::SOLID_BODY));
    FeatureType::new("wrap", "Wrap", Category::Solid, p).describe("Wrap planar faces or sketch regions onto a cylinder or cone")
}

pub fn decal() -> FeatureType {
    /// IMAGE_OFFSET_BOUNDS: -1e8 .. 1e8 m, default 0.
    const IMAGE_OFFSET: NumSpec = NumSpec::length(0.0, 0.0).bounded(-1e8, 1e8);
    /// IMAGE_SIZE_BOUNDS: 1e-5 .. 1e8 m, default 25 mm / 1 in.
    const IMAGE_SIZE: NumSpec = NumSpec::positive_length(25.0, 1.0).bounded(1e-5, 1e8);
    let keep = |v: &ParamValues| v.bool("maintainAspectRatio");
    let p = vec![
        // ImageData: an image file tab.
        ParamSpec::reference("image", "Image", crate::params::ReferenceKind::Image),
        ParamSpec::query1("face", "Face", QueryFilter::PLANAR_FACE | QueryFilter::CYLINDRICAL_FACE).hints(Hints::INITIAL_FOCUS),
        ParamSpec::num("angle", "Angle", angle_360(0.0)),
        ParamSpec::bool("angleOppositeDirection", "Angle opposite direction", false).hints(Hints::OPPOSITE_DIRECTION_CIRCULAR),
        ParamSpec::num("uShift", "U shift", IMAGE_OFFSET),
        ParamSpec::num("vShift", "V shift", IMAGE_OFFSET),
        ParamSpec::bool("realign", "Realign", false),
        ParamSpec::query1("originConnector", "Placement origin", QueryFilter::MATE_CONNECTOR).visible(|v| v.bool("realign")),
        ParamSpec::query1("horizontalReference", "Horizontal reference", QueryFilter::DIRECTION).visible(|v| v.bool("realign")),
        ParamSpec::bool("maintainAspectRatio", "Maintain aspect ratio", true),
        ParamSpec::enum_("aspectRatioConstraint", "Aspect ratio constraint", &[("WIDTH_DRIVING", "Width"), ("HEIGHT_DRIVING", "Height")]).visible(keep),
        ParamSpec::num("width", "Width", IMAGE_SIZE).visible(move |v| !keep(v) || v.is("aspectRatioConstraint", "WIDTH_DRIVING")),
        ParamSpec::num("height", "Height", IMAGE_SIZE).visible(move |v| !keep(v) || v.is("aspectRatioConstraint", "HEIGHT_DRIVING")),
        hidden(ParamSpec::num("imageOriginIndex", "Image origin index", NumSpec::integer(4, 0, 8))),
    ];
    FeatureType::new("decal", "Decal", Category::Solid, p).describe("Place an image on a planar or cylindrical face")
}
