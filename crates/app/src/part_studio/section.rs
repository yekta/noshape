//! Section view (Shift+X): up to three section planes from planes, planar
//! faces or mate connectors (the selection when the view opens, Front
//! otherwise), each with an offset and a flip; geometry is cut away where
//! every plane removes it. The state can be saved in named views.

use super::PartStudioView;
use crate::theme::{p, with_alpha};
use glam::DVec3;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{IconName, Sizable as _, h_flex, v_flex};
use gpui_kit::*;
use noshape_model::display::PlaneFrame;
use noshape_model::reference::Reference;

/// Most section planes the renderer cuts with.
pub const MAX_PLANES: usize = 3;

pub struct SectionPlane {
    id: u64,
    pub label: String,
    pub origin: DVec3,
    pub normal: DVec3,
    pub offset: f64,
    pub flipped: bool,
    input: Entity<InputState>,
    _sub: Subscription,
}

impl SectionPlane {
    /// Clip plane (normal, d): geometry with dot(normal, p) > d is removed.
    pub fn clip(&self) -> (DVec3, f64) {
        let n = if self.flipped { -self.normal } else { self.normal };
        (n, n.dot(self.origin + self.normal * self.offset))
    }
}

/// Saved form of a section plane in a named view: normal xyz and d.
pub fn encode(planes: &[SectionPlane]) -> Vec<[f64; 4]> {
    planes
        .iter()
        .map(|p| {
            let (n, d) = p.clip();
            [n.x, n.y, n.z, d]
        })
        .collect()
}

impl PartStudioView {
    /// Plane frame of a plane, planar face, sketch entity or mate connector.
    pub(super) fn frame_of_ref(&self, r: &Reference) -> Option<PlaneFrame> {
        match r {
            Reference::Plane(_) | Reference::FeaturePlane(_) => self.display.planes.iter().find(|p| &p.reference == r).map(|p| p.frame),
            Reference::SketchRegion { sketch, .. } | Reference::SketchEntity { sketch, .. } | Reference::Sketch(sketch) => self.display.sketches.iter().find(|s| s.feature == *sketch).map(|s| s.frame),
            Reference::MateConnector { .. } => self.display.connectors.iter().find(|c| &c.reference == r).map(|c| PlaneFrame { origin: c.origin, x: c.x, y: c.z.cross(c.x), normal: c.z }),
            Reference::Face(n) => self.context.as_ref()?.bodies.iter().find_map(|b| {
                let fi = *b.find_faces(n).first()?;
                let pl = b.shape.sub_shape(noshape_kernel::ShapeType::Face, fi).ok()?.face_plane().ok()??;
                let (normal, x) = (pl.unit_normal(), pl.unit_x());
                Some(PlaneFrame { origin: pl.origin, x, y: normal.cross(x), normal })
            }),
            _ => None,
        }
    }

