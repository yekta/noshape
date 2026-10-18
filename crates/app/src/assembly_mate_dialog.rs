//! Mate dialog (Fastened … Width): mate type selector, two mate connector
//! fields filled by clicking geometry in the viewport, flip primary /
//! reorient secondary, offsets and limits. Emits `Changed` for live preview.

use crate::assets::icon_path;
use crate::theme::{p, with_alpha};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{Icon, IconName, Selectable as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::assembly::eval::{limit_keys, offset_axes};
use noshape_model::assembly::{Limits, Mate, MateConnectorRef, MateId, MateType};
use noshape_units::ExprValue;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub enum MateDialogEvent {
    Changed,
    Accept,
    /// Shift+Enter / the heads-up checkmark: commit and start a new mate.
    AcceptAndNew,
    /// Solve: solve all mates including this one.
    Solve,
    /// Animate: preview the motion of the mate's degrees of freedom.
    Animate,
    /// The mate connector icon of a field: edit that mate connector.
    EditConnector(usize),
    Cancel,
}

pub type ConnectorLabeler = Arc<dyn Fn(&MateConnectorRef) -> String>;

pub fn mate_icon(t: MateType) -> SharedString {
    icon_path(match t {
        MateType::Fastened => "fastened",
        MateType::Revolute => "revolute",
        MateType::Slider => "slider",
        MateType::Planar => "planar",
        MateType::Cylindrical => "cylindrical",
        MateType::PinSlot => "pin-slot",
        MateType::Ball => "ball",
        MateType::Parallel => "parallel-mate",
        MateType::Tangent => "tangent-mate",
        MateType::Width => "width-mate",
    })
}

fn limit_label(key: &str) -> &'static str {
    match key {
        "rz" => "Z angle",
        "x" => "X distance",
        "y" => "Y distance",
        "cone" => "Maximum cone angle",
        _ => "Z distance",
    }
}

pub struct MateDialog {
    pub id: MateId,
    pub name: String,
    pub mate_type: MateType,
    pub a: Option<MateConnectorRef>,
    pub b: Option<MateConnectorRef>,
    /// Width mates: second width connector (slot 2) and optional second tab connector (slot 3).
    pub c: Option<MateConnectorRef>,
    pub d: Option<MateConnectorRef>,
    /// Connector field receiving viewport picks (0 = a, 1 = b, 2 = c, 3 = d).
    pub active: Option<usize>,
    pub flip_primary: bool,
    pub reorient: u8,
    pub suppressed: bool,
    /// Tangent mates: contact may move onto tangent-connected faces.
    pub tangent_propagation: bool,
    /// Axis of the rotation offset (0 = X, 1 = Y, 2 = Z).
    pub rotation_axis: u8,
    /// Mate indicator hidden (kept when editing).
    hidden: bool,
    /// "Reorder items" mode: handles to swap the two mate connectors.
    reordering: bool,
    /// Animate preview running.
    pub animating: bool,
    use_offset: bool,
    use_limits: bool,
    offsets: [Entity<InputState>; 4],
    limits: HashMap<&'static str, (Entity<InputState>, Entity<InputState>)>,
    labeler: ConnectorLabeler,
    pub status: Option<String>,
    focus: FocusHandle,
    _subs: Vec<Subscription>,
}

impl EventEmitter<MateDialogEvent> for MateDialog {}

