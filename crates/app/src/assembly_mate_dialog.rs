//! Mate dialog (Fastened … Width): mate type selector, two mate connector
//! fields filled by clicking geometry in the viewport, flip primary /
//! reorient secondary, offsets and limits. Emits `Changed` for live preview.

use crate::assets::icon_path;
use crate::os_ui as os;
use crate::theme::p;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use crate::os_menu::OsMenuItem;
use crate::os_ui::OsDropdownExt as _;
use gpui_kit::component::{h_flex, v_flex};
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
    /// "Select parameters to configure" mode: a parameter was clicked.
    ConfigureParam(&'static str),
    Cancel,
}

pub type ConnectorLabeler = Arc<dyn Fn(&MateConnectorRef) -> String>;

pub fn mate_icon(t: MateType) -> SharedString {
    icon_path(mate_icon_name(t))
}

/// The app icon of a mate type.
pub fn mate_icon_name(t: MateType) -> &'static str {
    match t {
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
    }
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
    /// The connector field the Mate connector panel is editing (its chip is
    /// bold, live 578).
    pub editing_field: Option<usize>,
    pub reorient: u8,
    pub suppressed: bool,
    /// Tangent mates: contact may move onto tangent-connected faces.
    pub tangent_propagation: bool,
    /// Axis of the rotation offset (0 = X, 1 = Y, 2 = Z).
    pub rotation_axis: u8,
    /// Mate indicator hidden (kept when editing).
    hidden: bool,
    /// Dynamic suppression expression (kept when editing).
    unsuppressed_if: Option<String>,
    /// Animate preview running.
    pub animating: bool,
    /// Configure assembly features mode (live 194–196): clicking a
    /// parameter configures it; the configured ones (listed) get a dashed
    /// amber frame and an "Unsuppressed" checkbox is shown.
    pub configuring: Option<Vec<String>>,
    use_offset: bool,
    use_limits: bool,
    offsets: [Entity<crate::quantity_field::QuantityInput>; 4],
    limits: HashMap<&'static str, (Entity<InputState>, Entity<InputState>)>,
    labeler: ConnectorLabeler,
    /// The mate's regeneration error (the red title's tooltip).
    pub status: Option<String>,
    /// Slots on an instance whose part is gone, with the instance's name
    /// (live E15: red chips; Tangent's read "Missing entity of <instance>").
    pub lost: HashMap<usize, String>,
    units: noshape_units::UnitPrefs,
    variables: std::collections::BTreeMap<String, noshape_units::Quantity>,
    focus: FocusHandle,
    _subs: Vec<Subscription>,
    /// The mate type select (Onshape's os-select, live assembly/36).
    type_select: Entity<crate::os_select::OsSelect>,
}

impl EventEmitter<MateDialogEvent> for MateDialog {}

