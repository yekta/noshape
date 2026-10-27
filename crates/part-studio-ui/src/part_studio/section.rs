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
    /// Where the manipulator's base starts (live qa4-r5-s9-5: the picked
    /// face's centre, a mate connector's or plane's origin); else (a
    /// restored or turned plane) across the model's centre.
    pub base: Option<DVec3>,
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
    /// The "Select default plane" popup and the plane it added.
    pub popup: Option<crate::section_dialog::PlanePopup>,
    pub popup_plane: Option<usize>,
}

/// The section view dialog's prompt (live part-studio/70).
const SELECT_SECTION_PLANE: &str = "Select a plane, planar face, cylinder, cone, or mate connector.";

impl PartStudioView {
    /// Plane frame of a plane, planar face, sketch entity or mate connector.
    pub(super) fn frame_of_ref(&self, r: &Reference) -> Option<PlaneFrame> {
        noshape_ops::studio::frame_of_ref(&self.display, self.context.as_deref(), r)
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
        let mut frames: Vec<(String, PlaneFrame, Option<DVec3>)> = refs.iter().filter_map(|r| self.frame_of_ref(r).map(|f| (labeler(r), f, self.section_home(r, &f)))).collect();
        if frames.is_empty() {
            frames.push(("Front plane".to_string(), PlaneFrame::FRONT, Some(PlaneFrame::FRONT.origin)));
        }
        frames
            .into_iter()
            .take(MAX_PLANES)
            .map(|(l, f, base)| SectionPlane { base, ..plane(&l, f.origin, f.normal) })
            .collect()
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
        self.ui.section_dialog = Some(SectionDialog { include, items, items_active: false, active, saved, popup: None, popup_plane: None });
        self.apply_section(window, cx);
    }

    /// "Edit section view…" (camera menu, right-click): the dialog on the
    /// current section, where ✕ turns the section off (live part-studio/2414:
    /// it does not restore it, even with nothing changed).
    pub(super) fn edit_section(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_section_dialog(window, cx);
        if let Some(d) = &mut self.ui.section_dialog {
            d.saved = Default::default();
        }
    }

    /// "Turn off section view" (also with the dialog open).
    pub(super) fn turn_off_section(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.end_section_dialog(false, window, cx);
        self.ui.section.clear();
        self.ui.section_parts = None;
        self.apply_section(window, cx);
    }

    /// Shift+X: open the dialog; with a section on and no dialog, turn it off.
    pub(super) fn toggle_section(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Live (psb 10-21 f, qa4 F14): with a feature dialog open, Shift+X
        // does nothing (Section view is only offered once it closes).
        if self.edit.is_some() && self.ui.section_dialog.is_none() {
            return;
        }
        // Live 10-27: a second Shift+X leaves the dialog open (its ✕ closes it).
        if self.ui.section_dialog.is_some() {
            return;
        }
        if !self.ui.section.is_empty() {
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

    /// Enter accepts; Esc closes the value box first (live: "Esc closes
    /// only the value box", then cancels the dialog).
    pub(super) fn close_section_dialog(&mut self, accept: bool, window: &mut Window, cx: &mut Context<Self>) {
        if !accept && std::mem::take(&mut self.ui.section_box) {
            self.ui.section_offset_edit = None;
            window.focus(&self.focus, cx);
            cx.notify();
            return;
        }
        self.end_section_dialog(accept, window, cx);
    }

    /// ✓ keeps the section, ✕ restores the state before the dialog.
    pub(super) fn end_section_dialog(&mut self, accept: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(d) = self.ui.section_dialog.take() else { return };
        self.ui.section_offset_edit = None;
        self.reset_section_manip();
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
                self.ui.section.push(SectionPlane { base: self.section_home(r, &f), ..plane(&label, f.origin, f.normal) });
                if self.message.as_ref().is_some_and(|(_, m)| m == SELECT_SECTION_PLANE) {
                    self.message = None;
                }
            }
            if let Some(d) = &mut self.ui.section_dialog {
                d.active = self.ui.section.len().saturating_sub(1);
            }
            self.reset_section_manip();
        }
        self.apply_section(window, cx);
        true
    }

