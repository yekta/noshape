//! drawing_move: what the editor's drags move on a sheet.

use super::doc_field;
use super::drawing::{commit, drawing_field, point, point_value, reply, sheet_field};
use super::drawing_tools::{target, view_arg};
use crate::error::OpError;
use crate::registry::{Args, Op, OpOutput, opt};
use crate::Ctx;
use noshape_model::drawing::{SheetLayer, ViewKind};
use noshape_scene::drawing_tools::Tool;
use serde_json::json;

pub(super) fn ops() -> Vec<Op> {
    vec![arrange_op(), Op {
        group: "drawing",
        verb: "move",
        summary: "Drag an annotation, table, image, view label, break line or cutting line",
        description: "Select and drag on the sheet, one edit with the drag's name. Exactly one of: `annotation` (a dimension's line follows the pointer as in the editor, a note or symbol moves; the other members of its annotation group follow unless `alone`, Shift: \"Move annotation\"), `table` (\"Move table\"), `image` (\"Move image\"), `view_label` (a view id: its label, \"Move view label\"), `break_line` (a broken view's id, with `index`: the break slides along the part keeping its width, \"Move break\"), `cutting_line` (a section view's id: its line moves sideways on the parent, \"Move cutting line\"). `by` is the drag (sheet mm); for annotations, tables and images `to` (sheet mm) is where their position goes instead.",
        fields: vec![
            doc_field(),
            drawing_field(),
            sheet_field(),
            opt("annotation", json!({ "type": "integer" }), "Annotation id (drawing_get)."),
            opt("table", json!({ "type": "integer" }), "Table id."),
            opt("image", json!({ "type": "integer" }), "Image id."),
            opt("view_label", json!({ "type": "integer" }), "View id: drag its label."),
            opt("break_line", json!({ "type": "integer" }), "Broken view id: drag break `index`."),
            opt("index", json!({ "type": "integer", "minimum": 0, "default": 0 }), "break_line: the break (0 = first)."),
            opt("cutting_line", json!({ "type": "integer" }), "Section view id: drag its cutting line."),
            opt("by", point(), "The drag [dx, dy] (sheet mm)."),
            opt("to", point(), "annotation / table / image: where its position goes (sheet mm)."),
            opt("alone", json!({ "type": "boolean", "default": false }), "annotation: Shift (its group doesn't follow)."),
        ],
        mutates: true,
        destructive: false,
        run: drag,
    }]
}

const TARGETS: [&str; 6] = ["annotation", "table", "image", "view_label", "break_line", "cutting_line"];

fn add(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] + b[0], a[1] + b[1]]
}

fn sub(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] - b[0], a[1] - b[1]]
}

/// The drag from `from`: `by`, or `to` minus `from` where `to` is allowed.
fn delta(a: &Args, from: Option<[f64; 2]>) -> Result<[f64; 2], OpError> {
    match (a.get("by"), a.get("to"), from) {
        (Some(_), Some(_), _) => Err(OpError::param("to", "give `by` or `to`, not both")),
        (Some(v), None, _) => point_value(v, "by"),
        (None, Some(v), Some(f)) => Ok(sub(point_value(v, "to")?, f)),
        (None, Some(_), None) => Err(OpError::param("to", "only for annotations, tables and images: use `by`")),
        (None, None, _) => Err(OpError::param("by", "required: the drag (sheet mm)")),
    }
}

