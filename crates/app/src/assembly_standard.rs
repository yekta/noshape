//! Standard content (child module of `assembly_view`): the Insert dialog's
//! Standard content tab (live assembly/70, 900–902): Standard › Category ›
//! Class › Component, then the component's parameters (Size with auto-size,
//! Head slot, Length, Thread length, Material, Finish) in two scroll groups,
//! Part number (edit pencil) and Description, a line drawing of the
//! component, and Insert | Insert closest to selection | Insert furthest
//! from selection. Inserting snaps to a hole's mate connector with a
//! Fastened mate (`a` flips the part); Edit standard content instance.

use super::*;
use noshape_model::assembly::eval::PartOccurrence;
use noshape_model::assembly::standard::{self, Shape, StandardSpec};

/// A face or edge selected before Insert opened (live: pre-select hole
/// edges or faces, or planar faces with holes, then Insert).
#[derive(Clone)]
pub(crate) struct StdPick {
    /// The holes it gives (connector on each hole's edge).
    pub holes: Vec<MateConnectorRef>,
    /// A planar face (enables Insert closest / furthest when it has holes).
    pub planar: bool,
}

/// Standard content state of the Insert dialog.
#[derive(Default)]
pub(crate) struct StdUi {
    /// Component, size and length chosen in the tab.
    pub spec: Option<StandardSpec>,
    /// Auto-size: waiting for a hole (circular edge, cylindrical or planar face).
    pub auto_size: bool,
    /// Holes picked with auto-size: Insert fastens a part in each.
    pub holes: Vec<MateConnectorRef>,
    /// The faces and edges selected when Insert opened (`std_take_selection`).
    pub picks: Vec<StdPick>,
    /// Edit standard content instance: the instances being edited.
    pub editing: Option<Vec<InstanceId>>,
    /// `a` while inserting: the part is flipped on its mate connector.
    pub flip: bool,
    /// Part number pencil (live skb/800): the panel's in-place edit of the
    /// company/user wide part properties (Part number, Description).
    pub props_edit: Option<(Entity<InputState>, Entity<InputState>)>,
    /// The two scroll groups (hierarchy, parameters).
    pub hier_scroll: ScrollHandle,
    pub param_scroll: ScrollHandle,
    /// A size live lists but can't build, shown in the Size select while
    /// every other field keeps the previous size's data (behaviours "skb
    /// (10-23 14:00)").
    pub failed_size: Option<String>,
    /// Live's red toasts at the top centre ("Standard content component
    /// failed to solve. …"); they stack, each ✕ closes one.
    pub toasts: Vec<String>,
}

/// Where Insert puts the part in a hole that already holds standard content.
#[derive(Clone, Copy, PartialEq)]
enum Stack {
    /// Insert: a washer or nut goes under a bolt already in the hole.
    Plain,
    /// Insert closest to selection: against the picked face, what was there
    /// moves onto the new part.
    Closest,
    /// Insert furthest from selection: at the far end of the stack.
    Furthest,
}

impl AssemblyView {
    /// Spec of the tab, starting with the first of every picker (live:
    /// ANSI inch › Bolts & screws › Countersunk bolts › Countersunk bolt).
    fn std_spec(&mut self) -> StandardSpec {
        self.ui.std.spec.get_or_insert_with(|| StandardSpec::new(standard::default_component())).clone()
    }

    /// Choose a level (0 standard, 1 category, 2 class, 3 component): the
    /// levels below fall back to their first entry, as in Onshape.
    fn std_choose(&mut self, level: usize, value: &str, cx: &mut Context<Self>) {
        self.ui.std.failed_size = None;
        let cur = self.std_spec();
        let mut path = [cur.standard.clone(), cur.category.clone(), cur.class.clone(), cur.component.clone()];
        path[level] = value.to_string();
        for l in level + 1..4 {
            let options: Vec<&str> = match l {
                1 => standard::categories(&path[0]),
                2 => standard::classes(&path[0], &path[1]),
                _ => standard::components(&path[0], &path[1], &path[2]).iter().map(|c| c.name.as_str()).collect(),
            };
            if !options.contains(&path[l].as_str()) {
                path[l] = options.first().map(|s| s.to_string()).unwrap_or_default();
            }
        }
        if let Some(c) = standard::components(&path[0], &path[1], &path[2]).into_iter().find(|c| c.name == path[3]) {
            let mut spec = StandardSpec::new(c);
            // Keep the size when the new component has it, and the material.
            if c.size(&cur.size).is_some() {
                spec.set_size(&cur.size);
            }
            spec.material = cur.material;
            spec.finish = cur.finish;
            self.ui.std.spec = Some(spec);
        }
        cx.notify();
    }

    /// The Size select: a size the server can't build (live) shows its
    /// error toast and leaves everything else as it was.
    fn std_pick_size(&mut self, size: &str, cx: &mut Context<Self>) {
        let spec = self.std_spec();
        if let Some(msg) = standard::component(&spec).and_then(|c| c.failure(size)) {
            self.ui.std.failed_size = Some(size.to_string());
            self.ui.std.toasts.push(format!("Standard content component failed to solve. {msg}"));
            cx.notify();
            return;
        }
        self.std_edit_spec(cx, |s| s.set_size(size));
    }

