//! Onshape's sheet metal form library: the Form tool's Libraries ›
//! "Onshape sheet metal forms" (the public "Onshape form library", V4;
//! live behaviours "skasm (2026-10-30) Onshape sheet metal forms library",
//! dom/sm-forms-v4/). Each form is a built-in Part Studio reached as
//! `library:<id>` (a Form's `formPartStudio`): its configuration inputs as
//! live's (named as live's variables, so std's Form fills `thickness` from
//! the sheet), a hidden `smFormTool` feature making the "Add material" and
//! "Subtract material" tools, and std's `tag` marking them FORM.

use crate::configuration::{ConfigInput, ConfigInputKind, ConfigValues, Configuration, ConfiguredParam, ListOption, VariableKind};
use crate::display::BodyKind;
use crate::params::ParamValue;
use crate::partstudio::PartStudio;
use crate::reference::{BodyId, Reference};
use crate::regen::context::Context;
use crate::regen::derived::DerivedProvider;
use crate::regen::eval::{EvalCtx, FResult, NamedShape, err};
use crate::regen::features::helpers_a::name_new;
use glam::{DVec2, DVec3};
use noshape_kernel as k;
use noshape_kernel::Shape;
use noshape_units::{ExprValue, UnitPrefs};
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, LazyLock, Mutex};

/// `formPartStudio` of a library form: `library:<id>`.
pub const PREFIX: &str = "library:";

/// A library form: its id and the name the picker lists.
pub struct Form {
    pub id: &'static str,
    pub name: &'static str,
}

/// The forms we build, in the picker's (alphabetical) order.
pub const FORMS: &[Form] = &[Form { id: "bridge-lance", name: "Bridge lance" }, Form { id: "circle-emboss", name: "Circle emboss" }, Form { id: "circle-extrusion", name: "Circle extrusion" }, Form { id: "countersink", name: "Countersink" }, Form { id: "lance", name: "Lance" }, Form { id: "louver", name: "Louver" }, Form { id: "slot-emboss", name: "Slot emboss" }, Form { id: "slot-extrusion", name: "Slot extrusion" }];

/// Radius control (live `List_WrgkY2rdTeUGo2`): the forms' bend radii.
const RADIUS_CONTROL: [(&str, &str); 4] = [("Default", "Double thickness"), ("Thickness", "Thickness"), ("Half_thickness", "Half thickness"), ("Custom", "Custom")];
const RADIUS_CONTROL_INPUT: &str = "Radius control";
/// Length to (live `List_rZVuoqSoy79i0I` / `List_OrKvSo3y1S6Ua4`): a slot's length between its ends' centres or tangents.
const LENGTH_TO: [(&str, &str); 2] = [("Default", "Slot centers"), ("Slot_tangents", "Slot tangents")];
const LENGTH_TO_INPUT: &str = "Length to";
/// Lance's Profile (live `List_g6XZAvs1qlzUG1`).
const PROFILE: [(&str, &str); 2] = [("Default", "Flat"), ("Straight", "Straight")];
const PROFILE_INPUT: &str = "Profile";

fn variable(name: &str, kind: VariableKind, default: &str) -> ConfigInput {
    ConfigInput { id: name.into(), name: name.into(), kind: ConfigInputKind::Variable { kind, default: ExprValue::new(default), min: None, max: None }, visible_when: None, excluded: false }
}