impl Focusable for MateDialog {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

fn text_of(e: &Option<ExprValue>) -> String {
    e.as_ref().map(|e| e.text.clone()).unwrap_or_default()
}

impl MateDialog {
    pub fn new(mate: Mate, is_new: bool, labeler: ConnectorLabeler, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mk = |text: String, ph: &str, window: &mut Window, cx: &mut Context<Self>| {
            let ph = ph.to_string();
            cx.new(|cx| InputState::new(window, cx).placeholder(ph).default_value(text))
        };
        let offsets = [
            mk(text_of(&mate.offset_x), "0 mm", window, cx),
            mk(text_of(&mate.offset_y), "0 mm", window, cx),
            mk(text_of(&mate.offset_z), "0 mm", window, cx),
            mk(text_of(&mate.rotation), "0 deg", window, cx),
        ];
        let mut limits = HashMap::new();
        for key in ["rz", "x", "y", "z", "cone"] {
            let l = mate.limits.iter().find(|(k, _)| k == key).map(|(_, l)| l.clone());
            let unit = if key == "rz" || key == "cone" { "deg" } else { "mm" };
            let lo = mk(l.as_ref().map(|l| text_of(&l.min)).unwrap_or_default(), &format!("Min ({unit})"), window, cx);
            let hi = mk(l.as_ref().map(|l| text_of(&l.max)).unwrap_or_default(), &format!("Max ({unit})"), window, cx);
            limits.insert(key, (lo, hi));
        }
        let mut subs = Vec::new();
        let all: Vec<Entity<InputState>> = offsets.iter().cloned().chain(limits.values().flat_map(|(a, b)| [a.clone(), b.clone()])).collect();
        for s in all {
            subs.push(cx.subscribe_in(&s, window, |_: &mut Self, _, ev: &InputEvent, _, cx| match ev {
                InputEvent::Change => cx.emit(MateDialogEvent::Changed),
                InputEvent::PressEnter { .. } => cx.emit(MateDialogEvent::Accept),
                _ => {}
            }));
        }
        let use_offset = [&mate.offset_x, &mate.offset_y, &mate.offset_z, &mate.rotation].iter().any(|e| e.as_ref().is_some_and(|e| !e.text.trim().is_empty()));
        let use_limits = !mate.limits.is_empty();
        let (a, b) = if is_new { (None, None) } else { (Some(mate.a.clone()), Some(mate.b.clone())) };
        let (c, d) = if is_new { (None, None) } else { (mate.extra.first().cloned(), mate.extra.get(1).cloned()) };
        let active = if a.is_none() { Some(0) } else if b.is_none() { Some(1) } else { None };
        Self {
            id: mate.id,
            name: mate.name,
            mate_type: mate.mate_type,
            a,
            b,
            c,
            d,
            active,
            flip_primary: mate.flip_primary,
            reorient: mate.reorient_secondary,
            suppressed: mate.suppressed,
            tangent_propagation: mate.tangent_propagation,
            rotation_axis: mate.rotation_axis,
            hidden: mate.hidden,
            reordering: false,
            animating: false,
            use_offset,
            use_limits,
            offsets,
            limits,
            labeler,
            status: None,
            focus: cx.focus_handle(),
            _subs: subs,
        }
    }

    /// Fill both connectors (snap mode: the dragged and the target connector).
    pub fn prefill(&mut self, a: MateConnectorRef, b: MateConnectorRef, cx: &mut Context<Self>) {
        self.a = Some(a);
        self.b = Some(b);
        self.active = self.next_active();
        cx.emit(MateDialogEvent::Changed);
        cx.notify();
    }

    pub fn awaiting_pick(&self) -> bool {
        self.active.is_some()
    }

    /// Tangent mates pick bare faces / edges / vertices instead of mate connectors.
    pub fn picks_entities(&self) -> bool {
        self.mate_type == MateType::Tangent
    }

    fn slot(&self, i: usize) -> &Option<MateConnectorRef> {
        match i {
            0 => &self.a,
            1 => &self.b,
            2 => &self.c,
            _ => &self.d,
        }
    }

    fn slot_mut(&mut self, i: usize) -> &mut Option<MateConnectorRef> {
        match i {
            0 => &mut self.a,
            1 => &mut self.b,
            2 => &mut self.c,
            _ => &mut self.d,
        }
    }

    /// The connector in a field.
    pub fn field(&self, i: usize) -> Option<&MateConnectorRef> {
        self.slot(i).as_ref()
    }

    /// Replace a filled field (after editing its mate connector).
    pub fn replace_field(&mut self, i: usize, c: MateConnectorRef, cx: &mut Context<Self>) {
        *self.slot_mut(i) = Some(c);
        cx.emit(MateDialogEvent::Changed);
        cx.notify();
    }

