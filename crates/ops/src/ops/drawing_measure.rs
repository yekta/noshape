//! drawing_measure: the drawing's Measure tool (`[`).

use super::doc_field;
use super::drawing::{drawing_field, point_value, sheet_field};
use super::drawing_tools::target;
use crate::error::OpError;
use crate::registry::{Args, Op, OpOutput, opt, req};
use crate::Ctx;
use noshape_scene::drawing_tools::{MEASURE_UNITS, MeasurePick, Measured, Tool, measure_pick_names, measure_rows};
use serde_json::json;

pub(super) fn ops() -> Vec<Op> {
    vec![Op {
        group: "drawing",
        verb: "measure",
        summary: "Measure points, straight edges and circles of a view (the Measure panel)",
        description: "The drawing's Measure ([): each of `picks` is a click on the sheet (sheet mm) taking a snap point, else a straight edge, else a face-on circle, as the editor's. All picks must be in the first pick's view. Replies the picks as the panel names them (\"Line 1\"), its rows in model units (a point alone: none; a line: Length; a circle: Diameter, Length; two points: Min distance, dx, dy; a point and a line: Min distance, Length; a point or a line and a circle: Min / Max / Center distance, Length; two circles: Min / Center distance, Length; two lines: Parallel distance or an oblique pair's Angle, Min distance, Length; three picks or more: the curves' Length) and its relation (\"Point-Line\", \"Parallel lines\"…). Each row has `mm` (or `deg`) and `text` as the panel shows it in `unit` (default the drawing's), with `dual` the other unit system in brackets.",
        fields: vec![
            doc_field(),
            drawing_field(),
            sheet_field(),
            req("picks", json!({ "type": "array", "items": { "type": "array", "items": { "type": "number" }, "minItems": 2, "maxItems": 2 }, "minItems": 1 }), "Clicks [x, y] on the sheet (sheet mm), in order."),
            opt("unit", json!({ "type": "string", "enum": ["inches", "millimeters", "centimeters", "feet", "meters", "yards"] }), "The panel's unit (default the drawing's own)."),
            opt("dual", json!({ "type": "boolean", "default": false }), "Show dual unit."),
        ],
        mutates: false,
        destructive: false,
        run: measure,
    }]
}

fn measure(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let t = target(od, a)?;
    let mut picks: Vec<MeasurePick> = Vec::new();
    for (i, v) in a.array("picks")?.iter().enumerate() {
        let field = format!("picks[{i}]");
        let c = point_value(v, &field)?;
        let p = t.m.pick(&Tool::Select, &[], c).measure_pick(c).ok_or_else(|| OpError::param(&field, "nothing to measure there: pick a point, a straight edge or a circle of a view"))?;
        if picks.first().is_some_and(|f| f.view() != p.view()) {
            return Err(OpError::param(&field, "in another view than the first pick: Measure measures within one view"));
        }
        picks.push(p);
    }
    let own = if t.d.units_mm { 1 } else { 0 };
    let unit = match a.opt_str("unit")? {
        None => own,
        Some(u) => MEASURE_UNITS.iter().position(|m| m.0.eq_ignore_ascii_case(u)).ok_or_else(|| OpError::param("unit", "one of inches, millimeters, centimeters, feet, meters, yards"))?,
    };
    let dual = a.bool_or("dual", false)?;
    let (_, per, abbr) = MEASURE_UNITS[unit];
    let text = |m: f64| {
        let main = format!("{:.2} {abbr}", m / per);
        if !dual {
            return main;
        }
        if t.d.units_mm { format!("{main} [{:.3} in]", m / 0.0254) } else { format!("{main} [{:.3} mm]", m * 1000.0) }
    };
    let (rows, relation) = measure_rows(&picks);
    let rows: Vec<_> = rows.iter().map(|(name, m)| match *m {
        Measured::Length(m) => json!({ "name": name, "mm": m * 1000.0, "text": text(m) }),
        Measured::Angle(deg) => json!({ "name": name, "deg": deg, "text": format!("{deg:.1} deg") }),
    }).collect();
    let names: Vec<_> = measure_pick_names(&picks).into_iter().zip(&picks).map(|(n, m)| json!({ "name": n, "view": m.view().0 })).collect();
    Ok(json!({ "picks": names, "rows": rows, "relation": relation, "unit": MEASURE_UNITS[unit].0 }).into())
}
