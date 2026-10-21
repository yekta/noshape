//! Interference detection in an assembly (Analysis tools ▸ Interference
//! detection…, and the instance / top row menus' Check interference…; live
//! assembly/735–736, dom/dialog-interference-assembly-*.txt).
//!
//! A 220 px dialog with only ✕: Instances to analyze (clicked instances are
//! added or removed), ☐ Include standard content, ☐ Show top level only and
//! the read-only Interferences list ("Part 1 <1> and Part 2 <1>", tooltip the
//! overlap's size). The analyzed parts are drawn see-through around the red
//! overlapping material.

use super::*;
use noshape_model::assembly::analysis::{InstanceInterference, instance_interferences};

/// The open Interference detection dialog.
pub(crate) struct InterferenceDialog {
    pub instances: Vec<InstanceId>,
    pub include_standard: bool,
    pub top_only: bool,
    pub rows: Vec<InstanceInterference>,
    /// The overlaps' meshes (world coordinates), drawn red.
    pub meshes: Vec<noshape_render::MeshHandle>,
    /// Interferences row under the pointer (its box is outlined).
    pub hover: Option<usize>,
    seq: u64,
}

impl AssemblyView {
    /// Open the dialog with these instances to analyze.
    pub(super) fn open_interference_with(&mut self, instances: Vec<InstanceId>, cx: &mut Context<Self>) {
        let seq = self.ui.interference.as_ref().map_or(0, |d| d.seq);
        // The dialog's picks are the selection (their list rows highlight).
        self.selected = instances.clone();
        self.ui.interference = Some(InterferenceDialog { instances, include_standard: false, top_only: false, rows: Vec::new(), meshes: Vec::new(), hover: None, seq });
        self.recompute_interference(cx);
        self.update_capture(cx);
        cx.notify();
    }

    /// Analysis tools › Interference detection…: the selected instances.
    pub(super) fn open_interference(&mut self, cx: &mut Context<Self>) {
        let tops: Vec<InstanceId> = self.selected.iter().copied().filter(|id| self.assembly(cx).is_some_and(|a| a.instance(*id).is_some())).collect();
        self.open_interference_with(tops, cx);
    }

    pub(super) fn close_interference(&mut self, cx: &mut Context<Self>) {
        if self.ui.interference.take().is_some() {
            self.selected.clear();
            self.update_capture(cx);
            self.rebuild_scene(false, cx);
            cx.notify();
        }
    }

    /// A click on an instance while the dialog is open adds or removes it.
    pub(super) fn toggle_interference_instance(&mut self, id: InstanceId, cx: &mut Context<Self>) {
        let Some(d) = &mut self.ui.interference else { return };
        match d.instances.iter().position(|x| *x == id) {
            Some(i) => {
                d.instances.remove(i);
            }
            None => d.instances.push(id),
        }
        self.selected = d.instances.clone();
        self.recompute_interference(cx);
        cx.notify();
    }

    /// Recompute the overlaps (background) for the current instances.
    fn recompute_interference(&mut self, cx: &mut Context<Self>) {
        let Some(d) = &mut self.ui.interference else { return };
        d.seq += 1;
        let (seq, ids, include) = (d.seq, d.instances.clone(), d.include_standard);
        let result = self.result.clone();
        cx.spawn(async move |this, cx| {
            let rows = cx.background_spawn(async move { if ids.is_empty() { Vec::new() } else { instance_interferences(&result, &ids, include) } }).await;
            let _ = this.update(cx, |this: &mut Self, cx| {
                let up = this.gpu.uploader();
                let Some(d) = this.ui.interference.as_mut().filter(|d| d.seq == seq) else { return };
                d.meshes = rows.iter().map(|r| up.upload_mesh_parts(&r.overlap.positions, &r.overlap.normals, &vec![0u32; r.overlap.positions.len()], &r.overlap.indices)).collect();
                d.rows = rows;
                d.hover = None;
                this.rebuild_scene(false, cx);
                cx.notify();
            });
        })
        .detach();
    }

    /// Whether the analyzed parts are drawn see-through (overlaps found).
    pub(super) fn interference_shown(&self) -> bool {
        self.ui.interference.as_ref().is_some_and(|d| !d.rows.is_empty())
    }

    /// The red overlaps and the hovered row's box (scene highlights).
    pub(super) fn interference_highlights(&self, h: &mut assembly_scene::Highlights) {
        let Some(d) = &self.ui.interference else { return };
        h.red_meshes.extend(d.meshes.iter().cloned());
        if let Some(r) = d.hover.and_then(|k| d.rows.get(k)) {
            h.boxes.push(r.overlap.bounds);
        }
    }

