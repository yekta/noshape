//! Assembly tab panels and dialogs (child module of `assembly_view`):
//! assembly mate connector dialog, pattern / mirror / replicate dialog,
//! triad typed values, exploded views (step editor, playback), named
//! positions, per-instance configuration, replace instance, mode banners
//! and the extra list rows (origin, connectors, patterns).

use super::*;
use crate::os_ui as os;
use gpui_kit::component::input::InputEvent;
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
    Explode(usize),
}

/// Where a feature dialog's preview slider rests (as the Part Studio's: from
/// there to the right end the feature shows fully, to the left it fades).
pub(super) const PREVIEW_REST: f32 = 0.7;

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
    /// Editing a mate dialog field's implicit connector: changes go into the
    /// mate's reference and it stays implicit (live 578–580; ✕ only).
    pub implicit: bool,
    /// Move checked (the X/Y/Z translation and rotation apply).
    pub moving: bool,
    /// Preview slider (0 before the connector, 1 after; rests at
    /// [`PREVIEW_REST`]) and its track bounds.
    pub preview: f32,
    preview_track: std::rc::Rc<std::cell::Cell<Bounds<Pixels>>>,
    /// Origin type and Rotation axis selects.
    origin_sel: Entity<crate::os_select::OsSelect>,
    axis_sel: Entity<crate::os_select::OsSelect>,
    _subs: Vec<Subscription>,
}

