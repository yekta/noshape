//! The drawing annotation menus: jogs, leaders and note leader nodes,
//! foreshortening, datum target dimensions, groups, Detach from geometry,
//! Align centermark and stacked callouts. Shared code in
//! `noshape_scene::drawing_tools::annotation_edit`, with the editor's edit
//! texts.

use super::doc_field;
use super::drawing::{commit, drawing_field, point, reply, sheet_field};
use super::drawing_tools::{Target, edge_at, pt_arg, target};
use crate::Ctx;
use crate::error::OpError;
use crate::registry::{Args, Op, OpOutput, opt, req};
use noshape_model::drawing::{Annotation, AnnotationKind, LeaderStart, TableKind};
use noshape_scene::drawing_tools::Tool;
use noshape_scene::drawing_tools::annotation_edit as ae;
use serde_json::{Value, json};

const ACTIONS: [&str; 27] = [
    "add_jog",
    "remove_jog",
    "add_leader",
    "remove_leaders",
    "add_node",
    "remove_node",
    "delete_leader",
    "move_leader",
    "foreshorten",
    "remove_foreshorten",
    "hide_dimension",
    "show_dimensions",
    "flip",
    "group",
    "remove_from_group",
    "group_annotation",
    "remove_annotation_from_group",
    "detach",
    "align_centermark",
    "centerline_end",
    "move_centerline",
    "stacked_callouts",
    "break",
    "reset_ordinate",
    "orientation",
    "flip_arrows",
    "arc_condition",
];