/// A form's configuration inputs (live's ids, defaults and order).
pub fn configuration(id: &str) -> Option<Configuration> {
    let radius_control = ConfigInput {
        id: RADIUS_CONTROL_INPUT.into(),
        name: RADIUS_CONTROL_INPUT.into(),
        kind: ConfigInputKind::List { options: RADIUS_CONTROL.iter().map(|(id, name)| ListOption { id: (*id).into(), name: (*name).into() }).collect(), default: "Default".into() },
        visible_when: None,
        excluded: false,
    };
    let length_to = ConfigInput {
        id: LENGTH_TO_INPUT.into(),
        name: LENGTH_TO_INPUT.into(),
        kind: ConfigInputKind::List { options: LENGTH_TO.iter().map(|(id, name)| ListOption { id: (*id).into(), name: (*name).into() }).collect(), default: "Default".into() },
        visible_when: None,
        excluded: false,
    };
    let inputs = match id {
        "circle-emboss" => vec![
            variable("thickness", VariableKind::Length, "1 mm"),
            variable("diameter", VariableKind::Length, "5 mm"),
            variable("height", VariableKind::Length, "5 mm"),
            variable("draft", VariableKind::Angle, "90 deg"),
            radius_control,
            variable("int_radius", VariableKind::Length, "1 mm"),
            variable("ext_radius", VariableKind::Length, "1 mm"),
        ],
        "circle-extrusion" => vec![
            variable("thickness", VariableKind::Length, "1 mm"),
            variable("diameter", VariableKind::Length, "5 mm"),
            variable("depth", VariableKind::Length, "1 mm"),
            variable("radius", VariableKind::Length, "1 mm"),
        ],
        "countersink" => vec![variable("thickness", VariableKind::Length, "1 mm"), variable("clearance_hole", VariableKind::Length, "10 mm"), variable("angle", VariableKind::Angle, "90 deg")],
        "bridge-lance" => vec![
            variable("thickness", VariableKind::Length, "1 mm"),
            variable("length", VariableKind::Length, "50 mm"),
            variable("width", VariableKind::Length, "20 mm"),
            variable("height", VariableKind::Length, "5 mm"),
            variable("draft", VariableKind::Angle, "90 deg"),
            radius_control,
            variable("int_radius", VariableKind::Length, "1 mm"),
            variable("ext_radius", VariableKind::Length, "1 mm"),
        ],
        "lance" => vec![
            variable("thickness", VariableKind::Length, "1 mm"),
            ConfigInput {
                id: PROFILE_INPUT.into(),
                name: PROFILE_INPUT.into(),
                kind: ConfigInputKind::List { options: PROFILE.iter().map(|(id, name)| ListOption { id: (*id).into(), name: (*name).into() }).collect(), default: "Default".into() },
                visible_when: None,
                excluded: false,
            },
            variable("length", VariableKind::Length, "20 mm"),
            variable("width", VariableKind::Length, "20 mm"),
            variable("height", VariableKind::Length, "5 mm"),
            variable("draft", VariableKind::Angle, "60 deg"),
            radius_control,
            variable("int_radius", VariableKind::Length, "1 mm"),
            variable("ext_radius", VariableKind::Length, "1 mm"),
        ],
        "louver" => vec![
            variable("thickness", VariableKind::Length, "1 mm"),
            variable("height", VariableKind::Length, "8 mm"),
            variable("length", VariableKind::Length, "100 mm"),
            variable("width", VariableKind::Length, "20 mm"),
            radius_control,
            variable("radius", VariableKind::Length, "1 mm"),
        ],
        "slot-emboss" => vec![
            variable("thickness", VariableKind::Length, "1 mm"),
            variable("length", VariableKind::Length, "100 mm"),
            length_to,
            variable("width", VariableKind::Length, "5 mm"),
            variable("height", VariableKind::Length, "5 mm"),
            variable("draft", VariableKind::Angle, "90 deg"),
            radius_control,
            variable("int_radius", VariableKind::Length, "1 mm"),
            variable("ext_radius", VariableKind::Length, "1 mm"),
        ],
        "slot-extrusion" => vec![
            variable("thickness", VariableKind::Length, "1 mm"),
            variable("length", VariableKind::Length, "100 mm"),
            length_to,
            variable("width", VariableKind::Length, "5 mm"),
            variable("depth", VariableKind::Length, "1 mm"),
            radius_control,
            variable("radius", VariableKind::Length, "1 mm"),
        ],
        _ => return None,
    };
    Some(Configuration { inputs, configured: Vec::new() })
}

/// The picker's label of a configuration input (live's names).
pub fn input_label(name: &str) -> &str {
    match name {
        "thickness" => "Thickness",
        "diameter" => "Diameter",
        "height" => "Height",
        "draft" => "Angle",
        "int_radius" => "Internal radius",
        "ext_radius" => "External radius",
        "depth" => "Depth",
        "radius" => "Radius",
        "clearance_hole" => "Clearance hole",
        "angle" => "Angle",
        "length" => "Length",
        "width" => "Width",
        other => other,
    }
}

static STUDIOS: LazyLock<Mutex<HashMap<String, PartStudio>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// The built-in Part Studio of form `id` (made once: its feature ids stay).
pub fn studio(id: &str) -> Option<PartStudio> {
    if let Some(s) = STUDIOS.lock().ok()?.get(id) {
        return Some(s.clone());
    }
    let s = make_studio(id)?;
    STUDIOS.lock().ok()?.insert(id.to_string(), s.clone());
    Some(s)
}

