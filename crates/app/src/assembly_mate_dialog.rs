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
        _ => "Z distance",
    }
}

pub struct MateDialog {
    pub id: MateId,
    pub name: String,
    #[allow(dead_code)]
    pub is_new: bool,
    pub mate_type: MateType,
    pub a: Option<MateConnectorRef>,
    pub b: Option<MateConnectorRef>,
    /// Connector field receiving viewport picks (0 = first, 1 = second).
    pub active: Option<usize>,
    pub flip_primary: bool,
    pub reorient: u8,
    pub suppressed: bool,
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
        for key in ["rz", "x", "y", "z"] {
            let l = mate.limits.iter().find(|(k, _)| k == key).map(|(_, l)| l.clone());
            let unit = if key == "rz" { "deg" } else { "mm" };
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
        let active = if a.is_none() { Some(0) } else if b.is_none() { Some(1) } else { None };
        Self {
            id: mate.id,
            name: mate.name,
            is_new,
            mate_type: mate.mate_type,
            a,
            b,
            active,
            flip_primary: mate.flip_primary,
            reorient: mate.reorient_secondary,
            suppressed: mate.suppressed,
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

    pub fn awaiting_pick(&self) -> bool {
        self.active.is_some()
    }

    /// Fill the active connector field; returns true when accepted.
    pub fn set_connector(&mut self, c: MateConnectorRef, cx: &mut Context<Self>) -> bool {
        let Some(field) = self.active else { return false };
        // Both connectors must be on different instances.
        let other = if field == 0 { &self.b } else { &self.a };
        if other.as_ref().is_some_and(|o| o.instance == c.instance && o.path == c.path) {
            self.status = Some("Select a mate connector on a different instance".into());
            cx.notify();
            return false;
        }
        if field == 0 {
            self.a = Some(c);
        } else {
            self.b = Some(c);
        }
        self.status = None;
        self.active = if self.a.is_none() { Some(0) } else if self.b.is_none() { Some(1) } else { None };
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
            flip_primary: self.flip_primary,
            reorient_secondary: self.reorient,
            limits,
            suppressed: self.suppressed,
        })
    }

    pub fn flip(&mut self, cx: &mut Context<Self>) {
        self.flip_primary = !self.flip_primary;
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
            cx.emit(MateDialogEvent::Changed);
            cx.notify();
        }
    }

    fn connector_row(&self, field: usize, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let value = if field == 0 { &self.a } else { &self.b };
        let active = self.active == Some(field);
        let label: SharedString = match value {
            Some(c) => (self.labeler)(c).into(),
            None => "Select mate connector".into(),
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
            .child(svg().path(icon_path("mate-connector")).size_3p5().text_color(if filled { pal.accent_hover } else { pal.text_faint }))
            .child(div().flex_1().truncate().text_sm().text_color(if filled { pal.text } else { pal.text_faint }).child(label))
            .when(filled, |d| {
                d.child(Button::new(SharedString::from(format!("mc-clear-{field}"))).icon(IconName::Close).ghost().xsmall().on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if field == 0 {
                        this.a = None;
                    } else {
                        this.b = None;
                    }
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
                offset_rows = offset_rows.child(field("Rotation (Z)", &self.offsets[3]));
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
                        .child(h_flex().gap_1().child(div().flex_1().child(Input::new(lo).xsmall())).child(div().flex_1().child(Input::new(hi).xsmall()))),
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
                    .child(div().text_xs().text_color(pal.text_muted).child("Mate connectors"))
                    .child(self.connector_row(0, cx))
                    .child(self.connector_row(1, cx))
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
                            ),
                    )
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
                    .when_some(self.status.clone(), |d, s| d.child(div().text_xs().text_color(pal.warning).child(s)))
                    .when(self.active.is_some(), |d| {
                        d.child(div().text_xs().text_color(pal.text_faint).child("Hover geometry to see inference points; click to place a mate connector."))
                    }),
            )
    }
}
