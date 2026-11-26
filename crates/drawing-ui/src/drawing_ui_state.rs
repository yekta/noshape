//! The ui-parity state of a drawing tab and where its targets show (tests/ui-parity/README.md "### Drawings").

use super::*;
use crate::drawing_sketch::SkTool;
use noshape_scene::drawing_tools::{DimTool, Pick, PickedPoint, Sel, Tool};
use noshape_model::drawing::sheet::GripKind;
use serde_json::{Value, json};

impl DrawingEditor {
    /// Every slice painted again (its named controls record where they are).
    pub fn ui_redraw(&self, cx: &mut Context<Self>) {
        if let Some(s) = &self.slices {
            for slice in [&s.toolbar, &s.panel] {
                slice.update(cx, |_, cx| cx.notify());
            }
        }
        cx.notify();
    }

    /// A sheet point (mm) in window coordinates.
    fn sheet_to_window(&self, s: [f64; 2]) -> Point<Pixels> {
        let size = self.size_px();
        let local = [(s[0] - self.center[0]) * self.zoom + size[0] / 2.0, -(s[1] - self.center[1]) * self.zoom + size[1] / 2.0];
        self.bounds.origin + point(px(local[0] as f32), px(local[1] as f32))
    }

    /// The view named `name` on the current sheet.
    fn view_named(&self, name: &str) -> Result<ViewId, String> {
        let sheet = self.display().sheets.get(self.sheet).ok_or("no sheet")?;
        sheet.views.iter().find(|v| self.ui_view_name(v.id) == name).map(|v| v.id).ok_or_else(|| format!("no view {name}"))
    }