fn make_studio(id: &str) -> Option<PartStudio> {
    let form = FORMS.iter().find(|f| f.id == id)?;
    let units = UnitPrefs::default();
    let mut ps = PartStudio::new();
    ps.configuration = configuration(id)?;
    // One feature per tool (live's two parts come from different features;
    // std's instantiation keeps the negative one's FORM attribute then).
    let mut ids = Vec::new();
    for (part, name) in [("ADD", "Add material"), ("SUBTRACT", "Subtract material")] {
        let mut tool = ps.new_feature("smFormTool", &units)?;
        tool.name = format!("{} {name}", form.name);
        let expr = |s: &str| ParamValue::Expr(ExprValue::new(s));
        tool.params.set("form", ParamValue::Text(id.into()));
        tool.params.set("part", ParamValue::Enum(part.into()));
        for (param, var) in [("thickness", "thickness"), ("diameter", "diameter"), ("height", "height"), ("draft", "draft"), ("intRadius", "int_radius"), ("extRadius", "ext_radius"), ("depth", "depth"), ("radius", "radius"), ("clearanceHole", "clearance_hole"), ("angle", "angle"), ("length", "length"), ("width", "width")] {
            if ps.configuration.inputs.iter().any(|i| i.name == var) {
                tool.params.set(param, expr(&format!("#{var}")));
            }
        }
        let fid = tool.id;
        ids.push(fid);
        ps.features.push(tool);
        for (input, param, options) in [(RADIUS_CONTROL_INPUT, "radiusControl", &RADIUS_CONTROL[..]), (LENGTH_TO_INPUT, "lengthTo", &LENGTH_TO[..]), (PROFILE_INPUT, "profile", &PROFILE[..])] {
            if ps.configuration.inputs.iter().any(|i| i.id == input) {
                ps.configuration.configured.push(ConfiguredParam {
                    input: input.into(),
                    feature: fid,
                    param: param.into(),
                    values: options.iter().map(|(o, _)| ((*o).to_string(), ParamValue::Enum((*o).into()))).collect::<BTreeMap<_, _>>(),
                });
            }
        }
    }
    let mut tag = ps.new_feature("tag", &units)?;
    tag.params.set("tagPurpose", ParamValue::Enum("FORM".into()));
    tag.params.set("positivePart", ParamValue::Query(vec![Reference::Body(BodyId { feature: ids[0], index: 0 })]));
    tag.params.set("negativePart", ParamValue::Query(vec![Reference::Body(BodyId { feature: ids[1], index: 0 })]));
    ps.features.push(tag);
    Some(ps)
}

/// `smFormTool`: the form's Add material or Subtract material tool
/// (`part`), the sheet's top at z = 0, the sheet under it.
pub fn tool_feature(ev: &mut EvalCtx) -> FResult<()> {
    let form = ev.params.text("form").to_string();
    let t = ev.length("thickness")?;
    if t <= 0.0 {
        return err("The thickness must be positive");
    }
    let control = ev.enum_("radiusControl").to_string();
    let radius = |ev: &EvalCtx, custom: &str| -> FResult<f64> {
        Ok(match control.as_str() {
            "Thickness" => t,
            "Half_thickness" => t * 0.5,
            "Custom" => ev.length(custom)?,
            _ => 2.0 * t,
        })
    };
    let tools = match form.as_str() {
        "circle-emboss" => circle_emboss(t, ev.length("diameter")?, ev.length("height")?, ev.angle("draft")?, radius(ev, "intRadius")?, radius(ev, "extRadius")?)?,
        "circle-extrusion" => circle_extrusion(t, ev.length("diameter")?, ev.length("depth")?, ev.length("radius")?)?,
        "countersink" => countersink(t, ev.length("clearanceHole")?, ev.angle("angle")?)?,
        "louver" => louver(t, ev.length("height")?, ev.length("length")?, ev.length("width")?, radius(ev, "radius")?)?,
        "lance" => lance(t, ev.length("length")?, ev.length("width")?, ev.length("height")?, ev.angle("draft")?, radius(ev, "intRadius")?, radius(ev, "extRadius")?)?,
        "bridge-lance" => bridge_lance(t, ev.length("length")?, ev.length("width")?, ev.length("height")?, ev.angle("draft")?, radius(ev, "intRadius")?, radius(ev, "extRadius")?)?,
        "slot-emboss" | "slot-extrusion" => {
            let (length, width) = (ev.length("length")?, ev.length("width")?);
            let tangents = ev.enum_("lengthTo") == "Slot_tangents";
            if form == "slot-emboss" {
                slot_emboss(t, length, tangents, width, ev.length("height")?, ev.angle("draft")?, radius(ev, "intRadius")?, radius(ev, "extRadius")?)?
            } else {
                slot_extrusion(t, length, tangents, width, ev.length("depth")?, radius(ev, "radius")?)?
            }
        }
        _ => return err("Unknown library form"),
    };
    let Some(s) = tools.into_iter().nth(if ev.enum_("part") == "SUBTRACT" { 1 } else { 0 }) else { return err("The form has no such tool") };
    let names = name_new(ev, &s);
    ev.apply_operation(vec![NamedShape::new(s, names)], "NEW", None, BodyKind::Solid)
}

/// The profile plane of a revolved tool: local (u, v) = world (r, z).
fn profile_plane() -> k::Plane {
    k::Plane::new(DVec3::ZERO, DVec3::NEG_Y, DVec3::X)
}