impl McSession {
    /// The implicit connector reference with this session's flip, reorient
    /// and Move values.
    pub fn to_implicit(&self, cx: &App) -> Option<MateConnectorRef> {
        let mut r = self.on.clone()?;
        let expr = |i: usize| {
            let t = self.inputs[i].read(cx).value().to_string();
            (!t.trim().is_empty()).then(|| ExprValue::new(t))
        };
        r.flip = self.flip;
        r.rotate_quarters = self.reorient;
        let m = noshape_model::assembly::ConnectorMove { translation: [expr(0), expr(1), expr(2)], rotation_axis: self.rot_axis, rotation: expr(3), off: !self.moving };
        r.moved = (m.translation.iter().any(|t| t.is_some()) || m.rotation.is_some()).then(|| Box::new(m));
        Some(r)
    }

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
        c.move_off = !self.moving;
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

/// The Replace instance dialog (live assembly/833–836): the instances to
/// replace, the chosen replacement (source, name), Replace all instances,
/// and whether the replacement picker is open.
pub(crate) struct ReplaceSession {
    pub instances: Vec<InstanceId>,
    pub all: bool,
    pub replacement: Option<(InstanceSource, String)>,
    pub picking: bool,
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
                // An implicit connector stays implicit: the panel edits the
                // mate's own reference (live 578–580).
                let mut on = c.clone();
                on.flip = false;
                on.rotate_quarters = 0;
                on.moved = None;
                self.open_mc_inner(None, Some(field), window, cx);
                let text = |e: &Option<ExprValue>| e.as_ref().map(|e| e.text.clone()).unwrap_or_default();
                if let Some(m) = &mut self.mc {
                    m.implicit = true;
                    m.name = "Mate connector".into();
                    m.on = Some(on);
                    // Live 578: the Origin entity field is active (blue), so a
                    // pick replaces the entity.
                    m.active = true;
                    m.slot = 0;
                    m.flip = c.flip;
                    m.reorient = c.rotate_quarters;
                    if let Some(mv) = c.moved.as_deref() {
                        m.moving = !mv.off;
                        m.rot_axis = mv.rotation_axis;
                        let vals = [text(&mv.translation[0]), text(&mv.translation[1]), text(&mv.translation[2]), text(&mv.rotation)];
                        for (st, v) in m.inputs.clone().iter().zip(vals) {
                            st.update(cx, |s, cx| s.set_value(v, window, cx));
                        }
                    }
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
        let origin_sel = cx.new(|_| crate::os_select::OsSelect::new("mcd-origin-type"));
        let axis_sel = cx.new(|_| crate::os_select::OsSelect::new("mcd-rot-axis"));
        subs.push(cx.subscribe_in(&origin_sel, window, |this: &mut Self, _, ev: &crate::os_select::OsSelectEvent, window, cx| {
            let crate::os_select::OsSelectEvent::Picked(id) = ev;
            let between = id.as_ref() == "BETWEEN_ENTITIES";
            if let Some(m) = &mut this.mc {
                m.between = between;
                if between && m.extra[0].is_none() {
                    m.active = true;
                    m.slot = 1;
                }
            }
            this.update_capture(cx);
            this.schedule_eval(window, cx);
            cx.notify();
        }));
        subs.push(cx.subscribe_in(&axis_sel, window, |this: &mut Self, _, ev: &crate::os_select::OsSelectEvent, window, cx| {
            let crate::os_select::OsSelectEvent::Picked(id) = ev;
            if let Some(m) = &mut this.mc {
                m.rot_axis = match id.as_ref() {
                    "ABOUT_X" => 0,
                    "ABOUT_Y" => 1,
                    _ => 2,
                };
            }
            this.schedule_eval(window, cx);
            cx.notify();
        }));
        let moving = existing.as_ref().is_some_and(|c| !c.move_off && (c.translation.iter().any(|t| t.is_some()) || c.rotation.is_some()));
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
            implicit: false,
            moving,
            preview: PREVIEW_REST,
            preview_track: Default::default(),
            origin_sel,
            axis_sel,
            _subs: subs,
        });
        self.insert_open = false;
        self.update_capture(cx);
        self.schedule_eval(window, cx);
        cx.notify();
    }

    /// Implicit connector edits go straight into the mate dialog's field.
    pub(super) fn sync_implicit_mc(&mut self, cx: &mut Context<Self>) {
        let Some(s) = self.mc.as_ref().filter(|s| s.implicit) else { return };
        let (Some(field), Some(r)) = (s.for_mate, s.to_implicit(cx)) else { return };
        let Some(d) = self.mate.as_ref().map(|m| m.dialog.clone()) else { return };
        if d.read(cx).field(field) != Some(&r) {
            d.update(cx, |d, cx| d.replace_field(field, r, cx));
        }
    }

    pub(super) fn close_mc(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_implicit_mc(cx);
        if self.mc.take().is_some() {
            self.infer = None;
            window.focus(&self.focus, cx);
            self.update_capture(cx);
            self.schedule_eval(window, cx);
            cx.notify();
        }
    }

    fn accept_mc(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.mc.as_ref().is_some_and(|s| s.implicit) {
            self.close_mc(window, cx);
            return;
        }
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

    /// The Mate connector dialog, Onshape's 220 px feature dialog (live
    /// dom/dialog-mate-connector.txt, assembly-mate-chip-connector-dialog.txt;
    /// fields of the std `mateConnector` feature without the owner entity):
    /// Origin type ▾, Origin entity (+ Between entity), ☐ Realign (Primary /
    /// Secondary axis entity), ☐ Move (X / Y / Z translation, Rotation axis ▾,
    /// Rotation angle), then the flip / reorient icons and `?`. An implicit
    /// connector opened from a mate dialog chip has a close button only.
    pub(super) fn render_mc_panel(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let Some(s) = &self.mc else { return div().into_any_element() };
        let labeler = self.labeler(cx);
        // The field names the entity itself ("Face of Part 1 <3>", live 578).
        let entity = move |r: &MateConnectorRef| -> SharedString {
            if r.instance == InstanceId::ORIGIN && r.connector.is_none() {
                return "Origin".into();
            }
            let mut r = r.clone();
            if let Reference::Implicit { on, .. } = &r.reference {
                r.reference = (**on).clone();
            }
            labeler(&r).into()
        };
        let field = |this: &Self, slot: usize, id: &'static str, label: &'static str, cx: &mut Context<Self>| -> AnyElement {
            let Some(s) = this.mc.as_ref() else { return div().into_any_element() };
            let value = if slot == 0 { s.on.as_ref() } else { s.extra[slot - 1].as_ref() };
            let active = s.active && s.slot == slot;
            let rows: Vec<AnyElement> = value
                .map(|v| {
                    let text = entity(v);
                    os::query_row(SharedString::from(format!("{id}-row")), text.clone())
                        .child(
                            div()
                                .id(SharedString::from(format!("{id}-del")))
                                .px(px(5.))
                                .text_size(px(16.67))
                                .text_color(rgb(0x666666))
                                .cursor_pointer()
                                .tooltip(crate::ui::tooltip(format!("Delete {text}")))
                                .child("×")
                                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                                    cx.stop_propagation();
                                    if let Some(m) = &mut this.mc {
                                        if slot == 0 {
                                            m.on = None;
                                        } else {
                                            m.extra[slot - 1] = None;
                                        }
                                        m.active = true;
                                        m.slot = slot;
                                    }
                                    this.update_capture(cx);
                                    this.schedule_eval(window, cx);
                                    cx.notify();
                                })),
                        )
                        .into_any_element()
                })
                .into_iter()
                .collect();
            div()
                .px(px(3.))
                .pt(px(5.))
                .child(os::query_box(SharedString::from(id.to_string()), label, active, rows).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if let Some(m) = &mut this.mc {
                        m.active = true;
                        m.slot = slot;
                    }
                    this.update_capture(cx);
                    this.refresh_scene(false, cx);
                    cx.notify();
                })))
                .into_any_element()
        };
        let (between, realign, moving, implicit) = (s.between, s.realign, s.moving, s.implicit);
        let origin = field(self, 0, "mcd-origin", "Origin entity", cx);
        let between_field = between.then(|| field(self, 1, "mcd-between", "Between entity", cx));
        let realign_fields = realign.then(|| [field(self, 2, "mcd-primary", "Primary axis entity", cx), field(self, 3, "mcd-secondary", "Secondary axis entity", cx)]);
        let Some(s) = &self.mc else { return div().into_any_element() };
        s.origin_sel.update(cx, |sel, _| sel.set_items(vec![("ON_ENTITY".into(), "On entity".into()), ("BETWEEN_ENTITIES".into(), "Between entities".into())], if between { "BETWEEN_ENTITIES" } else { "ON_ENTITY" }));
        let axis_id = ["ABOUT_X", "ABOUT_Y", "ABOUT_Z"][s.rot_axis.min(2) as usize];
        s.axis_sel.update(cx, |sel, _| sel.set_items(vec![("ABOUT_X".into(), "Rotate about X".into()), ("ABOUT_Y".into(), "Rotate about Y".into()), ("ABOUT_Z".into(), "Rotate about Z".into())], axis_id));
        let check = |id: &'static str, label: &'static str, on: bool, f: fn(&mut McSession, bool)| {
            os::checkbox(id, label, on).on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                if let Some(m) = &mut this.mc {
                    f(m, !on);
                }
                this.update_capture(cx);
                this.schedule_eval(window, cx);
                cx.notify();
            }))
        };
        let value_row = |label: &'static str, st: &Entity<InputState>| os::param_row(label, Input::new(st).appearance(false).text_size(px(13.33)).text_right().h(px(25.)));
        let icon_tool = |id: &'static str, icon: &'static str, tip: &'static str, on: bool, cx: &mut Context<Self>| {
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
                .child(crate::assets::icon(icon, 20., cx))
        };
        let (flip, reorient) = (s.flip, s.reorient);
        let body = v_flex()
            .pb(px(1.))
            .child(div().id("mcd-origin-type-tt").w(px(194.)).h(px(28.)).tooltip(crate::ui::tooltip("Origin type")).child(s.origin_sel.clone()))
            .child(origin)
            .children(between_field)
            .child(div().h(px(5.)))
            .child(check("mcd-realign", "Realign", realign, |m, v| {
                m.realign = v;
                if v && m.extra[1].is_none() {
                    m.active = true;
                    m.slot = 2;
                }
            }))
            .children(realign_fields.into_iter().flatten())
            .child(check("mcd-move", "Move", moving, |m, v| m.moving = v))
            .when(moving, |d| {
                d.child(value_row("X translation", &s.inputs[0]))
                    .child(value_row("Y translation", &s.inputs[1]))
                    .child(value_row("Z translation", &s.inputs[2]))
                    .child(div().id("mcd-rot-axis-tt").w(px(194.)).h(px(28.)).tooltip(crate::ui::tooltip("Rotation axis")).child(s.axis_sel.clone()))
                    .child(value_row("Rotation angle", &s.inputs[3]))
            })
            .child(
                h_flex()
                    .h(px(28.))
                    .child(icon_tool("mcd-flip", "flip-direction-opposite", "Flip primary axis", flip, cx).on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        if let Some(m) = &mut this.mc {
                            m.flip = !m.flip;
                        }
                        this.schedule_eval(window, cx);
                        cx.notify();
                    })))
                    .child(icon_tool("mcd-reorient", "realign-mate-dialog", "Reorient secondary axis", !reorient.is_multiple_of(4), cx).on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        if let Some(m) = &mut this.mc {
                            m.reorient = (m.reorient + 1) % 4;
                        }
                        this.schedule_eval(window, cx);
                        cx.notify();
                    })))
                    .child(div().flex_1())
                    // Live: the standalone dialog's `?` sits on the preview
                    // slider row (dom/dialog-mate-connector.txt).
                    .when(implicit, |d| d.child(div().mr(px(4.)).child(super::props::help_dot()))),
            )
            .when(!implicit, |d| d.child(self.render_mc_preview_slider(cx)));
        let title = if implicit { "Mate connector".to_string() } else { s.name.clone() };
        let ok = s.on.is_some();
        let header = h_flex()
            .h(px(29.))
            .flex_none()
            .bg(rgb(0xffffff))
            .border_b_1()
            .border_color(rgb(0xd7d7d7))
            .child(div().flex_1().pl(px(6.)).text_size(px(13.)).font_weight(FontWeight::MEDIUM).truncate().child(title));
        let header = if implicit {
            // Live 578: a Bootstrap close button at opacity 0.5.
            header.child(
                div()
                    .id("mcd-close")
                    .size(px(28.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .opacity(0.5)
                    .cursor_pointer()
                    .child(os::glyph(os::GLYPH_CLOSE, 20., rgb(0x000000)))
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.close_mc(window, cx))),
            )
        } else {
            header
                .child(
                    div()
                        .id("mcd-ok")
                        .size(px(28.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(pal.accept)
                        .when(!ok, |d| d.opacity(0.2))
                        .when(ok, |d| d.cursor_pointer())
                        .tooltip(crate::ui::tooltip("Accept (Enter)"))
                        .child(Icon::new(IconName::Check).with_size(px(18.)).text_color(rgb(0xffffff)))
                        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.accept_mc(window, cx))),
                )
                .child(
                    div()
                        .id("mcd-cancel")
                        .size(px(28.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .hover(|d| d.bg(pal.panel_hover))
                        .tooltip(crate::ui::tooltip("Cancel (Esc)"))
                        .cursor(gpui_kit::CursorStyle::Arrow).child(crate::assets::icon("dialog-cancel", 18., cx))
                        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.close_mc(window, cx))),
                )
        };
        v_flex()
            .id("mc-dialog")
            .w(px(220.))
            .bg(pal.dialog_bg)
            .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
            .text_color(pal.text)
            .child(header)
            .child(body)
            .into_any_element()
    }

    /// The connector dialog's preview slider (a feature dialog's: 112 px,
    /// 2 px `#777` track, 11 px white handle ringed `#888`) and `?`.
    fn render_mc_preview_slider(&self, cx: &mut Context<Self>) -> AnyElement {
        const W: f32 = 112.;
        const HANDLE: f32 = 11.;
        let Some(s) = &self.mc else { return div().into_any_element() };
        let (value, track) = (s.preview, s.preview_track.clone());
        let probe = track.clone();
        let set = move |this: &mut Self, x: Pixels, cx: &mut Context<Self>| {
            let b = track.get();
            let v = (f32::from(x - b.left()) / f32::from(b.size.width).max(1.)).clamp(0., 1.);
            if let Some(m) = this.mc.as_mut().filter(|m| (m.preview - v).abs() > f32::EPSILON) {
                m.preview = v;
                this.refresh_scene(false, cx);
                cx.notify();
            }
        };
        let set2 = set.clone();
        h_flex()
            .h(px(22.))
            .pl(px(7.))
            .pr(px(4.))
            .child(
                div()
                    .id("mcd-preview")
                    .relative()
                    .w(px(W))
                    .h(px(HANDLE + 2.))
                    .flex_none()
                    .cursor_pointer()
                    .child(div().absolute().left_0().right_0().top(px((HANDLE + 2.) * 0.5 - 1.)).h(px(2.)).bg(rgb(0x777777)))
                    .child(
                        div()
                            .absolute()
                            .top(px(1.))
                            .left(px(value * W - HANDLE * 0.5))
                            .size(px(HANDLE))
                            .rounded_full()
                            .bg(rgb(0xffffff))
                            .border_1()
                            .border_color(rgb(0x888888))
                            .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.6), offset: point(px(0.), px(0.)), blur_radius: px(1.5), spread_radius: px(0.), inset: false }]),
                    )
                    .child(canvas(move |b, _, _| probe.set(b), |_, _, _, _| {}).absolute().top_0().left_0().size_full())
                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, e: &MouseDownEvent, _, cx| {
                        cx.stop_propagation();
                        set(this, e.position.x, cx);
                    }))
                    .on_mouse_move(cx.listener(move |this, e: &MouseMoveEvent, _, cx| {
                        if e.pressed_button == Some(MouseButton::Left) {
                            set2(this, e.position.x, cx);
                        }
                    })),
            )
            .child(div().flex_1())
            .child(super::props::help_dot())
            .into_any_element()
    }

    // ------------------------------------------------------------ list rows

    pub(super) fn origin_row(&self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        h_flex()
            .id("asm-origin-row")
            // Live assembly/10: the Part Studio's origin row (dot icon x 73, name x 86).
            .h(px(crate::ui::ROW_H))
            .pl(px(22.))
            .pr(px(3.))
            .gap(px(3.))
            .text_size(px(crate::ui::ROW_TEXT))
            .text_color(pal.text)
            .hover(|d| d.bg(pal.row_hover))
            .child(crate::ui::tree_icon("origin", self.ui.origin_hidden, cx))
            .child(div().flex_1().when(self.ui.origin_hidden, |d| d.text_color(pal.text_faint)).child("Origin"))
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                if this.ui.ps_in_context.is_some() {
                    this.ui.ps_in_context = Some(None);
                    cx.notify();
                    return;
                }
                if this.pick_mode(cx).is_some() {
                    this.deliver_connector(MateConnectorRef::origin(), window, cx);
                    this.refresh_scene(false, cx);
                    cx.notify();
                }
            }))
            // Onshape's origin row menu: Create Part Studio in context…, Hide | Create empty subassembly.
            .os_context_menu({
                let view = cx.entity();
                let hidden = self.ui.origin_hidden;
                move |menu, window, cx| {
                    let entries = vec![
                        behaviors::MenuEntry::Item {
                            label: "Create Part Studio in context…".into(),
                            icon: Some("create-partstudio-incontext"),
                            disabled: false,
                            act: std::rc::Rc::new(|t: &mut AssemblyView, _: &mut Window, cx: &mut Context<AssemblyView>| t.open_ps_in_context(cx)),
                        },
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
                if this.ui.ps_in_context.is_some() {
                    // Origin of new Part Studio: this connector.
                    this.ui.ps_in_context = Some(Some(id));
                    cx.notify();
                    return;
                }
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
    /// Editing an exploded view: the yellow "Editing Explode 1 [Done]"
    /// banner over a toolbar reduced to Undo / Redo (live 62).
    pub(super) fn explode_banner(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.explode_edit {
            return None;
        }
        let name = self.explode.and_then(|i| self.assembly(cx).and_then(|a| a.exploded_views.get(i).map(|v| v.name.clone()))).unwrap_or_default();
        Some(super::positions::yellow_banner(
            format!("Editing {name}"),
            // Done leaves explode mode (bundle `exitExplodeMode`): assembled again.
            cx.listener(|this, _: &ClickEvent, _, cx| {
                this.message = None;
                this.activate_explode(None, cx);
                cx.notify();
            }),
        ))
    }

    pub(super) fn render_mode_banner(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let pal = p();
        let (text, done): (String, Option<u8>) = if self.explode_edit {
            // The yellow "Editing Explode 1 [Done]" banner sits in the toolbar row.
            return None;
        } else if let Some(s) = &self.snap {
            (if s.is_some() { "Snap mode: click the target face or mate connector".into() } else { "Snap mode: click a face or mate connector on the instance to move".into() }, Some(1))
        } else if self.show_mates_mode {
            ("Show mates mode: hover an instance to see its mates, click to pin (H to exit)".into(), Some(2))
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

    /// The docked 712 px Exploded views pane (bundle `explosionItemComponent`
    /// and the pane around it; live 181-strip-exploded-views, 62–64): with no
    /// view, [Add exploded view] over the zero state; else the tree of views
    /// (› expander, explode icon, name; the one in explode mode blue with a
    /// 5 px bar), their steps and the rollback bar, the "Build steps by
    /// dragging components" box (one view with at most one step) and the
    /// footer's [Add exploded view] + help. A click selects a row; a double
    /// click enters / leaves explode mode (a step: up to it, and edits it).
    pub(super) fn render_explode_panel(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let views: Vec<ExplodedView> = self.assembly(cx).map(|a| a.exploded_views.clone()).unwrap_or_default();
        let blue = rgb(0x1651b0);
        let add_button = |id: &'static str, cx: &mut Context<Self>| {
            div()
                .id(id)
                .h(px(26.))
                .px(px(11.))
                .flex()
                .items_center()
                .rounded(px(3.))
                .border_1()
                .border_color(rgb(0xaaaaaa))
                .bg(rgb(0xffffff))
                .text_size(px(13.33))
                .text_color(pal.text)
                .cursor_pointer()
                .hover(|d| d.bg(pal.panel_hover))
                .tooltip(crate::ui::tooltip("Add exploded view"))
                .child("Add exploded view")
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.add_exploded_view(cx)))
        };
        let title = div().pt(px(4.)).child(
            div()
                .h(px(26.))
                .flex()
                .items_center()
                .justify_center()
                .border_b_1()
                .border_color(blue)
                .text_size(px(13.33))
                .font_weight(FontWeight::MEDIUM)
                .text_color(blue)
                .child("Exploded views"),
        );
        let mut pane = v_flex().id("ex-pane").w(px(712.)).h_full().occlude().bg(rgb(0xffffff)).text_color(rgb(0x333333)).child(title);
        if views.is_empty() {
            // Zero state: the button, an illustration and the how-to.
            pane = pane.child(
                v_flex()
                    .flex_1()
                    .items_center()
                    .pt(px(8.))
                    .child(add_button("ex-add-top", cx))
                    .child(div().mt(px(105.)).opacity(0.35).child(crate::assets::icon("explode", 120., cx)))
                    .child(
                        v_flex()
                            .mt(px(16.))
                            .w(px(420.))
                            .items_center()
                            .text_size(px(13.33))
                            .text_color(pal.text_muted)
                            .child(div().text_center().child("Add exploded views to visualize how components fit together in an assembly by moving instances from their mated positions."))
                            .child(div().id("ex-learn").mt(px(4.)).font_weight(FontWeight::BOLD).text_color(rgb(0x094174)).underline().cursor_pointer().child("Learn more about exploded views.").on_click(|_, _, cx| cx.open_url("https://cad.onshape.com/help/Content/exploded_views.htm"))),
                    ),
            );
        } else {
            let mut tree = v_flex().id("ex-tree").flex_shrink_1().min_h_0().overflow_y_scroll().pt(px(1.));
            let sel = self.ui.explode_sel;
            for (i, v) in views.iter().enumerate() {
                let active = self.explode == Some(i);
                let open = active || self.ui.explode_open.contains(&i);
                let row_sel = sel == Some((i, None));
                let name_el: AnyElement = match self.rename_input(RenameTarget::Explode(i), cx) {
                    Some(inp) => inp,
                    None => div().flex_1().min_w_0().truncate().text_size(px(13.33)).text_color(if active { blue } else { pal.text }).child(v.name.clone()).into_any_element(),
                };
                let name = v.name.clone();
                tree = tree.child(
                    h_flex()
                        .id(SharedString::from(format!("ex-row-{i}")))
                        .h(px(26.))
                        .border_l(px(5.))
                        .border_color(if active { blue.into() } else { gpui_kit::transparent_black() })
                        .when(row_sel, |d| d.bg(pal.accent_soft))
                        .when(!row_sel, |d| d.hover(|d| d.bg(pal.row_hover)))
                        .child(
                            div()
                                .id(SharedString::from(format!("ex-exp-{i}")))
                                .w(px(20.))
                                .h_full()
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(crate::ui::glyph(if open { "ui-chevron-down" } else { "ui-chevron-right" }, 11., pal.text))
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    cx.stop_propagation();
                                    if !this.ui.explode_open.remove(&i) {
                                        this.ui.explode_open.insert(i);
                                    }
                                    cx.notify();
                                })),
                        )
                        .child(div().w(px(23.)).flex_none().opacity(0.8).child(crate::assets::icon("explode", 17., cx)))
                        .child(name_el)
                        .on_click(cx.listener(move |this, ev: &ClickEvent, _, cx| {
                            this.ui.explode_sel = Some((i, None));
                            if ev.click_count() >= 2 {
                                // Enter / leave explode mode for this view.
                                if this.explode == Some(i) {
                                    this.activate_explode(None, cx);
                                } else {
                                    this.explode_edit = true;
                                    this.activate_explode(Some(i), cx);
                                }
                            }
                            cx.notify();
                        }))
                        .os_context_menu({
                            let view = cx.entity();
                            move |m, _, _| {
                                let (v1, v2, n) = (view.clone(), view.clone(), name.clone());
                                m.list_no_icons()
                                    .item(crate::os_menu::OsMenuItem::new("Rename").on_click(move |_, window, cx| v1.update(cx, |this, cx| this.start_rename(RenameTarget::Explode(i), n.clone(), window, cx))))
                                    .item(crate::os_menu::OsMenuItem::new("Delete").on_click(move |_, _, cx| v2.update(cx, |this, cx| this.delete_exploded_view(i, cx))))
                            }
                        }),
                );
                if !open {
                    continue;
                }
                // Steps, then the rollback bar after the steps in effect.
                let n = v.steps.len();
                let upto = if active { self.explode_progress.map(|p| p.round() as usize).unwrap_or(n).min(n) } else { n };
                let bar = |id: SharedString, _: &mut Context<Self>| {
                    div()
                        .id(id)
                        .h(px(26.))
                        .pl(px(25. + 15.))
                        .pr(px(5.))
                        .flex()
                        .items_center()
                        .cursor_grab()
                        .tooltip(crate::ui::tooltip("Rollback bar"))
                        .child(div().w_full().h(px(6.)).bg(rgb(0xcccccc)))
                        .on_drag(ExplodeRollbackDrag { view: i }, |d, _, _, cx| cx.new(|_| d.clone()))
                        .into_any_element()
                };
                for (k, st) in v.steps.iter().enumerate() {
                    if k == upto {
                        tree = tree.child(bar(SharedString::from(format!("ex-rb-{i}")), cx));
                    }
                    let rotation = st.rotation_axis.is_some();
                    let step_sel = sel == Some((i, Some(k)));
                    let label: SharedString = format!("{} {}", if rotation { "Rotation" } else { "Translation" }, k + 1).into();
                    let accent = pal.accent;
                    tree = tree.child(
                        h_flex()
                            .id(SharedString::from(format!("ex-step-{i}-{k}")))
                            .h(px(26.))
                            .pl(px(5. + 20. + 15.))
                            .when(k >= upto, |d| d.opacity(0.5))
                            .when(step_sel, |d| d.bg(pal.accent_soft))
                            .when(!step_sel, |d| d.hover(|d| d.bg(pal.row_hover)))
                            .child(div().w(px(23.)).flex_none().child(crate::assets::icon(if rotation { "explode-step-rotation" } else { "explode-step-translation" }, 17., cx)))
                            .child(div().flex_1().min_w_0().truncate().text_size(px(13.33)).child(label.clone()))
                            .on_click(cx.listener(move |this, ev: &ClickEvent, window, cx| {
                                this.ui.explode_sel = Some((i, Some(k)));
                                if ev.click_count() >= 2 {
                                    // Explode mode up to this step, editing it.
                                    if this.explode != Some(i) {
                                        this.explode_edit = true;
                                        this.activate_explode(Some(i), cx);
                                    }
                                    this.set_explode_progress((k + 1) as f64, cx);
                                    this.open_step_edit(k, window, cx);
                                }
                                cx.notify();
                            }))
                            .os_context_menu({
                                let view = cx.entity();
                                move |m, _, _| {
                                    let v = view.clone();
                                    m.list_no_icons().item(crate::os_menu::OsMenuItem::new("Delete").on_click(move |_, _, cx| {
                                        v.update(cx, |this, cx| {
                                            if this.explode != Some(i) {
                                                this.activate_explode(Some(i), cx);
                                            }
                                            this.delete_explode_step(k, cx);
                                        })
                                    }))
                                }
                            })
                            .on_drag(ExplodeStepDrag { index: k, label }, |d, _, _, cx| cx.new(|_| d.clone()))
                            .drag_over::<ExplodeStepDrag>(move |st, _, _, _| st.border_t_2().border_color(accent))
                            .on_drop(cx.listener(move |this, d: &ExplodeStepDrag, _, cx| this.move_explode_step(d.index, k, cx)))
                            .drag_over::<ExplodeRollbackDrag>(move |st, _, _, _| st.border_b_2().border_color(accent))
                            .on_drop(cx.listener(move |this, d: &ExplodeRollbackDrag, _, cx| {
                                if d.view == i {
                                    if this.explode != Some(i) {
                                        this.activate_explode(Some(i), cx);
                                    }
                                    this.set_explode_progress((k + 1) as f64, cx);
                                }
                            })),
                    );
                }
                if upto >= n {
                    tree = tree.child(bar(SharedString::from(format!("ex-rb-{i}")), cx));
                }
            }
            pane = pane.child(tree);
            // The how-to box: one view with at most one step (bundle `A()`).
            if views.len() == 1 && views[0].steps.len() <= 1 {
                pane = pane.child(
                    div().mx(px(18.)).mt(px(18.)).mb(px(14.)).p(px(15.)).rounded(px(4.)).border_1().border_color(rgb(0xa6d5ec)).bg(rgb(0xd9edf7)).text_size(px(13.33)).text_color(rgb(0x094174)).child(
                        v_flex()
                            .child(div().font_weight(FontWeight::BOLD).child("Build steps by dragging components"))
                            .child(div().child("Select components in the graphics area or the instance list, then drag or rotate them using the triad manipulator.")),
                    ),
                );
            }
            // The list and the box sit at the top (live 63); the footer stays at the bottom.
            pane = pane.child(div().flex_1());
        }
        // Footer: [Add exploded view] (when views exist) and help.
        pane.child(
            h_flex()
                .h(px(40.))
                .flex_none()
                .border_t_1()
                .border_color(pal.border)
                .bg(rgb(0xf5f5f5))
                .child(div().flex_1())
                .when(!views.is_empty(), |d| d.child(add_button("ex-add", cx)))
                .child(h_flex().flex_1().justify_end().pr(px(14.)).child(super::props::help_dot())),
        )
        .into_any_element()
    }

    // ------------------------------------------------------------ configuration / replace

    pub(super) fn open_config(&mut self, top: InstanceId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(inst) = self.assembly(cx).and_then(|a| a.instance(top).cloned()) else { return };
        let studio_inputs = |doc: &Document, tab: TabId| match doc.tab(tab).map(|t| &t.content) {
            Some(TabContent::PartStudio(ps)) => ps.configuration.inputs.clone(),
            // Another document's Assembly (External key on an Assembly tab).
            Some(TabContent::Assembly(a)) => a.configuration.inputs.clone(),
            _ => vec![],
        };
        let (inputs, values) = match &inst.source {
            InstanceSource::Part { studio, config, .. } | InstanceSource::PartStudio { studio, config } | InstanceSource::MirroredPart { studio, config, .. } | InstanceSource::Sketch { studio, config, .. } => (studio_inputs(&self.doc.read(cx).session.doc, *studio), config.clone()),
            InstanceSource::External { key, config, .. } => {
                use noshape_model::assembly::eval::ExternalStudios as _;
                (AppExternal.studio(key).map(|(d, t)| studio_inputs(&d, t)).unwrap_or_default(), config.clone())
            }
            InstanceSource::Assembly { assembly, config } => match self.doc.read(cx).session.doc.tab(*assembly).map(|t| &t.content) {
                Some(TabContent::Assembly(a)) => (a.configuration.inputs.clone(), config.clone()),
                _ => (vec![], config.clone()),
            },
            InstanceSource::Standard { .. } => {
                self.edit_standard_instance(top, cx);
                return;
            }
        };
        if inputs.is_empty() {
            self.message = Some((false, format!("{} is not configured.", inst.name)));
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
        // Visibility conditions: hidden inputs and options don't show.
        let (shown, effective) = noshape_model::configuration::Configuration { inputs: inputs.clone(), configured: Vec::new() }.visibility(&s.values);
        for (inp, vis) in inputs.iter().zip(&shown) {
            if !vis.visible {
                continue;
            }
            let id = inp.id.clone();
            match &inp.kind {
                ConfigInputKind::List { options, .. } => {
                    let cur = effective.0.get(&inp.id).cloned().unwrap_or_default();
                    let mut seg = h_flex().gap_1().flex_wrap();
                    for o in options.iter().filter(|o| vis.options.contains(&o.id)) {
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
                ConfigInputKind::Checkbox { .. } => {
                    let cur = effective.0.get(&inp.id).is_some_and(|v| v == "true");
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
        self.open_replace_with(vec![top], cx);
    }

    /// Open the Replace instance dialog with these instances (none: the
    /// field is simply empty and active; live 833).
    pub(super) fn open_replace_with(&mut self, instances: Vec<InstanceId>, cx: &mut Context<Self>) {
        self.selected = instances.clone();
        self.replace_panel = Some(ReplaceSession { instances, all: false, replacement: None, picking: false });
        self.update_capture(cx);
        self.refresh_scene(false, cx);
        cx.notify();
    }

    pub(super) fn close_replace(&mut self, cx: &mut Context<Self>) {
        if self.replace_panel.take().is_some() {
            self.selected.clear();
            self.update_capture(cx);
            self.refresh_scene(false, cx);
            cx.notify();
        }
    }

    /// A click on an instance while the dialog is open adds or removes it.
    pub(super) fn toggle_replace_instance(&mut self, id: InstanceId, cx: &mut Context<Self>) {
        let Some(s) = &mut self.replace_panel else { return };
        match s.instances.iter().position(|x| *x == id) {
            Some(i) => {
                s.instances.remove(i);
            }
            None => s.instances.push(id),
        }
        self.selected = s.instances.clone();
        self.refresh_scene(false, cx);
        cx.notify();
    }

    /// ✓: replace every instance to replace (and, with Replace all
    /// instances, the others of the same source) with the replacement.
    fn accept_replace(&mut self, cx: &mut Context<Self>) {
        let Some(s) = &self.replace_panel else { return };
        let Some((source, base)) = s.replacement.clone() else { return };
        if s.instances.is_empty() {
            return;
        }
        let (ids, all) = (s.instances.clone(), s.all);
        self.replace_panel = None;
        for id in ids {
            self.replace_with_matching(id, all, source.clone(), base.clone(), cx);
        }
        self.selected.clear();
        self.update_capture(cx);
        cx.notify();
    }

    /// The 220 px Replace instance dialog (live 833: title, ✓ ✕, Instances
    /// to replace (active), the replacement field with a part icon, ☐ Replace
    /// all instances, ?; 836: title and the empty replacement field red once
    /// an instance is picked). The replacement field opens the component list
    /// beside the dialog.
    pub(super) fn render_replace_panel(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let Some(s) = &self.replace_panel else { return div().into_any_element() };
        let names: HashMap<InstanceId, String> = self.assembly(cx).map(|a| a.instances.iter().map(|i| (i.id, i.name.clone())).collect()).unwrap_or_default();
        let (all, picking) = (s.all, s.picking);
        let missing = !s.instances.is_empty() && s.replacement.is_none();
        let red = gpui_kit::rgb(0xbd3039);
        let entries: Vec<AnyElement> = s
            .instances
            .iter()
            .map(|id| {
                let id = *id;
                let n = names.get(&id).cloned().unwrap_or_default();
                crate::os_ui::query_row(SharedString::from(format!("rep-i-{}", id.0)), n.clone())
                    .child(
                        div()
                            .id(SharedString::from(format!("rep-x-{}", id.0)))
                            .px(px(5.))
                            .text_size(px(16.67))
                            .text_color(rgb(0x666666))
                            .cursor_pointer()
                            .tooltip(crate::ui::tooltip(format!("Delete {n}")))
                            .child("×")
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                cx.stop_propagation();
                                this.toggle_replace_instance(id, cx);
                            })),
                    )
                    .into_any_element()
            })
            .collect();
        let replacement = s.replacement.as_ref().map(|(_, n)| n.clone());
        let field = h_flex()
            .id("rep-component")
            .mx(px(3.))
            .h(px(28.))
            .px(px(5.))
            .gap(px(4.))
            .rounded(px(2.))
            .border_1()
            .border_color(if missing { red } else { pal.border })
            .when(missing, |d| d.bg(gpui_kit::rgb(0xf9ebec)))
            .cursor_pointer()
            .child(div().opacity(if replacement.is_some() { 1.0 } else { 0.4 }).child(crate::assets::icon("part", 20., cx)))
            .child(match replacement {
                Some(n) => div().flex_1().min_w_0().truncate().text_size(px(13.33)).text_color(rgb(0x555555)).child(n),
                None => div().flex_1().min_w_0().truncate().text_size(px(13.33)).text_color(if missing { red } else { pal.text_faint }).child("Select replacement component"),
            })
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                if let Some(s) = &mut this.replace_panel {
                    s.picking = !s.picking;
                }
                cx.notify();
            }));
        let dialog = v_flex()
            .id("rep-dialog")
            .occlude()
            .w(px(220.))
            .bg(pal.dialog_bg)
            .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
            .text_color(pal.text)
            .child(
                h_flex()
                    .h(px(29.))
                    .pl(px(6.))
                    .bg(rgb(0xffffff))
                    .border_b_1()
                    .border_color(rgb(0xd7d7d7))
                    .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).text_color(if missing { red } else { pal.text }).child("Replace instance"))
                    .child(
                        div()
                            .id("rep-ok")
                            .size(px(28.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(pal.accept)
                            .cursor_pointer()
                            .tooltip(crate::ui::tooltip("Accept (Enter)"))
                            .child(Icon::new(IconName::Check).with_size(px(18.)).text_color(rgb(0xffffff)))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.accept_replace(cx))),
                    )
                    .child(
                        div()
                            .id("rep-close")
                            .size(px(28.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .hover(|d| d.bg(pal.panel_hover))
                            .tooltip(crate::ui::tooltip("Cancel (Esc)"))
                            .child(crate::assets::icon("dialog-cancel", 18., cx))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_replace(cx))),
                    ),
            )
            .child(div().mx(px(3.)).mt(px(5.)).mb(px(4.)).child(crate::os_ui::query_box("rep-instances", "Instances to replace", !picking, entries)))
            .child(field)
            .child(crate::os_ui::checkbox("rep-all", "Replace all instances", all).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                if let Some(s) = &mut this.replace_panel {
                    s.all = !s.all;
                }
                cx.notify();
            })))
            .child(h_flex().h(px(22.)).px(px(4.)).justify_end().child(super::props::help_dot()));
        if !picking {
            return dialog.into_any_element();
        }
        // The replacement components of this document (Part Studios whole,
        // and their parts), beside the dialog.
        let mut list = v_flex().id("rep-list").w(px(260.)).max_h(px(440.)).overflow_y_scroll().occlude().bg(pal.dialog_bg).border_1().border_color(pal.border).p(px(4.)).gap(px(1.));
        for st in self.catalog.studios.clone() {
            let (tab, sname) = (st.tab, st.name.clone());
            list = list.child(
                h_flex()
                    .id(SharedString::from(format!("rep-ps-{tab}")))
                    .h(px(26.))
                    .px(px(4.))
                    .gap(px(6.))
                    .cursor_pointer()
                    .text_size(px(13.33))
                    .hover(|d| d.bg(pal.panel_hover))
                    .child(crate::assets::icon("part-studio", 17., cx))
                    .child(div().flex_1().truncate().child(sname.clone()))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if let Some(s) = &mut this.replace_panel {
                            s.replacement = Some((InstanceSource::PartStudio { studio: tab, config: ConfigValues::default() }, sname.clone()));
                            s.picking = false;
                        }
                        cx.notify();
                    })),
            );
            for (body_id, pname, _, _) in st.parts {
                list = list.child(
                    h_flex()
                        .id(SharedString::from(format!("rep-part-{tab}-{body_id}")))
                        .h(px(26.))
                        .pl(px(24.))
                        .pr(px(4.))
                        .gap(px(6.))
                        .cursor_pointer()
                        .text_size(px(13.33))
                        .hover(|d| d.bg(pal.panel_hover))
                        .child(crate::assets::icon("part", 17., cx))
                        .child(div().flex_1().truncate().child(pname.clone()))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            if let Some(s) = &mut this.replace_panel {
                                s.replacement = Some((InstanceSource::Part { studio: tab, body: body_id.clone(), config: ConfigValues::default() }, pname.clone()));
                                s.picking = false;
                            }
                            cx.notify();
                        })),
                );
            }
        }
        h_flex().items_start().gap(px(1.)).child(dialog).child(list).into_any_element()
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

