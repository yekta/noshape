//! Section view (Shift+X, camera menu, context menus). Onshape (live,
//! part-studio/70–74): a "Section view" dialog at the feature dialog
//! position with Exclude | Include tabs, a "Section planes" field (planes,
//! planar faces, mate connectors; up to three here) and "Items to exclude"
//! (or include). Dragging the arrow on the active plane moves it, with the
//! offset shown in a box beside the arrow. ✓ / Enter keeps the section,
//! ✕ / Esc restores what was there before. Geometry is cut away where every
//! plane removes it; the state can be saved in named views.

use super::PartStudioView;
use crate::theme::p;
use glam::DVec3;
use gpui_kit::component::Sizable as _;
use gpui_kit::*;
use noshape_model::display::PlaneFrame;
use noshape_model::reference::{BodyId, Reference};

/// Most section planes the renderer cuts with.
pub const MAX_PLANES: usize = 3;

#[derive(Clone, Debug)]
pub struct SectionPlane {
    pub label: String,
    pub origin: DVec3,
    pub normal: DVec3,
    pub offset: f64,
    pub flipped: bool,
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

/// The open Section view dialog.
pub struct SectionDialog {
    /// Include tab: only the listed parts are cut (Exclude: they aren't).
    pub include: bool,
    /// Parts listed in "Items to exclude/include".
    pub items: Vec<BodyId>,
    /// The "Items" field takes picks (else "Section planes").
    pub items_active: bool,
    /// Plane the arrow drives.
    pub active: usize,
    /// State before the dialog opened (restored by Cancel).
    saved: (Vec<SectionPlane>, Option<(bool, Vec<BodyId>)>),
}

/// The section view dialog's prompt (live part-studio/70).
const SELECT_SECTION_PLANE: &str = "Select a plane, planar face, cylinder, cone, or mate connector.";

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

    /// Push the planes and the excluded parts into the scene and rebuild.
    pub(super) fn apply_section(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.opts.section = self.ui.section.iter().map(|p| p.clip()).collect();
        // Live part-studio/72-73: while the dialog is open its plane's cap is
        // drawn selected (tan); after ✓ in the part color (74).
        self.opts.section_selected = self.ui.section_dialog.as_ref().map(|d| d.active);
        self.opts.section_skip = match &self.ui.section_parts {
            Some((false, items)) => items.clone(),
            Some((true, items)) => self.display.bodies.iter().map(|b| b.id.clone()).filter(|b| !items.contains(b)).collect(),
            None => Vec::new(),
        };
        let d = self.display.clone();
        self.apply_display(d, window, cx);
    }

    /// Planes for the given references (or the Front plane).
    fn planes_for(&self, refs: &[Reference], cx: &App) -> Vec<SectionPlane> {
        let labeler = self.labeler(cx);
        let frames: Vec<(String, PlaneFrame)> = refs.iter().filter_map(|r| self.frame_of_ref(r).map(|f| (labeler(r), f))).collect();
        let frames = if frames.is_empty() { vec![("Front plane".to_string(), PlaneFrame::FRONT)] } else { frames };
        frames.into_iter().take(MAX_PLANES).map(|(l, f)| plane(&l, f.origin, f.normal)).collect()
    }