impl Focusable for MateDialog {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

/// What an unset limit field reads (live).
const NO_LIMIT: [&str; 3] = ["No minimum", "No maximum", "No maximum angle"];

fn text_of(e: &Option<ExprValue>) -> String {
    e.as_ref().map(|e| e.text.clone()).unwrap_or_default()
}

impl MateDialog {
    /// In "Select parameters to configure" mode a parameter row is a pick
    /// target (click: configure it); configured rows get a dashed amber
    /// frame (live 195–196).
    fn cfg_param(&self, param: &'static str, el: impl IntoElement, cx: &mut Context<Self>) -> AnyElement {
        let Some(list) = &self.configuring else { return el.into_any_element() };
        let on = list.iter().any(|p| p == param);
        div()
            .relative()
            .border_1()
            .when(on, |d| d.border_dashed().border_color(gpui_kit::rgb(0xfab601)))
            .when(!on, |d| d.border_color(gpui_kit::transparent_white()))
            .child(el)
            .child(
                div()
                    .id(SharedString::from(format!("mate-cfg-{param}")))
                    .occlude()
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .cursor_pointer()
                    .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| cx.emit(MateDialogEvent::ConfigureParam(param)))),
            )
            .into_any_element()
    }

    /// `units` and `variables` (the assembly's): the offset fields evaluate
    /// with them (Onshape's number field, `quantity_field`).
    pub fn new(mate: Mate, is_new: bool, labeler: ConnectorLabeler, units: noshape_units::UnitPrefs, variables: std::collections::BTreeMap<String, noshape_units::Quantity>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        // Live (qa5 MT5, dom/qa5-asm-036): "0 mm", "No minimum" are the
        // fields' values in regular text (their placeholder is empty).
        let mk = |text: String, default: &str, window: &mut Window, cx: &mut Context<Self>| {
            let text = if text.trim().is_empty() { default.to_string() } else { text };
            cx.new(|cx| InputState::new(window, cx).default_value(text))
        };
        // Live ("Input behaviour outside feature dialogs", Fastened mate ›
        // Offset X): Onshape's number field: all selected on click, "5" →
        // "5 mm", "1+1" → "(1+1) mm", invalid pink, ↑ / ↓ ±1 mm, Esc reverts.
        use crate::quantity_field::{QuantityInput, QuantitySpec};
        let qty = |text: String, default: &str, dim: noshape_units::Dim, label: &'static str, window: &mut Window, cx: &mut Context<Self>| {
            let text = if text.trim().is_empty() { default.to_string() } else { text };
            let vars = variables.clone();
            cx.new(|cx| {
                let mut q = QuantityInput::new(text, QuantitySpec::quantity(dim).label(label), units, window, cx);
                q.set_variables(vars, cx);
                q
            })
        };
        let len = noshape_units::Dim::LENGTH;
        let offsets = [
            qty(text_of(&mate.offset_x), "0 mm", len, "Offset X", window, cx),
            qty(text_of(&mate.offset_y), "0 mm", len, "Offset Y", window, cx),
            qty(text_of(&mate.offset_z), "0 mm", len, "Offset Z", window, cx),
            qty(text_of(&mate.rotation), "0 deg", noshape_units::Dim::ANGLE, "Rotation angle", window, cx),
        ];
        let mut limits = HashMap::new();
        for key in ["rz", "x", "y", "z", "cone"] {
            let l = mate.limits.iter().find(|(k, _)| k == key).map(|(_, l)| l.clone());
            // Onshape's empty limit fields read "No minimum" / "No maximum".
            let lo = mk(l.as_ref().map(|l| text_of(&l.min)).unwrap_or_default(), "No minimum", window, cx);
            let hi = mk(l.as_ref().map(|l| text_of(&l.max)).unwrap_or_default(), if key == "cone" { "No maximum angle" } else { "No maximum" }, window, cx);
            limits.insert(key, (lo, hi));
        }
        let mut subs = Vec::new();
        // A committed offset previews; Enter with nothing new to apply
        // accepts the mate (like a feature dialog).
        for q in &offsets {
            subs.push(cx.subscribe(q, |_: &mut Self, _, ev: &crate::quantity_field::QuantityEvent, cx| match ev {
                crate::quantity_field::QuantityEvent::Committed(_) => cx.emit(MateDialogEvent::Changed),
                crate::quantity_field::QuantityEvent::Enter => cx.emit(MateDialogEvent::Accept),
                _ => {}
            }));
        }
        let all: Vec<Entity<InputState>> = limits.values().flat_map(|(a, b)| [a.clone(), b.clone()]).collect();
        for s in all {
            subs.push(cx.subscribe_in(&s, window, |_: &mut Self, _, ev: &InputEvent, _, cx| match ev {
                InputEvent::Change => cx.emit(MateDialogEvent::Changed),
                InputEvent::PressEnter { .. } => cx.emit(MateDialogEvent::Accept),
                _ => {}
            }));
        }
        let type_select = cx.new(|_| crate::os_select::OsSelect::new("mate-type"));
        subs.push(cx.subscribe_in(&type_select, window, |this: &mut Self, _, ev: &crate::os_select::OsSelectEvent, _, cx| {
            let crate::os_select::OsSelectEvent::Picked(id) = ev;
            if let Some(mt) = MATE_TYPE_ORDER.iter().copied().find(|mt| mt.name() == id.as_ref()) {
                this.set_type(mt, cx);
            }
        }));
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
            editing_field: None,
            reorient: mate.reorient_secondary,
            suppressed: mate.suppressed,
            tangent_propagation: mate.tangent_propagation,
            rotation_axis: mate.rotation_axis,
            hidden: mate.hidden,
            unsuppressed_if: mate.unsuppressed_if.clone(),
            animating: false,
            configuring: None,
            use_offset,
            use_limits,
            offsets,
            limits,
            labeler,
            status: None,
            lost: HashMap::new(),
            units,
            variables,
            focus: cx.focus_handle(),
            _subs: subs,
            type_select,
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

    /// The mate type being edited (its toolbar button shows as active).
    pub fn mate_type(&self) -> MateType {
        self.mate_type
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
                return false;
            }
        } else {
            // Live (E1): a second pick on the same instance is not accepted;
            // the title keeps "Mate needs two mate connectors.".
            let other = if field == 0 { &self.b } else { &self.a };
            if same(other) {
                return false;
            }
        }
        *self.slot_mut(field) = Some(c);
        self.active = self.next_active();
        cx.emit(MateDialogEvent::Changed);
        cx.notify();
        true
    }

    fn expr(state: &Entity<InputState>, cx: &App) -> Option<ExprValue> {
        let t = state.read(cx).value().to_string();
        let unset = t.trim().is_empty() || NO_LIMIT.contains(&t.trim());
        (!unset).then(|| ExprValue::new(t))
    }

    /// Limit keys whose minimum is above the maximum (live E11: both inputs pink).
    fn crossed_limits(&self, cx: &App) -> Vec<&'static str> {
        if !self.use_limits {
            return Vec::new();
        }
        limit_keys(self.mate_type)
            .iter()
            .copied()
            .filter(|key| {
                let dim = if matches!(*key, "rz" | "cone") { noshape_units::Dim::ANGLE } else { noshape_units::Dim::LENGTH };
                let (lo, hi) = &self.limits[key];
                let v = |st: &Entity<InputState>| Self::expr(st, cx).and_then(|e| noshape_units::evaluate_as(&e.text, dim, &self.units, &self.variables).ok()).map(|q| q.value);
                matches!((v(lo), v(hi)), (Some(a), Some(b)) if a > b)
            })
            .collect()
    }

    /// Live's title tooltip text: "<name> did not regenerate properly: <message>".
    fn regen_error(&self, cx: &App) -> Option<String> {
        let incomplete = match self.mate_type {
            MateType::Tangent if self.a.is_none() || self.b.is_none() => Some("Tangent mate needs two entities."),
            MateType::Width if self.a.is_none() || self.b.is_none() || self.c.is_none() => Some("Two width mate connectors are required."),
            MateType::Tangent | MateType::Width => None,
            _ if self.a.is_none() || self.b.is_none() => Some("Mate needs two mate connectors."),
            _ => None,
        };
        let msg = incomplete.map(str::to_string).or_else(|| (!self.crossed_limits(cx).is_empty()).then(|| "Minimum mate limit should be smaller than maximum limit. A parameter value is not valid.".to_string())).or_else(|| self.status.clone())?;
        Some(format!("{} did not regenerate properly: {msg}", self.name))
    }

    /// The mate as currently defined (None until both connectors are set).
    pub fn to_mate(&self, cx: &App) -> Option<Mate> {
        let (a, b) = (self.a.clone()?, self.b.clone()?);
        let extra = if self.mate_type == MateType::Width { std::iter::once(self.c.clone()?).chain(self.d.clone()).collect() } else { vec![] };
        let (ox, oy, oz, orot) = offset_axes(self.mate_type);
        let off = |i: usize, allowed: bool| {
            let t = self.offsets[i].read(cx).text();
            (self.use_offset && allowed && !t.trim().is_empty()).then(|| ExprValue::new(t.to_string()))
        };
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
            unsuppressed_if: self.unsuppressed_if.clone(),
        })
    }

    /// The primary-axis alignment chosen when both connectors of a new mate
    /// are picked (`flip_primary`: true = the Z axes agree).
    pub fn set_flip(&mut self, flip_primary: bool, cx: &mut Context<Self>) {
        if self.flip_primary != flip_primary {
            self.flip_primary = flip_primary;
            cx.emit(MateDialogEvent::Changed);
            cx.notify();
        }
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

    pub fn set_type(&mut self, t: MateType, cx: &mut Context<Self>) {
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

    /// One Onshape selection box over some connector slots: the filled
    /// slots are its rows; it is the active box while one of them takes the
    /// next pick.
    fn query_box(&self, id: &'static str, label: &'static str, fields: &[usize], cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let active = self.active.is_some_and(|a| fields.contains(&a));
        let mut rows = Vec::new();
        for &field in fields {
            let Some(c) = self.slot(field) else { continue };
            let lost = self.lost.get(&field);
            let text: SharedString = match lost {
                Some(inst) if self.mate_type == MateType::Tangent => format!("Missing entity of {inst}").into(),
                _ => (self.labeler)(c).into(),
            };
            let text_del = text.clone();
            // Live (qa5 MT3): "Mate connector of Part 1 <3> (Click to edit)".
            let edit_tip = format!("{text} (Click to edit)");
            let bold = self.editing_field == Some(field);
            rows.push(
                os::query_row(SharedString::from(format!("mc-field-{field}")), text)
                    .when(bold, |d| d.font_weight(FontWeight::BOLD))
                    .when(lost.is_some(), |d| d.text_color(pal.text_invalid))
                    .hover(|d| d.bg(pal.row_hover))
                    // The connector glyph opens the connector for editing.
                    .when(self.mate_type != MateType::Tangent, |d| {
                        d.child(
                            div()
                                .id(SharedString::from(format!("mc-edit-{field}")))
                                .tooltip(crate::ui::tooltip(edit_tip))
                                // Live: `mate-connector-qlv-item`, 15 px.
                                .child(crate::assets::icon("mate-connector-qlv-item", 15., cx))
                                .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| {
                                    cx.stop_propagation();
                                    cx.emit(MateDialogEvent::EditConnector(field));
                                })),
                        )
                    })
                    .map(|d| {
                        d.child(
                            div()
                                .id(SharedString::from(format!("mc-clear-{field}")))
                                .tooltip(crate::ui::tooltip(format!("Delete {text_del}")))
                                // Live: a 16.67 px "×" (#666), 5 px padding.
                                .px(px(5.))
                                .text_size(px(16.67))
                                .text_color(gpui_kit::rgb(0x666666))
                                .child("×")
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    cx.stop_propagation();
                                    *this.slot_mut(field) = None;
                                    this.active = Some(field);
                                    cx.emit(MateDialogEvent::Changed);
                                    cx.notify();
                                })),
                        )
                    })
                    .into_any_element(),
            );
        }
        let swap = fields == [0, 1] && self.a.is_some() && self.b.is_some() && self.mate_type != MateType::Tangent;
        let fields = fields.to_vec();
        let qbox = os::query_box(id, label, active, rows).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
            this.active = fields.iter().copied().find(|f| this.slot(*f).is_none()).or(fields.first().copied());
            cx.notify();
        }));
        // Onshape's ⇅ in the box header swaps the two connectors (which one
        // flips / reorients and which frame the offset is measured in).
        div()
            .relative()
            .child(qbox)
            .when(swap, |d| {
                d.child(
                    div()
                        .id("mate-swap")
                        .absolute()
                        .top(px(1.))
                        .right(px(5.))
                        .cursor_pointer()
                        // Live: `os-param-query-list-reorder-button`, the 20 px
                        // reorder-items icon at opacity 0.5, "Reorder items".
                        .opacity(0.5)
                        .tooltip(crate::ui::tooltip("Reorder items"))
                        .child(crate::assets::icon("reorder-items-button", 20., cx))
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            cx.stop_propagation();
                            std::mem::swap(&mut this.a, &mut this.b);
                            this.active = this.next_active();
                            cx.emit(MateDialogEvent::Changed);
                            cx.notify();
                        })),
                )
            })
            .into_any_element()
    }
}