    /// The manipulator base's start for a picked plane reference: a face's
    /// centre (area-weighted over its display triangles), a connector's or
    /// a plane's origin.
    fn section_home(&self, r: &Reference, f: &PlaneFrame) -> Option<DVec3> {
        match r {
            // Live (ui-snapv/25): a default (or feature) plane's base is at
            // its origin.
            Reference::MateConnector { .. } | Reference::Plane(_) | Reference::FeaturePlane(_) => Some(f.origin),
            Reference::Face(_) => self.display.bodies.iter().filter(|b| !b.hidden).find_map(|b| {
                let fi = b.face_refs.iter().position(|x| x == r)?;
                let &(first, count) = b.mesh.face_ranges.get(fi)?;
                let idx = b.mesh.indices.get(first as usize..(first + count) as usize)?;
                let p = |i: u32| b.mesh.positions.get(i as usize).map(|a| DVec3::new(a[0] as f64, a[1] as f64, a[2] as f64));
                let (mut sum, mut area) = (DVec3::ZERO, 0.0);
                for t in idx.as_chunks::<3>().0 {
                    let (a, b, c) = (p(t[0])?, p(t[1])?, p(t[2])?);
                    let w = (b - a).cross(c - a).length() * 0.5;
                    sum += (a + b + c) / 3.0 * w;
                    area += w;
                }
                (area > 0.0).then(|| sum / area)
            }),
            _ => None,
        }
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
        // Where the base circle was slid to (live), else across the model's centre.
        let at = self.ui.section_base.or(pl.base).unwrap_or(center);
        // A snapped base sits on what it snapped to, off the plane too.
        let base = match &self.ui.section_snap {
            Some((q, _, _)) => *q,
            None => at - pl.normal * pl.normal.dot(at - on_plane),
        };
        Some(super::manipulators::Arrow { base, dir: pl.normal, length: section_handle(lo, hi), param: SECTION_PARAM, flip: None })
    }

    /// A drag of the section arrow: its handle end is now at `length`
    /// along the normal from the current plane position.
    pub(super) fn drag_section_arrow(&mut self, length: f64, window: &mut Window, cx: &mut Context<Self>) {
        let Some((lo, hi)) = self.fit_bounds() else { return };
        let Some(i) = self.ui.section_dialog.as_ref().map(|d| d.active) else { return };
        if let Some(pl) = self.ui.section.get_mut(i) {
            pl.offset += length - section_handle(lo, hi);
            // Live: the plane moves in 0.1 mm (0.01 in) steps from the drag
            // start ("2.6 mm", "-8.5 mm").
            if let Some(s0) = self.ui.section_drag0 {
                let units = self.doc.read(cx).session.doc.settings.units;
                let imperial = matches!(units.length, noshape_units::LengthUnit::Inch | noshape_units::LengthUnit::Foot | noshape_units::LengthUnit::Yard);
                let step = if imperial { 0.000254 } else { 0.0001 };
                pl.offset = s0 + ((pl.offset - s0) / step).round() * step;
            }
        }
        self.ui.section_rot = None;
        self.apply_section(window, cx);
        if self.ui.section_box {
            self.open_section_box(window, cx);
        }
    }

    /// The value box's text, shown from a drag on (live): the last ring
    /// drag's angle ("15 deg"), else the distance from the arrow drag's start.
    fn section_value_text(&self, cx: &App) -> Option<String> {
        if !self.ui.section_box {
            return None;
        }
        if let Some(r) = self.ui.section_rot {
            return Some(super::section_manip::deg_text(r.deg));
        }
        let d = self.ui.section_dialog.as_ref()?;
        let pl = self.ui.section.get(d.active)?;
        let units = self.doc.read(cx).session.doc.settings.units;
        Some(noshape_units::format_quantity(noshape_units::Quantity::new(pl.offset - self.ui.section_drag0.unwrap_or(0.0), noshape_units::Dim::LENGTH), &units))
    }

