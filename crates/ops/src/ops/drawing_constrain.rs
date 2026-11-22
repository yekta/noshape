//! drawing_constrain: the drawing sketch's constraint tools (h, v, i, p, b, t).

use super::doc_field;
use super::drawing::{commit, drawing_field, point_value, reply, sheet_field};
use super::drawing_tools::target;
use crate::error::OpError;
use crate::registry::{Args, Op, OpOutput, opt, req};
use crate::Ctx;
use noshape_scene::drawing_tools::{SkCon, Tool, constraint_ready, sketch_constraint};
use serde_json::json;

pub(super) fn ops() -> Vec<Op> {
    vec![Op {
        group: "drawing",
        verb: "constrain",
        summary: "Constrain sheet sketch geometry: horizontal, vertical, coincident, perpendicular, parallel, tangent",
        description: "The drawing's sketch constraint tools. `picks` are clicks (sheet mm) as with the tool on: each takes a sheet sketch point (within 6 px), else a sketch curve, else a view's straight edge or face-on circle (copied into the sketch as fixed geometry first, its own edit \"Use view geometry\"); a constraint is made each time the picks complete it (Horizontal / Vertical: one line or two points; the others two), so `picks` can make several. `entities` instead are selected sketch entity ids (drawing_get `sketch`): the constraint key with them selected applies at once. One edit per constraint, named as the tool (\"Perpendicular\").",
        fields: vec![
            doc_field(),
            drawing_field(),
            sheet_field(),
            req("kind", json!({ "type": "string", "enum": KINDS.map(|k| k.0) }), "The constraint."),
            opt("picks", json!({ "type": "array", "items": { "type": "array", "items": { "type": "number" }, "minItems": 2, "maxItems": 2 }, "minItems": 1 }), "Clicks [x, y] (sheet mm) with the tool on."),
            opt("entities", json!({ "type": "array", "items": { "type": "integer" }, "minItems": 1, "maxItems": 2 }), "Selected sheet sketch entity ids (instead of `picks`)."),
        ],
        mutates: true,
        destructive: false,
        run: constrain,
    }]
}

const KINDS: [(&str, SkCon); 6] = [
    ("horizontal", SkCon::Horizontal),
    ("vertical", SkCon::Vertical),
    ("coincident", SkCon::Coincident),
    ("perpendicular", SkCon::Perpendicular),
    ("parallel", SkCon::Parallel),
    ("tangent", SkCon::Tangent),
];

fn constrain(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let kind = super::drawing_tools::enum_arg(a, "kind", &KINDS, SkCon::Horizontal)?;
    let od = ctx.doc(a.opt_str("doc")?)?;
    let mut t = target(od, a)?;
    let sheet = t.sheet;
    let mut made = Vec::new();
    let mut add = |t: &mut super::drawing_tools::Target, picks: &[u32], param: &str| -> Result<(), OpError> {
        let ck = sketch_constraint(kind, picks).ok_or_else(|| OpError::param(param, &format!("{} needs two entities", kind.name())))?;
        let sk = &mut t.d.sheets[sheet].sketch.sketch;
        let id = sk.add_constraint(ck).map_err(|e| OpError::param(param, &format!("{}: {e}", kind.name())))?;
        let _ = sk.solve();
        made.push(id.0);
        Ok(())
    };
    match (a.get("picks"), a.opt_array("entities")?) {
        (Some(_), Some(_)) => return Err(OpError::param("entities", "give `picks` or `entities`, not both")),
        (None, None) => return Err(OpError::param("picks", "required: clicks on the geometry to constrain (or `entities`)")),
        (None, Some(ids)) => {
            let sk = &t.d.sheets[sheet].sketch.sketch;
            let mut picks = Vec::new();
            for (i, v) in ids.iter().enumerate() {
                let id = v.as_u64().and_then(|x| u32::try_from(x).ok()).filter(|x| sk.entity(noshape_sketch::EntityId(*x)).is_some_and(|e| !e.reference_only));
                picks.push(id.ok_or_else(|| OpError::param(&format!("entities[{i}]"), "a sheet sketch entity id (drawing_get `sketch`)"))?);
            }
            if !constraint_ready(sk, kind, &picks) {
                return Err(OpError::param("entities", &format!("{} takes {}", kind.name(), if kind.needs() == 1 { "one line or two points" } else { "two entities" })));
            }
            add(&mut t, &picks, "entities")?;
            commit(od, t.tab, sheet, t.d, kind.name())?;
        }
        (Some(_), None) => {
            let mut picks: Vec<u32> = Vec::new();
            for (i, v) in a.array("picks")?.iter().enumerate() {
                let field = format!("picks[{i}]");
                let c = point_value(v, &field)?;
                let pick = {
                    let mut pk = t.m.pick(&Tool::Select, &[], c);
                    pk.drawing = &t.d;
                    pk.constrain_pick(c)
                };
                let id = match pick.ok_or_else(|| OpError::param(&field, "nothing to constrain there: pick a sketch point or curve, or a view's straight edge or circle"))? {
                    Ok(id) => id.0,
                    Err(curve) => {
                        // View geometry: a fixed external copy the constraint can use (its own edit, as in the editor).
                        let r = t.d.sheets[sheet].sketch.sketch.use_external(&curve).map_err(|e| OpError::param(&field, &format!("{e}")))?;
                        let id = r.created.last().copied().ok_or_else(|| OpError::param(&field, "that view geometry can't be used"))?;
                        commit(od, t.tab, sheet, t.d.clone(), "Use view geometry")?;
                        id.0
                    }
                };
                if !picks.contains(&id) {
                    picks.push(id);
                }
                if constraint_ready(&t.d.sheets[sheet].sketch.sketch, kind, &picks) {
                    add(&mut t, &picks, &field)?;
                    commit(od, t.tab, sheet, t.d.clone(), kind.name())?;
                    picks.clear();
                }
            }
            if !picks.is_empty() {
                return Err(OpError::param("picks", &format!("the last picks don't complete {}: it takes {}", kind.name(), if kind.needs() == 1 { "one line or two points" } else { "two entities" })));
            }
        }
    }
    let mut out = reply(od, t.tab, sheet, false)?;
    out["constraints"] = json!(made);
    Ok(out.into())
}
