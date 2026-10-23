//! Assembly Properties… (part / assembly metadata), Instance properties…
//! (Reference designator), the instance list's state pills and Dynamic
//! suppression (live assembly/100–104).

use super::*;
use crate::properties_dialog::{MetaSpec, MetaValues};
use gpui_kit::component::input::InputEvent;
use std::rc::Rc;

/// What a Dynamic suppression expression belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DynTarget {
    Instance(InstanceId),
    Mate(MateId),
    /// An instance folder (its instances).
    Folder(MateId),
}
use noshape_model::assembly::BomBehavior;

/// The dialogs' gray "?" help button (14 px, `#888`).
pub(super) fn help_dot() -> Div {
    div().size(px(14.)).rounded_full().bg(rgb(0x888888)).flex().items_center().justify_center().text_size(px(11.)).font_weight(FontWeight::BOLD).text_color(gpui_kit::rgb_static(0xffffff)).child("?")
}

use crate::properties_dialog::parse_mass;

impl AssemblyView {
    /// Instance row › Properties…: the metadata of the part (or
    /// subassembly) the instance shows.
    pub(super) fn open_instance_metadata(&mut self, top: InstanceId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(inst) = self.assembly(cx).and_then(|a| a.instance(top).cloned()) else { return };
        match inst.source.clone() {
            InstanceSource::Assembly { assembly, .. } => self.open_assembly_properties(assembly, window, cx),
            InstanceSource::Part { studio, body, .. } => {
                let (props, name) = {
                    let doc = &self.doc.read(cx).session.doc;
                    let props = match doc.tab(studio).map(|t| &t.content) {
                        Some(TabContent::PartStudio(ps)) => ps.part_properties.get(&body).cloned().unwrap_or_default(),
                        _ => Default::default(),
                    };
                    let name = self.result.occurrences.iter().find(|o| o.top() == top && o.body == body).map(|o| o.body_name.clone()).unwrap_or_else(|| inst.name.rsplit_once(" <").map(|(b, _)| b.to_string()).unwrap_or(inst.name.clone()));
                    (props, name)
                };
                let computed = self.result.occurrences.iter().find(|o| o.top() == top).filter(|_| props.material.is_some()).and_then(|o| {
                    let density = |_: &BodyId| props.density;
                    noshape_model::regen::measure::mass_properties(&o.context, std::slice::from_ref(&o.body), &density)?.mass
                });
                let spec = MetaSpec {
                    category: "Onshape Part",
                    material: Some(props.material.clone().unwrap_or_else(|| "Unassigned".into())),
                    computed_mass: computed.map(|m| format!("{} kg", noshape_units::to_fixed(m, 4))),
                    bom_options: None,
                    values: MetaValues {
                        name,
                        description: props.description.clone().unwrap_or_default(),
                        part_number: props.part_number.clone().unwrap_or_default(),
                        revision: props.revision.clone().unwrap_or_default(),
                        meta: props.meta.clone(),
                        mass_override: props.mass_override.map(|m| format!("{m} kg")),
                        bom: 0,
                    },
                };
                let (doc, view) = (self.doc.clone(), cx.entity());
                crate::properties_dialog::open_properties(
                    spec,
                    move |v, _, cx| {
                        let body = body.clone();
                        doc.update(cx, |d, cx| {
                            d.edit_studio(studio, &format!("Properties : {}", v.name), cx, |ps| {
                                let p = ps.part_properties.entry(body).or_default();
                                p.name = Some(v.name.clone());
                                p.description = Some(v.description.clone()).filter(|s| !s.is_empty());
                                p.part_number = Some(v.part_number.clone()).filter(|s| !s.is_empty());
                                p.revision = Some(v.revision.clone()).filter(|s| !s.is_empty());
                                p.meta = v.meta.clone();
                                p.mass_override = v.mass_override.as_deref().and_then(parse_mass);
                            });
                        });
                        view.update(cx, |this, cx| this.refresh_scene(true, cx));
                    },
                    window,
                    cx,
                );
            }
            _ => {
                if let Some(studio) = inst.source.studio() {
                    crate::doc_dialogs::tab_properties(self.doc.clone(), studio, window, cx);
                }
            }
        }
    }

    /// Top row (or subassembly) › Properties…: the assembly's metadata,
    /// with "Subassembly BOM behavior" and no Material / Mass.
    pub(super) fn open_assembly_properties(&mut self, tab: noshape_model::document::TabId, window: &mut Window, cx: &mut Context<Self>) {
        let (name, props) = {
            let doc = &self.doc.read(cx).session.doc;
            let Some(t) = doc.tab(tab) else { return };
            let TabContent::Assembly(a) = &t.content else { return };
            (t.name.clone(), a.properties.clone())
        };
        // This assembly's own row: its computed mass and Mass override
        // (live assembly/726: "Mass ƒ ☑ Override 2 lb").
        let computed = (tab == self.tab).then(|| self.assembly_total_mass(cx)).flatten();
        let spec = MetaSpec {
            category: "Onshape Assembly",
            material: None,
            computed_mass: (tab == self.tab).then(|| computed.map(|m| format!("{} kg", noshape_units::to_fixed(m, 4))).unwrap_or_default()),
            bom_options: Some(BomBehavior::ALL.iter().map(|b| b.label().to_string()).collect()),
            values: MetaValues {
                name,
                description: props.description.clone().unwrap_or_default(),
                part_number: props.part_number.clone().unwrap_or_default(),
                revision: props.revision.clone().unwrap_or_default(),
                meta: props.meta.clone(),
                mass_override: props.mass_override.map(|m| format!("{m} kg")),
                bom: BomBehavior::ALL.iter().position(|b| *b == props.bom_behavior).unwrap_or(0),
            },
        };
        let doc = self.doc.clone();
        crate::properties_dialog::open_properties(
            spec,
            move |v, _, cx| {
                doc.update(cx, |d, cx| {
                    d.edit_document(&format!("Properties : {}", v.name), cx, |document| {
                        if let Some(t) = document.tab_mut(tab) {
                            t.name = v.name.clone();
                            if let TabContent::Assembly(a) = &mut t.content {
                                a.properties.description = Some(v.description.clone()).filter(|s| !s.is_empty());
                                a.properties.part_number = Some(v.part_number.clone()).filter(|s| !s.is_empty());
                                a.properties.revision = Some(v.revision.clone()).filter(|s| !s.is_empty());
                                a.properties.meta = v.meta.clone();
                                a.properties.bom_behavior = BomBehavior::ALL.get(v.bom).copied().unwrap_or_default();
                                a.properties.mass_override = v.mass_override.as_deref().and_then(parse_mass);
                            }
                        }
                    })
                });
            },
            window,
            cx,
        );
    }