    /// Where target `t` shows now, in window coordinates: `sheet_point` [x, y] (sheet mm), `view` (its middle),
    /// `view_point` / `view_edge` {view, at: [x, y, z] mm}, `break_line` {view, index}, `hole_axis` ("X" / "Y"),
    /// `leader` {annotation, at} (a point along its first leader), `annotation` (by its text).
    pub fn ui_target_point(&self, t: &Value, _cx: &App) -> Result<Point<Pixels>, String> {
        let pair = |v: &Value| -> Option<[f64; 2]> { Some([v.get(0)?.as_f64()?, v.get(1)?.as_f64()?]) };
        let at = if let Some(p) = t.get("sheet_point") {
            pair(p).ok_or("a sheet point: [x, y] mm")?
        } else if t.get("empty").is_some() {
            // Live's empty spot ([1560, 960] px): the sheet's right margin, between the frame and the border, 19 mm up
            // (live drwm-2 s10, ANSI A at 0.23245 mm/px).
            let (w, _) = self.current_sheet().size.size_mm();
            [w - 4.6, 19.0]
        } else if let Some(name) = t.get("view").and_then(Value::as_str) {
            let pl = self.placement(self.view_named(name)?).ok_or("the view isn't built yet")?;
            [(pl.min[0] + pl.max[0]) / 2.0, (pl.min[1] + pl.max[1]) / 2.0]
        } else if let Some(v) = t.get("view_point").or_else(|| t.get("view_edge")) {
            let name = v.get("view").and_then(Value::as_str).ok_or("view_point: a view name")?;
            let p = v.get("at").and_then(Value::as_array).filter(|a| a.len() == 3).ok_or("view_point: at [x, y, z] mm")?;
            let p = DVec3::new(p[0].as_f64().unwrap_or(0.0), p[1].as_f64().unwrap_or(0.0), p[2].as_f64().unwrap_or(0.0)) * 1e-3;
            let id = self.view_named(name)?;
            let frame = self.geoms.get(&id).and_then(|g| g.frame).ok_or("the view isn't built yet")?;
            self.placement(id).ok_or("the view isn't built yet")?.to_sheet(frame.project(p))
        } else if let Some(b) = t.get("break_line") {
            // A break's first line (its middle), by the view's name and the break's index (live2 2188: a model point
            // there moves with the break).
            let name = b.get("view").and_then(Value::as_str).ok_or("break_line: a view name")?;
            let index = b.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
            let view = self.view_named(name)?;
            let built = self.built.as_ref().ok_or("the sheet isn't built yet")?;
            let pick = built.picks.iter().position(|p| *p == PickTarget::BreakLine { view, index }).ok_or_else(|| format!("no break {index} on {name}"))? as u32;
            let line = built.geometry.lines.iter().find(|l| l.pick == pick).ok_or("the break isn't drawn")?;
            *line.points.get(line.points.len() / 2).ok_or("an empty line")?
        } else if let Some(axis) = t.get("hole_axis").and_then(Value::as_str) {
            // The sheet's first hole table's origin indicator: the middle of its X or Y arrow (one pick, its menu has
            // the flips).
            let table = self.current_sheet().tables.iter().find(|t| matches!(t.kind, TableKind::Hole { .. })).ok_or("no hole table")?;
            let built = self.built.as_ref().ok_or("the sheet isn't built yet")?;
            let pick = built.picks.iter().position(|p| *p == PickTarget::HoleAxis(table.id)).ok_or("the hole table has no origin")? as u32;
            let k = match axis {
                "X" => 1,
                "Y" => 3,
                _ => return Err("hole_axis: \"X\" or \"Y\"".into()),
            };
            let line = built.geometry.lines.iter().filter(|l| l.pick == pick).nth(k).ok_or("the origin indicator isn't drawn")?;
            let (a, b) = (line.points[0], *line.points.last().ok_or("an empty line")?);
            [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0]
        } else if let Some(l) = t.get("leader") {
            // The point `at` (0..1, by length) along the first leader of the annotation shown as `annotation`.
            let text = l.get("annotation").and_then(Value::as_str).ok_or("leader: an annotation's text")?;
            let f = l.get("at").and_then(Value::as_f64).unwrap_or(0.5).clamp(0.0, 1.0);
            let id = self.ui_annotations().into_iter().find(|(_, _, s)| s == text).ok_or_else(|| format!("no annotation {text}"))?.0;
            let a = self.current_sheet().annotations.iter().find(|x| x.id == id).ok_or("no annotation")?;
            let path = self.leader_paths_of(a).into_iter().next().ok_or_else(|| format!("{text} has no leader"))?.1;
            let seg = |w: &[[f64; 2]]| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]);
            let total: f64 = path.points.windows(2).map(seg).sum();
            let mut left = total * f;
            let mut at = *path.points.last().ok_or("an empty leader")?;
            for w in path.points.windows(2) {
                let d = seg(w);
                if left <= d && d > 0.0 {
                    let k = left / d;
                    at = [w[0][0] + (w[1][0] - w[0][0]) * k, w[0][1] + (w[1][1] - w[0][1]) * k];
                    break;
                }
                left -= d;
            }
            at
        } else if let Some(text) = t.get("annotation").and_then(Value::as_str) {
            // `kind` (the record's): only annotations of that kind (live's zone labels "1" are notes, live2 2240).
            let kind = t.get("kind").and_then(Value::as_str);
            // An inspection bubble by its number (live2 2226).
            let bubble = self.built.as_ref().and_then(|b| b.geometry.texts.iter().find(|x| x.text == text && matches!(b.picks.get(x.pick as usize), Some(PickTarget::Inspection(_)))));
            if let Some(x) = bubble.filter(|_| kind.is_none_or(|k| k == "inspectionsymbol")) {
                return Ok(self.sheet_to_window(x.pos));
            }
            let a = self.ui_annotations().into_iter().find(|(_, k, s)| s == text && kind.is_none_or(|x| x == *k)).ok_or_else(|| format!("no annotation {text}"))?;
            // A note at its insertion point, where live's recorder presses one (live drw-triage-1126 (2)).
            let placed = || self.display().sheets.get(self.sheet).and_then(|s| s.annotations.iter().find(|x| x.id == a.0)).map(|x| x.position);
            if a.1 == "note"
                && let Some(at) = placed()
            {
                return self.window_point_on_sheet(at, t);
            }
            let picks = self.picks_of(Sel::Annotation(a.0));
            // An ordinate group at its first value's extension line, by the part (live drw-ordinate-add-to-group s6
            // pressed at Top (0, -9.5), the first value's point (0, -10): not on a value, no Reset ordinate).
            let first = self.display().sheets.get(self.sheet).and_then(|s| s.annotations.iter().find(|x| x.id == a.0)).and_then(|x| match &x.kind {
                AnnotationKind::OrdinateGroup { view, points, .. } => Some(self.placement(*view)?.to_sheet(*points.first()?)),
                _ => None,
            });
            let by_first = first.and_then(|f| {
                let ends = self.built.as_ref()?.geometry.lines.iter().filter(|l| picks.contains(&l.pick)).flat_map(|l| l.points.iter().copied());
                ends.min_by(|p, q| (p[0] - f[0]).hypot(p[1] - f[1]).total_cmp(&(q[0] - f[0]).hypot(q[1] - f[1])))
            });
            if let Some(at) = by_first {
                return self.window_point_on_sheet(at, t);
            }
            // Its text where it is drawn (a dimension's dragged text is not at its `position`: UIP-ANNOTATION-STALE-POS).
            let drawn = self.built.as_ref().and_then(|b| b.geometry.texts.iter().find(|x| picks.contains(&x.pick))).map(|x| {
                let q = noshape_model::drawing::font::text_box(x);
                [q.iter().map(|p| p[0]).sum::<f64>() / 4.0, q.iter().map(|p| p[1]).sum::<f64>() / 4.0]
            });
            match drawn {
                Some(at) => at,
                None => self.display().sheets.get(self.sheet).and_then(|s| s.annotations.iter().find(|x| x.id == a.0)).map(|x| x.position).ok_or("no annotation")?,
            }
        } else {
            return Err(format!("not a drawing target: {t}"));
        };
        self.window_point_on_sheet(at, t)
    }

    fn window_point_on_sheet(&self, at: [f64; 2], t: &Value) -> Result<Point<Pixels>, String> {
        let w = self.sheet_to_window(at);
        if !self.bounds.contains(&w) {
            return Err(format!("{t} is outside the sheet's area"));
        }
        Ok(w)
    }

    /// A picked edge's orange on screen in pixels (its length at the sheet's zoom, highlighted lines 2 px wide).
    fn pick_lit_px(&self, pick: &Pick) -> f64 {
        let view = match pick {
            Pick::Point(PickedPoint { view, .. }) | Pick::Line { a: PickedPoint { view, .. }, .. } => *view,
            Pick::Circle { view, .. } | Pick::Arc { view, .. } | Pick::CircleAt { view, .. } => *view,
        };
        let Some(pl) = self.placement(view) else { return 0.0 };
        let len = match pick {
            Pick::Point(_) => 0.0,
            Pick::Line { a, b } => {
                let (p, q) = (pl.to_sheet(a.p), pl.to_sheet(b.p));
                (p[0] - q[0]).hypot(p[1] - q[1])
            }
            // What the view draws of the circle: an arc picked as its circle (Maximum / minimum) lights only the arc
            // (live drw-max-min-fillet-arc s2: Front's R4 fillet read nothing, as in drw-dimension-arc-length-fillet).
            Pick::Circle { view, center, radius, .. } | Pick::CircleAt { view, center, radius, .. } => {
                use noshape_model::drawing::views::EdgeGeom;
                let on = |c: [f64; 2], r: f64| (c[0] - center[0]).hypot(c[1] - center[1]) < 1e-9 && (r - radius).abs() < 1e-9;
                let drawn: f64 = self.geoms.get(view).map(|g| {
                    let circles: Vec<u32> = g.model_edges.iter().enumerate().filter(|(_, me)| matches!(me.geom, EdgeGeom::Circle { center: c, radius: r, face_on: true, .. } if on(c, r))).map(|(i, _)| i as u32).collect();
                    g.edges.iter().filter(|d| d.model.is_some_and(|m| circles.contains(&m))).map(|d| d.points.windows(2).map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1])).sum::<f64>()).sum()
                }).unwrap_or_default();
                pl.len_to_sheet(if drawn > 0.0 { drawn } else { std::f64::consts::TAU * radius })
            }
            Pick::Arc { center, radius, a, b, mid, .. } => {
                let at = |p: [f64; 2]| (p[1] - center[1]).atan2(p[0] - center[0]);
                let from_a = |p: [f64; 2]| (at(p) - at(a.p)).rem_euclid(std::f64::consts::TAU);
                let to_b = from_a(b.p);
                let span = if from_a(*mid) <= to_b { to_b } else { std::f64::consts::TAU - to_b };
                pl.len_to_sheet(radius * span)
            }
        };
        len * self.zoom * 2.0
    }

    /// A view's name as its properties show it: its orientation (a projected view's: what it looks along), a
    /// section's "Section" (live 2063), a detail's "Detail".
    pub fn ui_view_name(&self, id: ViewId) -> String {
        let drawing = self.display();
        let Some(sheet) = drawing.sheets.get(self.sheet) else { return String::new() };
        let Some(v) = sheet.view(id) else { return String::new() };
        let oriented = |o: ViewOrientation| noshape_scene::drawing_tools::view_edit::orientation_name(o).to_string();
        match &v.kind {
            ViewKind::Base { orientation, .. } => oriented(*orientation),
            ViewKind::Projected { .. } => views::standard_orientation(drawing, sheet, id).map(oriented).unwrap_or_else(|| "Isometric".into()),
            // Live's export names it "Section", its "SECTION A - A" a note (2063).
            ViewKind::Section { .. } | ViewKind::AlignedSection { .. } => "Section".to_string(),
            ViewKind::Detail { .. } => "Detail".to_string(),
            ViewKind::Auxiliary { .. } => "Auxiliary".to_string(),
        }
    }

    fn ui_view_kind(&self, id: ViewId) -> &'static str {
        let drawing = self.display();
        let Some(v) = drawing.sheets.get(self.sheet).and_then(|s| s.view(id)) else { return "" };
        match &v.kind {
            ViewKind::Base { orientation: ViewOrientation::Isometric, .. } => "isometric",
            ViewKind::Base { .. } => "base",
            ViewKind::Projected { .. } if self.ui_view_name(id) == "Isometric" => "isometric",
            ViewKind::Projected { .. } => "projected",
            ViewKind::Section { .. } | ViewKind::AlignedSection { .. } => "section",
            ViewKind::Detail { .. } => "detail",
            ViewKind::Auxiliary { .. } => "auxiliary",
        }
    }

    /// Annotation `id`'s text as the runner records it.
    pub(crate) fn ui_annotation_text(&self, id: u64) -> Option<String> {
        self.ui_annotations().into_iter().find(|a| a.0 == id).map(|a| a.2)
    }

    /// The sheet's annotations: id, kind and the text drawn for it.
    pub(crate) fn ui_annotations(&self) -> Vec<(u64, &'static str, String)> {
        // What is placed (not a tool's preview: the note on the pointer).
        let Some(sheet) = self.drawing.sheets.get(self.sheet) else { return Vec::new() };
        let text = |id: u64| -> String {
            let Some(b) = &self.built else { return String::new() };
            // (A dimension's dual value, "[40.00]", isn't in live's reading: live2 drw-dprops-dual.)
            let dim = sheet.annotations.iter().any(|a| a.id == id && a.kind.is_dimension());
            let dual = |t: &str| dim && t.starts_with('[') && t.ends_with(']');
            let parts: Vec<&str> = b.geometry.texts.iter().filter(|t| matches!(b.picks.get(t.pick as usize), Some(PickTarget::Annotation(x)) if *x == id)).map(|t| t.text.as_str()).filter(|t| !dual(t.trim())).collect();
            parts.join(" ").split_whitespace().collect::<Vec<_>>().join(" ")
        };
        // The ordinate group being placed is listed once the tool ends (live drw-ordinate-dimension); one being added
        // to stays listed (live2 drw-ordinate-add-to-group).
        let placing = match self.tool {
            Tool::Ordinate { group, .. } if group != self.ordinate_extending.as_ref().map(|e| e.0) => group,
            _ => None,
        };
        sheet
            .annotations
            .iter()
            .filter(|a| Some(a.id) != placing)
            // Not in live's drawing export, which its recorder reads (live2 2181): weld and edge condition symbols,
            // revision callouts (live2 drw-revision-callout-add).
            .filter(|a| !matches!(a.kind, AnnotationKind::Weld { .. } | AnnotationKind::EdgeCondition { .. } | AnnotationKind::RevisionCallout { .. }))
            // An exploded note is polylines in the export (help: "explode the note into polylines upon export"; live2
            // drw-note-explode lists none).
            .filter(|a| !matches!(&a.kind, AnnotationKind::Note { style, .. } if style.explode))
            .map(|a| {
                let kind = format!("{:?}", a.kind);
                let variant = kind.split([' ', '(', '{']).next().unwrap_or("");
                // (Live's recorder's kinds from DRAWING_JSON's types, live 2146: Onshape::Callout "callout",
                // Onshape::Dimension::Ordinate "ordinate", Onshape::SurfaceFinishSymbol "surfacefinishsymbol".)
                let k = match variant {
                    "OrdinateDimension" | "OrdinateGroup" => "ordinate",
                    // (A foreshortened diameter isn't read: live drw-dimension-foreshorten-cylinder s8.)
                    "ForeshortenedDiameter" => "foreshortened",
                    v if v.ends_with("Dimension") || v == "ForeshortenedRadial" => "dimension",
                    "Note" => "note",
                    "Centermark" => "centermark",
                    v if v.contains("Centerline") => "centerline",
                    "Callout" | "HoleCallout" | "Balloon" => "callout",
                    "Datum" => "datum",
                    "DatumTarget" => "datumtarget",
                    "Gdt" => "gdt",
                    "SurfaceFinish" => "surfacefinishsymbol",
                    "VirtualSharp" => "virtualsharp",
                    _ => "other",
                };
                let t = match &a.kind {
                    // A note open in its editor isn't drawn; live reads its saved text until the edit ends (live2
                    // drw-note-edit).
                    AnnotationKind::Note { text: saved, .. } if text(a.id).is_empty() => saved.split_whitespace().collect::<Vec<_>>().join(" "),
                    _ => text(a.id),
                };
                // An ordinate group being added to: its values as they were (live2 drw-ordinate-add-to-group).
                let t = match (&self.ordinate_extending, &self.tool) {
                    (Some((g, before)), Tool::Ordinate { group: Some(cur), .. }) if *g == a.id && *cur == a.id => return (a.id, k, before.clone()),
                    _ => t,
                };
                let t = match k {
                    "dimension" => data_value(&dimension_value(&t), a.format.precision.or(self.drawing.props.precision), self.drawing.props.trailing_zeros),
                    // Each member's value: "0.000 1.575" (live).
                    "ordinate" => t.split_whitespace().map(dimension_value).collect::<Vec<_>>().join(" "),
                    _ => t,
                };
                // An open palette's previewed values: read as committed (live's recorder, see `PaletteDlg::committed`).
                let t = self.palette.as_ref().and_then(|p| p.committed.iter().find(|(id, _)| *id == a.id)).map_or(t, |(_, c)| c.clone());
                (a.id, k, t)
            })
            .collect()
    }

    /// Where live's recorder places an annotation (live2 16:46, sheet mm in tenths): a dimension or a note at its text's
    /// centre, anything else at the centre of its box; none for lines, centerlines and centermarks, which have no box.
    fn ui_position(&self, kind: &str, of: impl Fn(&PickTarget) -> bool) -> Option<[i64; 2]> {
        if matches!(kind, "line" | "centerline" | "centermark") {
            return None;
        }
        let b = self.built.as_ref()?;
        let mine = |pick: u32| b.picks.get(pick as usize).is_some_and(&of);
        let mut points: Vec<[f64; 2]> = b.geometry.texts.iter().filter(|t| mine(t.pick)).flat_map(noshape_model::drawing::font::text_box).collect();
        // A note's box is its text's (live: its leader isn't in it); a callout's its text and border's (live2
        // feel/callout-geom.json: a flag's centre takes its point, not its leader).
        if !matches!(kind, "dimension" | "note") || points.is_empty() {
            let leader = |closed: bool| kind == "callout" && !closed;
            points.extend(b.geometry.lines.iter().filter(|l| mine(l.pick) && !leader(l.closed)).flat_map(|l| l.points.iter().copied()));
        }
        let first = *points.first()?;
        let (lo, hi) = points.iter().fold((first, first), |(lo, hi), p| ([lo[0].min(p[0]), lo[1].min(p[1])], [hi[0].max(p[0]), hi[1].max(p[1])]));
        Some(tenths([(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0]))
    }

    /// The drawing's part of the state the ui-parity runner records (README "### Drawings").
    pub fn ui_drawing_state(&self, cx: &App) -> Value {
        // What is placed (not a tool's preview: the next projected view on the pointer).
        let drawing = &self.drawing;
        let tab = self.doc.read(cx).session.doc.tab(self.tab).map(|t| t.name.clone()).unwrap_or_default();
        let sheets: Vec<&str> = drawing.sheets.iter().map(|s| s.name.as_str()).collect();
        let Some(sheet) = drawing.sheets.get(self.sheet) else { return json!({ "tab": tab, "sheets": sheets }) };
        // A view moved to another sheet: live lists it with no parent, and not its parent's stand-in there (live2
        // drw-view-move-to-sheet-ok).
        let mut views: Vec<Value> = sheet
            .views
            .iter()
            .filter(|v| !v.id.is_ghost())
            .map(|v| {
                // (An aligned section's too: live names its parent, drw-aligned-section-edit.)
                let parent = sheet.parent_of(v.id).filter(|p| !p.is_ghost()).map(|p| self.ui_view_name(p));
                let pl = self.placement(v.id);
                let scale = pl.map(|p| noshape_model::drawing::sheet::format_scale(p.scale));
                // Live2 16:46: the centre of the view's frame, sheet mm (the frame's margin is even all round).
                let position = pl.map(|p| tenths([(p.min[0] + p.max[0]) / 2.0, (p.min[1] + p.max[1]) / 2.0])).map(mm);
                json!({ "name": self.ui_view_name(v.id), "kind": self.ui_view_kind(v.id), "position": position, "parent": parent, "scale": scale, "brokenOut": v.broken_out.len() })
            })
            .collect();
        views.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
        // Live's recorder lists no datum target (live2 drw-datum-target); a foreshortened radius is a dimension (live
        // drw-triage-1126: the 11-26 take lists it).
        let mut annotations: Vec<(&str, String, Option<[i64; 2]>)> = self
            .ui_annotations()
            .into_iter()
            .filter(|(_, k, _)| !matches!(*k, "datumtarget" | "foreshortened"))
            .map(|(id, k, t)| (k, t, self.ui_position(k, |p| matches!(p, PickTarget::Annotation(x) if *x == id))))
            .collect();
        // A view's label is a note in live's export ("SECTION A - A", 2063); a table its drawn texts.
        if let Some(b) = &self.built {
            // (A hole table isn't in live's exports, nor its recorder's state: live 2134.)
            // (A table whose cell is in its Note editor isn't in live's state yet: it shows after the editor closes.)
            let editing = match &self.panel {
                Some(crate::drawing_view::Panel::Cell { table, note: Some(_), .. }) if self.fresh_table == Some(*table) => Some(*table),
                _ => None,
            };
            // (Nor a cut list, drw-cut-list-table-properties; nor a revision table, drw-revision-table. A custom table
            // is: live's fresh drw-table-cell-menu, drw-custom-table-cell and -split takes, REQUESTS studios 17:02 (f).)
            for t in sheet.tables.iter().filter(|t| !matches!(t.kind, TableKind::Hole { .. } | TableKind::CutList { .. } | TableKind::Revision { .. }) && Some(t.id) != editing) {
                let mut texts: Vec<_> = b.geometry.texts.iter().filter(|x| matches!(b.picks.get(x.pick as usize), Some(PickTarget::Table(id)) if *id == t.id)).collect();
                // Bottom to top: live's recorder reads the rows in their data order (the header, item 1, item 2…; live2
                // drw-bom-multi-level-bottom-to-top), bottom up here.
                let upward = matches!(t.kind, TableKind::Bom { bottom_to_top: true, .. });
                if upward {
                    // (A row's cells by their whole millimetre: their anchors differ by a fraction.)
                    texts.sort_by(|a, b| a.pos[1].round().total_cmp(&b.pos[1].round()).then(a.pos[0].total_cmp(&b.pos[0])));
                }
                let parts: Vec<&str> = texts.iter().map(|x| x.text.as_str()).collect();
                annotations.push(("table", parts.join(" ").split_whitespace().collect::<Vec<_>>().join(" "), self.ui_position("table", |p| matches!(p, PickTarget::Table(id) if *id == t.id))));
            }
            for v in &sheet.views {
                let parts: Vec<&str> = b.geometry.texts.iter().filter(|t| matches!(b.picks.get(t.pick as usize), Some(PickTarget::ViewLabel(x)) if *x == v.id)).map(|t| t.text.as_str()).collect();
                let text = parts.join(" ").split_whitespace().collect::<Vec<_>>().join(" ");
                if !text.is_empty() {
                    annotations.push(("note", text, self.ui_position("note", |p| matches!(p, PickTarget::ViewLabel(x) if *x == v.id))));
                }
            }
        }
        // An inspection item is an annotation of its own in live, read as its number (live2 2226), hidden or not.
        annotations.extend(sheet.annotations.iter().filter_map(|a| a.inspection).map(|n| ("inspectionsymbol", n.to_string(), None)));
        let open: &[noshape_sketch::EntityId] = if matches!(self.tool, Tool::Sketch { tool: SkTool::Point, .. }) { &self.tool_points } else { &[] };
        // Profiles drawn in the open Hatch region dialog aren't read until ✓ (live2 drw-hatch-rect-edit s5-s10).
        let drawn: Vec<noshape_sketch::EntityId> = match &self.hatch_dlg {
            Some(d) if d.sections.is_none() => sheet.sketch.sketch.entities().map(|e| e.id).filter(|e| e.0 > d.sketch_start).collect(),
            _ => Vec::new(),
        };
        let open: Vec<noshape_sketch::EntityId> = open.iter().copied().chain(drawn).collect();
        annotations.extend(sheet_sketch_items(&sheet.sketch, &open).into_iter().map(|k| (k, String::new(), None)));
        annotations.sort();
        // What is drawn in selection colours (README "### Drawings"; live 2068): selected views; the Projected
        // tool's parent drawn picked after Insert view placed it. Live's reader (2402) says VIEW for a view drawn
        // with the dark selection frame and EDGE for orange inside a view without it: a selected section, auxiliary
        // or detail view is drawn with the light frame, an aligned section with the dark one (drw-aligned-section-edit;
        // a projected view either way, by its pixels: block-drawing's Right VIEW, its Top EDGE).
        let mut selection: Vec<(String, String)> = Vec::new();
        for s in &self.selection {
            match s {
                Sel::View(id) => {
                    // (Light-framed views read VIEW for the 11-15 recorder too: live2 drw-section-edit-depth-direction.)
                    let framed = !sheet.view(*id).is_some_and(|v| matches!(v.kind, ViewKind::Section { .. } | ViewKind::Auxiliary { .. } | ViewKind::Detail { .. }));
                    // Hovered, its frame is tan with its edges still orange: live's reader says EDGE (drwm-1 e3, e4, v3).
                    let hovered = self.view_frames().iter().any(|(v, c)| v == id && *c != crate::drawing_view::DRAWING_SELECTED);
                    if framed && hovered {
                        selection.push(("EDGE".into(), self.ui_view_name(*id)));
                    } else if framed {
                        selection.push(("VIEW".into(), self.ui_view_name(*id)));
                    } else {
                        selection.push(("EDGE-".into(), self.ui_view_name(*id)));
                        selection.push(("VIEW+".into(), self.ui_view_name(*id)));
                    }
                }
                Sel::Edge(id, _) => selection.push(("EDGE".into(), self.ui_view_name(*id))),
                // Live's recorder can't read an annotation's selection (a clicked callout records []: live2's batch
                // 9 and 10 recordings), so neither side records one.
                Sel::Annotation(_) => {}
                _ => {}
            }
        }
        // (The 11-15 recorder reads it VIEW, the whole view selected; older ones EDGE: live2 drw-insert-view-top-1to1
        // s7 against the other Insert view takes. "+" marks what only the 11-15 recorder reads, "-" what only older
        // ones do.)
        if let Tool::Projected { parent: Some(p) } = &self.tool
            && self.projection_parent_lit
        {
            selection.push(("EDGE-".into(), self.ui_view_name(*p)));
            selection.push(("VIEW+".into(), self.ui_view_name(*p)));
        }
        // A 2 point centerline Esc left: still drawn lit once the pointer is off (live drw-centerline-2-point-cylinder-axis
        // s5, 11-20; edge to edge and circle ones aren't, -edge-to-edge-cylinder s5, -circle-2-point-small-hole-centres s5).
        if matches!(self.tool, Tool::Select) {
            for s in &self.esc_kept {
                let Sel::Annotation(id) = s else { continue };
                if let Some(AnnotationKind::Centerline { view, .. }) = self.current_sheet().annotations.iter().find(|a| a.id == *id).map(|a| &a.kind) {
                    selection.push(("EDGE^".into(), self.ui_view_name(*view)));
                }
            }
        }
        // A dimension's picked edges and a closed broken-out profile are drawn picked (live: drw-dimension-*,
        // drw-broken-out-section).
        match &self.tool {
            // (Live keeps every pick orange, a line's first too: live2 11-14, drw-dimension-edge re-recorded; the older
            // takes' [] after a first line pick was their reader missing an edge on the view's box.)
            // Line to line and its angular one: the first line stays in the hover colour until the second is
            // picked (live2 drw-dimension-line-to-line, -angular).
            Tool::Dimension { kind: DimTool::LineLine | DimTool::Angle, picks } if picks.len() < 2 => {}
            // 3 point angular: its picked points' orange markers (and the preview after the third), then the placed
            // dimension, read as an edge from the first point on (live2 drw-dimension-3-point-angular 11-15 retake).
            Tool::Dimension { kind: DimTool::Angle3, picks } => {
                let placed = self.made_by_tool().into_iter().find_map(|s| match s {
                    Sel::Annotation(id) => self.current_sheet().annotations.iter().find(|a| a.id == id).and_then(|a| a.kind.view()),
                    _ => None,
                });
                let view = match picks.first() {
                    Some(Pick::Point(PickedPoint { view, .. })) => Some(*view),
                    _ => placed,
                };
                if let Some(v) = view {
                    selection.push(("EDGE".into(), self.ui_view_name(v)));
                }
            }
            Tool::Dimension { picks, .. } => {
                if picks.is_empty() {
                    for a in &self.dim_kept_points {
                        selection.push(("EDGE".into(), self.ui_view_name(a.view)));
                    }
                }
                let mut edge_px: Vec<(ViewId, f64)> = Vec::new();
                for p in picks {
                    match p {
                        // A picked point's disc (and the line from it to the pointer) in the 2 point and point to line
                        // tools (live drw-dimension-point-to-point-vertical s2-s3, -point-to-line-hole-centre s2).
                        Pick::Point(PickedPoint { view, .. }) if matches!(self.tool, Tool::Dimension { kind: DimTool::Point2 | DimTool::PointLine, .. }) => selection.push(("EDGE".into(), self.ui_view_name(*view))),
                        // Picked edges' orange, summed per view, read past 60 px as any lit line (live:
                        // Front's R4 fillet arc alone reads nothing, with the short side beside it it does:
                        // drw-max-min-fillet-arc s2-s3; the 4 mm chamfer edge alone, -chamfer-placed s1).
                        Pick::Line { a: PickedPoint { view, .. }, .. } | Pick::Circle { view, .. } | Pick::Arc { view, .. } | Pick::CircleAt { view, .. } => match edge_px.iter_mut().find(|(v, _)| v == view) {
                            Some((_, px)) => *px += self.pick_lit_px(p),
                            None => edge_px.push((*view, self.pick_lit_px(p))),
                        },
                        _ => {}
                    }
                }
                for (v, _) in edge_px.into_iter().filter(|(_, px)| *px > 60.0) {
                    selection.push(("EDGE".into(), self.ui_view_name(v)));
                }
            }
            // A centerline's picked points (a 2 point centerline's disc, a circle centerline's centre with its hole):
            // live drw-centerline-2-point-cylinder-axis s1, drw-centerline-circle-2-point-small-hole-centres s1 (11-18.1);
            // the 11-07 recorders read nothing (drw-centerline-2-point s2, -circle-2-point s2, -circle-3-point s2-s3): `^`.
            Tool::Centerline { first: Some(PickedPoint { view, .. }) } => selection.push(("EDGE^".into(), self.ui_view_name(*view))),
            Tool::CircleCenterline { points, .. } if !points.is_empty() => selection.push(("EDGE^".into(), self.ui_view_name(points[0].view))),
            // A datum target's anchor picked on a view: its edge lit until the target is placed (live2
            // drw-datum-target-hide-dim 11-15, step 3). Recorders before 2026-11-15.1 didn't read it ("EDGE+": marked
            // `new` for the runner).
            Tool::DatumTarget { anchor: Some(PickedPoint { view, .. }) } if *view != noshape_model::drawing::SHEET_VIEW => selection.push(("EDGE+".into(), self.ui_view_name(*view))),
            // A datum's edge until it is placed (drw-datum).
            Tool::Datum { anchor: Some(_) } if let Some((v, _)) = self.datum_at => selection.push(("EDGE".into(), self.ui_view_name(v))),
            // The edge the last symbol was placed on (drw-surface-finish).
            Tool::SurfaceFinish => {
                let last = match &self.panel { Some(Panel::SurfaceFinish { placed, .. }) => placed.last().copied(), _ => None };
                let on = last.and_then(|id| self.current_sheet().annotations.iter().find(|a| a.id == id)).and_then(|a| match &a.kind {
                    AnnotationKind::SurfaceFinish { on: Some((v, ..)), .. } => Some(*v),
                    _ => None,
                });
                if let Some(v) = on {
                    selection.push(("EDGE".into(), self.ui_view_name(v)));
                }
            }
            Tool::BrokenOut { parent: Some(v), done: true, .. } => selection.push(("EDGE".into(), self.ui_view_name(*v))),
            // The hole a Hole/thread callout is being placed for (live drw-hole-callout-palette).
            Tool::HoleCallout { circle: Some((v, ..)) } => selection.push(("EDGE".into(), self.ui_view_name(*v))),
            // An ordinate's datum and its values while the tool runs (live2 drw-ordinate-rclick); Add to ordinate
            // dimension group lights nothing until its first new value (live2 drw-ordinate-add-to-group s7).
            Tool::Ordinate { origin: Some(o), group, .. } if self.ordinate_added || !self.ordinate_extending.as_ref().is_some_and(|(g, _)| Some(*g) == *group) => selection.push(("EDGE".into(), self.ui_view_name(o.view))),
            _ => {}
        }
        // The edge an Edge condition symbol is put on (drw-edge-condition).
        if let Some(Sel::Edge(v, _)) = self.lit_tool_edge() {
            selection.push(("EDGE".into(), self.ui_view_name(v)));
        }
        // A crop profile drawn and closed, waiting for ✓, is orange in its view, and the view under it no longer
        // framed (drw-crop-spline, drw-crop-edit); a picked existing profile isn't (drw-crop-existing-profile).
        if let Some((v, _)) = self.crop_dlg.as_ref().filter(|d| d.picked.is_none()).and_then(|d| d.pending.as_ref()) {
            let name = self.ui_view_name(*v);
            selection.retain(|(k, o)| !(k == "VIEW" && *o == name));
            selection.push(("EDGE".into(), name));
        }
        // A view whose Edit auxiliary view / Edit section dialog is open has the dark frame (drw-aux-edit,
        // drw-section-edit); a broken-out section being edited is orange inside its view (drw-broken-out-edit).
        let framed = |selection: &mut Vec<(String, String)>, v: ViewId, kind: &str| {
            let name = self.ui_view_name(v);
            selection.retain(|(_, o)| *o != name);
            selection.push((kind.into(), name));
        };
        if let Some((v, _)) = &self.aux_edit {
            framed(&mut selection, *v, "VIEW");
        }
        if let Some(e) = &self.section_edit {
            framed(&mut selection, e.view, "VIEW");
        }
        // Edit detail…: its circle orange in the parent view (live2 drw-detail-id-profile-tips s7–s15, recorder
        // 11-17.7: EDGE Front); older recorders missed its thin line (drw-detail-edit, 11-09: nothing).
        if let Some(v) = self.detail_dlg.as_ref().filter(|d| d.edit.is_some()).and_then(|d| d.view) {
            selection.push(("EDGE+".into(), self.ui_view_name(v)));
        }
        if let Some(Panel::BrokenOut { view, edit: Some(_), .. }) = &self.panel {
            framed(&mut selection, *view, "EDGE");
        }
        // A break being edited (Edit... on its line: drw-break-edit; not while its menu is open).
        if let Some((v, _)) = self.break_dlg.as_ref().and_then(|d| d.edit) {
            selection.push(("EDGE".into(), self.ui_view_name(v)));
        }
        // Anything else drawn in selection colours inside a view's box (a selected table, dimension, hatch): live's
        // reader counts its orange pixels there, more than 60 an EDGE of that view (2402: drw-bom-* EDGE Isometric
        // under a selected BOM table). Only while nothing else is going on: a tool or a panel takes the highlight
        // (drw-dimension-foreshorten, Edit hatch region in drw-hatch-edit; BOM table properties keeps it,
        // drw-bom-table-properties). A centermark's or centerline's thin lines count only for the 11-15 recorder (live2
        // drw-centermark-align s5-s7: the right-clicked automatic centermark; older readers counted a pixel short).
        // (Placing a revision callout keeps its table lit: live2 drw-revision-callout-add; so does an open dimension
        // palette: live2 drw-hcallout-override, a callout in Top's box read EDGE Top while its Edit... palette is open.)
        if let Some(b) = self.built.as_ref().filter(|_| matches!(self.tool, Tool::Select | Tool::RevisionCallout { .. }) && self.panel.is_none() && self.hatch_dlg.is_none()) {
            let thin = |id: u64| sheet.annotations.iter().any(|a| a.id == id && matches!(a.kind, AnnotationKind::Centermark { .. } | AnnotationKind::Centerline { .. } | AnnotationKind::CircleCenterline { .. }));
            // A selected hole callout is orange only behind its text (live q-escsel: its leader stays black).
            let callout_lines = self.hole_callout_selection_picks();
            // A hovered note is drawn tan over its orange (live2 drw-leader-add-node s8: the pointer still on the leader
            // just added, nothing read); a hovered dimension keeps its orange (drw-dimension-angular s5).
            let tan = self.hover.and_then(|h| self.sel_of_pick(h)).filter(|s| matches!(s, Sel::Annotation(a) if sheet.annotations.iter().any(|x| x.id == *a && matches!(x.kind, AnnotationKind::Note { .. }))));
            // With Styles open, a selected dimension the pointer isn't on reads nothing, though drawn selected (live
            // drw-styles-radial s12-s13, recorder 11-18.1: R.197 clicked, then Styles; drw-dim-styles-open2 shows it).
            let styles = self.right_panel == Some(crate::drawing_panels::RightPanel::Styles);
            let unread_dim = |a: u64| styles && self.hover.and_then(|h| self.sel_of_pick(h)) != Some(Sel::Annotation(a)) && sheet.annotations.iter().any(|x| x.id == a && x.kind.is_dimension());
            // What Esc left lit (`esc_kept`: a tool made it or it was clicked), hovered under the resting pointer, a note's
            // but, is read as its selection is (live drw-dimension-angular s5, drw-hcallout-override s7, drw-styles-radial
            // s16); a dimension Esc didn't keep reads nothing there (drw-dimension-line-to-line s5).
            let hovered_ann = self.hover.and_then(|h| self.sel_of_pick(h)).and_then(|s| match s {
                Sel::Annotation(a) if self.esc_kept.contains(&s) && !sheet.annotations.iter().any(|x| x.id == a && matches!(x.kind, AnnotationKind::Note { .. })) => Some(a),
                _ => None,
            });
            let lit_as = |pick: u32, thin_ones: bool| {
                let Some(p) = b.picks.get(pick as usize) else { return false };
                if let (Some(a), PickTarget::Annotation(t)) = (hovered_ann, p) && a == *t && thin(a) == thin_ones {
                    return true;
                }
                self.selection.iter().any(|s| match (s, p) {
                    // A table with cells picked is orange only in them (live2 drw-revision-cell-*: no view's box reads it).
                    // (One just placed reads nothing for recorders before 2026-11-18.1 (live2 drw-bom-table s2); see `placed`.)
                    (Sel::Table(a), PickTarget::Table(t)) => a == t && self.cell_sel.is_none_or(|c| c.table != *a) && self.placed_table != Some(*a),
                    (Sel::Hatch(a), PickTarget::Hatch(t)) => a == t,
                    (Sel::Annotation(a), PickTarget::Annotation(t)) => a == t && thin(*a) == thin_ones && tan != Some(Sel::Annotation(*a)) && !unread_dim(*a),
                    (Sel::AutoCentermark(v, k), PickTarget::AutoCentermark { view, center }) => thin_ones && v == view && k == center,
                    (Sel::Sketch(a), PickTarget::SketchEntity(t)) => a == t,
                    _ => false,
                })
            };
            let lit = |pick: u32| lit_as(pick, false);
            let dim_pick = |pick: u32| matches!(b.picks.get(pick as usize), Some(PickTarget::Annotation(id)) if sheet.annotations.iter().any(|a| a.id == *id && a.kind.is_dimension()));
            // A selected view's orange inside another view's box counts there too (live2 2442: the Detail view placed
            // over Top's box, right-clicked, is orange all over, its frame, boundary and edges: EDGE Top and EDGE
            // Detail), a view with an edge picked included.
            let lit_view = |w: ViewId| self.selection.iter().any(|s| matches!(s, Sel::View(x) | Sel::Edge(x, _) if *x == w));
            let selected_view = |pick: u32, v: ViewId| match b.picks.get(pick as usize) {
                Some(PickTarget::Edge { view, .. } | PickTarget::View(view)) => *view != v && lit_view(*view),
                _ => false,
            };
            for v in &sheet.views {
                let Some(pl) = self.placement(v.id) else { continue };
                let inside = |p: [f64; 2]| p[0] >= pl.min[0] && p[0] <= pl.max[0] && p[1] >= pl.min[1] && p[1] <= pl.max[1];
                // A segment's length inside the box (a leader crossing a view into it counts: live2 drw-leader-add-node s10).
                let len_in = |a: [f64; 2], b: [f64; 2]| clipped_len(a, b, pl.min, pl.max);
                let mut px = 0.0;
                // (Highlighted lines are 2 px wide: render_style; a hovered annotation's are drawn tan over the
                // orange, about half of it read: live2 drw-dimension-radius s4, its leader into Top read nothing.)
                // Since the 11-15 recorder a hovered selected dimension reads in full (live drw-styles-radial s3, s9:
                // EDGE Top with the pointer on R.197; the 11-06 drw-dimension-radius s3 read nothing): `hover_px`.
                let mut hover_px = 0.0;
                // A hovered selected dimension's text: read since the 11-14 recorders (live drw-dimension-angular s5,
                // 11-14.12), not by the 11-07 or 11-12.1 ones (drw-dimension-diameter s3, drw-update-deleted-feature s4:
                // Ø.394 under the pointer after Escape).
                let mut hover_text = 0.0;
                for l in b.geometry.lines.iter().filter(|l| (lit(l.pick) && !callout_lines.contains(&l.pick)) || selected_view(l.pick, v.id)) {
                    let hovered = self.hover.is_some_and(|h| self.sel_of_pick(h).is_some_and(|s| self.picks_of(s).contains(&l.pick)));
                    for w in l.points.windows(2) {
                        let n = len_in(w[0], w[1]) * self.zoom;
                        px += n * if hovered { 1.0 } else { 2.0 };
                        if hovered && dim_pick(l.pick) {
                            hover_px += n;
                        }
                    }
                }
                // (A text's ink: about one and a half times its height a character.)
                for t in b.geometry.texts.iter().filter(|t| lit(t.pick) && inside(t.pos)) {
                    let n = t.text.chars().count() as f64 * t.height * self.zoom * 1.5;
                    if dim_pick(t.pick) && self.hover.is_some_and(|h| self.sel_of_pick(h) == self.sel_of_pick(t.pick)) { hover_text += n } else { px += n }
                }
                // A selected dimension's orange grips (push_annotation_look): a 9 px dot at each dimension-line end, a
                // 12 px diamond on each flip handle (live drw-styles-radial s3: R.197's leader end and diamond in Top's
                // box read EDGE Top).
                // (And the grips Esc left drawn once the pointer moved off: `esc_kept`.)
                let stale = |pick: u32| matches!(b.picks.get(pick as usize), Some(PickTarget::Annotation(t)) if self.esc_kept.contains(&Sel::Annotation(*t)));
                for gr in b.geometry.grips.iter().filter(|gr| gr.kind != GripKind::Origin && (lit(gr.pick) || stale(gr.pick)) && inside(gr.pos) && dim_pick(gr.pick)) {
                    let n = if gr.kind == GripKind::Handle { 72.0 } else { 64.0 };
                    if self.hover.is_none_or(|h| self.sel_of_pick(h) != self.sel_of_pick(gr.pick)) { px += n } else { hover_px += n }
                }
                // A selected table's tan grips (table_grip_overlay): a 9 px dot on each corner but the fixed one, a 7 px
                // square on each resize midpoint, kept while a cell is edited; read since the 11-18.1 recorder (live
                // drw-custom-table-cell s7-s11, s18: the table's top-left dot in Top's box; the 11-08 one read nothing
                // there, drw-custom-table-delete s9): in `placed_px`. (Not with a cell's menu open: s12-s13.)
                let cell_menu = self.cell_sel.is_some() && self.frame_menu.as_ref().is_some_and(|m| noshape_app_core::os_menu::is_open_menu(m, cx));
                let mut placed_px = 0.0;
                for s in self.selection.iter().filter(|_| !cell_menu) {
                    let Sel::Table(id) = s else { continue };
                    let (Some(t), Some(l)) = (sheet.table(*id), b.table_layout(*id)) else { continue };
                    let fixed = l.fixed_corner(t.corner);
                    placed_px += l.corners().iter().filter(|c| **c != fixed && inside(**c)).count() as f64 * 64.0
                        + l.resize_grips(t.corner).iter().filter(|g| inside(**g)).count() as f64 * 49.0;
                }
                if px > 60.0 {
                    selection.push(("EDGE".into(), self.ui_view_name(v.id)));
                    continue;
                }
                // A table just placed: the 2026-11-18.1 recorder reads its view's box under it too (live2's six
                // drw-bom-* retakes: EDGE Isometric with TABLE from the placing click), older ones nothing (drw-bom-table).
                let placed = |pick: u32| self.placed_table.is_some_and(|id| self.selection.contains(&Sel::Table(id)) && matches!(b.picks.get(pick as usize), Some(PickTarget::Table(t)) if *t == id));
                for l in b.geometry.lines.iter().filter(|l| placed(l.pick)) {
                    for w in l.points.windows(2) {
                        placed_px += len_in(w[0], w[1]) * self.zoom * 2.0;
                    }
                }
                for t in b.geometry.texts.iter().filter(|t| placed(t.pick) && inside(t.pos)) {
                    placed_px += t.text.chars().count() as f64 * t.height * self.zoom * 1.5;
                }
                if px + placed_px > 60.0 && placed_px > 0.0 {
                    selection.push(("EDGE*".into(), self.ui_view_name(v.id)));
                    continue;
                }
                if px + hover_text > 60.0 {
                    selection.push(("EDGE^".into(), self.ui_view_name(v.id)));
                    continue;
                }
                for l in b.geometry.lines.iter().filter(|l| lit_as(l.pick, true)) {
                    for w in l.points.windows(2) {
                        px += len_in(w[0], w[1]) * self.zoom;
                    }
                }
                if px + hover_px + hover_text > 60.0 {
                    selection.push(("EDGE+".into(), self.ui_view_name(v.id)));
                }
            }
            // A view hovered under the pointer where a menu row was clicked: live lights it at once, its 11-15 reader
            // reads it (live2 drw-note-move-to-title-block s8, drw-view-align-horizontally s4).
            // (Live's reach: lit 6 px under Front's bottom edge in drw-note-move-to-title-block, not 7 px over its top
            // in drw-surface-finish-flip s6.)
            let near = |v: ViewId| {
                self.placement(v).is_some_and(|pl| {
                    let c = self.cursor;
                    let d = (pl.min[0] - c[0]).max(c[0] - pl.max[0]).max(0.0).hypot((pl.min[1] - c[1]).max(c[1] - pl.max[1]).max(0.0));
                    d * self.zoom <= 6.5
                })
            };
            let hovered = self.hover.filter(|_| self.menu_closed_at == Some(self.cursor)).and_then(|h| match b.picks.get(h as usize) {
                Some(PickTarget::View(v) | PickTarget::Edge { view: v, .. }) => Some(*v),
                _ => None,
            }).filter(|v| near(*v));
            // (Not with table cells picked: live2 drw-table-cells-remove s11, Remove ▸ Columns clicked over Isometric.)
            if let Some(v) = hovered.filter(|_| self.cell_sel.is_none()) {
                selection.push(("EDGE+".into(), self.ui_view_name(v)));
            }
        }
        // An inspection item's red-orange bubble counts as selection colour inside its view's box, whatever is selected
        // (live drw-inspection-diameter s13-s14: Ø.394's bubble in Top, nothing selected; drw-inspection-item: 1.575's
        // above Front's box, nothing read).
        if let Some(b) = self.built.as_ref() {
            let bubbles: Vec<&[[f64; 2]]> = b.geometry.colors.iter().filter(|c| c.rgb == noshape_model::drawing::sheet::INSPECTION_FILL).flat_map(|c| b.geometry.fills[c.fills.clone()].iter().map(|f| f.points.as_slice())).collect();
            for v in sheet.views.iter().filter(|_| !bubbles.is_empty()) {
                let Some(pl) = self.placement(v.id) else { continue };
                let area: f64 = bubbles.iter().map(|p| area_inside(p, pl.min, pl.max)).sum();
                if area * self.zoom * self.zoom > 60.0 {
                    selection.push(("EDGE".into(), self.ui_view_name(v.id)));
                }
            }
        }
        // Align views: the view being aligned reads framed while the target is picked, and with the target (under
        // the pointer) after, until the pointer moves; the 11-15 recorder reads them, older ones didn't (live2
        // drw-view-align-horizontally s3-s4, drw-view-align-vertically).
        if let Some((v, _)) = self.align_views {
            selection.push(("VIEW+".into(), self.ui_view_name(v)));
        }
        if let Some((v, to, _)) = self.aligned.filter(|a| a.2 == self.cursor) {
            selection.push(("VIEW+".into(), self.ui_view_name(v)));
            selection.push(("EDGE+".into(), self.ui_view_name(to)));
        }
        // Nothing while the Edit hatch dialog is open (drw-section-hatch-edit, drw-hatch-edit).
        if self.hatch_dlg.is_some() {
            selection.clear();
        }
        // A table just placed, by its kind's name (" 2" for a second of a kind): live2's recorder from 2026-11-18.1 reads
        // its tan outline from the placing click, through Esc, not once it's right-clicked (the six drw-bom-* retakes);
        // older ones don't (marked `*`).
        for s in &self.selection {
            let Sel::Table(id) = s else { continue };
            if self.placed_table != Some(*id) {
                continue;
            }
            // (Not a custom table: live drw-custom-table-cell s7-s11 read only its view's box.)
            let Some(t) = sheet.table(*id).filter(|t| !matches!(t.kind, TableKind::Custom { .. })) else { continue };
            let n = sheet.tables.iter().take_while(|x| x.id != *id).filter(|x| x.type_name() == t.type_name()).count();
            selection.push(("TABLE*".into(), if n == 0 { t.type_name().to_string() } else { format!("{} {}", t.type_name(), n + 1) }));
        }
        selection.sort();
        selection.dedup();
        // (A marked one goes when the same is read anyway.)
        let plain: Vec<(String, String)> = selection.iter().filter(|(k, _)| !k.ends_with(['+', '-', '^'])).cloned().collect();
        selection.retain(|(k, o)| k.strip_suffix(['+', '-', '^']).is_none_or(|k| !plain.iter().any(|p| p.0 == k && p.1 == *o)));
        let selection: Vec<Value> = selection
            .into_iter()
            .map(|(k, o)| match (k.strip_suffix('+'), k.strip_suffix('-')) {
                _ if k.ends_with('*') => json!({ "kind": k.trim_end_matches('*'), "owner": o, "since": "2026-11-18.1" }),
                _ if k.ends_with('^') => json!({ "kind": k.trim_end_matches('^'), "owner": o, "since": "2026-11-14.1" }),
                (Some(k), _) => json!({ "kind": k, "owner": o, "new": true }),
                (_, Some(k)) => json!({ "kind": k, "owner": o, "old": true }),
                _ => json!({ "kind": k, "owner": o }),
            })
            .collect();
        json!({
            "tab": tab,
            "sheet": sheet.name,
            "sheets": sheets,
            // (The Insert view panel before a part is chosen is the Insert view tool's, as live's toolbar shows it.)
            "tool": self.ui_tool().or_else(|| match noshape_app_core::file_tab_view::open_picker(cx) {
                // (Live keeps the tool while its file picker is open: live2 drw-insert-image, drw-insert-dxf.)
                Some("Select an image") => Some("Insert image"),
                Some("Select a DXF or DWG file") => Some("Insert DXF or DWG"),
                _ => None,
            }),
            "views": views,
            // (Live's recorder reads no text for a centermark, a centerline or a surface finish symbol: null, live 2146.)
            "annotations": annotations.into_iter().map(|(k, t, p)| json!({ "kind": k, "text": (!matches!(k, "centermark" | "centerline" | "surfacefinishsymbol" | "virtualsharp" | "line" | "circle" | "arc" | "spline" | "splinepoint" | "sketchpoint" | "hatch")).then_some(t), "position": p.map(mm) })).collect::<Vec<_>>(),
            "selection": selection,
        })
    }

    /// Where the focus is, as live's recorder reads it (README "### Drawings"): the drawing (an iframe there) is
    /// "other"; the Insert view chooser's search box and list a panel "field"; a menu of the window's (a document
    /// tab's) takes it off the drawing.
    fn ui_focus(&self, cx: &App) -> Value {
        if self.ui_list(cx).is_some_and(|l| l.get("drawing").is_none()) {
            return json!({ "kind": "none" });
        }
        if self.insert.chooser.is_some() && !self.chooser.keyboard_left {
            return json!({ "kind": "field", "label": null });
        }
        // The custom table picker: once its list was opened, live's searchable select keeps the focus (live2
        // drw-custom-table-*).
        if matches!(self.custom_tables.dlg.as_ref().and_then(|d| d.picker.as_ref()), Some(crate::drawing_custom_table::Picker::Table { list_open, pending }) if *list_open || pending.is_some()) {
            return json!({ "kind": "field", "label": null });
        }
        json!({ "kind": "other" })
    }

    /// The state the ui-parity runner records after each step: the drawing's own key, the others empty but the
    /// undo state (README "state").
    pub fn ui_state(&self, cx: &App) -> Value {
        self.note_tool_tip();
        let undo = self.doc.read(cx).session.can_undo();
        let redo = self.doc.read(cx).session.can_redo();
        json!({
            "focus": self.ui_focus(cx),
            "dialog": noshape_app_core::ui_script::open_dialog(),
            "list": self.ui_list(cx),
            "selection": { "count": self.selection.len(), "items": [] },
            "features": { "rows": [], "rollbackAfter": null },
            "parts": [],
            // The message bubble shown (live2 drw-canvas-paste-as-table: "Clipboard is empty.").
            "message": self.toast.as_ref().map(|t| t.to_string()),
            "tooltip": null,
            "undo": { "undo": undo, "redo": redo },
            "sketch": null,
            "drawing": self.ui_drawing_state(cx),
        })
    }

    /// The open menu or list: a menu of the window's (a document tab's), else a list a control noted. A context menu
    /// of the drawing's own is in live's drawing iframe, which its recorder reads from 2026-11-15 on (live2
    /// drw-surface-finish-flip: "RmbMenu") and not before (live drw-note-delete, drw-view-properties: list null):
    /// marked `drawing` for the runner, as the panels' dropdowns.
    fn ui_list(&self, cx: &App) -> Option<Value> {
        // The Reference manager's open Version graph (noted as a list by `render_ref_manager`).
        if self.ref_manager.as_ref().is_some_and(|m| m.picking.is_some()) && noshape_app_core::os_menu::is_open(cx) {
            return noshape_app_core::ui_script::open_list();
        }
        if self.frame_menu.as_ref().is_some_and(|m| noshape_app_core::os_menu::is_open_menu(m, cx)) {
            return noshape_app_core::os_menu::root_ui_state(cx).map(|mut l| {
                l["drawing"] = serde_json::json!(true);
                l
            });
        }
        // A dropdown of a panel over the drawing (Drawing properties' selects, the Inspection table's ⋯): live's are
        // in the drawing's iframe, which its recorder reads from 2026-11-15.1 on (live drw-hole-table-cell-format) and
        // not before (live2 drw-drawprops-*, drw-inspection-table-export-csv): marked `drawing` for the runner.
        if noshape_app_core::os_menu::open_position(cx).is_some_and(|p| self.bounds.contains(&p)) {
            return noshape_app_core::os_menu::ui_state(cx).map(|mut l| {
                l["drawing"] = serde_json::json!(true);
                l
            });
        }
        noshape_app_core::os_menu::ui_state(cx).or_else(noshape_app_core::ui_script::open_list)
    }
}

