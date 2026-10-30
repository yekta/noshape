//! Drawing editor: linear dimensions' context menu as Onshape's (live
//! dom/context-menus-drawing.txt "rmb-dw-dimension", help addjog-contextmenu):
//! Edit…, Paste as table, Break / Unbreak dimension, Add jog (on an
//! extension line) | Clear selection, Zoom to fit, Delete; and dragging a
//! jog's snap points (help jog-dimensionline-highlightex).

use crate::drawing_view::*;
use noshape_model::drawing::sheet::LinearFrame;
use noshape_model::drawing::{Annotation, AnnotationKind};

impl DrawingEditor {
    /// A linear dimension's geometry on the sheet (resolved positions).
    pub fn dimension_frame(&self, a: &Annotation) -> Option<LinearFrame> {
        self.pk().dimension_frame(a)
    }

    /// The resolved annotation `id` (model geometry followed).
    fn resolved_annotation(&self, id: u64) -> Option<Annotation> {
        self.pk().resolved_annotation(id)
    }

    /// The extension line (0 / 1) of linear dimension `a` under the cursor.
    fn extension_line_at(&self, a: &Annotation) -> Option<usize> {
        self.pk().extension_line_at(a)
    }

    /// Onshape's menu of a linear dimension (None for other annotations).
    pub fn dimension_entries(&self, id: u64) -> Option<Vec<MenuEntry>> {
        let a = self.resolved_annotation(id)?;
        if !matches!(a.kind, AnnotationKind::LinearDimension { .. }) || a.inspection.is_some() {
            return None;
        }
        let mut out = Vec::new();
        let a2 = a.clone();
        out.push(MenuEntry::item("Edit...", move |t, w, cx| t.open_annotation_panel(&a2, w, cx)));
        // Live: always present.
        out.push(MenuEntry::item("Paste as table", |t, _, cx| {
            t.start_paste_table(cx);
        }));
        let unbroken = a.unbroken;
        out.push(MenuEntry::item(if unbroken { "Break dimension" } else { "Unbreak dimension" }, move |t, _, cx| {
            t.edit_sheet(if unbroken { "Break dimension" } else { "Unbreak dimension" }, cx, |s| s.annotations.iter_mut().filter(|x| x.id == id).for_each(|x| x.unbroken = !unbroken))
        }));
        if let Some(k) = self.extension_line_at(&a) {
            if a.jogs[k].is_some() {
                out.push(MenuEntry::item("Remove jog", move |t, _, cx| t.edit_sheet("Remove jog", cx, |s| noshape_scene::drawing_tools::annotation_edit::remove_jog(s, id, k))));
            } else {
                let f = self.dimension_frame(&a)?;
                let (jog, grow) = noshape_scene::drawing_tools::annotation_edit::jog_for(&f, k);
                out.push(MenuEntry::item("Add jog", move |t, _, cx| t.edit_sheet("Add jog", cx, |s| noshape_scene::drawing_tools::annotation_edit::add_jog(s, id, k, jog, grow))));
            }
        }
        out.push(MenuEntry::Separator);
        out.push(MenuEntry::item("Clear selection", |t, _, cx| {
            t.selection.clear();
            t.redraw(cx);
        }));
        out.push(MenuEntry::item("Zoom to fit", |t, _, cx| t.zoom_to_fit(cx)));
        out.push(MenuEntry::item("Delete", move |t, _, cx| {
            t.selection.clear();
            t.edit_sheet("Delete annotation", cx, |s| s.annotations.retain(|x| x.id != id));
        }));
        Some(out)
    }