    /// Instance row › Instance properties…: the Reference designator.
    pub(super) fn open_instance_properties(&mut self, top: InstanceId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(inst) = self.assembly(cx).and_then(|a| a.instance(top).cloned()) else { return };
        let view = cx.entity();
        crate::properties_dialog::open_instance_properties(
            inst.name.clone(),
            inst.reference_designator.clone().unwrap_or_default(),
            move |v, _, cx| {
                view.update(cx, |this, cx| {
                    this.edit("Instance properties", cx, |a| {
                        if let Some(i) = a.instance_mut(top) {
                            i.reference_designator = Some(v.clone()).filter(|s| !s.is_empty());
                        }
                    });
                })
            },
            window,
            cx,
        );
    }

    /// The release state of the part / assembly an instance shows (the
    /// instance list's state pill).
    pub(super) fn instance_state(&self, inst: &Instance, cx: &App) -> String {
        let doc = &self.doc.read(cx).session.doc;
        match &inst.source {
            InstanceSource::Part { studio, body, .. } => match doc.tab(*studio).map(|t| &t.content) {
                Some(TabContent::PartStudio(ps)) => ps.part_properties.get(body).map(|p| p.meta.state().to_string()),
                _ => None,
            },
            InstanceSource::Assembly { assembly, .. } => match doc.tab(*assembly).map(|t| &t.content) {
                Some(TabContent::Assembly(a)) => Some(a.properties.meta.state().to_string()),
                _ => None,
            },
            _ => None,
        }
        .unwrap_or_else(|| noshape_model::partstudio::RELEASE_STATES[0].to_string())
    }

    /// The gray state pill after an instance's name ("In progress";
    /// `span.os-item-identifier-state`: `#eee`, 13.3 px `#333`, radius 3).
    pub(super) fn state_pill(state: String) -> AnyElement {
        div()
            .flex_none()
            .max_w(px(30.))
            .h(px(18.))
            .pl(px(4.))
            .pr(px(3.))
            .rounded(px(3.))
            .bg(rgb(0xeeeeee))
            .text_size(px(13.3))
            .text_color(rgb(0x333333))
            .overflow_hidden()
            .text_ellipsis()
            .whitespace_nowrap()
            .child(state)
            .into_any_element()
    }

    // ------------------------------------------------------------ Dynamic suppression

    /// Dynamic suppression ▸ Add expression…: the "Dynamic suppression"
    /// dialog with its "Unsuppressed if" field (instances and mates).
    pub(super) fn open_dynamic_suppression(&mut self, target: DynTarget, window: &mut Window, cx: &mut Context<Self>) {
        let current = self
            .assembly(cx)
            .and_then(|a| match target {
                DynTarget::Instance(id) => a.instance(id).and_then(|i| i.unsuppressed_if.clone()),
                DynTarget::Mate(id) => a.mate(id).and_then(|m| m.unsuppressed_if.clone()),
                DynTarget::Folder(id) => a.folders.iter().find(|f| f.id == id).and_then(|f| f.unsuppressed_if.clone()),
            })
            .unwrap_or_default();
        let st = cx.new(|cx| InputState::new(window, cx).default_value(current));
        let sub = cx.subscribe(&st, move |this: &mut Self, _, ev: &InputEvent, cx| {
            if let InputEvent::PressEnter { .. } = ev {
                this.accept_dynamic_suppression(cx);
            }
        });
        st.update(cx, |s, cx| s.focus(window, cx));
        self.ui.dynamic_suppression = Some((target, st, sub));
        cx.notify();
    }

    pub(super) fn accept_dynamic_suppression(&mut self, cx: &mut Context<Self>) {
        let Some((target, st, _)) = self.ui.dynamic_suppression.take() else { return };
        let text = Some(st.read(cx).value().trim().to_string()).filter(|t| !t.is_empty());
        self.set_dynamic_suppression(target, text, cx);
    }

    /// Set (or remove, `None`) the "Unsuppressed if" expression.
    pub(super) fn set_dynamic_suppression(&mut self, target: DynTarget, text: Option<String>, cx: &mut Context<Self>) {
        self.edit("Dynamic suppression", cx, |a| match target {
            DynTarget::Instance(id) => {
                if let Some(i) = a.instance_mut(id) {
                    i.unsuppressed_if = text.clone();
                }
            }
            DynTarget::Mate(id) => {
                if let Some(m) = a.mates.iter_mut().find(|m| m.id == id) {
                    m.unsuppressed_if = text.clone();
                }
            }
            DynTarget::Folder(id) => {
                if let Some(f) = a.folders.iter_mut().find(|f| f.id == id) {
                    f.unsuppressed_if = text.clone();
                }
            }
        });
        self.refresh_scene(true, cx);
        cx.notify();
    }

    /// Dynamic suppression menu entries: "Dynamic suppression ▸ Add
    /// expression…", or once an expression is set "Unsuppress" (drops it)
    /// and "Edit dynamic suppression" (live assembly/142).
    pub(super) fn dynamic_suppression_entries(target: DynTarget, set: bool) -> Vec<behaviors::MenuEntry> {
        use behaviors::MenuEntry;
        let open: MenuAct = Rc::new(move |t, w, cx| t.open_dynamic_suppression(target, w, cx));
        let item = |label: &str, act: MenuAct| MenuEntry::Item { label: label.into(), icon: None, disabled: false, act };
        if set {
            vec![item("Unsuppress", Rc::new(move |t, _, cx| t.set_dynamic_suppression(target, None, cx))), item("Edit dynamic suppression", open)]
        } else {
            vec![MenuEntry::Sub { label: "Dynamic suppression".into(), children: vec![item("Add expression…", open)] }]
        }
    }

