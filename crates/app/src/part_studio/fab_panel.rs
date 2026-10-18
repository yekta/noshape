//! Fabrication panels: the sheet metal table (flat pattern preview, size,
//! bend table, DXF/SVG export) and the frame cut list.

use super::PartStudioView;
use crate::theme::{p, with_alpha};
use glam::DVec2;
use gpui_kit::component::button::Button;
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::*;
use noshape_model::reference::BodyId;
use noshape_model::regen::context::Context as RegenContext;
use noshape_model::sheetmetal::FlatPattern;
use std::sync::Arc;

/// Flat patterns of the sheet metal parts of one regeneration result.
pub(super) struct FlatCache {
    /// Identity of the regeneration context the patterns came from.
    context: usize,
    pub parts: Vec<(BodyId, Result<Arc<FlatPattern>, String>)>,
}

fn context_key(c: &Arc<RegenContext>) -> usize {
    Arc::as_ptr(c) as usize
}

impl PartStudioView {
    pub(super) fn has_sheet_metal(&self) -> bool {
        self.context.as_ref().is_some_and(|c| !c.sheet_metal.is_empty())
    }

    pub(super) fn has_frames(&self) -> bool {
        self.context.as_ref().is_some_and(|c| !c.frame_members.is_empty())
    }

    /// Unfold sheet metal parts in the background when the model changed.
    fn ensure_flats(&mut self, cx: &mut Context<Self>) {
        let Some(ctx) = self.context.clone() else { return };
        let key = context_key(&ctx);
        if self.flat_cache.as_ref().is_some_and(|c| c.context == key) || self.flat_task.as_ref().is_some_and(|(k, _)| *k == key) {
            return;
        }
        let task = cx.spawn(async move |this, cx| {
            let ctx2 = ctx.clone();
            let parts = cx
                .background_spawn(async move {
                    ctx2.sheet_metal.keys().map(|id| (id.clone(), noshape_model::sheetmetal::flat_pattern_for(&ctx2, id).map(Arc::new))).collect::<Vec<_>>()
                })
                .await;
            let _ = this.update(cx, |this: &mut Self, cx| {
                this.flat_cache = Some(FlatCache { context: key, parts });
                this.flat_task = None;
                cx.notify();
            });
        });
        self.flat_task = Some((key, task));
    }