/// The active drawing tool's tooltip name (the toolbar's, without its shortcut); `None` for Select.
impl DrawingEditor {
    /// The tool a dialog stands for while the tool itself is Select, as live's toolbar shows it active (the Insert
    /// view panel before a part is chosen, a placed weld symbol's dialog, Hatch region, Format painter…).
    /// The toolbar's active tool as live shows it: none while a menu's edit of an existing item runs (a break line's
    /// Edit..., a note's Add leader: live2 drw-break-edit, drw-note-add-leader).
    fn ui_tool(&self) -> Option<&'static str> {
        // (An edit isn't the tool's: Edit... on a break, Edit crop..., Edit hatch region..., live drw-break-edit,
        // drw-crop-edit, drw-hatch-edit.)
        if matches!(self.tool, Tool::GdtLeader { .. })
            || self.break_dlg.as_ref().is_some_and(|d| d.edit.is_some())
            || self.crop_dlg.as_ref().is_some_and(|d| d.edit.is_some())
            || self.detail_dlg.as_ref().is_some_and(|d| d.edit.is_some())
            || self.hatch_dlg.as_ref().is_some_and(|d| d.edit.is_some() && d.sections.is_none())
        {
            return None;
        }
        // A profile drawn in the Hatch region dialog: still its tool (live2 drw-hatch-rect-edit s3, Corner rectangle).
        if matches!(self.tool, Tool::Sketch { .. }) && self.hatch_dlg.as_ref().is_some_and(|d| d.sections.is_none() && d.edit.is_none()) {
            return Some("Hatch region");
        }
        ui_tool_name(&self.tool).or_else(|| self.dialog_tool_name())
    }

    fn dialog_tool_name(&self) -> Option<&'static str> {
        if !matches!(self.tool, Tool::Select) {
            return None;
        }
        // (Replace...'s chooser is no tool: live2 drw-sheet-ref-replace.)
        Some(if (self.insert.panel || self.insert.chooser.is_some()) && self.insert.replace.is_none() {
            "Insert view"
        } else if self.weld_dlg.is_some() {
            "Weld symbol"
        } else if self.edge_condition_dlg.is_some() {
            "Edge condition symbol"
        } else if self.hatch_dlg.as_ref().is_some_and(|d| d.sections.is_none()) {
            "Hatch region"
        } else if self.painter.is_some() {
            "Format painter"
        } else if matches!(&self.panel, Some(crate::drawing_view::Panel::Cell { table, note: Some(_), .. }) if self.fresh_table == Some(*table)) {
            // A new table's cell in its Note editor: still the Table tool (live 2184).
            "Table"
        } else if self.inspection.as_ref().is_some_and(|d| d.reorder.is_none() && d.edit.is_none()) {
            "Inspection item"
        } else if self.drawing.props.hide_inspection {
            // The toolbar's pressed toggle.
            "Hide inspection items"
        } else {
            return None;
        })
    }
}