    fn std_edit_spec(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut StandardSpec)) {
        self.ui.std.failed_size = None;
        let mut spec = self.std_spec();
        f(&mut spec);
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

    /// The camera direction toward the viewer in an occurrence's coordinates.
    fn std_toward(&self, o: &PartOccurrence, cx: &App) -> DVec3 {
        let forward = self.viewport.read(cx).camera.forward();
        o.transform.inverse().transform_vector3(-forward).normalize_or_zero()
    }

    /// Insert opened: the faces and edges selected before are what Insert
    /// fastens into and what enables the stack buttons (live: a selection
    /// made while the panel is open doesn't).
    pub(super) fn std_take_selection(&mut self, cx: &App) {
        let units = self.units(cx);
        let mut picks = Vec::new();
        for (path, reference) in self.ui.entity_sel.clone() {
            if !matches!(reference, Reference::Face(_) | Reference::Edge(_)) {
                continue;
            }
            let Some(o) = self.result.occurrences.iter().find(|o| o.path == path && (o.face_refs.contains(&reference) || o.edge_refs.contains(&reference))) else { continue };
            let toward = self.std_toward(o, cx);
            let holes = standard::hole_connectors(&o.context, Some(&o.body), &reference, toward, &units).iter().map(|(p, _)| p.connector(&path)).collect();
            let planar = standard::planar_face(&o.context, Some(&o.body), &reference);
            picks.push(StdPick { holes, planar });
        }
        self.ui.std.picks = picks;
    }

    /// Insert closest / furthest are enabled by a picked planar face with holes.
    fn std_stack_enabled(&self) -> bool {
        self.ui.std.picks.iter().any(|p| p.planar && !p.holes.is_empty())
    }

    /// Auto-size pick: the size fitting the picked hole (bolts and screws
    /// round down, nuts and washers up); the holes are kept so Insert
    /// fastens the part in them.
    pub(super) fn std_pick_hole(&mut self, t: AsmPickTarget, cx: &mut Context<Self>) {
        let Some(o) = self.result.occurrences.get(t.occ) else { return };
        let units = self.units(cx);
        let toward = self.std_toward(o, cx);
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
                self.std_edit_spec(cx, |s| s.set_size(size));
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
    fn std_insert(&mut self, mode: Stack, cx: &mut Context<Self>) {
        let spec = self.std_spec();
        // Live: a size the server can't build inserts nothing.
        if let Some(msg) = self.ui.std.failed_size.as_deref().and_then(|f| standard::component(&spec).and_then(|c| c.failure(f))) {
            self.ui.std.toasts.push(format!("Insertable standard content failed to generate. {msg}"));
            cx.notify();
            return;
        }
        // Replace instance's picker: the component becomes the replacement.
        if self.pick_replacement(InstanceSource::Standard { spec: spec.clone() }, spec.part_name(), cx) {
            return;
        }
        let mut holes = std::mem::take(&mut self.ui.std.holes);
        for p in std::mem::take(&mut self.ui.std.picks) {
            // Closest / furthest: the holes of the picked planar faces.
            if mode != Stack::Plain && !p.planar {
                continue;
            }
            for h in p.holes {
                if !holes.contains(&h) {
                    holes.push(h);
                }
            }
        }
        // Furthest onto a stack topped by a bolt is refused (live, behaviours
        // "Standard content: … stacking").
        if mode == Stack::Furthest && self.assembly(cx).is_some_and(|a| holes.iter().any(|h| stack_end(a, h).is_none())) {
            self.message = Some((true, "Component on top of existing stack cannot accept additional top stack components.".into()));
            cx.notify();
            return;
        }
        let flip = self.ui.std.flip;
        let base = spec.part_name();
        let ids = self.edit_insert(&format!("Insert : {base}"), cx, |a| {
            let mut ids = Vec::new();
            for _ in 0..holes.len().max(1) {
                let id = InstanceId(a.next_id());
                let name = a.next_instance_name(&base);
                a.instances.push(Instance::new(id, name, InstanceSource::Standard { spec: spec.clone() }, Placement::default()));
                ids.push(id);
            }
            for (id, hole) in ids.iter().zip(&holes) {
                let mut src = MateConnectorRef::on(*id, vec![], standard::connector_reference(0));
                src.flip = flip;
                let top = MateConnectorRef::on(*id, vec![], standard::connector_reference(1));
                let fastened = |a: &mut Assembly, x: MateConnectorRef, y: MateConnectorRef| {
                    let mid = MateId(a.next_id());
                    let n = a.next_feature_name("Fastened");
                    a.mates.push(Mate::new(mid, n, MateType::Fastened, x, y));
                };
                if mode == Stack::Furthest {
                    let at = stack_end(a, hole).unwrap_or_else(|| hole.clone());
                    fastened(a, src, at);
                    continue;
                }
                // Closest (and Insert of a washer / nut into a hole holding a
                // bolt): the part goes against the face. Live: the face's
                // Fastened is re-pointed from what sat there to the new part,
                // and a new Fastened joins the new part's top to it.
                let under = held(a, hole).filter(|(k, at_is_b)| {
                    let other = if *at_is_b { &a.mates[*k].a } else { &a.mates[*k].b };
                    stacks(a, *id) && (mode == Stack::Closest || !stacks(a, other.instance))
                });
                match under {
                    Some((k, at_is_b)) => {
                        let m = &mut a.mates[k];
                        let was = if at_is_b { std::mem::replace(&mut m.a, src) } else { std::mem::replace(&mut m.b, src) };
                        fastened(a, top, was);
                    }
                    None => fastened(a, src, hole.clone()),
                }
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

    /// Live's select: white, a 1 px `#d7d7d7` underline, 5 px in, a caret at the right.
    fn std_select(id: impl Into<SharedString>, w: f32, current: String, options: Vec<String>, disabled: bool, on_pick: impl Fn(&mut AssemblyView, &str, &mut Context<AssemblyView>) + 'static, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let id: SharedString = id.into();
        let on_pick = std::rc::Rc::new(on_pick);
        let field = div()
            .id(id.clone())
            .w(px(w))
            .h(px(26.))
            .flex_none()
            .flex()
            .items_center()
            .pl(px(5.))
            .pr(px(6.))
            // Live skb/800, 807: disabled selects are filled grey.
            .bg(gpui_kit::rgb(if disabled { 0xeeeeee } else { 0xffffff }))
            .border_b_1()
            .border_color(gpui_kit::rgb(0xd7d7d7))
            .text_size(px(12.))
            .text_color(if disabled { pal.text_faint } else { pal.text })
            .child(div().flex_1().min_w_0().truncate().child(current.clone()))
            .child(crate::os_ui::glyph(crate::os_ui::GLYPH_CARET, 8., if disabled { pal.text_faint } else { pal.text }));
        if disabled {
            return field.into_any_element();
        }
        let view = cx.entity();
        field
            .cursor_pointer()
            .os_dropdown_menu(SharedString::from(format!("{id}-dd")), true, move |mut menu, _, _| {
                for o in &options {
                    let v = view.clone();
                    let o2 = o.clone();
                    let pick = on_pick.clone();
                    menu = menu.item(crate::os_menu::OsMenuItem::new(o.clone()).checked(*o == current).on_click(move |_, _, cx| v.update(cx, |this, cx| pick(this, &o2, cx))));
                }
                menu
            })
            .into_any_element()
    }

    /// The Standard content tab body (also the Edit standard content dialog,
    /// where the Standard › Component pickers are greyed).
    pub(super) fn render_standard_content(&mut self, editing: bool, cx: &mut Context<Self>) -> AnyElement {
        let spec = self.std_spec();
        // The pencil's edit mode locks every selector (live skb/800).
        let props_edit = self.ui.std.props_edit.clone();
        let locked = props_edit.is_some();
        let props = std_props(&spec);
        // Live: rows of 12 px #333 labels in a 90 px column, 7 px in.
        let row = |name: &str, h: f32, field: AnyElement| h_flex().h(px(h)).flex_none().px(px(7.)).child(div().w(px(90.)).flex_none().text_size(px(12.)).text_color(gpui_kit::rgb(0x333333)).child(name.to_string())).child(field).into_any_element();
        let rule = || div().h(px(1.)).flex_none().bg(gpui_kit::rgb(0xd7d7d7)).into_any_element();
        // Group 1 (105 px, scrolls): Standard › Category › Class › Component, 181 px selects.
        let levels: [(&'static str, &'static str, String, Vec<String>); 4] = [
            ("std-standard", "Standard", spec.standard.clone(), standard::standards().into_iter().map(String::from).collect()),
            ("std-category", "Category", spec.category.clone(), standard::categories(&spec.standard).into_iter().map(String::from).collect()),
            ("std-class", "Class", spec.class.clone(), standard::classes(&spec.standard, &spec.category).into_iter().map(String::from).collect()),
            ("std-component", "Component", spec.component.clone(), standard::components(&spec.standard, &spec.category, &spec.class).iter().map(|c| c.name.to_string()).collect()),
        ];
        let pickers: [fn(&mut AssemblyView, &str, &mut Context<AssemblyView>); 4] = [|t, v, cx| t.std_choose(0, v, cx), |t, v, cx| t.std_choose(1, v, cx), |t, v, cx| t.std_choose(2, v, cx), |t, v, cx| t.std_choose(3, v, cx)];
        let mut hierarchy = v_flex().id("std-hierarchy").h(px(105.)).flex_none().overflow_y_scroll().track_scroll(&self.ui.std.hier_scroll).on_scroll_wheel(cx.listener(|_, _: &ScrollWheelEvent, _, cx| cx.notify()));
        for ((id, name, cur, options), pick) in levels.into_iter().zip(pickers) {
            hierarchy = hierarchy.child(row(name, 26.5, Self::std_select(id, 181., cur, options, editing || locked, pick, cx)));
        }
        // Live: 106 px of rows in the 105 px group, so it scrolls a little
        // and shows its scrollbar.
        hierarchy = hierarchy.child(div().h(px(4.)).flex_none());
        let mut body = v_flex().child(div().relative().child(hierarchy).child(Self::std_scrollbar("std-hier-sb", &self.ui.std.hier_scroll, 105., cx))).child(rule());
        let Some(c) = standard::component(&spec) else { return body.into_any_element() };
        // Group 2 (114 px, scrolls): the component's parameters, 146 px selects.
        let mut params = v_flex().id("std-params").h(px(114.)).flex_none().overflow_y_scroll().track_scroll(&self.ui.std.param_scroll).on_scroll_wheel(cx.listener(|_, _: &ScrollWheelEvent, _, cx| cx.notify()));
        let sizes: Vec<String> = c.size_names().into_iter().map(String::from).collect();
        let shown_size = self.ui.std.failed_size.clone().unwrap_or_else(|| spec.size.clone());
        let size_select = Self::std_select("std-size", 146., shown_size, sizes, locked, |t, v, cx| t.std_pick_size(v, cx), cx);
        // Live: a 26 px borderless icon button 9 px right of the select.
        let auto = div()
            .id("std-auto-size-btn")
            .size(px(26.))
            .ml(px(9.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(5.25))
            .cursor_pointer()
            .when(self.ui.std.auto_size, |d| d.bg(gpui_kit::rgb(0xd9e5f5)))
            .hover(|d| d.bg(gpui_kit::rgb(0xeeeeee)))
            .child(crate::assets::icon(&icon_path("auto-size"), 20., cx))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.std_toggle_auto_size(cx)));
        let auto = crate::ui::with_rich_tooltip("std-auto-size", "Auto-size fastener", "Set the standard content diameter to a selected hole. 1. Select Auto-size. 2. Select a circular edge or cylindrical face on a part.", auto);
        params = params.child(row(c.size_label(), 27., h_flex().child(size_select).when(!editing, |d| d.child(auto)).into_any_element()));
        // The component's own selects after Size (live: Drive type, Series,
        // Bearing face, Head slot, Head type…), then Length and Thread length.
        for p in &c.params {
            let label = p.label.clone();
            let value = spec.param(&p.label).to_string();
            let id = SharedString::from(format!("std-param-{}", p.label));
            let el = if p.checkbox {
                let on = value == standard::CHECKED;
                crate::os_ui::checkbox(id, "", on)
                    .pl(px(0.))
                    .when(locked, |d| d.opacity(0.5))
                    .when(!locked, |d| d.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.std_edit_spec(cx, |s| {
                        s.params.insert(label.clone(), if on { standard::UNCHECKED } else { standard::CHECKED }.into());
                    }))))
                    .into_any_element()
            } else {
                Self::std_select(id, 146., value, c.param_options(p, spec.size_row()).to_vec(), locked, move |t, v, cx| t.std_edit_spec(cx, |s| {
                    s.params.insert(label.clone(), v.to_string());
                    // Tapping screws: the Tip type / Threads/inch row has its own lengths.
                    s.fix_length();
                }), cx)
            };
            params = params.child(row(&p.label, 27., el));
        }
        if let Some(size) = spec.size_row()
            && c.shape.has_length()
        {
            let cur = spec.length.clone().unwrap_or_default();
            params = params.child(row(c.length_label(), 27., Self::std_select("std-length", 146., cur.clone(), c.lengths(size), locked, |t, v, cx| t.std_edit_spec(cx, |s| s.length = Some(v.to_string())), cx)));
            let threads = c.thread_lengths(size, &cur);
            if let Some(first) = threads.first().cloned() {
                let chosen = spec.params.get(standard::THREAD_LENGTH).filter(|t| threads.contains(*t)).cloned().unwrap_or(first);
                params = params.child(row("Thread length", 27., Self::std_select("std-thread", 146., chosen, threads, locked, |t, v, cx| t.std_edit_spec(cx, |s| {
                    s.params.insert(standard::THREAD_LENGTH.into(), v.to_string());
                }), cx)));
            }
        }
        let materials: Vec<String> = c.materials().iter().map(|s| s.to_string()).collect();
        params = params.child(row("Material", 27., Self::std_select("std-material", 146., spec.material().to_string(), materials, locked, |t, v, cx| t.std_edit_spec(cx, |s| s.material = Some(v.to_string())), cx)));
        let finishes: Vec<String> = c.finishes().iter().map(|s| s.to_string()).collect();
        params = params.child(row("Finish", 27., Self::std_select("std-finish", 146., spec.finish().to_string(), finishes, locked, |t, v, cx| t.std_edit_spec(cx, |s| s.finish = Some(v.to_string())), cx)));
        body = body.child(div().relative().child(params).child(Self::std_scrollbar("std-param-sb", &self.ui.std.param_scroll, 114., cx))).child(div().mt(px(5.)).child(rule()));
        // Part number / Description: read-only, 161 px, #eee on a #ccc underline.
        let readonly = |text: String| div().w(px(161.)).h(px(26.)).flex_none().flex().items_center().px(px(2.)).bg(gpui_kit::rgb(0xeeeeee)).border_b_1().border_color(gpui_kit::rgb(0xcccccc)).text_size(px(12.)).text_color(gpui_kit::rgb(0x666666)).overflow_hidden().whitespace_nowrap().child(text);
        let small_button = |id: &'static str| div()
            .id(id)
            .w(px(32.))
            .h(px(24.))
            .ml(px(3.))
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui_kit::rgb(0xffffff))
            .border_1()
            .border_color(gpui_kit::rgb(0xaaaaaa))
            .rounded(px(3.5))
            .cursor_pointer()
            .hover(|d| d.bg(gpui_kit::rgb(0xeeeeee)));
        body = body.child(div().h(px(6.)));
        if let Some((number, description)) = &props_edit {
            // Live skb/800: both fields editable (white), ⟲ resets the
            // Description, the note, then [Save] [Cancel].
            let field = |st: &Entity<InputState>| {
                gpui_kit::base::input::input_frame(div().w(px(161.)).h(px(26.)).flex_none().bg(gpui_kit::rgb(0xffffff)).border_b_1().border_color(gpui_kit::rgb(0xcccccc)).child(crate::os_ui::input_h(Input::new(st).appearance(false).text_size(px(12.)), 25.))).into_any_element()
            };
            let reset = small_button("std-description-reset").child(crate::assets::icon(&icon_path("reset-default"), 18., cx)).tooltip(crate::ui::tooltip("Reset to default")).on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                let d = this.std_spec().description();
                if let Some((_, st)) = &this.ui.std.props_edit {
                    st.update(cx, |st, cx| st.set_value(d, window, cx));
                }
            }));
            let btn = |id: &'static str, label: &'static str| div().id(id).h(px(34.)).px(px(12.)).flex().items_center().bg(gpui_kit::rgb(0xffffff)).border_1().border_color(gpui_kit::rgb(0xaaaaaa)).rounded(px(5.25)).text_size(px(14.)).text_color(gpui_kit::rgb(0x333333)).cursor_pointer().hover(|d| d.bg(gpui_kit::rgb(0xeeeeee))).child(label);
            body = body
                .child(row("Part number", 27., field(number)))
                .child(row("Description", 27., h_flex().child(field(description)).child(reset).into_any_element()))
                .child(div().mx(px(7.)).mt(px(6.)).py(px(6.)).flex().justify_center().bg(gpui_kit::rgb(0xd9edf7)).border_1().border_color(gpui_kit::rgb(0xbce8f1)).rounded(px(3.)).text_size(px(12.)).text_color(gpui_kit::rgb(0x1651b0)).child("Property changes apply company/user wide."))
                .child(
                    h_flex()
                        .justify_center()
                        .gap(px(4.))
                        .py(px(6.))
                        .child(btn("std-props-save", "Save").on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.std_props_done(true, cx))))
                        .child(btn("std-props-cancel", "Cancel").on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.std_props_done(false, cx)))),
                )
                .child(rule());
        } else {
            let pencil = small_button("std-part-number-edit")
                .child(crate::assets::icon(&icon_path("edit-pencil"), 20., cx))
                .tooltip(crate::ui::tooltip("Edit company/user wide part properties for this configuration"))
                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.std_props_open(window, cx)));
            body = body
                .child(row("Part number", 27., h_flex().child(readonly(props.part_number.clone())).child(pencil).into_any_element()))
                .child(row("Description", 27., readonly(props.description.clone().unwrap_or_else(|| spec.description())).into_any_element()))
                .child(div().mt(px(4.)).child(rule()));
        }
        // The component's line drawing (end view and side view), 242 x 105.
        // Live: the drawing is the component's, whatever Head slot says.
        let mut plain = spec.clone();
        plain.params.clear();
        let pv = Preview::of(c, &plain);
        body = body.child(div().pt(px(6.)).h(px(111.)).flex_none().flex().justify_center().child(canvas(|_, _, _| (), move |b, _, window, _| paint_fastener(b, pv, window)).w(px(242.)).h(px(105.))));
        if locked {
            // The edit mode's Save / Cancel stand in for the Insert group.
            return body.into_any_element();
        }
        let action = if editing {
            // Live 807: one white [Update] button under the preview.
            div()
                .id("std-update")
                .h(px(34.))
                .px(px(12.))
                .flex()
                .items_center()
                .bg(gpui_kit::rgb(0xffffff))
                .border_1()
                .border_color(gpui_kit::rgb(0xaaaaaa))
                .rounded(px(5.25))
                .text_size(px(14.))
                .text_color(gpui_kit::rgb(0x333333))
                .cursor_pointer()
                .hover(|d| d.bg(gpui_kit::rgb(0xeeeeee)))
                .child("Update")
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.std_update(cx)))
                .into_any_element()
        } else {
            self.std_insert_buttons(cx)
        };
        body = body.child(h_flex().justify_center().pb(px(5.)).child(action));
        body.into_any_element()
    }

    /// Live's always-visible thin scrollbar at a group's right (x 535 of the
    /// 300 px panel): ▲, a grey rounded thumb, ▼; nothing while the group
    /// doesn't scroll. Painted from the handle after layout (this frame's
    /// sizes), the arrows scroll a row.
    fn std_scrollbar(id: &'static str, handle: &ScrollHandle, h: f32, cx: &mut Context<Self>) -> AnyElement {
        const ARROW: f32 = 11.0;
        let step = |dy: f32, handle: ScrollHandle| {
            cx.listener(move |_, _: &ClickEvent, _, cx| {
                let y = (f32::from(handle.offset().y) - dy).clamp(-f32::from(handle.max_offset().y), 0.0);
                handle.set_offset(point(px(0.), px(y)));
                cx.notify();
            })
        };
        let paint = handle.clone();
        div()
            .id(id)
            .absolute()
            .top_0()
            .right(px(5.))
            .w(px(8.))
            .h(px(h))
            .child(
                canvas(
                    |_, _, _| (),
                    move |b, _, window, _| {
                        let max = f32::from(paint.max_offset().y);
                        if max < 0.5 {
                            return;
                        }
                        let grey = gpui_kit::rgb(0xa3a3a3);
                        let (x, y) = (b.origin.x, b.origin.y);
                        let tri = |window: &mut Window, top: f32, up: bool| {
                            let (a, c) = if up { (4.0, 0.0) } else { (0.0, 4.0) };
                            let mut pb = PathBuilder::fill();
                            pb.move_to(point(x + px(0.5), y + px(top + a)));
                            pb.line_to(point(x + px(7.5), y + px(top + a)));
                            pb.line_to(point(x + px(4.), y + px(top + c)));
                            pb.close();
                            if let Ok(path) = pb.build() {
                                window.paint_path(path, grey);
                            }
                        };
                        tri(window, 3.5, true);
                        tri(window, h - 7.5, false);
                        let track = h - 2.0 * ARROW;
                        let thumb = (track * h / (h + max)).max(16.0);
                        let offset = -f32::from(paint.offset().y);
                        let top = ARROW + (track - thumb) * (offset / max).clamp(0.0, 1.0);
                        window.paint_quad(fill(Bounds::new(point(x + px(1.), y + px(top)), size(px(6.), px(thumb))), gpui_kit::rgb(0xc1c1c1)).corner_radii(px(3.)));
                    },
                )
                .size_full(),
            )
            .child(div().id("up").absolute().top_0().left_0().w_full().h(px(ARROW)).on_click(step(-26.5, handle.clone())))
            .child(div().id("down").absolute().bottom_0().left_0().w_full().h(px(ARROW)).on_click(step(26.5, handle.clone())))
            .into_any_element()
    }

    /// Live's Insert button group (141 x 34): Insert | closest | furthest.
    fn std_insert_buttons(&self, cx: &mut Context<Self>) -> AnyElement {
        // The replacement picker has no selection to stack on.
        let stack_on = self.std_stack_enabled() && !self.picking_replacement();
        let cell = |id: &'static str, w: f32| div().id(id).w(px(w)).h(px(34.)).flex().items_center().justify_center().bg(gpui_kit::rgb(0xffffff)).border_1().border_color(gpui_kit::rgb(0xaaaaaa));
        let insert = cell("std-insert", 59.)
            .rounded_l(px(5.25))
            .text_size(px(14.))
            .text_color(gpui_kit::rgb(0x333333))
            .cursor_pointer()
            .hover(|d| d.bg(gpui_kit::rgb(0xeeeeee)))
            .child("Insert")
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.std_insert(Stack::Plain, cx)));
        let insert = crate::ui::with_rich_tooltip(
            "std-insert-tip",
            "Insert and drag to snap",
            "To insert standard content and add a fastened mate: - Pre-select edges or faces of holes and click 'Insert', or - Pre-select planar faces that contains holes or standard content and click 'Insert', or - Click 'Insert' and drag standard content into the model view and snap to a mate connecter at a desired location",
            insert,
        );
        let stack = |id: &'static str, icon: &'static str, mode: Stack, last: bool, cx: &mut Context<Self>| {
            cell(id, 42.)
                .ml(px(-1.))
                .when(last, |d| d.rounded_r(px(5.25)))
                .child(div().when(!stack_on, |d| d.opacity(0.4)).child(crate::assets::icon(&icon_path(icon), 20., cx)))
                .when(stack_on, |d| d.cursor_pointer().hover(|d| d.bg(gpui_kit::rgb(0xeeeeee))).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.std_insert(mode, cx))))
        };
        let closest = crate::ui::with_rich_tooltip("std-closest-tip", "Insert closest to selection", "Pre-select planar faces that contains holes or standard content and click 'Insert closest to selection'", stack("std-closest", "stack-closest", Stack::Closest, false, cx));
        let furthest = crate::ui::with_rich_tooltip("std-furthest-tip", "Insert furthest from selection", "Pre-select planar faces that contains holes or standard content and click 'Insert furthest from selection'", stack("std-furthest", "stack-furthest", Stack::Furthest, true, cx));
        h_flex().h(px(34.)).child(insert).child(closest).child(furthest).into_any_element()
    }

    /// Edit standard content (live 807): 300 px, ✓ ✕, the tab's layout
    /// with the hierarchy locked and [Update]; the help ? at the bottom.
    pub(super) fn render_standard_edit_panel(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let close = |this: &mut Self, cx: &mut Context<Self>| {
            this.ui.std.editing = None;
            this.ui.std.props_edit = None;
            cx.notify();
        };
        let head = h_flex()
            .h(px(29.))
            .pl(px(6.))
            .bg(gpui_kit::rgb(0xffffff))
            .border_b_1()
            .border_color(gpui_kit::rgb(0xd7d7d7))
            .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Edit standard content"))
            .child(div().id("std-edit-ok").size(px(28.)).flex().items_center().justify_center().bg(pal.accept).cursor_pointer().child(crate::assets::icon("dialog-ok", 24., cx)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.std_update(cx))))
            .child(div().id("std-edit-close").size(px(28.)).flex().items_center().justify_center().cursor_pointer().hover(|d| d.bg(pal.panel_hover)).child(crate::assets::icon("dialog-cancel", 24., cx)).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| close(this, cx))));
        v_flex()
            .id("std-edit-panel")
            .occlude()
            .w(px(300.))
            .bg(pal.dialog_bg)
            .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
            .text_color(pal.text)
            .child(head)
            .child(self.render_standard_content(true, cx))
            .child(h_flex().h(px(22.)).px(px(4.)).justify_end().child(super::props::help_dot()))
            .into_any_element()
    }

    /// The pencil: Part number and Description become editable in place.
    fn std_props_open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let spec = self.std_spec();
        let props = std_props(&spec);
        let number = cx.new(|cx| InputState::new(window, cx).default_value(props.part_number.clone()));
        let description = cx.new(|cx| InputState::new(window, cx).default_value(props.description.clone().unwrap_or_else(|| spec.description())));
        self.ui.std.props_edit = Some((number, description));
        cx.notify();
    }

    /// Save (stored user wide for this configuration) / Cancel.
    fn std_props_done(&mut self, save: bool, cx: &mut Context<Self>) {
        let Some((number, description)) = self.ui.std.props_edit.take() else { return };
        if save {
            let spec = self.std_spec();
            let d = description.read(cx).value().to_string();
            let props = StdProps { part_number: number.read(cx).value().trim().to_string(), description: (d != spec.description()).then_some(d) };
            save_std_props(&spec, props);
        }
        cx.notify();
    }
}

