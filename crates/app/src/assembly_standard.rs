//! Standard content (child module of `assembly_view`): the Insert dialog's
//! Standard content tab (live assembly/70, 900–902): Standard › Category ›
//! Class › Component, then the component's parameters (Size with auto-size,
//! Head slot, Length, Thread length, Material, Finish) in two scroll groups,
//! Part number (edit pencil) and Description, a line drawing of the
//! component, and Insert | Insert closest to selection | Insert furthest
//! from selection. Inserting snaps to a hole's mate connector with a
//! Fastened mate (`a` flips the part); Edit standard content instance.

use super::*;
use noshape_model::assembly::eval::{InstancePath, PartOccurrence};
use noshape_model::assembly::standard::{self, Shape, StandardSpec};

/// A face or edge picked while the tab is open (live: pre-select hole
/// edges or faces, or planar faces with holes, then Insert).
#[derive(Clone)]
pub(crate) struct StdPick {
    pub path: InstancePath,
    pub reference: Reference,
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
    /// Faces and edges picked in the graphics area while the tab is open.
    pub picks: Vec<StdPick>,
    /// Edit standard content instance: the instances being edited.
    pub editing: Option<Vec<InstanceId>>,
    /// `a` while inserting: the part is flipped on its mate connector.
    pub flip: bool,
    /// The two scroll groups (hierarchy, parameters).
    pub hier_scroll: ScrollHandle,
    pub param_scroll: ScrollHandle,
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

