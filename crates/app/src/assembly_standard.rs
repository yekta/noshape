//! Standard content (child module of `assembly_view`): the Insert dialog's
//! Standard content tab (Standard › Category › Class › Component, Size with
//! auto-size from a hole, Length), inserting with a snap to a hole's mate
//! connector and a Fastened mate (`a` flips the part), and Edit standard
//! content instance.

use super::*;
use gpui_kit::component::Disableable as _;
use noshape_model::assembly::standard::{self, StandardSpec};

/// Standard content state of the Insert dialog.
#[derive(Default)]
pub(crate) struct StdUi {
    /// Component, size and length chosen in the tab.
    pub spec: Option<StandardSpec>,
    /// Auto-size: waiting for a hole (circular edge, cylindrical or planar face).
    pub auto_size: bool,
    /// Holes picked with auto-size: Insert fastens a part in each.
    pub holes: Vec<MateConnectorRef>,
    /// Edit standard content instance: the instances being edited.
    pub editing: Option<Vec<InstanceId>>,
    /// `a` while inserting: the part is flipped on its mate connector.
    pub flip: bool,
}

impl AssemblyView {
    /// Spec of the tab, starting with Onshape's first standard's first component.
    fn std_spec(&mut self) -> StandardSpec {
        self.ui.std.spec.get_or_insert_with(|| StandardSpec::new(&standard::CATALOGUE[0])).clone()
    }

    /// Choose a level (0 standard, 1 category, 2 class, 3 component): the
    /// levels below fall back to their first entry, as in Onshape.
    fn std_choose(&mut self, level: usize, value: &str, cx: &mut Context<Self>) {
        let cur = self.std_spec();
        let mut path = [cur.standard.clone(), cur.category.clone(), cur.class.clone(), cur.component.clone()];
        path[level] = value.to_string();
        for l in level + 1..4 {
            let options: Vec<&str> = match l {
                1 => standard::categories(&path[0]),
                2 => standard::classes(&path[0], &path[1]),
                _ => standard::components(&path[0], &path[1], &path[2]).iter().map(|c| c.name).collect(),
            };
            if !options.contains(&path[l].as_str()) {
                path[l] = options.first().map(|s| s.to_string()).unwrap_or_default();
            }
        }
        if let Some(c) = standard::components(&path[0], &path[1], &path[2]).into_iter().find(|c| c.name == path[3]) {
            let mut spec = StandardSpec::new(c);
            // Keep the size when the new component has it.
            if c.size(&cur.size).is_some() {
                spec.set_size(&cur.size);
            }
            self.ui.std.spec = Some(spec);
        }
        cx.notify();
    }

    fn std_set_size(&mut self, size: &str, cx: &mut Context<Self>) {
        let mut spec = self.std_spec();
        spec.set_size(size);
        self.ui.std.spec = Some(spec);
        cx.notify();
    }

    fn std_set_length(&mut self, length: &str, cx: &mut Context<Self>) {
        let mut spec = self.std_spec();
        spec.length = Some(length.to_string());
        self.ui.std.spec = Some(spec);
        cx.notify();
    }

    /// Auto-size button: the next pick in the graphics area sizes the part.
    fn std_toggle_auto_size(&mut self, cx: &mut Context<Self>) {
        self.ui.std.auto_size = !self.ui.std.auto_size;
        self.message = self.ui.std.auto_size.then(|| (false, "To size the component, select a circular edge or cylindrical face.".to_string()));
        self.update_capture(cx);
        cx.notify();
    }

    pub(super) fn std_auto_size_active(&self) -> bool {
        self.ui.std.auto_size && self.insert_open
    }

    /// Auto-size pick: the size fitting the picked hole (bolts and screws
    /// round down, nuts and washers up); the holes are kept so Insert
    /// fastens the part in them.
    pub(super) fn std_pick_hole(&mut self, t: AsmPickTarget, cx: &mut Context<Self>) {
        let Some(o) = self.result.occurrences.get(t.occ) else { return };
        let units = self.units(cx);
        let forward = self.viewport.read(cx).camera.forward();
        let toward = o.transform.inverse().transform_vector3(-forward).normalize_or_zero();
        let holes = standard::hole_connectors(&o.context, Some(&o.body), &t.reference, toward, &units);
        let spec = self.std_spec();
        let Some(c) = standard::component(&spec) else { return };
        // Existing standard content: its nominal diameter (a nut for a bolt).
        let existing = self.assembly(cx).and_then(|a| a.instance(t.path[0])).and_then(|i| i.source.standard()).and_then(|s| {
            let sc = standard::component(s)?;
            Some(sc.diameter_m(sc.size(&s.size)?))
        });
        let Some(d) = existing.or_else(|| holes.first().map(|h| h.1)) else {
            self.message = Some((true, "Selection is not valid for auto-sizing. Selection must be a circular edge or cylindrical face or from existing standard content.".into()));
            cx.notify();
            return;
        };
        let size = standard::auto_size(c, d);
        match size {
            Some(size) => {
                self.std_set_size(size, cx);
                self.ui.std.holes = holes.iter().map(|(p, _)| p.connector(&t.path)).collect();
                self.ui.std.auto_size = false;
                let n = self.ui.std.holes.len();
                self.message = Some((false, if n > 1 { format!("Size {size}: Insert fastens one in each of the {n} holes.") } else { format!("Size {size}: Insert fastens it in the selected hole.") }));
            }
            None => self.message = Some((true, standard::NO_MATCHING_SIZE.to_string())),
        }
        self.update_capture(cx);
        cx.notify();
    }