/// The exploded view's rollback bar being dragged (Onshape's
/// `ns-list-item-rollbackbar`): dropped after a step, the view plays up to it.
#[derive(Clone)]
pub(crate) struct ExplodeRollbackDrag {
    pub view: usize,
}

impl Render for ExplodeRollbackDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().w(px(200.)).h(px(6.)).bg(rgb(0xcccccc))
    }
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
                    self.ui.fixed_drag = Some(position);
                    cx.notify();
                }
                return;
            }
            // Live 569: no message while snap-dragging.
            s.started = true;
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

/// Insert › Other documents categories (live assembly/71).
const OTHER_SOURCES: [(&str, &str); 5] = [("Owned by me", "src-owned"), ("Recently opened", "src-recent"), ("Created by me", "src-created"), ("Shared with me", "src-shared"), ("Public", "src-public")];

/// A document opened in Insert › Other documents (live assembly/612–619):
/// its versions (named ones newest first, then Start), the one shown, and
/// that version's Part Studios and assemblies.
#[derive(Clone)]
pub(crate) struct OtherDoc {
    pub path: std::path::PathBuf,
    pub name: String,
    pub versions: Vec<crate::derived::VersionChoice>,
    /// Microversions in the workspace since its last version.
    pub changes: usize,
    pub version: usize,
    /// The Version graph view replaces the list.
    pub graph: bool,
    /// (tab id, name, is an assembly) of the shown version.
    pub tabs: Vec<(String, String, bool)>,
    /// Parts of the expanded Part Studios (tab id → (body, name)).
    pub parts: HashMap<String, Vec<(BodyId, String)>>,
    pub expanded: HashSet<String>,
}