fn drag(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let given: Vec<&str> = TARGETS.iter().copied().filter(|k| a.get(k).is_some()).collect();
    let [what] = given[..] else {
        return Err(OpError::param(given.get(1).copied().unwrap_or("annotation"), "give exactly one of annotation, table, image, view_label, break_line, cutting_line"));
    };
    let od = ctx.doc(a.opt_str("doc")?)?;
    let mut t = target(od, a)?;
    let si = t.sheet;
    let id = a.opt_u64(what)?.unwrap_or(0);
    let name = match what {
        "annotation" => {
            let orig = t.d.sheets[si].annotations.iter().find(|x| x.id == id).cloned().ok_or_else(|| OpError::param("annotation", "not an annotation on the sheet"))?;
            let d = delta(a, Some(orig.position))?;
            let moved = t.m.pick(&Tool::Select, &[], add(orig.position, d)).dragged_annotation(&orig, orig.position);
            let s = &mut t.d.sheets[si];
            if let (Some(g), false) = (orig.ann_group, a.bool_or("alone", false)?) {
                let gd = sub(moved.position, orig.position);
                for x in s.annotations.iter_mut().filter(|x| x.ann_group == Some(g) && x.id != id) {
                    x.position = add(x.position, gd);
                }
            }
            if let Some(x) = s.annotations.iter_mut().find(|x| x.id == id) {
                *x = moved;
            }
            "Move annotation"
        }
        "table" => {
            let tb = t.d.sheets[si].table_mut(id).ok_or_else(|| OpError::param("table", "not a table on the sheet"))?;
            tb.position = add(tb.position, delta(a, Some(tb.position))?);
            "Move table"
        }
        "image" => {
            let im = t.d.sheets[si].images.iter_mut().find(|x| x.id == id).ok_or_else(|| OpError::param("image", "not an image on the sheet"))?;
            im.pos = add(im.pos, delta(a, Some(im.pos))?);
            "Move image"
        }
        "view_label" => {
            let view = view_arg(&t, a, "view_label")?;
            let d = delta(a, None)?;
            let v = t.d.sheets[si].view_mut(view).expect("view");
            v.label_at = Some(add(v.label_at.unwrap_or_default(), d));
            "Move view label"
        }
        "break_line" => {
            let view = view_arg(&t, a, "break_line")?;
            let d = delta(a, None)?;
            let pl = t.m.built.as_ref().and_then(|b| b.placement(view).cloned()).ok_or_else(|| OpError::param("break_line", "the view isn't drawn"))?;
            let index = a.opt_u64("index")?.unwrap_or(0) as usize;
            let v = t.d.sheets[si].view_mut(view).expect("view");
            let n = v.breaks.len();
            let b = v.breaks.get_mut(index).ok_or_else(|| OpError::param("index", &format!("the view has {n} breaks")))?;
            let nrm = pl.dir_to_sheet(if b.vertical { [1.0, 0.0] } else { [0.0, 1.0] });
            let dm = (d[0] * nrm[0] + d[1] * nrm[1]) / (pl.scale * 1000.0);
            (b.a, b.b) = (b.a + dm, b.b + dm);
            "Move break"
        }
        _ => {
            let view = view_arg(&t, a, "cutting_line")?;
            let d = delta(a, None)?;
            let parent = t.d.sheets[si].parent_of(view).filter(|_| matches!(t.d.sheets[si].view(view).map(|v| &v.kind), Some(ViewKind::Section { .. } | ViewKind::AlignedSection { .. }))).ok_or_else(|| OpError::param("cutting_line", "not a section view"))?;
            let ppl = t.m.built.as_ref().and_then(|b| b.placement(parent).cloned()).ok_or_else(|| OpError::param("cutting_line", "its parent view isn't drawn"))?;
            let dv = sub(ppl.to_view(d), ppl.to_view([0.0, 0.0]));
            let v = t.d.sheets[si].view_mut(view).expect("view");
            v.kind = match v.kind.clone() {
                ViewKind::Section { parent, line, label, cuts } if line.len() >= 2 => {
                    let e = sub(line[1], line[0]);
                    let l = (e[0] * e[0] + e[1] * e[1]).sqrt().max(1e-12);
                    let n = [-e[1] / l, e[0] / l];
                    let k = dv[0] * n[0] + dv[1] * n[1];
                    let shift = |p: &[f64; 2]| [p[0] + n[0] * k, p[1] + n[1] * k];
                    ViewKind::Section { parent, line: line.iter().map(shift).collect(), label, cuts: cuts.iter().map(shift).collect() }
                }
                ViewKind::AlignedSection { parent, center, a, b, label } => ViewKind::AlignedSection { parent, center: add(center, dv), a: add(a, dv), b: add(b, dv), label },
                k => k,
            };
            "Move cutting line"
        }
    };
    commit(od, t.tab, si, t.d, name)?;
    let mut out = reply(od, t.tab, si, false)?;
    out[what] = json!(id);
    Ok(out.into())
}

