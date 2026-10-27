//! Onshape's Tab manager (Alt+T): a 380 px panel docked left of the
//! document with a tab search, type filters, one 54 px row per tab (icon,
//! name, italic type), a large preview of the selected tab and the
//! keyboard hint at the bottom. ↑/↓ (or Tab) move the selection, Enter
//! opens it, Esc closes the panel. With no search and no type filter the
//! tab folders show (help: Document tabs › Searching and grouping tabs):
//! folder menu Rename, Unpack, Move to document… (as the tab bar);
//! Ctrl+click selects several tabs for Add selection to folder, Move to
//! document… or Delete.

use crate::doc_model::{DocEvent, DocModel};
use crate::os_menu::OsContextMenuExt as _;
use crate::os_ui as os;
use crate::theme::p;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::TabContent;
use noshape_model::document::TabId;
use std::collections::HashSet;

/// Tabs' pictures (moved to app-core).
pub(crate) use noshape_app_core::tab_thumbnail::{decode, drawing_thumbnail, tab_thumbnail};

pub enum TabManagerEvent {
    Open(TabId),
    Close,
    /// Context menu › Delete on the selected tabs.
    Delete(Vec<TabId>),
}

impl EventEmitter<TabManagerEvent> for TabManager {}

/// Tab kinds in Onshape's filter order: (key, icon, type label).
const KINDS: [(&str, &str, &str); 7] = [
    ("partstudio", "part-studio", "Part Studio"),
    ("assembly", "assembly", "Assembly"),
    ("drawing", "drawing", "Drawing"),
    ("variables", "variable-studio", "Variable Studio"),
    ("featurestudio", "feature-studio", "Feature Studio"),
    ("materiallibrary", "material-library", "Material Library"),
    ("file", "file", "File"),
];

fn kind_of(c: &TabContent) -> &'static str {
    match c {
        TabContent::PartStudio(_) => "partstudio",
        TabContent::Assembly(_) => "assembly",
        TabContent::Drawing(_) => "drawing",
        TabContent::VariableStudio(_) => "variables",
        TabContent::FeatureStudio(_) => "featurestudio",
        TabContent::MaterialLibrary(_) => "materiallibrary",
        // Live (tabbar/icons2-filetab-tabmanager-*): one kind per file
        // format, shown with its own icon in the rows and the filter.
        TabContent::File(f) => crate::file_tab_view::file_icon(f),
    }
}

fn kind_info(kind: &'static str) -> (&'static str, &'static str) {
    if kind.starts_with("file") {
        return (kind, "File");
    }
    KINDS.iter().find(|k| k.0 == kind).map(|k| (k.1, k.2)).unwrap_or(("part-studio", "Part Studio"))
}

/// Rendered tab pictures (Onshape shows the shaded part / assembly), made
/// off the UI thread and cached per content hash.
#[derive(Default)]
struct Thumbs {
    images: std::collections::HashMap<TabId, std::sync::Arc<RenderImage>>,
    made_for: std::collections::HashMap<TabId, u64>,
    busy: bool,
}

pub struct TabManager {
    doc: Entity<DocModel>,
    thumbs: Thumbs,
    evaluator: std::sync::Arc<parking_lot::Mutex<noshape_model::assembly::eval::AssemblyEvaluator>>,
    search: Entity<InputState>,
    query: String,
    /// Type filters (empty = all).
    kinds: HashSet<&'static str>,
    selected: Option<TabId>,
    /// Ctrl+click selection (tabs and folders).
    multi: Vec<TabId>,
    collapsed: HashSet<TabId>,
    /// Folder being renamed in place.
    renaming: Option<(TabId, Entity<InputState>, Subscription)>,
    /// Header toggles (live document/41–46): sort order, search box, type
    /// filter row, one-line list rows, and the preview (footer).
    sort: TabSort,
    /// The sort menu is open; the sort button's bounds (the menu hangs under it).
    sort_open: bool,
    sort_anchor: std::rc::Rc<std::cell::Cell<Bounds<Pixels>>>,
    show_search: bool,
    show_filters: bool,
    compact: bool,
    show_preview: bool,
    _subs: Vec<Subscription>,
}

/// The sort ▾ menu (live document/42).
#[derive(Clone, Copy, PartialEq, Eq)]
enum TabSort {
    TabOrder,
    NameAsc,
    NameDesc,
    Type,
}