    /// Filled connectors (for previews).
    pub fn connectors(&self) -> Vec<&MateConnectorRef> {
        [&self.a, &self.b, &self.c, &self.d].into_iter().flatten().collect()
    }

    fn next_active(&self) -> Option<usize> {
        let required: &[usize] = if self.mate_type == MateType::Width { &[0, 1, 2] } else { &[0, 1] };
        required.iter().copied().find(|i| self.slot(*i).is_none())
    }

    /// Fill the active connector field; returns true when accepted.
    pub fn set_connector(&mut self, c: MateConnectorRef, cx: &mut Context<Self>) -> bool {
        let Some(field) = self.active else { return false };
        let same = |o: &Option<MateConnectorRef>| o.as_ref().is_some_and(|o| o.instance == c.instance && o.path == c.path);
        if self.mate_type == MateType::Width {
            // Tab and width connectors must not share instances.
            let others: &[usize] = if field == 0 || field == 3 { &[1, 2] } else { &[0, 3] };
            if others.iter().any(|i| same(self.slot(*i))) {
                self.status = Some("Tab and width mate connectors must be on different instances".into());
                cx.notify();
                return false;
            }
        } else {
            // Both connectors must be on different instances.
            let other = if field == 0 { &self.b } else { &self.a };
            if same(other) {
                self.status = Some("Select a mate connector on a different instance".into());
                cx.notify();
                return false;
            }
        }
        *self.slot_mut(field) = Some(c);
        self.status = None;
        self.active = self.next_active();
        cx.emit(MateDialogEvent::Changed);
        cx.notify();
        true
    }

    fn expr(state: &Entity<InputState>, cx: &App) -> Option<ExprValue> {
        let t = state.read(cx).value().to_string();
        (!t.trim().is_empty()).then(|| ExprValue::new(t))
    }

    /// The mate as currently defined (None until both connectors are set).
    pub fn to_mate(&self, cx: &App) -> Option<Mate> {
        let (a, b) = (self.a.clone()?, self.b.clone()?);
        let extra = if self.mate_type == MateType::Width { std::iter::once(self.c.clone()?).chain(self.d.clone()).collect() } else { vec![] };
        let (ox, oy, oz, orot) = offset_axes(self.mate_type);
        let off = |i: usize, allowed: bool| if self.use_offset && allowed { Self::expr(&self.offsets[i], cx) } else { None };
        let mut limits = Vec::new();
        if self.use_limits {
            for key in limit_keys(self.mate_type) {
                let (lo, hi) = &self.limits[key];
                let (min, max) = (Self::expr(lo, cx), Self::expr(hi, cx));
                if min.is_some() || max.is_some() {
                    limits.push((key.to_string(), Limits { min, max }));
                }
            }
        }
        Some(Mate {
            id: self.id,
            name: self.name.clone(),
            mate_type: self.mate_type,
            a,
            b,
            offset_x: off(0, ox),
            offset_y: off(1, oy),
            offset_z: off(2, oz),
            rotation: off(3, orot),
            rotation_axis: self.rotation_axis,
            flip_primary: self.flip_primary,
            reorient_secondary: self.reorient,
            limits,
            suppressed: self.suppressed,
            extra,
            tangent_propagation: self.tangent_propagation,
            hidden: self.hidden,
        })
    }

    pub fn flip(&mut self, cx: &mut Context<Self>) {
        self.flip_primary = !self.flip_primary;
        cx.emit(MateDialogEvent::Changed);
        cx.notify();
    }

    /// Reorient the secondary axis by 90 degrees.
    pub fn reorient(&mut self, cx: &mut Context<Self>) {
        self.reorient = (self.reorient + 1) % 4;
        cx.emit(MateDialogEvent::Changed);
        cx.notify();
    }