/// A polyline painter (points, closed).
type Stroke<'a> = &'a mut dyn FnMut(&[(f32, f32)], bool);

/// What the preview draws: the family and its forms (`fasteners.fs`).
#[derive(Clone, Copy)]
struct Preview {
    shape: Shape,
    head: i32,
    drive: i32,
    point: i32,
    form: i32,
    /// A dome rather than a flat top (button, pan, round heads).
    dome: bool,
}

impl Preview {
    fn of(c: &standard::Component, spec: &StandardSpec) -> Self {
        let mut form = c.form.clone();
        for p in &c.params {
            if let Some(over) = p.forms.get(spec.param(&p.label)) {
                form.extend(over.iter().map(|(k, v)| (k.clone(), *v)));
            }
        }
        let get = |k: &str| form.get(k).copied().unwrap_or(0.0) as i32;
        let dome = spec.size_row().is_some_and(|s| s.dims.get("rf").copied().unwrap_or(0.0) * 2.0 >= s.dims.get("dk").copied().unwrap_or(f64::MAX) * 0.9);
        Preview { shape: c.shape, head: get("head"), drive: get("drive"), point: get("point"), form: get("form"), dome }
    }
}

/// The component's line drawing in a 242 x 105 box, like live's preview
/// (assembly/70): an end view at the left and a side view at the right,
/// thin dark lines on white, drawn from the component's family and forms.
fn paint_fastener(b: Bounds<Pixels>, pv: Preview, window: &mut Window) {
    use std::f32::consts::{FRAC_PI_2, FRAC_PI_3, FRAC_PI_4, FRAC_PI_6, TAU};
    let o = b.origin;
    let at = |x: f32, y: f32| o + point(px(x), px(y));
    let color = gpui_kit::rgb(0x333333);
    let mut stroke = |pts: &[(f32, f32)], closed: bool| {
        let Some(&(x0, y0)) = pts.first() else { return };
        let mut pb = PathBuilder::stroke(px(1.));
        pb.move_to(at(x0, y0));
        for &(x, y) in &pts[1..] {
            pb.line_to(at(x, y));
        }
        if closed {
            pb.line_to(at(x0, y0));
        }
        if let Ok(path) = pb.build() {
            window.paint_path(path, color);
        }
    };
    let arc = |cx: f32, cy: f32, r: f32, a0: f32, a1: f32| -> Vec<(f32, f32)> {
        let n = (((a1 - a0).abs() / TAU) * 72.0).ceil().max(2.0) as usize;
        (0..=n).map(|i| a0 + (a1 - a0) * i as f32 / n as f32).map(|a| (cx + r * a.cos(), cy + r * a.sin())).collect()
    };
    let polygon = |cx: f32, cy: f32, r: f32, n: usize, start: f32| -> Vec<(f32, f32)> { (0..n).map(|k| start + k as f32 * TAU / n as f32).map(|a| (cx + r * a.cos(), cy + r * a.sin())).collect() };
    let circle = |cx: f32, cy: f32, r: f32| arc(cx, cy, r, 0.0, TAU);
    let (ex, ey, r) = (54.0f32, 52.0f32, 32.0f32);
    // Script forms (fasteners.fs).
    let (hex_head, square_head, round_head, countersunk, raised) = (pv.head == 1, pv.head == 2, pv.head == 3, pv.head == 4 || pv.head == 5, pv.head == 5);
    let drive = |stroke: Stroke, rr: f32| match pv.drive {
        1 => stroke(&polygon(ex, ey, rr * 0.42, 6, FRAC_PI_6), true),
        2 => {
            let (w, f) = (rr * 0.14, rr * 0.98);
            stroke(&[(ex - w, ey - f), (ex + w, ey - f), (ex + w, ey + f), (ex - w, ey + f)], true);
        }
        3 | 4 => {
            let (w, f) = (rr * 0.1, rr * 0.4);
            stroke(&[(ex - w, ey - f), (ex + w, ey - f), (ex + w, ey - w), (ex + f, ey - w), (ex + f, ey + w), (ex + w, ey + w), (ex + w, ey + f), (ex - w, ey + f), (ex - w, ey + w), (ex - f, ey + w), (ex - f, ey - w), (ex - w, ey - w)], true);
            if pv.drive == 4 {
                for k in 0..4 {
                    let a = FRAC_PI_4 + k as f32 * FRAC_PI_2;
                    stroke(&[(ex + w * 1.5 * a.cos(), ey + w * 1.5 * a.sin()), (ex + f * 0.7 * a.cos(), ey + f * 0.7 * a.sin())], false);
                }
            }
        }
        5 => {
            let (ra, rb) = (rr * 0.42, rr * 0.3);
            let mut pts = Vec::new();
            for k in 0..6 {
                let a = k as f32 * FRAC_PI_3;
                pts.push((ex + rb * (a - FRAC_PI_6).cos(), ey + rb * (a - FRAC_PI_6).sin()));
                pts.push((ex + ra * a.cos(), ey + ra * a.sin()));
            }
            stroke(&pts, true);
        }
        _ => {}
    };
    // End view.
    match pv.shape {
        Shape::Screw if hex_head || square_head => {
            let n = if hex_head { 6 } else { 4 };
            stroke(&polygon(ex, ey, r, n, if hex_head { 0.0 } else { FRAC_PI_4 }), true);
            stroke(&circle(ex, ey, r * if hex_head { 0.866 } else { 0.707 }), false);
            drive(&mut stroke, r);
        }
        Shape::Screw if pv.head == 0 => {
            stroke(&circle(ex, ey, r * 0.6), false);
            drive(&mut stroke, r * 0.6);
        }
        Shape::Screw => {
            stroke(&circle(ex, ey, r), false);
            drive(&mut stroke, r);
        }
        Shape::Nut => {
            let n = if pv.form == 1 || pv.form == 6 { 4 } else { 6 };
            stroke(&polygon(ex, ey, r, n, if n == 6 { 0.0 } else { FRAC_PI_4 }), true);
            stroke(&circle(ex, ey, r * if n == 6 { 0.866 } else { 0.707 }), false);
            stroke(&circle(ex, ey, r * 0.45), false);
        }
        Shape::Washer => {
            if pv.form == 7 {
                stroke(&polygon(ex, ey, r, 4, FRAC_PI_4), true);
            } else {
                stroke(&circle(ex, ey, r), false);
            }
            stroke(&circle(ex, ey, r * 0.5), false);
        }
        Shape::Pin => stroke(&circle(ex, ey, r * 0.6), false),
        Shape::Key => stroke(&[(ex - r * 0.5, ey - r * 0.5), (ex + r * 0.5, ey - r * 0.5), (ex + r * 0.5, ey + r * 0.5), (ex - r * 0.5, ey + r * 0.5)], true),
        Shape::RetainingRing => {
            stroke(&arc(ex, ey, r, 0.35, TAU - 0.35), false);
            stroke(&arc(ex, ey, r * 0.75, 0.45, TAU - 0.45), false);
        }
        Shape::ORing => {
            stroke(&circle(ex, ey, r), false);
            stroke(&circle(ex, ey, r * 0.78), false);
        }
    }
    // Side view: the head at the left (x 110), the shank to the right.
    let (sx, end) = (110.0f32, 216.0f32);
    let point = pv.point;
    let shank = |stroke: Stroke, x0: f32, hr: f32| {
        match point {
            1 | 4 => {
                let tip = if point == 4 { 0.0 } else { hr * 0.35 };
                stroke(&[(x0, ey - hr), (end - hr * 0.6, ey - hr), (end, ey - tip), (end, ey + tip), (end - hr * 0.6, ey + hr), (x0, ey + hr)], false);
            }
            3 => {
                let dog = hr * 0.6;
                stroke(&[(x0, ey - hr), (end - 18.0, ey - hr), (end - 18.0 + hr - dog, ey - dog), (end, ey - dog), (end, ey + dog), (end - 18.0 + hr - dog, ey + dog), (end - 18.0, ey + hr), (x0, ey + hr)], false);
            }
            _ => {
                stroke(&[(x0, ey - hr), (end, ey - hr), (end + 3.0, ey - hr + 3.0), (end + 3.0, ey + hr - 3.0), (end, ey + hr), (x0, ey + hr)], false);
                stroke(&[(end, ey - hr), (end, ey + hr)], false);
            }
        }
    };
    match pv.shape {
        Shape::Screw if countersunk => {
            let (hr, x1) = (17.5f32, sx + 18.0);
            stroke(&[(x1, ey - hr), (sx, ey - r), (sx, ey + r), (x1, ey + hr)], false);
            stroke(&[(x1, ey - hr), (x1, ey + hr)], false);
            if raised {
                let pts: Vec<(f32, f32)> = (0..=24).map(|i| -1.0 + 2.0 * i as f32 / 24.0).map(|t| (sx - 6.0 * (1.0 - t * t), ey + r * t)).collect();
                stroke(&pts, false);
            }
            shank(&mut stroke, x1, hr);
        }
        Shape::Screw if hex_head || square_head => {
            let (hr, x1) = (14.0f32, sx + 22.0);
            stroke(&[(sx, ey - r), (x1, ey - r), (x1, ey + r), (sx, ey + r)], true);
            if hex_head {
                stroke(&[(sx, ey - r * 0.5), (x1, ey - r * 0.5)], false);
                stroke(&[(sx, ey + r * 0.5), (x1, ey + r * 0.5)], false);
            }
            shank(&mut stroke, x1, hr);
        }
        Shape::Screw if round_head || pv.head > 5 => {
            let (hr, x1) = (14.0f32, sx + if pv.dome { 16.0 } else { 26.0 });
            if pv.dome {
                // The dome: an arc through the head's top corners.
                let rr = (r * r + 16.0 * 16.0) / 32.0;
                let a = (r / rr).asin();
                let pts: Vec<(f32, f32)> = (0..=24).map(|i| -a + 2.0 * a * i as f32 / 24.0).map(|t| (x1 - rr * t.cos() + (rr - 16.0), ey + rr * t.sin())).collect();
                stroke(&pts, false);
                stroke(&[(x1, ey - r), (x1, ey + r)], false);
            } else {
                stroke(&[(sx, ey - r), (x1, ey - r), (x1, ey + r), (sx, ey + r)], true);
            }
            shank(&mut stroke, x1, hr);
        }
        Shape::Screw | Shape::Pin => {
            let hr = 19.0f32;
            stroke(&[(sx + 3.0, ey - hr), (sx, ey - hr + 3.0), (sx, ey + hr - 3.0), (sx + 3.0, ey + hr)], false);
            stroke(&[(sx + 3.0, ey - hr), (sx + 3.0, ey + hr)], false);
            shank(&mut stroke, sx + 3.0, hr);
        }
        Shape::Nut => {
            let x1 = sx + 30.0;
            stroke(&[(sx, ey - r), (x1, ey - r), (x1, ey + r), (sx, ey + r)], true);
            if pv.form != 1 && pv.form != 6 {
                stroke(&[(sx, ey - r * 0.5), (x1, ey - r * 0.5)], false);
                stroke(&[(sx, ey + r * 0.5), (x1, ey + r * 0.5)], false);
            }
        }
        Shape::Washer | Shape::RetainingRing => {
            let x1 = sx + 8.0;
            stroke(&[(sx, ey - r), (x1, ey - r), (x1, ey + r), (sx, ey + r)], true);
        }
        Shape::Key => {
            let (x1, h) = (end, 16.0f32);
            stroke(&[(sx + h, ey - h), (x1 - h, ey - h)], false);
            stroke(&[(sx + h, ey + h), (x1 - h, ey + h)], false);
            stroke(&arc(sx + h, ey, h, FRAC_PI_2, 3.0 * FRAC_PI_2), false);
            stroke(&arc(x1 - h, ey, h, -FRAC_PI_2, FRAC_PI_2), false);
        }
        Shape::ORing => {
            stroke(&circle(sx + 8.0, ey - r + 8.0, 8.0), false);
            stroke(&circle(sx + 8.0, ey + r - 8.0, 8.0), false);
            stroke(&[(sx + 8.0, ey - r), (sx + 8.0, ey - r + 16.0)], false);
        }
    }
}

