//! Drawing editor: Onshape's Weld symbol dialog (live drawing/140–159),
//! laid out like the symbol: Flip symbol (ISO: + Flip identification line)
//! and the Joint spacer (ANSI), Stagger (Left / Right radios) and
//! Symmetric; the other side's rows above the reference line (Groove, Root
//! opening, Finishing (ANSI), Contour, then the joint row: size, second
//! size, joint ▾, length, pitch) and the arrow side's mirrored below it; a
//! live preview of the symbol on the line, the Reference text area at the
//! tail, the field-weld flags and All around at the arrow end. Select the
//! joint (an edge or vertex), click to place; the dialog stays open until ✓.
//! The standard is the drawing's or Drawing properties › Annotations › Weld
//! symbols › Standard.

use crate::drawing_view::*;
use crate::os_ui::OsDropdownExt as _;
use crate::theme::p;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputState, Textarea, TextareaState};
use gpui_kit::component::radio::Radio;
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::drawing::weld::{FINISHES, JointSpacer, WeldContour, WeldFlag, WeldJoint, WeldSide, WeldSpec};
use noshape_model::drawing::{Annotation, AnnotationKind, ViewId};

/// Text boxes of one side, in [`WeldDlg::inputs`] order.
const SIDE_FIELDS: usize = 6;
const SIZE: usize = 0;
const DEPTH: usize = 1;
const LENGTH: usize = 2;
const PITCH: usize = 3;
const ROOT: usize = 4;
const GROOVE: usize = 5;

/// Dialog geometry (px, from the live dialog): width, the label column's
/// right edge, the joint row's left edge, box size and row pitch.
const WIDTH: f32 = 537.;
const LABEL_RIGHT: f32 = 250.;
const ROW_LEFT: f32 = 152.;
const BOX: f32 = 50.;
const ROW: f32 = 35.;

/// Weld dialog state.
pub struct WeldDlg {
    pub spec: WeldSpec,
    /// The symbol being edited (or placed by this dialog: `placed`).
    pub edit: Option<u64>,
    pub placed: bool,
    /// The joint the arrow points at (view, point; sheet when no view).
    pub target: Option<(Option<ViewId>, [f64; 2])>,
    /// Text boxes: the arrow side's [`SIDE_FIELDS`], then the other side's.
    inputs: Vec<Entity<InputState>>,
    reference: Entity<TextareaState>,
}

type SetSpec = std::rc::Rc<dyn Fn(&mut WeldSpec)>;

/// A checkbox / radio handler changing the dialog's spec.
fn weld_set(f: SetSpec, cx: &mut Context<DrawingEditor>) -> impl Fn(&bool, &mut Window, &mut App) + 'static {
    cx.listener(move |this: &mut DrawingEditor, _: &bool, _, cx| {
        if let Some(d) = &mut this.weld_dlg {
            f(&mut d.spec);
        }
        cx.notify();
    })
}

/// Icon of a joint (`below`: the arrow side's, mirrored).
fn joint_icon(j: WeldJoint, below: bool) -> Option<String> {
    let slug = match j {
        WeldJoint::None => return None,
        WeldJoint::SquareGroove => "square-groove",
        WeldJoint::VGroove => "v-groove",
        WeldJoint::UGroove => "u-groove",
        WeldJoint::JGroove => "j-groove",
        WeldJoint::BevelGroove => "bevel-groove",
        WeldJoint::BevelButtBroadRoot => "bevel-butt-broad-root",
        WeldJoint::VFlareGroove => "v-flare-groove",
        WeldJoint::BevelFlareGroove => "bevel-flare-groove",
        WeldJoint::Bead => "bead",
        WeldJoint::Fillet => "fillet",
        WeldJoint::PlugSlot => "plug-slot",
        WeldJoint::Seam => "seam",
        WeldJoint::Spot => "spot",
    };
    Some(format!("weld-{slug}{}", if below { "-below" } else { "" }))
}

/// The dropdown list's version of a glyph icon (upright, centred).
fn menu_icon(icon: Option<String>) -> Option<String> {
    icon.map(|i| i.replacen("weld-", "weld-menu-", 1))
}

fn spacer_icon(s: JointSpacer) -> Option<&'static str> {
    Some(match s {
        JointSpacer::None => return None,
        JointSpacer::DoubleV => "weld-double-v",
        JointSpacer::DoubleBevel => "weld-double-bevel",
        JointSpacer::DoubleU => "weld-double-u",
        JointSpacer::DoubleJ => "weld-double-j",
        JointSpacer::DoubleFlareV => "weld-double-flare-v",
        JointSpacer::DoubleFlareBevel => "weld-double-flare-bevel",
    })
}

