//! Onshape assembly behaviours (child module of `assembly_view`): the
//! instance / mate / mate connector context menus in Onshape's order,
//! hide / show / isolate / make transparent on hovered or selected
//! instances, Unfix all, copy / paste of instances, inline rename (Shift+N),
//! the instance list filter with colon commands, mate Reset and Apply limit
//! position, and the S shortcut toolbar.

use super::*;
use gpui_kit::component::input::InputEvent;
use gpui_kit::component::Disableable as _;
use gpui_kit::component::menu::PopupMenu;
use noshape_model::assembly::ops::{FilterCommand, InstanceClipboard, ListFilter, RowInfo, RowKind};
use std::rc::Rc;

pub(crate) type MenuAct = Rc<dyn Fn(&mut AssemblyView, &mut Window, &mut Context<AssemblyView>)>;

/// One context menu entry (built from the view state when the menu opens).
#[derive(Clone)]
pub(crate) enum MenuEntry {
    Item { label: String, icon: Option<&'static str>, disabled: bool, act: MenuAct },
    Sub { label: String, children: Vec<MenuEntry> },
    Sep,
}

fn item(label: impl Into<String>, act: impl Fn(&mut AssemblyView, &mut Window, &mut Context<AssemblyView>) + 'static) -> MenuEntry {
    MenuEntry::Item { label: label.into(), icon: None, disabled: false, act: Rc::new(act) }
}

fn icon_item(label: impl Into<String>, icon: &'static str, act: impl Fn(&mut AssemblyView, &mut Window, &mut Context<AssemblyView>) + 'static) -> MenuEntry {
    MenuEntry::Item { label: label.into(), icon: Some(icon), disabled: false, act: Rc::new(act) }
}

/// Fill a popup menu with entries; separators at the ends or doubled are dropped.
pub(crate) fn fill_menu(mut m: PopupMenu, entries: Vec<MenuEntry>, view: Entity<AssemblyView>, window: &mut Window, cx: &mut Context<PopupMenu>) -> PopupMenu {
    let mut cleaned: Vec<MenuEntry> = Vec::new();
    for e in entries {
        if matches!(e, MenuEntry::Sep) && cleaned.last().is_none_or(|l| matches!(l, MenuEntry::Sep)) {
            continue;
        }
        cleaned.push(e);
    }
    while cleaned.last().is_some_and(|l| matches!(l, MenuEntry::Sep)) {
        cleaned.pop();
    }
    for e in cleaned {
        match e {
            MenuEntry::Sep => m = m.separator(),
            MenuEntry::Item { label, icon, disabled, act } => {
                let v = view.clone();
                let mut it = PopupMenuItem::new(label).disabled(disabled).on_click(move |_, window, cx| {
                    let act = act.clone();
                    v.update(cx, |this, cx| {
                        this.ui.ctx_menu = None;
                        act(this, window, cx);
                        cx.notify();
                    })
                });
                if let Some(i) = icon {
                    it = it.icon(Icon::empty().path(icon_path(i)));
                }
                m = m.item(it);
            }
            MenuEntry::Sub { label, children } => {
                let v = view.clone();
                m = m.submenu(label, window, cx, move |sm, window, cx| fill_menu(sm, children.clone(), v.clone(), window, cx));
            }
        }
    }
    m
}

/// Instance row being dragged in the list.
#[derive(Clone)]
pub(crate) struct InstanceDrag {
    pub id: InstanceId,
    pub label: SharedString,
}

impl Render for InstanceDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        div().px_2().py_1().rounded_md().bg(pal.panel_raised).border_1().border_color(pal.accent).text_sm().text_color(pal.text).child(self.label.clone())
    }
}

/// Section view state.
pub(crate) struct AsmSection {
    pub label: String,
    pub origin: DVec3,
    pub normal: DVec3,
    pub offset: f64,
    pub flipped: bool,
    pub input: Entity<InputState>,
    _sub: Subscription,
}

impl AsmSection {
    /// Clip plane (normal, d): geometry with dot(normal, p) > d is removed.
    pub fn clip(&self) -> (DVec3, f64) {
        let n = if self.flipped { -self.normal } else { self.normal };
        (n, n.dot(self.origin + self.normal * self.offset))
    }
}

/// Row being renamed inline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RenameRow {
    Instance(InstanceId),
    Feature(MateId),
}

/// View state of the behaviours in this module.
#[derive(Default)]
pub(crate) struct AsmUi {
    /// Instance list filter box and its parsed value.
    pub filter_input: Option<Entity<InputState>>,
    pub filter: ListFilter,
    pub filter_sub: Option<Subscription>,
    /// Instance row under the cursor in the list (highlighted in the viewport).
    pub list_hover: Option<InstanceId>,
    /// Make transparent (Shift+T): instances drawn transparent.
    pub transparent: Option<HashSet<InstanceId>>,
    /// S shortcut toolbar position (viewport-local).
    pub mini_toolbar: Option<DVec2>,
    /// Graphics area context menu.
    pub ctx_menu: Option<(Point<Pixels>, Entity<PopupMenu>, Subscription)>,
    /// Inline rename (Shift+N / Rename).
    pub rename: Option<(RenameRow, Entity<InputState>, Subscription)>,
    /// Instance list scrolling (selection in the graphics scrolls to the row).
    pub list_scroll: ScrollHandle,
    pub scroll_to_selection: bool,
    /// Mates expanded in the list (their implicit mate connectors shown).
    pub expanded_mates: HashSet<MateId>,
    /// Keyboard shortcut list (Shift+/).
    pub show_shortcuts: bool,
    /// Insert dialog: search box, source (0 current document, 1 other
    /// documents) and element tab (0 Part Studios, 1 Assemblies).
    pub insert_search: Option<Entity<InputState>>,
    pub insert_search_sub: Option<Subscription>,
    pub insert_source: u8,
    pub insert_tab: u8,
    /// Snap-insert: (source connector on the inserted instance, target), and
    /// which of the instance's explicit mate connectors is the source.
    pub place_snap: Option<(MateConnectorRef, MateConnectorRef)>,
    pub place_src_ix: usize,
    /// Measure tool (`[`): picked entities and their measurements.
    pub measure: Option<Vec<(Vec<InstanceId>, Reference)>>,
    pub measure_result: Vec<(String, noshape_model::regen::measure::Measurement)>,
    /// Section view (Shift+X): plane origin, normal, offset, flipped, offset field.
    pub section: Option<AsmSection>,
    /// Isolate / Make transparent slider: opacity of the transparent instances.
    pub transparency: Option<f32>,
    /// Live mate value being typed (double-click a value label): mate, DOF.
    pub value_edit: Option<(MateId, usize, Entity<InputState>, Subscription)>,
    /// Insert dialog: configuration chosen per configured Part Studio.
    pub insert_config: HashMap<TabId, noshape_model::configuration::ConfigValues>,
    /// Display states dialog: open, name field, chosen state, renaming.
    pub display_states: bool,
    pub ds_name: Option<Entity<InputState>>,
    pub ds_selected: Option<usize>,
    pub ds_renaming: Option<Entity<InputState>>,
    /// Group dialog: (group being edited, instances).
    pub group: Option<(Option<MateId>, Vec<InstanceId>)>,
    /// Mate indicators in overlay pick order.
    pub overlay_mates: Vec<MateId>,
    /// Select other (`): instances under the cursor (nearest first), the
    /// chosen one and where the list opens (viewport-local).
    pub select_other: Option<(Vec<InstanceId>, usize, DVec2)>,
}

impl AssemblyView {
    // ------------------------------------------------------------ targets

    /// Top-level instance under the cursor (graphics or instance list).
    pub(super) fn hovered_top(&self) -> Option<InstanceId> {
        self.hover.as_ref().map(|t| t.path[0]).or(self.ui.list_hover)
    }

    /// Hovered instance plus the selection (Onshape's "selected or hovered").
    pub(super) fn hovered_or_selected(&self) -> Vec<InstanceId> {
        let mut out: Vec<InstanceId> = self.hovered_top().into_iter().collect();
        for id in &self.selected {
            if !out.contains(id) {
                out.push(*id);
            }
        }
        out
    }

    // ------------------------------------------------------------ visibility

    /// y: hide the hovered / selected instances (or the selected mate).
    pub(super) fn hide_hovered_or_selected(&mut self, cx: &mut Context<Self>) {
        if self.hover.is_none()
            && self.ui.list_hover.is_none()
            && self.selected.is_empty()
            && let Some(m) = self.selected_mate
        {
            self.edit("Hide", cx, |a| a.set_mates_hidden(&[m], true));
            return;
        }
        let ids = self.hovered_or_selected();
        if ids.is_empty() {
            self.message = Some((false, "Select or hover over an instance to hide it.".into()));
            cx.notify();
            return;
        }
        self.set_hidden(ids, true, cx);
    }

    /// Shift+Y: show hidden instances.
    pub(super) fn show_hidden(&mut self, cx: &mut Context<Self>) {
        let any = self.assembly(cx).is_some_and(|a| a.instances.iter().any(|i| i.hidden));
        if any {
            self.edit("Show all", cx, |a| a.set_all_hidden(false));
        }
    }

    pub(super) fn hide_other_instances(&mut self, keep: Vec<InstanceId>, cx: &mut Context<Self>) {
        self.edit("Hide other instances", cx, |a| a.hide_other_instances(&keep));
    }

    pub(super) fn hide_all_instances(&mut self, cx: &mut Context<Self>) {
        self.edit("Hide all instances", cx, |a| a.set_all_hidden(true));
    }

    pub(super) fn set_mates_hidden(&mut self, ids: Vec<MateId>, hidden: bool, cx: &mut Context<Self>) {
        self.edit(if hidden { "Hide" } else { "Show" }, cx, |a| a.set_mates_hidden(&ids, hidden));
        if !hidden && !self.show_mates {
            // Showing a mate indicator turns mate display on.
            self.show_mates = true;
        }
        self.refresh_scene(false, cx);
    }

    pub(super) fn show_all_mates(&mut self, cx: &mut Context<Self>) {
        let ids: Vec<MateId> = self.assembly(cx).map(|a| a.mates.iter().map(|m| m.id).collect()).unwrap_or_default();
        self.set_mates_hidden(ids, false, cx);
    }

    pub(super) fn show_all_connectors(&mut self, cx: &mut Context<Self>) {
        let ids: Vec<MateId> = self.assembly(cx).map(|a| a.connectors.iter().map(|c| c.id).collect()).unwrap_or_default();
        self.edit("Show all mate connectors", cx, |a| a.set_mates_hidden(&ids, false));
    }

    // ------------------------------------------------------------ isolate / transparent

    /// Shift+I / Shift+T (and the menus): cumulative while the mode is on,
    /// like Onshape (hover + key repeatedly adds instances).
    pub(super) fn isolate_cmd(&mut self, transparent: bool, ids: Vec<InstanceId>, cx: &mut Context<Self>) {
        if ids.is_empty() {
            self.message = Some((false, "Select or hover over an instance first.".into()));
            cx.notify();
            return;
        }
        let set = if transparent { &mut self.ui.transparent } else { &mut self.isolate };
        match set {
            Some(s) => s.extend(ids),
            None => *set = Some(ids.into_iter().collect()),
        }
        if transparent {
            self.isolate = None;
        } else {
            self.ui.transparent = None;
        }
        self.rebuild_scene(false, cx);
        cx.notify();
    }

    pub(super) fn exit_isolate(&mut self, cx: &mut Context<Self>) -> bool {
        if self.isolate.take().is_none() && self.ui.transparent.take().is_none() {
            return false;
        }
        self.rebuild_scene(false, cx);
        cx.notify();
        true
    }

