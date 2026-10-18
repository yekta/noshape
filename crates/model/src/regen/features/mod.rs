//! Feature evaluators, dispatched by feature type id.

use super::eval::{EvalCtx, FResult};

pub mod basic;
pub mod detail;
pub mod extrude;
pub mod revolve;
pub mod sketch;
pub mod thin;
// --- modules: batch A (solid/detail/pattern/direct) ---
pub mod detail_a;
pub mod direct;
pub mod helpers_a;
pub mod hole;
pub mod pattern;
pub mod solid_a;
pub mod split;
// --- end batch A modules ---
// --- modules: batch B (reference/curves/surfaces/sheet metal/import) ---
pub mod bshared;
pub mod curves;
pub mod importing;
pub mod refgeom;
pub mod sheetmetal;
pub mod surfaces;
// --- end batch B modules ---
// --- modules: batch SM (sheet metal operations, frames) ---
pub mod frames;
pub mod frames_profiles;
pub mod sheetmetal_flange;
pub mod sheetmetal_ops;
pub mod sheetmetal_util;
// --- end batch SM modules ---

pub type EvalFn = fn(&mut EvalCtx) -> FResult<()>;

pub fn evaluator(type_id: &str) -> Option<EvalFn> {
    Some(match type_id {
        "sketch" => sketch::eval,
        "extrude" => extrude::eval,
        "revolve" => revolve::eval,
        "fillet" => detail::fillet,
        "chamfer" => detail::chamfer,
        "shell" => detail::shell,
        "booleanBodies" => basic::boolean,
        "cPlane" => basic::plane,
        "assignVariable" => basic::variable,
        "deleteBodies" => basic::delete_bodies,
        // --- dispatch: batch A ---
        "sweep" => solid_a::sweep,
        "loft" => solid_a::loft,
        "thicken" => solid_a::thicken,
        "enclose" => solid_a::enclose,
        "rib" => solid_a::rib,
        "cube" => solid_a::cube,
        "fSphere" => solid_a::sphere,
        "hole" => hole::hole,
        "externalThread" => hole::external_thread,
        "draft" => detail_a::draft,
        "faceBlend" => detail_a::face_blend,
        "modifyFillet" => detail_a::modify_fillet,
        "bodyDraft" => detail_a::body_draft,
        "splitPart" => split::split_part,
        "linearPattern" => pattern::linear_pattern,
        "circularPattern" => pattern::circular_pattern,
        "curvePattern" => pattern::curve_pattern,
        "mirror" => pattern::mirror,
        "transform" => pattern::transform,
        "copyPart" => pattern::copy_part,
        "deleteFace" => direct::delete_face,
        "moveFace" => direct::move_face,
        "replaceFace" => direct::replace_face,
        // --- end batch A dispatch ---
        // --- dispatch: batch B ---
        "mateConnector" => refgeom::mate_connector,
        "cPoint" => refgeom::point,
        "compositePart" => refgeom::composite_part,
        "queryVariable" => refgeom::query_variable,
        "nameEntity" => refgeom::name_entity,
        "helix" => curves::helix,
        "fitSpline" => curves::fit_spline,
        "projectCurves" => curves::project_curves,
        "bridgingCurve" => curves::bridging_curve,
        "compositeCurve" => curves::composite_curve,
        "intersectionCurve" => curves::intersection_curve,
        "trimCurve" => curves::trim_curve,
        "isocline" => curves::isocline,
        "offsetCurveOnFace" => curves::offset_curve,
        "isoparametricCurve" => curves::isoparametric_curve,
        "editCurve" => curves::edit_curve,
        "routingCurve" => curves::routing_curve,
        "offsetSurface" => surfaces::offset_surface,
        "fill" => surfaces::fill,
        "extendSurface" => surfaces::extend_surface,
        "boundarySurface" => surfaces::boundary_surface,
        "ruledSurface" => surfaces::ruled_surface,
        "mutualTrim" => surfaces::mutual_trim,
        "constrainedSurface" => surfaces::constrained_surface,
        "sheetMetalStart" => sheetmetal::sheet_metal_start,
        "sheetMetalEnd" => sheetmetal::sheet_metal_end,
        "sheetMetalFlange" => sheetmetal::flange,
        "sheetMetalHem" => sheetmetal::hem,
        "sheetMetalTab" => sheetmetal::tab,
        "sheetMetalBend" => sheetmetal::bend,
        "sheetMetalJog" => sheetmetal::jog,
        "sheetMetalFormed" => sheetmetal::form,
        "sheetMetalLoft" => sheetmetal::loft,
        "sheetMetalMakeJoint" => sheetmetal::make_joint,
        "sheetMetalJoint" => sheetmetal::modify_joint,
        "sheetMetalCorner" => sheetmetal::corner,
        "sheetMetalBendRelief" => sheetmetal::bend_relief,
        "sheetMetalCornerBreak" => sheetmetal::corner_break,
        "sheetMetalRip" => sheetmetal::rip,
        "sheetMetalRecognize" => sheetmetal::recognize,
        "importDerived" => importing::import_derived,
        "derivedMirror" => importing::derived_mirror,
        "importForeign" => importing::import_foreign,
        "wrap" => importing::wrap,
        "decal" => importing::decal,
        // --- end batch B dispatch ---
        // --- dispatch: batch SM ---
        "frame" => frames::frame,
        "frameTrim" => frames::frame_trim,
        "gusset" => frames::gusset,
        "endcap" => frames::end_cap,
        "cutlist" => frames::cutlist,
        "tag" => frames::tag,
        // --- end batch SM dispatch ---
        _ => return None,
    })
}