    /// Insert: one instance per picked hole, fastened there, or one that
    /// follows the cursor and snaps to mate connectors (click places it).
    fn std_insert(&mut self, cx: &mut Context<Self>) {
        let spec = self.std_spec();
        let holes = std::mem::take(&mut self.ui.std.holes);
        let flip = self.ui.std.flip;
        let base = spec.part_name();
        let ids = self.edit(&format!("Insert : {base}"), cx, |a| {
            let mut ids = Vec::new();
            for _ in 0..holes.len().max(1) {
                let id = InstanceId(a.next_id());
                let name = a.next_instance_name(&base);
                a.instances.push(Instance::new(id, name, InstanceSource::Standard { spec: spec.clone() }, Placement::default()));
                ids.push(id);
            }
            let stacks = standard::component(&spec).is_some_and(|c| !c.shape.has_length());
            for (id, hole) in ids.iter().zip(&holes) {
                let mut src = MateConnectorRef::on(*id, vec![], standard::connector_reference(0));
                src.flip = flip;
                // A washer or nut in a hole that already holds a bolt goes under
                // the bolt head: the bolt now sits on its top face (Onshape).
                if stacks {
                    let same = |c: &MateConnectorRef| c.instance == hole.instance && c.path == hole.path && c.reference == hole.reference && c.align == hole.align;
                    let bolt = |c: &MateConnectorRef| a.instance(c.instance).and_then(|i| i.source.standard()).and_then(standard::component).is_some_and(|c| c.shape.has_length());
                    let held = a.mates.iter().position(|m| !m.suppressed && m.mate_type == MateType::Fastened && ((same(&m.b) && bolt(&m.a)) || (same(&m.a) && bolt(&m.b))));
                    if let Some(k) = held {
                        let top = MateConnectorRef::on(*id, vec![], standard::connector_reference(1));
                        let m = &mut a.mates[k];
                        if same(&m.b) {
                            m.b = top;
                        } else {
                            m.a = top;
                        }
                    }
                }
                let mid = MateId(a.next_id());
                let n = a.next_feature_name("Fastened");
                a.mates.push(Mate::new(mid, n, MateType::Fastened, src, hole.clone()));
            }
            ids
        });
        if let Some(ids) = ids
            && holes.is_empty()
        {
            self.start_placing(ids, cx);
        }
        self.message = None;
        cx.notify();
    }

    /// Whether an instance being placed is standard content (it snaps to
    /// mate connectors without snap mode).
    pub(super) fn std_placing(&self, id: InstanceId, cx: &App) -> bool {
        self.assembly(cx).and_then(|a| a.instance(id)).is_some_and(|i| i.source.standard().is_some())
    }

    /// `a` while placing standard content: flip it on the mate connector.
    pub(super) fn std_flip_placing(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.placing.as_ref().is_some_and(|p| p.len() == 1 && self.std_placing(p[0], cx)) {
            return false;
        }
        self.ui.std.flip = !self.ui.std.flip;
        if self.update_placing(cx) {
            self.rebuild_scene(true, cx);
        }
        true
    }

    /// Instance menu: Edit standard content instance… (Size and Length).
    pub(super) fn edit_standard_instance(&mut self, top: InstanceId, cx: &mut Context<Self>) {
        let Some(spec) = self.assembly(cx).and_then(|a| a.instance(top)).and_then(|i| i.source.standard().cloned()) else { return };
        // The selection when it holds the instance (same component only).
        let mut ids = vec![top];
        if self.selected.contains(&top)
            && let Some(a) = self.assembly(cx)
        {
            ids = self.selected.iter().copied().filter(|id| a.instance(*id).and_then(|i| i.source.standard()).is_some_and(|s| s.component == spec.component)).collect();
        }
        self.ui.std.spec = Some(spec);
        self.ui.std.editing = Some(ids);
        self.insert_open = false;
        cx.notify();
    }

    /// Update: the edited instances take the new size / length (mates stay:
    /// face names do not depend on the size).
    fn std_update(&mut self, cx: &mut Context<Self>) {
        let (Some(ids), Some(spec)) = (self.ui.std.editing.take(), self.ui.std.spec.clone()) else { return };
        let name = spec.part_name();
        self.edit(&format!("Edit standard content : {name}"), cx, |a| {
            for id in &ids {
                let base = a.next_instance_name(&name);
                if let Some(i) = a.instance_mut(*id) {
                    i.source = InstanceSource::Standard { spec: spec.clone() };
                    i.name = base;
                }
            }
        });
        cx.notify();
    }