fn ui_tool_name(t: &Tool) -> Option<&'static str> {
    Some(match t {
        // Foreshorten (a radius's menu) and Add revision callout (a revision table's) are no toolbar tools (live2
        // drw-revision-callout-add).
        Tool::Select | Tool::Foreshorten { .. } | Tool::RevisionCallout { .. } => return None,
        Tool::InsertView { .. } => "Insert view",
        Tool::Projected { .. } => "Projected view",
        Tool::Auxiliary { .. } => "Auxiliary view",
        Tool::Section { .. } => "Section view",
        Tool::AlignedSection { .. } => "Aligned section view",
        Tool::Detail { .. } => "Detail view",
        Tool::Crop { .. } | Tool::CropPoly { .. } => "Crop view",
        Tool::Break { .. } => "Break view",
        Tool::Dimension { kind, .. } => match kind {
            DimTool::Linear => "Dimension",
            DimTool::Angle => "Line to line angular dimension",
            DimTool::MaxMin => "Maximum or minimum dimension",
            DimTool::Foreshorten => "Foreshorten dimension",
            k => k.name(),
        },
        Tool::Ordinate { .. } => "Ordinate dimension",
        Tool::Note => "Note",
        Tool::Centermark => "Centermark",
        Tool::Centerline { .. } => "2 point centerline",
        Tool::EdgeCenterline { .. } => "Edge to edge centerline",
        Tool::VirtualSharp { .. } => "Virtual sharp",
        Tool::HoleCallout { .. } => "Hole/thread callout",
        // The Datum dialog's Datum target tab is still the Datum tool (live2 drw-datum-target).
        Tool::Datum { .. } | Tool::DatumTarget { .. } => "Datum",
        Tool::Gdt => "Geometric tolerance",
        Tool::SurfaceFinish => "Surface finish symbol",
        Tool::Weld => "Weld symbol",
        Tool::BrokenOut { .. } => "Broken-out section",
        Tool::Balloon { .. } => "Callout",
        Tool::EdgeCondition => "Edge condition symbol",
        // The rest by the tool's own name (the table tools, Crop, Break… as their tooltips).
        t => t.name(),
    })
}