pub fn ops() -> Vec<Op> {
    let int = || json!({ "type": "integer", "minimum": 0 });
    vec![Op {
        group: "drawing",
        verb: "annotation_edit",
        summary: "Edit an annotation from its menus: jogs, leaders, nodes, foreshorten, groups, detach, align centermark, stacked callouts",
        description: "The annotation context menus, one edit each with the menu's edit text:\n- Linear dimensions: `add_jog` / `remove_jog` on extension line `line` (0 / 1; a 45° jog, 11.2 mm; a short line first moves the dimension line out); linear and ordinate dimensions: `break` (`on`: Break / Unbreak dimension).\n- `add_leader` to `point` (sheet mm; a vertex or edge point of a view, else the sheet) on GD&T frames, callouts, surface finish and weld symbols and notes (a note's starts at its landing's bend; a note's or callout's at `from_node` [leader, node]); `remove_leaders` (a callout's added ones).\n- Note and callout leaders (a callout's leader 0 is the one to its anchor, then the added ones; drawing_get `leaders`): `add_node` on leader `leader` at `point` (on the leader line), `remove_node` (`leader`, `node`; its branches then start at the bend), `delete_leader` (`leader`; a callout's leader 0 leaves it in space), `move_leader` (Move by grip: leader `leader`'s end to `point`; onto a vertex, circle center or edge it attaches there, elsewhere it stays on the sheet; a callout's leader 0 only onto a model point).\n- Radial dimensions: `foreshorten`; `remove_foreshorten`.\n- Surface finish symbols on an edge: `flip` (to the edge's other side).\n- Datum targets: a new one draws no position dimensions (as live); `show_dimensions` (`all`: All targets, else This target; basic, boxed dimensions from its origin), `hide_dimension`.\n- Notes and callouts: `group` with view `view` (they move with it; `with`: more annotation ids), `remove_from_group`; `group_annotation` with `with`, `remove_annotation_from_group`.\n- `detach` (Detach from geometry: stays put, no longer follows the model).\n- Centermarks: `align_centermark` to the straight edge at `point`.\n- Centerlines (drag their grips): `centerline_end` slides end `end` along the line to `point` (shorter or longer than the references; not an end on a model point); `move_centerline` puts an edge to edge one's middle at `point`.\n- Callouts (with a BOM table in the drawing): `stacked_callouts` — the `parts` (names) stacked under it (Add stacked callouts…; [] clears).\n- Ordinate dimensions: `reset_ordinate` (Reset ordinate; as in the editor nothing moves: the values always sit on the group's text line, so no edit).\n- Linear dimensions: `orientation` (`value` horizontal / vertical / aligned: \"Dimension orientation\"); dimensions: `flip_arrows` (the flip node: \"Flip arrows\"); maximum / minimum dimensions: `arc_condition` (`end` 0 / 1, `value` center / minimum / maximum: \"Arc condition\").",
        fields: vec![
            doc_field(),
            drawing_field(),
            sheet_field(),
            req("annotation", int(), "Annotation id (drawing_get)."),
            req("action", json!({ "type": "string", "enum": ACTIONS }), "The menu action."),
            opt("line", json!({ "type": "integer", "enum": [0, 1] }), "add_jog / remove_jog: the extension line."),
            opt("point", point(), "add_leader: where it points; add_node: on the leader line; move_leader: where the leader's end goes; align_centermark: on a straight edge; centerline_end: where the end goes (projected onto the line); move_centerline: the new middle (sheet mm)."),
            opt("leader", int(), "add_node / remove_node / delete_leader / move_leader: the leader (a note's 0 = first; a callout's 0 = the one to its anchor)."),
            opt("node", int(), "remove_node: the node (0 = nearest the note)."),
            opt("from_node", json!({ "type": "array", "items": int(), "minItems": 2, "maxItems": 2 }), "add_leader on a note or callout: branch from [leader, node]."),
            opt("all", json!({ "type": "boolean", "default": false }), "show_dimensions: All targets."),
            opt("view", int(), "group: the view to group with."),
            opt("with", json!({ "type": "array", "items": int() }), "group / group_annotation: the other annotations."),
            opt("parts", json!({ "type": "array", "items": { "type": "string" } }), "stacked_callouts: part names."),
            opt("on", json!({ "type": "boolean", "default": true }), "break: Break (true) or Unbreak (false) dimension."),
            opt("value", json!({ "type": "string", "enum": ["horizontal", "vertical", "aligned", "center", "minimum", "maximum"] }), "orientation: horizontal / vertical / aligned; arc_condition: center / minimum / maximum."),
            opt("end", json!({ "type": "integer", "enum": [0, 1] }), "arc_condition: the end (0 = end 1); centerline_end: the end (0 = first)."),
        ],
        mutates: true,
        destructive: false,
        run: annotation_edit,
    }]
}

fn uint(a: &Args, name: &str) -> Result<usize, OpError> {
    Ok(a.opt_u64(name)?.ok_or_else(|| OpError::param(name, "required for this action"))? as usize)
}

/// The annotation ids in `with`, each on the sheet, plus `id`.
fn with_ids(t: &Target, a: &Args, id: u64) -> Result<Vec<u64>, OpError> {
    let mut ids = vec![id];
    for (i, v) in a.opt_array("with")?.cloned().unwrap_or_default().iter().enumerate() {
        let x = v.as_u64().filter(|x| t.d.sheets[t.sheet].annotations.iter().any(|y| y.id == *x)).ok_or_else(|| OpError::param(&format!("with[{i}]"), "an annotation id on the sheet"))?;
        if !ids.contains(&x) {
            ids.push(x);
        }
    }
    Ok(ids)
}