/// The emboss cross-section (Circle emboss, Slot emboss; live features): a
/// sheet `t` thick bent up into a cone and a flat top, r from the axis (or
/// the slot's centre line). Its outer (top) surfaces' sharp corners sit at
/// r = `d` / 2 and height `h` (clamped to t / 2 … 10 t); the cone stands at
/// half the full `draft` angle from the vertical; the top's inner bend
/// radius `ri`, the base's `re`. With the r where the base bend meets the
/// sheet (Subtract material's edge).
fn emboss_profile(t: f64, d: f64, h: f64, draft: f64, ri: f64, re: f64) -> FResult<(Vec<k::Segment2d>, f64)> {
    let (th, ht) = (draft * 0.5, h.clamp(t * 0.5, 10.0 * t));
    let alpha = std::f64::consts::FRAC_PI_2 - th;
    let r_top = (d * 0.5 - (ri + t) * (alpha * 0.5).tan()).max(0.0);
    let (segs, r, _) = bent_profile(t, r_top, ht, th, ri, re)?;
    Ok((segs, r))
}

/// A sheet `t` thick bent from a flat top (its mid line from r 0 to
/// `r_top`, its top at `h`) over an inner bend `ri` down a wall `th` from
/// the vertical and over a base bend `re` into the sheet (its top at z 0):
/// the (r, z) section closed by the line at r 0, with the r where the base
/// bend meets the sheet and the mid surface's developed length.
fn bent_profile(t: f64, r_top: f64, h: f64, th: f64, ri: f64, re: f64) -> FResult<(Vec<k::Segment2d>, f64, f64)> {
    if !(-1e-9..std::f64::consts::FRAC_PI_2 - 1e-6).contains(&th) || ri <= 0.0 || re <= 0.0 {
        return err("The form's angle and radii must be positive");
    }
    let alpha = std::f64::consts::FRAC_PI_2 - th;
    // The mid surface: top at h - t/2, bends of the radii + t/2.
    let (zt, rmi, rme) = (h - t * 0.5, ri + t * 0.5, re + t * 0.5);
    let ci = DVec2::new(r_top, zt - rmi);
    // The wall's mid line leaves the top bend tangentially, down and out.
    let dir = DVec2::new(th.sin(), -th.cos());
    let up = DVec2::new(th.cos(), th.sin());
    let w0 = ci + up * rmi;
    // Base bend centre: rme above the sheet's mid plane, rme off the wall's mid line.
    let zc = -t * 0.5 + rme;
    let q0 = w0 + up * rme;
    let ce = q0 + dir * ((zc - q0.y) / dir.y);
    let wall = (ce - up * rme - w0).dot(dir).max(0.0);
    let r_sub = ce.x;
    let (a_top, a_cone) = (std::f64::consts::FRAC_PI_2, th);
    let (b_cone, b_base) = (std::f64::consts::PI + th, 1.5 * std::f64::consts::PI);
    let at = |c: DVec2, r: f64, a: f64| c + DVec2::new(a.cos(), a.sin()) * r;
    let arc = |c: DVec2, r: f64, a0: f64, a1: f64| k::Segment2d::Arc { center: c, radius: r, start_angle: a0.min(a1), end_angle: a0.max(a1) };
    let mut segs = Vec::new();
    // Outer (top) side, from the axis out to the sheet; the end; the inner side back.
    line(DVec2::new(0.0, h), DVec2::new(r_top, h), &mut segs);
    segs.push(arc(ci, rmi + t * 0.5, a_top, a_cone));
    line(at(ci, rmi + t * 0.5, a_cone), at(ce, rme - t * 0.5, b_cone), &mut segs);
    segs.push(arc(ce, rme - t * 0.5, b_cone, b_base));
    line(DVec2::new(r_sub, 0.0), DVec2::new(r_sub, -t), &mut segs);
    segs.push(arc(ce, rme + t * 0.5, b_cone, b_base));
    line(at(ce, rme + t * 0.5, b_cone), at(ci, rmi - t * 0.5, a_cone), &mut segs);
    segs.push(arc(ci, rmi - t * 0.5, a_top, a_cone));
    line(DVec2::new(r_top, h - t), DVec2::new(0.0, h - t), &mut segs);
    line(DVec2::new(0.0, h - t), DVec2::new(0.0, h), &mut segs);
    Ok((segs, r_sub, r_top + alpha * (rmi + rme) + wall))
}

