//! Assembly tab panels and dialogs (child module of `assembly_view`):
//! assembly mate connector dialog, pattern / mirror / replicate dialog,
//! triad typed values, exploded views (step editor, playback), named
//! positions, per-instance configuration, replace instance, mode banners
//! and the extra list rows (origin, connectors, patterns).

use super::*;
use gpui_kit::component::input::InputEvent;
use gpui_kit::component::slider::{Slider, SliderEvent, SliderState};
use noshape_model::configuration::ConfigInputKind;
use noshape_units::{Dim, EmptyScope, Quantity};

/// What a viewport click picks while a dialog field is active.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PickMode {
    /// Implicit mate connector inference points (plus origin / assembly connectors).
    Inference,
    /// Bare faces, edges and vertices (tangent mates).
    Entity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PatternDialogKind {
    Linear,
    Circular,
    Mirror,
    PartPattern,
    Replicate,
}

impl PatternDialogKind {
    fn title(self) -> &'static str {
        match self {
            PatternDialogKind::Linear => "Linear pattern",
            PatternDialogKind::Circular => "Circular pattern",
            PatternDialogKind::Mirror => "Mirror",
            PatternDialogKind::PartPattern => "Part pattern",
            PatternDialogKind::Replicate => "Replicate",
        }
    }
    fn matched(self) -> bool {
        matches!(self, PatternDialogKind::PartPattern | PatternDialogKind::Replicate)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RenameTarget {
    Position(usize),
    Explode(usize),
}

pub(crate) struct McSession {
    pub id: MateId,
    pub editing: bool,
    pub name: String,
    pub on: Option<MateConnectorRef>,
    /// The origin entity field receives viewport picks.
    pub active: bool,
    pub flip: bool,
    pub reorient: u8,
    pub rot_axis: u8,
    pub hidden: bool,
    /// X / Y / Z translation and rotation angle.
    pub inputs: [Entity<InputState>; 4],
    /// Opened from a mate dialog's connector field (the mate keeps it).
    pub for_mate: Option<usize>,
    /// Field receiving picks while `active`: 0 origin entity, 1 between
    /// entity, 2 primary axis entity, 3 secondary axis entity.
    pub slot: usize,
    /// Origin type Between entities, and Realign.
    pub between: bool,
    pub realign: bool,
    /// Between entity, primary and secondary axis entities.
    pub extra: [Option<MateConnectorRef>; 3],
    _subs: Vec<Subscription>,
}

impl McSession {
    pub fn to_connector(&self, cx: &App) -> Option<AsmMateConnector> {
        let on = self.on.clone()?;
        let expr = |i: usize| {
            let t = self.inputs[i].read(cx).value().to_string();
            (!t.trim().is_empty()).then(|| ExprValue::new(t))
        };
        let mut c = AsmMateConnector::new(self.id, self.name.clone(), on);
        c.flip_primary = self.flip;
        c.reorient = self.reorient;
        c.translation = [expr(0), expr(1), expr(2)];
        c.rotation_axis = self.rot_axis;
        c.rotation = expr(3);
        c.hidden = self.hidden;
        if self.between {
            c.between = self.extra[0].clone();
        }
        if self.realign {
            c.primary_axis = self.extra[1].clone();
            c.secondary_axis = self.extra[2].clone();
        }
        Some(c)
    }
}

pub(crate) struct PatternSession {
    pub kind: PatternDialogKind,
    pub editing: Option<MateId>,
    pub name: String,
    pub seeds: Vec<InstanceId>,
    pub axis: Option<MateConnectorRef>,
    pub axis2: Option<MateConnectorRef>,
    /// Connector field receiving picks (0 = direction / axis / plane, 1 = second direction).
    pub active: Option<usize>,
    pub distance: Entity<InputState>,
    pub count: Entity<InputState>,
    pub distance2: Entity<InputState>,
    pub count2: Entity<InputState>,
    pub opposite: bool,
    pub opposite2: bool,
    pub equal: bool,
    pub second: bool,
    /// Seed mate (part pattern / replicate).
    pub mate: Option<MateId>,
    /// Matches found (part pattern / replicate) or why there are none.
    pub preview: Option<Result<usize, String>>,
}

pub(crate) struct ManipValue {
    pub handle: ManipHandle,
    pub axis: DVec3,
    pub center: DVec3,
    pub ids: Vec<InstanceId>,
    pub start: HashMap<InstanceId, Placement>,
    pub explode: bool,
    pub input: Entity<InputState>,
    _sub: Subscription,
}

pub(crate) struct ConfigSession {
    pub instance: InstanceId,
    /// Configuration inputs of the instance's Part Studio.
    pub inputs: Vec<noshape_model::configuration::ConfigInput>,
    pub values: ConfigValues,
    pub texts: Vec<(String, Entity<InputState>)>,
}

pub(crate) struct ReplaceSession {
    pub instance: InstanceId,
    pub all: bool,
}

fn section(label: &str) -> Div {
    div().pt_1().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(p().text_muted).child(label.to_string())
}

fn field_row(label: &str, el: impl IntoElement) -> Div {
    h_flex().gap_2().child(div().w(px(84.)).text_xs().text_color(p().text_muted).child(label.to_string())).child(div().flex_1().child(el))
}

impl AssemblyView {
    // ------------------------------------------------------------ connector fields

    /// A mate connector field (click to make it receive picks).
    fn conn_field(&self, id: &'static str, value: &Option<MateConnectorRef>, active: bool, placeholder: &'static str, cx: &mut Context<Self>, on_click: impl Fn(&mut Self, &mut Context<Self>) + 'static) -> AnyElement {
        let pal = p();
        let label: SharedString = match value {
            Some(c) => (self.labeler(cx))(c).into(),
            None => placeholder.into(),
        };
        let filled = value.is_some();
        h_flex()
            .id(id)
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
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                on_click(this, cx);
                this.update_capture(cx);
                this.refresh_scene(false, cx);
                cx.notify();
            }))
            .into_any_element()
    }

    // ------------------------------------------------------------ assembly mate connector dialog

    pub(super) fn open_mc(&mut self, existing: Option<AsmMateConnector>, window: &mut Window, cx: &mut Context<Self>) {
        self.close_mate(window, cx);
        self.open_mc_inner(existing, None, window, cx);
    }

    /// Mate dialog: the mate connector icon of a field opens its mate
    /// connector for editing (an implicit one becomes an assembly mate
    /// connector on the same entity); the mate dialog stays open.
    pub(super) fn edit_dialog_connector(&mut self, field: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(c) = self.mate.as_ref().and_then(|m| m.dialog.read(cx).field(field).cloned()) else { return };
        if c.is_origin() {
            return;
        }
        let existing = match c.connector {
            Some(id) => self.assembly(cx).and_then(|a| a.connector(id).cloned()),
            None => {
                let Some(asm) = self.assembly(cx) else { return };
                let mut on = c.clone();
                on.flip = false;
                on.rotate_quarters = 0;
                let mut mc = AsmMateConnector::new(MateId(asm.next_id()), asm.next_feature_name("Mate connector"), on);
                mc.flip_primary = c.flip;
                mc.reorient = c.rotate_quarters;
                self.open_mc_inner(None, Some(field), window, cx);
                if let Some(m) = &mut self.mc {
                    m.id = mc.id;
                    m.name = mc.name.clone();
                    m.on = Some(mc.on.clone());
                    m.active = false;
                    m.flip = mc.flip_primary;
                    m.reorient = mc.reorient;
                }
                self.schedule_eval(window, cx);
                return;
            }
        };
        if existing.is_some() {
            self.open_mc_inner(existing, Some(field), window, cx);
        } else {
            self.message = Some((false, "Mate connectors from Part Studios or subassemblies are edited where they were created.".into()));
            cx.notify();
        }
    }

    fn open_mc_inner(&mut self, existing: Option<AsmMateConnector>, for_mate: Option<usize>, window: &mut Window, cx: &mut Context<Self>) {
        self.mc = None;
        let Some(asm) = self.assembly(cx) else { return };
        let (id, name, editing) = match &existing {
            Some(c) => (c.id, c.name.clone(), true),
            None => (MateId(asm.next_id()), asm.next_feature_name("Mate connector"), false),
        };
        let text = |e: &Option<ExprValue>| e.as_ref().map(|e| e.text.clone()).unwrap_or_default();
        let init = match &existing {
            Some(c) => [text(&c.translation[0]), text(&c.translation[1]), text(&c.translation[2]), text(&c.rotation)],
            None => Default::default(),
        };
        let phs = ["0 mm", "0 mm", "0 mm", "0 deg"];
        let inputs: [Entity<InputState>; 4] = std::array::from_fn(|i| {
            let (t, ph) = (init[i].clone(), phs[i]);
            cx.new(|cx| InputState::new(window, cx).placeholder(ph).default_value(t))
        });
        let mut subs = Vec::new();
        for st in &inputs {
            subs.push(cx.subscribe_in(st, window, |this: &mut Self, _, ev: &InputEvent, window, cx| match ev {
                InputEvent::Change => this.schedule_eval(window, cx),
                InputEvent::PressEnter { .. } => this.accept_mc(window, cx),
                _ => {}
            }));
        }
        let active = existing.is_none();
        self.mc = Some(McSession {
            id,
            editing,
            name,
            on: existing.as_ref().map(|c| c.on.clone()),
            active,
            flip: existing.as_ref().is_some_and(|c| c.flip_primary),
            reorient: existing.as_ref().map(|c| c.reorient).unwrap_or(0),
            rot_axis: existing.as_ref().map(|c| c.rotation_axis).unwrap_or(2),
            hidden: existing.as_ref().is_some_and(|c| c.hidden),
            inputs,
            for_mate,
            slot: 0,
            between: existing.as_ref().is_some_and(|c| c.between.is_some()),
            realign: existing.as_ref().is_some_and(|c| c.primary_axis.is_some() || c.secondary_axis.is_some()),
            extra: existing.as_ref().map(|c| [c.between.clone(), c.primary_axis.clone(), c.secondary_axis.clone()]).unwrap_or_default(),
            _subs: subs,
        });
        self.insert_open = false;
        self.update_capture(cx);
        self.schedule_eval(window, cx);
        cx.notify();
    }

    pub(super) fn close_mc(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.mc.take().is_some() {
            self.infer = None;
            window.focus(&self.focus, cx);
            self.update_capture(cx);
            self.schedule_eval(window, cx);
            cx.notify();
        }
    }

    fn accept_mc(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(s) = &self.mc else { return };
        let Some(c) = s.to_connector(cx) else {
            self.message = Some((false, "Pick an origin entity (geometry, an inference point or the assembly origin).".into()));
            cx.notify();
            return;
        };
        let editing = s.editing;
        let for_mate = s.for_mate;
        self.mc = None;
        let desc = if editing { format!("Edit : {}", c.name) } else { format!("Insert : {}", c.name) };
        if let (Some(field), Some(d)) = (for_mate, self.mate.as_ref().map(|m| m.dialog.clone())) {
            // The mate now uses the (edited) assembly mate connector.
            let r = MateConnectorRef::assembly_connector(c.id, c.owner());
            d.update(cx, |d, cx| d.replace_field(field, r, cx));
        }
        self.edit(&desc, cx, |a| {
            // Mates that use the connector follow its owner.
            for m in &mut a.mates {
                for r in std::iter::once(&mut m.a).chain(std::iter::once(&mut m.b)).chain(m.extra.iter_mut()) {
                    if r.connector == Some(c.id) {
                        r.instance = c.on.instance;
                    }
                }
            }
            match a.connectors.iter().position(|x| x.id == c.id) {
                Some(i) => a.connectors[i] = c,
                None => a.connectors.push(c),
            }
        });
        self.infer = None;
        window.focus(&self.focus, cx);
        self.update_capture(cx);
        cx.notify();
    }

    pub(super) fn mc_at_instance_origin(&mut self, top: InstanceId, window: &mut Window, cx: &mut Context<Self>) {
        self.open_mc(None, window, cx);
        if let Some(m) = &mut self.mc {
            m.on = Some(MateConnectorRef::on(top, vec![], Reference::Origin));
            m.active = false;
        }
        self.update_capture(cx);
        self.schedule_eval(window, cx);
    }

    pub(super) fn render_mc_panel(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let Some(s) = &self.mc else { return div().into_any_element() };
        let field = self.conn_field("mcd-origin", &s.on, s.active && s.slot == 0, "Select origin entity", cx, |this, _| {
            if let Some(m) = &mut this.mc {
                m.active = true;
                m.slot = 0;
            }
        });
        let extra_field = |this: &Self, slot: usize, id: &'static str, ph: &'static str, cx: &mut Context<Self>| {
            let s = this.mc.as_ref()?;
            Some(this.conn_field(id, &s.extra[slot - 1], s.active && s.slot == slot, ph, cx, move |this, _| {
                if let Some(m) = &mut this.mc {
                    m.active = true;
                    m.slot = slot;
                }
            }))
        };
        let (between, realign) = (s.between, s.realign);
        let between_field = if between { extra_field(self, 1, "mcd-between", "Select between entity", cx) } else { None };
        let primary_field = if realign { extra_field(self, 2, "mcd-primary", "Primary axis entity", cx) } else { None };
        let secondary_field = if realign { extra_field(self, 3, "mcd-secondary", "Secondary axis entity", cx) } else { None };
        let Some(s) = &self.mc else { return div().into_any_element() };
        let origin_type = |k: bool, label: &'static str, cx: &mut Context<Self>| {
            Button::new(SharedString::from(format!("mcd-type-{k}"))).label(label).ghost().xsmall().selected(between == k).on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                if let Some(m) = &mut this.mc {
                    m.between = k;
                    if k && m.extra[0].is_none() {
                        m.active = true;
                        m.slot = 1;
                    }
                }
                this.update_capture(cx);
                this.schedule_eval(window, cx);
                cx.notify();
            }))
        };
        let axis_btn = |k: u8, label: &'static str, cur: u8, cx: &mut Context<Self>| {
            Button::new(SharedString::from(format!("mcd-ax-{k}"))).label(label).ghost().xsmall().selected(cur == k).on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                if let Some(m) = &mut this.mc {
                    m.rot_axis = k;
                }
                this.schedule_eval(window, cx);
                cx.notify();
            }))
        };
        let (flip, reorient, rot_axis) = (s.flip, s.reorient, s.rot_axis);
        let body = v_flex()
            .p_3()
            .gap_2()
            // Origin type: On entity / Between entities (Onshape).
            .child(h_flex().gap_1().child(origin_type(false, "On entity", cx)).child(origin_type(true, "Between entities", cx)))
            .child(div().text_xs().text_color(pal.text_muted).child("Origin entity (geometry inference point, or the assembly origin)"))
            .child(field)
            .children(between_field)
            .child(
                h_flex()
                    .gap_1()
                    .child(Button::new("mcd-flip").icon(Icon::empty().path(icon_path("flip"))).ghost().small().selected(flip).label("Flip primary").on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        if let Some(m) = &mut this.mc {
                            m.flip = !m.flip;
                        }
                        this.schedule_eval(window, cx);
                        cx.notify();
                    })))
                    .child(Button::new("mcd-reorient").icon(Icon::empty().path(icon_path("flip-circular"))).ghost().small().label(format!("Reorient {}°", (reorient % 4) as u32 * 90)).on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        if let Some(m) = &mut this.mc {
                            m.reorient = (m.reorient + 1) % 4;
                        }
                        this.schedule_eval(window, cx);
                        cx.notify();
                    }))),
            )
            .child(Checkbox::new("mcd-realign").label("Realign").checked(realign).on_click(cx.listener(|this, v: &bool, window, cx| {
                if let Some(m) = &mut this.mc {
                    m.realign = *v;
                    if *v && m.extra[1].is_none() {
                        m.active = true;
                        m.slot = 2;
                    }
                }
                this.update_capture(cx);
                this.schedule_eval(window, cx);
                cx.notify();
            })))
            .children(primary_field)
            .children(secondary_field)
            .child(section("MOVE"))
            .child(field_row("X translation", Input::new(&s.inputs[0]).xsmall()))
            .child(field_row("Y translation", Input::new(&s.inputs[1]).xsmall()))
            .child(field_row("Z translation", Input::new(&s.inputs[2]).xsmall()))
            .child(field_row("Rotation axis", h_flex().gap_1().child(axis_btn(0, "X", rot_axis, cx)).child(axis_btn(1, "Y", rot_axis, cx)).child(axis_btn(2, "Z", rot_axis, cx))))
            .child(field_row("Rotation angle", Input::new(&s.inputs[3]).xsmall()))
            .child(h_flex().justify_end().gap_2().child(Button::new("mcd-ok").label("OK").primary().small().on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.accept_mc(window, cx)))));
        let title = s.name.clone();
        Self::panel_frame(&title, body, cx.listener(|this, _: &ClickEvent, window, cx| this.close_mc(window, cx)), "mcd-close").into_any_element()
    }

    // ------------------------------------------------------------ list rows

    pub(super) fn origin_row(&self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        h_flex()
            .id("asm-origin-row")
            .h(px(24.))
            .pl(px(18.))
            .pr_1()
            .gap_1p5()
            .rounded_sm()
            .text_sm()
            .text_color(pal.text_muted)
            .hover(|d| d.bg(pal.panel_hover))
            .child(svg().path(icon_path("datum")).size_3p5().text_color(pal.text_muted))
            .child(div().flex_1().when(self.ui.origin_hidden, |d| d.text_color(pal.text_faint)).child("Origin"))
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                if this.pick_mode(cx).is_some() {
                    this.deliver_connector(MateConnectorRef::origin(), window, cx);
                    this.refresh_scene(false, cx);
                    cx.notify();
                }
            }))
            // Onshape's origin row menu: (Create Part Studio in context… n/a) | Hide | Create empty subassembly.
            .os_context_menu({
                let view = cx.entity();
                let hidden = self.ui.origin_hidden;
                move |menu, window, cx| {
                    let entries = vec![
                        behaviors::MenuEntry::Item {
                            label: if hidden { "Show".into() } else { "Hide".into() },
                            icon: Some(if hidden { "eye" } else { "eye-off" }),
                            disabled: false,
                            act: std::rc::Rc::new(move |t: &mut AssemblyView, _: &mut Window, cx: &mut Context<AssemblyView>| {
                                t.ui.origin_hidden = !hidden;
                                t.refresh_scene(false, cx);
                                cx.notify();
                            }),
                        },
                        behaviors::MenuEntry::Sep,
                        behaviors::MenuEntry::Item {
                            label: "Create empty subassembly".into(),
                            icon: None,
                            disabled: false,
                            act: std::rc::Rc::new(|t: &mut AssemblyView, _: &mut Window, cx: &mut Context<AssemblyView>| t.create_empty_subassembly(cx)),
                        },
                    ];
                    fill_menu(menu, entries, view.clone(), window, cx)
                }
            })
            .into_any_element()
    }

    pub(super) fn connector_row(&self, c: &AsmMateConnector, message: Option<String>, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let id = c.id;
        let owner = c.owner();
        let hidden = c.hidden;
        let selected = self.selected_mate == Some(id);
        let failed = message.is_some();
        let row = h_flex()
            .id(SharedString::from(format!("amc-{}", id.0)))
            .h(px(24.))
            .px_2()
            .gap_2()
            .rounded_sm()
            .text_sm()
            .text_color(if c.suppressed || hidden { pal.text_faint } else if failed { pal.error } else { pal.text })
            .when(selected, |d| d.bg(pal.accent_soft))
            .hover(|d| d.bg(pal.panel_hover))
            .child(svg().path(icon_path("mate-connector-assembly")).size_3p5().text_color(if failed { pal.error } else { pal.text_muted }))
            .child(div().flex_1().truncate().child(c.name.clone()))
            .when_some(message, |d, m| d.child(div().size(px(6.)).rounded_full().bg(pal.error)).tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(m.clone()).build(w, cx)))
            .child(
                Button::new(SharedString::from(format!("amc-eye-{}", id.0)))
                    .icon(Icon::empty().path(icon_path(if hidden { "eye-off" } else { "eye" })))
                    .ghost()
                    .xsmall()
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        cx.stop_propagation();
                        this.edit(if hidden { "Show mate connector" } else { "Hide mate connector" }, cx, |a| {
                            if let Some(c) = a.connectors.iter_mut().find(|c| c.id == id) {
                                c.hidden = !hidden;
                            }
                        });
                    })),
            )
            .on_hover(cx.listener(move |this, h: &bool, _, cx| {
                let new = if *h { Some(id) } else { None };
                if this.list_hover_mate != new {
                    this.list_hover_mate = new;
                    this.refresh_scene(false, cx);
                }
            }))
            .on_click(cx.listener(move |this, ev: &ClickEvent, window, cx| {
                if this.pick_mode(cx).is_some() {
                    this.deliver_connector(MateConnectorRef::assembly_connector(id, owner), window, cx);
                } else if ev.click_count() >= 2 {
                    this.edit_mate(id, window, cx);
                    return;
                } else {
                    this.selected_mate = if this.selected_mate == Some(id) { None } else { Some(id) };
                    this.selected.clear();
                }
                this.refresh_scene(false, cx);
                cx.notify();
            }));
        let view = cx.entity();
        row.os_context_menu(move |menu, window, cx| {
            let entries = view.read(cx).connector_menu(id, cx);
            fill_menu(menu, entries, view.clone(), window, cx)
        })
        .into_any_element()
    }

    pub(super) fn pattern_row(&self, pat: &InstancePattern, error: Option<String>, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let id = pat.id;
        let key = vec![InstanceId(id.0)];
        let expanded = self.expanded.contains(&key);
        let icon = match pat.kind {
            PatternKind::Linear { .. } | PatternKind::PartPattern { .. } => "linear-pattern",
            PatternKind::Circular { .. } => "circular-pattern",
            PatternKind::Mirror { .. } => "mirror",
            PatternKind::Replicate { .. } => "replicate",
        };
        let selected = self.selected_mate == Some(id);
        let suppressed = pat.suppressed;
        let n = pat.generated.len();
        let row = h_flex()
            .id(SharedString::from(format!("pat-{}", id.0)))
            .h(px(24.))
            .pl(px(6.))
            .pr_1()
            .gap_1p5()
            .rounded_sm()
            .text_sm()
            .text_color(if suppressed { pal.text_faint } else if error.is_some() { pal.error } else { pal.text })
            .when(selected, |d| d.bg(pal.accent_soft))
            .hover(|d| d.bg(pal.panel_hover))
            .child(
                div()
                    .id(SharedString::from(format!("pat-exp-{}", id.0)))
                    .w(px(12.))
                    .child(svg().path(icon_path(if expanded { "chevron-down" } else { "chevron-right" })).size_3().text_color(pal.text_faint))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        let k = vec![InstanceId(id.0)];
                        if !this.expanded.remove(&k) {
                            this.expanded.insert(k);
                        }
                        cx.stop_propagation();
                        cx.notify();
                    })),
            )
            .child(svg().path(icon_path(icon)).size_3p5().text_color(pal.text_muted))
            .child(div().flex_1().truncate().when(suppressed, |d| d.line_through()).child(pat.name.clone()))
            .child(div().text_xs().text_color(pal.text_faint).child(format!("{n}")))
            .when_some(error, |d, e| d.child(div().id(SharedString::from(format!("pat-err-{}", id.0))).size(px(6.)).rounded_full().bg(pal.error).tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(e.clone()).build(w, cx))))
            .on_hover(cx.listener(move |this, h: &bool, _, cx| {
                let new = if *h { Some(id) } else { None };
                if this.list_hover_mate != new {
                    this.list_hover_mate = new;
                    this.refresh_scene(false, cx);
                }
            }))
            .on_click(cx.listener(move |this, ev: &ClickEvent, window, cx| {
                if ev.click_count() >= 2 {
                    this.edit_mate(id, window, cx);
                    return;
                }
                this.selected_mate = if this.selected_mate == Some(id) { None } else { Some(id) };
                this.refresh_scene(false, cx);
                cx.notify();
            }));
        let view = cx.entity();
        row.os_context_menu(move |menu, _, _| {
            let (v1, v2, v3, v4) = (view.clone(), view.clone(), view.clone(), view.clone());
            menu.item(OsMenuItem::new("Edit…").on_click(move |_, window, cx| v1.update(cx, |t, cx| t.edit_mate(id, window, cx))))
                .item(OsMenuItem::new(if suppressed { "Unsuppress" } else { "Suppress" }).on_click(move |_, _, cx| {
                    v2.update(cx, |t, cx| {
                        t.edit("Suppress pattern", cx, |a| {
                            if let Some(p) = a.patterns.iter_mut().find(|p| p.id == id) {
                                p.suppressed = !p.suppressed;
                            }
                        });
                    })
                }))
                .item(OsMenuItem::new("Dissolve").on_click(move |_, _, cx| v3.update(cx, |t, cx| {
                    t.edit("Dissolve pattern", cx, |a| a.dissolve_pattern(id));
                })))
                .separator()
                .item(OsMenuItem::new("Delete").on_click(move |_, _, cx| v4.update(cx, |t, cx| t.delete_mate(id, cx))))
        })
        .into_any_element()
    }

    // ------------------------------------------------------------ patterns

    fn new_pattern_session(&self, kind: PatternDialogKind, window: &mut Window, cx: &mut Context<Self>) -> PatternSession {
        let (d, n) = match kind {
            PatternDialogKind::Circular => ("360 deg", "4"),
            _ => ("50 mm", "2"),
        };
        let mk = |t: &str, window: &mut Window, cx: &mut Context<Self>| {
            let t = t.to_string();
            cx.new(|cx| InputState::new(window, cx).default_value(t))
        };
        let seeds: Vec<InstanceId> = self.assembly(cx).map(|a| self.selected.iter().copied().filter(|id| a.generated_by(*id).is_none()).collect()).unwrap_or_default();
        let name = self.assembly(cx).map(|a| a.next_feature_name(kind.title())).unwrap_or_default();
        PatternSession {
            kind,
            editing: None,
            name,
            seeds,
            axis: None,
            axis2: None,
            active: if kind.matched() { None } else { Some(0) },
            distance: mk(d, window, cx),
            count: mk(n, window, cx),
            distance2: mk("50 mm", window, cx),
            count2: mk("2", window, cx),
            opposite: false,
            opposite2: false,
            equal: kind == PatternDialogKind::Circular,
            second: false,
            mate: None,
            preview: None,
        }
    }

    pub(super) fn open_pattern(&mut self, kind: PatternDialogKind, window: &mut Window, cx: &mut Context<Self>) {
        self.close_mate(window, cx);
        self.insert_open = false;
        let mut s = self.new_pattern_session(kind, window, cx);
        if kind.matched() {
            s.mate = self.seed_mate(&s.seeds, cx);
        }
        self.pattern = Some(s);
        self.update_pattern_preview(cx);
        self.update_capture(cx);
        self.refresh_scene(false, cx);
        cx.notify();
    }

    pub(super) fn open_pattern_edit(&mut self, pat: InstancePattern, window: &mut Window, cx: &mut Context<Self>) {
        let kind = match pat.kind {
            PatternKind::Linear { .. } => PatternDialogKind::Linear,
            PatternKind::Circular { .. } => PatternDialogKind::Circular,
            PatternKind::Mirror { .. } => PatternDialogKind::Mirror,
            PatternKind::PartPattern { .. } => PatternDialogKind::PartPattern,
            PatternKind::Replicate { .. } => PatternDialogKind::Replicate,
        };
        let mut s = self.new_pattern_session(kind, window, cx);
        s.editing = Some(pat.id);
        s.name = pat.name.clone();
        s.seeds = pat.seeds.clone();
        s.active = None;
        let set = |st: &Entity<InputState>, t: String, window: &mut Window, cx: &mut Context<Self>| st.update(cx, |s, cx| s.set_value(t, window, cx));
        match &pat.kind {
            PatternKind::Linear { direction, distance, count, opposite, equal_spacing, second } => {
                s.axis = Some(direction.clone());
                set(&s.distance, distance.text.clone(), window, cx);
                set(&s.count, count.to_string(), window, cx);
                s.opposite = *opposite;
                s.equal = *equal_spacing;
                if let Some(sd) = second {
                    s.second = true;
                    s.axis2 = Some(sd.direction.clone());
                    set(&s.distance2, sd.distance.text.clone(), window, cx);
                    set(&s.count2, sd.count.to_string(), window, cx);
                    s.opposite2 = sd.opposite;
                }
            }
            PatternKind::Circular { axis, angle, count, opposite, equal_spacing } => {
                s.axis = Some(axis.clone());
                set(&s.distance, angle.text.clone(), window, cx);
                set(&s.count, count.to_string(), window, cx);
                s.opposite = *opposite;
                s.equal = *equal_spacing;
            }
            PatternKind::Mirror { plane, handed } => {
                s.axis = Some(plane.clone());
                s.second = *handed;
            }
            PatternKind::PartPattern { mate } => s.mate = Some(*mate),
            PatternKind::Replicate { mate, all_instances } => {
                s.mate = Some(*mate);
                s.second = *all_instances;
            }
        }
        self.pattern = Some(s);
        self.update_pattern_preview(cx);
        self.update_capture(cx);
        cx.notify();
    }

    /// The seed's mate to match: its only mate, or the selected one.
    fn seed_mate(&self, seeds: &[InstanceId], cx: &App) -> Option<MateId> {
        let asm = self.assembly(cx)?;
        let seed = *seeds.first()?;
        let mates: Vec<&Mate> = asm.mates.iter().filter(|m| m.uses_instance(seed) && asm.mate_generated_by(m.id).is_none()).collect();
        if let Some(sel) = self.selected_mate
            && mates.iter().any(|m| m.id == sel)
        {
            return Some(sel);
        }
        (mates.len() == 1).then(|| mates[0].id)
    }

    fn pattern_from_session(&self, s: &PatternSession, cx: &App) -> Result<InstancePattern, String> {
        let asm = self.assembly(cx).ok_or("No assembly")?;
        if s.seeds.is_empty() {
            return Err("Select the instances to pattern (seed)".into());
        }
        let text = |st: &Entity<InputState>| st.read(cx).value().to_string();
        let count = |st: &Entity<InputState>| text(st).trim().parse::<f64>().ok().map(|v| v.round().clamp(1.0, 2500.0) as u32).ok_or("Instance count must be a whole number");
        let kind = match s.kind {
            PatternDialogKind::Linear => {
                let direction = s.axis.clone().ok_or("Select a direction (edge, face or mate connector)")?;
                let second = if s.second {
                    let d2 = s.axis2.clone().ok_or("Select the second direction")?;
                    Some(noshape_model::assembly::LinearDirection { direction: d2, distance: ExprValue::new(text(&s.distance2)), count: count(&s.count2)?, opposite: s.opposite2 })
                } else {
                    None
                };
                PatternKind::Linear { direction, distance: ExprValue::new(text(&s.distance)), count: count(&s.count)?, opposite: s.opposite, equal_spacing: s.equal, second }
            }
            PatternDialogKind::Circular => PatternKind::Circular { axis: s.axis.clone().ok_or("Select the axis of pattern")?, angle: ExprValue::new(text(&s.distance)), count: count(&s.count)?, opposite: s.opposite, equal_spacing: s.equal },
            PatternDialogKind::Mirror => PatternKind::Mirror { plane: s.axis.clone().ok_or("Select the mirror plane (planar face or mate connector)")?, handed: s.second },
            PatternDialogKind::PartPattern => PatternKind::PartPattern { mate: s.mate.ok_or("The seed needs exactly one mate to follow (or select the mate in the list)")? },
            PatternDialogKind::Replicate => PatternKind::Replicate { mate: s.mate.ok_or("The seed needs exactly one mate to match (or select the mate in the list)")?, all_instances: s.second },
        };
        let generated = s.editing.and_then(|id| asm.pattern(id)).map(|p| p.generated.clone()).unwrap_or_default();
        let id = s.editing.unwrap_or_else(|| MateId(asm.next_id()));
        let seeds = if s.kind.matched() { s.seeds[..1].to_vec() } else { s.seeds.clone() };
        Ok(InstancePattern { id, name: s.name.clone(), kind, seeds, generated, suppressed: false })
    }

    /// Matches for part pattern / replicate dialogs (shown before accepting).
    pub(super) fn update_pattern_preview(&mut self, cx: &mut Context<Self>) {
        let Some(s) = &self.pattern else { return };
        if !s.kind.matched() {
            return;
        }
        let preview = match (self.pattern_from_session(s, cx), self.assembly(cx)) {
            (Ok(pat), Some(asm)) => analysis::matched_targets(&self.result, asm, &pat, &self.units(cx)).map(|t| t.len()),
            (Err(e), _) => Err(e),
            _ => Err("No assembly".into()),
        };
        if let Some(s) = &mut self.pattern {
            s.preview = Some(preview);
        }
    }

    fn accept_pattern(&mut self, cx: &mut Context<Self>) {
        let Some(s) = &self.pattern else { return };
        let pat = match self.pattern_from_session(s, cx) {
            Ok(p) => p,
            Err(e) => {
                self.message = Some((false, e));
                cx.notify();
                return;
            }
        };
        let matched = s.kind.matched();
        let targets = if matched {
            let Some(asm) = self.assembly(cx) else { return };
            match analysis::matched_targets(&self.result, asm, &pat, &self.units(cx)) {
                Ok(t) => Some(t),
                Err(e) => {
                    self.message = Some((false, e));
                    cx.notify();
                    return;
                }
            }
        } else {
            None
        };
        let editing = s.editing.is_some();
        self.pattern = None;
        let id = pat.id;
        let desc = format!("{} : {}", if editing { "Edit" } else { "Insert" }, pat.name);
        let created = self.edit(&desc, cx, move |a| {
            match a.patterns.iter().position(|p| p.id == id) {
                Some(i) => a.patterns[i] = pat,
                None => a.patterns.push(pat),
            }
            match targets {
                Some(t) => a.sync_matched_pattern(id, t),
                None => a.sync_pattern(id),
            }
        });
        if let Some(c) = created {
            self.message = Some((false, format!("{} instance(s) created.", c.len())));
        }
        self.expanded.insert(vec![InstanceId(id.0)]);
        self.update_capture(cx);
        cx.notify();
    }

    pub(super) fn render_pattern_panel(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let Some(s) = &self.pattern else { return div().into_any_element() };
        let kind = s.kind;
        let names: HashMap<InstanceId, String> = self.assembly(cx).map(|a| a.instances.iter().map(|i| (i.id, i.name.clone())).collect()).unwrap_or_default();
        let seeds_label = if s.seeds.is_empty() { "None: select instances, then Use selection".to_string() } else { s.seeds.iter().filter_map(|i| names.get(i).cloned()).collect::<Vec<_>>().join(", ") };
        let mut body = v_flex().p_3().gap_2();
        body = body.child(section(if kind.matched() { "SEED INSTANCE" } else { "INSTANCES" })).child(
            h_flex()
                .gap_2()
                .child(div().flex_1().text_sm().truncate().child(seeds_label))
                .child(Button::new("pat-usesel").label("Use selection").ghost().xsmall().on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    let sel = this.selected.clone();
                    let mate = this.seed_mate(&sel, cx);
                    if let Some(s) = &mut this.pattern {
                        s.seeds = sel;
                        if s.kind.matched() {
                            s.mate = mate;
                        }
                    }
                    this.update_pattern_preview(cx);
                    cx.notify();
                }))),
        );
        let (axis_label, axis_ph) = match kind {
            PatternDialogKind::Linear => ("DIRECTION", "Select edge, face or mate connector"),
            PatternDialogKind::Circular => ("AXIS OF PATTERN", "Select cylinder, circular edge or mate connector"),
            PatternDialogKind::Mirror => ("MIRROR PLANE", "Select planar face or mate connector"),
            _ => ("", ""),
        };
        if !kind.matched() {
            let f = self.conn_field("pat-axis", &s.axis, s.active == Some(0), axis_ph, cx, |this, _| {
                if let Some(p) = &mut this.pattern {
                    p.active = Some(0);
                }
            });
            body = body.child(section(axis_label)).child(f);
        }
        let (opposite, equal, second) = (s.opposite, s.equal, s.second);
        match kind {
            PatternDialogKind::Linear | PatternDialogKind::Circular => {
                let label = if kind == PatternDialogKind::Linear { "Distance" } else { "Angle" };
                body = body
                    .child(field_row(label, Input::new(&s.distance).xsmall()))
                    .child(field_row("Instance count", Input::new(&s.count).xsmall()))
                    .child(Checkbox::new("pat-opp").label("Opposite direction").checked(opposite).on_click(cx.listener(|this, v: &bool, _, cx| {
                        if let Some(p) = &mut this.pattern {
                            p.opposite = *v;
                        }
                        cx.notify();
                    })))
                    .child(Checkbox::new("pat-eq").label("Equal spacing").checked(equal).on_click(cx.listener(|this, v: &bool, _, cx| {
                        if let Some(p) = &mut this.pattern {
                            p.equal = *v;
                        }
                        cx.notify();
                    })));
                if kind == PatternDialogKind::Linear {
                    body = body.child(Checkbox::new("pat-second").label("Second direction").checked(second).on_click(cx.listener(|this, v: &bool, _, cx| {
                        if let Some(p) = &mut this.pattern {
                            p.second = *v;
                            if *v && p.axis2.is_none() {
                                p.active = Some(1);
                            }
                        }
                        this.update_capture(cx);
                        cx.notify();
                    })));
                    if second {
                        let f2 = self.conn_field("pat-axis2", &s.axis2, s.active == Some(1), "Select second direction", cx, |this, _| {
                            if let Some(p) = &mut this.pattern {
                                p.active = Some(1);
                            }
                        });
                        let opp2 = s.opposite2;
                        body = body
                            .child(f2)
                            .child(field_row("Distance", Input::new(&s.distance2).xsmall()))
                            .child(field_row("Instance count", Input::new(&s.count2).xsmall()))
                            .child(Checkbox::new("pat-opp2").label("Opposite direction").checked(opp2).on_click(cx.listener(|this, v: &bool, _, cx| {
                                if let Some(p) = &mut this.pattern {
                                    p.opposite2 = *v;
                                }
                                cx.notify();
                            })));
                    }
                }
            }
            PatternDialogKind::Mirror => {
                let handed = s.second;
                body = body
                    .child(Checkbox::new("mir-handed").label("Create mirrored (opposite-handed) parts").checked(handed).on_click(cx.listener(|this, v: &bool, _, cx| {
                        if let Some(p) = &mut this.pattern {
                            p.second = *v;
                        }
                        cx.notify();
                    })))
                    .child(div().text_xs().text_color(pal.text_faint).child(if handed {
                        "Part copies use a mirrored version of the part (a true mirror image); they follow the seed."
                    } else {
                        "Copies are placed with a rigid transform (symmetric parts); they follow the seed."
                    }));
            }
            PatternDialogKind::PartPattern | PatternDialogKind::Replicate => {
                let mate_name = s.mate.and_then(|m| self.assembly(cx).and_then(|a| a.mate(m).map(|x| x.name.clone()))).unwrap_or_else(|| "Seed must have one mate (or select one in the list)".into());
                body = body.child(section("MATE TO BE MATCHED")).child(div().text_sm().child(mate_name)).child(div().text_xs().text_color(pal.text_faint).child(if kind == PatternDialogKind::Replicate {
                    "Matches geometry identical to the mate's target (same type and size) on the target instance."
                } else {
                    "Follows the Part Studio pattern that created copies of the mate's target geometry."
                }));
                if kind == PatternDialogKind::Replicate {
                    let all = s.second;
                    body = body.child(Checkbox::new("rep-allinst").label("Also match on other instances of the target part").checked(all).on_click(cx.listener(|this, v: &bool, _, cx| {
                        if let Some(p) = &mut this.pattern {
                            p.second = *v;
                        }
                        this.update_pattern_preview(cx);
                        cx.notify();
                    })));
                }
                match &s.preview {
                    Some(Ok(n)) => body = body.child(div().text_sm().text_color(pal.success).child(format!("{n} match(es) found"))),
                    Some(Err(e)) => body = body.child(div().text_xs().text_color(pal.warning).child(e.clone())),
                    None => {}
                }
            }
        }
        body = body.child(h_flex().justify_end().gap_2().child(Button::new("pat-ok").label("OK").primary().small().on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.accept_pattern(cx)))));
        let title = s.name.clone();
        Self::panel_frame(&title, body, cx.listener(|this, _: &ClickEvent, _, cx| {
            this.pattern = None;
            this.update_capture(cx);
            this.refresh_scene(false, cx);
            cx.notify();
        }), "pat-close")
        .into_any_element()
    }

    // ------------------------------------------------------------ snap mode

    pub(super) fn toggle_snap(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.snap.take().is_some() {
            self.message = None;
        } else {
            self.close_mate(window, cx);
            self.snap = Some(None);
            self.selected.clear();
            self.message = Some((false, "Snap mode: click a face or mate connector on the instance to move.".into()));
        }
        self.infer = None;
        self.update_capture(cx);
        self.refresh_scene(false, cx);
        cx.notify();
    }

    // ------------------------------------------------------------ typed manipulator values

    pub(super) fn open_manip_value(&mut self, d: &DragState, window: &mut Window, cx: &mut Context<Self>) {
        let Some(handle) = d.handle else { return };
        let units = self.units(cx);
        let q = match handle {
            ManipHandle::Axis(_) => Quantity::meters(d.value),
            ManipHandle::Ring(_) => Quantity::radians(d.value),
            // Plane drags and manipulator moves have no single typed value.
            ManipHandle::Plane(_) | ManipHandle::Center => return,
        };
        let text = noshape_units::format_quantity(q, &units);
        let input = cx.new(|cx| InputState::new(window, cx).default_value(text));
        let sub = cx.subscribe_in(&input, window, |this: &mut Self, _, ev: &InputEvent, window, cx| {
            if let InputEvent::PressEnter { .. } = ev {
                this.apply_manip_value(window, cx);
            }
        });
        self.manip_value = Some(ManipValue { handle, axis: d.plane_normal, center: d.plane_point, ids: d.ids.clone(), start: d.start.clone(), explode: d.explode, input, _sub: sub });
        cx.notify();
    }

    fn apply_manip_value(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(mv) = &self.manip_value else { return };
        let units = self.units(cx);
        let dim = if matches!(mv.handle, ManipHandle::Axis(_)) { Dim::LENGTH } else { Dim::ANGLE };
        let text = mv.input.read(cx).value().to_string();
        let v = match noshape_units::evaluate_as(&text, dim, &units, &EmptyScope) {
            Ok(q) => q.value,
            Err(e) => {
                self.message = Some((true, format!("{e}")));
                cx.notify();
                return;
            }
        };
        if mv.explode {
            // Edit the last explode step's distance / angle.
            let (handle, axis) = (mv.handle, mv.axis);
            if let Some(i) = self.explode {
                self.edit("Edit : Explode step", cx, |a| {
                    if let Some(step) = a.exploded_views.get_mut(i).and_then(|v| v.steps.last_mut()) {
                        match handle {
                            ManipHandle::Axis(_) => step.translation = (axis * v).into(),
                            ManipHandle::Ring(_) => step.rotation_angle = v,
                            ManipHandle::Plane(_) | ManipHandle::Center => {}
                        }
                    }
                });
            }
        } else {
            let targets = Self::manip_targets_for(mv.handle, mv.axis, mv.center, v, &mv.start);
            let mut start: HashMap<InstanceId, Placement> = self.result.solved.placements.iter().cloned().collect();
            start.extend(mv.start.iter().map(|(k, p)| (*k, *p)));
            let solved = self.result.prepared.solve(&SolveOptions { targets, start: Some(start), nested_start: Some(self.result.solved.nested.clone()), ..Default::default() });
            let name = self.assembly(cx).and_then(|a| mv.ids.first().and_then(|id| a.instance(*id)).map(|i| i.name.clone())).unwrap_or_default();
            self.apply_solved(solved, cx);
            self.edit(&format!("Move : {name}"), cx, |_| {});
        }
        window.focus(&self.focus, cx);
        self.manip_value = None;
        cx.notify();
    }

    pub(super) fn render_manip_value(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let pal = p();
        let mv = self.manip_value.as_ref()?;
        let f = self.manipulator(cx).map(|f| f.origin).unwrap_or(mv.center);
        let vp = self.viewport.read(cx);
        let (pos, size) = (vp.project(f)?, vp.size());
        // Keep the box inside the viewport (left of the triad near the right edge).
        let pos = DVec2::new(if pos.x + 300.0 > size.x { pos.x - 320.0 } else { pos.x }, pos.y.min(size.y - 60.0));
        let label = match mv.handle {
            ManipHandle::Axis(k) => format!("{} distance", ["X", "Y", "Z"][k]),
            ManipHandle::Ring(k) => format!("Angle about {}", ["X", "Y", "Z"][k]),
            ManipHandle::Plane(_) | ManipHandle::Center => String::new(),
        };
        Some(
            h_flex()
                .id("manip-value")
                .occlude()
                .absolute()
                .left(px(pos.x as f32 + 24.))
                .top(px(pos.y as f32 + 24.))
                .gap_1()
                .p_1()
                .rounded_md()
                .border_1()
                .border_color(pal.accent)
                .bg(pal.panel_raised)
                .shadow_lg()
                .child(div().text_xs().text_color(pal.text_muted).child(label))
                .child(div().w(px(90.)).child(Input::new(&mv.input).xsmall()))
                .child(Button::new("mv-ok").icon(Icon::empty().path(icon_path("check"))).primary().xsmall().on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.apply_manip_value(window, cx))))
                .child(Button::new("mv-x").icon(IconName::Close).ghost().xsmall().on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.manip_value = None;
                    cx.notify();
                })))
                .into_any_element(),
        )
    }

    /// Top banner for modes (snap, show mates, explode editing, placing).
    pub(super) fn render_mode_banner(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let pal = p();
        let (text, done): (String, Option<u8>) = if self.explode_edit {
            let name = self.explode.and_then(|i| self.assembly(cx).and_then(|a| a.exploded_views.get(i).map(|v| v.name.clone()))).unwrap_or_default();
            (format!("Editing {name}: select instances, then drag or rotate them with the triad to add steps"), Some(0))
        } else if let Some(s) = &self.snap {
            (if s.is_some() { "Snap mode: click the target face or mate connector".into() } else { "Snap mode: click a face or mate connector on the instance to move".into() }, Some(1))
        } else if self.show_mates_mode {
            ("Show mates mode: hover an instance to see its mates, click to pin (H to exit)".into(), Some(2))
        } else if self.placing.is_some() {
            ("Click to place the inserted instance".into(), None)
        } else {
            return None;
        };
        Some(
            h_flex()
                .absolute()
                .top_3()
                .left(relative(0.5))
                .ml(px(-260.))
                .w(px(520.))
                .justify_center()
                .child(
                    h_flex()
                        .id("mode-banner")
                        .occlude()
                        .gap_2()
                        .px_3()
                        .py_1()
                        .rounded_md()
                        .bg(with_alpha(pal.warning, 0.22))
                        .border_1()
                        .border_color(with_alpha(pal.warning, 0.6))
                        .text_xs()
                        .text_color(pal.text)
                        .child(text)
                        .when_some(done, |d, k| {
                            d.child(Button::new("banner-done").label("Done").small().primary().on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                                match k {
                                    0 => {
                                        this.explode_edit = false;
                                        this.message = None;
                                    }
                                    1 => this.toggle_snap(window, cx),
                                    _ => {
                                        this.show_mates_mode = false;
                                        this.mates_pinned = None;
                                        this.update_capture(cx);
                                        this.refresh_scene(false, cx);
                                    }
                                }
                                cx.notify();
                            })))
                        }),
                )
                .into_any_element(),
        )
    }

    // ------------------------------------------------------------ rename helper

    fn start_rename(&mut self, target: RenameTarget, current: String, window: &mut Window, cx: &mut Context<Self>) {
        let st = cx.new(|cx| InputState::new(window, cx).default_value(current));
        st.update(cx, |s, cx| s.focus(window, cx));
        self.transient_sub = Some(cx.subscribe_in(&st, window, |this: &mut Self, _, ev: &InputEvent, window, cx| {
            if let InputEvent::PressEnter { .. } = ev {
                this.commit_rename(cx);
                window.focus(&this.focus, cx);
            }
        }));
        self.renaming = Some((target, st));
        cx.notify();
    }

    fn commit_rename(&mut self, cx: &mut Context<Self>) {
        let Some((target, st)) = self.renaming.take() else { return };
        let name = st.read(cx).value().trim().to_string();
        if name.is_empty() {
            cx.notify();
            return;
        }
        match target {
            RenameTarget::Position(i) => {
                self.edit(&format!("Rename named position : {name}"), cx, |a| {
                    if let Some(np) = a.named_positions.get_mut(i) {
                        np.name = name;
                    }
                });
            }
            RenameTarget::Explode(i) => {
                self.edit(&format!("Rename : {name}"), cx, |a| {
                    if let Some(v) = a.exploded_views.get_mut(i) {
                        v.name = name;
                    }
                });
            }
        }
        cx.notify();
    }

    fn rename_input(&self, target: RenameTarget, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (t, st) = self.renaming.as_ref()?;
        if *t != target {
            return None;
        }
        Some(
            h_flex()
                .flex_1()
                .gap_1()
                .child(div().flex_1().child(Input::new(st).xsmall()))
                .child(Button::new("rename-ok").icon(Icon::empty().path(icon_path("check"))).primary().xsmall().on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.commit_rename(cx))))
                .into_any_element(),
        )
    }

    // ------------------------------------------------------------ exploded views panel

    pub(super) fn render_explode_panel(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let views: Vec<ExplodedView> = self.assembly(cx).map(|a| a.exploded_views.clone()).unwrap_or_default();
        let mut body = v_flex().id("explode-body").p_2().gap_1().overflow_y_scroll().max_h(px(520.));
        body = body.child(
            h_flex()
                .gap_1()
                .child(Button::new("ex-add").icon(IconName::Plus).label("Add exploded view").small().ghost().on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.add_exploded_view(false, cx))))
                .child(Button::new("ex-auto").icon(Icon::empty().path(icon_path("explode"))).label("Auto").small().ghost().tooltip("Automatic exploded view").on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.add_exploded_view(true, cx)))),
        );
        if views.is_empty() {
            body = body.child(div().p_2().text_xs().text_color(pal.text_faint).child("No exploded views yet."));
        }
        for (i, v) in views.iter().enumerate() {
            let active = self.explode == Some(i);
            let mut row = h_flex()
                .id(SharedString::from(format!("ex-row-{i}")))
                .h(px(26.))
                .px_2()
                .gap_2()
                .rounded_sm()
                .text_sm()
                .when(active, |d| d.bg(pal.accent_soft))
                .hover(|d| d.bg(pal.panel_hover))
                .child(svg().path(icon_path("explode")).size_3p5().text_color(if active { pal.accent } else { pal.text_muted }));
            match self.rename_input(RenameTarget::Explode(i), cx) {
                Some(inp) => row = row.child(inp),
                None => {
                    let name = v.name.clone();
                    row = row
                        .child(div().flex_1().truncate().child(v.name.clone()))
                        .child(div().text_xs().text_color(pal.text_faint).child(format!("{} steps", v.steps.len())))
                        .child(Button::new(SharedString::from(format!("ex-ren-{i}"))).icon(Icon::empty().path(icon_path("rename"))).ghost().xsmall().tooltip("Rename").on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                            cx.stop_propagation();
                            this.start_rename(RenameTarget::Explode(i), name.clone(), window, cx);
                        })))
                        .child(Button::new(SharedString::from(format!("ex-del-{i}"))).icon(Icon::empty().path(icon_path("delete"))).ghost().xsmall().tooltip("Delete").on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            cx.stop_propagation();
                            this.delete_exploded_view(i, cx);
                        })));
                }
            }
            body = body.child(row.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                let next = if this.explode == Some(i) { None } else { Some(i) };
                this.activate_explode(next, cx);
                if next.is_some() {
                    this.viewport.update(cx, |vp, cx| vp.zoom_to_fit(true, cx));
                }
            })));
            if !active {
                continue;
            }
            // Steps of the active view.
            let names: HashMap<InstanceId, String> = self.assembly(cx).map(|a| a.instances.iter().map(|i| (i.id, i.name.clone())).collect()).unwrap_or_default();
            let units = self.units(cx);
            for (k, st) in v.steps.iter().enumerate() {
                let what = match st.rotation_axis {
                    Some(_) => format!("Rotate {}", noshape_units::format_quantity(Quantity::radians(st.rotation_angle), &units)),
                    None => format!("Translate {}", noshape_units::format_quantity(Quantity::meters(DVec3::from(st.translation).length()), &units)),
                };
                let who = st.instances.iter().filter_map(|i| names.get(i).cloned()).collect::<Vec<_>>().join(", ");
                let editing_step = self.step_edit.as_ref().is_some_and(|s| s.view == i && s.step == k);
                let accent = pal.accent;
                let label: SharedString = format!("{}  {}", k + 1, what).into();
                body = body.child(
                    h_flex()
                        .id(SharedString::from(format!("ex-step-{k}")))
                        .pl(px(26.))
                        .pr_1()
                        .gap_2()
                        .min_h(px(24.))
                        .rounded_sm()
                        .cursor_pointer()
                        .when(editing_step, |d| d.bg(pal.accent_soft))
                        .hover(|d| d.bg(pal.panel_hover))
                        .child(div().text_xs().text_color(pal.text_faint).child(format!("{}", k + 1)))
                        .child(v_flex().flex_1().child(div().text_sm().child(what)).child(div().text_xs().text_color(pal.text_muted).truncate().child(who)))
                        .child(Button::new(SharedString::from(format!("ex-sdel-{k}"))).icon(IconName::Close).ghost().xsmall().tooltip("Delete step").on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            cx.stop_propagation();
                            this.delete_explode_step(k, cx)
                        })))
                        .tooltip(|w, cx| gpui_kit::component::tooltip::Tooltip::new("Click to edit, drag to reorder").build(w, cx))
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.open_step_edit(k, window, cx)))
                        .on_drag(ExplodeStepDrag { index: k, label }, |d, _, _, cx| cx.new(|_| d.clone()))
                        .drag_over::<ExplodeStepDrag>(move |st, _, _, _| st.border_t_2().border_color(accent))
                        .on_drop(cx.listener(move |this, d: &ExplodeStepDrag, _, cx| this.move_explode_step(d.index, k, cx))),
                );
            }
            let n = v.steps.len();
            let editing = self.explode_edit;
            body = body.child(
                h_flex()
                    .pl(px(26.))
                    .gap_1()
                    .child(Button::new("ex-edit").label(if editing { "Done" } else { "Add steps" }).small().when(editing, |b| b.primary()).when(!editing, |b| b.ghost()).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.explode_edit = !this.explode_edit;
                        this.message = None;
                        this.explode_progress = None;
                        this.reapply_display(cx);
                    })))
                    .child(Button::new("ex-play").icon(IconName::Play).label("Play").small().ghost().on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.play_explode(cx)))),
            );
            if n > 0 {
                let cur = self.explode_progress.unwrap_or(n as f64).min(n as f64) as f32;
                if self.explode_slider.as_ref().is_none_or(|(m, _, _)| *m != n) {
                    let st = cx.new(|_| SliderState::new().min(0.).max(n as f32).step(0.01).default_value(cur));
                    let sub = cx.subscribe(&st, |this: &mut Self, _, ev: &SliderEvent, cx| match ev {
                        SliderEvent::Change(v) | SliderEvent::Release(v) => {
                            this.explode_play = None;
                            this.set_explode_progress(v.start() as f64, cx);
                        }
                    });
                    self.explode_slider = Some((n, st, sub));
                }
                if let Some((_, st, _)) = &self.explode_slider {
                    body = body.child(h_flex().pl(px(26.)).pr_2().gap_2().child(div().text_xs().text_color(pal.text_muted).child("Assembled")).child(div().flex_1().child(Slider::new(st))).child(div().text_xs().text_color(pal.text_muted).child("Exploded")));
                }
            }
        }
        Self::panel_frame("Exploded views", body, cx.listener(|this, _: &ClickEvent, _, cx| {
            this.panel = None;
            this.explode_edit = false;
            cx.notify();
        }), "ex-close")
        .into_any_element()
    }

    // ------------------------------------------------------------ named positions panel

    pub(super) fn render_positions_panel(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let positions: Vec<String> = self.assembly(cx).map(|a| a.named_positions.iter().map(|n| n.name.clone()).collect()).unwrap_or_default();
        let mut body = v_flex().id("np-body").p_2().gap_1().overflow_y_scroll().max_h(px(480.));
        match self.naming_position.clone() {
            Some(st) => {
                body = body.child(
                    h_flex()
                        .gap_1()
                        .child(div().flex_1().child(Input::new(&st).xsmall()))
                        .child(Button::new("np-save").label("Save").primary().xsmall().on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            if let Some(st) = this.naming_position.take() {
                                let name = st.read(cx).value().to_string();
                                if !name.trim().is_empty() {
                                    this.save_named_position(name, cx);
                                }
                            }
                            cx.notify();
                        }))),
                );
            }
            None => {
                body = body.child(Button::new("np-add").icon(IconName::Plus).label("Add named position").small().ghost().on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                    let n = this.assembly(cx).map(|a| a.named_positions.len() + 1).unwrap_or(1);
                    let st = cx.new(|cx| InputState::new(window, cx).default_value(format!("Named position {n}")));
                    st.update(cx, |s, cx| s.focus(window, cx));
                    this.transient_sub = Some(cx.subscribe_in(&st, window, |this: &mut Self, _, ev: &InputEvent, window, cx| {
                        if let InputEvent::PressEnter { .. } = ev
                            && let Some(st) = this.naming_position.take()
                        {
                            let name = st.read(cx).value().to_string();
                            if !name.trim().is_empty() {
                                this.save_named_position(name, cx);
                            }
                            window.focus(&this.focus, cx);
                            cx.notify();
                        }
                    }));
                    this.naming_position = Some(st);
                    cx.notify();
                })));
            }
        }
        if positions.is_empty() {
            body = body.child(div().p_2().text_xs().text_color(pal.text_faint).child("Save the current instance positions to return to them later."));
        }
        for (i, name) in positions.iter().enumerate() {
            let mut row = h_flex().id(SharedString::from(format!("np-row-{i}"))).h(px(26.)).px_2().gap_1().rounded_sm().text_sm().hover(|d| d.bg(pal.panel_hover)).child(svg().path(icon_path("named-position")).size_3p5().text_color(pal.text_muted));
            match self.rename_input(RenameTarget::Position(i), cx) {
                Some(inp) => row = row.child(inp),
                None => {
                    let n2 = name.clone();
                    row = row
                        .child(div().flex_1().truncate().child(name.clone()))
                        .child(Button::new(SharedString::from(format!("np-apply-{i}"))).label("Apply").ghost().xsmall().on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.apply_named_position(i, cx))))
                        .child(Button::new(SharedString::from(format!("np-upd-{i}"))).icon(Icon::empty().path(icon_path("check-repeat"))).ghost().xsmall().tooltip("Update to the current positions").on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.update_named_position(i, cx))))
                        .child(Button::new(SharedString::from(format!("np-ren-{i}"))).icon(Icon::empty().path(icon_path("rename"))).ghost().xsmall().tooltip("Rename").on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.start_rename(RenameTarget::Position(i), n2.clone(), window, cx))))
                        .child(Button::new(SharedString::from(format!("np-del-{i}"))).icon(Icon::empty().path(icon_path("delete"))).ghost().xsmall().tooltip("Delete").on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.delete_named_position(i, cx))));
                }
            }
            body = body.child(row);
        }
        Self::panel_frame("Named positions", body, cx.listener(|this, _: &ClickEvent, _, cx| {
            this.panel = None;
            this.naming_position = None;
            cx.notify();
        }), "np-close")
        .into_any_element()
    }

    // ------------------------------------------------------------ configuration / replace

    pub(super) fn open_config(&mut self, top: InstanceId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(inst) = self.assembly(cx).and_then(|a| a.instance(top).cloned()) else { return };
        let studio_inputs = |doc: &Document, tab: TabId| match doc.tab(tab).map(|t| &t.content) {
            Some(TabContent::PartStudio(ps)) => ps.configuration.inputs.clone(),
            _ => vec![],
        };
        let (inputs, values) = match &inst.source {
            InstanceSource::Part { studio, config, .. } | InstanceSource::PartStudio { studio, config } | InstanceSource::MirroredPart { studio, config, .. } => (studio_inputs(&self.doc.read(cx).session.doc, *studio), config.clone()),
            InstanceSource::External { key, config, .. } => {
                use noshape_model::assembly::eval::ExternalStudios as _;
                (AppExternal.studio(key).map(|(d, t)| studio_inputs(&d, t)).unwrap_or_default(), config.clone())
            }
            InstanceSource::Assembly { .. } => {
                self.message = Some((false, "Subassembly instances have no configuration.".into()));
                cx.notify();
                return;
            }
            InstanceSource::Standard { .. } => {
                self.edit_standard_instance(top, cx);
                return;
            }
        };
        if inputs.is_empty() {
            self.message = Some((false, format!("{} is not configured (its Part Studio has no configuration inputs).", inst.name)));
            cx.notify();
            return;
        }
        let mut texts = Vec::new();
        for inp in &inputs {
            let default = match &inp.kind {
                ConfigInputKind::Variable { default, .. } => Some(default.text.clone()),
                ConfigInputKind::Text { default } => Some(default.clone()),
                _ => None,
            };
            if let Some(d) = default {
                let cur = values.0.get(&inp.id).cloned().unwrap_or(d);
                texts.push((inp.id.clone(), cx.new(|cx| InputState::new(window, cx).default_value(cur))));
            }
        }
        self.config_panel = Some(ConfigSession { instance: top, inputs, values, texts });
        cx.notify();
    }

    fn apply_config(&mut self, cx: &mut Context<Self>) {
        let Some(s) = self.config_panel.take() else { return };
        let mut values = s.values.clone();
        for (id, st) in &s.texts {
            values.0.insert(id.clone(), st.read(cx).value().to_string());
        }
        let name = self.assembly(cx).and_then(|a| a.instance(s.instance).map(|i| i.name.clone())).unwrap_or_default();
        let id = s.instance;
        self.edit(&format!("Change configuration : {name}"), cx, |a| {
            a.set_instance_config(id, values);
        });
        cx.notify();
    }

    pub(super) fn render_config_panel(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let Some(s) = &self.config_panel else { return div().into_any_element() };
        let inputs = s.inputs.clone();
        let name = self.assembly(cx).and_then(|a| a.instance(s.instance).map(|i| i.name.clone())).unwrap_or_default();
        let mut body = v_flex().p_3().gap_2().child(div().text_xs().text_color(pal.text_muted).child(format!("Configuration of {name}")));
        for inp in &inputs {
            let id = inp.id.clone();
            match &inp.kind {
                ConfigInputKind::List { options, default } => {
                    let cur = s.values.0.get(&inp.id).cloned().unwrap_or_else(|| default.clone());
                    let mut seg = h_flex().gap_1().flex_wrap();
                    for o in options {
                        let (oid, id2) = (o.id.clone(), id.clone());
                        seg = seg.child(Button::new(SharedString::from(format!("cfg-{}-{}", inp.id, o.id))).label(o.name.clone()).xsmall().ghost().selected(cur == o.id).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            if let Some(s) = &mut this.config_panel {
                                s.values.0.insert(id2.clone(), oid.clone());
                            }
                            cx.notify();
                        })));
                    }
                    body = body.child(section(&inp.name)).child(seg);
                }
                ConfigInputKind::Checkbox { default } => {
                    let cur = s.values.0.get(&inp.id).map(|v| v == "true").unwrap_or(*default);
                    body = body.child(Checkbox::new(SharedString::from(format!("cfg-{}", inp.id))).label(inp.name.clone()).checked(cur).on_click(cx.listener(move |this, v: &bool, _, cx| {
                        if let Some(s) = &mut this.config_panel {
                            s.values.0.insert(id.clone(), v.to_string());
                        }
                        cx.notify();
                    })));
                }
                ConfigInputKind::Variable { .. } | ConfigInputKind::Text { .. } => {
                    if let Some((_, st)) = s.texts.iter().find(|(k, _)| *k == inp.id) {
                        body = body.child(field_row(&inp.name, Input::new(st).xsmall()));
                    }
                }
            }
        }
        body = body.child(h_flex().justify_end().child(Button::new("cfg-ok").label("Apply").primary().small().on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.apply_config(cx)))));
        Self::panel_frame("Change configuration", body, cx.listener(|this, _: &ClickEvent, _, cx| {
            this.config_panel = None;
            cx.notify();
        }), "cfg-close")
        .into_any_element()
    }

    pub(super) fn open_replace(&mut self, top: InstanceId, cx: &mut Context<Self>) {
        self.replace_panel = Some(ReplaceSession { instance: top, all: false });
        cx.notify();
    }

    fn do_replace(&mut self, source: InstanceSource, base: String, cx: &mut Context<Self>) {
        let Some(s) = self.replace_panel.take() else { return };
        self.replace_with_matching(s.instance, s.all, source, base, cx);
        cx.notify();
    }

    pub(super) fn render_replace_panel(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let Some(s) = &self.replace_panel else { return div().into_any_element() };
        let name = self.assembly(cx).and_then(|a| a.instance(s.instance).map(|i| i.name.clone())).unwrap_or_default();
        let all = s.all;
        let mut body = v_flex()
            .id("replace-body")
            .p_2()
            .gap_1()
            .overflow_y_scroll()
            .max_h(px(440.))
            .child(div().px_1().text_xs().text_color(pal.text_muted).child(format!("Replace {name} with:")))
            .child(Checkbox::new("rep-all").label("Replace all instances").checked(all).on_click(cx.listener(|this, v: &bool, _, cx| {
                if let Some(s) = &mut this.replace_panel {
                    s.all = *v;
                }
                cx.notify();
            })));
        for st in self.catalog.studios.clone() {
            let (tab, sname) = (st.tab, st.name.clone());
            body = body.child(
                h_flex()
                    .id(SharedString::from(format!("rep-ps-{tab}")))
                    .h(px(24.))
                    .px_2()
                    .gap_2()
                    .rounded_sm()
                    .cursor_pointer()
                    .text_sm()
                    .hover(|d| d.bg(pal.panel_hover))
                    .child(svg().path(icon_path("part-studio")).size_3p5().text_color(pal.accent))
                    .child(div().flex_1().truncate().child(format!("{sname} (rigid)")))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.do_replace(InstanceSource::PartStudio { studio: tab, config: ConfigValues::default() }, sname.clone(), cx))),
            );
            for (body_id, pname, _, _) in st.parts {
                let pname2 = pname.clone();
                body = body.child(
                    h_flex()
                        .id(SharedString::from(format!("rep-part-{tab}-{body_id}")))
                        .h(px(24.))
                        .pl(px(26.))
                        .pr_2()
                        .gap_2()
                        .rounded_sm()
                        .cursor_pointer()
                        .text_sm()
                        .hover(|d| d.bg(pal.panel_hover))
                        .child(svg().path(icon_path("part")).size_3p5().text_color(pal.text_muted))
                        .child(div().flex_1().truncate().child(pname))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.do_replace(InstanceSource::Part { studio: tab, body: body_id.clone(), config: ConfigValues::default() }, pname2.clone(), cx))),
                );
            }
        }
        Self::panel_frame("Replace instance", body, cx.listener(|this, _: &ClickEvent, _, cx| {
            this.replace_panel = None;
            cx.notify();
        }), "rep-close")
        .into_any_element()
    }
}