impl Render for MateDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let t = self.mate_type;
        // Mate type: Onshape's select (live assembly/36): open, the list
        // hangs under the emptied field, plain rows, the current type blue.
        let types: Vec<MateType> = if MATE_TYPE_ORDER.contains(&t) { MATE_TYPE_ORDER.to_vec() } else { vec![t] };
        self.type_select.update(cx, |sel, _| sel.set_items(types.iter().map(|mt| (SharedString::from(mt.name()), SharedString::from(mt.name()))).collect(), t.name()));
        let type_field = div().w(px(194.)).child(self.type_select.clone());
        let (ox, oy, oz, orot) = offset_axes(t);
        let has_offset = ox || oy || oz || orot;
        let lkeys = limit_keys(t);
        let mut offset_rows = v_flex();
        if self.use_offset {
            // Live (dom/dialog-mate-fastened-offset.txt): per axis a 26 px
            // "along X/Y/Z" icon and a 168 px quantity field; then the
            // "Rotate about X ▾" select (tooltip "Rotation axis") and the
            // "Rotation angle" row (12 px #666 label, 108 px field).
            let axis_row = |icon: &'static str, st: &Entity<crate::quantity_field::QuantityInput>, cx: &App| {
                h_flex()
                    .w(px(194.))
                    .h(px(29.))
                    .child(div().size(px(26.)).flex_none().flex().items_center().justify_center().child(crate::assets::icon(icon, 26., cx)))
                    .child(div().w(px(168.)).h(px(26.)).child(st.clone()))
            };
            if ox {
                offset_rows = offset_rows.child(self.cfg_param(noshape_model::assembly::config::param::OFFSET_X, axis_row("xyz-along-x", &self.offsets[0], cx), cx));
            }
            if oy {
                offset_rows = offset_rows.child(self.cfg_param(noshape_model::assembly::config::param::OFFSET_Y, axis_row("xyz-along-y", &self.offsets[1], cx), cx));
            }
            if oz {
                offset_rows = offset_rows.child(self.cfg_param(noshape_model::assembly::config::param::OFFSET_Z, axis_row("xyz-along-z", &self.offsets[2], cx), cx));
            }
            if orot {
                let ax = self.rotation_axis;
                let label = ["Rotate about X", "Rotate about Y", "Rotate about Z"][ax.min(2) as usize];
                let view = cx.entity();
                let select = div()
                    .id("mate-rot-axis")
                    .w(px(194.))
                    .h(px(26.))
                    .pl(px(6.))
                    .pr(px(6.))
                    .flex()
                    .items_center()
                    .border_b_1()
                    .border_color(pal.border)
                    .cursor_pointer()
                    .tooltip(crate::ui::tooltip("Rotation axis"))
                    .child(h_flex().w_full().child(div().flex_1().text_size(px(14.)).text_color(pal.text).child(label)).child(os::glyph(os::GLYPH_CARET, 8., gpui_kit::rgb(0x333333))))
                    .os_dropdown_menu("mate-rot-axis-dd", true, move |mut menu, _, _| {
                        for (k, l) in ["Rotate about X", "Rotate about Y", "Rotate about Z"].into_iter().enumerate() {
                            let v = view.clone();
                            menu = menu.item(OsMenuItem::new(l).on_click(move |_, _, cx| {
                                v.update(cx, |this, cx| {
                                    this.rotation_axis = k as u8;
                                    cx.emit(MateDialogEvent::Changed);
                                    cx.notify();
                                })
                            }));
                        }
                        menu
                    });
                let angle = h_flex()
                    .w(px(194.))
                    .h(px(26.))
                    .child(div().w(px(86.)).h_full().pl(px(5.)).pt(px(7.)).text_size(px(12.)).text_color(gpui_kit::rgb(0x666666)).child("Rotation angle"))
                    .child(div().w(px(108.)).h(px(26.)).child(self.offsets[3].clone()));
                offset_rows = offset_rows.child(div().py(px(1.5)).child(select)).child(self.cfg_param(noshape_model::assembly::config::param::ROTATION, angle, cx));
            }
        }
        let mut limit_rows = v_flex();
        if self.use_limits {
            // Onshape (dom/dialog-mate-revolute-limits): per limit one 26 px
            // icon (axis letter + min / max glyph in the axis colour: X red,
            // Y green, Z blue), the right-aligned field ("No minimum" /
            // "No maximum") right after it.
            let crossed = self.crossed_limits(cx);
            let limit_field = |key: &'static str, max: bool, st: &Entity<InputState>, cx: &App| {
                let icon = match (key, max) {
                    ("cone", _) => "mate-limit-cone",
                    ("rz", false) => "mate-limit-z-rot-min",
                    ("rz", true) => "mate-limit-z-rot-max",
                    ("x", false) => "mate-limit-x-lin-min",
                    ("x", true) => "mate-limit-x-lin-max",
                    ("y", false) => "mate-limit-y-lin-min",
                    ("y", true) => "mate-limit-y-lin-max",
                    (_, false) => "mate-limit-z-lin-min",
                    (_, true) => "mate-limit-z-lin-max",
                };
                h_flex()
                    .id(SharedString::from(format!("mate-limit-{key}-{max}")))
                    // Live: a 194 x 29 row, the field ends 26 px before the dialog's edge.
                    .w(px(194.))
                    .h(px(29.))
                    .tooltip(crate::ui::tooltip(format!("{} {}", if max { "Maximum" } else { "Minimum" }, limit_label(key))))
                    .child(crate::assets::icon(&icon_path(icon), 26., cx))
                    .child(gpui_kit::base::input::input_frame(
                        div()
                            .flex_1()
                            .border_b_1()
                            .border_color(pal.border)
                            // Live E11: min above max turns both pink (#f9ebec, 2 px #bd3039).
                            .when(crossed.contains(&key), |d| d.bg(gpui_kit::rgb(0xf9ebec)).border_b_2().border_color(gpui_kit::rgb(0xbd3039)))
                            .child(crate::os_ui::input_h(Input::new(st).appearance(false).text_size(px(13.33)).px(px(3.)).text_right(), 25.)),
                    ))
            };
            for key in lkeys {
                let (lo, hi) = &self.limits[key];
                if *key != "cone" {
                    limit_rows = limit_rows.child(limit_field(key, false, lo, cx));
                }
                // Ball: a single maximum angle from the Z axis.
                limit_rows = limit_rows.child(limit_field(key, true, hi, cx));
            }
        }
        let regen_error = self.regen_error(cx);
        let invalid = self.to_mate(cx).is_none();
        let query_boxes = match t {
            // Live (dom/dialog-mate-tangent-*): one "Face, edge or vertex" field per entity.
            MateType::Tangent => vec![self.query_box("mate-q-ent-a", "Face, edge or vertex", &[0], cx), self.query_box("mate-q-ent-b", "Face, edge or vertex", &[1], cx)],
            MateType::Width => vec![self.query_box("mate-q-tab", "Tab mate connector(s)", &[0, 3], cx), self.query_box("mate-q-width", "Width mate connectors", &[1, 2], cx)],
            _ => vec![self.query_box("mate-q-mc", "Mate connectors", &[0, 1], cx)],
        };
        // Live dialog-mate-revolute: flip-direction-opposite, realign-mate-dialog.
        // Live (dom/dialog-mate-fastened-empty / -filled): the flip is a
        // boolean icon parameter shown by its icon, never pressed:
        // flip-direction while Onshape's flip flag is off, -opposite when on.
        // Live (qa5 MT1): until both connectors are picked the icon points
        // up-right, whatever the flag.
        // Live dom/dialog-mate-tangent-empty / -filled: Tangent shows the flag
        // alone (-opposite in both).
        let flip_shown = self.flip_primary || (t != MateType::Tangent && (self.a.is_none() || self.b.is_none()));
        let full_row = !matches!(t, MateType::Tangent | MateType::Width);
        let (flip_icon, reorient_icon) = (crate::assets::icon(flip_icon_name(flip_shown), 20., cx), crate::assets::icon("realign-mate-dialog", 20., cx));
        let small_tool = |id: &'static str, icon: AnyElement, tip: &'static str, on: bool| {
            div()
                .id(id)
                .size(px(26.))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .border_b_1()
                .border_color(gpui_kit::transparent_white())
                .hover(|d| d.bg(pal.panel_hover).border_color(pal.accent))
                .when(on, |d| d.bg(pal.panel_active))
                .tooltip(crate::ui::tooltip(tip))
                .child(icon)
        };
        v_flex()
            .id("mate-dialog")
            .key_context("FeatureDialog")
            .track_focus(&self.focus)
            .w(px(220.))
            // Onshape feature dialog frame (§3.9): white at 95%, no border or
            // radius, shadow 0 5 10 rgba(0,0,0,.2).
            .bg(pal.dialog_bg)
            .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
            .text_color(pal.text)
            .on_action(cx.listener(|_, _: &crate::actions::Accept, _, cx| cx.emit(MateDialogEvent::Accept)))
            .on_action(cx.listener(|_, _: &crate::actions::Cancel, _, cx| cx.emit(MateDialogEvent::Cancel)))
            .on_action(cx.listener(|this, _: &crate::actions::FlipPrimaryAxis, _, cx| this.flip(cx)))
            .on_action(cx.listener(|_, _: &crate::actions::AcceptAndRepeat, _, cx| cx.emit(MateDialogEvent::AcceptAndNew)))
            .child(
                h_flex()
                    .h(px(29.))
                    .bg(pal.panel)
                    .border_b_1()
                    .border_color(pal.border)
                    .child(
                        div()
                            .id("mate-title")
                            .flex_1()
                            .pl(px(6.))
                            .text_size(px(13.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(if regen_error.is_some() { pal.text_invalid } else { pal.text })
                            .truncate()
                            .child(self.name.clone())
                            .when_some(regen_error, |d, t| d.tooltip(crate::ui::tooltip(t))),
                    )
                    // Configure mode: only ✕ (live 194).
                    .when(self.configuring.is_none(), |d| d.child(
                        div()
                            .id("mate-ok")
                            .size(px(28.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(pal.accept).when(invalid, crate::ui::disabled_accept)
                            .when(!invalid, |d| d.hover(|d| d.bg(gpui_kit::rgb(0x74ce3a))))
                            .cursor_pointer()
                            .child(crate::assets::icon("dialog-ok", 24., cx))
                            .on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(MateDialogEvent::Accept))),
                    ))
                    .child(
                        div()
                            .id("mate-cancel")
                            .size(px(28.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .hover(|d| d.bg(pal.panel_hover))
                            .cursor(gpui_kit::CursorStyle::Arrow).child(crate::assets::icon("dialog-cancel", 24., cx))
                            .on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(MateDialogEvent::Cancel))),
                    ),
            )
            .child(
                // Live (dom/assembly-revolute-dialog-both.txt): the select flush
                // at the top-left (28 px row), each query box 3 px in and 5 px
                // below, 5 px before the 27 px checkbox rows (no gaps).
                v_flex()
                    .pb(px(2.))
                    // Live dom/dialog-mate-tangent-*, -width-*: no mate type select.
                    .when(full_row, |d| d.child(div().h(px(28.)).child(type_field)))
                    .children(query_boxes.into_iter().map(|q| div().px(px(3.)).pt(px(5.)).child(q)))
                    .child(div().h(px(5.)))
                    .when(t == MateType::Tangent, |d| {
                        d.child(os::checkbox("mate-tangent-prop", "Tangent propagation", self.tangent_propagation).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.tangent_propagation = !this.tangent_propagation;
                            cx.emit(MateDialogEvent::Changed);
                            cx.notify();
                        })))
                    })
                    .when(has_offset, |d| {
                        let offset = os::checkbox("mate-offset", "Offset", self.use_offset).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.use_offset = !this.use_offset;
                            cx.emit(MateDialogEvent::Changed);
                            cx.notify();
                        }));
                        d.child(self.cfg_param(noshape_model::assembly::config::param::OFFSET, offset, cx)).when(self.use_offset, |d| d.child(offset_rows))
                    })
                    .when(!lkeys.is_empty(), |d| {
                        d.child(os::checkbox("mate-limits", "Limits", self.use_limits).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.use_limits = !this.use_limits;
                            cx.emit(MateDialogEvent::Changed);
                            cx.notify();
                        })))
                        .when(self.use_limits, |d| d.child(limit_rows))
                    })
                    // Onshape's bottom row: flip primary axis, reorient secondary
                    // axis, animate, Solve; the help ? at the right. Live
                    // dom/dialog-mate-tangent-*: Tangent has only the flip (26 px
                    // row); dialog-mate-width-*: Width has no tools, only the help
                    // row (os-help-button-row-spacer, 22 px).
                    .child(
                        h_flex()
                            .h(px(match t { MateType::Width => 22., MateType::Tangent => 26., _ => 30. }))
                            .px(px(3.))
                            .gap(px(4.))
                            .when(t != MateType::Width, |d| d.child(small_tool("mate-flip", flip_icon, "Flip primary axis (a)", false).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.flip(cx)))))
                            .when(full_row, |d| d.child(small_tool("mate-reorient", reorient_icon, "Reorient secondary axis", !self.reorient.is_multiple_of(4)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.reorient = (this.reorient + 1) % 4;
                                cx.emit(MateDialogEvent::Changed);
                                cx.notify();
                            }))))
                            // Animate: preview the mate's motion (mates with degrees of freedom).
                            .when(full_row, |d| {
                                let can = !noshape_model::assembly::eval::mate_dofs(t).is_empty();
                                d.child(
                                    div()
                                        .id("mate-animate")
                                        .size(px(26.))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .cursor_pointer()
                                        .hover(|d| d.bg(pal.panel_hover))
                                        .when(self.animating, |d| d.bg(pal.panel_active))
                                        // Live: `ns-mate-play`, the play-dialog icon.
                                        .tooltip(crate::ui::tooltip("Animate mate degree of freedom"))
                                        .child(crate::assets::icon("play-dialog", 20., cx))
                                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                            if !can {
                                                return;
                                            }
                                            this.animating = !this.animating;
                                            cx.emit(MateDialogEvent::Animate);
                                            cx.notify();
                                        })),
                                )
                            })
                            .when(full_row, |d| d.child(
                                // Live dom dialog-mate-fastened-filled: 52 x 21, 1 px
                                // `#aaa`, radius 3.5, 12.25 px.
                                div()
                                    .id("mate-solve")
                                    .h(px(21.))
                                    .px(px(10.))
                                    .flex()
                                    .items_center()
                                    .bg(gpui_kit::rgb(0xffffff))
                                    .border_1()
                                    .border_color(gpui_kit::rgb(0xaaaaaa))
                                    .rounded(px(3.5))
                                    .text_size(px(12.25))
                                    .text_color(crate::theme::p().text)
                                    .cursor_pointer()
                                    .hover(|d| d.bg(gpui_kit::rgb(0xe6e6e6)))
                                    .child("Solve")
                                    .tooltip(crate::ui::tooltip("Solve all mates including this one"))
                                    .on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(MateDialogEvent::Solve))),
                            ))
                            .child(div().flex_1())
                            .child(
                                div()
                                    .size(px(14.))
                                    .when(t == MateType::Tangent, |d| d.mt(px(4.)))
                                    .mr(px(4.))
                                    .rounded_full()
                                    
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_size(px(10.))
                                    .font_weight(FontWeight::BOLD)
                                    
                                    .child(crate::ui::glyph("help-dot", 15.6, crate::theme::help_dot(0x999999))),
                            ),
                    )
                    // Configure mode: the mate's suppression as a checkbox (live 195).
                    .when(self.configuring.is_some(), |d| {
                        let unsup = os::checkbox("mate-unsuppressed", "Unsuppressed", !self.suppressed);
                        d.child(self.cfg_param(noshape_model::assembly::config::param::UNSUPPRESSED, unsup, cx))
                    }),
            )
    }
}

/// Onshape's order in the mate type select (live behaviours: Slider,
/// Cylindrical, Revolute, Pin slot, Planar, Ball, Fastened, Parallel);
/// Tangent and Width are separate tools.
const MATE_TYPE_ORDER: [MateType; 8] = [MateType::Slider, MateType::Cylindrical, MateType::Revolute, MateType::PinSlot, MateType::Planar, MateType::Ball, MateType::Fastened, MateType::Parallel];

/// The Flip primary axis icon for a mate's alignment (`flip_primary`: the
/// connectors' Z axes agree, which is Onshape's flip flag off).
pub(crate) fn flip_icon_name(flip_primary: bool) -> &'static str {
    if flip_primary { "flip-direction" } else { "flip-direction-opposite" }
}