impl DrawingEditor {
    fn new_weld_dlg(&mut self, spec: WeldSpec, edit: Option<u64>, window: &mut Window, cx: &mut Context<Self>) -> WeldDlg {
        let vals = |s: &WeldSide| [s.size.clone(), s.depth.clone(), s.length.clone(), s.pitch.clone(), s.root.clone(), s.groove.clone()];
        let inputs = vals(&spec.arrow).into_iter().chain(vals(&spec.other)).map(|v| cx.new(|cx| InputState::new(window, cx).default_value(v))).collect();
        let reference = cx.new(|cx| TextareaState::new(window, cx).rows(3).default_value(spec.reference.clone().unwrap_or_default()));
        WeldDlg { spec, edit, placed: false, target: None, inputs, reference }
    }

    /// Create / drop the dialog with the Weld symbol tool (called from render).
    pub fn sync_weld_dlg(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let active = matches!(self.tool, Tool::Weld);
        let placing = self.weld_dlg.as_ref().is_some_and(|d| d.edit.is_none());
        if active && self.weld_dlg.is_none() {
            self.weld_dlg = Some(self.new_weld_dlg(WeldSpec::default(), None, window, cx));
        } else if !active && placing {
            self.weld_dlg = None;
        }
    }

    /// Edit… on a weld symbol.
    pub fn open_weld_edit(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        let Some(spec) = self.current_sheet().annotations.iter().find(|a| a.id == id).and_then(|a| match &a.kind {
            AnnotationKind::Weld { spec, .. } => Some((**spec).clone()),
            _ => None,
        }) else {
            return;
        };
        self.tool = Tool::Select;
        self.weld_dlg = Some(self.new_weld_dlg(spec, Some(id), window, cx));
        cx.notify();
    }

    /// The spec with the typed texts.
    fn weld_spec(&self, cx: &App) -> Option<WeldSpec> {
        let d = self.weld_dlg.as_ref()?;
        let v = |i: usize| d.inputs[i].read(cx).value().trim().to_string();
        let mut s = d.spec.clone();
        for (k, side) in [&mut s.arrow, &mut s.other].into_iter().enumerate() {
            let o = k * SIDE_FIELDS;
            side.size = v(o + SIZE);
            side.depth = v(o + DEPTH);
            side.length = v(o + LENGTH);
            side.pitch = v(o + PITCH);
            side.root = v(o + ROOT);
            side.groove = v(o + GROOVE);
        }
        let r = d.reference.read(cx).value().to_string();
        s.reference = (s.reference.is_some() || !r.trim().is_empty()).then_some(r);
        Some(s)
    }

    /// Weld tool clicks: the joint, then the symbol's place (the dialog
    /// stays open on the placed symbol until ✓). True when used.
    pub fn weld_click(&mut self, cx: &mut Context<Self>) -> bool {
        if !matches!(self.tool, Tool::Weld) {
            return false;
        }
        let c = self.cursor;
        let target = self.weld_dlg.as_ref().and_then(|d| d.target);
        match target {
            None => {
                let t = match self.snap_at(c).or_else(|| self.any_point(c)) {
                    Some(p) => (Some(p.view), p.p),
                    None => (None, c),
                };
                if let Some(d) = &mut self.weld_dlg {
                    d.target = Some(t);
                }
            }
            Some((view, target)) => {
                let Some(spec) = self.weld_spec(cx) else { return true };
                let id = self.current_sheet().next_annotation_id();
                self.edit_sheet("Weld symbol", cx, |s| s.annotations.push(Annotation { id, kind: AnnotationKind::Weld { view, target, spec: Box::new(spec) }, position: c, ..Default::default() }));
                if let Some(d) = &mut self.weld_dlg {
                    d.edit = Some(id);
                    d.placed = true;
                }
                self.tool = Tool::Select;
                self.selection = vec![Sel::Annotation(id)];
            }
        }
        cx.notify();
        true
    }

    /// The leader preview while placing.
    pub fn weld_overlay(&self, overlay: &mut crate::drawing_scene::Overlay) {
        let Some((view, t)) = self.weld_dlg.as_ref().and_then(|d| d.target) else { return };
        let t = match view.and_then(|v| self.placement(v)) {
            Some(pl) => pl.to_sheet(t),
            None => t,
        };
        let c = self.cursor;
        let k = if c[0] >= t[0] { 1.0 } else { -1.0 };
        overlay.lines.push((vec![t, c, [c[0] + 26.0 * k, c[1]]], true));
    }