/// Manipulator context menu actions.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum TriadAction {
    MoveToOrigin,
    /// Align triad axis k with world +Z (`true`: anti-align, -Z).
    AlignZ(usize, bool),
    /// Rotate about triad axis k by an angle (radians).
    Rotate(usize, f64),
}

/// Explode step being dragged in the panel (reorder).
#[derive(Clone)]
pub(crate) struct ExplodeStepDrag {
    pub index: usize,
    pub label: SharedString,
}

impl Render for ExplodeStepDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        div().px_2().py_1().rounded_md().bg(pal.panel_raised).border_1().border_color(pal.accent).text_sm().text_color(pal.text).child(self.label.clone())
    }
}

/// Möller–Trumbore ray / triangle intersection (ray parameter).
pub(super) fn ray_triangle(o: DVec3, d: DVec3, a: DVec3, b: DVec3, c: DVec3) -> Option<f64> {
    let e1 = b - a;
    let e2 = c - a;
    let p = d.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1e-18 {
        return None;
    }
    let inv = 1.0 / det;
    let s = o - a;
    let u = s.dot(p) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = s.cross(e1);
    let v = d.dot(q) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = e2.dot(q) * inv;
    (t > 0.0).then_some(t)
}

fn ray_hits_box(o: DVec3, d: DVec3, lo: DVec3, hi: DVec3) -> bool {
    let mut t0 = f64::NEG_INFINITY;
    let mut t1 = f64::INFINITY;
    for k in 0..3 {
        if d[k].abs() < 1e-15 {
            if o[k] < lo[k] || o[k] > hi[k] {
                return false;
            }
            continue;
        }
        let (a, b) = ((lo[k] - o[k]) / d[k], (hi[k] - o[k]) / d[k]);
        t0 = t0.max(a.min(b));
        t1 = t1.min(a.max(b));
    }
    t1 >= t0.max(0.0)
}