/// A dimension as live's recorder reads it: its value without the Ø / R prefix, with its leading zero ("Ø.394" →
/// "0.394").
/// The sheet's sketch geometry as live's recorder lists it with the annotations (live2 drw-sketch-spline,
/// drw-sketch-point, drw-hatch-edit): each line, circle, arc and spline, a spline's through points, a point no curve
/// uses, each hatch region.
fn sheet_sketch_items(sk: &noshape_model::drawing::sheet_sketch::SheetSketch, open: &[noshape_sketch::EntityId]) -> Vec<&'static str> {
    use noshape_sketch::entities::Geometry;
    let used: std::collections::HashSet<_> = sk.sketch.entities().flat_map(|e| e.point_refs()).collect();
    let mut out = Vec::new();
    for e in sk.sketch.entities().filter(|e| !e.external && !open.contains(&e.id)) {
        match &e.geometry {
            Geometry::Point { .. } if !used.contains(&e.id) => out.push("sketchpoint"),
            Geometry::Point { .. } => {}
            Geometry::Line { .. } => out.push("line"),
            Geometry::Circle { .. } => out.push("circle"),
            Geometry::Arc { .. } => out.push("arc"),
            Geometry::FitSpline { points, .. } => {
                out.push("spline");
                // (A closed one lists its first point again at the end.)
                let n = if points.len() > 1 && points.first() == points.last() { points.len() - 1 } else { points.len() };
                out.extend(std::iter::repeat_n("splinepoint", n));
            }
            Geometry::BSpline { .. } | Geometry::Conic { .. } => out.push("spline"),
            Geometry::Ellipse { .. } | Geometry::EllipticalArc { .. } => out.push("ellipse"),
        }
    }
    out.extend(sk.hatches.iter().map(|_| "hatch"));
    out
}