    /// Interferences rows: (label, tooltip "w x h x d"), one per pair of
    /// top-level instances with Show top level only.
    fn interference_rows(&self, d: &InterferenceDialog, cx: &App) -> Vec<(usize, String, String)> {
        let units = self.units(cx);
        let len = |x: f64| noshape_units::format_quantity(noshape_units::Quantity::new(x, noshape_units::Dim::LENGTH), &units);
        let name = |p: &[InstanceId]| -> String {
            let p = if d.top_only { &p[..1] } else { p };
            self.result.node(p).map(|n| n.name.clone()).unwrap_or_default()
        };
        let mut out: Vec<(usize, String, String)> = Vec::new();
        let mut seen: Vec<(InstanceId, InstanceId)> = Vec::new();
        for (k, r) in d.rows.iter().enumerate() {
            if d.top_only {
                let pair = (r.a[0].min(r.b[0]), r.a[0].max(r.b[0]));
                if r.a[0] == r.b[0] || seen.contains(&pair) {
                    continue;
                }
                seen.push(pair);
            }
            let (lo, hi) = r.overlap.bounds;
            let size = hi - lo;
            out.push((k, format!("{} and {}", name(&r.a), name(&r.b)), format!("{} x {} x {}", len(size.x), len(size.y), len(size.z))));
        }
        out
    }

    /// The 220 px dialog (live dom/dialog-interference-assembly-*.txt).
    pub(super) fn render_interference_dialog(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let d = self.ui.interference.as_ref()?;
        let pal = p();
        let names: HashMap<InstanceId, String> = self.assembly(cx).map(|a| a.instances.iter().map(|i| (i.id, i.name.clone())).collect()).unwrap_or_default();
        let (include, top_only) = (d.include_standard, d.top_only);
        let entries: Vec<AnyElement> = d
            .instances
            .iter()
            .map(|id| {
                let id = *id;
                let n = names.get(&id).cloned().unwrap_or_default();
                crate::os_ui::query_row(SharedString::from(format!("aintf-i-{}", id.0)), n.clone())
                    .child(
                        div()
                            .id(SharedString::from(format!("aintf-x-{}", id.0)))
                            .px(px(5.))
                            .text_size(px(16.67))
                            .text_color(rgb(0x666666))
                            .cursor_pointer()
                            .tooltip(crate::ui::tooltip(format!("Delete {n}")))
                            .child("×")
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                cx.stop_propagation();
                                this.toggle_interference_instance(id, cx);
                            })),
                    )
                    .into_any_element()
            })
            .collect();
        let rows: Vec<AnyElement> = self
            .interference_rows(d, cx)
            .into_iter()
            .map(|(k, label, size)| {
                crate::os_ui::query_row(SharedString::from(format!("aintf-r-{k}")), label)
                    .text_color(rgb(0x666666))
                    .tooltip(crate::ui::tooltip(size))
                    .on_hover(cx.listener(move |this, on: &bool, _, cx| {
                        if let Some(d) = &mut this.ui.interference {
                            let new = on.then_some(k).or(d.hover.filter(|h| *h != k));
                            if d.hover != new {
                                d.hover = new;
                                this.refresh_scene(false, cx);
                            }
                        }
                    }))
                    .into_any_element()
            })
            .collect();
        // Include standard content changes what is analyzed; Show top level
        // only just how the rows are named.
        let check = |id: &'static str, label: &'static str, on: bool, f: fn(&mut InterferenceDialog), recompute: bool, cx: &mut Context<Self>| {
            crate::os_ui::checkbox(id, label, on).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                if let Some(d) = &mut this.ui.interference {
                    f(d);
                }
                if recompute {
                    this.recompute_interference(cx);
                }
                cx.notify();
            }))
        };
        Some(
            v_flex()
                .id("aintf-dialog")
                .occlude()
                .child(crate::ui::panel_probe("asm-interference"))
                .w(px(220.))
                .bg(pal.dialog_bg)
                .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
                .text_color(pal.text)
                .child(
                    h_flex()
                        .h(px(29.))
                        .pl(px(6.))
                        .bg(rgb(0xffffff))
                        .border_b_1()
                        .border_color(rgb(0xd7d7d7))
                        .child(crate::ui::panel_title("asm-interference", div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Interference detection")))
                        // Live: only ✕ (btn-close, 28 px, op .5).
                        .child(
                            div()
                                .id("aintf-close")
                                .size(px(28.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .opacity(0.5)
                                .cursor_pointer()
                                .child(crate::os_ui::glyph(crate::os_ui::GLYPH_CLOSE, 20., pal.text))
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_interference(cx))),
                        ),
                )
                .child(div().mx(px(3.)).mt(px(5.)).mb(px(5.)).child(crate::os_ui::query_box("aintf-instances", "Instances to analyze", true, entries)))
                .child(check("aintf-std", "Include standard content", include, |d| d.include_standard = !d.include_standard, true, cx))
                .child(check("aintf-top", "Show top level only", top_only, |d| d.top_only = !d.top_only, false, cx))
                .child(div().mx(px(3.)).mt(px(5.)).child(crate::os_ui::query_box("aintf-results", "Interferences", false, rows).cursor_default()))
                .child(h_flex().h(px(22.)).px(px(4.)).justify_end().child(super::props::help_dot()))
                .into_any_element(),
        )
    }
}