    pub(super) fn render_isolate_panel(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let pal = p();
        let title = if self.isolate.is_some() {
            "Isolate"
        } else if self.ui.transparent.is_some() {
            "Make transparent"
        } else {
            return None;
        };
        Some(
            h_flex()
                .id("asm-isolate-panel")
                .occlude()
                .absolute()
                .bottom(px(48.))
                .left(relative(0.5))
                .ml(px(-200.))
                .gap_2()
                .px_3()
                .py_1()
                .rounded_md()
                .bg(pal.panel_raised)
                .border_1()
                .border_color(pal.accent)
                .text_xs()
                .child(div().text_color(pal.text).child(title))
                // Transparency slider (Onshape: show / hide internal geometry).
                .child(div().text_color(pal.text_muted).child("Transparency"))
                .child(Button::new("asm-iso-less").label("−").ghost().xsmall().on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.step_transparency(0.1, cx))))
                .child(div().w(px(34.)).text_center().text_color(pal.text).child(format!("{:.0}%", (1.0 - self.ui.transparency.unwrap_or(if title == "Isolate" { 0.12 } else { 0.35 })) * 100.0)))
                .child(Button::new("asm-iso-more").label("+").ghost().xsmall().on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.step_transparency(-0.1, cx))))
                .child(Button::new("asm-iso-exit").label(if title == "Isolate" { "Exit isolate" } else { "Close" }).xsmall().ghost().on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.exit_isolate(cx);
                })))
                .into_any_element(),
        )
    }

    /// A click in the graphics area leaves text fields (like Onshape): the
    /// open mate dialog keeps the keyboard (Enter, a), else the view takes it.
    pub(super) fn focus_from_viewport(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(d) = self.mate.as_ref().map(|m| m.dialog.clone()) {
            let fh = d.read(cx).focus_handle(cx);
            if !fh.is_focused(window) {
                window.focus(&fh, cx);
            }
        } else if !self.focus.is_focused(window) {
            window.focus(&self.focus, cx);
        }
    }

    // ------------------------------------------------------------ a / q

    /// a (flip primary axis) / q (reorient secondary axis): the connector
    /// dragged in snap mode, else the open mate or mate connector dialog.
    pub(super) fn flip_or_reorient(&mut self, flip: bool, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(s) = &mut self.snap_drag
            && s.started
            && let Some(src) = s.sources.get_mut(s.source_ix)
        {
            if flip {
                src.flip = !src.flip;
            } else {
                src.rotate_quarters = (src.rotate_quarters + 1) % 4;
            }
            self.refresh_snap_preview(cx);
            return;
        }
        if let Some(d) = self.mate.as_ref().map(|m| m.dialog.clone()) {
            d.update(cx, |d, cx| if flip { d.flip(cx) } else { d.reorient(cx) });
            return;
        }
        if let Some(m) = &mut self.mc {
            if flip {
                m.flip = !m.flip;
            } else {
                m.reorient = (m.reorient + 1) % 4;
            }
            self.schedule_eval(window, cx);
            cx.notify();
        }
    }

    // ------------------------------------------------------------ fix

    pub(super) fn unfix_all(&mut self, cx: &mut Context<Self>) {
        self.edit("Unfix all", cx, |a| {
            a.unfix_all();
        });
    }

    // ------------------------------------------------------------ camera

    pub(super) fn zoom_to_instances(&mut self, ids: Vec<InstanceId>, cx: &mut Context<Self>) {
        let mut bb: Option<(DVec3, DVec3)> = None;
        for id in &ids {
            if let Some((a, b)) = self.result.instance_bounds(*id) {
                bb = Some(bb.map(|(x, y)| (x.min(a), y.max(b))).unwrap_or((a, b)));
            }
        }
        let Some((lo, hi)) = bb else { return };
        self.viewport.update(cx, |vp, cx| {
            let size = vp.size();
            let aspect = if size.y > 0.0 { size.x / size.y } else { 1.0 };
            let mut to = vp.camera;
            to.fit_sphere((lo + hi) * 0.5, ((hi - lo).length() * 0.5).max(1e-4), aspect);
            vp.animate_to(to, cx);
        });
    }

    // ------------------------------------------------------------ copy / paste

    /// Ctrl+C: copy the selected instances (with the mates and mate
    /// connectors among them) to the clipboard.
    pub(super) fn copy_selection(&mut self, cx: &mut Context<Self>) {
        let Some(asm) = self.assembly(cx) else { return };
        let ids: Vec<InstanceId> = self.selected.iter().copied().filter(|id| asm.generated_by(*id).is_none()).collect();
        if ids.is_empty() {
            self.message = Some((false, "Select instances to copy.".into()));
            cx.notify();
            return;
        }
        let clip = InstanceClipboard::copy(asm, &ids);
        let n = clip.instances.len();
        if let Ok(json) = serde_json::to_string(&serde_json::json!({ "noshape_instances": clip })) {
            cx.write_to_clipboard(ClipboardItem::new_string(json));
            self.message = Some((false, if n == 1 { format!("Copied {}", clip.instances[0].name) } else { format!("Copied {n} items") }));
            cx.notify();
        }
    }

    /// Ctrl+V: paste copied instances; they follow the cursor until a
    /// click places them (Onshape: pasted where the mouse is clicked).
    pub(super) fn paste_instances(&mut self, cx: &mut Context<Self>) {
        let Some(text) = cx.read_from_clipboard().and_then(|c| c.text()) else { return };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else { return };
        let Some(clip) = v.get("noshape_instances").and_then(|c| serde_json::from_value::<InstanceClipboard>(c.clone()).ok()) else {
            self.message = Some((false, "Nothing to paste: copy instances first.".into()));
            cx.notify();
            return;
        };
        // Sources must exist in this document (Part Studios / assemblies).
        let doc = self.doc.read(cx).session.doc.clone();
        let valid = clip.instances.iter().all(|i| match &i.source {
            InstanceSource::Part { studio, .. } | InstanceSource::PartStudio { studio, .. } | InstanceSource::MirroredPart { studio, .. } => doc.tab(*studio).is_some(),
            InstanceSource::Assembly { assembly } => doc.tab(*assembly).is_some() && !contains_assembly(&doc, *assembly, self.tab, 0),
            InstanceSource::External { .. } => true,
        });
        if !valid {
            self.message = Some((true, "The copied instances come from another document.".into()));
            cx.notify();
            return;
        }
        let n = clip.instances.len();
        let ids = self.edit(&if n == 1 { format!("Paste {}", clip.instances[0].name) } else { format!("Paste {n} items") }, cx, |a| clip.paste(a, [0.0; 3])).unwrap_or_default();
        self.start_placing(ids, cx);
    }

    // ------------------------------------------------------------ rename

    pub(super) fn start_row_rename(&mut self, row: RenameRow, window: &mut Window, cx: &mut Context<Self>) {
        let Some(asm) = self.assembly(cx) else { return };
        let current = match row {
            RenameRow::Instance(id) => asm.instance(id).map(|i| i.name.clone()),
            RenameRow::Feature(id) => asm
                .mate(id)
                .map(|m| m.name.clone())
                .or_else(|| asm.relations.iter().find(|r| r.id == id).map(|r| r.name.clone()))
                .or_else(|| asm.groups.iter().find(|g| g.id == id).map(|g| g.name.clone()))
                .or_else(|| asm.connector(id).map(|c| c.name.clone()))
                .or_else(|| asm.pattern(id).map(|p| p.name.clone())),
        };
        let current = current.or_else(|| match row {
            RenameRow::Feature(id) => asm.folders.iter().find(|f| f.id == id).map(|f| f.name.clone()),
            RenameRow::Instance(_) => None,
        });
        let Some(current) = current else { return };
        let st = cx.new(|cx| InputState::new(window, cx).default_value(current));
        // Focus after the context menu that started the rename has closed
        // (it gives focus back to where it was opened).
        let st2 = st.clone();
        cx.spawn_in(window, async move |_, cx| {
            cx.background_executor().timer(Duration::from_millis(40)).await;
            let _ = st2.update_in(cx, |s, window, cx| {
                s.focus(window, cx);
                s.select_all(window, cx);
            });
        })
        .detach();
        let sub = cx.subscribe_in(&st, window, |this: &mut Self, _, ev: &InputEvent, window, cx| match ev {
            InputEvent::PressEnter { .. } => {
                this.commit_row_rename(cx);
                window.focus(&this.focus, cx);
            }
            InputEvent::Blur => this.commit_row_rename(cx),
            _ => {}
        });
        self.ui.rename = Some((row, st, sub));
        cx.notify();
    }

    pub(super) fn commit_row_rename(&mut self, cx: &mut Context<Self>) {
        let Some((row, st, _)) = self.ui.rename.take() else { return };
        let name = st.read(cx).value().trim().to_string();
        let unchanged = self.assembly(cx).is_some_and(|a| match row {
            RenameRow::Instance(id) => a.instance(id).is_some_and(|i| i.name == name),
            RenameRow::Feature(_) => false,
        });
        if !name.is_empty() && !unchanged {
            let (inst, feat) = match row {
                RenameRow::Instance(id) => (Some(id), None),
                RenameRow::Feature(id) => (None, Some(id)),
            };
            self.edit(&format!("Rename : {name}"), cx, |a| a.rename(inst, feat, &name));
        }
        cx.notify();
    }

    /// The inline rename input when `row` is being renamed.
    pub(super) fn rename_field(&self, row: RenameRow) -> Option<AnyElement> {
        let (r, st, _) = self.ui.rename.as_ref()?;
        (*r == row).then(|| div().flex_1().child(Input::new(st).xsmall()).into_any_element())
    }

    /// Shift+N: rename the selected instance or mate feature.
    pub(super) fn rename_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(m) = self.selected_mate {
            self.start_row_rename(RenameRow::Feature(m), window, cx);
        } else if let Some(&id) = self.selected.first() {
            self.start_row_rename(RenameRow::Instance(id), window, cx);
        }
    }

    // ------------------------------------------------------------ mate positions

    /// Mate context menu > Reset: back to the mate's home position (all of
    /// its free values zero), other mates enforced.
    pub(super) fn reset_mate(&mut self, id: MateId, cx: &mut Context<Self>) {
        let Some(m) = self.assembly(cx).and_then(|a| a.mate(id).cloned()) else { return };
        if matches!(m.mate_type, MateType::Fastened | MateType::Tangent | MateType::Width) {
            return;
        }
        let solved = self.result.prepared.solve(&SolveOptions { locks: vec![(id, [0.0; 4])], ..Default::default() });
        self.apply_solved(solved, cx);
        self.edit(&format!("Reset : {}", m.name), cx, |_| {});
    }

    /// Mate context menu > Apply limit position > Min/Max: move the mate to
    /// the limit of one degree of freedom (the others stay where they are).
    pub(super) fn apply_limit_position(&mut self, id: MateId, key: &'static str, max: bool, cx: &mut Context<Self>) {
        let Some(m) = self.assembly(cx).and_then(|a| a.mate(id).cloned()) else { return };
        if self.result.mates.iter().any(|s| s.id == id && !s.ok) {
            self.message = Some((true, "Mates with errors cannot be applied.".into()));
            cx.notify();
            return;
        }
        let units = self.units(cx);
        let Some(ix) = noshape_model::assembly::eval::limit_index(key) else { return };
        let Some(l) = m.limits.iter().find(|(k, _)| k == key).map(|(_, l)| l.clone()) else { return };
        let dim = if ix == 0 { noshape_units::Dim::ANGLE } else { noshape_units::Dim::LENGTH };
        let e = if max { l.max } else { l.min };
        let Some(v) = e.and_then(|e| noshape_units::evaluate_as(&e.text, dim, &units, &noshape_units::EmptyScope).ok()).map(|q| q.value) else {
            self.message = Some((true, "Unable to apply limit".into()));
            cx.notify();
            return;
        };
        let solved = self.result.prepared.solve(&SolveOptions { hold_values: vec![(id, ix, v)], ..Default::default() });
        self.apply_solved(solved, cx);
        self.edit(&format!("Apply limit position : {}", m.name), cx, |_| {});
        let axis = match key {
            "rz" => "Z",
            "x" => "X",
            "y" => "Y",
            _ => "Z",
        };
        self.message = Some((false, format!("Moved mate to {} {axis} limit", if max { "max" } else { "min" })));
        cx.notify();
    }

    // ------------------------------------------------------------ context menus

    /// Instance context menu, in Onshape's order (part / subassembly / rigid
    /// Part Studio instance). `graphics`: opened in the graphics area.
    pub(super) fn instance_menu(&self, top: InstanceId, graphics: bool, cx: &App) -> Vec<MenuEntry> {
        let Some(asm) = self.assembly(cx) else { return vec![] };
        let Some(inst) = asm.instance(top).cloned() else { return vec![] };
        let targets = self.targets(Some(top));
        let multi = targets.len() > 1;
        let (fixed, hidden, suppressed) = (inst.fixed, inst.hidden, inst.suppressed);
        let is_asm = matches!(inst.source, InstanceSource::Assembly { .. });
        let isolating = self.isolate.is_some() || self.ui.transparent.is_some();
        let mut m: Vec<MenuEntry> = Vec::new();
        if isolating {
            m.push(item("Exit isolate", |t, _, cx| {
                t.exit_isolate(cx);
            }));
            m.push(MenuEntry::Sep);
        }
        let tg = targets.clone();
        m.push(icon_item(if hidden { "Show" } else { "Hide" }, "eye-off", move |t, _, cx| t.set_hidden(tg.clone(), !hidden, cx)));
        let tg = targets.clone();
        m.push(item("Hide other instances", move |t, _, cx| t.hide_other_instances(tg.clone(), cx)));
        m.push(item("Hide all instances", |t, _, cx| t.hide_all_instances(cx)));
        let tg = targets.clone();
        m.push(item("Isolate…", move |t, _, cx| t.isolate_cmd(false, tg.clone(), cx)));
        let tg = targets.clone();
        m.push(item("Make transparent…", move |t, _, cx| t.isolate_cmd(true, tg.clone(), cx)));
        let tg = targets.clone();
        m.push(item(if suppressed { "Unsuppress" } else { "Suppress" }, move |t, _, cx| t.set_suppressed(tg.clone(), !suppressed, cx)));
        let tg = targets.clone();
        m.push(icon_item(if fixed { "Unfix" } else { "Fix" }, "fix", move |t, _, cx| t.set_fixed(tg.clone(), !fixed, cx)));
        if is_asm || multi {
            let tg = targets.clone();
            m.push(item("Check interference…", move |t, _, cx| {
                t.selected = tg.clone();
                t.run_interference(cx);
            }));
        }
        m.push(MenuEntry::Sep);
        if !multi {
            m.push(icon_item("Add mate connector to instance origin…", "mate-connector-assembly", move |t, w, cx| t.mc_at_instance_origin(top, w, cx)));
        }
        m.push(item(if multi { format!("Replace {} instances…", targets.len()) } else { "Replace instances…".to_string() }, move |t, _, cx| t.open_replace(top, cx)));
        let switch_tab = match &inst.source {
            InstanceSource::Assembly { assembly } => Some(*assembly),
            other => other.studio(),
        };
        if let Some(tab) = switch_tab.filter(|_| !multi)
            && let Some(name) = self.doc.read(cx).session.doc.tab(tab).map(|t| t.name.clone())
        {
            m.push(icon_item(format!("Switch to {name}"), if is_asm { "assembly" } else { "part-studio" }, move |_, _, cx| cx.emit(AssemblyViewEvent::ActivateTab(tab))));
        }
        if let (false, InstanceSource::Assembly { assembly }) = (multi, &inst.source) {
            // Lock/follow position to ▸ Lock current position | <named positions> (Onshape).
            let names: Vec<String> = match self.doc.read(cx).session.doc.tab(*assembly).map(|t| &t.content) {
                Some(TabContent::Assembly(a)) => a.named_positions.iter().map(|n| n.name.clone()).collect(),
                _ => vec![],
            };
            let cur = inst.lock.clone();
            let mut children = vec![MenuEntry::Item {
                label: if cur == SubassemblyLock::Current { "✓ Lock current position".into() } else { "Lock current position".into() },
                icon: None,
                disabled: false,
                act: Rc::new(move |t: &mut AssemblyView, _: &mut Window, cx: &mut Context<AssemblyView>| t.set_sub_lock(top, SubassemblyLock::Current, cx)),
            }];
            for n in names {
                let on = cur == SubassemblyLock::Named(n.clone());
                let n2 = n.clone();
                children.push(item(if on { format!("✓ {n}") } else { n }, move |t, _, cx| t.set_sub_lock(top, SubassemblyLock::Named(n2.clone()), cx)));
            }
            if !cur.is_flexible() {
                children.push(MenuEntry::Sep);
                children.push(item("Unlock", move |t, _, cx| t.set_sub_lock(top, SubassemblyLock::Flexible, cx)));
            }
            m.push(MenuEntry::Sub { label: "Lock/follow position to".into(), children });
        }
        if !multi && matches!(inst.source, InstanceSource::Part { .. } | InstanceSource::PartStudio { .. }) {
            m.push(item("Change configuration…", move |t, w, cx| t.open_config(top, w, cx)));
            let same = inst.source.clone();
            m.push(item("Select instances with same part and same configuration", move |t, _, cx| {
                let ids: Vec<InstanceId> = t.assembly(cx).map(|a| a.instances.iter().filter(|i| i.source == same).map(|i| i.id).collect()).unwrap_or_default();
                t.selected = ids;
                t.ui.scroll_to_selection = true;
                t.refresh_scene(false, cx);
            }));
        }
        m.push(MenuEntry::Sep);
        let tg = targets.clone();
        let copy_label = if multi { format!("Copy {} items", targets.len()) } else { format!("Copy {}", inst.name) };
        m.push(icon_item(copy_label, "copy", move |t, _, cx| {
            t.selected = tg.clone();
            t.copy_selection(cx);
        }));
        m.push(MenuEntry::Sep);
        let tg = targets.clone();
        m.push(item("Move to new subassembly", move |t, _, cx| t.move_to_new_subassembly(tg.clone(), cx)));
        if is_asm {
            let tg: Vec<InstanceId> = targets.iter().copied().filter(|id| asm.instance(*id).is_some_and(|i| matches!(i.source, InstanceSource::Assembly { .. }))).collect();
            m.push(item("Dissolve subassembly", move |t, _, cx| t.dissolve_subassemblies(tg.clone(), cx)));
        }
        m.push(MenuEntry::Sep);
        if graphics {
            m.push(item("Show mates", move |t, _, cx| {
                t.show_mates_mode = true;
                t.mates_pinned = Some(top);
                t.update_capture(cx);
                t.refresh_scene(false, cx);
            }));
            m.push(item("Go to item in list", move |t, _, cx| {
                t.selected = vec![top];
                t.ui.scroll_to_selection = true;
                t.refresh_scene(false, cx);
            }));
        }
        let tg = targets.clone();
        m.push(item("Zoom to selection", move |t, _, cx| t.zoom_to_instances(tg.clone(), cx)));
        m.push(MenuEntry::Sep);
        let tg = targets.clone();
        m.push(icon_item("Add selection to folder…", "folder", move |t, w, cx| t.folder_from_instances(tg.clone(), w, cx)));
        let has_children = self.result.node(&[top]).is_some_and(|n| !n.children.is_empty());
        if has_children {
            m.push(MenuEntry::Sub {
                label: "Expand".into(),
                children: vec![
                    item("Expand selected", move |t, _, cx| {
                        t.expanded.insert(vec![top]);
                        cx.notify();
                    }),
                    item("Expand all", |t, _, cx| t.expand_all(true, cx)),
                ],
            });
            m.push(MenuEntry::Sub {
                label: "Collapse".into(),
                children: vec![
                    item("Collapse selected", move |t, _, cx| {
                        t.expanded.retain(|p| p[0] != top);
                        cx.notify();
                    }),
                    item("Collapse all", |t, _, cx| t.expand_all(false, cx)),
                ],
            });
            m.push(MenuEntry::Sep);
        }
        m.push(item("Create empty subassembly", |t, _, cx| t.create_empty_subassembly(cx)));
        m.push(MenuEntry::Sep);
        let tg = targets;
        m.push(icon_item("Delete", "delete", move |t, _, cx| t.delete_instances(tg.clone(), cx)));
        m
    }

    // ------------------------------------------------------------ folders

    /// Add selection to folder: a new folder around the instances, named
    /// inline right away.
    pub(super) fn folder_from_instances(&mut self, ids: Vec<InstanceId>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(fid) = self.edit("Add selection to folder", cx, |a| a.add_folder(&ids, &[])) else { return };
        self.expanded.insert(vec![InstanceId(fid.0)]);
        self.start_row_rename(RenameRow::Feature(fid), window, cx);
    }

    pub(super) fn folder_from_features(&mut self, ids: Vec<MateId>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(fid) = self.edit("Add selection to folder", cx, |a| a.add_folder(&[], &ids)) else { return };
        self.ui.expanded_mates.insert(fid);
        self.start_row_rename(RenameRow::Feature(fid), window, cx);
    }

    /// Folder row (instance list or Mate features list).
    pub(super) fn folder_row(&self, folder: &noshape_model::assembly::ListFolder, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let id = folder.id;
        let mate_folder = folder.is_mate_folder();
        let expanded = if mate_folder { self.ui.expanded_mates.contains(&id) } else { self.expanded.contains(&vec![InstanceId(id.0)]) };
        let asm = self.assembly(cx);
        let members: Vec<InstanceId> = folder.instances.clone();
        let hidden = if mate_folder {
            asm.is_some_and(|a| folder.features.iter().all(|f| a.mate(*f).is_some_and(|m| m.hidden) || a.connector(*f).is_some_and(|c| c.hidden)))
        } else {
            !members.is_empty() && asm.is_some_and(|a| members.iter().all(|m| a.instance(*m).is_some_and(|i| i.hidden)))
        };
        let count = if mate_folder { folder.features.len() } else { folder.instances.len() };
        let view = cx.entity();
        let features = folder.features.clone();
        h_flex()
            .id(SharedString::from(format!("folder-{}", id.0)))
            .group(SharedString::from(format!("folder-row-{}", id.0)))
            .h(px(24.))
            .pl(px(6.))
            .pr_1()
            .gap_1p5()
            .rounded_sm()
            .text_sm()
            .text_color(if hidden { pal.text_faint } else { pal.text })
            .hover(|d| d.bg(pal.panel_hover))
            .child(div().w(px(12.)).child(svg().path(icon_path(if expanded { "chevron-down" } else { "chevron-right" })).size_3().text_color(pal.text_faint)))
            .child(svg().path(icon_path("folder")).size_3p5().text_color(pal.text_muted))
            .child(match self.rename_field(RenameRow::Feature(id)) {
                Some(field) => field,
                None => div().flex_1().truncate().child(format!("{} ({count})", folder.name)).into_any_element(),
            })
            .child(
                div().when(!hidden, |d| d.invisible().group_hover(SharedString::from(format!("folder-row-{}", id.0)), |d| d.visible())).child(
                    Button::new(SharedString::from(format!("feye-{}", id.0))).icon(Icon::empty().path(icon_path(if hidden { "eye-off" } else { "eye" }))).ghost().xsmall().on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        cx.stop_propagation();
                        if mate_folder {
                            this.set_mates_hidden(features.clone(), !hidden, cx);
                        } else {
                            this.set_hidden(members.clone(), !hidden, cx);
                        }
                    })),
                ),
            )
            .drag_over::<InstanceDrag>(move |st, _, _, _| st.bg(with_alpha(pal.accent, 0.15)))
            .on_drop(cx.listener(move |this, d: &InstanceDrag, _, cx| {
                if !mate_folder {
                    let iid = d.id;
                    this.edit("Add to folder", cx, |a| a.move_into_folder(iid, id));
                }
            }))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                if mate_folder {
                    if !this.ui.expanded_mates.remove(&id) {
                        this.ui.expanded_mates.insert(id);
                    }
                } else {
                    let key = vec![InstanceId(id.0)];
                    if !this.expanded.remove(&key) {
                        this.expanded.insert(key);
                    }
                }
                cx.notify();
            }))
            .context_menu(move |menu, window, cx| {
                let entries = view.read(cx).folder_menu(id, cx);
                fill_menu(menu, entries, view.clone(), window, cx)
            })
            .into_any_element()
    }

    /// Mate features rows with mate folders folded in (a folder sits at the
    /// place of its first member).
    pub(super) fn fold_mate_rows(&self, asm: &Assembly, ids: Vec<MateId>, rows: Vec<AnyElement>, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let folders: Vec<&noshape_model::assembly::ListFolder> = asm.folders.iter().filter(|f| f.is_mate_folder()).collect();
        if folders.is_empty() {
            return rows;
        }
        let mut members: HashMap<MateId, Vec<AnyElement>> = HashMap::new();
        let mut order: Vec<(Option<MateId>, Option<AnyElement>)> = Vec::new();
        for (id, row) in ids.into_iter().zip(rows) {
            match folders.iter().find(|f| f.features.contains(&id)) {
                Some(f) => {
                    let e = members.entry(f.id).or_default();
                    if e.is_empty() {
                        order.push((Some(f.id), None));
                    }
                    e.push(row);
                }
                None => order.push((None, Some(row))),
            }
        }
        let mut out = Vec::new();
        for (fid, row) in order {
            match (fid, row) {
                (Some(fid), _) => {
                    let Some(f) = folders.iter().find(|f| f.id == fid) else { continue };
                    out.push(self.folder_row(f, cx));
                    let rows = members.remove(&fid).unwrap_or_default();
                    if self.ui.expanded_mates.contains(&fid) {
                        out.extend(rows.into_iter().map(|r| div().pl(px(14.)).child(r).into_any_element()));
                    }
                }
                (None, Some(r)) => out.push(r),
                _ => {}
            }
        }
        out
    }

    /// Folder context menu (Onshape order).
    pub(super) fn folder_menu(&self, id: MateId, cx: &App) -> Vec<MenuEntry> {
        let Some(f) = self.assembly(cx).and_then(|a| a.folders.iter().find(|f| f.id == id).cloned()) else { return vec![] };
        let mut m = vec![item("Rename", move |t, w, cx| t.start_row_rename(RenameRow::Feature(id), w, cx)), MenuEntry::Sep];
        if f.is_mate_folder() {
            let feats = f.features.clone();
            let (f1, f2, f3) = (feats.clone(), feats.clone(), feats.clone());
            m.push(icon_item("Hide", "eye-off", move |t, _, cx| t.set_mates_hidden(f1.clone(), true, cx)));
            m.push(item("Show", move |t, _, cx| t.set_mates_hidden(f2.clone(), false, cx)));
            m.push(item("Hide other mate connectors and mates", move |t, _, cx| {
                t.edit("Hide other mate connectors and mates", cx, |a| a.hide_other_mates(&f3));
            }));
            m.push(item("Hide all mate connectors and mates", |t, _, cx| {
                t.edit("Hide all mate connectors and mates", cx, |a| a.hide_other_mates(&[]));
            }));
            let insts: Vec<InstanceId> = {
                let mut v = Vec::new();
                for x in &feats {
                    for i in self.mate_instances(*x, cx) {
                        if !v.contains(&i) {
                            v.push(i);
                        }
                    }
                }
                v
            };
            let i2 = insts.clone();
            m.push(item("Isolate…", move |t, _, cx| t.isolate_cmd(false, insts.clone(), cx)));
            m.push(item("Make transparent…", move |t, _, cx| t.isolate_cmd(true, i2.clone(), cx)));
            let f4 = feats.clone();
            m.push(item("Suppress", move |t, _, cx| {
                t.edit("Suppress", cx, |a| {
                    for x in &f4 {
                        if let Some(mm) = a.mates.iter_mut().find(|mm| mm.id == *x) {
                            mm.suppressed = true;
                        }
                    }
                });
            }));
            m.push(MenuEntry::Sep);
            if let Some(sel) = self.selected_mate.filter(|s| !feats.contains(s)) {
                m.push(item("Add selection to folder…", move |t, _, cx| {
                    t.edit("Add selection to folder", cx, |a| {
                        for x in &mut a.folders {
                            x.features.retain(|y| *y != sel);
                        }
                        if let Some(x) = a.folders.iter_mut().find(|x| x.id == id) {
                            x.features.push(sel);
                        }
                    });
                }));
            }
            m.push(item("Expand", move |t, _, cx| {
                t.ui.expanded_mates.insert(id);
                cx.notify();
            }));
            m.push(item("Collapse", move |t, _, cx| {
                t.ui.expanded_mates.remove(&id);
                cx.notify();
            }));
        } else {
            let insts = f.instances.clone();
            let (a1, a2, a3, a4, a5) = (insts.clone(), insts.clone(), insts.clone(), insts.clone(), insts.clone());
            let hidden = self.assembly(cx).is_some_and(|a| !insts.is_empty() && insts.iter().all(|i| a.instance(*i).is_some_and(|x| x.hidden)));
            m.push(icon_item(if hidden { "Show" } else { "Hide" }, "eye-off", move |t, _, cx| t.set_hidden(a1.clone(), !hidden, cx)));
            m.push(item("Hide other instances", move |t, _, cx| t.hide_other_instances(a2.clone(), cx)));
            m.push(item("Hide all instances", |t, _, cx| t.hide_all_instances(cx)));
            m.push(item("Isolate…", move |t, _, cx| t.isolate_cmd(false, a3.clone(), cx)));
            m.push(item("Make transparent…", move |t, _, cx| t.isolate_cmd(true, a4.clone(), cx)));
            let suppressed = self.assembly(cx).is_some_and(|a| !insts.is_empty() && insts.iter().all(|i| a.instance(*i).is_some_and(|x| x.suppressed)));
            m.push(item(if suppressed { "Unsuppress" } else { "Suppress" }, move |t, _, cx| t.set_suppressed(a5.clone(), !suppressed, cx)));
            m.push(MenuEntry::Sep);
            let sel: Vec<InstanceId> = self.selected.iter().copied().filter(|s| !insts.contains(s)).collect();
            if !sel.is_empty() {
                m.push(item("Add selection to folder…", move |t, _, cx| {
                    t.edit("Add selection to folder", cx, |a| {
                        for x in &mut a.folders {
                            x.instances.retain(|y| !sel.contains(y));
                        }
                        if let Some(x) = a.folders.iter_mut().find(|x| x.id == id) {
                            x.instances.extend(sel.iter().copied());
                        }
                    });
                }));
            }
            let key = vec![InstanceId(id.0)];
            let k2 = key.clone();
            m.push(item("Expand", move |t, _, cx| {
                t.expanded.insert(key.clone());
                cx.notify();
            }));
            m.push(item("Collapse", move |t, _, cx| {
                t.expanded.remove(&k2);
                cx.notify();
            }));
        }
        m.push(MenuEntry::Sep);
        m.push(icon_item("Delete", "delete", move |t, _, cx| {
            t.edit("Delete folder", cx, |a| a.delete_folder(id));
        }));
        m.push(item("Unpack folder", move |t, _, cx| {
            t.edit("Unpack folder", cx, |a| a.unpack_folder(id));
        }));
        m
    }

    // ------------------------------------------------------------ subassemblies

    /// Lock/follow position to: lock a subassembly at its current position or
    /// a named position (rigid), or unlock it (flexible).
    pub(super) fn set_sub_lock(&mut self, id: InstanceId, lock: SubassemblyLock, cx: &mut Context<Self>) {
        let name = self.assembly(cx).and_then(|a| a.instance(id).map(|i| i.name.clone())).unwrap_or_default();
        self.message = Some((
            false,
            match &lock {
                SubassemblyLock::Flexible => "Assembly is unlocked from named position.".to_string(),
                SubassemblyLock::Current => format!("{name} locked at current position."),
                SubassemblyLock::Named(n) => format!("{name} locked at named position {n}."),
            },
        ));
        self.edit(&format!("Lock/follow position : {name}"), cx, |a| {
            if let Some(i) = a.instance_mut(id) {
                i.lock = lock;
            }
        });
    }

    /// Structural edit touching several tabs, with the solved placements folded in.
    fn edit_doc(&mut self, desc: &str, cx: &mut Context<Self>, f: impl FnOnce(&mut noshape_model::Document, TabId)) {
        let solved = self.result.solved.placements.clone();
        let tab = self.tab;
        self.doc.update(cx, |d, cx| {
            d.edit_document(desc, cx, |doc| {
                if let Some(TabContent::Assembly(a)) = doc.tab_mut(tab).map(|t| &mut t.content) {
                    for (id, p) in &solved {
                        if let Some(i) = a.instance_mut(*id) {
                            i.placement = *p;
                        }
                    }
                }
                f(doc, tab)
            })
        });
    }

    /// Move to new subassembly: a new Assembly tab holding the instances
    /// (and their mates), inserted here in their place.
    pub(super) fn move_to_new_subassembly(&mut self, ids: Vec<InstanceId>, cx: &mut Context<Self>) {
        let ids: Vec<InstanceId> = self.assembly(cx).map(|a| ids.into_iter().filter(|id| a.generated_by(*id).is_none()).collect()).unwrap_or_default();
        if ids.is_empty() {
            return;
        }
        let mut new_id = None;
        self.edit_doc("Move to new subassembly", cx, |doc, tab| {
            let name = noshape_model::assembly::ops::next_assembly_tab_name(doc);
            let sub_tab = doc.add_tab(&name, TabContent::Assembly(Assembly::default()));
            let Some(TabContent::Assembly(parent)) = doc.tab_mut(tab).map(|t| &mut t.content) else { return };
            let (sub, sid) = parent.move_to_new_subassembly(&ids, sub_tab, &name);
            new_id = Some(sid);
            if let Some(t) = doc.tab_mut(sub_tab) {
                t.content = TabContent::Assembly(sub);
            }
        });
        if let Some(id) = new_id {
            self.selected = vec![id];
            self.selected_mate = None;
        }
    }

    /// Drag and drop onto a subassembly row: the instances move into it.
    pub(super) fn move_into_subassembly(&mut self, ids: Vec<InstanceId>, sub_inst: InstanceId, cx: &mut Context<Self>) {
        let Some(InstanceSource::Assembly { assembly: sub_tab }) = self.assembly(cx).and_then(|a| a.instance(sub_inst)).map(|i| i.source.clone()) else { return };
        let doc = self.doc.read(cx).session.doc.clone();
        // Never move a subassembly into itself or one that contains it.
        if ids.iter().any(|id| self.assembly(cx).and_then(|a| a.instance(*id)).is_some_and(|i| matches!(i.source, InstanceSource::Assembly { assembly } if contains_assembly(&doc, assembly, sub_tab, 0)))) {
            self.message = Some((true, "Cannot move the instance into the subassembly because it would create cyclical references".into()));
            cx.notify();
            return;
        }
        let mut moved = Vec::new();
        self.edit_doc("Move to subassembly", cx, |doc, tab| {
            let Some(TabContent::Assembly(mut sub)) = doc.tab(sub_tab).map(|t| t.content.clone()) else { return };
            if let Some(TabContent::Assembly(parent)) = doc.tab_mut(tab).map(|t| &mut t.content) {
                moved = parent.move_into_subassembly(&ids, sub_inst, &mut sub);
            }
            if let Some(t) = doc.tab_mut(sub_tab) {
                t.content = TabContent::Assembly(sub);
            }
        });
        if !moved.is_empty() {
            self.selected = vec![sub_inst];
            self.expanded.insert(vec![sub_inst]);
        }
    }

    /// Create empty subassembly: a new, empty Assembly tab inserted here.
    pub(super) fn create_empty_subassembly(&mut self, cx: &mut Context<Self>) {
        let mut new_id = None;
        self.edit_doc("Create empty subassembly", cx, |doc, tab| {
            let name = noshape_model::assembly::ops::next_assembly_tab_name(doc);
            let sub_tab = doc.add_tab(&name, TabContent::Assembly(Assembly::default()));
            let Some(TabContent::Assembly(parent)) = doc.tab_mut(tab).map(|t| &mut t.content) else { return };
            let id = InstanceId(parent.next_id());
            let n = parent.next_instance_name(&name);
            parent.instances.push(Instance { id, name: n, source: InstanceSource::Assembly { assembly: sub_tab }, placement: Placement::default(), fixed: false, hidden: false, suppressed: false, lock: SubassemblyLock::Flexible, nested: Vec::new() });
            new_id = Some(id);
        });
        if let Some(id) = new_id {
            self.selected = vec![id];
        }
    }

    /// Dissolve subassembly: its instances and mates move up into this
    /// assembly; the subassembly tab is left empty when nothing else uses it.
    pub(super) fn dissolve_subassemblies(&mut self, ids: Vec<InstanceId>, cx: &mut Context<Self>) {
        if ids.is_empty() {
            return;
        }
        let mut created = Vec::new();
        self.edit_doc("Dissolve subassembly", cx, |doc, tab| {
            for sid in &ids {
                let Some(TabContent::Assembly(parent)) = doc.tab(tab).map(|t| &t.content) else { return };
                let Some(InstanceSource::Assembly { assembly: sub_tab }) = parent.instance(*sid).map(|i| i.source.clone()) else { continue };
                let Some(TabContent::Assembly(sub)) = doc.tab(sub_tab).map(|t| t.content.clone()) else { continue };
                if let Some(TabContent::Assembly(parent)) = doc.tab_mut(tab).map(|t| &mut t.content) {
                    created.extend(parent.dissolve_subassembly(*sid, &sub));
                }
                let used_elsewhere = doc.tabs.iter().any(|t| match &t.content {
                    TabContent::Assembly(a) => a.instances.iter().any(|i| matches!(i.source, InstanceSource::Assembly { assembly } if assembly == sub_tab)),
                    _ => false,
                });
                if !used_elsewhere && let Some(t) = doc.tab_mut(sub_tab) {
                    t.content = TabContent::Assembly(Assembly::default());
                }
            }
        });
        self.selected = created;
    }

    /// Top-level assembly row menu.
    pub(super) fn assembly_menu(&self, cx: &App) -> Vec<MenuEntry> {
        let any_fixed = self.assembly(cx).is_some_and(|a| a.instances.iter().any(|i| i.fixed));
        let mut m = vec![
            item("Rename", |t, w, cx| t.rename_tab(w, cx)),
            item("Check interference…", |t, _, cx| {
                t.selected.clear();
                t.run_interference(cx);
            }),
            item("Add exploded view…", |t, _, cx| {
                t.panel = Some(SidePanel::Explode);
                t.add_exploded_view(false, cx);
            }),
        ];
        if any_fixed {
            m.push(item("Unfix all", |t, _, cx| t.unfix_all(cx)));
        }
        m.push(MenuEntry::Sep);
        m.push(item("Expand all", |t, _, cx| t.expand_all(true, cx)));
        m.push(item("Collapse all", |t, _, cx| t.expand_all(false, cx)));
        m
    }

    /// Mate (and relation / group) context menu.
    pub(super) fn mate_menu(&self, id: MateId, cx: &App) -> Vec<MenuEntry> {
        let Some(asm) = self.assembly(cx) else { return vec![] };
        let mate = asm.mate(id).cloned();
        let suppressed = mate.as_ref().map(|m| m.suppressed).or_else(|| asm.relations.iter().find(|r| r.id == id).map(|r| r.suppressed)).or_else(|| asm.groups.iter().find(|g| g.id == id).map(|g| g.suppressed)).unwrap_or(false);
        let isolating = self.isolate.is_some() || self.ui.transparent.is_some();
        let mut m: Vec<MenuEntry> = Vec::new();
        if isolating {
            m.push(item("Exit isolate", |t, _, cx| {
                t.exit_isolate(cx);
            }));
            m.push(MenuEntry::Sep);
        }
        m.push(item("Rename", move |t, w, cx| t.start_row_rename(RenameRow::Feature(id), w, cx)));
        m.push(icon_item("Edit…", "edit", move |t, w, cx| t.edit_mate(id, w, cx)));
        if let Some(mt) = &mate {
            let keys: Vec<&'static str> = noshape_model::assembly::eval::limit_keys(mt.mate_type).iter().copied().filter(|k| *k != "cone" && mt.limits.iter().any(|(x, _)| x == k)).collect();
            if !keys.is_empty() {
                let mut children = Vec::new();
                for k in keys {
                    let axis = match k {
                        "rz" => "Z angle",
                        "x" => "X",
                        "y" => "Y",
                        _ => "Z",
                    };
                    children.push(item(format!("Min {axis} limit"), move |t, _, cx| t.apply_limit_position(id, k, false, cx)));
                    children.push(item(format!("Max {axis} limit"), move |t, _, cx| t.apply_limit_position(id, k, true, cx)));
                }
                m.push(MenuEntry::Sub { label: "Apply limit position".into(), children });
            }
            if mt.mate_type.dof() > 0 && !matches!(mt.mate_type, MateType::Tangent | MateType::Width) {
                m.push(item("Reset", move |t, _, cx| t.reset_mate(id, cx)));
                m.push(icon_item(format!("Animate {}…", mt.name), "animate", move |t, w, cx| {
                    t.selected_mate = Some(id);
                    t.start_animation(w, cx);
                }));
            }
            let hidden = mt.hidden || !self.show_mates;
            m.push(MenuEntry::Sep);
            m.push(icon_item(if hidden { "Show" } else { "Hide" }, "eye-off", move |t, _, cx| t.set_mates_hidden(vec![id], !hidden, cx)));
            m.push(item("Show all mates", |t, _, cx| t.show_all_mates(cx)));
        }
        let ids = self.mate_instances(id, cx);
        let ids2 = ids.clone();
        m.push(item("Isolate…", move |t, _, cx| t.isolate_cmd(false, ids.clone(), cx)));
        m.push(item("Make transparent…", move |t, _, cx| t.isolate_cmd(true, ids2.clone(), cx)));
        m.push(item(if suppressed { "Unsuppress" } else { "Suppress" }, move |t, _, cx| t.toggle_mate_suppressed(id, cx)));
        m.push(MenuEntry::Sep);
        m.push(icon_item("Add selection to folder…", "folder", move |t, w, cx| t.folder_from_features(vec![id], w, cx)));
        m.push(item("Clear selection", |t, _, cx| {
            t.selected.clear();
            t.selected_mate = None;
            t.refresh_scene(false, cx);
        }));
        m.push(MenuEntry::Sep);
        m.push(icon_item("Delete", "delete", move |t, _, cx| t.delete_mate(id, cx)));
        m
    }

    /// Assembly mate connector context menu.
    pub(super) fn connector_menu(&self, id: MateId, cx: &App) -> Vec<MenuEntry> {
        let Some(c) = self.assembly(cx).and_then(|a| a.connector(id).cloned()) else { return vec![] };
        let hidden = c.hidden;
        vec![
            item("Rename", move |t, w, cx| t.start_row_rename(RenameRow::Feature(id), w, cx)),
            icon_item("Edit…", "edit", move |t, w, cx| t.edit_mate(id, w, cx)),
            MenuEntry::Sep,
            icon_item(if hidden { "Show" } else { "Hide" }, "eye-off", move |t, _, cx| {
                t.edit(if hidden { "Show mate connector" } else { "Hide mate connector" }, cx, |a| a.set_mates_hidden(&[id], !hidden));
            }),
            item("Hide other mate connectors", move |t, _, cx| {
                t.edit("Hide other mate connectors", cx, |a| {
                    for x in &mut a.connectors {
                        x.hidden = x.id != id;
                    }
                });
            }),
            item("Show all mate connectors", |t, _, cx| t.show_all_connectors(cx)),
            item(if c.suppressed { "Unsuppress" } else { "Suppress" }, move |t, _, cx| {
                t.edit("Suppress mate connector", cx, |a| {
                    if let Some(x) = a.connectors.iter_mut().find(|x| x.id == id) {
                        x.suppressed = !x.suppressed;
                    }
                });
            }),
            MenuEntry::Sep,
            icon_item("Delete", "delete", move |t, _, cx| t.delete_mate(id, cx)),
        ]
    }

    /// Graphics area context menu (instance under the cursor, or the
    /// general menu on empty space).
    pub(super) fn open_graphics_menu(&mut self, top: Option<InstanceId>, position: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let entries = match top {
            Some(t) => self.instance_menu(t, true, cx),
            None => {
                let any_hidden = self.assembly(cx).is_some_and(|a| a.instances.iter().any(|i| i.hidden));
                let mut m = Vec::new();
                if self.isolate.is_some() || self.ui.transparent.is_some() {
                    m.push(item("Exit isolate", |t, _, cx| {
                        t.exit_isolate(cx);
                    }));
                    m.push(MenuEntry::Sep);
                }
                if any_hidden {
                    m.push(item("Show all", |t, _, cx| t.show_hidden(cx)));
                }
                m.push(item("Zoom to fit", |t, _, cx| t.viewport.update(cx, |vp, cx| vp.zoom_to_fit(true, cx))));
                m.push(item("Isometric", |t, _, cx| t.view(StandardView::Isometric, cx)));
                if !self.selected.is_empty() || self.selected_mate.is_some() {
                    m.push(item("Clear selection", |t, _, cx| {
                        t.selected.clear();
                        t.selected_mate = None;
                        t.refresh_scene(false, cx);
                    }));
                }
                m
            }
        };
        if entries.is_empty() {
            return;
        }
        let view = cx.entity();
        let menu = PopupMenu::build(window, cx, move |m, window, cx| fill_menu(m, entries.clone(), view.clone(), window, cx));
        let sub = cx.subscribe(&menu, |this: &mut Self, _, _: &DismissEvent, cx| {
            this.ui.ctx_menu = None;
            cx.notify();
        });
        menu.update(cx, |m, cx| window.focus(&m.focus_handle(cx), cx));
        self.ui.ctx_menu = Some((position, menu, sub));
        cx.notify();
    }

    /// Mate indicator context menu in the graphics area.
    pub(super) fn open_mate_menu(&mut self, id: MateId, position: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let entries = self.mate_menu(id, cx);
        let view = cx.entity();
        let menu = PopupMenu::build(window, cx, move |m, window, cx| fill_menu(m, entries.clone(), view.clone(), window, cx));
        let sub = cx.subscribe(&menu, |this: &mut Self, _, _: &DismissEvent, cx| {
            this.ui.ctx_menu = None;
            cx.notify();
        });
        menu.update(cx, |m, cx| window.focus(&m.focus_handle(cx), cx));
        self.ui.ctx_menu = Some((position, menu, sub));
        self.refresh_scene(false, cx);
        cx.notify();
    }

    pub(super) fn render_graphics_menu(&self) -> Option<AnyElement> {
        let (pos, menu, _) = self.ui.ctx_menu.as_ref()?;
        Some(deferred(anchored().position(*pos).snap_to_window_with_margin(px(8.)).child(menu.clone())).with_priority(3).into_any_element())
    }

    pub(super) fn expand_all(&mut self, expand: bool, cx: &mut Context<Self>) {
        if expand {
            fn walk(nodes: &[InstanceNode], out: &mut HashSet<Vec<InstanceId>>) {
                for n in nodes {
                    if !n.children.is_empty() {
                        out.insert(n.path.clone());
                        walk(&n.children, out);
                    }
                }
            }
            let mut set = HashSet::new();
            walk(&self.result.nodes, &mut set);
            if let Some(a) = self.assembly(cx) {
                for p in &a.patterns {
                    set.insert(vec![InstanceId(p.id.0)]);
                }
                self.ui.expanded_mates.extend(a.mates.iter().map(|m| m.id));
            }
            self.expanded = set;
        } else {
            self.expanded.clear();
            self.ui.expanded_mates.clear();
        }
        cx.notify();
    }

    /// Rename the assembly tab (top-level assembly row > Rename).
    pub(super) fn rename_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.doc.read(cx).session.doc.tab(self.tab).map(|t| t.name.clone()).unwrap_or_default();
        let st = cx.new(|cx| InputState::new(window, cx).default_value(name));
        let st2 = st.clone();
        cx.spawn_in(window, async move |_, cx| {
            cx.background_executor().timer(Duration::from_millis(40)).await;
            let _ = st2.update_in(cx, |s, window, cx| {
                s.focus(window, cx);
                s.select_all(window, cx);
            });
        })
        .detach();
        let sub = cx.subscribe_in(&st, window, |this: &mut Self, st, ev: &InputEvent, window, cx| {
            if matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                let name = st.read(cx).value().trim().to_string();
                let tab = this.tab;
                if !name.is_empty() {
                    this.doc.update(cx, |d, cx| {
                        d.edit_document(&format!("Rename tab : {name}"), cx, |doc| {
                            if let Some(t) = doc.tab_mut(tab) {
                                t.name = name.clone();
                            }
                        })
                    });
                }
                this.ui.rename = None;
                window.focus(&this.focus, cx);
                cx.notify();
            }
        });
        self.ui.rename = Some((RenameRow::Instance(InstanceId::ORIGIN), st, sub));
        cx.notify();
    }

    // ------------------------------------------------------------ list filter

    pub(super) fn ensure_filter_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.ui.filter_input.is_some() {
            return;
        }
        let st = cx.new(|cx| InputState::new(window, cx).placeholder("Filter by name or type"));
        let sub = cx.subscribe(&st, |this: &mut Self, st, ev: &InputEvent, cx| {
            if matches!(ev, InputEvent::Change) {
                this.ui.filter = ListFilter::parse(&st.read(cx).value());
                cx.notify();
            }
        });
        self.ui.filter_input = Some(st);
        self.ui.filter_sub = Some(sub);
        let st = cx.new(|cx| InputState::new(window, cx).placeholder("Search by name"));
        self.ui.insert_search_sub = Some(cx.subscribe(&st, |_: &mut Self, _, ev: &InputEvent, cx| {
            if matches!(ev, InputEvent::Change) {
                cx.notify();
            }
        }));
        self.ui.insert_search = Some(st);
    }

    /// i / toolbar Insert: open the Insert dialog, or close it when open.
    pub(super) fn toggle_insert(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.insert_open {
            self.close_insert(window, cx);
        } else {
            self.close_mate(window, cx);
            self.insert_open = true;
            cx.notify();
        }
    }

    /// Close the Insert dialog (checkmark / Esc): an instance still following
    /// the cursor stays where it is.
    pub(super) fn close_insert(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.placing.is_some() {
            self.finish_placing(true, window, cx);
        }
        self.insert_open = false;
        self.ext_pick = None;
        cx.notify();
    }

    pub(super) fn set_list_filter(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(st) = &self.ui.filter_input {
            st.update(cx, |s, cx| s.set_value(text.to_string(), window, cx));
        }
        self.ui.filter = ListFilter::parse(text);
        cx.notify();
    }

    /// Filter box with its colon-command dropdown (top of the instance list).
    pub(super) fn render_list_filter(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let Some(st) = self.ui.filter_input.clone() else { return div().into_any_element() };
        let view = cx.entity();
        let current = self.ui.filter.clone();
        // Onshape filter row: funnel button, then a 28 px "Filter by name or type" box.
        let _ = pal;
        h_flex()
            .h(px(34.))
            .flex_none()
            .pl(px(2.))
            .pr(px(5.))
            .gap(px(2.))
            .child(
                Button::new("asm-filter-cmds").ghost().child(crate::assets::icon("filter", 20., cx)).w(px(26.)).h(px(26.)).p_0().tooltip("Filter").dropdown_menu(move |mut menu, _, _| {
                    for c in FilterCommand::ALL {
                        let v = view.clone();
                        let current = current.clone();
                        menu = menu.item(PopupMenuItem::new(format!("{}  {}", c.label(), c.token())).checked(current.commands.contains(&c)).on_click(move |_, window, cx| {
                            v.update(cx, |this, cx| {
                                let text = this.ui.filter_input.as_ref().map(|s| s.read(cx).value().to_string()).unwrap_or_default();
                                let mut words: Vec<String> = text.split_whitespace().map(|w| w.to_string()).collect();
                                if let Some(i) = words.iter().position(|w| w.eq_ignore_ascii_case(c.token())) {
                                    words.remove(i);
                                } else {
                                    words.insert(0, c.token().to_string());
                                }
                                this.set_list_filter(&words.join(" "), window, cx);
                            })
                        }));
                    }
                    menu
                }),
            )
            .child(div().flex_1().child(Input::new(&st).small().h(px(28.)).rounded(px(2.)).cleanable(true)))
            .into_any_element()
    }

    /// Does an instance row pass the list filter?
    pub(super) fn instance_passes(&self, inst: &Instance, node: Option<&InstanceNode>) -> bool {
        let f = &self.ui.filter;
        if f.is_empty() {
            return true;
        }
        if !f.wants_instances() {
            return false;
        }
        let kind = match inst.source {
            InstanceSource::Assembly { .. } => RowKind::Assembly,
            _ => RowKind::Part,
        };
        f.matches(&RowInfo { kind, name: &inst.name, error: node.is_some_and(|n| n.error.is_some()), warning: false, suppressed: inst.suppressed, hidden: inst.hidden, has_limits: false })
    }

    /// Does a mate feature row pass the list filter?
    pub(super) fn feature_passes(&self, kind: RowKind, name: &str, error: bool, suppressed: bool, hidden: bool, has_limits: bool) -> bool {
        let f = &self.ui.filter;
        if f.is_empty() {
            return true;
        }
        f.wants_mate_features() && f.matches(&RowInfo { kind, name, error, warning: false, suppressed, hidden, has_limits })
    }

    // ------------------------------------------------------------ S shortcut toolbar

    pub(super) fn toggle_mini_toolbar(&mut self, cx: &mut Context<Self>) {
        if self.ui.mini_toolbar.take().is_none() {
            self.ui.mini_toolbar = Some(self.pointer);
        }
        cx.notify();
    }

    pub(super) fn render_mini_toolbar(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let at = self.ui.mini_toolbar?;
        let pal = p();
        // Keep the toolbar inside the graphics area.
        let size = self.viewport.read(cx).size();
        let at = DVec2::new(at.x.min(size.x - 440.0 + 60.0).max(64.0), at.y.max(48.0));
        type Act = fn(&mut AssemblyView, &mut Window, &mut Context<AssemblyView>);
        let tools: [(&str, &str, Act); 14] = [
            ("Insert parts and assemblies (I)", "insert", |t, _, cx| {
                t.insert_open = true;
                cx.notify();
            }),
            ("Mate connector (Ctrl+M)", "mate-connector-assembly", |t, w, cx| t.open_mc(None, w, cx)),
            ("Fastened mate (M)", "fastened", |t, w, cx| t.start_mate(MateType::Fastened, w, cx)),
            ("Revolute mate", "revolute", |t, w, cx| t.start_mate(MateType::Revolute, w, cx)),
            ("Slider mate", "slider", |t, w, cx| t.start_mate(MateType::Slider, w, cx)),
            ("Planar mate", "planar", |t, w, cx| t.start_mate(MateType::Planar, w, cx)),
            ("Cylindrical mate", "cylindrical", |t, w, cx| t.start_mate(MateType::Cylindrical, w, cx)),
            ("Pin slot mate", "pin-slot", |t, w, cx| t.start_mate(MateType::PinSlot, w, cx)),
            ("Ball mate", "ball", |t, w, cx| t.start_mate(MateType::Ball, w, cx)),
            ("Parallel mate", "parallel-mate", |t, w, cx| t.start_mate(MateType::Parallel, w, cx)),
            ("Tangent mate", "tangent-mate", |t, w, cx| t.start_mate(MateType::Tangent, w, cx)),
            ("Group", "group", |t, _, cx| t.create_group(cx)),
            ("Linear pattern", "linear-pattern", |t, w, cx| t.open_pattern(PatternDialogKind::Linear, w, cx)),
            ("Replicate", "replicate", |t, w, cx| t.open_pattern(PatternDialogKind::Replicate, w, cx)),
        ];
        let mut row = h_flex().gap_0p5();
        for (tip, icon, act) in tools {
            row = row.child(
                div()
                    .id(SharedString::from(format!("asm-mini-{icon}")))
                    .size(px(30.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_md()
                    .cursor_pointer()
                    .hover(|d| d.bg(pal.panel_hover))
                    .child(svg().path(icon_path(icon)).size(px(18.)).text_color(pal.text))
                    .tooltip(move |window, cx| gpui_kit::component::tooltip::Tooltip::new(tip).build(window, cx))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _: &MouseDownEvent, window, cx| {
                            cx.stop_propagation();
                            this.ui.mini_toolbar = None;
                            act(this, window, cx);
                            cx.notify();
                        }),
                    ),
            );
        }
        Some(
            div()
                .id("asm-mini-toolbar")
                .occlude()
                .absolute()
                .left(px((at.x as f32 - 60.0).max(4.0)))
                .top(px((at.y as f32 - 44.0).max(4.0)))
                .p_1()
                .rounded_lg()
                .bg(pal.panel_raised)
                .border_1()
                .border_color(pal.border_strong)
                .shadow_lg()
                .child(row)
                .into_any_element(),
        )
    }

    // ------------------------------------------------------------ shortcut list

    pub(super) fn render_shortcuts(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.ui.show_shortcuts {
            return None;
        }
        let pal = p();
        let rows: &[(&str, &str)] = &[
            ("i", "Insert parts and assemblies"),
            ("m", "Fastened mate"),
            ("Ctrl+M", "Mate connector"),
            ("j", "Show or hide mates"),
            ("k", "Show or hide mate connectors"),
            ("h", "Show mates mode"),
            ("Shift+S", "Snap mode"),
            ("a", "Flip primary axis (mate / mate connector / snap drag)"),
            ("q", "Reorient secondary axis (snap drag)"),
            ("Shift (hold)", "Lock mate inference"),
            ("Ctrl (tap)", "Cycle the dragged mate connector (snap mode)"),
            ("Enter / Shift+Enter", "Commit / commit and start another"),
            ("Esc", "Cancel"),
            ("Space", "Clear selection"),
            ("Delete", "Delete selection"),
            ("Shift+N", "Rename selection"),
            ("y / Shift+Y", "Hide hovered or selected / show hidden"),
            ("Shift+I / Shift+T", "Isolate / make transparent"),
            ("Ctrl+C / Ctrl+V", "Copy / paste instances"),
            ("s", "Shortcut toolbar"),
            ("Shift+/", "Keyboard shortcuts"),
        ];
        let mut list = v_flex().gap_0p5();
        for (k, d) in rows {
            list = list.child(h_flex().gap_3().text_xs().child(div().w(px(130.)).text_color(pal.accent_hover).child(k.to_string())).child(div().text_color(pal.text).child(d.to_string())));
        }
        Some(
            v_flex()
                .id("asm-shortcuts")
                .occlude()
                .absolute()
                .top(px(40.))
                .left(relative(0.5))
                .ml(px(-210.))
                .w(px(420.))
                .p_3()
                .gap_2()
                .rounded_lg()
                .bg(pal.panel_raised)
                .border_1()
                .border_color(pal.border_strong)
                .shadow_lg()
                .child(h_flex().justify_between().child(div().text_sm().font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child("Assembly keyboard shortcuts")).child(Button::new("asm-sc-close").icon(IconName::Close).ghost().xsmall().on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.ui.show_shortcuts = false;
                    cx.notify();
                }))))
                .child(list)
                .into_any_element(),
        )
    }
}