impl OtherDoc {
    /// Read another document's versions, Onshape style: only versions can be
    /// inserted; the root microversion is the "Start" version.
    fn open(path: &std::path::Path) -> anyhow::Result<Self> {
        let store = noshape_store::Document::open_read_only(path)?;
        let graph = store.graph()?;
        let mut versions: Vec<crate::derived::VersionChoice> = graph.versions.iter().rev().map(|v| crate::derived::VersionChoice { label: v.name.clone(), microversion: v.microversion }).collect();
        if let Some(root) = graph.microversions.iter().find(|m| m.parents.is_empty()) {
            versions.push(crate::derived::VersionChoice { label: "Start".into(), microversion: root.id });
        }
        // Changes since the last version: steps back from the head to a
        // versioned microversion (or the start).
        let marks: HashSet<noshape_store::MicroversionId> = versions.iter().map(|v| v.microversion).collect();
        let parent: HashMap<noshape_store::MicroversionId, Option<noshape_store::MicroversionId>> = graph.microversions.iter().map(|m| (m.id, m.parents.first().copied())).collect();
        let mut at = Some(store.head(store.default_branch()?)?);
        let mut changes = 0;
        while let Some(mv) = at.filter(|mv| !marks.contains(mv)) {
            changes += 1;
            at = parent.get(&mv).copied().flatten();
        }
        let name = store.meta().map(|m| m.name).unwrap_or_else(|_| path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default());
        let mut d = OtherDoc { path: path.to_path_buf(), name, versions, changes, version: 0, graph: false, tabs: Vec::new(), parts: HashMap::new(), expanded: HashSet::new() };
        d.load_tabs();
        Ok(d)
    }