    fn std_dropdown(&self, id: &'static str, current: String, options: Vec<String>, disabled: bool, on_pick: fn(&mut AssemblyView, &str, &mut Context<AssemblyView>), cx: &mut Context<Self>) -> AnyElement {
        let view = cx.entity();
        let button = Button::new(id).label(current.clone()).outline().small().w_full().disabled(disabled);
        if disabled {
            return button.into_any_element();
        }
        button
            .dropdown_caret(true)
            .os_dropdown_menu(SharedString::from(format!("{id}-dd")), true, move |mut menu, _, _| {
                for o in &options {
                    let v = view.clone();
                    let o2 = o.clone();
                    menu = menu.item(crate::os_menu::OsMenuItem::new(o.clone()).checked(*o == current).on_click(move |_, _, cx| v.update(cx, |this, cx| on_pick(this, &o2, cx))));
                }
                menu
            })
            .into_any_element()
    }

    /// The Standard content tab body (also the Edit standard content dialog,
    /// where only Size and Length change).
    pub(super) fn render_standard_content(&mut self, editing: bool, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let spec = self.std_spec();
        let label = |s: &'static str| div().text_xs().text_color(pal.text_muted).child(s);
        let mut body = v_flex().gap_1();
        let levels: [(&'static str, &'static str, String, Vec<String>); 4] = [
            ("std-standard", "Standard", spec.standard.clone(), standard::standards().into_iter().map(String::from).collect()),
            ("std-category", "Category", spec.category.clone(), standard::categories(&spec.standard).into_iter().map(String::from).collect()),
            ("std-class", "Class", spec.class.clone(), standard::classes(&spec.standard, &spec.category).into_iter().map(String::from).collect()),
            ("std-component", "Component", spec.component.clone(), standard::components(&spec.standard, &spec.category, &spec.class).iter().map(|c| c.name.to_string()).collect()),
        ];
        let pickers: [fn(&mut AssemblyView, &str, &mut Context<AssemblyView>); 4] = [|t, v, cx| t.std_choose(0, v, cx), |t, v, cx| t.std_choose(1, v, cx), |t, v, cx| t.std_choose(2, v, cx), |t, v, cx| t.std_choose(3, v, cx)];
        for ((id, name, cur, options), pick) in levels.into_iter().zip(pickers) {
            body = body.child(label(name)).child(self.std_dropdown(id, cur, options, editing, pick, cx));
        }
        let Some(c) = standard::component(&spec) else { return body.into_any_element() };
        let sizes: Vec<String> = c.sizes.iter().map(|s| s.name.to_string()).collect();
        let size_dd = self.std_dropdown("std-size", spec.size.clone(), sizes, false, |t, v, cx| t.std_set_size(v, cx), cx);
        let auto = Button::new("std-auto-size")
            .ghost()
            .small()
            .selected(self.ui.std.auto_size)
            .child(crate::assets::icon(&icon_path("auto-size"), 16., cx))
            .tooltip("Auto-size fastener: select a hole")
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.std_toggle_auto_size(cx)));
        body = body.child(label("Size")).child(h_flex().gap_1().child(div().flex_1().child(size_dd)).when(!editing, |d| d.child(auto)));
        if let Some(size) = c.size(&spec.size)
            && c.shape.has_length()
        {
            let lengths = c.lengths(size);
            let cur = spec.length.clone().unwrap_or_default();
            body = body.child(label("Length")).child(self.std_dropdown("std-length", cur, lengths, false, |t, v, cx| t.std_set_length(v, cx), cx));
        }
        // Preview: the part's name and a short description.
        body = body.child(
            h_flex()
                .mt_1()
                .gap_2()
                .p_2()
                .bg(pal.panel)
                .child(crate::assets::icon(&icon_path("standard-content"), 28., cx))
                .child(v_flex().flex_1().min_w_0().child(div().text_sm().child(spec.part_name())).child(div().text_xs().text_color(pal.text_faint).child(format!("{} › {} › {}", spec.standard, spec.category, spec.class)))),
        );
        let holes = self.ui.std.holes.len();
        let action = if editing {
            Button::new("std-update").label("Update").primary().small().on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.std_update(cx)))
        } else {
            Button::new("std-insert").label(if holes > 1 { format!("Insert ({holes})") } else { "Insert".to_string() }).primary().small().on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.std_insert(cx)))
        };
        body = body.child(h_flex().justify_end().pt_1().child(action));
        body.into_any_element()
    }

    /// Edit standard content dialog (left column).
    pub(super) fn render_standard_edit_panel(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let body = v_flex().p_2().child(self.render_standard_content(true, cx));
        Self::panel_frame("Edit standard content", body, cx.listener(|this, _: &ClickEvent, _, cx| {
            this.ui.std.editing = None;
            cx.notify();
        }), "std-edit-close")
        .into_any_element()
    }
}