// ---------------------------------------------------------------- Animate mate

/// Animate playback type (Onshape: Single, Reciprocate, Loop).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Playback {
    Single,
    Reciprocate,
    Loop,
}

/// Animate mate dialog state.
pub(crate) struct AnimState {
    pub mate: MateId,
    /// Driven free value (0 = Rz, 1..3 = Tx/Ty/Tz).
    pub dof: usize,
    pub start: Entity<InputState>,
    pub end: Entity<InputState>,
    pub steps: Entity<InputState>,
    /// Position along start..end (0..1) and direction.
    pub t: f64,
    pub dir: f64,
    pub playing: bool,
    pub playback: Playback,
    pub busy: bool,
    pub current: f64,
    pub task: Option<Task<()>>,
    /// Preview from the mate dialog's Animate button (no Animate dialog).
    pub preview: bool,
}

impl AnimState {
    pub fn angular(&self) -> bool {
        self.dof == 0
    }
}

impl AssemblyView {
    fn anim_defaults(&self, m: &Mate, dof: usize, cx: &App) -> (String, String) {
        let units = self.units(cx);
        let key = match dof {
            0 => "rz",
            1 => "x",
            2 => "y",
            _ => "z",
        };
        let fmt = |v: f64| {
            let q = if dof == 0 { noshape_units::Quantity::radians(v) } else { noshape_units::Quantity::meters(v) };
            noshape_units::format_quantity(q, &units)
        };
        // Start / End default to the mate's limits (Onshape), else a full turn / a span.
        if let Some(l) = m.limits.iter().find(|(k, _)| k == key).map(|(_, l)| l.clone())
            && let (Some(a), Some(b)) = (l.min, l.max)
        {
            return (a.text, b.text);
        }
        if dof == 0 {
            (fmt(0.0), fmt(std::f64::consts::TAU))
        } else {
            let size = self.result.bounds().map(|(a, b)| (b - a).length()).unwrap_or(0.1);
            (fmt(0.0), fmt(size * 0.5))
        }
    }