/// Lance (live "Lance"; its faces, dom/sm-forms-v4/lance__bodydetails): a
/// strip `width` wide (at least t) cut free on three sides and bent up: its
/// flat top `length` long from the free end, its wall at `draft` from the
/// horizontal, the bends `ri` / `re`. Subtract material: the slit, as long
/// as the strip unbent (live t 1, h 5, 100 × 1, 90°: 107.853982 both).
/// Live places it by a mate connector we don't know the place of: here the
/// slit's middle sits on the location. The Straight profile is built as
/// Flat.
#[allow(clippy::too_many_arguments)]
fn lance(t: f64, length: f64, width: f64, h: f64, draft: f64, ri: f64, re: f64) -> FResult<Vec<Shape>> {
    let w = width.max(t);
    let th = (std::f64::consts::FRAC_PI_2 - draft).max(0.0);
    let (segs, x_end, developed) = bent_profile(t, length.max(0.0), h.clamp(t * 0.5, 10.0 * t), th, ri, re)?;
    let shift = x_end - developed * 0.5;
    let plane = k::Plane::new(DVec3::new(-shift, -w * 0.5, 0.0), DVec3::NEG_Y, DVec3::X);
    let add = k::extrude(&k::make_face_2d(&plane, &segs, &[])?, DVec3::Y * w)?.shape;
    let sub = k::make_box(&k::Plane::new(DVec3::new(-developed * 0.5, -w * 0.5, -t), DVec3::Z, DVec3::X), developed, w, t)?;
    Ok(vec![add, sub])
}

/// A line of a profile, unless it has no length.
fn line(a: DVec2, b: DVec2, segs: &mut Vec<k::Segment2d>) {
    if a.distance(b) > 1e-12 {
        segs.push(k::Segment2d::Line { a, b });
    }
}

/// Circle emboss: the emboss revolved; Subtract material the sheet's disc
/// out to the base bend (live t 1: 273.79979 / 217.909344 mm³; on a 2 mm
/// plate +94.552620).
fn circle_emboss(t: f64, d: f64, h: f64, draft: f64, ri: f64, re: f64) -> FResult<Vec<Shape>> {
    let (segs, r) = emboss_profile(t, d, h, draft, ri, re)?;
    Ok(vec![revolved(&segs)?, sheet_disc(t, r)?])
}

/// Slot emboss: the emboss run along a slot of `length` between its ends'
/// centres (or tangents) and width `width` (live: no measured tools; its
/// features are Circle emboss's along the slot).
#[allow(clippy::too_many_arguments)]
fn slot_emboss(t: f64, length: f64, tangents: bool, width: f64, h: f64, draft: f64, ri: f64, re: f64) -> FResult<Vec<Shape>> {
    let l = slot_length(length, tangents, width.max(t));
    let (segs, r) = emboss_profile(t, width, h, draft, ri, re)?;
    Ok(vec![slot_swept(&segs, l)?, sheet_stadium(t, r, l)?])
}

/// The straight run between a slot's end centres (live's Length variable:
/// tangent lengths less twice the width).
fn slot_length(length: f64, tangents: bool, width: f64) -> f64 {
    if tangents { (length - 2.0 * width).max(0.0) } else { length.max(0.0) }
}

/// A disc of radius `r` through the sheet (z -t … 0).
fn sheet_disc(t: f64, r: f64) -> FResult<Shape> {
    let disc = k::make_face_2d(&k::Plane::new(DVec3::new(0.0, 0.0, -t), DVec3::Z, DVec3::X), &[k::Segment2d::Circle { center: DVec2::ZERO, radius: r }], &[])?;
    Ok(k::extrude(&disc, DVec3::Z * t)?.shape)
}

/// A slot's stadium (half-width `r`, end centres `l` apart along x)
/// through the sheet.
fn sheet_stadium(t: f64, r: f64, l: f64) -> FResult<Shape> {
    if l <= 1e-12 {
        return sheet_disc(t, r);
    }
    let (h, q) = (l * 0.5, std::f64::consts::FRAC_PI_2);
    let segs = [
        k::Segment2d::Line { a: DVec2::new(-h, -r), b: DVec2::new(h, -r) },
        k::Segment2d::Arc { center: DVec2::new(h, 0.0), radius: r, start_angle: -q, end_angle: q },
        k::Segment2d::Line { a: DVec2::new(h, r), b: DVec2::new(-h, r) },
        k::Segment2d::Arc { center: DVec2::new(-h, 0.0), radius: r, start_angle: q, end_angle: 3.0 * q },
    ];
    let face = k::make_face_2d(&k::Plane::new(DVec3::new(0.0, 0.0, -t), DVec3::Z, DVec3::X), &segs, &[])?;
    Ok(k::extrude(&face, DVec3::Z * t)?.shape)
}

/// A closed (r, z) profile of lines and arcs revolved about the z axis.
fn revolved(segs: &[k::Segment2d]) -> FResult<Shape> {
    let face = k::make_face_2d(&profile_plane(), segs, &[])?;
    Ok(k::revolve(&face, DVec3::ZERO, DVec3::Z, std::f64::consts::TAU)?.shape)
}

