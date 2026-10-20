//! Draft analysis (Analysis tools menu): the dialog (Mold split direction
//! with its flip arrow, Minimum draft angle, Entities to check, Show
//! undercut regions; ✓ keeps the analysis on, ✕ turns it off), the color
//! legend at the top of the graphics area, an arrow pointing to side 1 and
//! the draft angle under the cursor (Onshape).

use super::PartStudioView;
use super::analysis::{Analysis, DRAFT_SIDE1, DRAFT_SIDE2, DRAFT_STEEP, DRAFT_UNDERCUT};
use crate::theme::p;
use glam::DVec3;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{Icon, IconName, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::reference::Reference;
use noshape_render::{Drawable, Instance, LineData, LineSegment};

pub struct DraftState {
    /// Active field: 0 mold split direction, 1 entities to check.
    pub active: u8,
    pub direction: Option<Reference>,
    pub flipped: bool,
    pub angle_deg: f64,
    pub parts: Vec<Reference>,
    pub undercut: bool,
    angle_input: Entity<InputState>,
    _sub: Subscription,
}

impl PartStudioView {
    pub(super) fn open_draft(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let st = cx.new(|cx| InputState::new(window, cx).default_value("3 deg"));
        let sub = cx.subscribe_in(&st, window, |this: &mut Self, st, ev: &InputEvent, window, cx| {
            if !matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                return;
            }
            let text = st.read(cx).value().to_string();
            match text.trim().trim_end_matches("deg").trim().parse::<f64>() {
                Ok(v) if (0.0..90.0).contains(&v) => {
                    if let Some(d) = &mut this.ui.draft {
                        d.angle_deg = v;
                    }
                    let shown = format!("{v} deg");
                    if text != shown {
                        st.update(cx, |s, cx| s.set_value(shown, window, cx));
                    }
                    this.sync_draft(cx);
                }
                _ => this.flash_info("Enter an angle from 0 to 90 degrees".into(), cx),
            }
        });
        // A preselected plane, face or edge is the mold split direction;
        // every part is checked.
        let direction = self.selection.iter().find(|r| self.frame_of_ref(r).is_some() || matches!(r, Reference::Edge(_))).cloned();
        let parts = self.display.bodies.iter().filter(|b| !b.hidden).map(|b| Reference::Body(b.id.clone())).collect();
        self.ui.draft = Some(DraftState { active: if direction.is_some() { 1 } else { 0 }, direction, flipped: false, angle_deg: 3.0, parts, undercut: true, angle_input: st, _sub: sub });
        self.sync_draft(cx);
    }

    /// The pull direction: the plane / face normal or the edge's direction.
    fn draft_pull(&self, d: &DraftState) -> Option<DVec3> {
        let r = d.direction.as_ref()?;
        let dir = match self.frame_of_ref(r) {
            Some(f) => f.normal,
            None => self.linear_edge_direction(r)?,
        };
        Some(if d.flipped { -dir } else { dir })
    }

    /// Show the dialog's settings on the parts.
    pub(super) fn sync_draft(&mut self, cx: &mut Context<Self>) {
        let analysis = self.ui.draft.as_ref().and_then(|d| {
            let pull = self.draft_pull(d)?;
            let parts = d.parts.iter().filter_map(|r| if let Reference::Body(b) = r { Some(b.clone()) } else { None }).collect();
            Some(Analysis::Draft { pull: pull.to_array(), angle_deg: d.angle_deg, undercut: d.undercut, parts })
        });
        self.ui.analysis = analysis;
        self.rebuild_scene(cx);
    }

    pub(super) fn draft_pick(&mut self, r: &Reference, cx: &mut Context<Self>) {
        let body = self.display.bodies.iter().find(|b| b.face_refs.contains(r) || b.edge_refs.contains(r) || b.vertex_refs.contains(r)).map(|b| Reference::Body(b.id.clone()));
        let usable = self.frame_of_ref(r).is_some() || self.linear_edge_direction(r).is_some();
        let Some(d) = &mut self.ui.draft else { return };
        match d.active {
            0 if usable => {
                d.direction = Some(r.clone());
                d.active = 1;
            }
            0 => return self.flash_info("Select a plane, a planar face or a straight edge".into(), cx),
            _ => {
                let Some(b) = body.or_else(|| matches!(r, Reference::Body(_)).then(|| r.clone())) else { return self.flash_info("Select parts".into(), cx) };
                match d.parts.iter().position(|x| *x == b) {
                    Some(i) => {
                        d.parts.remove(i);
                    }
                    None => d.parts.push(b),
                }
            }
        }
        self.sync_draft(cx);
    }

    /// The legend at the top of the graphics area.
    pub(super) fn render_draft_legend(&self) -> Option<AnyElement> {
        if !matches!(self.ui.analysis, Some(Analysis::Draft { .. })) {
            return None;
        }
        let pal = p();
        let swatch = |c: [u8; 4], label: &'static str| {
            h_flex().gap_2().child(div().w(px(28.)).h(px(20.)).bg(gpui_kit::Rgba { r: c[0] as f32 / 255.0, g: c[1] as f32 / 255.0, b: c[2] as f32 / 255.0, a: 1.0 })).child(div().text_size(px(14.)).text_color(pal.text).child(label))
        };
        Some(
            div()
                .absolute()
                .top(px(8.))
                .left_0()
                .right_0()
                .flex()
                .justify_center()
                .child(h_flex().gap(px(28.)).child(swatch(DRAFT_STEEP, "Steep")).child(swatch(DRAFT_UNDERCUT, "Undercut")).child(swatch(DRAFT_SIDE1, "Side 1")).child(swatch(DRAFT_SIDE2, "Side 2")))
                .into_any_element(),
        )
    }

    /// The draft angle of the face under the cursor, beside the cursor.
    pub(super) fn render_draft_hover(&self, cx: &App) -> Option<AnyElement> {
        let Some(Analysis::Draft { pull, .. }) = &self.ui.analysis else { return None };
        let at = self.ui.pointer?;
        let vp = self.viewport.read(cx);
        let (o, dir) = vp.ray(at);
        let hit = noshape_model::interaction::entities_along_ray(&self.display, o, dir, 0.0, &|_| false, 1).into_iter().next()?;
        let point = o + dir.normalize() * hit.depth;
        // Normal of the nearest vertex of that face.
        let n = self.display.bodies.iter().find_map(|b| {
            let fi = b.face_refs.iter().position(|x| *x == hit.reference)?;
            let &(first, count) = b.mesh.face_ranges.get(fi)?;
            let idx = b.mesh.indices.get(first as usize..(first + count) as usize)?;
            let p3 = |v: [f32; 3]| DVec3::new(v[0] as f64, v[1] as f64, v[2] as f64);
            let best = idx.iter().min_by(|a, b2| p3(b.mesh.positions[**a as usize]).distance(point).total_cmp(&p3(b.mesh.positions[**b2 as usize]).distance(point)))?;
            Some(p3(b.mesh.normals[*best as usize]).normalize_or_zero())
        })?;
        let angle = n.dot(DVec3::from_array(*pull).normalize_or(DVec3::Z)).clamp(-1.0, 1.0).asin().to_degrees();
        let pal = p();
        Some(
            div()
                .absolute()
                .left(px(at.x as f32 + 14.))
                .top(px(at.y as f32 + 14.))
                .px(px(4.))
                .py(px(1.))
                .bg(gpui_kit::rgb(0xffffff))
                .border_1()
                .border_color(gpui_kit::rgb(0xbdbdbd))
                .text_size(px(12.))
                .text_color(pal.text)
                .child(format!("{angle:.2}°"))
                .into_any_element(),
        )
    }

    /// The arrow pointing to side 1, beside the checked parts.
    pub(super) fn draft_arrow(&self) -> Vec<Instance> {
        let Some(d) = self.ui.draft.as_ref() else { return Vec::new() };
        let Some(pull) = self.draft_pull(d) else { return Vec::new() };
        let Some((lo, hi)) = self.fit_bounds() else { return Vec::new() };
        let (c, size) = ((lo + hi) * 0.5, (hi - lo).length());
        let start = c + pull * size * 0.55;
        let tip = start + pull * size * 0.25;
        let side = pull.any_orthonormal_vector() * size * 0.04;
        let f = |v: DVec3| [v.x as f32, v.y as f32, v.z as f32];
        let mut lines = LineData::default();
        lines.push_polyline(&[f(start), f(tip)], [120, 120, 120, 255], 0, 2.0, LineSegment::ON_TOP);
        lines.push_polyline(&[f(tip - pull * size * 0.06 + side), f(tip), f(tip - pull * size * 0.06 - side), f(tip - pull * size * 0.06 + side)], [120, 120, 120, 255], 0, 2.0, LineSegment::ON_TOP);
        vec![Instance::new(Drawable::Lines(self.gpu.uploader().upload_lines(&lines))).per_primitive_color().unpickable().ignore_clip().layer(35)]
    }

    /// The dialog body.
    pub(super) fn render_draft_body(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let d = self.ui.draft.as_ref()?;
        let pal = p();
        let labeler = self.labeler(cx);
        let field = |idx: u8, caption: &'static str, refs: Vec<Reference>, cx: &mut Context<Self>| {
            let active = d.active == idx;
            let empty = refs.is_empty();
            let rows: Vec<AnyElement> = refs
                .iter()
                .enumerate()
                .map(|(i, r)| {
                    let r2 = r.clone();
                    h_flex()
                        .id(SharedString::from(format!("dr-{idx}-{i}")))
                        .group("drrow")
                        .h(px(23.))
                        .px(px(6.))
                        .text_size(px(13.))
                        .child(div().flex_1().truncate().child(labeler(r)))
                        .child(
                            div()
                                .id(SharedString::from(format!("dr-{idx}-x-{i}")))
                                .opacity(0.)
                                .group_hover("drrow", |d| d.opacity(1.))
                                .cursor_pointer()
                                .child(Icon::new(IconName::Close).with_size(px(11.)).text_color(pal.text_muted))
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    cx.stop_propagation();
                                    if let Some(d) = &mut this.ui.draft {
                                        if d.direction.as_ref() == Some(&r2) {
                                            d.direction = None;
                                        }
                                        d.parts.retain(|x| *x != r2);
                                    }
                                    this.sync_draft(cx);
                                })),
                        )
                        .into_any_element()
                })
                .collect();
            v_flex()
                .id(SharedString::from(format!("dr-field-{idx}")))
                .flex_1()
                .min_h(px(28.))
                .rounded(px(2.))
                .border_1()
                .border_color(if active { pal.accent } else { gpui_kit::rgb(0xd7d7d7) })
                .bg(if active { gpui_kit::rgb(0xdef1fb) } else { gpui_kit::rgb(0xffffff) })
                .cursor_pointer()
                .child(div().px(px(6.)).when(empty, |d| d.py(px(6.)).text_size(px(12.))).when(!empty, |d| d.pt(px(2.)).text_size(px(10.))).text_color(pal.text_muted).child(caption))
                .children(rows)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if let Some(d) = &mut this.ui.draft {
                        d.active = idx;
                    }
                    this.refresh_scene(false, cx);
                    cx.notify();
                }))
        };
        let direction: Vec<Reference> = d.direction.iter().cloned().collect();
        let flipped = d.flipped;
        Some(
            v_flex()
                .p(px(4.))
                .gap(px(6.))
                .child(
                    h_flex()
                        .gap_1()
                        .child(field(0, "Mold split direction", direction, cx))
                        .child(
                            div()
                                .id("dr-flip")
                                .cursor_pointer()
                                .rounded(px(2.))
                                .when(flipped, |d| d.bg(gpui_kit::rgb(0xdef1fb)))
                                .child(crate::assets::icon("flip-direction", 20., cx))
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    if let Some(d) = &mut this.ui.draft {
                                        d.flipped = !d.flipped;
                                    }
                                    this.sync_draft(cx);
                                })),
                        ),
                )
                .child(h_flex().h(px(26.)).gap_2().text_size(px(12.)).child(div().text_color(pal.text_muted).child("Minimum draft angle")).child(gpui_kit::base::input::input_frame(div().flex_1().border_b_1().border_color(gpui_kit::rgb(0xd7d7d7)).child(Input::new(&d.angle_input).appearance(false).xsmall().text_right()))))
                .child(field(1, "Entities to check", d.parts.clone(), cx))
                .child(Checkbox::new("dr-undercut").label("Show undercut regions").checked(d.undercut).on_click(cx.listener(|this, v: &bool, _, cx| {
                    if let Some(d) = &mut this.ui.draft {
                        d.undercut = *v;
                    }
                    this.sync_draft(cx);
                })))
                .into_any_element(),
        )
    }
}