    /// Mate (or mate indicator) context menu > Animate: open the Animate
    /// dialog for the selected mate (paused; Play starts it).
    pub(super) fn start_animation(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(asm) = self.assembly(cx) else { return };
        let animatable = |m: &&Mate| !noshape_model::assembly::eval::mate_dofs(m.mate_type).is_empty();
        let pick = self.selected_mate.and_then(|id| asm.mate(id)).filter(animatable).or_else(|| asm.mates.iter().filter(|m| !m.suppressed).find(animatable));
        let Some(m) = pick.cloned() else {
            self.message = Some((false, "Select a mate with degrees of freedom to animate.".into()));
            cx.notify();
            return;
        };
        self.stop_animation(window, cx);
        let dof = noshape_model::assembly::eval::mate_dofs(m.mate_type)[0].0;
        let (a, b) = self.anim_defaults(&m, dof, cx);
        let mk = |t: String, window: &mut Window, cx: &mut Context<Self>| cx.new(|cx| InputState::new(window, cx).default_value(t));
        let start = mk(a, window, cx);
        let end = mk(b, window, cx);
        let steps = mk("60".into(), window, cx);
        let current = self.result.solved.mate_values.get(&m.id).map(|v| v[dof]).unwrap_or(0.0);
        self.selected_mate = Some(m.id);
        self.anim = Some(AnimState { mate: m.id, dof, start, end, steps, t: 0.0, dir: 1.0, playing: false, playback: Playback::Single, busy: false, current, task: None, preview: false });
        cx.notify();
    }