/// An (r, z) profile along a slot: half revolved round each end's centre
/// (x = ±l / 2) and run straight between them on both sides (live: the
/// half Revolves, Extrudes, Splits and Mirrors); `l` 0: revolved.
fn slot_swept(segs: &[k::Segment2d], l: f64) -> FResult<Shape> {
    if l <= 1e-12 {
        return revolved(segs);
    }
    let (h, pi, q) = (l * 0.5, std::f64::consts::PI, std::f64::consts::FRAC_PI_2);
    let mut pieces = Vec::new();
    for (x, a0) in [(h, -q), (-h, q)] {
        let c = DVec3::new(x, 0.0, 0.0);
        let face = k::make_face_2d(&k::Plane::new(c, DVec3::NEG_Y, DVec3::X), segs, &[])?;
        pieces.push(k::revolve_between(&face, c, DVec3::Z, a0, a0 + pi)?.shape);
    }
    for (n, u) in [(DVec3::X, DVec3::Y), (DVec3::NEG_X, DVec3::NEG_Y)] {
        let face = k::make_face_2d(&k::Plane::new(DVec3::new(-h, 0.0, 0.0), n, u), segs, &[])?;
        pieces.push(k::extrude(&face, DVec3::X * l)?.shape);
    }
    let fused = k::fuse_all(&pieces, &k::BooleanOptions::default())?.shape;
    Ok(k::unify_same_domain(&fused)?.shape)
}

/// The extrusion cross-section (Circle extrusion, Slot extrusion; live
/// features): a hole of width `d` (at least t) whose rim is bent down into a
/// collar `depth` deep (t … (d + t) / 2) round an outer radius radius + t
/// (≤ the depth). With the r where the bend meets the sheet.
fn extrusion_profile(t: f64, d: f64, depth: f64, radius: f64) -> (Vec<k::Segment2d>, f64) {
    let d = d.max(t);
    let depth = depth.clamp(t, (d + t) * 0.5);
    let r = (radius + t).clamp(0.0, depth);
    let (h, c) = (d * 0.5, DVec2::new(d * 0.5 + r, -r));
    let (pi, half) = (std::f64::consts::PI, std::f64::consts::FRAC_PI_2);
    let mut segs = Vec::new();
    // Bottom of the collar, its outer wall up, the inner bend, the sheet's
    // edge, the outer bend down to the hole's wall.
    line(DVec2::new(h, -depth), DVec2::new(h + t, -depth), &mut segs);
    line(DVec2::new(h + t, -depth), DVec2::new(h + t, -r), &mut segs);
    if r - t > 1e-12 {
        segs.push(k::Segment2d::Arc { center: c, radius: r - t, start_angle: half, end_angle: pi });
    }
    line(DVec2::new(h + r, -t), DVec2::new(h + r, 0.0), &mut segs);
    segs.push(k::Segment2d::Arc { center: c, radius: r, start_angle: half, end_angle: pi });
    line(DVec2::new(h, -r), DVec2::new(h, -depth), &mut segs);
    (segs, h + r)
}

/// Circle extrusion: the collar revolved; Subtract material the sheet's
/// disc out to the bend (live t 1: 15.177413 / 38.484510 mm³).
fn circle_extrusion(t: f64, d: f64, depth: f64, radius: f64) -> FResult<Vec<Shape>> {
    let (segs, r) = extrusion_profile(t, d, depth, radius);
    Ok(vec![revolved(&segs)?, sheet_disc(t, r)?])
}

/// Slot extrusion: the collar along a slot (live t 1, 100 × 5: 172.257045
/// / 738.484510 mm³).
fn slot_extrusion(t: f64, length: f64, tangents: bool, width: f64, depth: f64, radius: f64) -> FResult<Vec<Shape>> {
    let w = width.max(t);
    let l = slot_length(length, tangents, w);
    let (segs, r) = extrusion_profile(t, w, depth, radius);
    Ok(vec![slot_swept(&segs, l)?, sheet_stadium(t, r, l)?])
}

/// Countersink (live "Countersink"): a hole of the clearance diameter, its
/// cone at half the `angle` down to 0.1 t above the sheet's bottom.
/// Subtract material: that countersunk hole; Add material: the rest of the
/// sheet's disc out to the cone's top (live t 1, Ø1, 60°: 1.491592 /
/// 1.774456 mm³).
fn countersink(t: f64, clearance: f64, angle: f64) -> FResult<Vec<Shape>> {
    let half = angle * 0.5;
    if !(1e-6..std::f64::consts::FRAC_PI_2 - 1e-6).contains(&half) || clearance <= 0.0 {
        return err("The countersink's angle and hole must be positive");
    }
    let rh = clearance * 0.5;
    let top = rh + 0.9 * t * half.tan();
    let hole = [
        k::Segment2d::Line { a: DVec2::new(0.0, -t), b: DVec2::new(rh, -t) },
        k::Segment2d::Line { a: DVec2::new(rh, -t), b: DVec2::new(rh, -0.9 * t) },
        k::Segment2d::Line { a: DVec2::new(rh, -0.9 * t), b: DVec2::new(top, 0.0) },
        k::Segment2d::Line { a: DVec2::new(top, 0.0), b: DVec2::new(0.0, 0.0) },
        k::Segment2d::Line { a: DVec2::new(0.0, 0.0), b: DVec2::new(0.0, -t) },
    ];
    let ring = [
        k::Segment2d::Line { a: DVec2::new(rh, -t), b: DVec2::new(top, -t) },
        k::Segment2d::Line { a: DVec2::new(top, -t), b: DVec2::new(top, 0.0) },
        k::Segment2d::Line { a: DVec2::new(top, 0.0), b: DVec2::new(rh, -0.9 * t) },
        k::Segment2d::Line { a: DVec2::new(rh, -0.9 * t), b: DVec2::new(rh, -t) },
    ];
    Ok(vec![revolved(&ring)?, revolved(&hole)?])
}