    /// Onshape's 220 px feature-style dialog: "Dynamic suppression" header
    /// with ✓ ✕, one "Unsuppressed if" expression row, and `?`.
    pub(super) fn render_dynamic_suppression(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (_, st, _) = self.ui.dynamic_suppression.as_ref()?;
        let pal = p();
        let field = crate::os_ui::input_h(Input::new(st).appearance(false).text_size(px(13.33)), 25.);
        Some(
            v_flex()
                .w(px(220.))
                .bg(pal.dialog_bg)
                .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
                .text_color(pal.text)
                .child(
                    h_flex()
                        .h(px(29.))
                        .bg(rgb(0xffffff))
                        .border_b_1()
                        .border_color(rgb(0xd7d7d7))
                        .child(div().flex_1().pl(px(6.)).text_size(px(13.)).font_weight(FontWeight::MEDIUM).truncate().child("Dynamic suppression"))
                        .child(
                            div()
                                .id("dynsup-ok")
                                .size(px(28.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .bg(pal.accept)
                                .cursor_pointer()
                                .child(crate::assets::icon("dialog-ok", 24., cx))
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.accept_dynamic_suppression(cx))),
                        )
                        .child(
                            div()
                                .id("dynsup-cancel")
                                .size(px(28.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .hover(|d| d.bg(pal.panel_hover))
                                .cursor(gpui_kit::CursorStyle::Arrow).child(crate::assets::icon("dialog-cancel", 24., cx))
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.ui.dynamic_suppression = None;
                                    cx.notify();
                                })),
                        ),
                )
                .child(
                    h_flex()
                        .h(px(26.))
                        .child(div().w(px(96.)).pl(px(5.)).text_size(px(12.)).text_color(rgb(0x666666)).child("Unsuppressed if"))
                        .child(div().w(px(98.)).border_b_1().border_color(rgb(0x1651b0)).child(field)),
                )
                .child(h_flex().h(px(22.)).justify_end().pr(px(4.)).child(help_dot()))
                .into_any_element(),
        )
    }
}

// ---------------------------------------------------------------- Edit in context

impl AssemblyView {
    /// The assembly view's camera seen from a Part Studio placed by `to_studio`.
    fn camera_in(&self, to_studio: glam::DMat4, cx: &App) -> noshape_render::Camera {
        let mut cam = self.viewport.read(cx).camera;
        let (_, rot, _) = to_studio.to_scale_rotation_translation();
        cam.target = to_studio.transform_point3(cam.target);
        cam.rotation = rot * cam.rotation;
        cam
    }

    /// Instance menu › Edit in context: switch to the instance's Part
    /// Studio with the rest of the assembly shown around it (in the
    /// Part Studio's coordinates), the part selected.
    /// Assembly contexts of this assembly in a Part Studio: (id, name).
    pub(super) fn contexts_in(&self, studio: TabId, cx: &App) -> Vec<(u32, String)> {
        match self.doc.read(cx).session.doc.tab(studio).map(|t| &t.content) {
            Some(TabContent::PartStudio(ps)) => {
                let referenced = noshape_model::in_context::referenced_contexts(&ps.features);
                ps.contexts.iter().filter(|c| c.assembly == self.tab && referenced.contains(&c.id)).map(|c| (c.id, c.name.clone())).collect()
            }
            _ => Vec::new(),
        }
    }

    /// Instance › Update context ▸ <context>: a new snapshot of the
    /// assembly as it is now around the context's primary instance (or
    /// this one); features referencing it regenerate.
    pub(super) fn update_context(&mut self, top: InstanceId, studio: TabId, id: u32, cx: &mut Context<Self>) {
        let names: HashMap<InstanceId, String> = self.assembly(cx).map(|a| a.instances.iter().map(|i| (i.id, i.name.clone())).collect()).unwrap_or_default();
        let name_of = |p: &noshape_model::assembly::eval::InstancePath| names.get(&p[0]).cloned().unwrap_or_default();
        let Some(TabContent::PartStudio(ps)) = self.doc.read(cx).session.doc.tab(studio).map(|t| &t.content) else { return };
        let Some(old) = ps.contexts.iter().find(|c| c.id == id).cloned() else { return };
        let primary: Vec<InstanceId> = match old.primary.first() {
            Some(p) if names.contains_key(&InstanceId(*p)) => old.primary.iter().map(|i| InstanceId(*i)).collect(),
            _ => vec![top],
        };
        let Some(mut fresh) = noshape_model::in_context::snapshot(&self.result, self.tab, id, &old.name, &primary, &name_of) else { return };
        fresh.revision = old.revision + 1;
        fresh.hidden = old.hidden.clone();
        self.doc.update(cx, |d, cx| {
            d.edit_document(&format!("Update context : {}", old.name), cx, |doc| {
                if let Some(TabContent::PartStudio(ps)) = doc.tab_mut(studio).map(|t| &mut t.content)
                    && let Some(c) = ps.contexts.iter_mut().find(|c| c.id == id)
                {
                    *c = fresh;
                }
            })
        });
        cx.default_global::<StaleContexts>().0.remove(&(studio, id));
        self.doc.update(cx, |_, cx| cx.emit(crate::doc_model::DocEvent::TabChanged(studio)));
    }

    /// The Assembly contexts an instance takes part in: (studio, context
    /// id, name, primary instance?, update available?). An instance of the
    /// primary instance's part that is not the primary is listed too
    /// (dashed arrow, live 517).
    pub(super) fn instance_contexts(&self, top: InstanceId, cx: &App) -> Vec<(TabId, u32, String, bool, bool)> {
        let Some(inst) = self.assembly(cx).and_then(|a| a.instance(top)) else { return Vec::new() };
        let (studio, body) = match &inst.source {
            InstanceSource::Part { studio, body, .. } => (*studio, Some(body.clone())),
            InstanceSource::PartStudio { studio, .. } => (*studio, None),
            _ => return Vec::new(),
        };
        let Some(TabContent::PartStudio(ps)) = self.doc.read(cx).session.doc.tab(studio).map(|t| &t.content) else { return Vec::new() };
        // A context exists for the user only once a feature references it
        // (live assembly/548: Edit in context alone adds no context arrow).
        let referenced = noshape_model::in_context::referenced_contexts(&ps.features);
        ps.contexts
            .iter()
            .filter(|c| c.assembly == self.tab && referenced.contains(&c.id))
            .filter_map(|c| {
                let primary = c.primary == vec![top.0];
                if !primary && (body.is_none() || c.primary_body != body) {
                    return None;
                }
                let path: Vec<InstanceId> = c.primary.iter().map(|i| InstanceId(*i)).collect();
                let current = noshape_model::in_context::state_hash(&self.result, &path);
                let out_of_date = c.state != 0 && current.is_some_and(|h| h != c.state);
                Some((studio, c.id, c.name.clone(), primary, out_of_date))
            })
            .collect()
    }

    /// Publish which Part Studio contexts of this assembly are out of date
    /// (the Part Studio's blue dots, live 520–521).
    pub(super) fn publish_stale_contexts(&self, cx: &mut Context<Self>) {
        let mut flags: Vec<((TabId, u32), bool)> = Vec::new();
        for t in &self.doc.read(cx).session.doc.tabs {
            let TabContent::PartStudio(ps) = &t.content else { continue };
            let referenced = noshape_model::in_context::referenced_contexts(&ps.features);
            for c in ps.contexts.iter().filter(|c| c.assembly == self.tab && referenced.contains(&c.id)) {
                let path: Vec<InstanceId> = c.primary.iter().map(|i| InstanceId(*i)).collect();
                let current = noshape_model::in_context::state_hash(&self.result, &path);
                flags.push(((t.id, c.id), c.state != 0 && current.is_some_and(|h| h != c.state)));
            }
        }
        let stale = &mut cx.default_global::<StaleContexts>().0;
        for (key, out_of_date) in flags {
            if out_of_date {
                stale.insert(key);
            } else {
                stale.remove(&key);
            }
        }
    }