/// (An arc length's ⌒ isn't in it: live drw-dimension-arc-length-fillet "0.247"; a chamfer's callout is read as drawn,
/// ".157 X 45.0°": drw-dimension-chamfer-placed.)
fn dimension_value(shown: &str) -> String {
    let v = shown.trim_start_matches(['Ø', '⌀', 'R', 'S', '⌒']);
    if v.starts_with('.') && !v.contains(' ') { format!("0{v}") } else { v.to_string() }
}

/// A plain dimension value as live's recorder reads it from the drawing data: a period, and every digit of the
/// precision whatever the sheet's zero suppression (live units-1119: an ISO drawing draws "40", reads "40.00").
fn data_value(v: &str, precision: Option<u8>, keep_zeros: bool) -> String {
    if v.is_empty() || !v.chars().all(|c| c.is_ascii_digit() || matches!(c, '.' | ',' | '-')) {
        return v.to_string();
    }
    let v = v.replace(',', ".");
    let (Some(p), false) = (precision.map(usize::from), keep_zeros) else { return v };
    let (int, dec) = v.split_once('.').unwrap_or((&v, ""));
    if p == 0 || dec.len() >= p {
        return v;
    }
    format!("{int}.{dec:0<p$}")
}

/// A sheet point in tenths of a millimetre (live's recorder rounds to 0.1 mm), comparable and sortable.
fn tenths(p: [f64; 2]) -> [i64; 2] {
    [(p[0] * 10.0).round() as i64, (p[1] * 10.0).round() as i64]
}