const LAYERS: [(&str, SheetLayer); 4] = [("drawing", SheetLayer::Drawing), ("border_frame", SheetLayer::BorderFrame), ("border_zones", SheetLayer::BorderZones), ("title_block", SheetLayer::TitleBlock)];

fn arrange_op() -> Op {
    Op {
        group: "drawing",
        verb: "arrange",
        summary: "Move to ▸ layer, Bring to front / Send to back, Move to sheet… on annotations, sketch entities, tables and images",
        description: "The context menus of annotations, tables and images, one edit each with the menu's name. `action`: `move_to` (Move to ▸ `layer`: drawing | border_frame | border_zones | title_block; annotations (several), sheet sketch entities (several), a table or an image; \"Move to\"), `bring_to_front` / `send_to_back` (a table or an image: the stacking order among the sheet's tables / images), `move_to_sheet` (a table: Move to sheet… `to_sheet`, number (1 = first) or name, omitted: a new sheet; it keeps its place and gets a new id there, the reply's `table`; \"Move to sheet\"). drawing_get lists a `layer` other than drawing.",
        fields: vec![
            doc_field(),
            drawing_field(),
            sheet_field(),
            crate::registry::req("action", json!({ "type": "string", "enum": ["move_to", "bring_to_front", "send_to_back", "move_to_sheet"] }), "The menu action."),
            opt("annotations", json!({ "type": "array", "items": { "type": "integer" }, "minItems": 1 }), "Annotation ids (move_to)."),
            opt("table", json!({ "type": "integer" }), "Table id."),
            opt("image", json!({ "type": "integer" }), "Image id."),
            opt("sketch_entities", json!({ "type": "array", "items": { "type": "integer" }, "minItems": 1 }), "Sheet sketch entity ids (move_to; drawing_get `sketch`)."),
            opt("layer", json!({ "type": "string", "enum": LAYERS.map(|l| l.0) }), "move_to: the layer."),
            opt("to_sheet", json!({ "type": ["integer", "string"] }), "move_to_sheet: the sheet, number (1 = first) or name; omitted: a new sheet."),
        ],
        mutates: true,
        destructive: false,
        run: arrange,
    }
}