    pub(super) fn render_sheet_metal_panel(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        self.ensure_flats(cx);
        let units = self.doc.read(cx).session.doc.settings.units;
        let len = |v: f64| noshape_units::format_quantity(noshape_units::Quantity::new(v, noshape_units::Dim::LENGTH), &units);
        let Some(cache) = self.flat_cache.as_ref().filter(|c| self.context.as_ref().is_some_and(|x| context_key(x) == c.context)) else {
            return div().text_xs().text_color(pal.text_faint).child(if self.has_sheet_metal() { "Unfolding…" } else { "No sheet metal parts." }).into_any_element();
        };
        let mut out: Vec<AnyElement> = Vec::new();
        for (id, flat) in &cache.parts {
            let name = self.display.body(id).map(|b| b.name.clone()).unwrap_or_else(|| "Part".into());
            out.push(div().pt_1().text_sm().font_weight(FontWeight::SEMIBOLD).child(name).into_any_element());
            let flat = match flat {
                Ok(f) => f.clone(),
                Err(e) => {
                    out.push(div().text_xs().text_color(pal.error).child(e.clone()).into_any_element());
                    continue;
                }
            };
            out.push(flat_preview(flat.clone()));
            let size = flat.size();
            let kv = |k: &str, v: String| h_flex().justify_between().text_xs().child(div().text_color(pal.text_muted).child(k.to_string())).child(div().text_color(pal.text).child(v)).into_any_element();
            out.push(kv("Flat size", format!("{} × {}", len(size.x), len(size.y))));
            out.push(kv("Thickness", len(flat.thickness)));
            out.push(kv("Area", format!("{:.0} mm²", flat.area() * 1e6)));
            out.push(kv("Bends", flat.bend_lines.len().to_string()));
            for w in &flat.warnings {
                out.push(div().text_xs().text_color(pal.warning).child(w.clone()).into_any_element());
            }
            if !flat.bend_lines.is_empty() {
                let header = h_flex().pt_1().text_xs().text_color(pal.text_faint).child(div().w(px(24.)).child("#")).child(div().w(px(52.)).child("Dir")).child(div().w(px(56.)).child("Angle")).child(div().w(px(64.)).child("Radius")).child(div().flex_1().child("Allow."));
                out.push(header.into_any_element());
                for (i, b) in flat.bend_lines.iter().enumerate() {
                    out.push(
                        h_flex()
                            .text_xs()
                            .text_color(pal.text)
                            .child(div().w(px(24.)).text_color(pal.text_muted).child(format!("{}", i + 1)))
                            .child(div().w(px(52.)).text_color(if b.up { pal.success } else { pal.info }).child(if b.up { "Up" } else { "Down" }))
                            .child(div().w(px(56.)).child(format!("{:.1}°", b.angle.to_degrees())))
                            .child(div().w(px(64.)).child(len(b.radius)))
                            .child(div().flex_1().child(len(b.allowance)))
                            .into_any_element(),
                    );
                }
            }
            let (b1, b2) = (id.clone(), id.clone());
            out.push(
                h_flex()
                    .gap_1()
                    .pt_1()
                    .child(Button::new(SharedString::from(format!("flat-dxf-{id}"))).label("Export DXF").xsmall().outline().on_click(cx.listener(move |this, _: &ClickEvent, w, cx| this.export_flat(b1.clone(), "dxf", w, cx))))
                    .child(Button::new(SharedString::from(format!("flat-svg-{id}"))).label("Export SVG").xsmall().outline().on_click(cx.listener(move |this, _: &ClickEvent, w, cx| this.export_flat(b2.clone(), "svg", w, cx))))
                    .into_any_element(),
            );
            out.push(div().h(px(1.)).my_1().bg(pal.border).into_any_element());
        }
        v_flex().gap_1().children(out).into_any_element()
    }

    pub(super) fn render_cut_list_panel(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let Some(ctx) = self.context.clone() else { return div().into_any_element() };
        let rows = noshape_model::regen::features::frames::cut_list(&ctx, None);
        if rows.is_empty() {
            return div().text_xs().text_color(pal.text_faint).child("No frame members. Create a Frame feature to get a cut list.").into_any_element();
        }
        let units = self.doc.read(cx).session.doc.settings.units;
        let len = |v: f64| noshape_units::format_quantity(noshape_units::Quantity::new(v, noshape_units::Dim::LENGTH), &units);
        let ang = |a: Option<f64>| a.map(|a| format!("{:.1}°", a.to_degrees())).unwrap_or_else(|| "—".into());
        let header = h_flex()
            .text_xs()
            .text_color(pal.text_faint)
            .child(div().w(px(22.)).child("#"))
            .child(div().w(px(28.)).child("Qty"))
            .child(div().flex_1().child("Profile"))
            .child(div().w(px(70.)).child("Length"))
            .child(div().w(px(80.)).child("Angles"));
        let mut out = vec![header.into_any_element()];
        let total: f64 = rows.iter().filter_map(|r| r.length.map(|l| l * r.qty as f64)).sum();
        for r in rows {
            let bodies = r.bodies.clone();
            out.push(
                h_flex()
                    .id(SharedString::from(format!("cut-{}", r.item)))
                    .py_0p5()
                    .rounded_sm()
                    .text_xs()
                    .text_color(pal.text)
                    .hover(|d| d.bg(pal.panel_hover))
                    .child(div().w(px(22.)).text_color(pal.text_muted).child(r.item.to_string()))
                    .child(div().w(px(28.)).child(r.qty.to_string()))
                    .child(div().flex_1().truncate().child(if r.description.is_empty() { r.standard.clone() } else { format!("{} {}", r.standard, r.description) }))
                    .child(div().w(px(70.)).child(r.length.map(len).unwrap_or_else(|| "—".into())))
                    .child(div().w(px(80.)).child(format!("{} / {}", ang(r.angle1), ang(r.angle2))))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        // Select the members of this row.
                        this.selection = bodies.iter().cloned().map(noshape_model::reference::Reference::Body).collect();
                        this.refresh_scene(false, cx);
                        cx.notify();
                    }))
                    .into_any_element(),
            );
        }
        out.push(h_flex().pt_1().justify_between().text_xs().child(div().text_color(pal.text_muted).child("Total length")).child(div().child(len(total))).into_any_element());
        v_flex().gap_px().children(out).into_any_element()
    }
}