/// [`tenths`] back to millimetres for the state.
fn mm(p: [i64; 2]) -> [f64; 2] {
    [p[0] as f64 / 10.0, p[1] as f64 / 10.0]
}

/// The area of polygon `p` inside the box `min`-`max` (Sutherland-Hodgman clipping, then the shoelace).
fn area_inside(p: &[[f64; 2]], min: [f64; 2], max: [f64; 2]) -> f64 {
    let mut poly = p.to_vec();
    for (k, bound, keep_below) in [(0, min[0], false), (0, max[0], true), (1, min[1], false), (1, max[1], true)] {
        let inside = |q: &[f64; 2]| if keep_below { q[k] <= bound } else { q[k] >= bound };
        let mut out = Vec::with_capacity(poly.len() + 4);
        for (i, a) in poly.iter().enumerate() {
            let b = &poly[(i + 1) % poly.len()];
            if inside(a) {
                out.push(*a);
            }
            if inside(a) != inside(b) {
                let t = (bound - a[k]) / (b[k] - a[k]);
                out.push([a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1])]);
            }
        }
        poly = out;
        if poly.is_empty() {
            return 0.0;
        }
    }
    (0..poly.len()).map(|i| { let (a, b) = (poly[i], poly[(i + 1) % poly.len()]); a[0] * b[1] - b[0] * a[1] }).sum::<f64>().abs() / 2.0
}

