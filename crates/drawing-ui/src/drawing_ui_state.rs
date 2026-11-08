//! The ui-parity state of a drawing tab and where its targets show (tests/ui-parity/README.md "### Drawings").

use super::*;
use noshape_scene::drawing_tools::{DimTool, Pick, PickedPoint, Sel, Tool};
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
            self.display().sheets.get(self.sheet).and_then(|s| s.annotations.iter().find(|x| x.id == a.0)).map(|x| x.position).ok_or("no annotation")?
        } else {
            return Err(format!("not a drawing target: {t}"));
        };
        let w = self.sheet_to_window(at);
        if !self.bounds.contains(&w) {
            return Err(format!("{t} is outside the sheet's area"));
        }
        Ok(w)
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
    fn ui_annotations(&self) -> Vec<(u64, &'static str, String)> {
        // What is placed (not a tool's preview: the note on the pointer).
        let Some(sheet) = self.drawing.sheets.get(self.sheet) else { return Vec::new() };
        let text = |id: u64| -> String {
            let Some(b) = &self.built else { return String::new() };
            let parts: Vec<&str> = b.geometry.texts.iter().filter(|t| matches!(b.picks.get(t.pick as usize), Some(PickTarget::Annotation(x)) if *x == id)).map(|t| t.text.as_str()).collect();
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
            // Not in live's drawing export, which its recorder reads (live2 2181): weld and edge condition symbols.
            .filter(|a| !matches!(a.kind, AnnotationKind::Weld { .. } | AnnotationKind::EdgeCondition { .. }))
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
                    "ForeshortenedRadial" | "ForeshortenedDiameter" => "foreshortened",
                    v if v.ends_with("Dimension") => "dimension",
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
                    "dimension" => dimension_value(&t),
                    // Each member's value: "0.000 1.575" (live).
                    "ordinate" => t.split_whitespace().map(dimension_value).collect::<Vec<_>>().join(" "),
                    _ => t,
                };
                (a.id, k, t)
            })
            .collect()
    }

    /// The drawing's part of the state the ui-parity runner records (README "### Drawings").
    pub fn ui_drawing_state(&self, cx: &App) -> Value {
        // What is placed (not a tool's preview: the next projected view on the pointer).
        let drawing = &self.drawing;
        let tab = self.doc.read(cx).session.doc.tab(self.tab).map(|t| t.name.clone()).unwrap_or_default();
        let sheets: Vec<&str> = drawing.sheets.iter().map(|s| s.name.as_str()).collect();
        let Some(sheet) = drawing.sheets.get(self.sheet) else { return json!({ "tab": tab, "sheets": sheets }) };
        let mut views: Vec<Value> = sheet
            .views
            .iter()
            .map(|v| {
                let parent = match &v.kind {
                    ViewKind::Projected { parent, .. } | ViewKind::Section { parent, .. } | ViewKind::Detail { parent, .. } | ViewKind::Auxiliary { parent, .. } => Some(self.ui_view_name(*parent)),
                    _ => None,
                };
                let scale = self.placement(v.id).map(|p| noshape_model::drawing::sheet::format_scale(p.scale));
                json!({ "name": self.ui_view_name(v.id), "kind": self.ui_view_kind(v.id), "parent": parent, "scale": scale, "brokenOut": v.broken_out.len() })
            })
            .collect();
        views.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
        // Live's recorder lists no datum target (live2 drw-datum-target), nor a foreshortened dimension (live2
        // drw-dimension-foreshorten: its export drops it).
        let mut annotations: Vec<(&str, String)> = self.ui_annotations().into_iter().filter(|(_, k, _)| !matches!(*k, "datumtarget" | "foreshortened")).map(|(_, k, t)| (k, t)).collect();
        // A view's label is a note in live's export ("SECTION A - A", 2063); a table its drawn texts.
        if let Some(b) = &self.built {
            // (A hole table isn't in live's exports, nor its recorder's state: live 2134.)
            // (A table whose cell is in its Note editor isn't in live's state yet: it shows after the editor closes.)
            let editing = match &self.panel {
                Some(crate::drawing_view::Panel::Cell { table, note: Some(_), .. }) => Some(*table),
                _ => None,
            };
            // (Nor a custom table: live2 drw-custom-table-insert lists none after placing one.)
            for t in sheet.tables.iter().filter(|t| !matches!(t.kind, TableKind::Hole { .. } | TableKind::Custom { .. }) && Some(t.id) != editing) {
                let parts: Vec<&str> = b.geometry.texts.iter().filter(|x| matches!(b.picks.get(x.pick as usize), Some(PickTarget::Table(id)) if *id == t.id)).map(|x| x.text.as_str()).collect();
                annotations.push(("table", parts.join(" ").split_whitespace().collect::<Vec<_>>().join(" ")));
            }
            for v in &sheet.views {
                let parts: Vec<&str> = b.geometry.texts.iter().filter(|t| matches!(b.picks.get(t.pick as usize), Some(PickTarget::ViewLabel(x)) if *x == v.id)).map(|t| t.text.as_str()).collect();
                let text = parts.join(" ").split_whitespace().collect::<Vec<_>>().join(" ");
                if !text.is_empty() {
                    annotations.push(("note", text));
                }
            }
        }
        // An inspection item is an annotation of its own in live, read as its number (live2 2226), hidden or not.
        annotations.extend(sheet.annotations.iter().filter_map(|a| a.inspection).map(|n| ("inspectionsymbol", n.to_string())));
        annotations.extend(sheet_sketch_items(&sheet.sketch).into_iter().map(|k| (k, String::new())));
        annotations.sort();
        // What is drawn in selection colours (README "### Drawings"; live 2068): selected views; the Projected
        // tool's parent drawn picked after Insert view placed it.
        let mut selection: Vec<(String, String)> = Vec::new();
        for s in &self.selection {
            match s {
                Sel::View(id) => selection.push(("VIEW".into(), self.ui_view_name(*id))),
                // Live's recorder can't read an annotation's selection (a clicked callout records []: live2's batch
                // 9 and 10 recordings), so neither side records one.
                Sel::Annotation(_) => {}
                _ => {}
            }
        }
        if let Tool::Projected { parent: Some(p) } = &self.tool
            && self.projection_parent_lit
        {
            selection.push(("EDGE".into(), self.ui_view_name(*p)));
        }
        // A dimension's picked edges and a closed broken-out profile are drawn picked (live: drw-dimension-*,
        // drw-broken-out-section).
        match &self.tool {
            // (Live: a picked circle shows at once, a line once it is the second pick: drw-dimension-edge, -angular,
            // -point-to-line.)
            Tool::Dimension { picks, .. } => {
                let lines = picks.iter().filter(|p| matches!(p, Pick::Line { .. })).count();
                for p in picks {
                    match p {
                        Pick::Line { a: PickedPoint { view, .. }, .. } if lines >= 2 || picks.len() >= 2 => selection.push(("EDGE".into(), self.ui_view_name(*view))),
                        Pick::Circle { view, .. } | Pick::Arc { view, .. } | Pick::CircleAt { view, .. } => selection.push(("EDGE".into(), self.ui_view_name(*view))),
                        _ => {}
                    }
                }
            }
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
            _ => {}
        }
        selection.sort();
        selection.dedup();
        let selection: Vec<Value> = selection.into_iter().map(|(k, o)| json!({ "kind": k, "owner": o })).collect();
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
            "annotations": annotations.into_iter().map(|(k, t)| json!({ "kind": k, "text": (!matches!(k, "centermark" | "centerline" | "surfacefinishsymbol" | "virtualsharp" | "line" | "circle" | "arc" | "spline" | "splinepoint" | "sketchpoint" | "hatch")).then_some(t) })).collect::<Vec<_>>(),
            "selection": selection,
        })
    }

    /// Where the focus is, as live's recorder reads it (README "### Drawings"): the drawing (an iframe there) is
    /// "other"; the Insert view chooser's search box and list a panel "field"; a menu of the window's (a document
    /// tab's) takes it off the drawing.
    fn ui_focus(&self, cx: &App) -> Value {
        if self.ui_list(cx).is_some() {
            return json!({ "kind": "none" });
        }
        if self.insert.chooser.is_some() {
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

    /// The open menu or list: a menu of the window's (a document tab's), else a list a control noted; not a context
    /// menu of the drawing's own, which is in live's drawing iframe where its recorder can't read it (live
    /// drw-note-delete, drw-view-properties: list null, focus other).
    fn ui_list(&self, cx: &App) -> Option<Value> {
        if self.frame_menu.as_ref().is_some_and(|m| noshape_app_core::os_menu::is_open_menu(m, cx)) {
            return None;
        }
        // A dropdown of a panel over the drawing (Drawing properties' selects, the Inspection table's ⋯): live's are
        // in the drawing's iframe too (live2 drw-drawprops-*, drw-inspection-table-export-csv).
        if noshape_app_core::os_menu::open_position(cx).is_some_and(|p| self.bounds.contains(&p)) {
            return None;
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
        if matches!(self.tool, Tool::GdtLeader { .. }) || self.break_dlg.as_ref().is_some_and(|d| d.edit.is_some()) {
            return None;
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
        } else if matches!(self.panel, Some(crate::drawing_view::Panel::Cell { note: Some(_), .. })) {
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
        // Foreshorten (a radius's menu) is no toolbar tool.
        Tool::Select | Tool::Foreshorten { .. } => return None,
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
fn sheet_sketch_items(sk: &noshape_model::drawing::sheet_sketch::SheetSketch) -> Vec<&'static str> {
    use noshape_sketch::entities::Geometry;
    let used: std::collections::HashSet<_> = sk.sketch.entities().flat_map(|e| e.point_refs()).collect();
    let mut out = Vec::new();
    for e in sk.sketch.entities().filter(|e| !e.external) {
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

fn dimension_value(shown: &str) -> String {
    let v = shown.trim_start_matches(['Ø', '⌀', 'R', 'S']);
    if v.starts_with('.') { format!("0{v}") } else { v.to_string() }
}