/// Nuts and washers stack: they have a second connector on their top face.
fn stacks(a: &Assembly, id: InstanceId) -> bool {
    a.instance(id).and_then(|i| i.source.standard()).and_then(standard::component).is_some_and(|c| !c.shape.has_length())
}

fn same_connector(c: &MateConnectorRef, at: &MateConnectorRef) -> bool {
    c.instance == at.instance && c.path == at.path && c.reference == at.reference && c.align == at.align
}

/// The fastened mate on connector `at`: its index and whether `at` is its `b`.
fn held(a: &Assembly, at: &MateConnectorRef) -> Option<(usize, bool)> {
    a.mates.iter().position(|m| !m.suppressed && m.mate_type == MateType::Fastened && (same_connector(&m.b, at) || same_connector(&m.a, at))).map(|k| (k, same_connector(&a.mates[k].b, at)))
}

/// Insert furthest from selection: the free connector at the outer end of
/// the stack on `hole` (past every nut and washer); None when a bolt or
/// screw tops it.
fn stack_end(a: &Assembly, hole: &MateConnectorRef) -> Option<MateConnectorRef> {
    let mut at = hole.clone();
    for _ in 0..32 {
        let Some((k, at_is_b)) = held(a, &at) else { return Some(at) };
        let other = if at_is_b { a.mates[k].a.clone() } else { a.mates[k].b.clone() };
        if !stacks(a, other.instance) {
            return None;
        }
        let next = if other.reference == standard::connector_reference(0) { 1 } else { 0 };
        at = MateConnectorRef::on(other.instance, other.path.clone(), standard::connector_reference(next));
    }
    Some(at)
}