fn arrange(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let given: Vec<&str> = ["annotations", "table", "image", "sketch_entities"].into_iter().filter(|k| a.get(k).is_some()).collect();
    let [what] = given[..] else {
        return Err(OpError::param(given.get(1).copied().unwrap_or("annotations"), "give exactly one of annotations, table, image, sketch_entities"));
    };
    let action = a.str("action")?;
    let od = ctx.doc(a.opt_str("doc")?)?;
    let mut t = target(od, a)?;
    let si = t.sheet;
    let s = &mut t.d.sheets[si];
    let id = if matches!(what, "annotations" | "sketch_entities") { 0 } else { a.opt_u64(what)?.unwrap_or(0) };
    if what == "table" && !s.tables.iter().any(|x| x.id == id) {
        return Err(OpError::param("table", "not a table on the sheet"));
    }
    if what == "image" && !s.images.iter().any(|x| x.id == id) {
        return Err(OpError::param("image", "not an image on the sheet"));
    }
    let name = match action {
        "move_to" => {
            let layer = super::drawing_tools::enum_arg(a, "layer", &LAYERS, SheetLayer::Drawing)?;
            if a.get("layer").is_none() {
                return Err(OpError::param("layer", "required: the layer to move to"));
            }
            match what {
                "annotations" => {
                    let mut ids = Vec::new();
                    for (i, v) in a.array("annotations")?.iter().enumerate() {
                        ids.push(v.as_u64().filter(|x| s.annotations.iter().any(|y| y.id == *x)).ok_or_else(|| OpError::param(&format!("annotations[{i}]"), "an annotation id on the sheet"))?);
                    }
                    s.annotations.iter_mut().filter(|x| ids.contains(&x.id)).for_each(|x| x.layer = layer);
                }
                "sketch_entities" => {
                    let sk = &mut s.sketch;
                    let mut ids = Vec::new();
                    for (i, v) in a.array("sketch_entities")?.iter().enumerate() {
                        let id = v.as_u64().and_then(|x| u32::try_from(x).ok()).filter(|x| sk.sketch.entity(noshape_sketch::EntityId(*x)).is_some_and(|e| !e.reference_only));
                        ids.push(id.ok_or_else(|| OpError::param(&format!("sketch_entities[{i}]"), "a sheet sketch entity id (drawing_get `sketch`)"))?);
                    }
                    for i in ids {
                        let mut st = sk.style(noshape_sketch::EntityId(i));
                        st.layer = layer;
                        sk.styles.insert(i, st);
                    }
                }
                "table" => s.tables.iter_mut().filter(|x| x.id == id).for_each(|x| x.layer = layer),
                _ => s.images.iter_mut().filter(|x| x.id == id).for_each(|x| x.layer = layer),
            }
            "Move to"
        }
        "bring_to_front" | "send_to_back" => {
            let front = action == "bring_to_front";
            match what {
                "table" => {
                    let i = s.tables.iter().position(|x| x.id == id).expect("checked");
                    let x = s.tables.remove(i);
                    if front { s.tables.push(x) } else { s.tables.insert(0, x) }
                }
                "image" => {
                    let i = s.images.iter().position(|x| x.id == id).expect("checked");
                    let x = s.images.remove(i);
                    if front { s.images.push(x) } else { s.images.insert(0, x) }
                }
                _ => return Err(OpError::param(what, "only tables and images have Bring to front / Send to back")),
            }
            if front { "Bring to front" } else { "Send to back" }
        }
        "move_to_sheet" => {
            if what != "table" {
                return Err(OpError::param(what, "Move to sheet… is a table's (views: drawing_view_edit move_to_sheet)"));
            }
            let names = t.d.sheets.iter().map(|x| x.name.clone()).collect::<Vec<_>>().join(", ");
            let to = match a.get("to_sheet") {
                None => t.d.add_sheet(si),
                Some(v) => {
                    let n = t.d.sheets.len();
                    let to = match v {
                        serde_json::Value::Number(k) => k.as_u64().filter(|k| (1..=n as u64).contains(k)).map(|k| k as usize - 1).ok_or_else(|| OpError::param("to_sheet", &format!("sheet numbers are 1 to {n}")))?,
                        serde_json::Value::String(x) => t.d.sheets.iter().position(|y| y.name == *x).ok_or_else(|| OpError::param("to_sheet", &format!("no sheet `{x}`; sheets: {names}")))?,
                        _ => return Err(OpError::param("to_sheet", "a sheet number (1 = first) or name")),
                    };
                    if to == si {
                        return Err(OpError::param("to_sheet", "another sheet"));
                    }
                    to
                }
            };
            let i = t.d.sheets[si].tables.iter().position(|x| x.id == id).expect("checked");
            let mut x = t.d.sheets[si].tables.remove(i);
            let nid = t.d.sheets[to].next_table_id();
            x.id = nid;
            t.d.sheets[to].tables.push(x);
            commit(od, t.tab, to, t.d, "Move to sheet")?;
            let mut out = reply(od, t.tab, to, false)?;
            out["table"] = json!(nid);
            out["sheet"] = json!(to + 1);
            return Ok(out.into());
        }
        _ => return Err(OpError::param("action", "move_to, bring_to_front, send_to_back or move_to_sheet")),
    };
    commit(od, t.tab, si, t.d, name)?;
    let mut out = reply(od, t.tab, si, false)?;
    out["action"] = json!(action);
    Ok(out.into())
}