fn annotation_edit(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let mut t = target(od, a)?;
    let id = a.opt_u64("annotation")?.ok_or_else(|| OpError::param("annotation", "an annotation id (drawing_get)"))?;
    let ann: Annotation = t.d.sheets[t.sheet].annotations.iter().find(|x| x.id == id).cloned().ok_or_else(|| OpError::param("annotation", "no such annotation on the sheet"))?;
    let action = a.str("action")?;
    let refuse = |why: &str| Err(OpError::param("action", &format!("{action}: {why}")));
    let note = matches!(ann.kind, AnnotationKind::Note { .. });
    let note_or_callout = note || matches!(ann.kind, AnnotationKind::Balloon { .. });
    let s = t.sheet;
    if action == "reset_ordinate" {
        if !matches!(ann.kind, AnnotationKind::OrdinateGroup { .. }) {
            return refuse("ordinate dimensions only");
        }
        let mut out = reply(od, t.tab, s, false)?;
        out["message"] = json!("Nothing to reset: the ordinate values sit on the group's text line");
        return Ok(out.into());
    }
    let desc: &str = match action {
        "add_jog" | "remove_jog" if !matches!(ann.kind, AnnotationKind::LinearDimension { .. }) || ann.inspection.is_some() => return refuse("linear dimensions only"),
        "break" if !matches!(ann.kind, AnnotationKind::LinearDimension { .. } | AnnotationKind::OrdinateGroup { .. }) || ann.inspection.is_some() => return refuse("linear and ordinate dimensions only"),
        "add_jog" => {
            let k = uint(a, "line")?;
            if k > 1 {
                return Err(OpError::param("line", "0 or 1"));
            }
            if ann.jogs[k].is_some() {
                return refuse("that extension line has a jog (remove_jog)");
            }
            let pk = t.m.pick(&Tool::Select, &[], [0.0; 2]);
            let f = pk.dimension_frame(&pk.resolved_annotation(id).unwrap_or(ann.clone())).ok_or_else(|| OpError::param("annotation", "the dimension's view is gone"))?;
            let (jog, grow) = ae::jog_for(&f, k);
            ae::add_jog(&mut t.d.sheets[s], id, k, jog, grow);
            "Add jog"
        }
        "remove_jog" => {
            let k = uint(a, "line")?;
            if ann.jogs.get(k).is_none_or(|j| j.is_none()) {
                return refuse("no jog on that extension line");
            }
            ae::remove_jog(&mut t.d.sheets[s], id, k);
            "Remove jog"
        }
        "orientation" => {
            use noshape_model::drawing::DimOrientation as O;
            let o = match a.opt_str("value")? {
                Some("horizontal") => O::Horizontal,
                Some("vertical") => O::Vertical,
                Some("aligned") => O::Aligned,
                _ => return Err(OpError::param("value", "horizontal, vertical or aligned")),
            };
            if !ae::set_dimension_orientation(&mut t.d.sheets[s], id, o) {
                return refuse("linear dimensions only");
            }
            "Dimension orientation"
        }
        "flip_arrows" => {
            if !ann.kind.is_dimension() {
                return refuse("dimensions only");
            }
            ae::flip_arrows(&mut t.d.sheets[s], id);
            "Flip arrows"
        }
        "arc_condition" => {
            use noshape_model::drawing::ArcCondition as C;
            let c = match a.opt_str("value")? {
                Some("center") => C::Center,
                Some("minimum") => C::Min,
                Some("maximum") => C::Max,
                _ => return Err(OpError::param("value", "center, minimum or maximum")),
            };
            let end = uint(a, "end")?;
            let has_arc = match &ann.kind {
                AnnotationKind::MaxMinDimension { ra, rb, .. } => [*ra, *rb].get(end).is_some_and(|r| *r > 0.0),
                _ => return refuse("maximum / minimum dimensions only"),
            };
            if !has_arc {
                return Err(OpError::param("end", "that end isn't on an arc"));
            }
            ae::set_arc_condition(&mut t.d.sheets[s], id, end, c);
            "Arc condition"
        }
        "break" => {
            let on = a.bool_or("on", true)?;
            if ann.unbroken != on {
                return refuse(if on { "the dimension is already broken" } else { "the dimension is not broken" });
            }
            if let Some(x) = t.d.sheets[s].annotations.iter_mut().find(|x| x.id == id) {
                x.unbroken = !on;
            }
            if on { "Break dimension" } else { "Unbreak dimension" }
        }
        "add_leader" | "remove_leaders" if ae::leaders(&ann).is_none() => return refuse("GD&T frames, callouts, surface finish and weld symbols and notes only"),
        "add_leader" => {
            let p = pt_arg(a, "point")?;
            let start = match a.opt_array("from_node")? {
                // Notes and callouts (a callout's leader 0 is the one to its anchor).
                Some(v) if ae::all_leaders(&ann).is_some() => {
                    let (l, n) = (v.first().and_then(Value::as_u64).unwrap_or(u64::MAX) as usize, v.get(1).and_then(Value::as_u64).unwrap_or(u64::MAX) as usize);
                    if ae::all_leaders(&ann).and_then(|ls| ls.get(l).cloned()).is_none_or(|ld| n >= ld.nodes.len()) {
                        return Err(OpError::param("from_node", "[leader, node] of a node of this note or callout"));
                    }
                    LeaderStart::Node(l, n)
                }
                Some(_) => return Err(OpError::param("from_node", "notes and callouts only")),
                None => ae::note_leader_start(&ann),
            };
            let leader = ae::leader_to(&t.m.pick(&Tool::GdtLeader { id }, &[], p), p);
            ae::add_leader(&mut t.d.sheets[s], id, leader, start);
            "Add leader"
        }
        "remove_leaders" => {
            if ae::leaders(&ann).is_some_and(|l| l.is_empty()) {
                return refuse("it has no leaders");
            }
            ae::remove_leaders(&mut t.d.sheets[s], id);
            "Remove leaders"
        }
        "add_node" | "remove_node" | "delete_leader" | "move_leader" if ae::all_leaders(&ann).is_none() => return refuse("note and callout leaders only"),
        "add_node" | "remove_node" | "delete_leader" | "move_leader" => {
            // A note's leaders in order; a callout's: 0 = the leader to its
            // anchor, then the added ones (drawing_get `leaders`).
            let k = uint(a, "leader")?;
            let all = ae::all_leaders(&ann).unwrap_or_default();
            if k >= all.len() {
                return Err(OpError::param("leader", &format!("the annotation has {} leader(s)", all.len())));
            }
            let built = t.m.built.clone();
            let pick_at = |p: [f64; 2]| t.m.pick(&Tool::Note, &[], p).snap_at(p);
            let snapped = match action {
                "move_leader" => {
                    let p = pt_arg(a, "point")?;
                    Some(match pick_at(p) {
                        Some(q) => (Some(q.view), q.p, q.r),
                        None => (None, p, None),
                    })
                }
                _ => None,
            };
            let x = t.d.sheets[s].annotations.iter_mut().find(|x| x.id == id).expect("found");
            match action {
                "add_node" => {
                    let p = pt_arg(a, "point")?;
                    let path = ae::leader_paths(built.as_deref(), &ann).into_iter().find(|(i, _)| *i == k).map(|(_, p)| p).ok_or_else(|| OpError::param("leader", "the leader has no path (a callout in space)"))?;
                    let (seg, d) = ae::nearest_segment(&path, p).ok_or_else(|| OpError::param("leader", "the leader has no path"))?;
                    // Exactly on the leader line, as the right-click is.
                    if d > 0.5 {
                        return Err(OpError::param("point", &format!("not on the leader line ({d:.2} mm away)")));
                    }
                    ae::with_all_leaders(x, |l| ae::add_node(l, k, seg, path.head, p));
                    "Add node"
                }
                "remove_node" => {
                    let n = uint(a, "node")?;
                    if n >= all[k].nodes.len() {
                        return Err(OpError::param("node", &format!("the leader has {} node(s)", all[k].nodes.len())));
                    }
                    ae::remove_node(x, k, n);
                    "Remove node"
                }
                "move_leader" => {
                    // Move by grip: onto a vertex, circle center or edge it
                    // attaches there, elsewhere it stays on the sheet.
                    let (view, point, r) = snapped.expect("move_leader");
                    if !ae::move_leader_end(x, k, view, point, r) {
                        return Err(OpError::param("point", "a callout's leader to its anchor only attaches to a model point"));
                    }
                    "Move by grip"
                }
                _ => {
                    ae::delete_leader(x, k);
                    "Delete leader"
                }
            }
        }
        "foreshorten" => {
            let k = ae::foreshortened(&ann.kind).ok_or_else(|| OpError::param("action", "foreshorten: radius dimensions only"))?;
            t.d.sheets[s].annotations.iter_mut().filter(|x| x.id == id).for_each(|x| x.kind = k.clone());
            "Foreshorten"
        }
        "remove_foreshorten" => {
            let k = ae::unforeshortened(&ann.kind).ok_or_else(|| OpError::param("action", "remove_foreshorten: foreshortened radius dimensions only"))?;
            t.d.sheets[s].annotations.iter_mut().filter(|x| x.id == id).for_each(|x| x.kind = k.clone());
            "Remove foreshorten"
        }
        "flip" => {
            if !matches!(ann.kind, AnnotationKind::SurfaceFinish { on: Some(_), .. }) {
                return refuse("surface finish symbols on an edge only");
            }
            if let Some(AnnotationKind::SurfaceFinish { on: Some(o), .. }) = t.d.sheets[s].annotations.iter_mut().find(|x| x.id == id).map(|x| &mut x.kind) {
                o.3 = !o.3;
            }
            "Flip"
        }
        "hide_dimension" | "show_dimensions" if !matches!(ann.kind, AnnotationKind::DatumTarget { .. }) => return refuse("datum targets only"),
        "hide_dimension" => {
            let o = ann.target.as_deref().cloned().unwrap_or_default();
            if o.origin.is_none() || o.hide_dims {
                return refuse("the target has no position dimensions shown");
            }
            ae::set_target_dims_hidden(&mut t.d.sheets[s], id, true, false);
            "Hide dimension"
        }
        "show_dimensions" => {
            let all = a.bool_or("all", false)?;
            if !all && !ann.target.as_ref().is_some_and(|o| o.hide_dims) {
                return refuse("this target's dimensions are shown (all: true for every target)");
            }
            ae::set_target_dims_hidden(&mut t.d.sheets[s], id, false, all);
            "Show dimensions"
        }
        "group" | "remove_from_group" | "group_annotation" | "remove_annotation_from_group" if !note_or_callout => return refuse("notes and callouts only"),
        "group" => {
            if ann.view_group.is_some() {
                return refuse("already grouped with a view (remove_from_group)");
            }
            let v = super::drawing_tools::view_arg(&t, a, "view")?;
            let ids = with_ids(&t, a, id)?;
            ae::group_with_view(&mut t.d.sheets[s], &ids, Some(v));
            "Group"
        }
        "remove_from_group" => {
            if ann.view_group.is_none() {
                return refuse("not grouped with a view");
            }
            ae::group_with_view(&mut t.d.sheets[s], &[id], None);
            "Remove from group"
        }
        "group_annotation" => {
            if ann.ann_group.is_some() {
                return refuse("already in an annotation group (remove_annotation_from_group)");
            }
            let ids = with_ids(&t, a, id)?;
            if ids.len() < 2 {
                return Err(OpError::param("with", "the other annotations to group with (two or more in all)"));
            }
            ae::group_annotations(&mut t.d.sheets[s], &ids, true);
            "Group annotation"
        }
        "remove_annotation_from_group" => {
            if ann.ann_group.is_none() {
                return refuse("not in an annotation group");
            }
            let ids = with_ids(&t, a, id)?;
            ae::group_annotations(&mut t.d.sheets[s], &ids, false);
            "Remove annotation from group"
        }
        "detach" => {
            if ann.refs.is_empty() {
                return refuse("not attached to geometry");
            }
            ae::detach(&mut t.d.sheets[s], id);
            "Detach"
        }
        "align_centermark" => {
            if !matches!(ann.kind, AnnotationKind::Centermark { .. }) {
                return refuse("centermarks only");
            }
            let p = pt_arg(a, "point")?;
            let (pa, pb, _) = edge_at(&t, &Tool::Select, p, 0).map_err(|_| OpError::param("point", "Select a straight edge"))?;
            ae::align_centermark(&mut t.d.sheets[s], id, pa.p, pb.p);
            "Align centermark"
        }
        "centerline_end" | "move_centerline" => {
            let AnnotationKind::Centerline { view, a: pa, b: pb } = ann.kind.clone() else { return refuse("centerlines only") };
            let p = pt_arg(a, "point")?;
            let pk = t.m.pick(&Tool::Select, &[], p);
            let orig = pk.resolved_annotation(id).unwrap_or(ann.clone());
            let new = if action == "centerline_end" {
                let end = uint(a, "end")?;
                if end > 1 {
                    return Err(OpError::param("end", "0 or 1"));
                }
                if ann.refs.iter().any(|r| r.slot as usize == end) {
                    return Err(OpError::param("end", "that end is on a model point"));
                }
                pk.dragged_centerline(&orig, end)
            } else {
                if !ann.refs.is_empty() {
                    return refuse("a 2 point centerline on model points doesn't move");
                }
                let pl = pk.placement(view).ok_or_else(|| OpError::param("annotation", "the centerline's view is gone"))?;
                let (sa, sb) = (pl.to_sheet(pa), pl.to_sheet(pb));
                pk.dragged_annotation(&orig, [(sa[0] + sb[0]) / 2.0, (sa[1] + sb[1]) / 2.0])
            };
            if let Some(x) = t.d.sheets[s].annotations.iter_mut().find(|x| x.id == id) {
                x.kind = new.kind;
            }
            if action == "centerline_end" { "Resize centerline" } else { "Move annotation" }
        }
        "stacked_callouts" => {
            if !matches!(ann.kind, AnnotationKind::Balloon { .. }) {
                return refuse("callouts only");
            }
            if !t.d.sheets.iter().flat_map(|x| &x.tables).any(|x| matches!(x.kind, TableKind::Bom { .. })) {
                return refuse("the drawing has no BOM table");
            }
            let sheet = &t.d.sheets[s];
            let src = ann.kind.view().and_then(|v| noshape_model::drawing::views::view_source(sheet, v)).and_then(|(k, _)| t.m.sources.get(&k)).ok_or_else(|| OpError::param("annotation", "the callout's view has no model"))?;
            let mut stacked = Vec::new();
            for (i, n) in a.opt_array("parts")?.cloned().unwrap_or_default().iter().enumerate() {
                let n = n.as_str().ok_or_else(|| OpError::param(&format!("parts[{i}]"), "a part name"))?;
                let body = src.parts.iter().find(|p| p.name == n).and_then(|p| p.body.clone()).ok_or_else(|| OpError::param(&format!("parts[{i}]"), &format!("the view's parts: {}", src.parts.iter().map(|p| p.name.as_str()).collect::<Vec<_>>().join(", "))))?;
                stacked.push(noshape_model::drawing::StackedCallout { text: String::new(), body: Some(body) });
            }
            if let Some(AnnotationKind::Balloon { callout, .. }) = t.d.sheets[s].annotations.iter_mut().find(|x| x.id == id).map(|x| &mut x.kind) {
                callout.stacked = stacked;
            }
            "Add stacked callouts"
        }
        other => return Err(OpError::param("action", &format!("`{other}`: one of {}", ACTIONS.join(", ")))),
    };
    commit(od, t.tab, t.sheet, t.d, desc)?;
    let mut out = reply(od, t.tab, t.sheet, false)?;
    out["annotation"] = json!(id);
    Ok(out.into())
}