    /// An ordinate dimension's menu (live behaviours.md "Ordinate dimensions"):
    /// Add to ordinate dimension group · Edit... · Paste · Paste as table ·
    /// Reset ordinate · Unbreak dimension | Clear selection · Zoom to fit ·
    /// Delete. (Add comment is a cloud feature.)
    pub fn ordinate_entries(&self, id: u64, cx: &gpui_kit::App) -> Option<Vec<MenuEntry>> {
        let a = self.resolved_annotation(id)?;
        let AnnotationKind::OrdinateGroup { view, origin, horizontal, .. } = a.kind else { return None };
        let r = a.refs.iter().find(|x| x.slot == 0).map(|x| x.point.clone());
        let mut out = vec![MenuEntry::item("Add to ordinate dimension group", move |t, _, cx| {
            let o = PickedPoint { view, p: origin, r: r.clone() };
            t.set_tool(Tool::Ordinate { origin: Some(o), horizontal: Some(horizontal), group: Some(id) }, cx)
        })];
        let a2 = a.clone();
        out.push(MenuEntry::item("Edit...", move |t, w, cx| t.open_annotation_panel(&a2, w, cx)));
        out.extend(crate::drawing_props::paste_entry(cx));
        out.push(MenuEntry::item("Paste as table", |t, _, cx| {
            t.start_paste_table(cx);
        }));
        // The values sit on the group's text line (no moved values to reset).
        out.push(MenuEntry::item("Reset ordinate", |_, _, _| {}));
        let unbroken = a.unbroken;
        out.push(MenuEntry::item(if unbroken { "Break dimension" } else { "Unbreak dimension" }, move |t, _, cx| {
            t.edit_sheet(if unbroken { "Break dimension" } else { "Unbreak dimension" }, cx, |s| s.annotations.iter_mut().filter(|x| x.id == id).for_each(|x| x.unbroken = !unbroken))
        }));
        out.push(MenuEntry::Separator);
        out.push(MenuEntry::item("Clear selection", |t, _, cx| {
            t.selection.clear();
            t.redraw(cx);
        }));
        out.push(MenuEntry::item("Zoom to fit", |t, _, cx| t.zoom_to_fit(cx)));
        out.push(MenuEntry::item("Delete", move |t, _, cx| {
            t.selection.clear();
            t.edit_sheet("Delete annotation", cx, |s| s.annotations.retain(|x| x.id != id));
        }));
        Some(out)
    }

    /// A jog snap point of the selected dimension under `start`: (dimension,
    /// extension line, the far point).
    pub fn jog_grip_at(&self, start: [f64; 2]) -> Option<(Annotation, usize, bool)> {
        self.pk().jog_grip_at(start)
    }

    /// An arm-end grip of a selected centermark under `start`: (mark, arm).
    pub fn centermark_grip_at(&self, start: [f64; 2]) -> Option<(Annotation, usize)> {
        self.pk().centermark_grip_at(start)
    }

    /// The centermark `orig` with arm `k`'s end dragged to the cursor: all
    /// four arms change by the same amount, only that arm with Shift.
    pub fn dragged_centermark(&self, orig: &Annotation, k: usize, shift: bool) -> Annotation {
        self.pk().dragged_centermark(orig, k, shift)
    }

    /// An end grip of a selected centerline under `start`: (centerline, end).
    pub fn centerline_grip_at(&self, start: [f64; 2]) -> Option<(Annotation, usize)> {
        self.pk().centerline_grip_at(start)
    }

    /// The middle snap of a selected centerline under `start`.
    pub fn centerline_middle_at(&self, start: [f64; 2]) -> Option<Annotation> {
        self.pk().centerline_middle_at(start)
    }

    /// The centerline `orig` with end `k` dragged to the cursor along its line.
    pub fn dragged_centerline(&self, orig: &Annotation, k: usize) -> Annotation {
        self.pk().dragged_centerline(orig, k)
    }

    /// An attachment point (light-blue grip) of a selected linear dimension
    /// under `start`: (dimension, end 0 / 1).
    pub fn attach_grip_at(&self, start: [f64; 2]) -> Option<(Annotation, usize)> {
        self.pk().attach_grip_at(start)
    }

    /// The dimension `orig` with end `k` re-attached to the snap point under
    /// the cursor (same view), keeping the other end (help: "dragging any of
    /// its grip points to another edge, point or center re-attaches that end").
    pub fn reattached(&self, orig: &Annotation, k: usize) -> Annotation {
        self.pk().reattached(orig, k)
    }

    /// The dimension `orig` with jog `k`'s snap point (`far`: the end of
    /// the diagonal, else its start) dragged to the cursor.
    pub fn dragged_jog(&self, orig: &Annotation, k: usize, far: bool) -> Annotation {
        self.pk().dragged_jog(orig, k, far)
    }
}