    /// Instance row icons: a solid ⇦ on the primary instance (tooltip the
    /// context), a dashed one on other instances of its part, and a blue
    /// dot while an update is available (live 517, 519).
    pub(super) fn context_icons(&self, top: InstanceId, cx: &App) -> Option<AnyElement> {
        let list = self.instance_contexts(top, cx);
        let first = list.first()?;
        let primary = list.iter().any(|c| c.3);
        let names: Vec<String> = list.iter().map(|c| c.2.clone()).collect();
        let tip = if primary { names.join(", ") } else { format!("{}. Instance is not primary instance of context.", names.join(", ")) };
        let stale: Vec<String> = list.iter().filter(|c| c.4 && c.3).map(|c| c.2.clone()).collect();
        let _ = first;
        // Out of date: one icon (a short ⇦ and the blue dot, live
        // ns-icon-incontext-import-out-of-date) with one tooltip (qa5 DER11).
        if !stale.is_empty() {
            return Some(
                div()
                    .id(SharedString::from(format!("ctx-icon-{}", top.0)))
                    .flex_none()
                    .tooltip(crate::ui::tooltip(format!("Update available for {}", stale.join(", "))))
                    .child(crate::assets::icon("incontext-arrow-out-of-date", 20., cx))
                    .into_any_element(),
            );
        }
        Some(div().id(SharedString::from(format!("ctx-icon-{}", top.0))).flex_none().tooltip(crate::ui::tooltip(tip)).child(crate::part_studio::context_arrow_icon(false, !primary)).into_any_element())
    }

    /// Instance › Set as primary instance ▸ <context>: the context is
    /// re-anchored on this instance (new snapshot around it).
    pub(super) fn set_primary_instance(&mut self, top: InstanceId, studio: TabId, id: u32, cx: &mut Context<Self>) {
        if let Some(TabContent::PartStudio(ps)) = self.doc.read(cx).session.doc.tab(studio).map(|t| &t.content)
            && let Some(c) = ps.contexts.iter().find(|c| c.id == id)
        {
            let mut moved = c.clone();
            moved.primary = vec![top.0];
            self.doc.update(cx, |d, cx| {
                d.edit_document_not_undoable("Set as primary instance", cx, |doc| {
                    if let Some(TabContent::PartStudio(ps)) = doc.tab_mut(studio).map(|t| &mut t.content)
                        && let Some(c) = ps.contexts.iter_mut().find(|c| c.id == id)
                    {
                        *c = moved;
                    }
                })
            });
            // The Part Studio regenerates on its own tab's change event.
            self.doc.update(cx, |_, cx| cx.emit(crate::doc_model::DocEvent::TabChanged(studio)));
        }
        self.update_context(top, studio, id, cx);
    }

    pub(super) fn edit_in_context(&mut self, top: InstanceId, cx: &mut Context<Self>) {
        self.edit_in_context_with(top, None, cx);
    }

    /// Edit in context in an existing context (`Some`) or a new one.
    pub(super) fn edit_in_context_with(&mut self, top: InstanceId, existing: Option<u32>, cx: &mut Context<Self>) {
        let Some(inst) = self.assembly(cx).and_then(|a| a.instance(top).cloned()) else { return };
        let Some(studio) = inst.source.studio() else { return };
        let Some(own) = self.result.occurrences.iter().find(|o| o.top() == top) else { return };
        let to_studio = own.transform.inverse();
        let occurrences = self
            .result
            .occurrences
            .iter()
            .filter(|o| o.top() != top && !o.hidden && matches!(o.kind, BodyKind::Solid | BodyKind::Surface))
            .map(|o| crate::part_studio::ContextOccurrence {
                top: o.top(),
                name: self.assembly(cx).and_then(|a| a.instance(o.top())).map(|i| i.name.clone()).unwrap_or_else(|| o.instance_name.clone()),
                mesh: o.mesh.clone(),
                transform: to_studio * o.transform,
                color: o.color,
            })
            .collect();
        let assembly_name = self.doc.read(cx).session.doc.tab(self.tab).map(|t| t.name.clone()).unwrap_or_default();
        let part = match &inst.source {
            InstanceSource::Part { body, .. } => Some(body.clone()),
            _ => None,
        };
        let camera = Some(self.camera_in(to_studio, cx));
        let context = self.add_assembly_context(studio, Some(top), existing, to_studio, cx);
        cx.emit(AssemblyViewEvent::EditInContext(studio, Box::new(crate::part_studio::AssemblyContext { assembly: self.tab, assembly_name, part, occurrences, insert_at: None, camera, context })));
    }

    /// The Assembly context (snapshot) a Part Studio shows while edited in
    /// context: the one of this assembly with the same primary instance, or
    /// a new "Context N". Not undoable (Onshape keeps it only once a
    /// feature references it, see `PartStudioView::exit_context`).
    fn add_assembly_context(&mut self, studio: TabId, primary: Option<InstanceId>, existing: Option<u32>, to_studio: glam::DMat4, cx: &mut Context<Self>) -> Option<u32> {
        let names: HashMap<InstanceId, String> = self.assembly(cx).map(|a| a.instances.iter().map(|i| (i.id, i.name.clone())).collect()).unwrap_or_default();
        let name_of = |p: &noshape_model::assembly::eval::InstancePath| names.get(&p[0]).cloned().unwrap_or_default();
        let asm_tab = self.tab;
        let primary_path = primary.map(|p| vec![p]);
        let existing = match self.doc.read(cx).session.doc.tab(studio).map(|t| &t.content) {
            Some(TabContent::PartStudio(ps)) => existing.filter(|id| ps.contexts.iter().any(|c| c.id == *id && c.assembly == asm_tab)),
            _ => return None,
        };
        let new_context = match existing {
            Some(_) => None,
            None => {
                let (id, name) = match self.doc.read(cx).session.doc.tab(studio).map(|t| &t.content) {
                    Some(TabContent::PartStudio(ps)) => {
                        let referenced = noshape_model::in_context::referenced_contexts(&ps.features);
                        let kept: Vec<_> = ps.contexts.iter().filter(|c| referenced.contains(&c.id)).cloned().collect();
                        noshape_model::in_context::next_context(&kept)
                    }
                    _ => return None,
                };
                Some(match &primary_path {
                    Some(p) => noshape_model::in_context::snapshot(&self.result, asm_tab, id, &name, p, &name_of)?,
                    None => noshape_model::in_context::snapshot_with(&self.result, asm_tab, id, &name, to_studio, None, &name_of),
                })
            }
        };
        let id = existing.or(new_context.as_ref().map(|c| c.id))?;
        self.doc.update(cx, |d, cx| {
            d.edit_document_not_undoable("Edit in context", cx, |doc| {
                if let Some(TabContent::PartStudio(ps)) = doc.tab_mut(studio).map(|t| &mut t.content) {
                    if let Some(c) = new_context {
                        // Contexts no feature references don't persist
                        // (Onshape); a new one replaces them.
                        let referenced = noshape_model::in_context::referenced_contexts(&ps.features);
                        ps.contexts.retain(|c| referenced.contains(&c.id));
                        ps.contexts.push(c);
                    }
                    ps.active_context = Some(id);
                }
            })
        });
        // The Part Studio regenerates on its own tab's change event.
        self.doc.update(cx, |_, cx| cx.emit(crate::doc_model::DocEvent::TabChanged(studio)));
        Some(id)
    }
}