    fn std_edit_spec(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut StandardSpec)) {
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

    /// The Standard content tab is shown: graphics clicks pick faces and edges.
    pub(super) fn std_picking(&self) -> bool {
        self.insert_open && self.ui.insert_source == 2 && !self.ui.std.auto_size && self.placing.is_none()
    }

    /// The picks drawn selected while the tab is shown.
    pub(super) fn std_picked_entities(&self) -> Vec<(InstancePath, Reference)> {
        if !self.std_picking() {
            return Vec::new();
        }
        self.ui.std.picks.iter().map(|p| (p.path.clone(), p.reference.clone())).collect()
    }

    /// The camera direction toward the viewer in an occurrence's coordinates.
    fn std_toward(&self, o: &PartOccurrence, cx: &App) -> DVec3 {
        let forward = self.viewport.read(cx).camera.forward();
        o.transform.inverse().transform_vector3(-forward).normalize_or_zero()
    }

    /// A click on a face or edge while the tab is shown: picked (again: dropped).
    pub(super) fn std_pick(&mut self, t: AsmPickTarget, cx: &mut Context<Self>) {
        if !matches!(t.kind, PickKind::Face | PickKind::Edge) {
            return;
        }
        if let Some(i) = self.ui.std.picks.iter().position(|p| p.path == t.path && p.reference == t.reference) {
            self.ui.std.picks.remove(i);
        } else {
            let Some(o) = self.result.occurrences.get(t.occ) else { return };
            let units = self.units(cx);
            let toward = self.std_toward(o, cx);
            let holes = standard::hole_connectors(&o.context, Some(&o.body), &t.reference, toward, &units).iter().map(|(p, _)| p.connector(&t.path)).collect();
            let planar = standard::planar_face(&o.context, Some(&o.body), &t.reference);
            self.ui.std.picks.push(StdPick { path: t.path.clone(), reference: t.reference.clone(), holes, planar });
        }
        self.refresh_scene(false, cx);
        cx.notify();
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
            // Nuts and washers have a second connector on their top face.
            let stacks = |a: &Assembly, id: InstanceId| a.instance(id).and_then(|i| i.source.standard()).and_then(standard::component).is_some_and(|c| !c.shape.has_length());
            let same = |c: &MateConnectorRef, at: &MateConnectorRef| c.instance == at.instance && c.path == at.path && c.reference == at.reference && c.align == at.align;
            // The fastened mate on connector `at` (its index, and whether `at` is its `b` side).
            let held = |a: &Assembly, at: &MateConnectorRef| a.mates.iter().position(|m| !m.suppressed && m.mate_type == MateType::Fastened && (same(&m.b, at) || same(&m.a, at))).map(|k| (k, same(&a.mates[k].b, at)));
            for (id, hole) in ids.iter().zip(&holes) {
                let mut src = MateConnectorRef::on(*id, vec![], standard::connector_reference(0));
                src.flip = flip;
                let top = MateConnectorRef::on(*id, vec![], standard::connector_reference(1));
                let new_stacks = stacks(a, *id);
                let mut at = hole.clone();
                if mode == Stack::Furthest {
                    // Walk out along the stack: past every nut and washer, to
                    // the free face of the last one (or under a bolt's head).
                    for _ in 0..32 {
                        let Some((k, at_is_b)) = held(a, &at) else { break };
                        let other = if at_is_b { a.mates[k].a.clone() } else { a.mates[k].b.clone() };
                        if !stacks(a, other.instance) {
                            break;
                        }
                        let next = if other.reference == standard::connector_reference(0) { 1 } else { 0 };
                        at = MateConnectorRef::on(other.instance, other.path.clone(), standard::connector_reference(next));
                    }
                }
                // What sits where the new part goes moves onto its top face:
                // closest / furthest anything, Insert a bolt (Onshape puts a
                // washer or nut under the bolt head).
                if new_stacks && let Some((k, at_is_b)) = held(a, &at) {
                    let other = if at_is_b { &a.mates[k].a } else { &a.mates[k].b };
                    let bolt = !stacks(a, other.instance);
                    if mode != Stack::Plain || bolt {
                        let m = &mut a.mates[k];
                        if at_is_b {
                            m.b = top.clone();
                        } else {
                            m.a = top.clone();
                        }
                    }
                }
                let mid = MateId(a.next_id());
                let n = a.next_feature_name("Fastened");
                a.mates.push(Mate::new(mid, n, MateType::Fastened, src, at));
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
    fn std_select(id: &'static str, w: f32, current: String, options: Vec<String>, disabled: bool, on_pick: fn(&mut AssemblyView, &str, &mut Context<AssemblyView>), cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let field = div()
            .id(id)
            .w(px(w))
            .h(px(26.))
            .flex_none()
            .flex()
            .items_center()
            .pl(px(5.))
            .pr(px(6.))
            .bg(gpui_kit::rgb(0xffffff))
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
                    menu = menu.item(crate::os_menu::OsMenuItem::new(o.clone()).checked(*o == current).on_click(move |_, _, cx| v.update(cx, |this, cx| on_pick(this, &o2, cx))));
                }
                menu
            })
            .into_any_element()
    }

    /// The Standard content tab body (also the Edit standard content dialog,
    /// where the Standard › Component pickers are greyed).
    pub(super) fn render_standard_content(&mut self, editing: bool, cx: &mut Context<Self>) -> AnyElement {
        let spec = self.std_spec();
        // Live: rows of 12 px #333 labels in a 90 px column, 7 px in.
        let row = |name: &'static str, h: f32, field: AnyElement| h_flex().h(px(h)).flex_none().px(px(7.)).child(div().w(px(90.)).flex_none().text_size(px(12.)).text_color(gpui_kit::rgb(0x333333)).child(name)).child(field).into_any_element();
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
            hierarchy = hierarchy.child(row(name, 26.5, Self::std_select(id, 181., cur, options, editing, pick, cx)));
        }
        let mut body = v_flex().child(div().relative().child(hierarchy).children(Self::std_scrollbar("std-hier-sb", &self.ui.std.hier_scroll, 105., cx))).child(rule());
        let Some(c) = standard::component(&spec) else { return body.into_any_element() };
        // Group 2 (114 px, scrolls): the component's parameters, 146 px selects.
        let mut params = v_flex().id("std-params").h(px(114.)).flex_none().overflow_y_scroll().track_scroll(&self.ui.std.param_scroll).on_scroll_wheel(cx.listener(|_, _: &ScrollWheelEvent, _, cx| cx.notify()));
        let sizes: Vec<String> = c.sizes.iter().map(|s| s.name.to_string()).collect();
        let size_select = Self::std_select("std-size", 146., spec.size.clone(), sizes, false, |t, v, cx| t.std_edit_spec(cx, |s| s.set_size(v)), cx);
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
        params = params.child(row("Size", 27., h_flex().child(size_select).when(!editing, |d| d.child(auto)).into_any_element()));
        if c.head_slot {
            let on = spec.slotted();
            params = params.child(row(
                "Head slot",
                27.,
                crate::os_ui::checkbox("std-head-slot", "", on).pl(px(0.)).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.std_edit_spec(cx, |s| s.head_slot = Some(!on)))).into_any_element(),
            ));
        }
        if let Some(size) = c.size(&spec.size)
            && c.shape.has_length()
        {
            let cur = spec.length.clone().unwrap_or_default();
            params = params.child(row("Length", 27., Self::std_select("std-length", 146., cur.clone(), c.lengths(size), false, |t, v, cx| t.std_edit_spec(cx, |s| s.length = Some(v.to_string())), cx)));
            if let Some(t) = c.thread_length(size, &cur) {
                params = params.child(row("Thread length", 27., Self::std_select("std-thread", 146., t.clone(), vec![t], false, |_, _, _| {}, cx)));
            }
        }
        let materials: Vec<String> = standard::MATERIALS.iter().map(|s| s.to_string()).collect();
        params = params.child(row("Material", 27., Self::std_select("std-material", 146., spec.material().to_string(), materials, false, |t, v, cx| t.std_edit_spec(cx, |s| s.material = Some(v.to_string())), cx)));
        let finishes: Vec<String> = standard::FINISHES.iter().map(|s| s.to_string()).collect();
        params = params.child(row("Finish", 27., Self::std_select("std-finish", 146., spec.finish().to_string(), finishes, false, |t, v, cx| t.std_edit_spec(cx, |s| s.finish = Some(v.to_string())), cx)));
        body = body.child(div().relative().child(params).children(Self::std_scrollbar("std-param-sb", &self.ui.std.param_scroll, 114., cx))).child(div().mt(px(5.)).child(rule()));
        // Part number / Description: read-only, 161 px, #eee on a #ccc underline.
        let readonly = |text: String| div().w(px(161.)).h(px(26.)).flex_none().flex().items_center().px(px(2.)).bg(gpui_kit::rgb(0xeeeeee)).border_b_1().border_color(gpui_kit::rgb(0xcccccc)).text_size(px(12.)).text_color(gpui_kit::rgb(0x666666)).overflow_hidden().whitespace_nowrap().child(text);
        let pencil = div()
            .id("std-part-number-edit")
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
            .child(crate::assets::icon(&icon_path("edit-pencil"), 20., cx))
            .tooltip(crate::ui::tooltip("Edit company/user wide part properties for this configuration"));
        body = body
            .child(div().h(px(6.)))
            .child(row("Part number", 27., h_flex().child(readonly(String::new())).child(pencil).into_any_element()))
            .child(row("Description", 27., readonly(spec.description()).into_any_element()))
            .child(div().mt(px(4.)).child(rule()));
        // The component's line drawing (end view and side view), 242 x 105.
        let (shape, slotted) = (c.shape, spec.slotted());
        body = body.child(div().pt(px(6.)).h(px(111.)).flex_none().flex().justify_center().child(canvas(|_, _, _| (), move |b, _, window, _| paint_fastener(b, shape, slotted, window)).w(px(242.)).h(px(105.))));
        let action = if editing {
            Button::new("std-update").label("Update").outline().h(px(30.)).px(px(10.)).rounded(px(3.)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.std_update(cx))).into_any_element()
        } else {
            self.std_insert_buttons(cx)
        };
        body = body.child(h_flex().justify_center().pb(px(5.)).child(action));
        body.into_any_element()
    }

    /// Live's always-visible thin scrollbar at a group's right (x 535 of the
    /// 300 px panel): ▲, a grey rounded thumb, ▼. None while nothing scrolls.
    fn std_scrollbar(id: &'static str, handle: &ScrollHandle, h: f32, cx: &mut Context<Self>) -> Option<AnyElement> {
        let max = f32::from(handle.max_offset().y);
        if max < 0.5 {
            return None;
        }
        let offset = -f32::from(handle.offset().y);
        let arrow = 11.0;
        let track = h - 2.0 * arrow;
        let thumb = (track * h / (h + max)).max(16.0);
        let top = arrow + (track - thumb) * (offset / max).clamp(0.0, 1.0);
        let step = |dy: f32, handle: ScrollHandle| {
            cx.listener(move |_, _: &ClickEvent, _, cx| {
                let y = (f32::from(handle.offset().y) - dy).clamp(-f32::from(handle.max_offset().y), 0.0);
                handle.set_offset(point(px(0.), px(y)));
                cx.notify();
            })
        };
        let tri = |up: bool| {
            canvas(
                |_, _, _| (),
                move |b, _, window, _| {
                    let (x, y) = (b.origin.x, b.origin.y);
                    let (a, c) = if up { (px(4.), px(0.)) } else { (px(0.), px(4.)) };
                    let mut pb = PathBuilder::fill();
                    pb.move_to(point(x, y + a));
                    pb.line_to(point(x + px(7.), y + a));
                    pb.line_to(point(x + px(3.5), y + c));
                    pb.close();
                    if let Ok(path) = pb.build() {
                        window.paint_path(path, gpui_kit::rgb(0xa3a3a3));
                    }
                },
            )
            .w(px(7.))
            .h(px(4.))
        };
        Some(
            div()
                .id(id)
                .absolute()
                .top_0()
                .right(px(5.))
                .w(px(8.))
                .h(px(h))
                .child(div().id("up").absolute().top_0().h(px(arrow)).w_full().flex().items_center().justify_center().child(tri(true)).on_click(step(-26.5, handle.clone())))
                .child(div().absolute().top(px(top)).left(px(1.)).w(px(6.)).h(px(thumb)).rounded(px(3.)).bg(gpui_kit::rgb(0xc1c1c1)))
                .child(div().id("down").absolute().bottom_0().h(px(arrow)).w_full().flex().items_center().justify_center().child(tri(false)).on_click(step(26.5, handle.clone())))
                .into_any_element(),
        )
    }

    /// Live's Insert button group (141 x 34): Insert | closest | furthest.
    fn std_insert_buttons(&self, cx: &mut Context<Self>) -> AnyElement {
        let stack_on = self.std_stack_enabled();
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

/// A polyline painter (points, closed).
type Stroke<'a> = &'a mut dyn FnMut(&[(f32, f32)], bool);

/// The component's line drawing in a 242 x 105 box, like live's preview
/// (assembly/70): an end view at the left and a side view at the right,
/// thin dark lines on white. Generic proportions per shape, not to scale.
fn paint_fastener(b: Bounds<Pixels>, shape: Shape, slotted: bool, window: &mut Window) {
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
        let n = (((a1 - a0).abs() / std::f32::consts::TAU) * 72.0).ceil().max(2.0) as usize;
        (0..=n).map(|i| a0 + (a1 - a0) * i as f32 / n as f32).map(|a| (cx + r * a.cos(), cy + r * a.sin())).collect()
    };
    let hexagon = |cx: f32, cy: f32, r: f32, flat_top: bool| -> Vec<(f32, f32)> {
        let start = if flat_top { 0.0 } else { std::f32::consts::FRAC_PI_6 };
        (0..6).map(|k| start + k as f32 * std::f32::consts::FRAC_PI_3).map(|a| (cx + r * a.cos(), cy + r * a.sin())).collect()
    };
    let (ex, ey, r) = (54.0f32, 52.0f32, 32.0f32);
    // End view.
    match shape {
        Shape::Countersunk if slotted => {
            // A circle split by the slot; the slot's floor between.
            let w = 4.5f32;
            let t = (w / r).asin();
            let mut left = arc(ex, ey, r, std::f32::consts::FRAC_PI_2 + t, 3.0 * std::f32::consts::FRAC_PI_2 - t);
            left.push(left[0]);
            stroke(&left, false);
            let mut right = arc(ex, ey, r, -std::f32::consts::FRAC_PI_2 + t, std::f32::consts::FRAC_PI_2 - t);
            right.push(right[0]);
            stroke(&right, false);
            let f = r * 0.72;
            stroke(&[(ex - w + 1.0, ey - f), (ex + w - 1.0, ey - f), (ex + w - 1.0, ey + f), (ex - w + 1.0, ey + f)], true);
        }
        Shape::HexBolt | Shape::HexNut => {
            stroke(&hexagon(ex, ey, r, true), true);
            stroke(&arc(ex, ey, r * 0.866, 0.0, std::f32::consts::TAU), false);
            if shape == Shape::HexNut {
                stroke(&arc(ex, ey, r * 0.45, 0.0, std::f32::consts::TAU), false);
            }
        }
        Shape::Washer => {
            stroke(&arc(ex, ey, r, 0.0, std::f32::consts::TAU), false);
            stroke(&arc(ex, ey, r * 0.5, 0.0, std::f32::consts::TAU), false);
        }
        Shape::Pin => stroke(&arc(ex, ey, r * 0.6, 0.0, std::f32::consts::TAU), false),
        Shape::SetScrew => {
            stroke(&arc(ex, ey, r * 0.6, 0.0, std::f32::consts::TAU), false);
            stroke(&hexagon(ex, ey, r * 0.3, false), true);
        }
        _ => {
            stroke(&arc(ex, ey, r, 0.0, std::f32::consts::TAU), false);
            stroke(&hexagon(ex, ey, r * 0.42, false), true);
        }
    }
    // Side view: the head at the left (x 110), the shank to the right.
    let (sx, end) = (110.0f32, 216.0f32);
    let shank = |stroke: Stroke, x0: f32, hr: f32| {
        stroke(&[(x0, ey - hr), (end, ey - hr), (end + 3.0, ey - hr + 3.0), (end + 3.0, ey + hr - 3.0), (end, ey + hr), (x0, ey + hr)], false);
        stroke(&[(end, ey - hr), (end, ey + hr)], false);
    };
    match shape {
        Shape::Countersunk => {
            let (hr, x1) = (17.5f32, sx + 18.0);
            stroke(&[(x1, ey - hr), (sx, ey - r), (sx, ey + r), (x1, ey + hr)], false);
            stroke(&[(x1, ey - hr), (x1, ey + hr)], false);
            shank(&mut stroke, x1, hr);
        }
        Shape::HexBolt => {
            let (hr, x1) = (14.0f32, sx + 22.0);
            stroke(&[(sx, ey - r), (x1, ey - r), (x1, ey + r), (sx, ey + r)], true);
            stroke(&[(sx, ey - r * 0.5), (x1, ey - r * 0.5)], false);
            stroke(&[(sx, ey + r * 0.5), (x1, ey + r * 0.5)], false);
            shank(&mut stroke, x1, hr);
        }
        Shape::SocketHead | Shape::ButtonHead => {
            let (hr, x1) = (14.0f32, sx + if shape == Shape::ButtonHead { 16.0 } else { 26.0 });
            if shape == Shape::ButtonHead {
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
        Shape::SetScrew | Shape::Pin => {
            let hr = 19.0f32;
            stroke(&[(sx + 3.0, ey - hr), (sx, ey - hr + 3.0), (sx, ey + hr - 3.0), (sx + 3.0, ey + hr)], false);
            stroke(&[(sx + 3.0, ey - hr), (sx + 3.0, ey + hr)], false);
            shank(&mut stroke, sx + 3.0, hr);
        }
        Shape::HexNut => {
            let x1 = sx + 30.0;
            stroke(&[(sx, ey - r), (x1, ey - r), (x1, ey + r), (sx, ey + r)], true);
            stroke(&[(sx, ey - r * 0.5), (x1, ey - r * 0.5)], false);
            stroke(&[(sx, ey + r * 0.5), (x1, ey + r * 0.5)], false);
        }
        Shape::Washer => {
            let x1 = sx + 8.0;
            stroke(&[(sx, ey - r), (x1, ey - r), (x1, ey + r), (sx, ey + r)], true);
        }
    }
}
