//! The ui-parity state of a drawing tab and where its targets show (tests/ui-parity/README.md "### Drawings").

use super::*;
use noshape_scene::drawing_tools::{Sel, Tool};
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
    /// `view_point` / `view_edge` {view, at: [x, y, z] mm}, `annotation` (by its text).
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
        } else if let Some(text) = t.get("annotation").and_then(Value::as_str) {
            let a = self.ui_annotations().into_iter().find(|(_, _, s)| s == text).ok_or_else(|| format!("no annotation {text}"))?;
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
    /// section's "Section A-A", a detail's "Detail A".
    pub fn ui_view_name(&self, id: ViewId) -> String {
        let drawing = self.display();
        let Some(sheet) = drawing.sheets.get(self.sheet) else { return String::new() };
        let Some(v) = sheet.view(id) else { return String::new() };
        let oriented = |o: ViewOrientation| noshape_scene::drawing_tools::view_edit::orientation_name(o).to_string();
        match &v.kind {
            ViewKind::Base { orientation, .. } => oriented(*orientation),
            ViewKind::Projected { .. } => views::standard_orientation(drawing, sheet, id).map(oriented).unwrap_or_else(|| "Isometric".into()),
            ViewKind::Section { label, .. } => format!("Section {label}-{label}"),
            ViewKind::Detail { label, .. } => format!("Detail {label}"),
            _ => v.label.clone().unwrap_or_else(|| self.ui_view_kind(id).to_string()),
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
            ViewKind::Section { .. } => "section",
            ViewKind::Detail { .. } => "detail",
            ViewKind::Auxiliary { .. } => "auxiliary",
            _ => "other",
        }
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
        sheet
            .annotations
            .iter()
            .map(|a| {
                let kind = format!("{:?}", a.kind);
                let variant = kind.split([' ', '(', '{']).next().unwrap_or("");
                let k = match variant {
                    v if v.ends_with("Dimension") => "dimension",
                    "Note" => "note",
                    "Centermark" => "centermark",
                    v if v.contains("Centerline") => "centerline",
                    "Callout" | "HoleCallout" => "callout",
                    "Datum" => "datum",
                    "Gdt" => "gdt",
                    _ => "other",
                };
                (a.id, k, text(a.id))
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
                json!({ "name": self.ui_view_name(v.id), "kind": self.ui_view_kind(v.id), "parent": parent, "scale": scale })
            })
            .collect();
        views.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
        let mut annotations: Vec<(&str, String)> = self.ui_annotations().into_iter().map(|(_, k, t)| (k, t)).collect();
        annotations.sort();
        let selection: Vec<Value> = self
            .selection
            .iter()
            .filter_map(|s| match s {
                Sel::View(id) => Some(json!({ "kind": "VIEW", "owner": self.ui_view_name(*id) })),
                Sel::Annotation(id) => Some(json!({ "kind": "ANNOTATION", "owner": self.ui_annotations().into_iter().find(|a| a.0 == *id).map(|a| a.2) })),
                _ => None,
            })
            .collect();
        json!({
            "tab": tab,
            "sheet": sheet.name,
            "sheets": sheets,
            "tool": ui_tool_name(&self.tool),
            "views": views,
            "annotations": annotations.into_iter().map(|(k, t)| json!({ "kind": k, "text": t })).collect::<Vec<_>>(),
            "selection": selection,
        })
    }

    /// The state the ui-parity runner records after each step: the drawing's own key, the others empty but the
    /// undo state (README "state").
    pub fn ui_state(&self, cx: &App) -> Value {
        let undo = self.doc.read(cx).session.can_undo();
        let redo = self.doc.read(cx).session.can_redo();
        json!({
            "focus": { "kind": "graphics" },
            "dialog": noshape_app_core::ui_script::open_dialog(),
            "list": noshape_app_core::ui_script::open_list(),
            "selection": { "count": self.selection.len(), "items": [] },
            "features": { "rows": [], "rollbackAfter": null },
            "parts": [],
            "message": null,
            "tooltip": null,
            "undo": { "undo": undo, "redo": redo },
            "sketch": null,
            "drawing": self.ui_drawing_state(cx),
        })
    }
}

/// The active drawing tool's tooltip name (the toolbar's, without its shortcut); `None` for Select.
fn ui_tool_name(t: &Tool) -> Option<&'static str> {
    Some(match t {
        Tool::Select => return None,
        Tool::InsertView { .. } => "Insert view",
        Tool::Projected { .. } => "Projected view",
        Tool::Auxiliary { .. } => "Auxiliary view",
        Tool::Section { .. } => "Section view",
        Tool::AlignedSection { .. } => "Aligned section view",
        Tool::Detail { .. } => "Detail view",
        Tool::Crop { .. } => "Crop view",
        Tool::Break { .. } => "Break view",
        Tool::Dimension { .. } => "Dimension",
        Tool::Ordinate { .. } => "Ordinate dimension",
        Tool::Note => "Note",
        Tool::Centermark => "Centermark",
        Tool::Centerline { .. } => "2 point centerline",
        Tool::EdgeCenterline { .. } => "Edge to edge centerline",
        Tool::VirtualSharp { .. } => "Virtual sharp",
        Tool::HoleCallout { .. } => "Hole/thread callout",
        Tool::Datum { .. } => "Datum",
        Tool::Gdt => "Geometric tolerance",
        Tool::SurfaceFinish => "Surface finish symbol",
        Tool::Weld => "Weld symbol",
        _ => "other",
    })
}