    fn section_plane(&mut self, label: String, origin: DVec3, normal: DVec3, offset: f64, flipped: bool, window: &mut Window, cx: &mut Context<Self>) -> SectionPlane {
        let units = self.doc.read(cx).session.doc.settings.units;
        let text = noshape_units::format_quantity(noshape_units::Quantity::new(offset, noshape_units::Dim::LENGTH), &units);
        let input = cx.new(|cx| InputState::new(window, cx).default_value(text));
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let _sub = cx.subscribe_in(&input, window, move |this: &mut Self, st, ev: &InputEvent, window, cx| {
            if matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Change) {
                let text = st.read(cx).value().to_string();
                let units = this.doc.read(cx).session.doc.settings.units;
                if let Ok(q) = noshape_units::evaluate_as(&text, noshape_units::Dim::LENGTH, &units, &noshape_units::EmptyScope)
                    && let Some(p) = this.ui.section.iter_mut().find(|p| p.id == id)
                {
                    p.offset = q.value;
                    this.apply_section(window, cx);
                }
            }
        });
        SectionPlane { id, label, origin, normal, offset, flipped, input, _sub }
    }

    /// Push the planes into the scene options and rebuild.
    pub(super) fn apply_section(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.opts.section = self.ui.section.iter().map(|p| p.clip()).collect();
        let d = self.display.clone();
        self.apply_display(d, window, cx);
    }

    /// Planes for the given references (or the Front plane).
    fn planes_for(&mut self, refs: &[Reference], window: &mut Window, cx: &mut Context<Self>) -> Vec<SectionPlane> {
        let labeler = self.labeler(cx);
        let frames: Vec<(String, PlaneFrame)> = refs.iter().filter_map(|r| self.frame_of_ref(r).map(|f| (labeler(r), f))).collect();
        let frames = if frames.is_empty() { vec![("Front plane".to_string(), PlaneFrame::FRONT)] } else { frames };
        frames.into_iter().take(MAX_PLANES).map(|(l, f)| self.section_plane(l, f.origin, f.normal, 0.0, false, window, cx)).collect()
    }

    /// Shift+X / menu "Section view": on with the selected planes, faces or
    /// mate connectors (Front otherwise); off when already on.
    pub(super) fn toggle_section(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.ui.section.is_empty() {
            self.ui.section.clear();
        } else {
            let refs = self.selection.clone();
            self.ui.section = self.planes_for(&refs, window, cx);
        }
        self.apply_section(window, cx);
    }

    /// Context menu "Section view" on an entity.
    pub(super) fn section_through(&mut self, r: Reference, window: &mut Window, cx: &mut Context<Self>) {
        self.ui.section = self.planes_for(&[r], window, cx);
        self.apply_section(window, cx);
    }

    /// Add section planes from the selection (up to three in total).
    fn add_section_planes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let refs = self.selection.clone();
        if refs.iter().all(|r| self.frame_of_ref(r).is_none()) {
            self.flash_info("Select a plane, planar face or mate connector to add a section plane.".into(), cx);
            return;
        }
        let room = MAX_PLANES.saturating_sub(self.ui.section.len());
        if room == 0 {
            self.flash_info("Maximum number of section planes reached".into(), cx);
            return;
        }
        let mut new = self.planes_for(&refs, window, cx);
        new.truncate(room);
        self.ui.section.extend(new);
        self.apply_section(window, cx);
    }

    /// Restore saved planes (named views).
    pub(super) fn restore_section(&mut self, saved: &[[f64; 4]], window: &mut Window, cx: &mut Context<Self>) {
        self.ui.section.clear();
        for (i, s) in saved.iter().take(MAX_PLANES).enumerate() {
            let n = DVec3::new(s[0], s[1], s[2]);
            let plane = self.section_plane(format!("Section plane {}", i + 1), n * s[3], n, 0.0, false, window, cx);
            self.ui.section.push(plane);
        }
        self.apply_section(window, cx);
    }

    pub(super) fn render_section_panel(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.ui.section.is_empty() {
            return None;
        }
        let pal = p();
        let rows: Vec<AnyElement> = self
            .ui
            .section
            .iter()
            .enumerate()
            .map(|(i, pl)| {
                h_flex()
                    .gap_2()
                    .child(div().w(px(110.)).truncate().text_xs().text_color(pal.text).child(pl.label.clone()))
                    .child(div().text_xs().text_color(pal.text_muted).child("Offset"))
                    .child(div().w(px(84.)).child(Input::new(&pl.input).xsmall()))
                    .child(Button::new(SharedString::from(format!("section-flip-{i}"))).label("Flip").xsmall().ghost().on_click(cx.listener(move |this, _: &ClickEvent, w, cx| {
                        if let Some(p) = this.ui.section.get_mut(i) {
                            p.flipped = !p.flipped;
                        }
                        this.apply_section(w, cx);
                    })))
                    .child(Button::new(SharedString::from(format!("section-rm-{i}"))).icon(IconName::Close).xsmall().ghost().on_click(cx.listener(move |this, _: &ClickEvent, w, cx| {
                        if i < this.ui.section.len() {
                            this.ui.section.remove(i);
                        }
                        this.apply_section(w, cx);
                    })))
                    .into_any_element()
            })
            .collect();
        Some(
            v_flex()
                .id("section-panel")
                .occlude()
                .absolute()
                .top_3()
                .left(px(340.))
                .gap_1()
                .px_2()
                .py_1()
                .rounded_md()
                .bg(with_alpha(pal.panel_raised, 0.95))
                .border_1()
                .border_color(pal.accent)
                .child(
                    h_flex()
                        .gap_2()
                        .child(div().flex_1().text_xs().text_color(pal.accent_hover).child("SECTION VIEW"))
                        .child(Button::new("section-add").label("Add plane from selection").xsmall().ghost().on_click(cx.listener(|this, _: &ClickEvent, w, cx| this.add_section_planes(w, cx))))
                        .child(Button::new("section-off").label("Turn off").xsmall().ghost().on_click(cx.listener(|this, _: &ClickEvent, w, cx| this.toggle_section(w, cx)))),
                )
                .children(rows)
                .into_any_element(),
        )
    }
}