/// Company/user wide properties of a standard content configuration (the
/// Part number pencil, live skb/800): kept in the config directory for the
/// user, keyed by the full configuration.
#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
pub(crate) struct StdProps {
    #[serde(default)]
    pub part_number: String,
    /// None = the generated description.
    #[serde(default)]
    pub description: Option<String>,
}

fn std_props_path() -> std::path::PathBuf {
    crate::files::config_dir().join("standard_content_properties.json")
}

/// The configuration's key: the spec with its defaults spelt out.
fn std_props_key(spec: &StandardSpec) -> String {
    let mut s = spec.clone();
    s.material = Some(spec.material().to_string());
    s.finish = Some(spec.finish().to_string());
    if let Some(c) = standard::component(spec) {
        for p in &c.params {
            s.params.insert(p.label.clone(), spec.param(&p.label).to_string());
        }
    }
    serde_json::to_string(&s).unwrap_or_default()
}

fn load_std_props() -> std::collections::BTreeMap<String, StdProps> {
    let saved: std::collections::BTreeMap<String, StdProps> = std::fs::read_to_string(std_props_path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    // Keys saved before the generic parameters (head_slot, series…) read
    // again as specs and keyed the current way.
    saved.into_iter().map(|(k, v)| (serde_json::from_str::<StandardSpec>(&k).map(|s| std_props_key(&s)).unwrap_or(k), v)).collect()
}

pub(crate) fn std_props(spec: &StandardSpec) -> StdProps {
    load_std_props().remove(&std_props_key(spec)).unwrap_or_default()
}

fn save_std_props(spec: &StandardSpec, props: StdProps) {
    let mut all = load_std_props();
    let key = std_props_key(spec);
    if props.part_number.is_empty() && props.description.is_none() {
        all.remove(&key);
    } else {
        all.insert(key, props);
    }
    let p = std_props_path();
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(t) = serde_json::to_string_pretty(&all) {
        let _ = std::fs::write(p, t);
    }
}