    /// The value box from a drag on: a field with its value selected,
    /// following the drag (live asm/2370–2372).
    pub(super) fn open_section_box(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(text) = self.section_value_text(cx) else { return };
        if let Some((input, _)) = &self.ui.section_offset_edit {
            input.update(cx, |s, cx| {
                s.set_value(text, window, cx);
                s.select_all(window, cx);
            });
            return;
        }
        self.open_section_offset_edit(text, window, cx);
    }

    /// Forget the manipulator's rotation and moved base (another plane).
    fn reset_section_manip(&mut self) {
        self.ui.section_rot = None;
        self.ui.section_box = false;
        self.ui.section_drag0 = None;
        self.ui.section_base = None;
        self.ui.section_snap = None;
    }

    pub(super) fn render_section_panel(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let d = self.ui.section_dialog.as_ref()?;
        // Live (skb 10-24 assembly section, qa4-r5-s9-5): a chip is named by
        // its plane's nearest standard direction, whatever was picked (a
        // face, a connector: "Section plane 1 (Top)"); a turned plane has no
        // name.
        // Flip plane names the other side (live: "Section plane 2 (Back)").
        let planes = self.ui.section.iter().map(|pl| if pl.label.starts_with("Section plane") { String::new() } else if pl.flipped { opposite_name(direction_name(pl.normal)).to_string() } else { direction_name(pl.normal).to_string() }).collect();
        let items = d.items.iter().map(|b| self.display.body(b).map(|x| x.name.clone()).unwrap_or_else(|| "Part".into())).collect();
        let props = crate::section_dialog::Props { include: d.include, items_active: d.items_active, planes, active: d.active, items, popup: d.popup };
        Some(crate::section_dialog::render(props, "ps-section", &cx.entity(), Self::on_section_action, cx))
    }