    /// Mate dialog Animate button: preview the mate's first degree of
    /// freedom back and forth until pressed again or the dialog closes.
    pub(super) fn toggle_mate_preview(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.anim.as_ref().is_some_and(|a| a.preview) {
            self.anim = None;
            self.schedule_eval(window, cx);
            return;
        }
        let Some(m) = self.mate.as_ref().and_then(|s| s.dialog.read(cx).to_mate(cx)) else {
            self.message = Some((false, "Select two mate connectors first.".into()));
            if let Some(d) = self.mate.as_ref().map(|s| s.dialog.clone()) {
                d.update(cx, |d, cx| {
                    d.animating = false;
                    cx.notify();
                });
            }
            cx.notify();
            return;
        };
        let Some(&(dof, _)) = noshape_model::assembly::eval::mate_dofs(m.mate_type).first() else { return };
        let (a, b) = self.anim_defaults(&m, dof, cx);
        let mk = |t: String, window: &mut Window, cx: &mut Context<Self>| cx.new(|cx| InputState::new(window, cx).default_value(t));
        let start = mk(a, window, cx);
        let end = mk(b, window, cx);
        let steps = mk("90".into(), window, cx);
        self.anim = Some(AnimState { mate: m.id, dof, start, end, steps, t: 0.0, dir: 1.0, playing: false, playback: Playback::Reciprocate, busy: false, current: 0.0, task: None, preview: true });
        self.play_animation(cx);
    }