    /// Open the dialog (keeping the current planes, or starting from the
    /// selection / the Front plane).
    pub(super) fn open_section_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.ui.section_dialog.is_some() {
            return;
        }
        let saved = (self.ui.section.clone(), self.ui.section_parts.clone());
        // Live (part-studio/70): from the camera menu with nothing selected
        // the dialog opens empty and asks for a plane.
        if self.ui.section.is_empty() && !self.selection.is_empty() {
            let refs = self.selection.clone();
            self.ui.section = self.planes_for(&refs, cx);
        }
        if self.ui.section.is_empty() {
            self.message = Some((noshape_model::display::FeatureStatus::Info, SELECT_SECTION_PLANE.into()));
        }
        let (include, items) = self.ui.section_parts.clone().unwrap_or((false, Vec::new()));
        let active = self.ui.section.len().saturating_sub(1);
        self.ui.section_dialog = Some(SectionDialog { include, items, items_active: false, active, saved });
        self.apply_section(window, cx);
    }

    /// Shift+X: open the dialog; with a section on and no dialog, turn it off.
    pub(super) fn toggle_section(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Live (psb 10-21 f, qa4 F14): with a feature dialog open, Shift+X
        // does nothing (Section view is only offered once it closes).
        if self.edit.is_some() && self.ui.section_dialog.is_none() {
            return;
        }
        if self.ui.section_dialog.is_some() {
            self.close_section_dialog(false, window, cx);
        } else if !self.ui.section.is_empty() {
            self.ui.section.clear();
            self.ui.section_parts = None;
            self.apply_section(window, cx);
        } else {
            self.open_section_dialog(window, cx);
        }
    }

    /// Context menu "Section view…" on an entity.
    pub(super) fn section_through(&mut self, r: Reference, window: &mut Window, cx: &mut Context<Self>) {
        self.section_through_all(&[r], window, cx);
    }

    /// Section view… with several references (planes of the selected
    /// sketches and planes; the Front plane when none is planar).
    pub(super) fn section_through_all(&mut self, refs: &[Reference], window: &mut Window, cx: &mut Context<Self>) {
        let saved = (self.ui.section.clone(), self.ui.section_parts.clone());
        self.ui.section = self.planes_for(refs, cx);
        self.ui.section_dialog = None;
        self.open_section_dialog(window, cx);
        if let Some(d) = &mut self.ui.section_dialog {
            d.saved = saved;
        }
    }

    /// ✓ keeps the section, ✕ restores the state before the dialog.
    pub(super) fn close_section_dialog(&mut self, accept: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(d) = self.ui.section_dialog.take() else { return };
        self.ui.section_offset_edit = None;
        if self.message.as_ref().is_some_and(|(_, m)| m == SELECT_SECTION_PLANE) {
            self.message = None;
        }
        if accept {
            self.ui.section_parts = (!d.items.is_empty()).then_some((d.include, d.items));
        } else {
            (self.ui.section, self.ui.section_parts) = d.saved;
        }
        self.apply_section(window, cx);
    }

    /// A pick while the dialog is open: a plane for "Section planes" (again
    /// removes it), a part for "Items". Returns false when not consumed.
    pub(super) fn section_pick(&mut self, r: &Reference, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(d) = &self.ui.section_dialog else { return false };
        if d.items_active {
            let Some(b) = self.body_of(r) else {
                self.flash_info("Select parts".into(), cx);
                return true;
            };
            if let Some(d) = &mut self.ui.section_dialog {
                match d.items.iter().position(|x| *x == b) {
                    Some(i) => {
                        d.items.remove(i);
                    }
                    None => d.items.push(b),
                }
                self.ui.section_parts = Some((d.include, d.items.clone()));
            }
        } else {
            let Some(f) = self.frame_of_ref(r) else {
                self.flash_info(SELECT_SECTION_PLANE.into(), cx);
                return true;
            };
            let label = self.labeler(cx)(r);
            if let Some(i) = self.ui.section.iter().position(|p| p.label == label) {
                self.ui.section.remove(i);
            } else if self.ui.section.len() >= MAX_PLANES {
                self.flash_info("Maximum number of section planes reached".into(), cx);
                return true;
            } else {
                self.ui.section.push(plane(&label, f.origin, f.normal));
                if self.message.as_ref().is_some_and(|(_, m)| m == SELECT_SECTION_PLANE) {
                    self.message = None;
                }
            }
            if let Some(d) = &mut self.ui.section_dialog {
                d.active = self.ui.section.len().saturating_sub(1);
            }
        }
        self.apply_section(window, cx);
        true
    }

    /// Restore saved planes (named views).
    pub(super) fn restore_section(&mut self, saved: &[[f64; 4]], window: &mut Window, cx: &mut Context<Self>) {
        self.ui.section = saved
            .iter()
            .take(MAX_PLANES)
            .enumerate()
            .map(|(i, s)| {
                let n = DVec3::new(s[0], s[1], s[2]);
                plane(&format!("Section plane {}", i + 1), n * s[3], n)
            })
            .collect();
        self.apply_section(window, cx);
    }

    /// The arrow of the active plane while the dialog is open: on the plane
    /// (where it passes the model's center) along its normal.
    pub(super) fn section_arrow(&self) -> Option<super::manipulators::Arrow> {
        let d = self.ui.section_dialog.as_ref()?;
        let pl = self.ui.section.get(d.active)?;
        let (lo, hi) = self.fit_bounds()?;
        let center = (lo + hi) * 0.5;
        let on_plane = pl.origin + pl.normal * pl.offset;
        let base = center - pl.normal * pl.normal.dot(center - on_plane);
        Some(super::manipulators::Arrow { base, dir: pl.normal, length: section_handle(lo, hi), param: SECTION_PARAM, flip: None })
    }

    /// A drag of the section arrow: its handle end is now at `length`
    /// along the normal from the current plane position.
    pub(super) fn drag_section_arrow(&mut self, length: f64, window: &mut Window, cx: &mut Context<Self>) {
        let Some((lo, hi)) = self.fit_bounds() else { return };
        let Some(i) = self.ui.section_dialog.as_ref().map(|d| d.active) else { return };
        if let Some(pl) = self.ui.section.get_mut(i) {
            pl.offset += length - section_handle(lo, hi);
        }
        self.apply_section(window, cx);
    }

    pub(super) fn render_section_panel(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let d = self.ui.section_dialog.as_ref()?;
        let planes = self.ui.section.iter().map(|pl| pl.label.trim_end_matches(" plane").to_string()).collect();
        let items = d.items.iter().map(|b| self.display.body(b).map(|x| x.name.clone()).unwrap_or_else(|| "Part".into())).collect();
        let props = crate::section_dialog::Props { include: d.include, items_active: d.items_active, planes, active: d.active, items };
        Some(crate::section_dialog::render(props, "ps-section", &cx.entity(), Self::on_section_action, cx))
    }

    /// The Section view dialog's buttons, tabs, fields and chips.
    fn on_section_action(&mut self, a: crate::section_dialog::Action, w: &mut Window, cx: &mut Context<Self>) {
        use crate::section_dialog::Action;
        match a {
            Action::Accept => self.close_section_dialog(true, w, cx),
            Action::Cancel => self.close_section_dialog(false, w, cx),
            Action::Include(include) => {
                if let Some(d) = &mut self.ui.section_dialog {
                    d.include = include;
                    if !d.items.is_empty() {
                        self.ui.section_parts = Some((include, d.items.clone()));
                    }
                }
                self.apply_section(w, cx);
            }
            Action::ItemsActive(items) => {
                if let Some(d) = &mut self.ui.section_dialog {
                    d.items_active = items;
                }
                cx.notify();
            }
            Action::RemovePlane(i) => {
                if i < self.ui.section.len() {
                    self.ui.section.remove(i);
                }
                if let Some(d) = &mut self.ui.section_dialog {
                    d.active = self.ui.section.len().saturating_sub(1);
                }
                self.apply_section(w, cx);
            }
            Action::RemoveItem(i) => {
                if let Some(d) = &mut self.ui.section_dialog
                    && i < d.items.len()
                {
                    d.items.remove(i);
                    self.ui.section_parts = (!d.items.is_empty()).then(|| (d.include, d.items.clone()));
                }
                self.apply_section(w, cx);
            }
            Action::PlaneButton => self.add_view_section_plane(w, cx),
        }
    }

    /// The offset box beside the section arrow, drawn in the 3D frame (it
    /// follows the camera without a window redraw); [`Self::render_section_value`]
    /// is the GPUI element it reproduces.
    pub(super) fn section_value_label(&self, window: &Window, cx: &App) -> Option<noshape_render::overlay::Label> {
        // While it is typed into, the box is a GPUI field (section_value_field).
        if self.ui.section_offset_edit.is_some() {
            return None;
        }
        let a = self.section_arrow()?;
        let d = self.ui.section_dialog.as_ref()?;
        let pl = self.ui.section.get(d.active)?;
        let units = self.doc.read(cx).session.doc.settings.units;
        let text = noshape_units::format_quantity(noshape_units::Quantity::new(pl.offset, noshape_units::Dim::LENGTH), &units);
        let pal = p();
        let b = crate::overlay_labels::ValueBox { text: &text, text_size: 12., text_color: pal.text.into(), background: gpui_kit::rgb(0xffffff).into(), border: Some(pal.accent.into()), pad_x: 4., offset: [12., -10.] };
        crate::overlay_labels::value_box(window, cx, &b, a.base + a.dir * a.length)
    }

    /// The offset box beside the section arrow (live: e.g. "-0.91 in").
    pub(super) fn render_section_value(&self, cx: &App) -> Option<AnyElement> {
        let a = self.section_arrow()?;
        let d = self.ui.section_dialog.as_ref()?;
        let pl = self.ui.section.get(d.active)?;
        let tip = a.base + a.dir * a.length;
        let s = self.viewport.read(cx).project(tip)?;
        let units = self.doc.read(cx).session.doc.settings.units;
        let text = noshape_units::format_quantity(noshape_units::Quantity::new(pl.offset, noshape_units::Dim::LENGTH), &units);
        let pal = p();
        Some(
            div()
                .absolute()
                .left(px(s.x as f32 + 12.))
                .top(px(s.y as f32 - 10.))
                .px(px(4.))
                .border_1()
                .border_color(pal.accent)
                .bg(gpui_kit::rgb(0xffffff))
                .text_size(px(12.))
                .text_color(pal.text)
                .child(text)
                .into_any_element(),
        )
    }

    /// The offset box as a field (qa5 AN3; live: the arrow's value box takes
    /// a typed offset): over the drawn box a click target that opens it; then
    /// an input there. Enter or leaving it moves the plane; the click never
    /// reaches the graphics (it would pick the plane under it).
    pub(super) fn section_value_field(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let a = self.section_arrow()?;
        let d = self.ui.section_dialog.as_ref()?;
        let pl = self.ui.section.get(d.active)?;
        let s = self.viewport.read(cx).project(a.base + a.dir * a.length)?;
        let units = self.doc.read(cx).session.doc.settings.units;
        let text = noshape_units::format_quantity(noshape_units::Quantity::new(pl.offset, noshape_units::Dim::LENGTH), &units);
        let at = div().absolute().left(px(s.x as f32 + 12.)).top(px(s.y as f32 - 10.)).occlude();
        if let Some((input, _)) = &self.ui.section_offset_edit {
            let field = gpui_kit::component::input::Input::new(input).xsmall().appearance(false).text_size(px(12.));
            return Some(at.id("section-offset-field").w(px(90.)).px(px(4.)).border_1().border_color(p().accent).bg(gpui_kit::rgb(0xffffff)).child(gpui_kit::base::input::input_frame(div().child(field))).into_any_element());
        }
        // The drawn box: 12 px text, 4 px padding.
        let w = text.chars().count() as f32 * 7.0 + 10.0;
        Some(
            at.id("section-offset-hit")
                .w(px(w))
                .h(px(18.))
                .cursor(CursorStyle::IBeam)
                .on_mouse_down(MouseButton::Left, cx.listener(move |this, _: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    this.open_section_offset_edit(text.clone(), window, cx);
                }))
                .into_any_element(),
        )
    }

    fn open_section_offset_edit(&mut self, text: String, window: &mut Window, cx: &mut Context<Self>) {
        use gpui_kit::component::input::{InputEvent, InputState};
        let input = cx.new(|cx| InputState::new(window, cx).default_value(text));
        let sub = cx.subscribe_in(&input, window, |this: &mut Self, st, ev: &InputEvent, window, cx| {
            if matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                let typed = st.read(cx).value().to_string();
                this.commit_section_offset(&typed, window, cx);
            }
        });
        input.update(cx, |s, cx| {
            s.focus(window, cx);
            s.select_all(window, cx);
        });
        self.ui.section_offset_edit = Some((input, sub));
        cx.notify();
    }

    /// A typed offset (a bare number in the workspace unit) moves the active
    /// plane; text that doesn't evaluate leaves it.
    fn commit_section_offset(&mut self, typed: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.ui.section_offset_edit = None;
        let units = self.doc.read(cx).session.doc.settings.units;
        let vars = self.scope_variables(None, cx);
        let text = noshape_model::quantity_text::commit_text(typed, noshape_units::Dim::LENGTH, &units, &vars);
        if let Ok(q) = noshape_units::evaluate_as(&text, noshape_units::Dim::LENGTH, &units, &vars)
            && let Some(active) = self.ui.section_dialog.as_ref().map(|d| d.active)
            && let Some(pl) = self.ui.section.get_mut(active)
        {
            pl.offset = q.value;
            self.apply_section(window, cx);
        }
        window.focus(&self.focus, cx);
        cx.notify();
    }

    fn add_view_section_plane(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.ui.section.len() >= MAX_PLANES {
            self.flash_info("Maximum number of section planes reached".into(), cx);
            return;
        }
        let center = self.fit_bounds().map(|(lo, hi)| (lo + hi) * 0.5).unwrap_or_default();
        let n = -self.viewport.read(cx).camera.forward();
        self.ui.section.push(plane("View", center, n));
        if let Some(d) = &mut self.ui.section_dialog {
            d.active = self.ui.section.len() - 1;
            d.items_active = false;
        }
        self.apply_section(window, cx);
    }
}

impl PartStudioView {
    /// `NOSHAPE_PERF_SECTION=1` (benchmarks, perf.yml variants): a section
    /// by a horizontal plane through the middle of the parts, once they are
    /// there.
    pub(super) fn bench_section(&mut self, cx: &mut Context<Self>) {
        if !self.ui.section.is_empty() || std::env::var_os("NOSHAPE_PERF_SECTION").is_none() {
            return;
        }
        let Some((lo, hi)) = self.fit_bounds() else { return };
        self.ui.section.push(plane("Top plane", (lo + hi) * 0.5, DVec3::Z));
        self.opts.section = self.ui.section.iter().map(|p| p.clip()).collect();
        self.refresh_scene(true, cx);
    }
}

/// Parameter name marking the section arrow among the manipulators.
pub(super) const SECTION_PARAM: &str = "__section";

fn plane(label: &str, origin: DVec3, normal: DVec3) -> SectionPlane {
    SectionPlane { label: label.to_string(), origin, normal, offset: 0.0, flipped: false }
}

/// Length of the section arrow's handle: a fraction of the model size.
fn section_handle(lo: DVec3, hi: DVec3) -> f64 {
    ((hi - lo).length() * 0.12).max(1e-3)
}