    /// ✓ applies the dialog to the symbol; ✕ drops a symbol this dialog
    /// placed (an edited one keeps its old settings).
    pub fn close_weld_dlg(&mut self, ok: bool, cx: &mut Context<Self>) {
        let spec = self.weld_spec(cx);
        let Some(d) = self.weld_dlg.take() else { return };
        match (ok, d.edit, spec) {
            (true, Some(id), Some(spec)) => self.edit_sheet("Edit weld symbol", cx, |s| {
                if let Some(AnnotationKind::Weld { spec: sp, .. }) = s.annotations.iter_mut().find(|a| a.id == id).map(|a| &mut a.kind) {
                    **sp = spec;
                }
            }),
            (false, Some(id), _) if d.placed => {
                self.edit_sheet("Weld symbol", cx, |s| s.annotations.retain(|a| a.id != id));
                self.selection.clear();
            }
            _ => {}
        }
        self.tool = Tool::Select;
        cx.notify();
    }

    pub fn render_weld_dlg(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let d = self.weld_dlg.as_ref()?;
        let pal = p();
        let s = d.spec.clone();
        let inputs = d.inputs.clone();
        let reference = d.reference.clone();
        let can_accept = d.edit.is_some();
        let iso = !self.drawing.weld_ansi();
        let ed = cx.entity();
        let set = weld_set;
        let ink = gpui_kit::rgb(0x333333);
        let check = |id: &'static str, label: &'static str, on: bool, enabled: bool, f: fn(&mut WeldSpec), cx: &mut Context<Self>| {
            div().when(!enabled, |d| d.opacity(0.45)).child(Checkbox::new(id).label(label).small().checked(on).when(enabled, |c| c.on_click(set(std::rc::Rc::new(f), cx))))
        };
        // A 50 × 30 dropdown showing an icon (joints, spacers) or a name.
        let dropdown = |id: SharedString, icon: Option<String>, label: String, items: Vec<(Option<String>, String, bool, SetSpec)>| {
            let ed = ed.clone();
            let btn = Button::new(id.clone()).ghost().xsmall().w(px(BOX)).h(px(30.)).dropdown_caret(true).border_1().border_color(gpui_kit::rgb(0xcccccc)).rounded(px(3.));
            let btn = match icon {
                Some(i) => btn.child(crate::ui::glyph(&i, 20., ink)),
                None => btn.label(label),
            };
            btn.os_dropdown_menu(SharedString::from(format!("{id}-dd")), false, move |mut m, _, _| {
                for (icon, l, on, f) in &items {
                    let (e, f) = (ed.clone(), f.clone());
                    // Glyph lists (joints, spacers): glyph + name, no check.
                    let any_icon = items.iter().any(|i| i.0.is_some());
                    let mut item = crate::os_menu::OsMenuItem::new(l.clone()).checked(*on && !any_icon);
                    if let Some(i) = icon {
                        item = item.icon_name(i);
                    }
                    m = m.item(item.on_click(move |_, _, cx| {
                        e.update(cx, |t, cx| {
                            if let Some(d) = &mut t.weld_dlg {
                                f(&mut d.spec);
                            }
                            cx.notify();
                        })
                    }));
                }
                m
            })
        };
        let boxed = |i: usize| div().w(px(BOX)).h(px(30.)).child(Input::new(&inputs[i]).small());
        // A labelled row: the label right-aligned before the control.
        let row = |l: &'static str, body: AnyElement| h_flex().h(px(ROW)).child(div().w(px(LABEL_RIGHT)).pr(px(8.)).flex().justify_end().text_size(px(12.)).child(l)).child(body).into_any_element();
        let no_spacer = s.spacer == JointSpacer::None;
        // One side's rows; `other`: above the line (listed outermost first).
        let side_rows = |other: bool| -> Vec<AnyElement> {
            // Symmetric: only the other side's rows.
            if !other && s.symmetric {
                return vec![];
            }
            let side = if other { &s.other } else { &s.arrow };
            let o = if other { SIDE_FIELDS } else { 0 };
            let (joint, picker_on) = match s.spacer {
                JointSpacer::None => (side.joint, true),
                sp => (sp.joint(), false),
            };
            let joints: Vec<(Option<String>, String, bool, SetSpec)> = WeldJoint::ALL
                .iter()
                .map(|&j| (menu_icon(joint_icon(j, false)), j.name().to_string(), j == side.joint, std::rc::Rc::new(move |s: &mut WeldSpec| if other { s.other.joint = j } else { s.arrow.joint = j }) as SetSpec))
                .collect();
            let id = if other { "weld-joint-o" } else { "weld-joint-a" };
            let picker = if picker_on {
                dropdown(id.into(), joint_icon(side.joint, !other), String::new(), joints).into_any_element()
            } else {
                div().w(px(BOX)).h(px(30.)).flex().items_center().justify_center().children(joint_icon(joint, !other).map(|i| crate::ui::glyph(&i, 20., gpui_kit::rgb(0x999999)))).into_any_element()
            };
            if joint == WeldJoint::None {
                return vec![h_flex().h(px(ROW)).pl(px(ROW_LEFT + 2. * (BOX + 2.))).child(picker).into_any_element()];
            }
            let main = h_flex().h(px(ROW)).pl(px(ROW_LEFT)).gap(px(2.)).child(boxed(o + SIZE)).child(boxed(o + DEPTH)).child(picker).child(boxed(o + LENGTH)).child(boxed(o + PITCH)).into_any_element();
            let contours: Vec<(Option<String>, String, bool, SetSpec)> = WeldContour::ALL.iter().map(|&c| (None, c.name().to_string(), c == side.contour, std::rc::Rc::new(move |s: &mut WeldSpec| if other { s.other.contour = c } else { s.arrow.contour = c }) as SetSpec)).collect();
            let finishes: Vec<(Option<String>, String, bool, SetSpec)> = FINISHES.iter().map(|&(f, n)| (None, n.to_string(), side.finish == f, std::rc::Rc::new(move |s: &mut WeldSpec| if other { s.other.finish = f.into() } else { s.arrow.finish = f.into() }) as SetSpec)).collect();
            let c_label = if side.contour == WeldContour::None { String::new() } else { side.contour.name().to_string() };
            let mut rows = vec![main, row("Contour", dropdown(if other { "weld-contour-o" } else { "weld-contour-a" }.into(), None, c_label, contours).into_any_element())];
            // ISO has no Finishing row.
            if !iso {
                rows.push(row("Finishing", dropdown(if other { "weld-finish-o" } else { "weld-finish-a" }.into(), None, side.finish.clone(), finishes).into_any_element()));
            }
            rows.push(row("Root opening", boxed(o + ROOT).into_any_element()));
            rows.push(row("Groove", boxed(o + GROOVE).into_any_element()));
            if other {
                rows.reverse();
            }
            rows
        };
        // Stagger and Symmetric need joints on both sides.
        let both = !no_spacer || (s.arrow.joint != WeldJoint::None && s.other.joint != WeldJoint::None);
        let spacers: Vec<(Option<String>, String, bool, SetSpec)> = JointSpacer::ALL.iter().map(|&j| (menu_icon(spacer_icon(j).map(str::to_string)), j.name().to_string(), j == s.spacer, std::rc::Rc::new(move |s: &mut WeldSpec| s.spacer = j) as SetSpec)).collect();
        let flag = |id: &'static str, f: WeldFlag, cx: &mut Context<Self>| {
            let on = s.flag == f;
            div()
                .id(id)
                .size(px(22.))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .rounded(px(2.))
                .when(on, |d| d.bg(gpui_kit::rgb(0xcfdbe5)))
                .child(crate::ui::glyph(if f == WeldFlag::Up { "weld-flag-up" } else { "weld-flag-down" }, 16., ink))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if let Some(d) = &mut this.weld_dlg {
                        d.spec.flag = if d.spec.flag == f { WeldFlag::None } else { f };
                    }
                    cx.notify();
                }))
        };
        // The symbol preview: the reference line (x 153–405), the leader
        // with its arrow, and the chosen joints' glyphs at x ≈ 270.
        let (up_joint, down_joint) = {
            let (a, o) = match s.spacer {
                JointSpacer::None if s.symmetric => (s.other.joint, s.other.joint),
                JointSpacer::None => (s.arrow.joint, s.other.joint),
                sp => (sp.joint(), sp.joint()),
            };
            if s.flip && !s.symmetric { (a, o) } else { (o, a) }
        };
        let st = if s.stagger { if s.stagger_right { 6. } else { -6. } } else { 0. };
        let glyph_at = |icon: Option<String>, x: f32| div().absolute().left(px(x - 24.)).top(px(18.)).children(icon.map(|i| crate::ui::glyph(&i, 48., ink)));
        let middle = div()
            .relative()
            .w(px(WIDTH))
            .h(px(84.))
            .child(div().absolute().left(px(11.)).top(px(0.)).w(px(100.)).h(px(60.)).child(Textarea::new(&reference).w(px(100.)).h(px(60.))))
            .child(div().absolute().left(px(11.)).top(px(66.)).child(Checkbox::new("weld-ref").label("Reference").small().checked(s.reference.is_some()).on_click(set(std::rc::Rc::new(|s: &mut WeldSpec| s.reference = if s.reference.is_some() { None } else { Some(String::new()) }), cx))))
            .child(div().absolute().left(px(153.)).top(px(42.)).w(px(252.)).h(px(1.)).bg(ink))
            .when(iso && !s.symmetric, |m| m.child(div().absolute().left(px(153.)).top(px(if s.flip_id_line { 38. } else { 46. })).w(px(252.)).h(px(1.)).border_t_1().border_dashed().border_color(ink)))
            .child(div().absolute().left(px(405.)).top(px(42.)).child(crate::ui::glyph("weld-arrow", 40., ink)))
            .child(glyph_at(joint_icon(up_joint, false), 270. + st))
            .child(glyph_at(joint_icon(down_joint, true), 270. - st))
            .child(div().absolute().left(px(453.)).top(px(9.)).child(flag("weld-flag-up", WeldFlag::Up, cx)))
            .child(div().absolute().left(px(451.)).top(px(34.)).child(check("weld-all", "All around", s.all_around, true, |s| s.all_around = !s.all_around, cx)))
            .child(div().absolute().left(px(453.)).top(px(60.)).child(flag("weld-flag-down", WeldFlag::Down, cx)));
        let upper = side_rows(true);
        let lower = side_rows(false);
        let flip_ok = !s.symmetric;
        Some(
            v_flex()
                .id("dr-weld-dlg")
                .absolute()
                .top(px(DIALOG_TOP))
                .left(px(self.dialog_left()))
                .w(px(WIDTH))
                .bg(pal.dialog_bg)
                .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
                .text_color(ink)
                .text_size(px(12.))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    h_flex()
                        .h(px(28.))
                        .pl(px(11.))
                        .border_b_1()
                        .border_color(gpui_kit::rgb(0xd3d7d9))
                        .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Weld symbol"))
                        .child(div().id("weld-ok").w(px(28.)).h(px(27.)).flex().items_center().justify_center().cursor_pointer().bg(gpui_kit::rgb(0x009500)).when(!can_accept, |d| d.opacity(0.3)).child(crate::ui::glyph("check", 18., gpui_kit::white())).when(can_accept, |d| d.on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_weld_dlg(true, cx)))))
                        .child(div().id("weld-cancel").w(px(28.)).h(px(27.)).flex().items_center().justify_center().cursor_pointer().child(crate::ui::glyph("close", 14., pal.cancel)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_weld_dlg(false, cx)))),
                )
                .child(
                    v_flex()
                        .pb(px(8.))
                        .child(
                            h_flex()
                                .h(px(30.))
                                .pl(px(6.))
                                .child(div().w(px(145.)).child(check("weld-flip", "Flip symbol", s.flip, flip_ok, |s| s.flip = !s.flip, cx)))
                                .when(iso, |r| r.child(check("weld-flip-id", "Flip identification line", s.flip_id_line, !s.symmetric, |s| s.flip_id_line = !s.flip_id_line, cx)))
                                .when(!iso, |r| r.child(div().pr(px(6.)).child("Joint spacer")).child(dropdown("weld-spacer".into(), spacer_icon(s.spacer).map(str::to_string), String::new(), spacers))),
                        )
                        .child(
                            h_flex()
                                .h(px(30.))
                                .pl(px(6.))
                                .child(div().w(px(149.)).child(check("weld-stagger", "Stagger", s.stagger, both, |s| s.stagger = !s.stagger, cx)))
                                .child(div().pt(px(6.)).child(check("weld-sym", "Symmetric", s.symmetric, both && !(!iso && !no_spacer), |s| s.symmetric = !s.symmetric, cx))),
                        )
                        .when(s.stagger, |v| {
                            v.child(
                                h_flex()
                                    .h(px(44.))
                                    .px(px(20.))
                                    .child(v_flex().items_center().child(Radio::new("weld-stagger-l").checked(!s.stagger_right).on_click(set(std::rc::Rc::new(|s: &mut WeldSpec| s.stagger_right = false), cx))).child("Left"))
                                    .child(div().flex_1())
                                    .child(v_flex().items_center().child(Radio::new("weld-stagger-r").checked(s.stagger_right).on_click(set(std::rc::Rc::new(|s: &mut WeldSpec| s.stagger_right = true), cx))).child("Right")),
                            )
                        })
                        .children(upper)
                        .child(middle)
                        .children(lower),
                )
                .into_any_element(),
        )
    }
}