    fn set_anim_dof(&mut self, dof: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(mid) = self.anim.as_ref().map(|a| a.mate) else { return };
        let Some(m) = self.assembly(cx).and_then(|a| a.mate(mid).cloned()) else { return };
        let (a, b) = self.anim_defaults(&m, dof, cx);
        if let Some(an) = &mut self.anim {
            an.dof = dof;
            an.t = 0.0;
            an.start.update(cx, |s, cx| s.set_value(a, window, cx));
            an.end.update(cx, |s, cx| s.set_value(b, window, cx));
        }
        cx.notify();
    }

    /// Start / end values and step count from the dialog fields.
    fn anim_range(&self, cx: &App) -> Option<(f64, f64, u32)> {
        let a = self.anim.as_ref()?;
        let units = self.units(cx);
        let dim = if a.angular() { noshape_units::Dim::ANGLE } else { noshape_units::Dim::LENGTH };
        let ev = |st: &Entity<InputState>| noshape_units::evaluate_as(&st.read(cx).value(), dim, &units, &noshape_units::EmptyScope).ok().map(|q| q.value);
        let steps = a.steps.read(cx).value().trim().parse::<u32>().ok().unwrap_or(60).max(2);
        // Up to 100 revolutions.
        let lim = |v: f64| if a.angular() { v.clamp(-100.0 * std::f64::consts::TAU, 100.0 * std::f64::consts::TAU) } else { v };
        Some((lim(ev(&a.start)?), lim(ev(&a.end)?), steps))
    }

    pub(super) fn play_animation(&mut self, cx: &mut Context<Self>) {
        let Some((start, end, steps)) = self.anim_range(cx) else {
            self.message = Some((true, "Enter valid start and end values.".into()));
            cx.notify();
            return;
        };
        let Some(a) = &mut self.anim else { return };
        if a.playback == Playback::Single && ((a.dir > 0.0 && a.t >= 1.0) || (a.dir < 0.0 && a.t <= 0.0)) {
            // Single playback finished: play again from the start.
            a.t = if a.dir > 0.0 { 0.0 } else { 1.0 };
        }
        a.playing = true;
        // About 60 steps per second; more steps play slower.
        let step = 1.0 / (steps - 1) as f64;
        a.task = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(16)).await;
                let job = this.update(cx, |this: &mut Self, cx| {
                    let a = this.anim.as_mut()?;
                    if !a.playing || a.busy {
                        return a.playing.then_some(None);
                    }
                    a.t += a.dir * step;
                    if a.t > 1.0 + 1e-9 || a.t < -1e-9 {
                        match a.playback {
                            Playback::Single => {
                                a.t = a.t.clamp(0.0, 1.0);
                                a.playing = false;
                                a.task = None;
                                cx.notify();
                                return None;
                            }
                            Playback::Loop => a.t = if a.dir > 0.0 { 0.0 } else { 1.0 },
                            Playback::Reciprocate => {
                                a.dir = -a.dir;
                                a.t = a.t.clamp(0.0, 1.0);
                            }
                        }
                    }
                    let v = start + (end - start) * a.t;
                    a.current = v;
                    a.busy = true;
                    let (mate, dof) = (a.mate, a.dof);
                    let start: HashMap<InstanceId, Placement> = this.result.solved.placements.iter().cloned().collect();
                    Some(Some((this.result.prepared.clone(), SolveOptions { hold_values: vec![(mate, dof, v)], start: Some(start), nested_start: Some(this.result.solved.nested.clone()), ..Default::default() })))
                });
                let Ok(Some(job)) = job else { break };
                let Some((prepared, opts)) = job else { continue };
                let solved = cx.background_spawn(async move { prepared.solve(&opts) }).await;
                let ok = this.update(cx, |this: &mut Self, cx| {
                    if let Some(a) = &mut this.anim {
                        a.busy = false;
                    }
                    this.apply_solved(solved, cx);
                    this.rebuild_scene(true, cx);
                    cx.notify();
                });
                if ok.is_err() {
                    break;
                }
            }
        }));
        cx.notify();
    }

    /// Stop: the assembly stays at the current value.
    pub(super) fn stop_animation(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if let Some(a) = &mut self.anim {
            a.playing = false;
            a.task = None;
        }
        cx.notify();
    }

    pub(super) fn close_animation(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.anim = None;
        // Back to the document's positions (animation is not saved).
        self.schedule_eval(window, cx);
    }

    pub(super) fn render_anim_panel(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let Some(a) = &self.anim else { return div().into_any_element() };
        let Some(m) = self.assembly(cx).and_then(|x| x.mate(a.mate).cloned()) else { return div().into_any_element() };
        let units = self.units(cx);
        let q = if a.angular() { noshape_units::Quantity::radians(a.current) } else { noshape_units::Quantity::meters(a.current) };
        let current = noshape_units::format_quantity(q, &units);
        let (playing, pb, dof, reversed) = (a.playing, a.playback, a.dof, a.dir < 0.0);
        let dofs = noshape_model::assembly::eval::mate_dofs(m.mate_type);
        let row = |label: &'static str, el: AnyElement| h_flex().gap_2().child(div().w(px(96.)).text_xs().text_color(pal.text_muted).child(label)).child(div().flex_1().child(el));
        let mut body = v_flex().p_3().gap_2();
        if dofs.len() > 1 {
            let mut sel = h_flex().gap_1().flex_wrap();
            for (ix, label) in dofs {
                let ix = *ix;
                sel = sel.child(Button::new(SharedString::from(format!("anim-dof-{ix}"))).label(*label).ghost().xsmall().selected(dof == ix).on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.set_anim_dof(ix, window, cx))));
            }
            body = body.child(div().text_xs().text_color(pal.text_muted).child("Select degree of freedom")).child(sel);
        }
        let pb_btn = |id: &'static str, label: &'static str, v: Playback, cx: &mut Context<Self>| {
            Button::new(id).label(label).ghost().xsmall().selected(pb == v).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                if let Some(a) = &mut this.anim {
                    a.playback = v;
                }
                cx.notify();
            }))
        };
        body = body
            .child(row("Start value", Input::new(&a.start).xsmall().into_any_element()))
            .child(row("End value", Input::new(&a.end).xsmall().into_any_element()))
            .child(row("Steps", Input::new(&a.steps).xsmall().into_any_element()))
            .child(row(
                "Playback type",
                h_flex().gap_1().child(pb_btn("anim-single", "Single", Playback::Single, cx)).child(pb_btn("anim-recip", "Reciprocate", Playback::Reciprocate, cx)).child(pb_btn("anim-loop", "Loop", Playback::Loop, cx)).into_any_element(),
            ))
            .child(row("Current value", div().text_sm().child(current).into_any_element()))
            .child(
                h_flex()
                    .gap_1()
                    .child(Button::new("anim-reverse").icon(if reversed { IconName::ArrowLeft } else { IconName::ArrowRight }).ghost().small().selected(reversed).tooltip("Reverse").on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        if let Some(a) = &mut this.anim {
                            a.dir = -a.dir;
                        }
                        cx.notify();
                    })))
                    .child(div().flex_1())
                    .child(Button::new("anim-play").icon(if playing { IconName::Pause } else { IconName::Play }).small().primary().label(if playing { "Stop" } else { "Play" }).on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        if playing {
                            this.stop_animation(window, cx);
                        } else {
                            this.play_animation(cx);
                        }
                    }))),
            );
        Self::panel_frame(&format!("Animate {}", m.name), body, cx.listener(|this, _: &ClickEvent, window, cx| this.close_animation(window, cx)), "anim-close").into_any_element()
    }
}

// ---------------------------------------------------------------- display states, show mates, replace

impl AssemblyView {
    /// Relation dialog: a mate picked in the list or the graphics fills the
    /// first empty mate field (Screw: its single field).
    pub(super) fn pick_relation_mate(&mut self, id: MateId, cx: &mut Context<Self>) {
        let Some(r) = &mut self.relation else { return };
        if r.kind == RelationType::Screw {
            r.a = Some(id);
        } else if r.a.is_none() || (r.b.is_some() && r.a != Some(id)) {
            r.a = Some(id);
            if r.b == Some(id) {
                r.b = None;
            }
        } else if r.a != Some(id) {
            r.b = Some(id);
        }
        cx.notify();
    }

    /// h / toolbar: Show mates mode (hovered instance shows its mates, click pins).
    pub(super) fn toggle_show_mates_mode(&mut self, cx: &mut Context<Self>) {
        self.show_mates_mode = !self.show_mates_mode;
        self.mates_pinned = None;
        self.update_capture(cx);
        self.refresh_scene(false, cx);
        cx.notify();
    }

    /// Toolbar Replace instance: replaces the selected instance.
    pub(super) fn replace_selected(&mut self, cx: &mut Context<Self>) {
        match self.selected.first().copied() {
            Some(id) => self.open_replace(id, cx),
            None => {
                self.message = Some((false, "Select an instance to replace.".into()));
                cx.notify();
            }
        }
    }

    pub(super) fn toggle_display_states(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.ui.display_states = !self.ui.display_states;
        if self.ui.ds_name.is_none() {
            self.ui.ds_name = Some(cx.new(|cx| InputState::new(window, cx).placeholder("New display state name")));
        }
        cx.notify();
    }

    fn add_display_state(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(st) = self.ui.ds_name.clone() else { return };
        let name = st.read(cx).value().trim().to_string();
        if name.is_empty() {
            return;
        }
        if self.assembly(cx).is_some_and(|a| a.display_states.iter().any(|d| d.name == name)) {
            self.message = Some((true, "There is already a display state with this name.".into()));
            cx.notify();
            return;
        }
        self.edit(&format!("Add display state : {name}"), cx, |a| a.save_display_state(&name));
        self.ui.ds_selected = self.assembly(cx).map(|a| a.display_states.len().saturating_sub(1));
        st.update(cx, |s, cx| s.set_value("", window, cx));
        cx.notify();
    }