/// Flat pattern drawing: outline loops solid, bend lines dashed.
fn flat_preview(flat: Arc<FlatPattern>) -> AnyElement {
    let pal = p();
    let (lo, hi) = flat.bounds();
    let size = (hi - lo).max(DVec2::splat(1e-9));
    let aspect = (size.y / size.x).clamp(0.2, 1.5);
    let outline = pal.text;
    let up = pal.success;
    let down = pal.info;
    div()
        .w_full()
        .h(px((296.0 * aspect) as f32))
        .rounded_md()
        .bg(with_alpha(pal.app_bg, 0.6))
        .child(
            canvas(
                |_, _, _| {},
                move |bounds, _, window, _| {
                    let pad = 10.0f32;
                    let w = f32::from(bounds.size.width) - 2.0 * pad;
                    let h = f32::from(bounds.size.height) - 2.0 * pad;
                    let s = (w as f64 / size.x).min(h as f64 / size.y);
                    let ox = f32::from(bounds.origin.x) + pad + (w - (size.x * s) as f32) * 0.5;
                    let oy = f32::from(bounds.origin.y) + pad + (h - (size.y * s) as f32) * 0.5;
                    // Flip Y: plane +Y is up on screen.
                    let pt = |q: DVec2| point(px(ox + ((q.x - lo.x) * s) as f32), px(oy + ((hi.y - q.y) * s) as f32));
                    for l in &flat.loops {
                        if l.len() < 2 {
                            continue;
                        }
                        let mut b = PathBuilder::stroke(px(1.25));
                        b.move_to(pt(l[0]));
                        for q in &l[1..] {
                            b.line_to(pt(*q));
                        }
                        b.close();
                        if let Ok(path) = b.build() {
                            window.paint_path(path, outline);
                        }
                    }
                    // Form outlines with center marks.
                    for f in &flat.forms {
                        for l in f.loops.iter().filter(|l| l.len() >= 2) {
                            let mut b = PathBuilder::stroke(px(1.));
                            b.move_to(pt(l[0]));
                            for q in &l[1..] {
                                b.line_to(pt(*q));
                            }
                            b.close();
                            if let Ok(path) = b.build() {
                                window.paint_path(path, outline.opacity(0.7));
                            }
                        }
                        let c = pt(f.center);
                        let mut b = PathBuilder::stroke(px(1.));
                        b.move_to(point(c.x - px(4.), c.y));
                        b.line_to(point(c.x + px(4.), c.y));
                        b.move_to(point(c.x, c.y - px(4.)));
                        b.line_to(point(c.x, c.y + px(4.)));
                        if let Ok(path) = b.build() {
                            window.paint_path(path, outline.opacity(0.7));
                        }
                    }
                    for bl in &flat.bend_lines {
                        let mut b = PathBuilder::stroke(px(1.)).dash_array(&[px(4.), px(3.)]);
                        b.move_to(pt(bl.a));
                        b.line_to(pt(bl.b));
                        if let Ok(path) = b.build() {
                            window.paint_path(path, if bl.up { up } else { down });
                        }
                    }
                },
            )
            .size_full(),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    /// Writes a sample document for manual UI checks:
    /// `cargo test -p noshape -- --ignored write_fabrication_sample`.
    #[test]
    #[ignore]
    fn write_fabrication_sample() {
        use noshape_model::document::{Document, TabContent};
        let mut doc = Document::new();
        if let Some(t) = doc.tabs.iter_mut().find(|t| matches!(t.content, TabContent::PartStudio(_))) {
            t.content = TabContent::PartStudio(noshape_model::demo_fab::fabrication_studio());
        }
        let path = std::env::temp_dir().join("noshape-fabrication.noshape");
        let _ = std::fs::remove_file(&path);
        crate::session::Session::create(&path, doc).expect("create sample");
    }
}