// ---------------------------------------------------------------- Reference manager

/// A referenced document with a newer version (Reference manager row).
#[derive(Clone)]
pub(crate) struct RefDoc {
    pub name: String,
    /// "Main" (this document's workspace) or the pinned version's label.
    pub current: String,
    /// Newest first; the first is what "Update" moves to.
    pub versions: Vec<crate::derived::VersionChoice>,
    /// External keys of the instances (empty: this document's workspace
    /// references).
    pub keys: Vec<String>,
    pub path: std::path::PathBuf,
    /// The document thumbnail (66x40 in the row, live 111).
    pub thumbnail: Option<std::sync::Arc<RenderImage>>,
}

/// The open Reference manager: tab (0 Update to latest, 1 Selective
/// update), documents, checked rows and per-row chosen versions.
pub(crate) struct RefManager {
    /// Change to version…: only these instances (else every reference).
    pub scope: Option<Vec<InstanceId>>,
    pub tab: u8,
    pub docs: Vec<RefDoc>,
    pub checked: HashSet<usize>,
    pub chosen: HashMap<usize, usize>,
}

impl AssemblyView {
    /// Referenced documents that have newer versions.
    fn referenced_docs(&self, scope: Option<&[InstanceId]>, cx: &App) -> Vec<RefDoc> {
        let mut out = Vec::new();
        let Some(asm) = self.assembly(cx) else { return out };
        let dm = self.doc.read(cx);
        let in_scope = |i: &Instance| scope.is_none_or(|s| s.contains(&i.id));
        // This document's workspace references (Main) and its versions.
        let local = asm.instances.iter().filter(|i| in_scope(i)).any(|i| matches!(i.source, InstanceSource::Part { .. } | InstanceSource::PartStudio { .. } | InstanceSource::Assembly { .. }));
        if local && let Ok(vs) = dm.session.store.list_versions() {
            let versions: Vec<crate::derived::VersionChoice> = vs.iter().rev().map(|v| crate::derived::VersionChoice { label: v.name.clone(), microversion: v.microversion }).collect();
            if !versions.is_empty() {
                let thumbnail = dm.session.store.get_meta("thumbnail").ok().flatten().as_deref().and_then(crate::start_library::decode_thumbnail);
                out.push(RefDoc { name: dm.name(), current: dm.session.store.branch(dm.session.branch).map(|b| b.name).unwrap_or_else(|_| "Main".into()), versions, keys: Vec::new(), path: dm.session.path.clone(), thumbnail });
            }
        }
        // Other documents (and this one pinned to a version).
        let mut by_path: Vec<(std::path::PathBuf, Vec<String>, noshape_store::MicroversionId)> = Vec::new();
        for i in asm.instances.iter().filter(|i| in_scope(i)) {
            let Some(k) = i.source.external_key() else { continue };
            let Some(r) = crate::derived::ExternalRef::parse(k) else { continue };
            match by_path.iter_mut().find(|(p, _, mv)| *p == r.path && *mv == r.microversion) {
                Some(e) => {
                    if !e.1.iter().any(|x| x == k) {
                        e.1.push(k.to_string());
                    }
                }
                None => by_path.push((r.path.clone(), vec![k.to_string()], r.microversion)),
            }
        }
        for (path, keys, mv) in by_path {
            let Ok(info) = crate::derived::external_doc_info(&path) else { continue };
            let versions: Vec<crate::derived::VersionChoice> = info.versions.iter().filter(|v| v.label != "Current state").cloned().collect();
            if versions.first().is_none_or(|v| v.microversion == mv) {
                continue;
            }
            let current = versions.iter().find(|v| v.microversion == mv).map(|v| v.label.clone()).unwrap_or_else(|| format!("state {}", mv.short()));
            let thumbnail = crate::start_library::file_thumbnail(&path);
            out.push(RefDoc { name: info.name, current, versions, keys, path, thumbnail });
        }
        out
    }

    /// Instance menu › Change to version… and the toolbar's Update all
    /// references: Onshape's Reference manager dialog.
    /// `scope`: Change to version… on these instances (live 604–607: Update
    /// all then changes only them).
    pub(super) fn open_reference_manager(&mut self, scope: Option<Vec<InstanceId>>, cx: &mut Context<Self>) {
        let docs = self.referenced_docs(scope.as_deref(), cx);
        self.ui.ref_manager = Some(RefManager { scope, tab: 0, docs, checked: HashSet::new(), chosen: HashMap::new() });
        cx.notify();
    }

    /// Whether another document referenced here has a newer version (the
    /// toolbar's Update all references button is enabled then; live 621).
    pub(super) fn has_newer_references(&self, cx: &App) -> bool {
        let own = &self.doc.read(cx).session.path;
        self.ext_updates.iter().any(|(k, v)| v.is_some() && crate::derived::ExternalRef::parse(k).is_some_and(|r| r.path != *own))
    }