impl AssemblyView {
    // ------------------------------------------------------------ picking without the pick buffer

    /// Face under the cursor by ray casting the display meshes (used while a
    /// drag holds the pointer and the pick buffer is not updated), skipping
    /// the instances in `exclude`.
    pub(super) fn raycast(&self, position: DVec2, exclude: &[InstanceId], cx: &App) -> Option<AsmPickTarget> {
        let (o, d) = self.viewport.read(cx).ray(position);
        let mut best: Option<(f64, AsmPickTarget)> = None;
        for (oi, occ) in self.result.occurrences.iter().enumerate() {
            if occ.hidden || exclude.contains(&occ.top()) || !matches!(occ.kind, BodyKind::Solid | BodyKind::Surface) {
                continue;
            }
            if self.isolate.as_ref().is_some_and(|s| !s.contains(&occ.top())) {
                continue;
            }
            if let Some((lo, hi)) = occ.world_bounds()
                && !ray_hits_box(o, d, lo - DVec3::splat(1e-6), hi + DVec3::splat(1e-6))
            {
                continue;
            }
            let inv = occ.transform.inverse();
            let (lo, ld) = (inv.transform_point3(o), inv.transform_vector3(d));
            let m = &occ.mesh;
            let pos = |i: u32| m.positions.get(i as usize).map(|p| DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64)).unwrap_or_default();
            for (fi, &(start, count)) in m.face_ranges.iter().enumerate() {
                let Some(tris) = m.indices.get(start as usize..(start + count) as usize) else { continue };
                for t in tris.as_chunks::<3>().0 {
                    if let Some(tt) = ray_triangle(lo, ld, pos(t[0]), pos(t[1]), pos(t[2]))
                        && best.as_ref().is_none_or(|(b, _)| tt < *b)
                        && let Some(r) = occ.face_refs.get(fi)
                    {
                        best = Some((tt, AsmPickTarget { path: occ.path.clone(), reference: r.clone(), kind: PickKind::Face, occ: oi, anchor: None }));
                    }
                }
            }
        }
        best.map(|b| b.1)
    }

    // ------------------------------------------------------------ triad manipulator

    /// Snap frame for the manipulator center near the cursor: assembly and
    /// Part Studio mate connectors, the origin, inference points of the face
    /// under the cursor and its vertices.
    pub(super) fn triad_snap(&self, position: DVec2, cx: &App) -> Option<Frame> {
        let vp = self.viewport.read(cx);
        let units = self.units(cx);
        let exclude: Vec<InstanceId> = Vec::new();
        let mut best: Option<(f64, Frame)> = None;
        let mut consider = |f: Frame, bias: f64| {
            if let Some(sp) = vp.project(f.origin) {
                let d = sp.distance(position);
                if d < 14.0 && best.as_ref().is_none_or(|(b, _)| d + bias < *b) {
                    best = Some((d + bias, f));
                }
            }
        };
        consider(Frame::default(), 0.0);
        for c in &self.result.connectors {
            if let Some(f) = c.frame {
                consider(f, 0.0);
            }
        }
        let mut seen = HashSet::new();
        for o in &self.result.occurrences {
            if o.hidden || !seen.insert(o.path.clone()) {
                continue;
            }
            let t = noshape_model::assembly::eval::matrix_frame(&o.transform);
            for c in &o.context.connectors {
                consider(t.mul(&Frame::from_axes(c.origin, c.z, c.x)), 0.0);
            }
        }
        if let Some(hit) = self.raycast(position, &exclude, cx)
            && let Some(o) = self.result.occurrences.get(hit.occ)
        {
            let t = noshape_model::assembly::eval::matrix_frame(&o.transform);
            for p in inference_candidates(&o.context, Some(&o.body), &hit.reference, &units) {
                consider(t.mul(&p.frame), 1.0);
            }
            let rot = self.manipulator(cx).map(|f| f.rotation).unwrap_or_default();
            for v in &o.mesh.vertices {
                let w = o.transform.transform_point3(DVec3::new(v[0] as f64, v[1] as f64, v[2] as f64));
                consider(Frame::new(w, rot), 2.0);
            }
        }
        best.map(|b| b.1)
    }

    pub(super) fn triad_action(&mut self, action: TriadAction, cx: &mut Context<Self>) {
        self.triad_menu = None;
        let Some(f) = self.manipulator(cx) else { return };
        let ids = self.manip_targets(cx);
        let axes = [f.x(), f.y(), f.z()];
        let m = match action {
            TriadAction::MoveToOrigin => Frame::new(-f.origin, glam::DQuat::IDENTITY),
            TriadAction::AlignZ(k, anti) => {
                let r = glam::DQuat::from_rotation_arc(axes[k].normalize(), if anti { DVec3::NEG_Z } else { DVec3::Z });
                Frame::new(f.origin - r * f.origin, r)
            }
            TriadAction::Rotate(k, a) => {
                let r = glam::DQuat::from_axis_angle(axes[k].normalize(), a);
                Frame::new(f.origin - r * f.origin, r)
            }
        };
        let start: HashMap<InstanceId, Placement> = ids.iter().filter_map(|id| self.result.solved.placement(*id).map(|p| (*id, p))).collect();
        if self.explode_edit {
            // In explode mode the action becomes an explode step.
            let step = match action {
                TriadAction::MoveToOrigin => ExplodeStep::translate(ids.clone(), m.origin.into()),
                TriadAction::AlignZ(..) | TriadAction::Rotate(..) => {
                    let (axis, angle) = m.rotation.to_axis_angle();
                    ExplodeStep::rotate(ids.clone(), axis.into(), angle, f.origin.into())
                }
            };
            self.push_explode_step(step, cx);
            self.reapply_display(cx);
            return;
        }
        let targets = Self::moved_placements(&m, &start);
        let all: HashMap<InstanceId, Placement> = self.result.solved.placements.iter().cloned().collect();
        let solved = self.result.prepared.solve(&SolveOptions { targets, start: Some(all), nested_start: Some(self.result.solved.nested.clone()), ..Default::default() });
        let name = self.assembly(cx).and_then(|a| ids.first().and_then(|id| a.instance(*id)).map(|i| i.name.clone())).unwrap_or_default();
        let what = match action {
            TriadAction::MoveToOrigin => "Move to origin",
            TriadAction::AlignZ(..) => "Align with Z",
            _ => "Rotate",
        };
        self.apply_solved(solved, cx);
        self.edit(&format!("{what} : {name}"), cx, |_| {});
        cx.notify();
    }

    pub(super) fn render_triad_menu(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let pal = p();
        let (pos, handle) = self.triad_menu?;
        let item = |id: SharedString, label: String, cx: &mut Context<Self>, a: TriadAction| {
            h_flex()
                .id(id)
                .h(px(26.))
                .px_3()
                .rounded_sm()
                .text_sm()
                .cursor_pointer()
                .hover(|d| d.bg(pal.panel_hover))
                .child(label)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.triad_action(a, cx)))
        };
        let mut menu = v_flex().id("triad-menu").occlude().w(px(210.)).p_1().rounded_md().border_1().border_color(pal.border_strong).bg(pal.panel_raised).shadow_lg().text_color(pal.text);
        // Onshape's manipulator menus: the center offers Move to origin, an
        // axis arrow Align / Anti-align with Z, a rotation arc Rotate 90 / 180 degrees.
        match handle {
            ManipHandle::Axis(k) => {
                menu = menu.child(item("tm-align".into(), "Align with Z".into(), cx, TriadAction::AlignZ(k, false))).child(item("tm-anti".into(), "Anti-align with Z".into(), cx, TriadAction::AlignZ(k, true)));
            }
            ManipHandle::Ring(k) => {
                menu = menu
                    .child(item("tm-rot90".into(), "Rotate 90 degrees".into(), cx, TriadAction::Rotate(k, std::f64::consts::FRAC_PI_2)))
                    .child(item("tm-rot180".into(), "Rotate 180 degrees".into(), cx, TriadAction::Rotate(k, std::f64::consts::PI)));
            }
            ManipHandle::Center | ManipHandle::Plane(_) => {
                menu = menu.child(item("tm-origin".into(), "Move to origin".into(), cx, TriadAction::MoveToOrigin));
            }
        }
        Some(deferred(anchored().position(pos).snap_to_window().child(menu)).with_priority(3).into_any_element())
    }

    // ------------------------------------------------------------ snap mode drag

    /// Connector chosen by the current hover (inference point, origin or
    /// assembly connector), as a click would pick it.
    pub(super) fn connector_from_hover(&mut self, cx: &mut Context<Self>) -> Option<MateConnectorRef> {
        match self.hover_overlay {
            Some(OverlayPick::Origin) => return Some(MateConnectorRef::origin()),
            Some(OverlayPick::Connector(i)) => {
                let id = *self.overlay_connectors.get(i)?;
                let c = self.result.connectors.iter().find(|c| c.id == id)?;
                return Some(MateConnectorRef::assembly_connector(c.id, c.owner));
            }
            _ => {}
        }
        let target = self.hover.clone()?;
        self.update_inference(cx);
        let (path, c) = self.infer.as_ref().zip(self.infer_choice).and_then(|(((path, _), _, c), i)| c.get(i).map(|x| (path.clone(), x.connector(path))))?;
        (path == target.path).then_some(c)
    }

    pub(super) fn start_snap_drag(&mut self, position: DVec2, cx: &mut Context<Self>) {
        let Some(t) = self.hover.clone() else { return };
        let top = t.path[0];
        if self.assembly(cx).and_then(|a| a.instance(top)).is_some_and(|i| i.fixed) {
            // Fixed instances do not move; a click still picks their connector.
            if let Some(c) = self.connector_from_hover(cx) {
                self.snap_drag = Some(SnapDrag { top, press: position, started: false, sources: vec![c], source_ix: 0, target: None, start: Placement::default(), grab: DVec3::ZERO, normal: DVec3::Z });
                self.snap_fixed = true;
            }
            return;
        }
        self.snap_fixed = false;
        let Some(first) = self.connector_from_hover(cx) else { return };
        let units = self.units(cx);
        // Sources: the picked point, then explicit mate connectors on the
        // instance, then the other inference points of the picked entity.
        let mut sources = vec![first];
        for c in self.result.connectors.iter().filter(|c| c.owner == top) {
            sources.push(MateConnectorRef::assembly_connector(c.id, c.owner));
        }
        let mut seen = HashSet::new();
        for o in self.result.occurrences.iter().filter(|o| o.top() == top) {
            if !seen.insert(o.path.clone()) {
                continue;
            }
            for c in &o.context.connectors {
                sources.push(MateConnectorRef::on(top, o.path[1..].to_vec(), c.reference.clone()));
            }
        }
        if let Some(o) = self.result.occurrences.get(t.occ) {
            for p in inference_candidates(&o.context, Some(&o.body), &t.reference, &units) {
                sources.push(p.connector(&t.path));
            }
        }
        let mut uniq: Vec<MateConnectorRef> = Vec::new();
        for s in sources {
            if !uniq.contains(&s) && self.result.connector_world(&s, &units).is_some() {
                uniq.push(s);
            }
        }
        let Some(start) = self.result.solved.placement(top) else { return };
        let (o, d) = self.viewport.read(cx).ray(position);
        let center = self.result.instance_bounds(top).map(|(a, b)| (a + b) * 0.5).unwrap_or(DVec3::ZERO);
        let grab = o + d * (center - o).dot(d);
        let normal = self.viewport.read(cx).camera.forward();
        self.snap_drag = Some(SnapDrag { top, press: position, started: false, sources: uniq, source_ix: 0, target: None, start, grab, normal });
    }

    /// Target connector near the cursor on another instance: assembly mate
    /// connectors and the origin within a few pixels, else the nearest
    /// inference point of the face under the cursor.
    pub(super) fn snap_target(&self, position: DVec2, top: InstanceId, cx: &App) -> Option<MateConnectorRef> {
        let vp = self.viewport.read(cx);
        let near = |w: DVec3| vp.project(w).map(|s| s.distance(position)).filter(|d| *d < 16.0);
        let mut best: Option<(f64, MateConnectorRef)> = None;
        if let Some(d) = near(DVec3::ZERO) {
            best = Some((d, MateConnectorRef::origin()));
        }
        for c in self.result.connectors.iter().filter(|c| c.owner != top) {
            if let Some(f) = c.frame
                && let Some(d) = near(f.origin)
                && best.as_ref().is_none_or(|(b, _)| d < *b)
            {
                best = Some((d, MateConnectorRef::assembly_connector(c.id, c.owner)));
            }
        }
        if let Some((_, c)) = best {
            return Some(c);
        }
        let hit = self.raycast(position, &[top], cx)?;
        let o = self.result.occurrences.get(hit.occ)?;
        let units = self.units(cx);
        let t = noshape_model::assembly::eval::matrix_frame(&o.transform);
        let mut best: Option<(f64, MateConnectorRef)> = None;
        for p in inference_candidates(&o.context, Some(&o.body), &hit.reference, &units) {
            let w = t.mul(&p.frame).origin;
            if let Some(s) = vp.project(w) {
                let d = s.distance(position);
                if best.as_ref().is_none_or(|(b, _)| d < *b) {
                    best = Some((d, p.connector(&hit.path)));
                }
            }
        }
        best.map(|(_, c)| c)
    }

    /// Placement of the dragged instance for the current source / target.
    fn snap_preview_placement(&self, position: DVec2, cx: &App) -> Option<Placement> {
        let s = self.snap_drag.as_ref()?;
        let units = self.units(cx);
        if let (Some(src), Some(dst)) = (s.sources.get(s.source_ix), &s.target) {
            // Source frame relative to the instance, from its start placement.
            let cur = self.result.solved.placement(s.top)?;
            let src_now = self.result.connector_world(src, &units)?;
            let src_start = placement_to_frame(&s.start).mul(&placement_to_frame(&cur).inverse().mul(&src_now));
            let dst = self.result.connector_world(dst, &units)?;
            return Some(noshape_model::assembly::eval::snap_placement(&s.start, &src_start, &dst));
        }
        // No target: the instance follows the cursor in the camera plane.
        let hit = self.ray_plane(position, s.grab, s.normal, cx)?;
        let mut p = s.start;
        p.translation = (DVec3::from(p.translation) + (hit - s.grab)).into();
        Some(p)
    }

    pub(super) fn update_snap_drag(&mut self, position: DVec2, cx: &mut Context<Self>) {
        let fixed = self.snap_fixed;
        let Some(s) = &mut self.snap_drag else { return };
        if !s.started {
            if position.distance(s.press) <= 3.0 || fixed {
                if fixed && position.distance(s.press) > 3.0 {
                    self.message = Some((false, "Snap: fixed instances cannot be dragged; drag the other instance onto them.".into()));
                    cx.notify();
                }
                return;
            }
            s.started = true;
            self.message = Some((false, "Snap: drop onto a mate connector of another instance (Ctrl cycles the dragged connector).".into()));
        }
        self.pointer = position;
        let top = s.top;
        let target = self.snap_target(position, top, cx);
        if let Some(s) = &mut self.snap_drag {
            s.target = target;
        }
        self.refresh_snap_preview(cx);
    }

    pub(super) fn refresh_snap_preview(&mut self, cx: &mut Context<Self>) {
        let Some(top) = self.snap_drag.as_ref().map(|s| s.top) else { return };
        let Some(p) = self.snap_preview_placement(self.pointer, cx) else { return };
        let mut solved = self.result.solved.clone();
        for (id, pl) in &mut solved.placements {
            if *id == top {
                *pl = p;
            }
        }
        self.apply_solved(solved, cx);
        self.rebuild_scene(true, cx);
        cx.notify();
    }

    /// Ctrl: next connector on the dragged instance.
    pub(super) fn cycle_snap_source(&mut self, cx: &mut Context<Self>) {
        if self.cycle_insert_source(cx) {
            return;
        }
        let Some(s) = &mut self.snap_drag else { return };
        if !s.started || s.sources.len() < 2 {
            return;
        }
        s.source_ix = (s.source_ix + 1) % s.sources.len();
        self.refresh_snap_preview(cx);
    }

    pub(super) fn finish_snap_drag(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(s) = self.snap_drag.take() else { return };
        if !s.started {
            // A click: the click-click snap flow.
            if let Some(c) = s.sources.first().cloned() {
                self.snap_pick(c, cx);
            }
            self.refresh_scene(false, cx);
            cx.notify();
            return;
        }
        let (Some(src), Some(dst)) = (s.sources.get(s.source_ix).cloned(), s.target.clone()) else {
            // Dropped on nothing: back to where it was.
            self.message = Some((false, "Snap: no target mate connector under the cursor.".into()));
            self.schedule_eval(window, cx);
            return;
        };
        let snapped = self.result.solved.placement(s.top).unwrap_or(s.start);
        // The mate dialog opens with both connectors filled (Fastened by default).
        self.snap = None;
        self.message = None;
        self.start_mate(MateType::Fastened, window, cx);
        self.snap_start = Some((s.top, snapped));
        if let Some(d) = self.mate.as_ref().map(|m| m.dialog.clone()) {
            d.update(cx, |d, cx| d.prefill(src, dst, cx));
        }
        self.update_capture(cx);
        self.schedule_eval(window, cx);
        cx.notify();
    }

    // ------------------------------------------------------------ automatic pattern update

    /// Re-sync part patterns / replicates whose target geometry changed.
    pub(super) fn resync_patterns(&mut self, cx: &mut Context<Self>) {
        let Some(asm) = self.assembly(cx) else { return };
        if !asm.patterns.iter().any(|p| matches!(p.kind, PatternKind::PartPattern { .. } | PatternKind::Replicate { .. })) {
            return;
        }
        let stale = analysis::stale_matched_patterns(&self.result, asm, &self.units(cx));
        if stale.is_empty() {
            return;
        }
        let names: Vec<String> = stale.iter().filter_map(|(id, _)| asm.pattern(*id).map(|p| p.name.clone())).collect();
        self.edit(&format!("Update : {}", names.join(", ")), cx, |a| {
            for (id, t) in stale {
                a.sync_matched_pattern(id, t);
            }
        });
        self.message = Some((false, format!("{} updated to the changed geometry.", names.join(", "))));
    }

    // ------------------------------------------------------------ explode steps

    pub(super) fn move_explode_step(&mut self, from: usize, to: usize, cx: &mut Context<Self>) {
        let Some(i) = self.explode else { return };
        if from == to {
            return;
        }
        self.edit("Reorder : Explode steps", cx, |a| {
            if let Some(v) = a.exploded_views.get_mut(i) {
                v.move_step(from, to);
            }
        });
        self.step_edit = None;
        self.explode_progress = None;
        self.reapply_display(cx);
    }

    pub(super) fn open_step_edit(&mut self, k: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(i) = self.explode else { return };
        let Some(step) = self.assembly(cx).and_then(|a| a.exploded_views.get(i).and_then(|v| v.steps.get(k).cloned())) else { return };
        let units = self.units(cx);
        let fmt_len = |v: f64| noshape_units::format_quantity(Quantity::meters(v), &units);
        let texts = [fmt_len(step.translation[0]), fmt_len(step.translation[1]), fmt_len(step.translation[2]), noshape_units::format_quantity(Quantity::radians(step.rotation_angle), &units)];
        let inputs: [Entity<InputState>; 4] = std::array::from_fn(|j| {
            let t = texts[j].clone();
            cx.new(|cx| InputState::new(window, cx).default_value(t))
        });
        self.step_edit = Some(StepEdit { view: i, step: k, instances: step.instances.clone(), inputs, rotation: step.rotation_axis.is_some() });
        cx.notify();
    }

    fn accept_step_edit(&mut self, cx: &mut Context<Self>) {
        let Some(s) = self.step_edit.take() else { return };
        let units = self.units(cx);
        let val = |st: &Entity<InputState>, dim: Dim| noshape_units::evaluate_as(st.read(cx).value().as_ref(), dim, &units, &EmptyScope).map(|q| q.value);
        let (tx, ty, tz, ang) = (val(&s.inputs[0], Dim::LENGTH), val(&s.inputs[1], Dim::LENGTH), val(&s.inputs[2], Dim::LENGTH), val(&s.inputs[3], Dim::ANGLE));
        let (Ok(tx), Ok(ty), Ok(tz), Ok(ang)) = (tx, ty, tz, ang) else {
            self.message = Some((true, "Explode step: enter valid distances and angle.".into()));
            self.step_edit = Some(s);
            cx.notify();
            return;
        };
        if s.instances.is_empty() {
            self.message = Some((false, "Explode step: select at least one instance.".into()));
            self.step_edit = Some(s);
            cx.notify();
            return;
        }
        let (vi, k, ids, rotation) = (s.view, s.step, s.instances.clone(), s.rotation);
        self.edit("Edit : Explode step", cx, |a| {
            if let Some(step) = a.exploded_views.get_mut(vi).and_then(|v| v.steps.get_mut(k)) {
                step.instances = ids;
                step.translation = [tx, ty, tz];
                if rotation {
                    step.rotation_angle = ang;
                }
            }
        });
        self.explode_progress = None;
        self.reapply_display(cx);
    }

    pub(super) fn render_step_edit(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let Some(s) = &self.step_edit else { return div().into_any_element() };
        let names: HashMap<InstanceId, String> = self.assembly(cx).map(|a| a.instances.iter().map(|i| (i.id, i.name.clone())).collect()).unwrap_or_default();
        let who = if s.instances.is_empty() { "None".to_string() } else { s.instances.iter().filter_map(|i| names.get(i).cloned()).collect::<Vec<_>>().join(", ") };
        let mut body = v_flex()
            .p_3()
            .gap_2()
            .child(section("INSTANCES"))
            .child(
                h_flex().gap_2().child(div().flex_1().text_sm().truncate().child(who)).child(Button::new("se-usesel").label("Use selection").ghost().xsmall().on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    let sel = this.selected.clone();
                    if let Some(s) = &mut this.step_edit
                        && !sel.is_empty()
                    {
                        s.instances = sel;
                    }
                    cx.notify();
                }))),
            )
            .child(section("TRANSLATION"))
            .child(field_row("X", Input::new(&s.inputs[0]).xsmall()))
            .child(field_row("Y", Input::new(&s.inputs[1]).xsmall()))
            .child(field_row("Z", Input::new(&s.inputs[2]).xsmall()));
        if s.rotation {
            body = body.child(section("ROTATION")).child(field_row("Angle", Input::new(&s.inputs[3]).xsmall()));
        }
        body = body.child(h_flex().justify_end().gap_2().child(Button::new("se-ok").label("OK").primary().small().on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.accept_step_edit(cx)))));
        let title = format!("Explode step {}", s.step + 1);
        let _ = pal;
        Self::panel_frame(&title, body, cx.listener(|this, _: &ClickEvent, _, cx| {
            this.step_edit = None;
            cx.notify();
        }), "se-close")
        .into_any_element()
    }

    // ------------------------------------------------------------ replace with geometric matching

    /// Replace instances, then re-point mates whose geometry names do not
    /// exist on the replacement to geometrically matching faces / edges.
    pub(super) fn replace_with_matching(&mut self, id: InstanceId, all: bool, source: InstanceSource, base: String, cx: &mut Context<Self>) {
        let old = self.result.clone();
        let evaluator = self.evaluator.clone();
        let doc = self.doc.read(cx).session.doc.clone();
        let (studio, body, config) = match &source {
            InstanceSource::Part { studio, body, config } => (Some(*studio), Some(body.clone()), config.clone()),
            InstanceSource::PartStudio { studio, config } => (Some(*studio), None, config.clone()),
            _ => (None, None, ConfigValues::default()),
        };
        cx.spawn(async move |this, cx| {
            let ctx = match studio {
                Some(t) => cx.background_spawn(async move { evaluator.lock().studio(&doc, t, &config).map(|r| r.context.clone()) }).await,
                None => None,
            };
            let _ = this.update(cx, |this: &mut Self, cx| {
                let r = this.edit(&format!("Replace instance : {base}"), cx, |a| {
                    let ids = a.replace_instance(id, source, &base, all);
                    let n = ctx.as_ref().map(|c| analysis::remap_replaced(a, &ids, &old, c, body.as_ref())).unwrap_or(0);
                    (ids.len(), n)
                });
                let (count, remapped) = r.unwrap_or((0, 0));
                this.message = Some((
                    false,
                    if remapped > 0 { format!("Replaced {count} instance(s); {remapped} mate reference(s) matched to the nearest geometry of the same type.") } else { format!("Replaced {count} instance(s); mates are kept where the new geometry has the same names.") },
                ));
                cx.notify();
            });
        })
        .detach();
    }
}

