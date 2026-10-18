//! Appearances (Onshape "Customizing appearances"): the Appearance editor
//! for parts (color, transparency, tessellation quality), faces, features
//! and sketches, and the Appearance panel listing every custom appearance.
//! Face and feature colors reach the renderer as per-vertex colors.

use super::PartStudioView;
use crate::theme::p;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::slider::{Slider, SliderEvent, SliderState};
use gpui_kit::component::{IconName, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use crate::os_menu::OsContextMenuExt as _;
use gpui_kit::*;
use noshape_model::BodyId;
use noshape_model::partstudio::{Appearance, AppearanceTarget, FaceAppearance, TessQuality};
use noshape_model::reference::{FeatureId, Reference};
use std::collections::HashMap;
use std::sync::Arc;

/// Per-body per-vertex sRGB colors handed to the scene builder.
pub type VertexColors = Arc<HashMap<BodyId, Arc<Vec<[u8; 4]>>>>;

/// Palette of the color picker (sRGB), Onshape-like hues in light/dark rows.
pub const PALETTE: &[u32] = &[
    0xffffff, 0xd9d9d9, 0xa6a6a6, 0x737373, 0x404040, 0x000000, 0x8ab5cf, 0x4e6fb7, //
    0xc8e6f5, 0x9a9a9a, 0xfd9c28, 0xebb115, 0xd9534f, 0xe67e22, 0xf1c40f, 0x2ecc71, //
    0x1abc9c, 0x3498db, 0x9b59b6, 0xe84393, 0x7f8c8d, 0x8e5a3c, 0x2c3e50, 0x16a085,
];

/// What the open Appearance editor edits.
#[derive(Clone, Debug, PartialEq)]
pub enum EditTarget {
    Parts(Vec<BodyId>),
    /// Faces (and the index of the appearance being edited, if any).
    Faces(Vec<Reference>, Option<usize>),
    /// A feature's faces or a sketch's curves.
    Feature(FeatureId, Option<usize>),
}

pub struct AppearanceEdit {
    pub target: EditTarget,
    /// sRGB.
    pub rgb: [u8; 3],
    /// 0 = opaque, 1 = invisible (Onshape "Part transparency").
    pub transparency: f32,
    pub tessellation: TessQuality,
    hex: Entity<InputState>,
    slider: Entity<SliderState>,
    _subs: Vec<Subscription>,
}

/// sRGB 0..255 to linear 0..1.
pub fn srgb_to_lin(c: u8) -> f32 {
    let c = c as f32 / 255.0;
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

/// Linear 0..1 to sRGB 0..255.
pub fn lin_to_srgb(c: f32) -> u8 {
    let c = c.clamp(0.0, 1.0);
    let s = if c <= 0.0031308 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
    (s * 255.0).round() as u8
}

/// Parse `#rrggbb` / `rrggbb` / `rgb`.
pub fn parse_hex(text: &str) -> Option<[u8; 3]> {
    let t = text.trim().trim_start_matches('#');
    let full: String = match t.len() {
        3 => t.chars().flat_map(|c| [c, c]).collect(),
        6 => t.to_string(),
        _ => return None,
    };
    let v = u32::from_str_radix(&full, 16).ok()?;
    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

pub fn hex_of(rgb: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2])
}

/// Per-vertex sRGB colors of a body mesh: faces with a custom appearance
/// get its color, others alpha 0 (the part color).
pub fn face_colors(body: &noshape_model::display::BodyDisplay, appearances: &[FaceAppearance]) -> Option<Vec<[u8; 4]>> {
    let mut colors: Option<Vec<[u8; 4]>> = None;
    for a in appearances {
        let c = [lin_to_srgb(a.color[0]), lin_to_srgb(a.color[1]), lin_to_srgb(a.color[2]), 255];
        for (fi, r) in body.face_refs.iter().enumerate() {
            let hit = match &a.target {
                AppearanceTarget::Faces(faces) => faces.contains(r),
                AppearanceTarget::Feature(f) => r.depends_on() == Some(*f),
            };
            if !hit {
                continue;
            }
            let Some(&(first, count)) = body.mesh.face_ranges.get(fi) else { continue };
            let v = colors.get_or_insert_with(|| vec![[0; 4]; body.mesh.positions.len()]);
            for &i in &body.mesh.indices[first as usize..(first + count) as usize] {
                if let Some(slot) = v.get_mut(i as usize) {
                    *slot = c;
                }
            }
        }
    }
    colors
}

impl PartStudioView {
    /// Per-vertex colors for the scene: an active analysis map, otherwise
    /// face/feature appearances. Cached per display and inputs.
    pub(super) fn vertex_colors(&mut self, cx: &App) -> Option<VertexColors> {
        let appearances = self.studio(cx).map(|s| s.appearances.clone()).unwrap_or_default();
        let key = (Arc::as_ptr(&self.display) as usize, format!("{:?}{:?}", appearances, self.ui.analysis));
        if let Some((k, v)) = &self.ui.vcolor_cache
            && *k == key
        {
            return v.clone();
        }
        let mut map: HashMap<BodyId, Arc<Vec<[u8; 4]>>> = HashMap::new();
        for b in self.display.bodies.iter().filter(|b| !b.hidden) {
            let colors = match &self.ui.analysis {
                Some(a) => Some(super::analysis::analysis_colors(b, a)),
                None => face_colors(b, &appearances),
            };
            if let Some(c) = colors {
                map.insert(b.id.clone(), Arc::new(c));
            }
        }
        let v = (!map.is_empty()).then(|| Arc::new(map));
        self.ui.vcolor_cache = Some((key, v.clone()));
        v
    }

    /// Open the Appearance editor.
    pub(super) fn edit_appearance(&mut self, target: EditTarget, window: &mut Window, cx: &mut Context<Self>) {
        let studio = self.studio(cx).cloned();
        let (color, tess) = match (&target, &studio) {
            (EditTarget::Parts(b), Some(s)) => {
                let first = b.first();
                let props = first.and_then(|b| s.part_properties.get(b));
                let c = props.and_then(|p| p.appearance).map(|a| a.color).or_else(|| first.and_then(|b| self.display.body(b)).map(|b| b.color)).unwrap_or([0.7, 0.7, 0.7, 1.0]);
                (c, props.and_then(|p| p.tessellation).unwrap_or(TessQuality::Auto))
            }
            (EditTarget::Faces(_, Some(i)) | EditTarget::Feature(_, Some(i)), Some(s)) => (s.appearances.get(*i).map(|a| a.color).unwrap_or([0.8, 0.3, 0.2, 1.0]), TessQuality::Auto),
            _ => ([0.831, 0.44, 0.007, 1.0], TessQuality::Auto),
        };
        let rgb = [lin_to_srgb(color[0]), lin_to_srgb(color[1]), lin_to_srgb(color[2])];
        let transparency = 1.0 - color[3].clamp(0.0, 1.0);
        let hex = cx.new(|cx| InputState::new(window, cx).default_value(hex_of(rgb)));
        let slider = cx.new(|_| SliderState::new().min(0.0).max(1.0).step(0.01).default_value(transparency));
        let s1 = cx.subscribe_in(&hex, window, |this: &mut Self, st, ev: &InputEvent, _, cx| {
            if matches!(ev, InputEvent::Change | InputEvent::PressEnter { .. })
                && let Some(rgb) = parse_hex(&st.read(cx).value())
                && let Some(e) = &mut this.ui.appearance_edit
            {
                e.rgb = rgb;
                cx.notify();
            }
        });
        let s2 = cx.subscribe_in(&slider, window, |this: &mut Self, _, ev: &SliderEvent, _, cx| {
            let (SliderEvent::Change(v) | SliderEvent::Release(v)) = ev;
            if let Some(e) = &mut this.ui.appearance_edit {
                e.transparency = v.start();
                cx.notify();
            }
        });
        self.ui.appearance_edit = Some(AppearanceEdit { target, rgb, transparency, tessellation: tess, hex, slider, _subs: vec![s1, s2] });
        cx.notify();
    }

    fn set_edit_color(&mut self, rgb: [u8; 3], window: &mut Window, cx: &mut Context<Self>) {
        let Some(e) = &mut self.ui.appearance_edit else { return };
        e.rgb = rgb;
        let hex = e.hex.clone();
        hex.update(cx, |s, cx| s.set_value(hex_of(rgb), window, cx));
        cx.notify();
    }

    /// Apply the editor (✓).
    fn accept_appearance(&mut self, cx: &mut Context<Self>) {
        let Some(e) = self.ui.appearance_edit.take() else { return };
        let color = [srgb_to_lin(e.rgb[0]), srgb_to_lin(e.rgb[1]), srgb_to_lin(e.rgb[2]), 1.0 - e.transparency];
        let tess = e.tessellation;
        self.doc.update(cx, |doc, cx| {
            doc.edit_studio(self.tab, "Edit appearance", cx, |ps| match &e.target {
                EditTarget::Parts(bodies) => {
                    for b in bodies {
                        let p = ps.part_properties.entry(b.clone()).or_default();
                        p.appearance = Some(Appearance { color });
                        p.tessellation = (tess != TessQuality::Auto).then_some(tess);
                    }
                }
                EditTarget::Faces(faces, index) => {
                    let a = FaceAppearance { target: AppearanceTarget::Faces(faces.clone()), color };
                    match index.and_then(|i| ps.appearances.get_mut(i)) {
                        Some(slot) => *slot = a,
                        None => ps.appearances.push(a),
                    }
                }
                EditTarget::Feature(f, index) => {
                    let a = FaceAppearance { target: AppearanceTarget::Feature(*f), color };
                    match index.or_else(|| ps.appearances.iter().position(|x| x.target == a.target)).and_then(|i| ps.appearances.get_mut(i)) {
                        Some(slot) => *slot = a,
                        None => ps.appearances.push(a),
                    }
                }
            });
        });
        cx.notify();
    }

    fn remove_appearance(&mut self, index: usize, cx: &mut Context<Self>) {
        self.doc.update(cx, |doc, cx| {
            doc.edit_studio(self.tab, "Remove appearance", cx, |ps| {
                if index < ps.appearances.len() {
                    ps.appearances.remove(index);
                }
            });
        });
    }

    fn reset_part_appearance(&mut self, b: BodyId, cx: &mut Context<Self>) {
        self.edit_part_appearance(&b, cx, |p| {
            p.appearance = None;
            p.tessellation = None;
        });
    }

    fn edit_part_appearance(&mut self, b: &BodyId, cx: &mut Context<Self>, f: impl FnOnce(&mut noshape_model::partstudio::PartProperties)) {
        let b = b.clone();
        self.doc.update(cx, |doc, cx| {
            doc.edit_studio(self.tab, "Edit appearance", cx, |ps| f(ps.part_properties.entry(b).or_default()));
        });
    }

    /// Faces of the selection (for "Add appearance").
    pub(super) fn selected_faces(&self) -> Vec<Reference> {
        self.selection.iter().filter(|r| matches!(r, Reference::Face(_))).cloned().collect()
    }

    /// The floating Appearance editor.
    pub(super) fn render_appearance_editor(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let e = self.ui.appearance_edit.as_ref()?;
        let pal = p();
        let names = self.labeler(cx);
        let (title, subject, is_part) = match &e.target {
            EditTarget::Parts(b) => ("Appearance", b.iter().filter_map(|b| self.display.body(b).map(|d| d.name.clone())).collect::<Vec<_>>().join(", "), true),
            EditTarget::Faces(f, _) => ("Face appearance", format!("{} face{}", f.len(), if f.len() == 1 { "" } else { "s" }), false),
            EditTarget::Feature(f, _) => ("Appearance", names(&Reference::Feature(*f)), false),
        };
        let is_faces = matches!(e.target, EditTarget::Faces(..));
        let rgb = e.rgb;
        let swatch = |hex: u32, cx: &mut Context<Self>| {
            let c = [(hex >> 16) as u8, (hex >> 8) as u8, hex as u8];
            let on = c == rgb;
            div()
                .id(SharedString::from(format!("ap-sw-{hex:06x}")))
                .size(px(20.))
                .rounded_sm()
                .bg(rgb_u32(hex))
                .border_1()
                .border_color(if on { pal.accent } else { pal.border })
                .when(on, |d| d.border_2())
                .cursor_pointer()
                .on_click(cx.listener(move |this, _: &ClickEvent, w, cx| this.set_edit_color(c, w, cx)))
        };
        let swatches: Vec<AnyElement> = PALETTE.iter().map(|h| swatch(*h, cx).into_any_element()).collect();
        let channel = |i: usize, label: &'static str, cx: &mut Context<Self>| {
            let v = rgb[i];
            h_flex()
                .gap_1()
                .child(div().w(px(12.)).text_xs().text_color(pal.text_muted).child(label))
                .child(Button::new(SharedString::from(format!("ap-{label}-m"))).label("−").xsmall().ghost().on_click(cx.listener(move |this, ev: &ClickEvent, w, cx| {
                    let step = if ev.modifiers().shift { 16 } else { 1 };
                    let mut c = rgb;
                    c[i] = c[i].saturating_sub(step);
                    this.set_edit_color(c, w, cx);
                })))
                .child(div().w(px(26.)).text_center().text_xs().text_color(pal.text).child(v.to_string()))
                .child(Button::new(SharedString::from(format!("ap-{label}-p"))).label("+").xsmall().ghost().on_click(cx.listener(move |this, ev: &ClickEvent, w, cx| {
                    let step = if ev.modifiers().shift { 16 } else { 1 };
                    let mut c = rgb;
                    c[i] = c[i].saturating_add(step);
                    this.set_edit_color(c, w, cx);
                })))
        };
        let tess = e.tessellation;
        let view = cx.entity();
        let tess_menu = Button::new("ap-tess").label(tess.label()).outline().xsmall().dropdown_caret(true).dropdown_menu(move |mut m, _, _| {
            for q in TessQuality::ALL {
                let v = view.clone();
                m = m.item(PopupMenuItem::new(q.label()).checked(q == tess).on_click(move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        if let Some(e) = &mut this.ui.appearance_edit {
                            e.tessellation = q;
                        }
                        cx.notify();
                    })
                }));
            }
            m
        });
        Some(
            v_flex()
                .id("appearance-editor")
                .occlude()
                .absolute()
                .top_3()
                .left(px(340.))
                .w(px(250.))
                .p_2()
                .gap_2()
                .rounded_md()
                .bg(pal.panel_raised)
                .border_1()
                .border_color(pal.border_strong)
                .shadow_lg()
                .child(
                    h_flex()
                        .gap_2()
                        .child(div().flex_1().text_sm().font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child(title))
                        .child(Button::new("ap-ok").icon(IconName::Check).xsmall().ghost().tooltip("Accept").on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.accept_appearance(cx))))
                        .child(Button::new("ap-cancel").icon(IconName::Close).xsmall().ghost().tooltip("Cancel").on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.ui.appearance_edit = None;
                            cx.notify();
                        }))),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .child(div().flex_1().truncate().text_xs().text_color(pal.text_muted).child(subject))
                        .when(is_faces, |d| {
                            d.child(Button::new("ap-use-sel").label("Use selection").xsmall().ghost().on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                let faces = this.selected_faces();
                                if let Some(EditTarget::Faces(f, _)) = this.ui.appearance_edit.as_mut().map(|e| &mut e.target)
                                    && !faces.is_empty()
                                {
                                    *f = faces;
                                }
                                cx.notify();
                            })))
                        }),
                )
                .child(h_flex().flex_wrap().gap_1().children(swatches))
                .child(
                    h_flex()
                        .gap_2()
                        .child(div().size(px(28.)).rounded_sm().border_1().border_color(pal.border).bg(gpui_kit::Rgba { r: rgb[0] as f32 / 255.0, g: rgb[1] as f32 / 255.0, b: rgb[2] as f32 / 255.0, a: 1.0 }))
                        .child(div().text_xs().text_color(pal.text_muted).child("#"))
                        .child(div().flex_1().child(Input::new(&e.hex).xsmall())),
                )
                .child(v_flex().gap_0p5().child(channel(0, "R", cx)).child(channel(1, "G", cx)).child(channel(2, "B", cx)))
                .when(is_part, |d| {
                    d.child(
                        v_flex()
                            .gap_1()
                            .child(h_flex().justify_between().child(div().text_xs().text_color(pal.text_muted).child("Part transparency")).child(div().text_xs().text_color(pal.text).child(format!("{:.2}", e.transparency))))
                            .child(Slider::new(&e.slider)),
                    )
                    .child(h_flex().gap_2().child(div().text_xs().text_color(pal.text_muted).child("Tessellation quality")).child(tess_menu))
                })
                .into_any_element(),
        )
    }

    /// Appearance panel (right side): every custom appearance.
    /// Onshape's Appearances panel (live, dom/panel-appearance.txt): tabs
    /// "Parts and surfaces | Sketches and curves", then one row per part or
    /// sketch: the name in bold 14 px on `#eaeaea` beside its swatch bar
    /// (half the width each, 29 px, 1 px `#aaa`). Face and feature
    /// appearances follow their part. Double-click a row to edit it.
    pub(super) fn render_appearance_panel(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let Some(studio) = self.studio(cx).cloned() else { return div().into_any_element() };
        let names = self.labeler(cx);
        let sketches_tab = self.ui.appearance_sketches;
        let grid = rgb_u32(0xaaaaaa);
        let swatch = |color: [f32; 4]| {
            let c = gpui_kit::Rgba { r: lin_to_srgb(color[0]) as f32 / 255.0, g: lin_to_srgb(color[1]) as f32 / 255.0, b: lin_to_srgb(color[2]) as f32 / 255.0, a: color[3].clamp(0.0, 1.0) };
            div()
                .flex_1()
                .h(px(29.))
                .p(px(3.))
                .bg(rgb_u32(0xeaeaea))
                .border_t_1()
                .border_r_1()
                .border_b_1()
                .border_color(grid)
                .child(
                    // Translucent colors show over a checkerboard (live `checkerboard-background`).
                    div().size_full().rounded(px(4.)).border_1().border_color(grid).bg(rgb_u32(0xffffff)).child(div().size_full().rounded(px(3.)).bg(c)).when(c.a < 0.99, |d| {
                        d.child(
                            canvas(
                                |_, _, _| {},
                                |b, _, window, _| {
                                    let cell = px(5.);
                                    let (cols, rows) = ((b.size.width / cell).ceil() as i32, (b.size.height / cell).ceil() as i32);
                                    for r in 0..rows {
                                        for c in (r % 2..cols).step_by(2) {
                                            let o = point(b.origin.x + cell * c as f32, b.origin.y + cell * r as f32);
                                            window.paint_quad(fill(Bounds::new(o, size(cell, cell)).intersect(&b), rgb_u32(0xd7d7d7)));
                                        }
                                    }
                                },
                            )
                            .absolute()
                            .size_full(),
                        )
                    }),
                )
        };
        let row = |id: SharedString, color: [f32; 4], label: String, indent: bool| {
            h_flex()
                .id(id)
                .w_full()
                .cursor_pointer()
                .child(
                    div()
                        .flex_1()
                        .h(px(29.))
                        .flex()
                        .items_center()
                        .pl(px(if indent { 18. } else { 4. }))
                        .bg(rgb_u32(0xeaeaea))
                        .border_1()
                        .border_color(grid)
                        .text_size(px(14.))
                        .when(!indent, |d| d.font_weight(FontWeight::BOLD))
                        .text_color(pal.text)
                        .truncate()
                        .child(label),
                )
                .child(swatch(color))
        };
        let mut rows: Vec<AnyElement> = Vec::new();
        if !sketches_tab {
            for body in self.display.bodies.iter().filter(|b| b.kind != noshape_model::display::BodyKind::Wire) {
                let props = studio.part_properties.get(&body.id);
                let color = props.and_then(|p| p.appearance).map(|a| a.color).unwrap_or(body.color);
                let (b1, b2) = (body.id.clone(), body.id.clone());
                let custom = props.is_some_and(|p| p.appearance.is_some());
                let view = cx.entity();
                rows.push(
                    row(SharedString::from(format!("ap-part-{}", body.id)), color, body.name.clone(), false)
                        .on_click(cx.listener(move |this, ev: &ClickEvent, w, cx| {
                            if ev.click_count() >= 2 {
                                this.edit_appearance(EditTarget::Parts(vec![b1.clone()]), w, cx);
                            }
                        }))
                        .when(custom, |d| {
                            d.os_context_menu(move |m, _, _| {
                                let v = view.clone();
                                let b = b2.clone();
                                m.item(crate::os_menu::OsMenuItem::new("Remove appearance").on_click(move |_, _, cx| v.update(cx, |this, cx| this.reset_part_appearance(b.clone(), cx))))
                            })
                        })
                        .into_any_element(),
                );
            }
        }
        for (i, a) in studio.appearances.iter().enumerate() {
            let is_sketch = matches!(&a.target, AppearanceTarget::Feature(f) if self.display.sketches.iter().any(|s| s.feature == *f));
            if is_sketch != sketches_tab {
                continue;
            }
            let (label, target) = match &a.target {
                AppearanceTarget::Faces(f) => (format!("{} face{}", f.len(), if f.len() == 1 { "" } else { "s" }), EditTarget::Faces(f.clone(), Some(i))),
                AppearanceTarget::Feature(f) => (names(&Reference::Feature(*f)), EditTarget::Feature(*f, Some(i))),
            };
            let faces = match &a.target {
                AppearanceTarget::Faces(f) => f.clone(),
                AppearanceTarget::Feature(_) => Vec::new(),
            };
            let view = cx.entity();
            rows.push(
                row(SharedString::from(format!("ap-face-{i}")), a.color, label, !is_sketch)
                    .on_hover(cx.listener(move |this, h: &bool, _, cx| {
                        // Hovering a face appearance highlights its faces.
                        this.ui.appearance_hover = if *h { faces.clone() } else { Vec::new() };
                        this.refresh_scene(false, cx);
                    }))
                    .on_click(cx.listener(move |this, ev: &ClickEvent, w, cx| {
                        if ev.click_count() >= 2 {
                            this.edit_appearance(target.clone(), w, cx);
                        }
                    }))
                    .os_context_menu(move |m, _, _| {
                        let v = view.clone();
                        m.item(crate::os_menu::OsMenuItem::new("Remove appearance").on_click(move |_, _, cx| v.update(cx, |this, cx| this.remove_appearance(i, cx))))
                    })
                    .into_any_element(),
            );
        }
        if sketches_tab {
            // Sketches without their own appearance, in the inactive sketch color.
            let with_own: Vec<FeatureId> = studio.appearances.iter().filter_map(|a| if let AppearanceTarget::Feature(f) = a.target { Some(f) } else { None }).collect();
            for sk in self.display.sketches.iter().filter(|s| !with_own.contains(&s.feature)) {
                let f = sk.feature;
                let color = sk.color.unwrap_or_else(|| crate::theme::lin(crate::theme::viewport::INACTIVE));
                rows.push(
                    row(SharedString::from(format!("ap-sk-{f}")), color, sk.name.clone(), false)
                        .on_click(cx.listener(move |this, ev: &ClickEvent, w, cx| {
                            if ev.click_count() >= 2 {
                                this.edit_appearance(EditTarget::Feature(f, None), w, cx);
                            }
                        }))
                        .into_any_element(),
                );
            }
        }
        let tab = |id: &'static str, label: &'static str, on: bool, sketches: bool, cx: &mut Context<Self>| {
            div()
                .id(id)
                .flex_1()
                .h(px(26.))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(13.33))
                .cursor_pointer()
                .when(on, |d| d.text_color(pal.accent).font_weight(FontWeight::MEDIUM).border_b_2().border_color(pal.accent))
                .when(!on, |d| d.text_color(pal.text).border_b_1().border_color(pal.border))
                .child(label)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.ui.appearance_sketches = sketches;
                    cx.notify();
                }))
        };
        v_flex()
            .child(h_flex().w_full().mt(px(-5.)).mx(px(-5.)).child(tab("ap-tab-parts", "Parts and surfaces", !sketches_tab, false, cx)).child(tab("ap-tab-sketches", "Sketches and curves", sketches_tab, true, cx)))
            .child(v_flex().pt(px(10.)).children(rows))
            .into_any_element()
    }

    /// The "Add face appearance" button at the right of the Appearances title.
    pub(super) fn render_add_face_appearance(&mut self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id("ap-add-face")
            .size(px(26.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(3.))
            .cursor_pointer()
            .hover(|d| d.bg(p().panel_hover))
            .child(crate::assets::icon("add-appearance", 20., cx))
            .tooltip(crate::ui::tooltip("Add face appearance"))
            .on_click(cx.listener(|this, _: &ClickEvent, w, cx| {
                let faces = this.selected_faces();
                this.edit_appearance(EditTarget::Faces(faces, None), w, cx);
            }))
            .into_any_element()
    }
}

fn rgb_u32(hex: u32) -> gpui_kit::Rgba {
    gpui_kit::rgb(hex)
}

#[cfg(test)]
mod tests {
    use super::*;
    // The glob import brings GPUI's `test` attribute; use the built-in one.
    use std::prelude::v1::test;

    #[test]
    fn hex_and_srgb_round_trip() {
        assert_eq!(parse_hex("#ff8000"), Some([255, 128, 0]));
        assert_eq!(parse_hex("f80"), Some([255, 136, 0]));
        assert_eq!(parse_hex("nope"), None);
        assert_eq!(hex_of([1, 2, 255]), "#0102ff");
        for c in [0u8, 17, 128, 200, 255] {
            assert_eq!(lin_to_srgb(srgb_to_lin(c)), c);
        }
    }
}