/// The length of segment `a`-`b` inside the box `min`-`max` (Liang-Barsky clipping).
fn clipped_len(a: [f64; 2], b: [f64; 2], min: [f64; 2], max: [f64; 2]) -> f64 {
    let d = [b[0] - a[0], b[1] - a[1]];
    let (mut t0, mut t1) = (0.0_f64, 1.0_f64);
    for k in 0..2 {
        if d[k] == 0.0 {
            if a[k] < min[k] || a[k] > max[k] {
                return 0.0;
            }
            continue;
        }
        let (ta, tb) = ((min[k] - a[k]) / d[k], (max[k] - a[k]) / d[k]);
        t0 = t0.max(ta.min(tb));
        t1 = t1.min(ta.max(tb));
    }
    if t1 <= t0 {
        return 0.0;
    }
    (t1 - t0) * d[0].hypot(d[1])
}

#[cfg(test)]
mod tests {
    use super::{area_inside, clipped_len, data_value};

    #[test]
    fn a_fill_counts_its_area_inside_the_box() {
        let (min, max) = ([0.0, 0.0], [10.0, 4.0]);
        let square = |x: f64, y: f64| [[x, y], [x + 2.0, y], [x + 2.0, y + 2.0], [x, y + 2.0]];
        assert_eq!(area_inside(&square(1.0, 1.0), min, max), 4.0, "all inside");
        assert_eq!(area_inside(&square(9.0, 3.0), min, max), 1.0, "over a corner");
        assert_eq!(area_inside(&square(1.0, 6.0), min, max), 0.0, "above the box (drw-inspection-item: 1.575's bubble over Front)");
    }

    #[test]
    fn a_segment_counts_its_length_inside_the_box() {
        let (min, max) = ([0.0, 0.0], [10.0, 4.0]);
        assert_eq!(clipped_len([5.0, -6.0], [5.0, 4.0], min, max), 4.0, "a leader ending on the box's top edge from below");
        assert_eq!(clipped_len([2.0, 1.0], [8.0, 1.0], min, max), 6.0, "all inside");
        assert_eq!(clipped_len([-5.0, 6.0], [15.0, 6.0], min, max), 0.0, "outside");
        assert!((clipped_len([-2.0, -2.0], [12.0, 12.0], min, max) - 4.0 * 2f64.sqrt()).abs() < 1e-12, "a diagonal through a corner");
    }

    #[test]
    fn a_dimension_reads_as_lives_drawing_data() {
        assert_eq!(data_value("40", Some(2), false), "40.00", "ISO: 40 drawn");
        assert_eq!(data_value("12,5", Some(2), false), "12.50", "a comma read as a period");
        assert_eq!(data_value("1.575", Some(3), true), "1.575", "inch: zeros kept already");
        assert_eq!(data_value("2 3/4", Some(3), false), "2 3/4", "not a plain number");
        assert_eq!(data_value("90.0°", Some(1), false), "90.0°");
    }
}