    pub(super) fn render_display_states(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let states: Vec<String> = self.assembly(cx).map(|a| a.display_states.iter().map(|d| d.name.clone()).collect()).unwrap_or_default();
        let sel = self.ui.ds_selected.filter(|i| *i < states.len());
        let mut body = v_flex().p_2().gap_1();
        if states.is_empty() {
            body = body.child(div().px_1().text_xs().text_color(pal.text_faint).child("No display states created"));
        }
        for (i, name) in states.iter().enumerate() {
            let renaming = sel == Some(i) && self.ui.ds_renaming.is_some();
            body = body.child(
                h_flex()
                    .id(SharedString::from(format!("ds-{i}")))
                    .h(px(24.))
                    .px_1()
                    .gap_1()
                    .rounded_sm()
                    .text_sm()
                    .cursor_pointer()
                    .when(sel == Some(i), |d| d.bg(pal.accent_soft))
                    .hover(|d| d.bg(pal.panel_hover))
                    .child(match (renaming, &self.ui.ds_renaming) {
                        (true, Some(st)) => div().flex_1().child(Input::new(st).xsmall()).into_any_element(),
                        _ => div().flex_1().truncate().child(name.clone()).into_any_element(),
                    })
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        // Selecting a display state applies it.
                        this.ui.ds_selected = Some(i);
                        this.ui.ds_renaming = None;
                        let name = this.assembly(cx).and_then(|a| a.display_states.get(i).map(|d| d.name.clone())).unwrap_or_default();
                        this.edit(&format!("Apply display state : {name}"), cx, |a| a.apply_display_state(i));
                        cx.notify();
                    })),
            );
        }
        let actions = h_flex()
            .gap_1()
            .child(Button::new("ds-update").label("Update").xsmall().ghost().tooltip("Update this display state").disabled(sel.is_none()).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                let Some(i) = sel else { return };
                let name = this.assembly(cx).and_then(|a| a.display_states.get(i).map(|d| d.name.clone())).unwrap_or_default();
                this.edit(&format!("Update display state : {name}"), cx, |a| a.save_display_state(&name));
                this.message = Some((false, "Display state updated.".into()));
                cx.notify();
            })))
            .child(Button::new("ds-rename").icon(Icon::empty().path(icon_path("edit"))).xsmall().ghost().tooltip("Rename display state").disabled(sel.is_none()).on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                let Some(i) = sel else { return };
                let name = this.assembly(cx).and_then(|a| a.display_states.get(i).map(|d| d.name.clone())).unwrap_or_default();
                let st = cx.new(|cx| InputState::new(window, cx).default_value(name));
                st.update(cx, |s, cx| s.focus(window, cx));
                let sub = cx.subscribe_in(&st, window, move |this: &mut Self, st, ev: &InputEvent, window, cx| {
                    if matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                        let new = st.read(cx).value().trim().to_string();
                        this.ui.ds_renaming = None;
                        if !new.is_empty() {
                            this.edit(&format!("Rename display state : {new}"), cx, |a| {
                                if let Some(d) = a.display_states.get_mut(i) {
                                    d.name = new.clone();
                                }
                            });
                        }
                        window.focus(&this.focus, cx);
                        cx.notify();
                    }
                });
                this.transient_sub = Some(sub);
                this.ui.ds_renaming = Some(st);
                cx.notify();
            })))
            .child(Button::new("ds-delete").icon(Icon::empty().path(icon_path("delete"))).xsmall().ghost().tooltip("Remove display state").disabled(sel.is_none()).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                let Some(i) = sel else { return };
                this.edit("Remove display state", cx, |a| {
                    if i < a.display_states.len() {
                        a.display_states.remove(i);
                    }
                });
                this.ui.ds_selected = None;
                this.message = Some((false, "Display state deleted.".into()));
                cx.notify();
            })));
        body = body.child(actions);
        if let Some(st) = self.ui.ds_name.clone() {
            body = body.child(
                h_flex()
                    .gap_1()
                    .child(div().flex_1().child(Input::new(&st).xsmall()))
                    .child(Button::new("ds-add").icon(IconName::Plus).xsmall().primary().tooltip("Add display state").on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.add_display_state(window, cx)))),
            );
        }
        Self::panel_frame("Display states", body, cx.listener(|this, _: &ClickEvent, _, cx| {
            this.ui.display_states = false;
            cx.notify();
        }), "ds-close")
        .into_any_element()
    }
}

// ---------------------------------------------------------------- Group dialog

impl AssemblyView {
    pub(super) fn open_group(&mut self, editing: Option<MateId>, ids: Vec<InstanceId>, cx: &mut Context<Self>) {
        self.ui.group = Some((editing, ids));
        self.insert_open = false;
        self.update_capture(cx);
        cx.notify();
    }

    pub(super) fn toggle_group_member(&mut self, id: InstanceId, cx: &mut Context<Self>) {
        if self.assembly(cx).is_some_and(|a| a.generated_by(id).is_some()) {
            return;
        }
        if let Some((_, ids)) = &mut self.ui.group {
            match ids.iter().position(|x| *x == id) {
                Some(i) => {
                    ids.remove(i);
                }
                None => ids.push(id),
            }
        }
        self.refresh_scene(false, cx);
        cx.notify();
    }

    pub(super) fn render_group_dialog(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let Some((editing, ids)) = self.ui.group.clone() else { return div().into_any_element() };
        let names: HashMap<InstanceId, String> = self.assembly(cx).map(|a| a.instances.iter().map(|i| (i.id, i.name.clone())).collect()).unwrap_or_default();
        let title = editing.and_then(|g| self.assembly(cx).and_then(|a| a.groups.iter().find(|x| x.id == g).map(|x| x.name.clone()))).unwrap_or_else(|| "Group".into());
        let mut field = v_flex().gap_px().p_1().rounded_md().border_1().border_color(pal.accent).bg(with_alpha(pal.accent, 0.06)).min_h(px(28.));
        if ids.is_empty() {
            field = field.child(div().px_1().text_sm().text_color(pal.text_faint).child("Instances"));
        }
        for id in ids {
            field = field.child(
                h_flex()
                    .px_1()
                    .gap_1()
                    .text_sm()
                    .child(div().flex_1().truncate().child(names.get(&id).cloned().unwrap_or_default()))
                    .child(Button::new(SharedString::from(format!("grp-x-{}", id.0))).icon(IconName::Close).ghost().xsmall().on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.toggle_group_member(id, cx)))),
            );
        }
        let body = v_flex()
            .p_3()
            .gap_2()
            .child(field)
            .child(div().text_xs().text_color(pal.text_faint).child("Select instances in the graphics area or the instance list."))
            .child(h_flex().justify_end().child(Button::new("grp-ok").icon(Icon::empty().path(icon_path("check"))).primary().small().tooltip("Accept").on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.accept_group(cx)))));
        Self::panel_frame(&title, body, cx.listener(|this, _: &ClickEvent, _, cx| {
            this.ui.group = None;
            this.update_capture(cx);
            cx.notify();
        }), "grp-close")
        .into_any_element()
    }
}

// ---------------------------------------------------------------- Select other

impl AssemblyView {
    /// Instances under the cursor, nearest first.
    fn instances_under(&self, position: DVec2, cx: &App) -> Vec<InstanceId> {
        let (o, d) = self.viewport.read(cx).ray(position);
        let mut hits: Vec<(f64, InstanceId)> = Vec::new();
        for occ in &self.result.occurrences {
            if occ.hidden || !matches!(occ.kind, BodyKind::Solid | BodyKind::Surface) {
                continue;
            }
            if self.isolate.as_ref().is_some_and(|s| !s.contains(&occ.top())) {
                continue;
            }
            let inv = occ.transform.inverse();
            let (lo, ld) = (inv.transform_point3(o), inv.transform_vector3(d));
            let m = &occ.mesh;
            let pos = |i: u32| m.positions.get(i as usize).map(|p| DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64)).unwrap_or_default();
            let mut best: Option<f64> = None;
            for t in m.indices.as_chunks::<3>().0 {
                if let Some(tt) = panels::ray_triangle(lo, ld, pos(t[0]), pos(t[1]), pos(t[2]))
                    && best.is_none_or(|b| tt < b)
                {
                    best = Some(tt);
                }
            }
            if let Some(t) = best {
                match hits.iter_mut().find(|(_, id)| *id == occ.top()) {
                    Some(h) => h.0 = h.0.min(t),
                    None => hits.push((t, occ.top())),
                }
            }
        }
        hits.sort_by(|a, b| a.0.total_cmp(&b.0));
        hits.into_iter().map(|(_, id)| id).collect()
    }

    /// ` (Select other): cycles through the instances under the cursor
    /// (Shift+` backwards), selecting each; the list shows them all.
    pub(super) fn select_other(&mut self, backwards: bool, cx: &mut Context<Self>) {
        let (list, ix, at) = match self.ui.select_other.take() {
            Some((list, ix, at)) if !list.is_empty() => {
                let n = list.len();
                let ix = if backwards { (ix + n - 1) % n } else { (ix + 1) % n };
                (list, ix, at)
            }
            _ => {
                let list = self.instances_under(self.pointer, cx);
                if list.is_empty() {
                    return;
                }
                // The first press picks the one behind the current choice.
                let first = self.selected.first().and_then(|s| list.iter().position(|x| x == s)).map(|i| (i + 1) % list.len()).unwrap_or(0);
                (list, first, self.pointer)
            }
        };
        self.selected = vec![list[ix]];
        self.selected_mate = None;
        self.ui.scroll_to_selection = true;
        self.ui.select_other = Some((list, ix, at));
        self.refresh_scene(false, cx);
        cx.notify();
    }

    pub(super) fn render_select_other(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (list, ix, at) = self.ui.select_other.clone()?;
        let pal = p();
        let names: HashMap<InstanceId, String> = self.assembly(cx).map(|a| a.instances.iter().map(|i| (i.id, i.name.clone())).collect()).unwrap_or_default();
        let mut col = v_flex().id("asm-select-other").occlude().absolute().left(px(at.x as f32 + 12.)).top(px(at.y as f32 + 12.)).min_w(px(160.)).p_1().rounded_md().bg(pal.panel_raised).border_1().border_color(pal.border_strong).shadow_lg();
        for (k, id) in list.into_iter().enumerate() {
            col = col.child(
                h_flex()
                    .id(SharedString::from(format!("so-{k}")))
                    .h(px(22.))
                    .px_2()
                    .rounded_sm()
                    .text_sm()
                    .text_color(pal.text)
                    .cursor_pointer()
                    .when(k == ix, |d| d.bg(pal.accent_soft))
                    .hover(|d| d.bg(pal.panel_hover))
                    .child(names.get(&id).cloned().unwrap_or_default())
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.selected = vec![id];
                        this.ui.select_other = None;
                        this.ui.scroll_to_selection = true;
                        this.refresh_scene(false, cx);
                        cx.notify();
                    })),
            );
        }
        Some(col.into_any_element())
    }
}

// ---------------------------------------------------------------- Insert: configuration inputs

impl AssemblyView {
    /// Configuration inputs of a configured Part Studio in the Insert
    /// dialog (list and checkbox inputs; values apply to the next insert).
    pub(super) fn render_insert_config(&mut self, studio: TabId, cx: &mut Context<Self>) -> Option<AnyElement> {
        use noshape_model::configuration::ConfigInputKind;
        let inputs = match self.doc.read(cx).session.doc.tab(studio).map(|t| &t.content) {
            Some(TabContent::PartStudio(ps)) => ps.configuration.inputs.clone(),
            _ => return None,
        };
        if inputs.is_empty() {
            return None;
        }
        let pal = p();
        let values = self.ui.insert_config.get(&studio).cloned().unwrap_or_default();
        let mut col = v_flex().pl(px(26.)).pr_2().py_1().gap_1();
        for inp in &inputs {
            let id = inp.id.clone();
            match &inp.kind {
                ConfigInputKind::List { options, default } => {
                    let cur = values.0.get(&inp.id).cloned().unwrap_or_else(|| default.clone());
                    let mut seg = h_flex().gap_1().flex_wrap().child(div().text_xs().text_color(pal.text_muted).child(inp.name.clone()));
                    for o in options {
                        let (oid, id2) = (o.id.clone(), id.clone());
                        seg = seg.child(Button::new(SharedString::from(format!("inscfg-{studio}-{}-{}", inp.id, o.id))).label(o.name.clone()).xsmall().ghost().selected(cur == o.id).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.ui.insert_config.entry(studio).or_default().0.insert(id2.clone(), oid.clone());
                            cx.notify();
                        })));
                    }
                    col = col.child(seg);
                }
                ConfigInputKind::Checkbox { default } => {
                    let cur = values.0.get(&inp.id).map(|v| v == "true").unwrap_or(*default);
                    col = col.child(Checkbox::new(SharedString::from(format!("inscfg-{studio}-{}", inp.id))).label(inp.name.clone()).checked(cur).on_click(cx.listener(move |this, v: &bool, _, cx| {
                        this.ui.insert_config.entry(studio).or_default().0.insert(id.clone(), v.to_string());
                        cx.notify();
                    })));
                }
                ConfigInputKind::Variable { default, .. } => {
                    col = col.child(div().text_xs().text_color(pal.text_faint).child(format!("{} = {}", inp.name, values.0.get(&inp.id).cloned().unwrap_or_else(|| default.text.clone()))));
                }
                ConfigInputKind::Text { .. } => {}
            }
        }
        if !values.0.is_empty() {
            col = col.child(div().id(SharedString::from(format!("inscfg-reset-{studio}"))).text_xs().text_color(pal.accent).cursor_pointer().child("Reset to default").on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.ui.insert_config.remove(&studio);
                cx.notify();
            })));
        }
        Some(col.into_any_element())
    }
}

// ---------------------------------------------------------------- live mate values

impl AssemblyView {
    /// Mates whose values show in the graphics: those of the instance being
    /// dragged, else the selected mate (Onshape mate value display).
    fn value_mates(&self, cx: &App) -> Vec<MateId> {
        let Some(asm) = self.assembly(cx) else { return vec![] };
        let shown = |m: &&Mate| !noshape_model::assembly::eval::mate_dofs(m.mate_type).is_empty();
        if let Some(d) = self.drag.as_ref().filter(|d| d.started && !d.explode) {
            return asm.mates.iter().filter(|m| m.uses_instance(d.top)).filter(shown).map(|m| m.id).collect();
        }
        if let Some((id, ..)) = &self.ui.value_edit {
            return vec![*id];
        }
        self.selected_mate.and_then(|id| asm.mate(id)).filter(shown).map(|m| vec![m.id]).unwrap_or_default()
    }