/// The profile mirrored across its axis (r -> -r).
fn mirrored(segs: &[k::Segment2d]) -> Vec<k::Segment2d> {
    let pi = std::f64::consts::PI;
    segs.iter()
        .map(|s| match *s {
            k::Segment2d::Line { a, b } => k::Segment2d::Line { a: DVec2::new(-a.x, a.y), b: DVec2::new(-b.x, b.y) },
            k::Segment2d::Arc { center, radius, start_angle, end_angle } => k::Segment2d::Arc { center: DVec2::new(-center.x, center.y), radius, start_angle: pi - end_angle, end_angle: pi - start_angle },
            ref other => other.clone(),
        })
        .collect()
}

/// Bridge lance (live "Bridge lance"): the emboss section (its top's
/// corners `length` apart) run straight across a strip `width` wide (at
/// least t) between two slits. Subtract material: the strip's footprint
/// (live t 1, h 5, 100 × 1: 115.368700 / 111.656854 mm³).
#[allow(clippy::too_many_arguments)]
fn bridge_lance(t: f64, length: f64, width: f64, h: f64, draft: f64, ri: f64, re: f64) -> FResult<Vec<Shape>> {
    let w = width.max(t);
    let (half, r) = emboss_profile(t, length.max(0.0), h, draft, ri, re)?;
    let plane = k::Plane::new(DVec3::new(0.0, -w * 0.5, 0.0), DVec3::NEG_Y, DVec3::X);
    let mut pieces = Vec::new();
    for segs in [half.clone(), mirrored(&half)] {
        let face = k::make_face_2d(&plane, &segs, &[])?;
        pieces.push(k::extrude(&face, DVec3::Y * w)?.shape);
    }
    let add = k::unify_same_domain(&k::fuse_all(&pieces, &k::BooleanOptions::default())?.shape)?.shape;
    let sub = k::make_box(&k::Plane::new(DVec3::new(-r, -w * 0.5, -t), DVec3::Z, DVec3::X), 2.0 * r, w, t)?;
    Ok(vec![add, sub])
}

/// Louver (live "Louver"; its faces, dom/sm-forms-v4/louver__bodydetails):
/// a hood opening along y = 0 into +y. Its outer surface a cylinder along
/// x with spheres at the ends (x = ±(l/2 - w)), through (0, w, 0) and (0, 0,
/// h) in section; joined to the sheet's top by a fillet of radius `rf`
/// (`rf` + t underneath); the sheet `t` thick inside. Height t … 10 t, width
/// at least 2 h, length at least 2 w. Subtract material: the D-shaped
/// footprint to where the fillets meet the sheet (live t 1, h 5, 100 × 20:
/// 1975.362600 / 1889.368549 mm³).
fn louver(t: f64, height: f64, length: f64, width: f64, rf: f64) -> FResult<Vec<Shape>> {
    let h = height.clamp(t, 10.0 * t);
    let w = width.max(2.0 * h);
    let l = length.max(2.0 * w);
    let a = l * 0.5 - w;
    let ro = (w * w + h * h) / (2.0 * h);
    let co = DVec2::new(0.0, h - ro);
    let yf = ((ro + rf).powi(2) - (rf - co.y).powi(2)).sqrt();
    let cf = DVec2::new(yf, rf);
    let toward = (cf - co).normalize();
    let (to, tf) = (toward.y.atan2(toward.x), toward.y.atan2(toward.x) + std::f64::consts::PI);
    let (top, bottom) = (std::f64::consts::FRAC_PI_2, 1.5 * std::f64::consts::PI);
    // (y, z): the opening at y = 0, the hood, the fillets, the footprint's edge.
    let segs = vec![
        k::Segment2d::Arc { center: co, radius: ro, start_angle: to, end_angle: top },
        k::Segment2d::Arc { center: cf, radius: rf, start_angle: tf, end_angle: bottom },
        k::Segment2d::Line { a: DVec2::new(yf, 0.0), b: DVec2::new(yf, -t) },
        k::Segment2d::Arc { center: cf, radius: rf + t, start_angle: tf, end_angle: bottom },
        k::Segment2d::Arc { center: co, radius: ro - t, start_angle: to, end_angle: top },
        k::Segment2d::Line { a: DVec2::new(0.0, h - t), b: DVec2::new(0.0, h) },
    ];
    let half = std::f64::consts::FRAC_PI_2;
    let mut pieces = Vec::new();
    for (x, a0, a1) in [(a, -half, 0.0), (-a, 0.0, half)] {
        let c = DVec3::new(x, 0.0, 0.0);
        let face = k::make_face_2d(&k::Plane::new(c, DVec3::X, DVec3::Y), &segs, &[])?;
        pieces.push(k::revolve_between(&face, c, DVec3::Z, a0, a1)?.shape);
    }
    if a > 1e-12 {
        let face = k::make_face_2d(&k::Plane::new(DVec3::new(-a, 0.0, 0.0), DVec3::X, DVec3::Y), &segs, &[])?;
        pieces.push(k::extrude(&face, DVec3::X * (2.0 * a))?.shape);
    }
    let add = k::unify_same_domain(&k::fuse_all(&pieces, &k::BooleanOptions::default())?.shape)?.shape;
    let foot = [
        k::Segment2d::Line { a: DVec2::new(-a - yf, 0.0), b: DVec2::new(a + yf, 0.0) },
        k::Segment2d::Arc { center: DVec2::new(a, 0.0), radius: yf, start_angle: 0.0, end_angle: half },
        k::Segment2d::Line { a: DVec2::new(a, yf), b: DVec2::new(-a, yf) },
        k::Segment2d::Arc { center: DVec2::new(-a, 0.0), radius: yf, start_angle: half, end_angle: 2.0 * half },
    ];
    let face = k::make_face_2d(&k::Plane::new(DVec3::new(0.0, 0.0, -t), DVec3::Z, DVec3::X), &foot, &[])?;
    let sub = k::extrude(&face, DVec3::Z * t)?.shape;
    Ok(vec![add, sub])
}