    /// Move one document's references to `version` (index into its versions).
    fn update_reference(&mut self, d: &RefDoc, version: usize, scope: Option<&[InstanceId]>, cx: &mut Context<Self>) -> usize {
        let Some(to) = d.versions.get(version).cloned() else { return 0 };
        let scope: Option<Vec<InstanceId>> = scope.map(|s| s.to_vec());
        let in_scope = move |i: &Instance| scope.as_ref().is_none_or(|s| s.contains(&i.id));
        if d.keys.is_empty() {
            // Workspace references of this document become version references.
            let path = d.path.clone();
            return self
                .edit(&format!("Change to version : {}", to.label), cx, |a| {
                    let mut n = 0;
                    for i in a.instances.iter_mut().filter(|i| in_scope(i)) {
                        let (tab, body, config) = match &i.source {
                            InstanceSource::Part { studio, body, config } => (*studio, Some(body.clone()), config.clone()),
                            InstanceSource::PartStudio { studio, config } => (*studio, None, config.clone()),
                            // A subassembly: that version of the Assembly (evaluated as a linked assembly).
                            InstanceSource::Assembly { assembly, config } => (*assembly, None, config.clone()),
                            _ => continue,
                        };
                        let key = crate::derived::ExternalRef { path: path.clone(), microversion: to.microversion, tab: tab.to_string() }.key();
                        i.source = InstanceSource::External { key, body, config };
                        n += 1;
                    }
                    n
                })
                .unwrap_or(0);
        }
        let mut n = 0;
        for k in &d.keys {
            let Some(r) = crate::derived::ExternalRef::parse(k) else { continue };
            let new_key = crate::derived::ExternalRef { microversion: to.microversion, ..r }.key();
            n += self
                .edit(&format!("Update to {}", to.label), cx, |a| {
                    let mut n = 0;
                    for i in a.instances.iter_mut().filter(|i| in_scope(i)) {
                        if let InstanceSource::External { key, .. } = &mut i.source
                            && key == k
                        {
                            *key = new_key.clone();
                            n += 1;
                        }
                    }
                    n
                })
                .unwrap_or(0);
            self.ext_updates.remove(k);
        }
        n
    }

    /// Update all / Update selected: no message; the dialog stays open and
    /// lists what is still out of date (live 606–607).
    fn apply_reference_manager(&mut self, cx: &mut Context<Self>) {
        let Some(m) = self.ui.ref_manager.take() else { return };
        for (i, d) in m.docs.iter().enumerate() {
            if m.tab == 1 && !m.checked.contains(&i) {
                continue;
            }
            self.update_reference(d, m.chosen.get(&i).copied().unwrap_or(0), m.scope.as_deref(), cx);
        }
        let docs = self.referenced_docs(m.scope.as_deref(), cx);
        self.ui.ref_manager = Some(RefManager { scope: m.scope, tab: m.tab, docs, checked: HashSet::new(), chosen: HashMap::new() });
        self.check_external_updates(cx);
        self.refresh_scene(true, cx);
        cx.notify();
    }

    /// The 330x433 "Reference manager" dialog docked at the top left.
    pub(super) fn render_reference_manager(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let m = self.ui.ref_manager.as_ref()?;
        let pal = p();
        let tab = m.tab;
        let tab_button = |id: &'static str, label: &'static str, ix: u8| {
            let on = tab == ix;
            div()
                .id(id)
                .flex_1()
                .h(px(32.))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(12.))
                .cursor_pointer()
                .when(on, |d| d.font_weight(FontWeight::MEDIUM).text_color(rgb(0x1651b0)).border_b_2().border_color(rgb(0x1651b0)))
                .when(!on, |d| d.text_color(rgb(0x333333)))
                .child(label)
        };
        let n = m.docs.len();
        let mut list = v_flex().flex_1().min_h_0();
        for (i, d) in m.docs.iter().enumerate() {
            let chosen = m.chosen.get(&i).copied().unwrap_or(0);
            let newest = d.versions.get(chosen).map(|v| v.label.clone()).unwrap_or_default();
            let checked = m.checked.contains(&i);
            let versions: Vec<String> = d.versions.iter().map(|v| v.label.clone()).collect();
            list = list.child(
                h_flex()
                    .h(px(40.))
                    .my(px(2.))
                    .when(tab == 1, |r| {
                        r.child(
                            div().w(px(19.)).pl(px(4.)).child(
                                div()
                                    .id(SharedString::from(format!("rm-check-{i}")))
                                    .size(px(13.))
                                    .rounded(px(2.))
                                    .border_1()
                                    .border_color(rgb(0x767676))
                                    .bg(if checked { rgb(0x1651b0) } else { rgb(0xffffff) })
                                    .when(checked, |d| d.child(Icon::new(IconName::Check).size(px(10.)).text_color(gpui_kit::rgb_static(0xffffff))))
                                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                        if let Some(m) = &mut this.ui.ref_manager
                                            && !m.checked.remove(&i)
                                        {
                                            m.checked.insert(i);
                                        }
                                        cx.notify();
                                    })),
                            ),
                        )
                    })
                    .child(div().w(px(70.)).h(px(40.)).px(px(2.)).flex_none().flex().items_center().justify_center().child(match d.thumbnail.clone() {
                        Some(img) => gpui_kit::img(img).w(px(66.)).h(px(40.)).object_fit(ObjectFit::Contain).into_any_element(),
                        None => crate::assets::icon(&icon_path("document"), 32., cx).into_any_element(),
                    }))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .pl(px(4.))
                            .child(div().text_size(px(14.67)).font_weight(FontWeight::MEDIUM).text_color(rgb(0x333333)).truncate().child(d.name.clone()))
                            .child(div().text_size(px(12.)).opacity(0.5).text_color(rgb(0x333333)).truncate().child(format!("{} ⇨ {newest}", d.current))),
                    )
                    .when(tab == 1, |r| {
                        r.child(
                            div()
                                .id(SharedString::from(format!("rm-graph-{i}")))
                                .mr(px(21.))
                                .size(px(30.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(3.))
                                .border_1()
                                .border_color(rgb(0xaaaaaa))
                                .cursor_pointer()
                                .tooltip(crate::ui::tooltip("Version graph"))
                                .child(crate::ui::glyph("version-graph", 20., rgb(0x333333)))
                                .on_click(cx.listener(move |_this, ev: &ClickEvent, window, cx| {
                                    let view = cx.entity();
                                    let versions = versions.clone();
                                    let menu = crate::os_menu::OsMenu::build(window, cx, move |mut menu, _, _| {
                                        for (vi, label) in versions.iter().enumerate() {
                                            let v = view.clone();
                                            menu = menu.item(OsMenuItem::new(label.clone()).checked(vi == chosen).on_click(move |_, _, cx| {
                                                v.update(cx, |this, cx| {
                                                    if let Some(m) = &mut this.ui.ref_manager {
                                                        m.chosen.insert(i, vi);
                                                        m.checked.insert(i);
                                                    }
                                                    cx.notify();
                                                })
                                            }));
                                        }
                                        menu
                                    });
                                    crate::os_menu::open_anchored(ev.position(), Anchor::TopLeft, menu, window, cx);
                                })),
                        )
                    }),
            );
        }
        let enabled = n > 0 && (tab == 0 || !m.checked.is_empty());
        // Live 607: the line stays, "Newer versions available for 0 documents".
        let info = format!("Newer versions available for {n} document{}", if n == 1 { "" } else { "s" });
        Some(
            v_flex()
                .w(px(330.))
                .h(px(433.))
                .bg(pal.dialog_bg)
                .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
                .child(
                    h_flex()
                        .h(px(29.))
                        .bg(rgb(0xffffff))
                        .border_b_1()
                        .border_color(rgb(0xd7d7d7))
                        .child(div().flex_1().pl(px(6.)).text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Reference manager"))
                        .child(
                            div()
                                .id("rm-close")
                                .size(px(28.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .opacity(0.5)
                                .cursor_pointer()
                                // Bootstrap `btn-close` (live 111: an 11 px ✕).
                                .child(crate::os_ui::glyph(crate::os_ui::GLYPH_CLOSE, 20., rgb(0x000000)))
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.ui.ref_manager = None;
                                    cx.notify();
                                })),
                        ),
                )
                .child(
                    h_flex()
                        .bg(rgb(0xffffff))
                        .border_b_1()
                        .border_color(rgb(0xeeeeee))
                        .child(tab_button("rm-tab-latest", "Update to latest", 0).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            if let Some(m) = &mut this.ui.ref_manager {
                                m.tab = 0;
                            }
                            cx.notify();
                        })))
                        .child(tab_button("rm-tab-selective", "Selective update", 1).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            if let Some(m) = &mut this.ui.ref_manager {
                                m.tab = 1;
                            }
                            cx.notify();
                        }))),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_h_0()
                        .bg(rgb(0xffffff))
                        .pt(px(6.))
                        .child(
                            h_flex()
                                .h(px(30.))
                                .px(px(3.))
                                .border_b_1()
                                .border_color(rgb(0xeaeaea))
                                // Live dialog-reference-manager: the two-tone 20 px icon, text right after it.
                                .child(crate::assets::icon(&icon_path("inserted-version-update"), 20., cx))
                                .child(div().text_size(px(12.)).text_color(rgb(0x333333)).child(info)),
                        )
                        .child(list),
                )
                .child(
                    h_flex().h(px(46.)).px(px(5.)).justify_end().border_t_1().border_b_1().border_color(rgb(0xeaeaea)).child(
                        div()
                            .id("rm-update")
                            .h(px(34.))
                            .px(px(10.5))
                            .flex()
                            .items_center()
                            .rounded(px(5.25))
                            .bg(rgb(0x1651b0))
                            .text_size(px(14.))
                            .text_color(gpui_kit::rgb_static(0xffffff))
                            .when(!enabled, |d| d.opacity(0.4))
                            .when(enabled, |d| d.cursor_pointer())
                            .child(if tab == 0 { "Update all" } else { "Update selected" })
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                if enabled {
                                    this.apply_reference_manager(cx);
                                }
                            })),
                    ),
                )
                .child(h_flex().h(px(22.)).justify_end().pr(px(4.)).child(help_dot()))
                .into_any_element(),
        )
    }
}