    /// The Section view dialog's buttons, tabs, fields and chips.
    fn on_section_action(&mut self, a: crate::section_dialog::Action, w: &mut Window, cx: &mut Context<Self>) {
        use crate::section_dialog::Action;
        match a {
            Action::Accept => self.close_section_dialog(true, w, cx),
            Action::Cancel => self.end_section_dialog(false, w, cx),
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
                self.reset_section_manip();
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
            // Live ("Select default plane"): the button opens the popup; a
            // plane chosen there is added (and replaced by the next choice)
            // until ✓ commits it or ✕ takes it away again.
            Action::PlaneButton => {
                if let Some(d) = &mut self.ui.section_dialog {
                    d.popup = if d.popup.is_some() { None } else { Some(Default::default()) };
                    d.popup_plane = None;
                }
                cx.notify();
            }
            Action::PopupList => {
                if let Some(p) = self.ui.section_dialog.as_mut().and_then(|d| d.popup.as_mut()) {
                    p.list_open = !p.list_open;
                }
                cx.notify();
            }
            Action::PopupPick(k) => {
                let Some(d) = &mut self.ui.section_dialog else { return };
                let normal = [DVec3::Z, DVec3::NEG_Y, DVec3::X][k.min(2)];
                let pl = plane(["Top", "Front", "Right"][k.min(2)], DVec3::ZERO, normal);
                let i = match d.popup_plane.filter(|i| *i < self.ui.section.len()) {
                    Some(i) => {
                        self.ui.section[i] = pl;
                        i
                    }
                    None if self.ui.section.len() >= MAX_PLANES => {
                        self.flash_info("Maximum number of section planes reached".into(), cx);
                        return;
                    }
                    None => {
                        self.ui.section.push(pl);
                        self.ui.section.len() - 1
                    }
                };
                d.popup_plane = Some(i);
                d.active = i;
                d.items_active = false;
                if let Some(p) = &mut d.popup {
                    p.list_open = false;
                }
                self.reset_section_manip();
                self.apply_section(w, cx);
            }
            Action::PopupFlip => {
                let Some(d) = &self.ui.section_dialog else { return };
                let i = d.popup_plane.unwrap_or(d.active);
                if let Some(pl) = self.ui.section.get_mut(i) {
                    pl.flipped = !pl.flipped;
                }
                self.apply_section(w, cx);
            }
            Action::PopupCommit => {
                if let Some(d) = &mut self.ui.section_dialog {
                    d.popup = None;
                    d.popup_plane = None;
                }
                cx.notify();
            }
            Action::PopupCancel => {
                let Some(d) = &mut self.ui.section_dialog else { return };
                d.popup = None;
                if let Some(i) = d.popup_plane.take().filter(|i| *i < self.ui.section.len()) {
                    self.ui.section.remove(i);
                    d.active = self.ui.section.len().saturating_sub(1);
                    self.reset_section_manip();
                }
                self.apply_section(w, cx);
            }
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
        let (anchor, _) = self.section_box_anchor(cx)?;
        let text = self.section_value_text(cx)?;
        let pal = p();
        let b = crate::overlay_labels::ValueBox { text: &text, text_size: 13.33, text_color: pal.text.into(), background: gpui_kit::rgb(0xffffff).into(), border: Some(gpui_kit::rgb(0x1651b0).into()), pad_x: 3., offset: [BOX_DX, BOX_DY] };
        crate::overlay_labels::value_box(window, cx, &b, anchor)
    }

    /// The offset box beside the section arrow (live: e.g. "-0.91 in").
    pub(super) fn render_section_value(&self, cx: &App) -> Option<AnyElement> {
        let (_, s) = self.section_box_anchor(cx)?;
        let text = self.section_value_text(cx)?;
        Some(section_box(s).flex().items_center().justify_end().px(px(3.)).text_size(px(13.33)).text_color(p().text).child(text).into_any_element())
    }

    /// The offset box as a field (qa5 AN3; live: the arrow's value box takes
    /// a typed offset): over the drawn box a click target that opens it; then
    /// an input there. Enter or leaving it moves the plane; the click never
    /// reaches the graphics (it would pick the plane under it).
    pub(super) fn section_value_field(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (_, s) = self.section_box_anchor(cx)?;
        let text = self.section_value_text(cx)?;
        let at = section_box(s).occlude();
        if let Some((input, _)) = &self.ui.section_offset_edit {
            let field = gpui_kit::component::input::Input::new(input).xsmall().appearance(false).text_size(px(13.33)).text_align(TextAlign::Right);
            return Some(
                at.id("section-offset-field")
                    .px(px(3.))
                    // Esc typing in the box closes it (Esc is bound to Cancel only outside fields); the next Esc
                    // cancels the dialog (live asm/2373).
                    .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                        if ev.keystroke.key == "escape" {
                            cx.stop_propagation();
                            this.close_section_dialog(false, window, cx);
                        }
                    }))
                    .child(gpui_kit::base::input::input_frame(div().size_full().flex().items_center().child(field)))
                    .into_any_element(),
            );
        }
        // The drawn box takes the click.
        Some(
            at.id("section-offset-hit")
                .bg(gpui_kit::rgba(0x00000000))
                .border_color(gpui_kit::rgba(0x00000000))
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
                this.commit_section_offset(&typed, matches!(ev, InputEvent::PressEnter { .. }), window, cx);
            }
        });
        input.update(cx, |s, cx| {
            s.focus(window, cx);
            s.select_all(window, cx);
        });
        self.ui.section_offset_edit = Some((input, sub));
        cx.notify();
    }

    /// A typed offset (a bare number in the workspace unit, from the arrow
    /// drag's start) moves the active plane; text that doesn't evaluate
    /// leaves it. After Enter the box stays open with its value selected
    /// (live: "5" + Enter → "5 mm").
    fn commit_section_offset(&mut self, typed: &str, enter: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.ui.section_offset_edit = None;
        let units = self.doc.read(cx).session.doc.settings.units;
        let vars = self.scope_variables(None, cx);
        // After a ring drag the box takes an angle (live: "33" + Enter → 33 deg).
        if let Some(r) = self.ui.section_rot {
            let text = noshape_model::quantity_text::commit_text(typed, noshape_units::Dim::ANGLE, &units, &vars);
            if let Ok(q) = noshape_units::evaluate_as(&text, noshape_units::Dim::ANGLE, &units, &vars) {
                self.set_section_rotation(super::section_manip::RingDrag { deg: q.value.to_degrees(), dragging: false, ..r }, window, cx);
            }
            self.after_section_commit(enter, window, cx);
            return;
        }
        let text = noshape_model::quantity_text::commit_text(typed, noshape_units::Dim::LENGTH, &units, &vars);
        if let Ok(q) = noshape_units::evaluate_as(&text, noshape_units::Dim::LENGTH, &units, &vars)
            && let Some(active) = self.ui.section_dialog.as_ref().map(|d| d.active)
            && let Some(pl) = self.ui.section.get_mut(active)
        {
            pl.offset = self.ui.section_drag0.unwrap_or(0.0) + q.value;
            self.apply_section(window, cx);
        }
        self.after_section_commit(enter, window, cx);
    }

    fn after_section_commit(&mut self, enter: bool, window: &mut Window, cx: &mut Context<Self>) {
        if enter {
            self.open_section_box(window, cx);
        } else {
            window.focus(&self.focus, cx);
        }
        cx.notify();
    }
}

