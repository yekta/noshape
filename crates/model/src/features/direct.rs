//! Direct-editing schemas: Delete face, Move face, Replace face
//! (D-features-other.md §2, 03a PS-25..27). Modify fillet lives in
//! `detail.rs`, Offset surface in `surface.rs`.

use super::common::*;
use super::{Category, FeatureType};
use crate::params::{Hints, ParamSpec, ParamValues, QueryFilter};

pub fn register(out: &mut Vec<FeatureType>) {
    out.push(delete_face());
    out.push(move_face());
    out.push(replace_face());
}

pub fn delete_face() -> FeatureType {
    let p = vec![
        ParamSpec::query("deleteFaces", "Delete faces", QueryFilter::ANY_FACE).hints(Hints::INITIAL_FOCUS | Hints::SHOW_CREATE_SELECTION),
        ParamSpec::enum_("healType", "Type", &[("HEAL", "Heal"), ("CAP", "Cap"), ("VOID", "Leave open")]).hints(Hints::REMEMBER_PREVIOUS_VALUE),
        ParamSpec::bool("includeFillet", "Delete fillet faces", true),
    ];
    FeatureType::new("deleteFace", "Delete face", Category::Direct, p).describe("Remove faces and heal, cap or leave the body open")
}

pub fn move_face() -> FeatureType {
    let ty = |v: &ParamValues, t: &str| v.is("moveFaceType", t);
    let mv = |v: &ParamValues| v.is("outputType", "MOVE");
    let blind = move |v: &ParamValues| !ty(v, "ROTATE") && v.is("limitType", "BLIND");
    let up_to = move |v: &ParamValues| !ty(v, "ROTATE") && v.is("limitType", "UP_TO_ENTITY");
    let has_off = move |v: &ParamValues| up_to(v) && v.bool("hasOffset");
    let p = vec![
        hidden(ParamSpec::enum_("outputType", "Creation type", &[("MOVE", "Move"), ("CREATE", "Create")])),
        ParamSpec::query("moveFaces", "Faces to move", QueryFilter::ANY_FACE).hints(Hints::INITIAL_FOCUS | Hints::SHOW_CREATE_SELECTION).visible(mv),
        ParamSpec::query(
            "surfacesAndFaces",
            "Faces, surfaces, and sketch regions to offset",
            QueryFilter::ANY_FACE | QueryFilter::SKETCH_REGION | QueryFilter::SHEET_BODY,
        )
        .hints(Hints::SHOW_CREATE_SELECTION)
        .visible(move |v| !mv(v)),
        ParamSpec::enum_("moveFaceType", "Move face type", &[("OFFSET", "Offset"), ("TRANSLATE", "Translate"), ("ROTATE", "Rotate")]),
        ParamSpec::query1("direction", "Direction", QueryFilter::DIRECTION).visible(move |v| ty(v, "TRANSLATE")),
        ParamSpec::query1("axis", "Axis", QueryFilter::AXIS | QueryFilter::CIRCULAR_EDGE).visible(move |v| ty(v, "ROTATE")),
        ParamSpec::enum_("limitType", "End type", &[("BLIND", "Blind"), ("UP_TO_ENTITY", "Up to entity")])
            .hints(Hints::SHOW_LABEL)
            .visible(move |v| !ty(v, "ROTATE")),
        ParamSpec::num("translationDistance", "Distance", ZERO_INCLUSIVE_OFFSET).visible(move |v| ty(v, "TRANSLATE") && blind(v)),
        ParamSpec::num("angle", "Rotation angle", angle_360(0.0)).visible(move |v| ty(v, "ROTATE")),
        ParamSpec::num("offsetDistance", "Distance", ZERO_INCLUSIVE_OFFSET).visible(move |v| ty(v, "OFFSET") && blind(v)),
        ParamSpec::flip("oppositeDirection", "Opposite direction").visible(move |v| blind(v) || ty(v, "ROTATE")),
        ParamSpec::query1("limitEntity", "Up to entity", QueryFilter::PLANAR_FACE | QueryFilter::PLANE | QueryFilter::POINT_LIKE)
            .visible(move |v| ty(v, "TRANSLATE") && up_to(v)),
        ParamSpec::query1("limitQuery", "Up to entity", QueryFilter::POINT_LIKE | QueryFilter::ANY_EDGE | QueryFilter::ANY_FACE | QueryFilter::BODY)
            .visible(move |v| ty(v, "OFFSET") && up_to(v)),
        ParamSpec::bool("hasOffset", "Offset distance", false).hints(Hints::DISPLAY_SHORT | Hints::FIRST_IN_ROW).visible(up_to),
        ParamSpec::num("offset", "Offset", NONNEGATIVE_ZERO_DEFAULT).hints(Hints::DISPLAY_SHORT).visible(has_off),
        default_true(ParamSpec::flip("oppositeOffsetDirection", "Opposite offset direction")).visible(has_off),
        ParamSpec::bool("reFillet", "Reapply fillet", true).visible(mv),
    ];
    FeatureType::new("moveFace", "Move face", Category::Direct, p).describe("Offset, translate or rotate faces; neighbors follow")
}

pub fn replace_face() -> FeatureType {
    let p = vec![
        ParamSpec::query("replaceFaces", "Faces to replace", QueryFilter::ANY_FACE).hints(Hints::INITIAL_FOCUS | Hints::SHOW_CREATE_SELECTION),
        ParamSpec::query("templateFace", "Faces to replace with", QueryFilter::ANY_FACE | QueryFilter::PLANE),
        hidden(ParamSpec::bool("showFlip", "Show flip", true)),
        ParamSpec::bool("oppositeSense", "Flip alignment", false).visible(|v| v.bool("showFlip")),
        ParamSpec::num("offset", "Offset distance", NONNEGATIVE_ZERO_DEFAULT),
        ParamSpec::flip("oppositeDirection", "Opposite direction"),
    ];
    FeatureType::new("replaceFace", "Replace face", Category::Direct, p).describe("Trim or extend faces to the surface of other faces")
}