// ---------------------------------------------------------------- Create Part Studio in context

impl AssemblyView {
    /// Toolbar / Origin row › Create Part Studio in context…: the "Origin
    /// of new Part Studio" dialog, its field pre-filled with the Origin.
    pub(super) fn open_ps_in_context(&mut self, cx: &mut Context<Self>) {
        self.ui.ps_in_context = Some(None);
        cx.notify();
    }

    /// ✓: a new Part Studio whose origin sits at the chosen connector, opened
    /// with the assembly as context graphics.
    pub(super) fn accept_ps_in_context(&mut self, cx: &mut Context<Self>) {
        let Some(origin) = self.ui.ps_in_context.take() else { return };
        let frame = origin.and_then(|id| self.result.connector_frame(id)).unwrap_or_default();
        let to_studio = noshape_model::assembly::eval::frame_matrix(&frame).inverse();
        let after = self.tab;
        let mut new_tab = None;
        self.doc.update(cx, |doc, cx| {
            doc.edit_document("Create Part Studio in context", cx, |d| {
                let name = d.next_tab_name("Part Studio");
                let id = d.add_tab(&name, TabContent::PartStudio(noshape_model::PartStudio::new()));
                crate::workspace::place_after(d, id, Some(after));
                new_tab = Some(id);
            })
        });
        let Some(studio) = new_tab else { return };
        let occurrences = self
            .result
            .occurrences
            .iter()
            .filter(|o| !o.hidden && matches!(o.kind, BodyKind::Solid | BodyKind::Surface))
            .map(|o| crate::part_studio::ContextOccurrence {
                top: o.top(),
                name: self.assembly(cx).and_then(|a| a.instance(o.top())).map(|i| i.name.clone()).unwrap_or_else(|| o.instance_name.clone()),
                mesh: o.mesh.clone(),
                transform: to_studio * o.transform,
                color: o.color,
            })
            .collect();
        let assembly_name = self.doc.read(cx).session.doc.tab(self.tab).map(|t| t.name.clone()).unwrap_or_default();
        let insert_at = Some(frame_to_placement(&frame));
        let camera = Some(self.camera_in(to_studio, cx));
        let context = self.add_assembly_context(studio, None, None, to_studio, cx);
        cx.emit(AssemblyViewEvent::EditInContext(studio, Box::new(crate::part_studio::AssemblyContext { assembly: self.tab, assembly_name, part: None, occurrences, insert_at, camera, context })));
        cx.notify();
    }

    /// "Insert and go to assembly" from a Part Studio created in context:
    /// its parts are inserted at the Part Studio's origin connector.
    pub fn insert_from_context(&mut self, studio: TabId, parts: Vec<BodyId>, sketches: Vec<FeatureId>, at: Placement, cx: &mut Context<Self>) {
        let names: HashMap<BodyId, String> = self.evaluator.lock().studio(&self.doc.read(cx).session.doc, studio, &ConfigValues::default()).map(|r| r.display.bodies.iter().map(|b| (b.id.clone(), b.name.clone())).collect()).unwrap_or_default();
        let sketch_names: HashMap<FeatureId, String> = match self.doc.read(cx).session.doc.tab(studio).map(|t| &t.content) {
            Some(noshape_model::TabContent::PartStudio(ps)) => ps.features.iter().map(|f| (f.id, f.name.clone())).collect(),
            _ => HashMap::new(),
        };
        self.edit("Insert and go to assembly", cx, |a| {
            let sources = parts.iter().map(|b| (names.get(b).map(|s| s.as_str()).unwrap_or("Part"), InstanceSource::Part { studio, body: b.clone(), config: ConfigValues::default() }));
            let sketch_sources = sketches.iter().map(|f| (sketch_names.get(f).map(|s| s.as_str()).unwrap_or("Sketch"), InstanceSource::Sketch { studio, sketch: *f, config: ConfigValues::default() }));
            for (base, source) in sources.chain(sketch_sources) {
                let id = InstanceId(a.next_id());
                let name = a.next_instance_name(base);
                let mut inst = Instance::new(id, name, source, at);
                inst.hidden = false;
                a.instances.push(inst);
            }
        });
        self.refresh_scene(true, cx);
    }

