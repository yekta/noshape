//! Drawing editor: Onshape's Reference manager (Sheets panel › a reference
//! row). It lists the drawing's references (Part Studios / Assemblies) with
//! their state; "Update all" updates the drawing, "Selective update" shows
//! checkboxes and "Update selected" regenerates the chosen ones only.

use crate::drawing_view::*;
use crate::theme::p;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::*;
use noshape_model::drawing::views::{self, SourceKey};

/// Reference manager state.
pub struct RefManager {
    /// Selective update: the checkboxes show.
    pub selective: bool,
    pub picked: Vec<SourceKey>,
}

impl DrawingEditor {
    pub fn open_ref_manager(&mut self, cx: &mut Context<Self>) {
        self.ref_manager = Some(RefManager { selective: false, picked: Vec::new() });
        cx.notify();
    }

    /// The drawing's references, one per referenced tab (its configurations
    /// and exploded states together): (keys, name, out of date).
    fn references(&self, cx: &App) -> Vec<(Vec<SourceKey>, String, bool)> {
        let doc = &self.doc.read(cx).session.doc;
        let mut out: Vec<(Vec<SourceKey>, String, bool)> = Vec::new();
        for k in views::referenced_sources(&self.drawing) {
            let stale = match (self.sources.get(&k), views::source_hash(doc, &k)) {
                (Some(s), Some(h)) => s.hash != h,
                _ => true,
            };
            match out.iter_mut().find(|(ks, _, _)| ks[0].tab == k.tab) {
                Some(e) => {
                    e.0.push(k);
                    e.2 |= stale;
                }
                None => {
                    let name = doc.tab(k.tab).map(|t| t.name.clone()).unwrap_or_else(|| "Missing reference".into());
                    out.push((vec![k], name, stale));
                }
            }
        }
        out
    }

    fn update_selected_refs(&mut self, cx: &mut Context<Self>) {
        let Some(m) = self.ref_manager.take() else { return };
        for k in &m.picked {
            self.sources.remove(k);
        }
        self.ensure_sources(false, cx);
        self.status = Some("Updating drawing…".into());
        cx.notify();
    }

    pub fn render_ref_manager(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let m = self.ref_manager.as_ref()?;
        let pal = p();
        let selective = m.selective;
        let picked = m.picked.clone();
        let mut rows = v_flex().p(px(8.)).gap(px(4.));
        for (i, (keys, name, stale)) in self.references(cx).into_iter().enumerate() {
            let state = h_flex()
                .gap(px(6.))
                .child(div().flex_1().truncate().child(name))
                .child(div().px(px(4.)).rounded(px(3.)).bg(if stale { gpui_kit::rgb(0xffe0a3) } else { gpui_kit::rgb(0x9ccef0) }).text_size(px(11.)).child(if stale { "Out of date" } else { "Up to date" }));
            rows = rows.child(if selective {
                let on = keys.iter().all(|k| picked.contains(k));
                h_flex()
                    .gap(px(4.))
                    .child(Checkbox::new(("ref-pick", i)).small().checked(on).on_click(cx.listener(move |this, v: &bool, _, cx| {
                        if let Some(m) = &mut this.ref_manager {
                            m.picked.retain(|x| !keys.contains(x));
                            if *v {
                                m.picked.extend(keys.iter().cloned());
                            }
                        }
                        cx.notify();
                    })))
                    .child(state)
                    .into_any_element()
            } else {
                state.into_any_element()
            });
        }
        let buttons = if selective {
            h_flex()
                .gap(px(6.))
                .p(px(8.))
                .justify_end()
                .child(Button::new("ref-cancel").label("Cancel").ghost().small().on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.ref_manager = None;
                    cx.notify();
                })))
                .child(Button::new("ref-update-sel").label("Update selected").primary().small().on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.update_selected_refs(cx))))
        } else {
            h_flex()
                .gap(px(6.))
                .p(px(8.))
                .justify_end()
                .child(Button::new("ref-selective").label("Selective update").ghost().small().on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    if let Some(m) = &mut this.ref_manager {
                        m.selective = true;
                    }
                    cx.notify();
                })))
                .child(Button::new("ref-update-all").label("Update all").primary().small().on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.ref_manager = None;
                    this.update_drawing(cx);
                    cx.notify();
                })))
        };
        Some(
            div()
                .id("dr-ref-backdrop")
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(pal.modal_backdrop)
                .occlude()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    v_flex()
                        .w(px(380.))
                        .bg(pal.modal_bg)
                        .rounded(px(4.))
                        .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.25), offset: point(px(0.), px(6.)), blur_radius: px(18.), spread_radius: px(0.), inset: false }])
                        .text_color(pal.text)
                        .child(
                            h_flex()
                                .h(px(40.))
                                .px(px(10.))
                                .border_b_1()
                                .border_color(pal.border)
                                .child(div().flex_1().text_size(px(16.)).child("Reference manager"))
                                .child(Button::new("ref-close").ghost().child(crate::ui::glyph("close", 14., pal.text_muted)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.ref_manager = None;
                                    cx.notify();
                                }))),
                        )
                        .child(rows)
                        .child(buttons),
                )
                .into_any_element(),
        )
    }
}