impl AssemblyView {
    // ------------------------------------------------------------ insert from another document

    /// Version and Part Studio chooser after picking another document.
    pub(super) fn render_ext_insert_chooser(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (info, vi) = self.ext_pick.clone()?;
        let pal = p();
        let version = info.versions.get(vi)?.clone();
        let studios: Vec<(String, String)> = crate::derived::external_snapshot(&info.path, version.microversion).map(|d| crate::derived::DocStudios::sources(&d)).unwrap_or_default();
        let view = cx.entity();
        let versions = info.versions.clone();
        let version_menu = Button::new("ins-ext-version").label(version.label.clone()).outline().xsmall().dropdown_caret(true).dropdown_menu(move |mut m, _, _| {
            for (i, v) in versions.iter().enumerate() {
                let view = view.clone();
                m = m.item(PopupMenuItem::new(v.label.clone()).checked(i == vi).on_click(move |_, _, cx| {
                    view.update(cx, |this, cx| {
                        if let Some(p) = &mut this.ext_pick {
                            p.1 = i;
                        }
                        cx.notify();
                    })
                }));
            }
            m
        });
        let empty = studios.is_empty();
        let mut col = v_flex()
            .gap_1()
            .p_1p5()
            .rounded_md()
            .border_1()
            .border_color(pal.border_strong)
            .child(
                h_flex()
                    .gap_2()
                    .child(div().flex_1().text_xs().font_weight(FontWeight::SEMIBOLD).truncate().child(info.name.clone()))
                    .child(version_menu)
                    .child(Button::new("ins-ext-x").icon(IconName::Close).ghost().xsmall().on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.ext_pick = None;
                        cx.notify();
                    }))),
            )
            .when(empty, |d| d.child(div().text_xs().text_color(pal.text_faint).child("No Part Studios in this version")));
        for (tab, name) in studios {
            let r = crate::derived::ExternalRef { path: info.path.clone(), microversion: version.microversion, tab: tab.clone() };
            let n2 = name.clone();
            col = col.child(
                h_flex()
                    .id(SharedString::from(format!("ins-ext-{tab}")))
                    .h(px(24.))
                    .px_2()
                    .gap_2()
                    .rounded_sm()
                    .cursor_pointer()
                    .text_sm()
                    .hover(|d| d.bg(pal.panel_hover))
                    .child(svg().path(icon_path("part-studio")).size_3p5().text_color(pal.accent))
                    .child(div().flex_1().truncate().child(name))
                    .child(div().text_xs().text_color(pal.text_faint).child(if self.insert_rigid { "rigid" } else { "parts" }))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.insert_external(r.key(), n2.clone(), cx))),
            );
        }
        Some(col.into_any_element())
    }

    /// Insert the parts (or the whole Part Studio, rigid) of an external Part Studio.
    fn insert_external(&mut self, key: String, studio_name: String, cx: &mut Context<Self>) {
        let evaluator = self.evaluator.clone();
        let rigid = self.insert_rigid;
        self.ext_pick = None;
        let k2 = key.clone();
        cx.spawn(async move |this, cx| {
            let parts: Vec<(BodyId, String)> = cx
                .background_spawn(async move {
                    evaluator
                        .lock()
                        .external_studio(&k2, &ConfigValues::default())
                        .map(|(_, r)| r.display.bodies.iter().filter(|b| matches!(b.kind, BodyKind::Solid | BodyKind::Surface)).map(|b| (b.id.clone(), b.name.clone())).collect())
                        .unwrap_or_default()
                })
                .await;
            let _ = this.update(cx, |this: &mut Self, cx| {
                if parts.is_empty() {
                    this.message = Some((true, "The other document's Part Studio has no parts (or could not be read).".into()));
                    cx.notify();
                    return;
                }
                let items: Vec<(Option<BodyId>, String)> = if rigid { vec![(None, studio_name.clone())] } else { parts.into_iter().map(|(b, n)| (Some(b), n)).collect() };
                let ids = this.edit(&format!("Insert : {studio_name} (other document)"), cx, |a| {
                    let mut ids = Vec::new();
                    for (k, (body, name)) in items.into_iter().enumerate() {
                        let id = InstanceId(a.next_id());
                        let n = a.next_instance_name(&name);
                        let placement = Placement { translation: [k as f64 * 0.03, 0.0, 0.0], rotation: [0.0, 0.0, 0.0, 1.0] };
                        a.instances.push(Instance { id, name: n, source: InstanceSource::External { key: key.clone(), body, config: ConfigValues::default() }, placement, fixed: false, hidden: false, suppressed: false, lock: SubassemblyLock::Flexible, nested: Vec::new() });
                        ids.push(id);
                    }
                    ids
                });
                if let Some(ids) = ids {
                    this.start_placing(ids, cx);
                }
            });
        })
        .detach();
    }

    /// Look up (in the background) whether external references have newer versions.
    pub(super) fn check_external_updates(&mut self, cx: &mut Context<Self>) {
        let Some(asm) = self.assembly(cx) else { return };
        let keys: Vec<String> = asm.instances.iter().filter_map(|i| i.source.external_key().map(|k| k.to_string())).filter(|k| !self.ext_updates.contains_key(k) && !self.ext_checking.contains(k)).collect();
        for key in keys {
            self.ext_checking.insert(key.clone());
            cx.spawn(async move |this, cx| {
                let k2 = key.clone();
                let newer = cx.background_spawn(async move { crate::derived::ExternalRef::parse(&k2).and_then(|r| crate::derived::newer_version(&r)) }).await;
                let _ = this.update(cx, |this: &mut Self, cx| {
                    this.ext_checking.remove(&key);
                    this.ext_updates.insert(key, newer);
                    cx.notify();
                });
            })
            .detach();
        }
    }

    /// Update every instance of an external reference to a newer version.
    pub(super) fn update_external(&mut self, key: &str, to: &crate::derived::VersionChoice, cx: &mut Context<Self>) {
        let Some(r) = crate::derived::ExternalRef::parse(key) else { return };
        let new_key = crate::derived::ExternalRef { microversion: to.microversion, ..r }.key();
        let from = key.to_string();
        let n = self.edit(&format!("Update to {}", to.label), cx, |a| a.update_external(&from, &new_key)).unwrap_or(0);
        self.message = Some((false, format!("Updated {n} instance(s) to {}.", to.label)));
        cx.notify();
    }
}