    fn set_type(&mut self, t: MateType, cx: &mut Context<Self>) {
        if self.mate_type != t {
            // Keep custom names; rename default ones ("Fastened 1" -> "Revolute 1").
            if let Some(rest) = self.name.strip_prefix(self.mate_type.name()) {
                self.name = format!("{}{}", t.name(), rest);
            }
            self.mate_type = t;
            self.active = self.next_active();
            cx.emit(MateDialogEvent::Changed);
            cx.notify();
        }
    }

    fn connector_row(&self, field: usize, placeholder: &'static str, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let value = self.slot(field);
        let active = self.active == Some(field);
        let label: SharedString = match value {
            Some(c) => (self.labeler)(c).into(),
            None => placeholder.into(),
        };
        let filled = value.is_some();
        h_flex()
            .id(SharedString::from(format!("mc-field-{field}")))
            .h(px(28.))
            .px_2()
            .gap_2()
            .rounded_md()
            .border_1()
            .border_color(if active { pal.accent } else { pal.border_strong })
            .bg(if active { with_alpha(pal.accent, 0.10) } else { pal.panel })
            .cursor_pointer()
            .child(
                div()
                    .id(SharedString::from(format!("mc-edit-{field}")))
                    .when(filled && self.mate_type != MateType::Tangent, |d| {
                        d.cursor_pointer().tooltip(|w, cx| gpui_kit::component::tooltip::Tooltip::new("Edit mate connector").build(w, cx)).on_click(cx.listener(move |_, _: &ClickEvent, _, cx| {
                            cx.stop_propagation();
                            cx.emit(MateDialogEvent::EditConnector(field));
                        }))
                    })
                    .child(svg().path(icon_path("mate-connector")).size_3p5().text_color(if filled { pal.accent_hover } else { pal.text_faint })),
            )
            .child(div().flex_1().truncate().text_sm().text_color(if filled { pal.text } else { pal.text_faint }).child(label))
            .when(self.reordering && field < 2, |d| {
                d.child(Button::new(SharedString::from(format!("mc-handle-{field}"))).icon(if field == 0 { IconName::ArrowDown } else { IconName::ArrowUp }).ghost().xsmall().tooltip("Drag to reorder").on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    std::mem::swap(&mut this.a, &mut this.b);
                    this.active = this.next_active();
                    cx.emit(MateDialogEvent::Changed);
                    cx.notify();
                })))
            })
            .when(filled && !self.reordering, |d| {
                d.child(Button::new(SharedString::from(format!("mc-clear-{field}"))).icon(IconName::Close).ghost().xsmall().on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    *this.slot_mut(field) = None;
                    this.active = Some(field);
                    cx.emit(MateDialogEvent::Changed);
                    cx.notify();
                })))
            })
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.active = Some(field);
                cx.notify();
            }))
            .into_any_element()
    }
}