/// The value box (live asm/2370–2371: an `os-canvas-text-edit` 75×26 with a
/// 1 px #1651b0 border, its corner 40 px right of the rotate glyph's centre
/// and 25 px above it; the value right-aligned).
const BOX_DX: f32 = 40.;
const BOX_DY: f32 = -25.;

fn section_box(anchor: glam::DVec2) -> Div {
    div().absolute().left(px(anchor.x as f32 + BOX_DX)).top(px(anchor.y as f32 + BOX_DY)).w(px(75.)).h(px(26.)).border_1().border_color(gpui_kit::rgb(0x1651b0)).bg(gpui_kit::rgb(0xffffff))
}

/// Parameter name marking the section arrow among the manipulators.
pub(super) const SECTION_PARAM: &str = "__section";

/// The default plane a normal is nearest to.
fn direction_name(n: DVec3) -> &'static str {
    let a = n.abs();
    if a.z >= a.x && a.z >= a.y {
        "Top"
    } else if a.y >= a.x {
        "Front"
    } else {
        "Right"
    }
}

/// The standard direction opposite `name` (a flipped plane's chip).
fn opposite_name(name: &str) -> &'static str {
    match name {
        "Top" => "Bottom",
        "Front" => "Back",
        _ => "Left",
    }
}

fn plane(label: &str, origin: DVec3, normal: DVec3) -> SectionPlane {
    SectionPlane { label: label.to_string(), origin, normal, offset: 0.0, flipped: false, base: None }
}

/// Length of the section arrow's handle: a fraction of the model size.
fn section_handle(lo: DVec3, hi: DVec3) -> f64 {
    ((hi - lo).length() * 0.12).max(1e-3)
}

#[cfg(test)]
mod tests {
    use super::direction_name;
    use glam::DVec3;

    /// Live: the top face of a block or a connector on it reads "(Top)".
    #[::core::prelude::v1::test]
    fn chips_are_named_by_the_nearest_default_plane() {
        assert_eq!(direction_name(DVec3::Z), "Top");
        assert_eq!(direction_name(-DVec3::Y), "Front");
        assert_eq!(direction_name(DVec3::new(0.9, 0.1, 0.3).normalize()), "Right");
    }
}