impl TabSort {
    const ALL: [TabSort; 4] = [TabSort::TabOrder, TabSort::NameAsc, TabSort::NameDesc, TabSort::Type];
    fn label(self) -> &'static str {
        match self {
            TabSort::TabOrder => "Tab order",
            TabSort::NameAsc => "Name: A → Z",
            TabSort::NameDesc => "Name: Z → A",
            TabSort::Type => "Type",
        }
    }
}

impl TabManager {
    pub fn new(doc: Entity<DocModel>, active: Option<TabId>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).placeholder("Search tabs"));
        let subs = vec![
            cx.subscribe_in(&search, window, |this: &mut Self, st, ev: &InputEvent, _, cx| match ev {
                InputEvent::Change => {
                    this.query = st.read(cx).value().to_string();
                    let m = this.matches(cx);
                    if this.selected.is_none_or(|s| !m.iter().any(|t| t.0 == s)) {
                        this.selected = m.first().map(|t| t.0);
                    }
                    cx.notify();
                }
                InputEvent::PressEnter { .. } => {
                    if let Some(t) = this.selected {
                        cx.emit(TabManagerEvent::Open(t));
                    }
                }
                _ => {}
            }),
            cx.subscribe_in(&doc, window, |_: &mut Self, _, _: &DocEvent, _, cx| cx.notify()),
        ];
        search.update(cx, |s, cx| s.focus(window, cx));
        Self { doc, thumbs: Thumbs::default(), evaluator: Default::default(), search, query: String::new(), kinds: HashSet::new(), selected: active, multi: Vec::new(), collapsed: HashSet::new(), renaming: None, sort: TabSort::TabOrder, sort_open: false, sort_anchor: Default::default(), show_search: true, show_filters: true, compact: false, show_preview: true, _subs: subs }
    }

    /// The workspace's active tab changed (keep it selected, like Onshape).
    pub fn set_active(&mut self, tab: Option<TabId>, cx: &mut Context<Self>) {
        self.selected = tab;
        cx.notify();
    }

    /// Folders show only with no search and no type filter.
    fn tree_mode(&self) -> bool {
        self.query.trim().is_empty() && self.kinds.is_empty() && self.sort == TabSort::TabOrder
    }

    /// Tabs and folders in folder order with their depth; `all` includes
    /// the contents of collapsed folders.
    fn tree(&self, all: bool, cx: &App) -> Vec<(TabId, usize)> {
        fn walk(d: &noshape_model::Document, f: Option<TabId>, depth: usize, collapsed: Option<&HashSet<TabId>>, out: &mut Vec<(TabId, usize)>) {
            for c in d.children(f) {
                out.push((c, depth));
                if d.is_folder(c) && !collapsed.is_some_and(|s| s.contains(&c)) {
                    walk(d, Some(c), depth + 1, collapsed, out);
                }
            }
        }
        let mut out = Vec::new();
        walk(&self.doc.read(cx).session.doc, None, 0, (!all).then_some(&self.collapsed), &mut out);
        out
    }

    /// The rows shown (shared with the agent API's tab_manager).
    fn matches(&self, cx: &App) -> Vec<(TabId, String, &'static str)> {
        use noshape_ops::window::TabSort as S;
        let sort = match self.sort {
            TabSort::TabOrder => S::TabOrder,
            TabSort::NameAsc => S::NameAsc,
            TabSort::NameDesc => S::NameDesc,
            TabSort::Type => S::Type,
        };
        let kinds: Vec<&str> = self.kinds.iter().copied().collect();
        noshape_ops::window::tab_manager_rows(&self.doc.read(cx).session.doc, &self.query, &kinds, sort, &kind_of)
    }

    /// The search box's text (the agent API's tab_manager search).
    pub fn set_search(&mut self, text: String, window: &mut Window, cx: &mut Context<Self>) {
        self.query = text.clone();
        self.search.update(cx, |s, cx| s.set_value(text, window, cx));
        cx.notify();
    }

    /// The rows a context menu acts on: the Ctrl+click selection if `id` is in it.
    fn targets(&self, id: TabId) -> Vec<TabId> {
        if self.multi.contains(&id) { self.multi.clone() } else { vec![id] }
    }

    /// Add selection to folder: a new folder (in the first item's folder)
    /// holding `ids`, then its name field.
    fn add_to_new_folder(&mut self, ids: Vec<TabId>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(first) = ids.first().copied() else { return };
        let mut made = None;
        self.doc.update(cx, |doc, cx| {
            doc.edit_document("Add selection to folder", cx, |d| {
                let parent = d.parent_of(first);
                let f = d.add_folder(&d.next_folder_name(), parent);
                d.move_in_order(f, Some(first));
                for id in &ids {
                    d.set_parent(*id, Some(f));
                }
                made = Some(f);
            });
        });
        self.multi.clear();
        if let Some(f) = made {
            self.start_rename(f, window, cx);
        }
    }

    fn unpack(&mut self, folder: TabId, cx: &mut Context<Self>) {
        self.doc.update(cx, |doc, cx| doc.edit_document("Unpack folder", cx, |d| d.unpack_folder(folder)));
        cx.notify();
    }

    fn start_rename(&mut self, folder: TabId, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.doc.read(cx).session.doc.folder(folder).map(|f| f.name.clone()).unwrap_or_default();
        let st = cx.new(|cx| InputState::new(window, cx).default_value(name));
        let sub = cx.subscribe_in(&st, window, move |this: &mut Self, st, ev: &InputEvent, _, cx| {
            if matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Blur) && this.renaming.is_some() {
                let name = st.read(cx).value().trim().to_string();
                this.renaming = None;
                if !name.is_empty() {
                    this.doc.update(cx, |doc, cx| doc.edit_document_not_undoable(&format!("Rename tab : {name}"), cx, |d| d.rename_folder(folder, &name)));
                }
                cx.notify();
            }
        });
        st.update(cx, |s, cx| {
            s.focus(window, cx);
            s.select_all(window, cx);
        });
        self.renaming = Some((folder, st, sub));
        cx.notify();
    }

    /// A folder row: chevron, folder glyph, name (or its rename field).
    fn folder_row(&self, id: TabId, depth: usize, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let name = self.doc.read(cx).session.doc.folder(id).map(|f| f.name.clone()).unwrap_or_default();
        let open = !self.collapsed.contains(&id);
        let picked = self.multi.contains(&id);
        let view = cx.entity();
        h_flex()
            .id(SharedString::from(format!("tm-f-{id}")))
            .h(px(40.))
            .flex_none()
            .pl(px(8. + depth as f32 * 20.))
            .gap(px(6.))
            .cursor_pointer()
            .when(picked, |d| d.bg(pal.accent_soft))
            .when(!picked, |d| d.hover(|d| d.bg(pal.panel_hover)))
            .child(os::glyph(if open { os::GLYPH_CHEVRON_DOWN } else { os::GLYPH_CHEVRON_RIGHT }, 14., pal.text))
            .child(os::glyph(os::GLYPH_FOLDER, 24., pal.text))
            .child(match self.renaming.as_ref().filter(|r| r.0 == id) {
                Some((_, st, _)) => div().w(px(200.)).child(Input::new(st).h(px(26.)).text_size(px(14.))).into_any_element(),
                None => div().flex_1().min_w_0().truncate().text_size(px(14.)).text_color(pal.text).child(crate::ui::user_text(name)).into_any_element(),
            })
            .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
                if e.modifiers().secondary() {
                    if let Some(i) = this.multi.iter().position(|t| *t == id) {
                        this.multi.remove(i);
                    } else {
                        this.multi.push(id);
                    }
                } else if e.click_count() >= 2 {
                    this.start_rename(id, window, cx);
                } else {
                    this.multi.clear();
                    if !this.collapsed.remove(&id) {
                        this.collapsed.insert(id);
                    }
                }
                cx.notify();
            }))
            .os_context_menu(move |menu, _, _| {
                use crate::os_menu::OsMenuItem as I;
                // Live (behaviours "Folders"): the tab bar's three items.
                let (v1, v2, v3) = (view.clone(), view.clone(), view.clone());
                menu.item(I::new("Rename").on_click(move |_, w, cx| v1.update(cx, |t, cx| t.start_rename(id, w, cx))))
                    .item(I::new("Unpack").on_click(move |_, _, cx| v2.update(cx, |t, cx| t.unpack(id, cx))))
                    .separator()
                    .item(I::new("Move to document…").on_click(move |_, w, cx| {
                        let doc = v3.read(cx).doc.clone();
                        crate::doc_dialogs::move_to_document(doc, vec![id], w, cx)
                    }))
            })
            .into_any_element()
    }

    /// Render pictures of Part Studios and Assemblies whose content changed.
    fn request_thumbs(&mut self, cx: &mut Context<Self>) {
        use std::hash::{Hash as _, Hasher as _};
        if self.thumbs.busy {
            return;
        }
        let doc = self.doc.read(cx).session.doc.clone();
        let versions = self.doc.read(cx).session.pinned_documents();
        // Assemblies depend on their Part Studios: hash the whole document for them.
        let whole = {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            serde_json::to_vec(&doc.tabs.iter().map(|t| &t.content).collect::<Vec<_>>()).unwrap_or_default().hash(&mut h);
            h.finish()
        };
        let todo: Vec<(TabId, u64, bool)> = doc
            .ordered_tabs()
            .into_iter()
            .filter_map(|t| match &t.content {
                TabContent::PartStudio(_) => noshape_model::drawing::views::source_hash(&doc, &noshape_model::drawing::views::SourceKey::studio(t.id)).map(|h| (t.id, h, false)),
                TabContent::Assembly(_) | TabContent::Drawing(_) => Some((t.id, whole, true)),
                _ => None,
            })
            .filter(|(t, h, _)| self.thumbs.made_for.get(t) != Some(h))
            .collect();
        if todo.is_empty() {
            return;
        }
        self.thumbs.busy = true;
        let gpu = cx.global::<crate::gpu::GpuGlobal>().0.clone();
        let evaluator = self.evaluator.clone();
        cx.spawn(async move |this, cx| {
            let made = cx
                .background_spawn(async move {
                    let mut out = Vec::new();
                    for (tab, _, assembly) in &todo {
                        if let Some(TabContent::Drawing(d)) = doc.tab(*tab).map(|t| &t.content) {
                            let name = doc.tab(*tab).map(|t| t.name.clone()).unwrap_or_default();
                            if let Some(img) = drawing_thumbnail(&doc, &versions, d, name) {
                                out.push((*tab, img));
                            }
                            continue;
                        }
                        if let Some(img) = tab_thumbnail(&doc, *tab, *assembly, &evaluator, &gpu).await {
                            out.push((*tab, img));
                        }
                    }
                    (todo, out)
                })
                .await;
            let _ = this.update(cx, |this: &mut Self, cx| {
                let (todo, out) = made;
                this.thumbs.busy = false;
                for (tab, h, _) in todo {
                    this.thumbs.images.remove(&tab);
                    this.thumbs.made_for.insert(tab, h);
                }
                this.thumbs.images.extend(out);
                cx.notify();
            });
        })
        .detach();
    }

    fn step(&mut self, delta: isize, cx: &mut Context<Self>) {
        let m = self.matches(cx);
        if m.is_empty() {
            return;
        }
        let i = self.selected.and_then(|s| m.iter().position(|t| t.0 == s)).map(|i| i as isize + delta).unwrap_or(0);
        self.selected = Some(m[i.rem_euclid(m.len() as isize) as usize].0);
        cx.notify();
    }
}