impl Render for MateDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let t = self.mate_type;
        let mut grid = h_flex().flex_wrap().gap_1();
        for mt in MateType::ALL {
            grid = grid.child(
                Button::new(SharedString::from(format!("mt-{}", mt.name())))
                    .icon(Icon::empty().path(mate_icon(mt)).with_size(px(16.)))
                    .ghost()
                    .with_size(px(26.))
                    .selected(mt == t)
                    .tooltip(format!("{} mate", mt.name()))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.set_type(mt, cx))),
            );
        }
        let (ox, oy, oz, orot) = offset_axes(t);
        let has_offset = ox || oy || oz || orot;
        let lkeys = limit_keys(t);
        let field = |label: &'static str, st: &Entity<InputState>| {
            h_flex().gap_2().child(div().w(px(84.)).text_xs().text_color(pal.text_muted).child(label)).child(div().flex_1().child(Input::new(st).xsmall()))
        };
        let mut offset_rows = v_flex().gap_1();
        if self.use_offset {
            if ox {
                offset_rows = offset_rows.child(field("X offset", &self.offsets[0]));
            }
            if oy {
                offset_rows = offset_rows.child(field("Y offset", &self.offsets[1]));
            }
            if oz {
                offset_rows = offset_rows.child(field("Z offset", &self.offsets[2]));
            }
            if orot {
                let ax = self.rotation_axis;
                let mut axes = h_flex().gap_0p5();
                for (k, label) in ["X", "Y", "Z"].into_iter().enumerate() {
                    let k = k as u8;
                    axes = axes.child(Button::new(SharedString::from(format!("mate-rot-ax-{k}"))).label(label).ghost().xsmall().selected(ax == k).tooltip(format!("Rotate about {label}")).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.rotation_axis = k;
                        cx.emit(MateDialogEvent::Changed);
                        cx.notify();
                    })));
                }
                offset_rows = offset_rows.child(h_flex().gap_2().child(div().w(px(84.)).text_xs().text_color(pal.text_muted).child("Rotation axis")).child(axes)).child(field("Rotation", &self.offsets[3]));
            }
        }
        let mut limit_rows = v_flex().gap_1();
        if self.use_limits {
            for key in lkeys {
                let (lo, hi) = &self.limits[key];
                limit_rows = limit_rows.child(
                    v_flex()
                        .gap_0p5()
                        .child(div().text_xs().text_color(pal.text_muted).child(limit_label(key)))
                        .child(if *key == "cone" {
                            // Ball: a single maximum angle from the Z axis.
                            h_flex().child(div().flex_1().child(Input::new(hi).xsmall()))
                        } else {
                            h_flex().gap_1().child(div().flex_1().child(Input::new(lo).xsmall())).child(div().flex_1().child(Input::new(hi).xsmall()))
                        }),
                );
            }
        }
        v_flex()
            .id("mate-dialog")
            .key_context("FeatureDialog")
            .track_focus(&self.focus)
            .w(px(300.))
            .rounded_lg()
            .border_1()
            .border_color(pal.border_strong)
            .bg(pal.panel_raised)
            .shadow_lg()
            .text_color(pal.text)
            .on_action(cx.listener(|_, _: &crate::actions::Accept, _, cx| cx.emit(MateDialogEvent::Accept)))
            .on_action(cx.listener(|_, _: &crate::actions::Cancel, _, cx| cx.emit(MateDialogEvent::Cancel)))
            .on_action(cx.listener(|this, _: &crate::actions::FlipPrimaryAxis, _, cx| this.flip(cx)))
            .on_action(cx.listener(|_, _: &crate::actions::AcceptAndRepeat, _, cx| cx.emit(MateDialogEvent::AcceptAndNew)))
            .child(
                h_flex()
                    .px_3()
                    .py_2()
                    .gap_2()
                    .border_b_1()
                    .border_color(pal.border)
                    .child(svg().path(mate_icon(t)).size_4().text_color(pal.accent))
                    .child(div().flex_1().text_sm().font_weight(FontWeight::SEMIBOLD).truncate().child(self.name.clone()))
                    .child(
                        Button::new("mate-ok")
                            .icon(Icon::empty().path(icon_path("check")))
                            .primary()
                            .xsmall()
                            .tooltip("Accept (Enter)")
                            .on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(MateDialogEvent::Accept))),
                    )
                    .child(Button::new("mate-cancel").icon(IconName::Close).ghost().xsmall().tooltip("Cancel (Esc)").on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(MateDialogEvent::Cancel)))),
            )
            .child(
                v_flex()
                    .p_3()
                    .gap_2()
                    .child(div().text_xs().text_color(pal.text_muted).child(format!("{} mate", t.name())))
                    .child(grid)
                    .children(match t {
                        MateType::Tangent => vec![
                            div().text_xs().text_color(pal.text_muted).child("Entities (faces, edges or vertices)").into_any_element(),
                            self.connector_row(0, "Select first entity", cx),
                            self.connector_row(1, "Select second entity", cx),
                        ],
                        MateType::Width => vec![
                            div().text_xs().text_color(pal.text_muted).child("Tab mate connector(s)").into_any_element(),
                            self.connector_row(0, "Select tab mate connector", cx),
                            self.connector_row(3, "Second tab mate connector (optional)", cx),
                            div().text_xs().text_color(pal.text_muted).child("Width mate connectors").into_any_element(),
                            self.connector_row(1, "Select first width mate connector", cx),
                            self.connector_row(2, "Select second width mate connector", cx),
                        ],
                        _ => vec![
                            div().text_xs().text_color(pal.text_muted).child("Mate connectors").into_any_element(),
                            self.connector_row(0, "Select mate connector", cx),
                            self.connector_row(1, "Select mate connector", cx),
                            // Reorder items: the order decides which connector flips /
                            // reorients and which frame the offset is relative to.
                            h_flex()
                                .justify_end()
                                .child(
                                    div()
                                        .id("mate-reorder")
                                        .text_xs()
                                        .text_color(pal.accent)
                                        .cursor_pointer()
                                        .hover(|d| d.underline())
                                        .child(if self.reordering { "Done" } else { "Reorder items" })
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.reordering = !this.reordering;
                                            cx.notify();
                                        })),
                                )
                                .into_any_element(),
                        ],
                    })
                    .child(
                        h_flex()
                            .gap_1()
                            .child(
                                Button::new("mate-flip")
                                    .icon(Icon::empty().path(icon_path("flip")))
                                    .ghost()
                                    .small()
                                    .selected(self.flip_primary)
                                    .label("Flip primary")
                                    .tooltip("Flip primary axis (A)")
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.flip(cx))),
                            )
                            .child(
                                Button::new("mate-reorient")
                                    .icon(Icon::empty().path(icon_path("flip-circular")))
                                    .ghost()
                                    .small()
                                    .label(format!("Reorient {}°", (self.reorient % 4) as u32 * 90))
                                    .tooltip("Reorient secondary axis (90° steps)")
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.reorient = (this.reorient + 1) % 4;
                                        cx.emit(MateDialogEvent::Changed);
                                        cx.notify();
                                    })),
                            )
                            // Animate: preview the mate's motion (mates with degrees of freedom).
                            .when(!noshape_model::assembly::eval::mate_dofs(t).is_empty(), |d| {
                                d.child(Button::new("mate-animate").icon(IconName::Play).ghost().small().selected(self.animating).tooltip("Animate mate degree of freedom").on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.animating = !this.animating;
                                    cx.emit(MateDialogEvent::Animate);
                                    cx.notify();
                                })))
                            }),
                    )
                    .when(t == MateType::Tangent, |d| {
                        d.child(Checkbox::new("mate-tangent-prop").label("Tangent propagation").checked(self.tangent_propagation).on_click(cx.listener(|this, v: &bool, _, cx| {
                            this.tangent_propagation = *v;
                            cx.emit(MateDialogEvent::Changed);
                            cx.notify();
                        })))
                    })
                    .when(has_offset, |d| {
                        d.child(Checkbox::new("mate-offset").label("Offset").checked(self.use_offset).on_click(cx.listener(|this, v: &bool, _, cx| {
                            this.use_offset = *v;
                            cx.emit(MateDialogEvent::Changed);
                            cx.notify();
                        })))
                        .child(offset_rows)
                    })
                    .when(!lkeys.is_empty(), |d| {
                        d.child(Checkbox::new("mate-limits").label("Limits").checked(self.use_limits).on_click(cx.listener(|this, v: &bool, _, cx| {
                            this.use_limits = *v;
                            cx.emit(MateDialogEvent::Changed);
                            cx.notify();
                        })))
                        .child(limit_rows)
                    })
                    .child(h_flex().justify_end().child(Button::new("mate-solve").label("Solve").small().outline().tooltip("Solve all mates including this one").on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(MateDialogEvent::Solve)))))
                    .when_some(self.status.clone(), |d, s| d.child(div().text_xs().text_color(pal.warning).child(s)))
                    .when(self.active.is_some(), |d| {
                        d.child(div().text_xs().text_color(pal.text_faint).child(if t == MateType::Tangent {
                            "Click a face, edge or vertex on each instance."
                        } else {
                            "Hover geometry to see inference points; click to place a mate connector. The origin and assembly mate connectors can be picked too."
                        }))
                    }),
            )
    }
}