    fn load_tabs(&mut self) {
        self.parts.clear();
        self.expanded.clear();
        self.tabs = self
            .versions
            .get(self.version)
            .and_then(|v| crate::derived::external_snapshot(&self.path, v.microversion).ok())
            .map(|d| {
                d.tabs
                    .iter()
                    .filter_map(|t| match &t.content {
                        noshape_model::TabContent::PartStudio(_) => Some((t.id.to_string(), t.name.clone(), false)),
                        noshape_model::TabContent::Assembly(_) => Some((t.id.to_string(), t.name.clone(), true)),
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default();
    }

    fn key(&self, tab: &str) -> Option<String> {
        let v = self.versions.get(self.version)?;
        Some(crate::derived::ExternalRef { path: self.path.clone(), microversion: v.microversion, tab: tab.to_string() }.key())
    }
}

impl AssemblyView {
    // ------------------------------------------------------------ insert from another document

    /// Scan the local document library (background) for the Other
    /// documents lists; once per Insert session.
    fn ensure_other_docs(&mut self, cx: &mut Context<Self>) {
        if self.ui.insert_other_docs.is_some() {
            return;
        }
        self.ui.insert_other_docs = Some(Vec::new());
        cx.spawn(async move |this, cx| {
            let (docs, versions) = cx
                .background_spawn(async {
                    let docs = crate::start_library::Library::default().scan(&crate::start_library::ScanInput::current()).docs;
                    // Each document's latest version name (grey under the name).
                    let versions: HashMap<std::path::PathBuf, String> = docs.iter().filter_map(|d| OtherDoc::open(&d.path).ok().and_then(|o| o.versions.first().map(|v| (d.path.clone(), v.label.clone())))).collect();
                    (docs, versions)
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.ui.insert_other_docs = Some(docs);
                this.ui.insert_other_versions = versions;
                cx.notify();
            });
        })
        .detach();
    }

    /// "Search or paste URL" › Enter / 🔍: a pasted document path opens that
    /// document, other text lists the documents whose name contains it.
    pub(super) fn search_other_documents(&mut self, text: String, cx: &mut Context<Self>) {
        let path = std::path::PathBuf::from(text.trim_start_matches("file://"));
        if path.extension().is_some_and(|e| e == crate::start_library::EXT) && path.exists() {
            self.pick_other_document(path, cx);
            return;
        }
        self.ensure_other_docs(cx);
        self.ui.insert_other_doc = None;
        self.ui.insert_other_query = (!text.is_empty()).then_some(text);
        cx.notify();
    }

    fn pick_other_document(&mut self, path: std::path::PathBuf, cx: &mut Context<Self>) {
        match OtherDoc::open(&path) {
            Ok(d) => self.ui.insert_other_doc = Some(d),
            Err(e) => self.message = Some((true, format!("Could not open {}: {e:#}", path.display()))),
        }
        cx.notify();
    }

    /// Back to the document filters (home) or one level up (‹).
    fn other_docs_back(&mut self, home: bool, window: &mut Window, cx: &mut Context<Self>) {
        // ‹ in a folder of Owned by me goes up one folder.
        if !home
            && self.ui.insert_other_doc.is_none()
            && let Some(dir) = self.ui.insert_other_dir.take()
        {
            let root = docs_root();
            self.ui.insert_other_dir = dir.parent().map(canon_dir).filter(|p| *p != root && p.starts_with(&root));
            cx.notify();
            return;
        }
        if home || self.ui.insert_other_doc.take().is_none() {
            self.ui.insert_other_dir = None;
            self.ui.insert_other_cat = None;
            self.ui.insert_other_query = None;
            self.ui.insert_other_doc = None;
            if let Some(st) = &self.ui.insert_url {
                st.update(cx, |s, cx| s.set_value("", window, cx));
            }
        }
        self.sync_other_placeholder(cx);
        cx.notify();
    }

    /// The search box reads 'Search in "<category>"' inside a category.
    fn sync_other_placeholder(&mut self, cx: &mut Context<Self>) {
        let ph = match self.ui.insert_other_cat {
            Some(c) if self.ui.insert_other_query.is_none() => format!("Search in \"{}\"", OTHER_SOURCES[c].0),
            _ => "Search or paste URL".to_string(),
        };
        self.ui.insert_url_ph = ph.into();
        cx.notify();
    }

    /// The Insert dialog's "Search Part Studios" / "Search Assemblies" box;
    /// the placeholder is italic (live 320, 612).
    pub(super) fn insert_search_box(&self, cx: &App) -> Option<AnyElement> {
        let st = self.ui.insert_search.as_ref()?;
        let empty = st.read(cx).value().is_empty();
        let ph = if self.ui.insert_tab == 0 { "Search Part Studios" } else { "Search Assemblies" };
        Some(
            div()
                .pt(px(6.))
                .px(px(6.))
                .pb(px(6.))
                .child(
                    div()
                        .relative()
                        .child(Input::new(st).h(px(30.)).cleanable(true))
                        .when(empty, |d| d.child(div().absolute().left(px(9.)).top_0().bottom_0().flex().items_center().text_size(px(13.33)).italic().text_color(rgb(0x999999)).child(ph))),
                )
                .into_any_element(),
        )
    }

    /// Expand a Part Studio of the opened document: its parts (background).
    fn expand_other_studio(&mut self, tab: String, cx: &mut Context<Self>) {
        let Some(d) = &mut self.ui.insert_other_doc else { return };
        if !d.expanded.remove(&tab) {
            d.expanded.insert(tab.clone());
        }
        if d.parts.contains_key(&tab) {
            cx.notify();
            return;
        }
        let Some(key) = d.key(&tab) else { return };
        d.parts.insert(tab.clone(), Vec::new());
        let evaluator = self.evaluator.clone();
        cx.spawn(async move |this, cx| {
            let parts: Vec<(BodyId, String)> = cx
                .background_spawn(async move { evaluator.lock().external_studio(&key, &ConfigValues::default()).map(|(_, r)| r.display.bodies.iter().filter(|b| matches!(b.kind, BodyKind::Solid | BodyKind::Surface) && !b.hidden).map(|b| (b.id.clone(), b.name.clone())).collect()).unwrap_or_default() })
                .await;
            let _ = this.update(cx, |this: &mut Self, cx| {
                if let Some(d) = &mut this.ui.insert_other_doc {
                    d.parts.insert(tab, parts);
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Insert › Other documents (live assembly/610–619): the search row, then
    /// the categories, a category's documents, search results, or the opened
    /// document (version header, Part Studios | Assemblies, its tree or the
    /// Version graph).
    pub(super) fn render_insert_other(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let blue = rgb(0x1651b0);
        if (self.ui.insert_other_cat.is_some() || self.ui.insert_other_query.is_some()) && self.ui.insert_other_docs.is_none() {
            self.ensure_other_docs(cx);
        }
        let mut col = v_flex().flex_none();
        let searching = self.ui.insert_other_query.is_some();
        let has_text = self.ui.insert_url.as_ref().is_some_and(|s| !s.read(cx).value().trim().is_empty());
        let header_btn = |id: &'static str, icon: &'static str, tip: SharedString| {
            // Live 611: dark `#333` home and ‹ (back) glyphs.
            div().id(id).size(px(30.)).flex().items_center().justify_center().rounded(px(3.)).cursor_pointer().hover(|d| d.bg(rgb(0xeaeaea))).tooltip(crate::ui::tooltip(tip)).child(crate::ui::glyph(icon, if icon == "ui-chevron-left" { 16. } else { 20. }, rgb(0x333333)))
        };
        // The opened document: home · ‹ · name / ⑂ version, Create version…,
        // Version graph; Part Studios | Assemblies; search; tree.
        if let Some(d) = self.ui.insert_other_doc.clone() {
            let back_tip = self.ui.insert_other_query.as_ref().map(|q| format!("Results for \"{q}\"")).or_else(|| self.ui.insert_other_cat.map(|c| OTHER_SOURCES[c].0.to_string())).unwrap_or_else(|| "Go to document filters".into());
            let version = d.versions.get(d.version).map(|v| v.label.clone()).unwrap_or_default();
            let square = |id: &'static str, icon: &'static str, tip: &'static str, on: bool, cx: &App| {
                div()
                    .id(id)
                    .size(px(30.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(3.))
                    .border_1()
                    .border_color(rgb(0xaaaaaa))
                    .cursor_pointer()
                    .when(on, |d| d.bg(rgb(0xeaeaea)))
                    .hover(|d| d.bg(pal.panel_hover))
                    .child(crate::assets::icon(&icon_path(icon), 20., cx))
                    .tooltip(crate::ui::tooltip(tip))
            };
            col = col.child(
                h_flex()
                    .h(px(40.))
                    .pl(px(5.))
                    .pr(px(5.))
                    .gap(px(3.))
                    .border_b_1()
                    .border_color(rgb(0xeeeeee))
                    .child(header_btn("ins-other-home", "home", "Go to document filters".into()).on_click(cx.listener(|this, _: &ClickEvent, w, cx| this.other_docs_back(true, w, cx))))
                    .child(header_btn("ins-other-back", "ui-chevron-left", back_tip.into()).on_click(cx.listener(|this, _: &ClickEvent, w, cx| this.other_docs_back(false, w, cx))))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(div().truncate().text_size(px(14.67)).font_weight(FontWeight::MEDIUM).line_height(px(19.)).text_color(pal.text).child(d.name.clone()))
                            .child(h_flex().gap_1().opacity(0.5).text_size(px(12.)).line_height(px(16.)).child(crate::ui::glyph("branch", 10., pal.text)).child(version)),
                    )
                    .child(square("ins-other-create-version", "version", "Create version…", false, cx).on_click({
                        let path = d.path.clone();
                        let gpu = self.gpu.clone();
                        move |_, _, cx| {
                            let _ = crate::files::open_document_window(path.clone(), None, gpu.clone(), cx);
                        }
                    }))
                    .child(square("ins-other-graph", "version-graph", "Version graph", d.graph, cx).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        if let Some(d) = &mut this.ui.insert_other_doc {
                            d.graph = !d.graph;
                        }
                        cx.notify();
                    }))),
            );
            if d.graph {
                // Version graph: ○ Workspace ◆ Version, then Main (not
                // selectable) and the versions (live 613, 618).
                col = col.child(
                    h_flex()
                        .h(px(36.))
                        .pl(px(6.))
                        .gap(px(4.))
                        .text_size(px(11.))
                        .text_color(pal.text)
                        .border_b_1()
                        .border_color(rgb(0xeeeeee))
                        .child(div().size(px(9.)).rounded_full().border_1().border_color(pal.text))
                        .child("Workspace")
                        .child(div().ml(px(4.)).size(px(7.)).rounded_full().bg(pal.text))
                        .child("Version"),
                );
                let node = |hollow: bool, last: bool| {
                    div()
                        .w(px(140.))
                        .h_full()
                        .relative()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(div().absolute().left(px(69.5)).w(px(1.5)).top(px(if hollow { 18. } else { 0. })).bottom(px(if last { 18. } else { 0. })).bg(blue))
                        .child(div().size(px(if hollow { 13. } else { 9. })).rounded_full().border_1().border_color(blue).bg(if hollow { rgb(0xffffff) } else { blue }))
                };
                col = col.child(h_flex().h(px(36.)).child(node(true, d.versions.is_empty())).child(div().flex_1().text_size(px(13.33)).italic().text_color(rgb(0x999999)).child("Main")));
                let n = d.versions.len();
                for (i, v) in d.versions.iter().enumerate() {
                    let on = i == d.version;
                    col = col.child(
                        h_flex()
                            .id(SharedString::from(format!("ins-other-v-{i}")))
                            .h(px(36.))
                            .cursor_pointer()
                            .when(on, |r| r.bg(rgb(0xb2ddf6)))
                            .when(!on, |r| r.hover(|r| r.bg(pal.panel_hover)))
                            .child(node(false, i + 1 == n))
                            .child(div().flex_1().text_size(px(13.33)).text_color(pal.text).child(v.label.clone()))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                if let Some(d) = &mut this.ui.insert_other_doc {
                                    d.version = i;
                                    d.graph = false;
                                    d.load_tabs();
                                }
                                cx.notify();
                            })),
                    );
                }
                return col.into_any_element();
            }
            let tab = self.ui.insert_tab;
            let dtab = |id: &'static str, label: &'static str, ix: u8, cx: &mut Context<Self>| {
                let on = tab == ix;
                div()
                    .id(id)
                    .w(px(150.))
                    .h(px(32.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(12.))
                    .cursor_pointer()
                    .text_color(if on { pal.accent } else { pal.text })
                    .when(on, |d| d.font_weight(FontWeight::MEDIUM).border_b_2().border_color(pal.accent))
                    .child(label)
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.ui.insert_tab = ix;
                        cx.notify();
                    }))
            };
            col = col.child(h_flex().h(px(32.)).border_b_1().border_color(rgb(0xeeeeee)).child(dtab("ins-other-ps", "Part Studios", 0, cx)).child(dtab("ins-other-asm", "Assemblies", 1, cx)));
            if let Some(b) = self.insert_search_box(cx) {
                col = col.child(b);
            }
            // Unversioned work in the document (live 612).
            if d.changes > 0 {
                let path = d.path.clone();
                let gpu = self.gpu.clone();
                col = col.child(
                    v_flex()
                        .mx(px(4.))
                        .mb(px(4.))
                        .p(px(4.))
                        .rounded(px(5.25))
                        .border_1()
                        .border_color(rgb(0xb2ddf6))
                        .bg(rgb(0xdef1fb))
                        .text_size(px(14.))
                        .text_color(rgb(0x094174))
                        .child(div().px(px(1.)).child(format!("{} change{} since the last version.", d.changes, if d.changes == 1 { "" } else { "s" })))
                        .child(
                            div()
                                .id("ins-other-create-link")
                                .pt(px(4.))
                                .pb(px(2.))
                                .flex()
                                .justify_center()
                                .font_weight(FontWeight::SEMIBOLD)
                                .underline()
                                .cursor_pointer()
                                .child(format!("Create a version in {}.", d.name))
                                .on_click(move |_, _, cx| {
                                    let _ = crate::files::open_document_window(path.clone(), None, gpu.clone(), cx);
                                }),
                        ),
                );
            }
            let query = self.ui.insert_search.as_ref().map(|s| s.read(cx).value().trim().to_lowercase()).unwrap_or_default();
            let tabs: Vec<(String, String, bool)> = d.tabs.iter().filter(|t| t.2 == (tab == 1) && (query.is_empty() || t.1.to_lowercase().contains(&query))).cloned().collect();
            if tabs.is_empty() {
                col = col.child(div().pl(px(10.)).h(px(28.)).flex().items_center().text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child(if tab == 0 { "No Part Studios available." } else { "No Assemblies available." }));
            }
            // Pictures of the shown Part Studios and their parts (as the
            // Current document tab's), rendered in the background.
            for (t, _, is_asm) in &tabs {
                if let (false, Some(k)) = (*is_asm, d.key(t)) {
                    self.request_external_thumbs(k, cx);
                }
            }
            let item_row = |id: SharedString, indent: f32, expander: Option<AnyElement>, icon: &'static str, thumb: Option<Arc<RenderImage>>, name: String, cx: &App| {
                h_flex()
                    .id(id)
                    .flex_none()
                    .h(px(40.))
                    .pl(px(4. + indent))
                    .cursor_pointer()
                    .hover(|d| d.bg(pal.row_hover))
                    .child(div().w(px(14.)).h_full().flex_none().flex().items_center().children(expander))
                    .child(div().w(px(70.)).h(px(40.)).flex_none().flex().items_center().justify_center().child(match thumb {
                        Some(img) => gpui_kit::img(img).w(px(70.)).h(px(40.)).object_fit(ObjectFit::Contain).into_any_element(),
                        None => crate::assets::icon(&icon_path(icon), 28., cx).into_any_element(),
                    }))
                    .child(div().flex_1().min_w_0().pl(px(4.)).truncate().text_size(px(13.33)).font_weight(FontWeight::LIGHT).child(name))
            };
            for (t, name, is_asm) in tabs {
                let key = d.key(&t);
                if is_asm {
                    let (k2, n2) = (key.clone(), name.clone());
                    col = col.child(item_row(SharedString::from(format!("ins-other-a-{t}")), 0., None, "sub-assembly", None, name, cx).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if let Some(k) = k2.clone() {
                            this.insert_external_assembly(k, n2.clone(), cx);
                        }
                    })));
                    continue;
                }
                let open = d.expanded.contains(&t);
                let t2 = t.clone();
                let expander = div()
                    .id(SharedString::from(format!("ins-other-x-{t}")))
                    .child(crate::ui::glyph(if open { "chevron-down" } else { "chevron-right" }, 12., pal.text_muted))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        cx.stop_propagation();
                        this.expand_other_studio(t2.clone(), cx);
                    }))
                    .into_any_element();
                let (k2, n2) = (key.clone(), name.clone());
                let studio_thumb = key.as_deref().and_then(|k| self.external_thumb(k, None));
                col = col.child(item_row(SharedString::from(format!("ins-other-s-{t}")), 0., Some(expander), "part-studio", studio_thumb, name, cx).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if let Some(k) = k2.clone() {
                        this.insert_external(k, None, n2.clone(), cx);
                    }
                })));
                if open {
                    for (i, (b, pname)) in d.parts.get(&t).cloned().unwrap_or_default().into_iter().enumerate() {
                        let k3 = key.clone();
                        let n3 = pname.clone();
                        let part_thumb = key.as_deref().and_then(|k| self.external_thumb(k, Some(&b)));
                        col = col.child(item_row(SharedString::from(format!("ins-other-p-{t}-{i}")), 14., None, "part", part_thumb, pname, cx).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            if let Some(k) = k3.clone() {
                                this.insert_external(k, Some(b.clone()), n3.clone(), cx);
                            }
                        })));
                    }
                }
            }
            return col.into_any_element();
        }
        // The search row: funnel (blue while showing results), the field
        // and the 🔍 button.
        col = col.child(
            h_flex()
                .h(px(39.))
                .pt(px(6.))
                .px(px(6.))
                .child(div().w(px(32.)).h(px(30.)).flex_none().flex().items_center().justify_center().child(crate::ui::glyph("filter", 20., if searching { blue } else { rgb(0x666666) })))
                .child(
                    div()
                        .ml(px(6.))
                        .flex_1()
                        .h(px(30.))
                        .rounded_l(px(3.))
                        .border_1()
                        .border_color(rgb(0xcccccc))
                        .relative()
                        .children(self.ui.insert_url.as_ref().map(|st| Input::new(st).appearance(false).cleanable(true).text_size(px(13.33)).h(px(28.))))
                        .when(!has_text, |d| d.child(div().absolute().left(px(7.)).top_0().bottom_0().flex().items_center().text_size(px(13.33)).italic().text_color(rgb(0x999999)).child(self.ui.insert_url_ph.clone()))),
                )
                .child(
                    div()
                        .id("ins-other-search")
                        .w(px(34.))
                        .h(px(30.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_r(px(3.))
                        .border_1()
                        .border_l_0()
                        .border_color(if has_text { rgb(0x90cef1) } else { rgb(0xeaeaea) })
                        .cursor_pointer()
                        .child(crate::ui::glyph("search", 20., if has_text { blue } else { rgb(0xcccccc) }))
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            let text = this.ui.insert_url.as_ref().map(|s| s.read(cx).value().trim().to_string()).unwrap_or_default();
                            this.search_other_documents(text, cx);
                            this.sync_other_placeholder(cx);
                        })),
                ),
        );
        let listing = self.ui.insert_other_query.clone().map(|q| (None, Some(q))).or_else(|| self.ui.insert_other_cat.map(|c| (Some(c), None)));
        let Some((cat, query)) = listing else {
            // Top level: the categories (32 px rows, 20 px icons).
            for (i, (label, icon)) in OTHER_SOURCES.iter().enumerate() {
                col = col.child(
                    h_flex()
                        .id(SharedString::from(format!("ins-other-cat-{i}")))
                        .h(px(32.))
                        .pl(px(16.))
                        .gap(px(6.))
                        .cursor_pointer()
                        .text_size(px(14.67))
                        .text_color(pal.text)
                        .hover(|d| d.bg(pal.panel_hover))
                        .child(crate::assets::icon(&icon_path(icon), 20., cx))
                        .child(div().truncate().child(label.to_string()))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.ensure_other_docs(cx);
                            this.ui.insert_other_cat = Some(i);
                            this.sync_other_placeholder(cx);
                            cx.notify();
                        })),
                );
            }
            return col.into_any_element();
        };
        // A category: home · ‹ · its name (live 611).
        if let Some(c) = cat {
            col = col.child(
                h_flex()
                    .h(px(36.))
                    .pl(px(2.))
                    .gap(px(3.))
                    .border_b_1()
                    .border_color(rgb(0xeeeeee))
                    .child(header_btn("ins-other-home", "home", "Go to document filters".into()).on_click(cx.listener(|this, _: &ClickEvent, w, cx| this.other_docs_back(true, w, cx))))
                    .child(header_btn("ins-other-back", "ui-chevron-left", "Filters".into()).on_click(cx.listener(|this, _: &ClickEvent, w, cx| this.other_docs_back(false, w, cx))))
                    .child(div().pl(px(3.)).text_size(px(14.67)).font_weight(FontWeight::MEDIUM).text_color(pal.text).child(match (&self.ui.insert_other_dir, c) {
                        (Some(dir), 0) => dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
                        _ => OTHER_SOURCES[c].0.to_string(),
                    })),
            );
        }
        let me = self.doc.read(cx).session.path.clone();
        let loading = self.ui.insert_other_docs.as_ref().is_none_or(|d| d.is_empty()) && self.ui.insert_other_versions.is_empty();
        let mut docs: Vec<crate::start_library::DocEntry> = self.ui.insert_other_docs.clone().unwrap_or_default().into_iter().filter(|d| !d.trashed && !d.broken && d.path != me).collect();
        let q = query.as_ref().map(|q| q.to_lowercase());
        match (cat, &q) {
            // Local documents are all the user's own; nothing is shared or public.
            (Some(3 | 4), _) => docs.clear(),
            (Some(1), _) => {
                docs.retain(|d| d.recent_rank.is_some());
                docs.sort_by_key(|d| d.recent_rank);
            }
            (_, Some(q)) => docs.retain(|d| d.name.to_lowercase().contains(q.as_str())),
            _ => docs.sort_by_key(|d| std::cmp::Reverse(d.modified)),
        }
        // Owned by me (live 611): the folders first (folder outline + bold
        // name; a click opens one), then the documents at that level.
        if cat == Some(0) && q.is_none() {
            let root = docs_root();
            let outside: Vec<std::path::PathBuf> = crate::files::watched_folders().into_iter().map(|f| canon_dir(&f)).filter(|f| !f.starts_with(&root) && f.is_dir()).collect();
            let here = self.ui.insert_other_dir.clone();
            let dirs: Vec<std::path::PathBuf> = match &here {
                Some(dir) => crate::start_window::subfolders(dir),
                None => crate::start_window::subfolders(&root).into_iter().chain(outside.iter().cloned()).collect(),
            };
            docs.retain(|d| {
                let f = canon_dir(d.folder());
                match &here {
                    Some(dir) => f == *dir,
                    // The top level: documents in the documents folder itself,
                    // and those outside every listed folder.
                    None => f == root || (!f.starts_with(&root) && !outside.iter().any(|o| f.starts_with(o))),
                }
            });
            for (i, dir) in dirs.into_iter().enumerate() {
                let name = dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                col = col.child(
                    h_flex()
                        .id(SharedString::from(format!("ins-other-dir-{i}")))
                        .h(px(51.))
                        .pl(px(2.))
                        .cursor_pointer()
                        .hover(|d| d.bg(pal.panel_hover))
                        .child(div().w(px(74.)).h(px(40.)).flex_none().flex().items_center().justify_center().child(crate::ui::glyph("folder", 28., rgb(0x999999))))
                        .child(div().flex_1().min_w_0().truncate().text_size(px(14.67)).font_weight(FontWeight::MEDIUM).text_color(pal.text).child(name))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.ui.insert_other_dir = Some(dir.clone());
                            cx.notify();
                        })),
                );
            }
        }
        if docs.is_empty() && !loading {
            col = col.child(div().pl(px(10.)).h(px(28.)).flex().items_center().text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child("No documents found"));
        }
        for (i, d) in docs.into_iter().enumerate() {
            let path = d.path.clone();
            let thumb = match d.thumbnail.clone() {
                Some(img) => gpui_kit::img(img).w(px(70.)).h(px(40.)).object_fit(ObjectFit::Contain).into_any_element(),
                None => crate::assets::icon(&icon_path("document"), 32., cx).into_any_element(),
            };
            // Search results: the matched text in blue (live 615).
            let name_el = match q.as_ref().and_then(|q| d.name.to_lowercase().find(q.as_str()).map(|at| (at, q.len()))) {
                Some((at, len)) if d.name.is_char_boundary(at) && d.name.is_char_boundary(at + len) => h_flex()
                    .child(d.name[..at].to_string())
                    .child(div().text_color(blue).child(d.name[at..at + len].to_string()))
                    .child(d.name[at + len..].to_string())
                    .into_any_element(),
                _ => div().child(d.name.clone()).into_any_element(),
            };
            let version = self.ui.insert_other_versions.get(&d.path).cloned();
            col = col.child(
                h_flex()
                    .id(SharedString::from(format!("ins-other-doc-{i}")))
                    .h(px(51.))
                    .pl(px(2.))
                    .cursor_pointer()
                    .hover(|d| d.bg(pal.panel_hover))
                    .tooltip(crate::ui::tooltip(match &version {
                        Some(v) => format!("{} -- {v}", d.name),
                        None => d.name.clone(),
                    }))
                    .child(div().w(px(74.)).h(px(40.)).flex_none().flex().items_center().justify_center().child(thumb))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(div().truncate().text_size(px(14.67)).font_weight(FontWeight::MEDIUM).line_height(px(19.)).text_color(pal.text).child(name_el))
                            .children(version.map(|v| div().opacity(0.5).text_size(px(12.)).line_height(px(16.)).text_color(pal.text).child(v))),
                    )
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.pick_other_document(path.clone(), cx))),
            );
        }
        col.into_any_element()
    }

    /// Insert the parts (or the whole Part Studio, rigid) of an external Part Studio.
    fn insert_external(&mut self, key: String, only: Option<BodyId>, studio_name: String, cx: &mut Context<Self>) {
        let evaluator = self.evaluator.clone();
        let rigid = self.insert_rigid && only.is_none();
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
                let items: Vec<(Option<BodyId>, String)> = if rigid { vec![(None, studio_name.clone())] } else { parts.into_iter().filter(|(b, _)| only.as_ref().is_none_or(|o| o == b)).map(|(b, n)| (Some(b), n)).collect() };
                let ids = this.edit(&format!("Insert : {studio_name} (other document)"), cx, |a| {
                    let mut ids = Vec::new();
                    for (k, (body, name)) in items.into_iter().enumerate() {
                        let id = InstanceId(a.next_id());
                        let n = a.next_instance_name(&name);
                        let placement = Placement { translation: [k as f64 * 0.03, 0.0, 0.0], rotation: [0.0, 0.0, 0.0, 1.0] };
                        a.instances.push(Instance { id, name: n, source: InstanceSource::External { key: key.clone(), body, config: ConfigValues::default() }, placement, fixed: false, hidden: false, suppressed: false, lock: SubassemblyLock::Flexible, nested: Vec::new(), best_tessellation: false, reference_designator: None, unsuppressed_if: None });
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

    /// Insert › Other documents › Assemblies: the other document's Assembly
    /// as one subassembly instance, pinned to the chosen version (it lists
    /// in the Reference manager like the other document's parts).
    fn insert_external_assembly(&mut self, key: String, name: String, cx: &mut Context<Self>) {
        let id = self.edit(&format!("Insert : {name} (other document)"), cx, |a| {
            let id = InstanceId(a.next_id());
            let n = a.next_instance_name(&name);
            a.instances.push(Instance::new(id, n, InstanceSource::External { key: key.clone(), body: None, config: ConfigValues::default() }, Placement::default()));
            id
        });
        if let Some(id) = id {
            self.start_placing(vec![id], cx);
        }
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
}

/// The documents folder (canonical): the top of Owned by me.
fn docs_root() -> std::path::PathBuf {
    canon_dir(&crate::files::documents_dir())
}

fn canon_dir(p: &std::path::Path) -> std::path::PathBuf {
    p.canonicalize().unwrap_or_else(|_| p.to_path_buf())
}