impl Render for TabManager {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        self.request_thumbs(cx);
        let matches = self.matches(cx);
        let present: Vec<&'static str> = {
            let tabs = self.doc.read(cx).session.doc.ordered_tabs();
            let mut v: Vec<&'static str> = KINDS.iter().map(|k| k.0).filter(|k| tabs.iter().any(|t| kind_of(&t.content) == *k)).collect();
            for t in &tabs {
                let k = kind_of(&t.content);
                if k.starts_with("file") && !v.contains(&k) {
                    v.push(k);
                }
            }
            v
        };
        // Tree mode: folders and their tabs (indented); otherwise the matches.
        let entries: Vec<(TabId, usize)> = if self.tree_mode() { self.tree(false, cx) } else { matches.iter().map(|t| (t.0, 0)).collect() };
        let rows: Vec<AnyElement> = entries
            .into_iter()
            .filter_map(|(id, depth)| {
                if self.doc.read(cx).session.doc.is_folder(id) {
                    return Some(self.folder_row(id, depth, cx));
                }
                let (_, name, kind) = matches.iter().find(|t| t.0 == id)?;
                let (icon, label) = kind_info(kind);
                let sel = self.selected == Some(id) || self.multi.contains(&id);
                let view = cx.entity();
                let compact = self.compact;
                // Live document/41: the selected row's name is bold; 45: list
                // view rows are one line (30 px), icon + name.
                Some(h_flex()
                    .id(SharedString::from(format!("tm-{id}")))
                    .relative()
                    .h(px(if compact { 30. } else { 54. }))
                    .flex_none()
                    .pl(px(if compact { 16. } else { 22. } + depth as f32 * 20.))
                    .cursor_pointer()
                    .when(sel, |d| d.bg(pal.accent_soft).border_1().border_color(pal.accent).child(div().absolute().left_0().top_0().bottom_0().w(px(if compact { 2. } else { 6. })).bg(pal.accent)))
                    .when(!sel, |d| d.hover(|d| d.bg(pal.panel_hover)))
                    .when(compact, |d| d.gap(px(6.)).text_size(px(14.)).text_color(pal.text).child(crate::assets::icon(icon, 20., cx)).child(div().truncate().when(sel, |d| d.font_weight(FontWeight::BOLD)).child(crate::ui::user_text(name.clone()))))
                    .when(!compact, |d| {
                        d.child(div().w(px(70.)).h(px(40.)).flex_none().flex().items_center().justify_center().child(match self.thumbs.images.get(&id) {
                            Some(img) => gpui_kit::img(img.clone()).w(px(70.)).h(px(40.)).object_fit(ObjectFit::Contain).into_any_element(),
                            None => crate::assets::icon(icon, 30., cx),
                        }))
                        .child(
                            v_flex()
                                .pl(px(7.))
                                .flex_1()
                                .min_w_0()
                                .child(h_flex().h(px(21.)).gap(px(4.)).text_size(px(14.)).text_color(pal.text).child(crate::assets::icon(icon, 20., cx)).child(div().truncate().when(sel, |d| d.font_weight(FontWeight::BOLD)).child(crate::ui::user_text(name.clone()))))
                                .child(div().h(px(19.)).text_size(px(12.6)).line_height(px(18.9)).italic().text_color(pal.text_muted).child(label)),
                        )
                    })
                    .tooltip(crate::ui::tooltip(format!("Click to navigate to {name}")))
                    .on_click(cx.listener(move |this, e: &ClickEvent, _, cx| {
                        // Ctrl+click: select several (Onshape); a click opens the tab.
                        if e.modifiers().secondary() {
                            if this.multi.is_empty()
                                && let Some(s) = this.selected.filter(|s| *s != id)
                            {
                                this.multi.push(s);
                            }
                            if let Some(i) = this.multi.iter().position(|t| *t == id) {
                                this.multi.remove(i);
                            } else {
                                this.multi.push(id);
                            }
                        } else {
                            this.multi.clear();
                            this.selected = Some(id);
                            cx.emit(TabManagerEvent::Open(id));
                        }
                        cx.notify();
                    }))
                    .os_context_menu(move |menu, _, cx| {
                        use crate::os_menu::OsMenuItem as I;
                        let ids = view.read(cx).targets(id);
                        let (v1, v2, v3) = (view.clone(), view.clone(), view.clone());
                        let (i1, i2, i3) = (ids.clone(), ids.clone(), ids);
                        menu.item(I::new("Add selection to folder").on_click(move |_, w, cx| v1.update(cx, |t, cx| t.add_to_new_folder(i1.clone(), w, cx))))
                            .separator()
                            .item(I::new("Move to document…").on_click(move |_, w, cx| {
                                let doc = v2.read(cx).doc.clone();
                                crate::doc_dialogs::move_to_document(doc, i2.clone(), w, cx)
                            }))
                            .item(I::new("Delete").on_click(move |_, _, cx| v3.update(cx, |t, cx| {
                                t.multi.clear();
                                cx.emit(TabManagerEvent::Delete(i3.clone()))
                            })))
                    })
                    .into_any_element())
            })
            .collect();
        let filters = present.iter().map(|k| {
            let (icon, label) = kind_info(k);
            let k: &'static str = k;
            let on = self.kinds.contains(k);
            crate::ui::icon_button(SharedString::from(format!("tm-kind-{k}")), icon, 30., on, cx).tooltip(label).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                if !this.kinds.remove(k) {
                    this.kinds.insert(k);
                }
                cx.notify();
            }))
        });
        let preview = self.selected.filter(|_| self.show_preview).and_then(|s| matches.iter().find(|t| t.0 == s)).map(|(id, name, kind)| {
            let (icon, _) = kind_info(kind);
            let picture = self.thumbs.images.get(id).cloned();
            v_flex()
                .h(px(214.))
                .flex_none()
                .bg(rgb(0xf5f5f5))
                .px(px(9.))
                .pt(px(12.))
                .child(h_flex().gap(px(4.)).text_size(px(15.)).font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child(crate::assets::icon(icon, 20., cx)).child(div().truncate().child(crate::ui::user_text(name.clone()))))
                .child(div().flex_1().flex().items_center().justify_center().child(match picture {
                    Some(img) => gpui_kit::img(img).w(px(340.)).h(px(170.)).object_fit(ObjectFit::Contain).into_any_element(),
                    None => crate::assets::icon(icon, 130., cx),
                }))
        });
        let key = |t: &'static str| div().px(px(3.)).border_1().border_color(rgb(0xaaaaaa)).rounded(px(2.)).bg(rgb(0xffffff)).child(t);
        let any_filter = !self.kinds.is_empty();
        v_flex()
            .id("tab-manager")
            .size_full()
            .bg(pal.panel)
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| match ev.keystroke.key.as_str() {
                // Live ("spec: psui part-studio.md … 1.27 Alt+T"): Esc does not
                // close it; Alt+T again or its ✕ does.
                "down" => this.step(1, cx),
                "up" => this.step(-1, cx),
                "tab" => this.step(if ev.keystroke.modifiers.shift { -1 } else { 1 }, cx),
                _ => {}
            }))
            .child(
                h_flex()
                    .h(px(37.))
                    .flex_none()
                    .pl(px(5.))
                    .border_t_1()
                    .border_b_1()
                    .border_color(rgb(0xd7d7d7))
                    .child(div().flex_1().text_size(px(14.)).font_weight(FontWeight::BOLD).text_color(pal.text).child("Tabs"))
                    // Live dom/panel-tab-manager-header: 30x30 search / Sort tabs / Filter tabs
                    // 6 px apart, then the bare 20 px List view / Detail view pair (inactive at
                    // 0.2), then the 20 px ×, 5 px from the edge.
                    .child(tm_toggle("tm-search", "search", self.show_search, cx).mr(px(6.)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.show_search = !this.show_search;
                        cx.notify();
                    })))
                    .child({
                        let probe = self.sort_anchor.clone();
                        div()
                            .relative()
                            .mr(px(6.))
                            .child(tm_toggle("tm-sort", "tm-sort", false, cx).when(self.sort_open, |b| b.bg(rgb(0xb2ddf6))).tooltip(crate::ui::tooltip("Sort tabs")).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.sort_open = !this.sort_open;
                                cx.notify();
                            })))
                            .child(canvas(move |b, _, _| probe.set(b), |_, _, _, _| {}).absolute().top_0().left_0().size_full())
                    })
                    .child(tm_toggle("tm-filter", "tm-filter", self.show_filters, cx).mr(px(9.)).tooltip(crate::ui::tooltip("Filter tabs")).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.show_filters = !this.show_filters;
                        cx.notify();
                    })))
                    .child(div().id("tm-list").cursor_pointer().mr(px(3.)).when(!self.compact, |d| d.opacity(0.2)).tooltip(crate::ui::tooltip("List view")).child(crate::assets::icon("tm-list", 20., cx)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.compact = true;
                        cx.notify();
                    })))
                    .child(div().id("tm-detailed").cursor_pointer().mr(px(9.)).when(self.compact, |d| d.opacity(0.2)).tooltip(crate::ui::tooltip("Detail view")).child(crate::assets::icon("tm-detailed", 20., cx)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.compact = false;
                        cx.notify();
                    })))
                    .child(div().id("tm-close").cursor_pointer().mr(px(5.)).child(crate::assets::icon("close", 20., cx)).on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(TabManagerEvent::Close)))),
            )
            // Live document/30: the search box 6 px under the header, the filter row
            // 40 px, rows from y 154.
            .when(self.show_search, |d| d.child(div().px(px(6.)).pt(px(6.)).flex_none().child(gpui_kit::base::input::input_frame(div().h(px(30.)).border_1().border_color(rgb(0xaaaaaa)).rounded(px(4.)).child(Input::new(&self.search).appearance(false).text_size(px(14.)).pl(px(5.)))))))
            .when(self.show_filters, |d| d.child(
                h_flex()
                    .h(px(41.))
                    .px(px(6.))
                    .gap(px(4.))
                    .flex_none()
                    .border_b_1()
                    .border_color(pal.border)
                    .children(filters)
                    .child(div().flex_1())
                    .child(
                        div()
                            .id("tm-clear")
                            .px(px(7.))
                            .h(px(24.))
                            .flex()
                            .items_center()
                            .rounded(px(3.))
                            .border_1()
                            .border_color(pal.border)
                            .text_size(px(12.))
                            .text_color(if any_filter { pal.text } else { pal.text_faint })
                            .when(any_filter, |d| d.cursor_pointer().hover(|d| d.bg(pal.panel_hover)))
                            .child("Clear")
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.kinds.clear();
                                cx.notify();
                            })),
                    ),
            ))
            .child(v_flex().id("tm-rows").flex_1().min_h_0().overflow_y_scroll().children(rows))
            .when(self.sort_open, |d| d.child(self.render_sort_menu(cx)))
            .children(preview)
            .child(
                h_flex()
                    .h(px(30.))
                    .flex_none()
                    .px(px(6.))
                    .gap(px(3.))
                    .border_t_1()
                    .border_color(pal.border)
                    .text_size(px(11.))
                    .text_color(pal.text_muted)
                    .child(key("tab"))
                    .child("or")
                    .child(key("↑"))
                    .child(key("↓"))
                    .child("to navigate")
                    .child(div().w(px(10.)))
                    .child(key("↵"))
                    .child("to open")
                    .child(div().flex_1())
                    // Live document/41, 46, 61–62: the preview toggle (its icon stays the same) and help.
                    .child(div().id("tm-preview").cursor_pointer().mr(px(4.)).tooltip(crate::ui::tooltip("Toggle preview")).child(crate::assets::icon("tm-preview", 20., cx)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.show_preview = !this.show_preview;
                        cx.notify();
                    })))
                    // Live document/63: `a.os-help-link`, no tooltip; opens "Document Tabs".
                    .child(div().id("tm-help").cursor_pointer().child(crate::assets::icon("help-faint", 16., cx)).on_click(|_, _, cx| cx.open_url("https://cad.onshape.com/help/Content/Document/document_tabs.htm?cshid=elementtabs"))),
            )
    }
}