type Built = Mutex<HashMap<(String, ConfigValues), Option<Arc<Context>>>>;
static BUILT: LazyLock<Built> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// Form `id` regenerated in `values` (unset inputs at their defaults).
fn built(id: &str, values: &ConfigValues) -> Option<Arc<Context>> {
    let studio = studio(id)?;
    let mut all = studio.configuration.defaults();
    all.0.extend(values.0.iter().map(|(k, v)| (k.clone(), v.clone())));
    let key = (id.to_string(), all.clone());
    if let Some(c) = BUILT.lock().ok()?.get(&key) {
        return c.clone();
    }
    let r = crate::regen::Regenerator::new().regenerate(&studio, &all, &UnitPrefs::default(), &crate::regen::RegenOptions::default());
    let ok = r.display.features.iter().all(|f| !matches!(f.status, crate::display::FeatureStatus::Error));
    let ctx = ok.then_some(r.context);
    BUILT.lock().ok()?.insert(key, ctx.clone());
    ctx
}

/// A derived-source provider that also knows the library's forms.
#[derive(Debug)]
pub struct WithLibrary(pub Option<Arc<dyn DerivedProvider>>);

impl DerivedProvider for WithLibrary {
    fn context(&self, tab: &str) -> Option<Arc<Context>> {
        match tab.strip_prefix(PREFIX) {
            Some(id) => built(id, &ConfigValues::default()),
            None => self.0.as_ref()?.context(tab),
        }
    }
    fn context_configured(&self, tab: &str, values: &ConfigValues) -> Option<Arc<Context>> {
        match tab.strip_prefix(PREFIX) {
            Some(id) => built(id, values),
            None => self.0.as_ref()?.context_configured(tab, values),
        }
    }
    fn version(&self, tab: &str) -> u64 {
        match tab.strip_prefix(PREFIX) {
            Some(_) => 1,
            None => self.0.as_ref().map_or(0, |d| d.version(tab)),
        }
    }
    fn file(&self, key: &str) -> Option<crate::file_tab::FileTab> {
        self.0.as_ref()?.file(key)
    }
    fn configuration(&self, tab: &str) -> Option<Configuration> {
        match tab.strip_prefix(PREFIX) {
            Some(id) => configuration(id),
            None => self.0.as_ref()?.configuration(tab),
        }
    }
    fn feature_type(&self, tab: &str, feature: crate::reference::FeatureId) -> Option<String> {
        match tab.strip_prefix(PREFIX) {
            Some(id) => studio(id)?.features.iter().find(|f| f.id == feature).map(|f| f.type_id.clone()),
            None => self.0.as_ref()?.feature_type(tab, feature),
        }
    }
    fn part_name(&self, tab: &str, part: &BodyId) -> Option<String> {
        self.0.as_ref()?.part_name(tab, part)
    }
    fn part_appearance(&self, tab: &str, part: &BodyId) -> Option<[f32; 4]> {
        self.0.as_ref()?.part_appearance(tab, part)
    }
}