    /// Value labels of the shown mates at their connectors (Rz, Tx, Ty, Tz as the
    /// mate allows); double-click one to type a value that moves the parts.
    pub(super) fn render_mate_values(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let ids = self.value_mates(cx);
        if ids.is_empty() || self.mate.is_some() {
            return vec![];
        }
        let pal = p();
        let units = self.units(cx);
        let vp = self.viewport.read(cx);
        let size = vp.size();
        let mut out = Vec::new();
        for id in ids {
            let Some(ms) = self.result.mates.iter().find(|m| m.id == id && m.ok && !m.inactive) else { continue };
            let Some(t) = self.assembly(cx).and_then(|a| a.mate(id)).map(|m| m.mate_type) else { continue };
            let Some(f) = ms.frame_b else { continue };
            let Some(at) = vp.project(f.origin) else { continue };
            if at.x < 0.0 || at.y < 0.0 || at.x > size.x || at.y > size.y {
                continue;
            }
            let mut col = v_flex().absolute().left(px(at.x as f32 + 10.)).top(px(at.y as f32 - 10.)).gap_px();
            for (ix, label) in noshape_model::assembly::eval::mate_dofs(t) {
                let ix = *ix;
                let v = ms.values[ix];
                let q = if ix == 0 { noshape_units::Quantity::radians(v) } else { noshape_units::Quantity::meters(v) };
                let axis = match ix {
                    0 => "Rz",
                    1 => "X",
                    2 => "Y",
                    _ => "Z",
                };
                let _ = label;
                let editing = self.ui.value_edit.as_ref().filter(|(m, d, ..)| *m == id && *d == ix).map(|(_, _, st, _)| st.clone());
                let chip = h_flex()
                    .id(SharedString::from(format!("mv-{}-{ix}", id.0)))
                    .gap_1()
                    .px_1()
                    .rounded_sm()
                    .bg(with_alpha(pal.panel_raised, 0.92))
                    .border_1()
                    .border_color(pal.border_strong)
                    .text_xs()
                    .text_color(pal.text)
                    .child(div().text_color(pal.text_muted).child(axis));
                let chip = match editing {
                    Some(st) => chip.child(div().w(px(80.)).child(Input::new(&st).xsmall())),
                    None => chip.child(noshape_units::format_quantity(q, &units)).cursor_pointer().on_click(cx.listener(move |this, ev: &ClickEvent, window, cx| {
                        if ev.click_count() >= 2 {
                            this.start_value_edit(id, ix, window, cx);
                        }
                    })),
                };
                col = col.child(chip.occlude());
            }
            out.push(col.into_any_element());
        }
        out
    }

    fn start_value_edit(&mut self, id: MateId, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        let units = self.units(cx);
        let v = self.result.mates.iter().find(|m| m.id == id).map(|m| m.values[ix]).unwrap_or(0.0);
        let q = if ix == 0 { noshape_units::Quantity::radians(v) } else { noshape_units::Quantity::meters(v) };
        let st = cx.new(|cx| InputState::new(window, cx).default_value(noshape_units::format_quantity(q, &units)));
        st.update(cx, |s, cx| {
            s.focus(window, cx);
            s.select_all(window, cx);
        });
        let sub = cx.subscribe_in(&st, window, move |this: &mut Self, st, ev: &InputEvent, window, cx| match ev {
            InputEvent::PressEnter { .. } => {
                let text = st.read(cx).value().to_string();
                this.ui.value_edit = None;
                this.apply_mate_value(id, ix, &text, cx);
                window.focus(&this.focus, cx);
            }
            InputEvent::Blur => {
                this.ui.value_edit = None;
                cx.notify();
            }
            _ => {}
        });
        self.ui.value_edit = Some((id, ix, st, sub));
        cx.notify();
    }

    /// Move the parts so one mate value becomes `text` (other mates enforced).
    pub(super) fn apply_mate_value(&mut self, id: MateId, ix: usize, text: &str, cx: &mut Context<Self>) {
        let units = self.units(cx);
        let dim = if ix == 0 { noshape_units::Dim::ANGLE } else { noshape_units::Dim::LENGTH };
        let v = match noshape_units::evaluate_as(text, dim, &units, &noshape_units::EmptyScope) {
            Ok(q) => q.value,
            Err(e) => {
                self.message = Some((true, format!("{e}")));
                cx.notify();
                return;
            }
        };
        let start: HashMap<InstanceId, Placement> = self.result.solved.placements.iter().cloned().collect();
        let solved = self.result.prepared.solve(&SolveOptions { hold_values: vec![(id, ix, v)], start: Some(start), nested_start: Some(self.result.solved.nested.clone()), ..Default::default() });
        let name = self.assembly(cx).and_then(|a| a.mate(id).map(|m| m.name.clone())).unwrap_or_default();
        self.apply_solved(solved, cx);
        self.edit(&format!("Move : {name}"), cx, |_| {});
        cx.notify();
    }
}

// ---------------------------------------------------------------- snap insert

impl AssemblyView {
    /// Explicit mate connectors (from its Part Studio) of an instance.
    pub(super) fn explicit_connectors(&self, id: InstanceId) -> Vec<MateConnectorRef> {
        let mut out = Vec::new();
        let mut seen = HashSet::new();
        for o in self.result.occurrences.iter().filter(|o| o.top() == id) {
            if !seen.insert(o.path.clone()) {
                continue;
            }
            for c in &o.context.connectors {
                out.push(MateConnectorRef::on(id, o.path[1..].to_vec(), c.reference.clone()));
            }
        }
        out
    }

    /// Placing in snap mode: the chosen explicit connector of the inserted
    /// instance lands on the mate connector under the cursor. Returns false
    /// (free placement) without connectors or target.
    pub(super) fn update_insert_snap(&mut self, id: InstanceId, cx: &mut Context<Self>) -> bool {
        let sources = self.explicit_connectors(id);
        if sources.is_empty() {
            return false;
        }
        let src = sources[self.ui.place_src_ix % sources.len()].clone();
        let Some(dst) = self.snap_target(self.pointer, id, cx) else {
            self.ui.place_snap = None;
            return false;
        };
        let units = self.units(cx);
        let (Some(fs), Some(fd), Some(pose)) = (self.result.connector_world(&src, &units), self.result.connector_world(&dst, &units), self.result.solved.placement(id)) else { return false };
        let np = noshape_model::assembly::eval::snap_placement(&pose, &fs, &fd);
        let mut solved = self.result.solved.clone();
        for (i, pl) in &mut solved.placements {
            if *i == id {
                *pl = np;
            }
        }
        self.apply_solved(solved, cx);
        self.placing_point = Some(fd.origin);
        self.ui.place_snap = Some((src, dst));
        true
    }

    /// Ctrl while snap-inserting: the next explicit connector is the source.
    pub(super) fn cycle_insert_source(&mut self, cx: &mut Context<Self>) -> bool {
        if self.placing.as_ref().is_none_or(|p| p.len() != 1) || self.snap.is_none() {
            return false;
        }
        self.ui.place_src_ix += 1;
        if self.update_placing(cx) {
            self.rebuild_scene(true, cx);
        }
        true
    }
}

// ---------------------------------------------------------------- measure and section view

impl AssemblyView {
    /// `[`: Measure tool on / off. While on, clicked faces, edges and vertices
    /// are measured (click again to drop one).
    pub(super) fn toggle_measure(&mut self, cx: &mut Context<Self>) {
        self.ui.measure = if self.ui.measure.is_some() { None } else { Some(Vec::new()) };
        self.ui.measure_result.clear();
        self.update_capture(cx);
        self.refresh_scene(false, cx);
        cx.notify();
    }

    pub(super) fn measure_pick(&mut self, t: AsmPickTarget, cx: &mut Context<Self>) {
        let Some(list) = &mut self.ui.measure else { return };
        if !matches!(t.kind, PickKind::Face | PickKind::Edge | PickKind::Vertex) {
            return;
        }
        let key = (t.path.clone(), t.reference.clone());
        match list.iter().position(|x| *x == key) {
            Some(i) => {
                list.remove(i);
            }
            None => list.push(key),
        }
        let picks = list.clone();
        self.ui.measure_result = noshape_model::assembly::analysis::measure_entities(&self.result, &picks);
        self.refresh_scene(false, cx);
        cx.notify();
    }

    pub(super) fn render_measure(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        use noshape_model::regen::measure::Measurement;
        let picks = self.ui.measure.as_ref()?;
        let pal = p();
        let units = self.units(cx);
        let l = |x: f64| noshape_units::format_quantity(noshape_units::Quantity::meters(x), &units);
        let mut rows: Vec<(String, String)> = Vec::new();
        for (label, m) in &self.ui.measure_result {
            let v = match m {
                Measurement::Length(x) | Measurement::Radius(x) | Measurement::Diameter(x) => l(*x),
                Measurement::Area(x) => noshape_units::format_quantity(noshape_units::Quantity::new(*x, noshape_units::Dim { length: 2, angle: 0 }), &units),
                Measurement::Volume(x) => noshape_units::format_quantity(noshape_units::Quantity::new(*x, noshape_units::Dim { length: 3, angle: 0 }), &units),
                Measurement::Angle(x) => noshape_units::format_quantity(noshape_units::Quantity::radians(*x), &units),
                Measurement::Point(p) => format!("{}, {}, {}", l(p.x), l(p.y), l(p.z)),
                Measurement::Distance { distance, dx, dy, dz, .. } => {
                    rows.push(("ΔX / ΔY / ΔZ".into(), format!("{} / {} / {}", l(*dx), l(*dy), l(*dz))));
                    l(*distance)
                }
            };
            rows.push((label.clone(), v));
        }
        if picks.is_empty() {
            rows.push(("Select entities to measure".into(), String::new()));
        }
        let mut body = v_flex().p_2().gap_1();
        for (k, v) in rows {
            body = body.child(h_flex().gap_3().text_xs().child(div().w(px(110.)).text_color(pal.text_muted).child(k)).child(div().text_color(pal.text).child(v)));
        }
        Some(
            v_flex()
                .id("asm-measure")
                .occlude()
                .absolute()
                .bottom(px(16.))
                .right(px(64.))
                .w(px(300.))
                .rounded_md()
                .bg(pal.panel_raised)
                .border_1()
                .border_color(pal.border_strong)
                .shadow_lg()
                .child(
                    h_flex()
                        .px_2()
                        .py_1()
                        .border_b_1()
                        .border_color(pal.border)
                        .child(div().flex_1().text_sm().text_color(pal.text).child("Measure"))
                        .child(Button::new("asm-measure-x").icon(IconName::Close).ghost().xsmall().on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.toggle_measure(cx)))),
                )
                .child(body)
                .into_any_element(),
        )
    }

    /// Shift+X: section view on (through the hovered planar face or the
    /// selected assembly mate connector, else the Front plane) / off.
    pub(super) fn toggle_section(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.ui.section.take().is_some() {
            self.rebuild_scene(false, cx);
            cx.notify();
            return;
        }
        let units = self.units(cx);
        let mut plane: Option<(String, DVec3, DVec3)> = None;
        if let Some(t) = self.hover.as_ref().filter(|t| t.kind == PickKind::Face)
            && let Some(o) = self.result.occurrences.get(t.occ)
        {
            let r = Reference::Implicit { on: Box::new(t.reference.clone()), inference: Inference::Centroid };
            if let Some(f) = noshape_model::assembly::eval::resolve_connector_in(&o.context, Some(&o.body), &r, &units) {
                let w = noshape_model::assembly::eval::matrix_frame(&o.transform).mul(&f);
                plane = Some(("Face".into(), w.origin, w.z()));
            }
        }
        if plane.is_none()
            && let Some(id) = self.selected_mate
            && let Some(f) = self.result.connector_frame(id)
        {
            plane = Some((self.assembly(cx).and_then(|a| a.connector(id).map(|c| c.name.clone())).unwrap_or_default(), f.origin, f.z()));
        }
        let (label, origin, normal) = plane.unwrap_or_else(|| ("Front plane".into(), DVec3::ZERO, DVec3::NEG_Y));
        let text = noshape_units::format_quantity(noshape_units::Quantity::meters(0.0), &units);
        let input = cx.new(|cx| InputState::new(window, cx).default_value(text));
        let sub = cx.subscribe_in(&input, window, |this: &mut Self, st, ev: &InputEvent, _, cx| {
            if matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Change) {
                let units = this.units(cx);
                if let Ok(q) = noshape_units::evaluate_as(&st.read(cx).value(), noshape_units::Dim::LENGTH, &units, &noshape_units::EmptyScope)
                    && let Some(s) = &mut this.ui.section
                {
                    s.offset = q.value;
                    this.rebuild_scene(false, cx);
                    cx.notify();
                }
            }
        });
        self.ui.section = Some(AsmSection { label, origin, normal, offset: 0.0, flipped: false, input, _sub: sub });
        self.rebuild_scene(false, cx);
        cx.notify();
    }

    pub(super) fn render_section(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let s = self.ui.section.as_ref()?;
        let pal = p();
        let flipped = s.flipped;
        let body = v_flex()
            .p_2()
            .gap_2()
            .child(div().text_xs().text_color(pal.text_muted).child(s.label.clone()))
            .child(h_flex().gap_2().child(div().w(px(60.)).text_xs().text_color(pal.text_muted).child("Offset")).child(div().flex_1().child(Input::new(&s.input).xsmall())))
            .child(Button::new("asm-sec-flip").icon(Icon::empty().path(icon_path("flip"))).label("Flip").ghost().small().selected(flipped).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                if let Some(s) = &mut this.ui.section {
                    s.flipped = !s.flipped;
                }
                this.rebuild_scene(false, cx);
                cx.notify();
            })));
        Some(Self::panel_frame("Section view", body, cx.listener(|this, _: &ClickEvent, window, cx| this.toggle_section(window, cx)), "asm-sec-close").into_any_element())
    }

    /// Isolate / Make transparent panel slider steps.
    pub(super) fn step_transparency(&mut self, delta: f32, cx: &mut Context<Self>) {
        let default = if self.isolate.is_some() { 0.12 } else { 0.35 };
        let v = (self.ui.transparency.unwrap_or(default) + delta).clamp(0.0, 0.9);
        self.ui.transparency = Some(v);
        self.rebuild_scene(false, cx);
        cx.notify();
    }
}