impl TabManager {
    /// The sort menu (live document/42, behaviours "Tab manager header icons"):
    /// 120 px under the sort button, 22 px rows of 12 px `#555` text, the
    /// current order bold on `#b2ddf6`.
    fn render_sort_menu(&self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let b = self.sort_anchor.get();
        let mut list = v_flex()
            .id("tm-sort-menu")
            .occlude()
            .w(px(120.))
            .py(px(1.))
            .bg(pal.panel)
            .border_1()
            .border_color(rgb(0xd7d7d7))
            .rounded(px(3.))
            .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(6.)), blur_radius: px(12.), spread_radius: px(0.), inset: false }])
            .on_mouse_down_out(cx.listener(|this, _: &MouseDownEvent, _, cx| {
                this.sort_open = false;
                cx.notify();
            }));
        for s in TabSort::ALL {
            let cur = s == self.sort;
            list = list.child(
                div()
                    .id(SharedString::from(format!("tm-sort-{}", s.label())))
                    .h(px(22.))
                    .px(px(16.))
                    .flex()
                    .items_center()
                    .text_size(px(12.))
                    .cursor_pointer()
                    .text_color(if cur { pal.text } else { rgb(0x555555) })
                    .when(cur, |d| d.bg(rgb(0xb2ddf6)).font_weight(FontWeight::BOLD))
                    .when(!cur, |d| d.hover(|d| d.bg(pal.row_hover)))
                    .child(s.label())
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.sort = s;
                        this.sort_open = false;
                        cx.notify();
                    })),
            );
        }
        deferred(anchored().position(point(b.left(), b.bottom() + px(1.))).child(list)).with_priority(1).into_any_element()
    }
}

/// A tab manager header toggle (live): 30x30, radius 3; on = `#eaeaea` with
/// an inset 0 1 1 shadow.
fn tm_toggle(id: &'static str, icon: &'static str, on: bool, cx: &App) -> Stateful<Div> {
    div()
        .id(id)
        .size(px(30.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(3.))
        .cursor_pointer()
        .when(on, |d| d.bg(rgb(0xeaeaea)).shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.247), offset: point(px(0.), px(1.)), blur_radius: px(1.), spread_radius: px(0.), inset: true }]))
        .when(!on, |d| d.hover(|d| d.bg(rgb(0xeaeaea))))
        .child(crate::assets::icon(icon, 20., cx))
}