    /// The 220 px "Origin of new Part Studio" dialog.
    pub(super) fn render_ps_in_context_dialog(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let origin = *self.ui.ps_in_context.as_ref()?;
        let pal = p();
        let label = match origin {
            None => "Origin".to_string(),
            Some(id) => self.assembly(cx).and_then(|a| a.connector(id).map(|c| c.name.clone())).unwrap_or_else(|| "Origin".into()),
        };
        let chip = h_flex()
            .h(px(24.))
            .pl(px(6.))
            .pr(px(5.))
            .text_size(px(13.33))
            .text_color(rgb(0x555555))
            .child(div().flex_1().truncate().child(label.clone()))
            .child(
                div()
                    .id("psic-del")
                    .px(px(5.))
                    .text_size(px(16.67))
                    .text_color(rgb(0x666666))
                    .cursor_pointer()
                    .tooltip(crate::ui::tooltip(format!("Delete {label}")))
                    .child("×")
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.ui.ps_in_context = Some(None);
                        cx.notify();
                    })),
            );
        Some(
            v_flex()
                .w(px(220.))
                .bg(pal.dialog_bg)
                .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
                .child(
                    h_flex()
                        .h(px(29.))
                        .bg(rgb(0xffffff))
                        .border_b_1()
                        .border_color(rgb(0xd7d7d7))
                        .child(div().flex_1().pl(px(6.)).text_size(px(13.)).font_weight(FontWeight::MEDIUM).truncate().child("Origin of new Part Studio"))
                        .child(
                            div()
                                .id("psic-ok")
                                .size(px(28.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .bg(pal.accept)
                                .cursor_pointer()
                                .child(crate::assets::icon("dialog-ok", 24., cx))
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.accept_ps_in_context(cx))),
                        )
                        .child(
                            div()
                                .id("psic-cancel")
                                .size(px(28.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .hover(|d| d.bg(pal.panel_hover))
                                .cursor(gpui_kit::CursorStyle::Arrow).child(crate::assets::icon("dialog-cancel", 24., cx))
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.ui.ps_in_context = None;
                                    cx.notify();
                                })),
                        ),
                )
                .child(
                    div().p(px(3.)).child(
                        v_flex()
                            .rounded(px(2.))
                            .border_1()
                            .border_color(rgb(0x1651b0))
                            .bg(rgb(0xdef1fb))
                            .child(div().px(px(6.)).pt(px(6.)).pb(px(2.)).text_size(px(12.)).text_color(rgb(0x555555)).child("Select origin/mate connector"))
                            .child(chip),
                    ),
                )
                .child(h_flex().h(px(22.)).justify_end().pr(px(4.)).child(help_dot()))
                .into_any_element(),
        )
    }
}

/// Part Studio contexts (studio tab, context id) whose assembly has moved on
/// since their last update, as last seen by the assembly.
#[derive(Default)]
pub struct StaleContexts(pub std::collections::HashSet<(TabId, u32)>);

impl Global for StaleContexts {}

#[cfg(test)]
mod tests {
    //! T10 fixture: a document whose assembly references another
    //! document's Part Studio at V1 while that document has a V2. With
    //! `NOSHAPE_T10_DIR` set, the two documents are written there (for the
    //! app check: open "T10 source" once so it gets its thumbnail, then
    //! "T10 assembly": the toolbar's Update all references is enabled and
    //! the row shows the newer-version badge).
    use std::prelude::v1::test;

    #[test]
    fn t10_fixture_reports_the_newer_version_of_a_referenced_document() {
        use noshape_model::assembly::{Assembly, Instance, InstanceId, InstanceSource, Placement};
        use noshape_model::document::{Document, TabContent};
        let dir = std::env::var_os("NOSHAPE_T10_DIR").map(std::path::PathBuf::from).unwrap_or_else(|| std::env::temp_dir().join(format!("noshape-t10-{}", std::process::id())));
        std::fs::create_dir_all(&dir).unwrap();
        let (src_path, asm_path) = (dir.join("T10 source.noshape"), dir.join("T10 assembly.noshape"));
        for p in [&src_path, &asm_path] {
            let _ = std::fs::remove_file(p);
        }
        let mut src = crate::session::Session::create(&src_path, noshape_model::demo::demo_document()).unwrap();
        src.create_version("V1", "").unwrap();
        let v1 = src.head().unwrap();
        let tab = src.doc.tabs[0].id;
        src.doc.tabs[0].name = "Plate".into();
        src.commit_all("Rename tab").unwrap();
        src.create_version("V2", "").unwrap();
        let v2 = src.head().unwrap();

        let key = crate::derived::ExternalRef { path: src_path.clone(), microversion: v1, tab: tab.to_string() }.key();
        let mut doc = Document::new();
        let mut asm = Assembly::default();
        asm.instances.push(Instance::new(InstanceId(1), "Part Studio 1 <1>".into(), InstanceSource::External { key: key.clone(), body: None, config: Default::default() }, Placement::default()));
        let tab = doc.tabs.iter_mut().find(|t| matches!(t.content, TabContent::Assembly(_))).expect("a new document has Assembly 1");
        tab.content = TabContent::Assembly(Box::new(asm));
        drop(crate::session::Session::create(&asm_path, doc).unwrap());

        let r = crate::derived::ExternalRef::parse(&key).unwrap();
        let newer = crate::derived::newer_version(&r).expect("V2 is newer");
        assert_eq!((newer.label.as_str(), newer.microversion), ("V2", v2));
        // Once pinned to V2 there is nothing newer.
        assert!(crate::derived::newer_version(&crate::derived::ExternalRef { microversion: v2, ..r }).is_none());
        if std::env::var_os("NOSHAPE_T10_DIR").is_none() {
            let _ = std::fs::remove_dir_all(&dir);
        }
    }
}
