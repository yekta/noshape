//! The "Select CSV file" / "Select Part Studio" panel of a FeatureScript
//! reference parameter (300×559 beside the feature dialog; live
//! dom/fs-refparams*, fs-refpanel2-*): Current document | Other documents,
//! the document at Main or a version (version graph, Create version…), a
//! search box and the matching tabs. (The Feature Studio's Import panel is
//! fs_import.rs.)

use crate::fs_reference::{RefSource, RefSourceKind};
use crate::theme::p;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::configuration::{ConfigInputKind, ConfigValues, Configuration};
use noshape_model::file_tab::FileKind;
use noshape_model::params::ReferenceKind;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

const BLUE: u32 = 0x1651b0;

/// What the panel picks: a reference parameter of this kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanelMode {
    Reference(ReferenceKind),
}

/// Document filters of the Other documents tab (live 602).
const FILTERS: [(&str, &str); 6] = [("Recent", "recent"), ("Owned by me", "src-owned"), ("Recently opened", "src-recent"), ("Created by me", "src-created"), ("Shared with me", "src-shared"), ("Public", "src-public")];

pub enum SelectItemEvent {
    /// A tab (`tab`, `tab#part`, or `ext:` key of another document / version).
    Picked(String),
    Close,
    /// Create version…: this document (`None`) or another one.
    CreateVersion(Option<PathBuf>),
    /// The footer's "Import…" (CSV and image references).
    ImportFile,
}

impl EventEmitter<SelectItemEvent> for SelectItemPanel {}

/// A document whose tabs can be picked, at one of its versions.
struct VersionedDoc {
    path: PathBuf,
    name: String,
    /// Named versions, newest first, then "Start" (the first state).
    versions: Vec<crate::derived::VersionChoice>,
    /// The chosen version (index into `versions`); `None` is Main, the
    /// workspace (only the current document offers it).
    version: Option<usize>,
    /// Changes on Main since the chosen version.
    changes: usize,
    /// The tabs at the chosen version (`ext:` keys), once loaded.
    sources: Option<Vec<RefSource>>,
}

/// What a panel opens with: this document's tabs, name and file, the tab
/// it works for (not offered), the renderer for pictures, the current value.
pub struct PanelSetup {
    pub mode: PanelMode,
    pub sources: Vec<RefSource>,
    pub doc_name: String,
    pub doc_path: Option<PathBuf>,
    pub except: String,
    pub gpu: Option<crate::gpu::Gpu>,
    pub value: String,
}

pub struct SelectItemPanel {
    mode: PanelMode,
    /// This document: its tabs (at Main), name, file and the tab the panel
    /// works for (not offered).
    sources: Vec<RefSource>,
    doc_name: String,
    doc_path: Option<PathBuf>,
    except: String,
    gpu: Option<crate::gpu::Gpu>,
    /// Pictures of Part Studios and parts, by value.
    pub thumbs: HashMap<String, Arc<RenderImage>>,
    /// The current value (highlighted row).
    pub value: String,
    other_documents: bool,
    search: Entity<InputState>,
    /// Part Studios whose parts are listed.
    expanded: HashSet<String>,
    /// The picked Part Studio shown alone, expanded, with a ‹ back to the
    /// list (live part-studio/407, 418: also when the field is reopened).
    focused: Option<String>,
    /// The version graph replaces the list.
    graph: bool,
    current: Option<VersionedDoc>,
    url: Entity<InputState>,
    category: Option<usize>,
    doc_query: Option<String>,
    docs: Option<Vec<crate::start_library::DocEntry>>,
    /// Latest version label and whether it has matching tabs, per document.
    doc_info: HashMap<PathBuf, (String, bool)>,
    other: Option<VersionedDoc>,
    /// Configured Part Studios: values being edited (applied by Generate).
    cfg_edit: HashMap<String, ConfigValues>,
    cfg_selects: HashMap<(String, String), (Entity<crate::os_select::OsSelect>, Subscription)>,
    cfg_fields: HashMap<(String, String), (Entity<InputState>, Subscription)>,
    _subs: Vec<Subscription>,
}

/// The stored value's base (`key[#part]`) and configuration (after a tab:
/// `input=value` pairs joined by `;`, model custom/reference.rs).
fn split_value(value: &str) -> (&str, ConfigValues) {
    let Some((base, cfg)) = value.split_once('\t') else { return (value, ConfigValues::default()) };
    let mut out = ConfigValues::default();
    for kv in cfg.split(';').filter(|s| !s.is_empty()) {
        if let Some((k, x)) = kv.split_once('=') {
            out.0.insert(k.to_string(), x.to_string());
        }
    }
    (base, out)
}

fn join_value(base: &str, cfg: &ConfigValues) -> String {
    if cfg.0.is_empty() {
        return base.to_string();
    }
    let pairs: Vec<String> = cfg.0.iter().map(|(k, v)| format!("{k}={v}")).collect();
    format!("{base}\t{}", pairs.join(";"))
}

/// A document's versions (named ones newest first, then "Start") and name.
pub(crate) fn document_versions(path: &std::path::Path) -> Option<(String, Vec<crate::derived::VersionChoice>)> {
    let store = noshape_store::Document::open_read_only(path).ok()?;
    let graph = store.graph().ok()?;
    let mut versions: Vec<crate::derived::VersionChoice> = graph.versions.iter().rev().map(|v| crate::derived::VersionChoice { label: v.name.clone(), microversion: v.microversion }).collect();
    if let Some(root) = graph.microversions.iter().find(|m| m.parents.is_empty()) {
        versions.push(crate::derived::VersionChoice { label: "Start".into(), microversion: root.id });
    }
    let name = store.meta().map(|m| m.name).unwrap_or_else(|_| path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default());
    Some((name, versions))
}

/// Changes on Main since microversion `mv`.
pub(crate) fn changes_since(path: &std::path::Path, mv: noshape_store::MicroversionId) -> usize {
    let Ok(store) = noshape_store::Document::open_read_only(path) else { return 0 };
    let Ok(history) = store.default_branch().and_then(|b| store.history(b)) else { return 0 };
    history.iter().take_while(|m| m.id != mv).count()
}

/// The tabs of a document at a version, keyed by `ext:` references.
fn version_sources(path: &std::path::Path, mv: noshape_store::MicroversionId, except: &str) -> Vec<RefSource> {
    let Ok(doc) = crate::derived::external_snapshot(path, mv) else { return Vec::new() };
    let key = |tab: &str| crate::derived::ExternalRef { path: path.to_path_buf(), microversion: mv, tab: tab.to_string() };
    crate::fs_reference::sources(&doc, except, |tab| crate::derived::external_context(&key(tab)).map(|c| crate::fs_reference::parts_and_sketches(&c)).unwrap_or_default())
        .into_iter()
        .map(|s| RefSource { tab: key(&s.tab).key(), ..s })
        .collect()
}

/// Title, search placeholder and "No … available." text.
fn texts(PanelMode::Reference(kind): PanelMode) -> (&'static str, &'static str, &'static str) {
    match kind {
        ReferenceKind::Csv => ("Select CSV file", "Search CSV files", "No CSV files available."),
        ReferenceKind::Image => ("Select image", "Search image files", "No image files available."),
        ReferenceKind::Json => ("Select JSON file", "Search JSON files", "No JSON files available."),
        ReferenceKind::Text => ("Select text file", "Search text files", "No text files available."),
        ReferenceKind::PartStudio => ("Select Part Studio", "Search entities or features", "No Part Studios available."),
        ReferenceKind::VariableStudio => ("Select Variable Studio", "Search Variable Studios", "No Variable Studios available."),
        ReferenceKind::CadImport => ("Select CAD file", "Search CAD files", "No CAD files available."),
    }
}

impl SelectItemPanel {
    pub fn new(setup: PanelSetup, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let PanelSetup { mode, sources, doc_name, doc_path, except, gpu, value } = setup;
        let search = cx.new(|cx| InputState::new(window, cx));
        let url = cx.new(|cx| InputState::new(window, cx));
        let subs = vec![
            cx.subscribe(&search, |_: &mut Self, _, ev: &InputEvent, cx| {
                if matches!(ev, InputEvent::Change) {
                    cx.notify();
                }
            }),
            cx.subscribe(&url, |this: &mut Self, st, ev: &InputEvent, cx| match ev {
                InputEvent::PressEnter { .. } => {
                    let text = st.read(cx).value().trim().to_string();
                    this.search_documents(text, cx);
                }
                InputEvent::Change => cx.notify(),
                _ => {}
            }),
        ];
        let picked = source_of(split_value(&value).0);
        let focused = (matches!(mode, PanelMode::Reference(ReferenceKind::PartStudio)) && !picked.is_empty()).then_some(picked);
        let current = doc_path.as_deref().and_then(document_versions).map(|(name, versions)| VersionedDoc { path: doc_path.clone().unwrap_or_default(), name, versions, version: None, changes: 0, sources: None });
        SelectItemPanel {
            mode,
            sources,
            doc_name,
            doc_path,
            except,
            gpu,
            thumbs: HashMap::new(),
            value,
            other_documents: false,
            search,
            expanded: focused.iter().cloned().collect(),
            focused,
            graph: false,
            current,
            url,
            category: None,
            doc_query: None,
            docs: None,
            doc_info: HashMap::new(),
            other: None,
            cfg_edit: HashMap::new(),
            cfg_selects: HashMap::new(),
            cfg_fields: HashMap::new(),
            _subs: subs,
        }
    }

    fn accepts(&self, s: &RefSource) -> bool {
        let PanelMode::Reference(kind) = self.mode;
        crate::fs_reference::accepts(kind, s)
    }

    /// Load the tabs of the shown document at its chosen version.
    fn load_version_sources(&mut self, other: bool, cx: &mut Context<Self>) {
        let me = self.doc_path.clone();
        let except_tab = self.except.clone();
        let doc = if other { self.other.as_mut() } else { self.current.as_mut() };
        let Some(d) = doc else { return };
        let Some(v) = d.version.and_then(|i| d.versions.get(i)).cloned() else { return };
        d.sources = None;
        d.changes = 0;
        let path = d.path.clone();
        let path2 = path.clone();
        let except = if Some(&path) == me.as_ref() { except_tab } else { String::new() };
        cx.spawn(async move |this, cx| {
            let (list, changes) = cx.background_spawn(async move { (version_sources(&path, v.microversion, &except), changes_since(&path, v.microversion)) }).await;
            let _ = this.update(cx, |this, cx| {
                if let Some(d) = if other { this.other.as_mut() } else { this.current.as_mut() }
                    && d.version.and_then(|i| d.versions.get(i)).is_some_and(|x| x.microversion == v.microversion)
                {
                    d.sources = Some(list);
                    d.changes = changes;
                }
                this.render_version_pictures(path2, v.microversion, cx);
                cx.notify();
            });
        })
        .detach();
    }

    /// Pictures of the Part Studios of a document at a version (keyed by
    /// their `ext:` values).
    fn render_version_pictures(&mut self, path: PathBuf, mv: noshape_store::MicroversionId, cx: &mut Context<Self>) {
        let wants = self.mode == PanelMode::Reference(ReferenceKind::PartStudio);
        let Some(gpu) = self.gpu.clone().filter(|_| wants) else { return };
        let Ok(doc) = crate::derived::external_snapshot(&path, mv) else { return };
        let studios: Vec<noshape_model::document::TabId> = doc.tabs.iter().filter(|t| matches!(t.content, noshape_model::document::TabContent::PartStudio(_))).map(|t| t.id).collect();
        let doc = (*doc).clone();
        cx.spawn(async move |this, cx| {
            let key = move |t: noshape_model::document::TabId| crate::derived::ExternalRef { path: path.clone(), microversion: mv, tab: t.to_string() }.key();
            let pictures = cx.background_spawn(crate::fs_reference::render_thumbnails(gpu, doc, studios, key)).await;
            let _ = this.update(cx, |this, cx| {
                this.thumbs.extend(pictures);
                cx.notify();
            });
        })
        .detach();
    }

    /// The documents of this computer (Other documents), and per document
    /// its latest version and whether it has matching tabs (others grey).
    fn ensure_documents(&mut self, cx: &mut Context<Self>) {
        if self.docs.is_some() {
            return;
        }
        self.docs = Some(Vec::new());
        let PanelMode::Reference(kind) = self.mode;
        cx.spawn(async move |this, cx| {
            let (docs, info) = cx
                .background_spawn(async move {
                    let docs = crate::start_library::Library::default().scan(&crate::start_library::ScanInput::current()).docs;
                    let info: HashMap<PathBuf, (String, bool)> = docs
                        .iter()
                        .filter_map(|d| {
                            let (_, versions) = document_versions(&d.path)?;
                            let latest = versions.first()?;
                            let any = version_sources(&d.path, latest.microversion, "").iter().any(|s| crate::fs_reference::accepts(kind, s));
                            Some((d.path.clone(), (latest.label.clone(), any)))
                        })
                        .collect();
                    (docs, info)
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.docs = Some(docs);
                this.doc_info = info;
                cx.notify();
            });
        })
        .detach();
    }

    /// "Search or paste URL": a document path opens it, other text lists
    /// the documents whose name contains it.
    fn search_documents(&mut self, text: String, cx: &mut Context<Self>) {
        let path = PathBuf::from(text.trim_start_matches("file://"));
        if path.extension().is_some_and(|e| e == crate::start_library::EXT) && path.exists() {
            self.open_other_document(path, cx);
            return;
        }
        self.ensure_documents(cx);
        self.other = None;
        self.doc_query = (!text.is_empty()).then_some(text);
        cx.notify();
    }

    /// Open another document at its latest version (live 604: never its workspace).
    fn open_other_document(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let Some((name, versions)) = document_versions(&path) else { return };
        self.other = Some(VersionedDoc { path, name, version: (!versions.is_empty()).then_some(0), versions, changes: 0, sources: None });
        self.graph = false;
        self.load_version_sources(true, cx);
        cx.notify();
    }

    fn pick(&mut self, value: String, cx: &mut Context<Self>) {
        if matches!(self.mode, PanelMode::Reference(ReferenceKind::PartStudio)) {
            let studio = source_of(split_value(&value).0);
            self.focused = (!studio.is_empty()).then(|| {
                self.expanded.insert(studio.clone());
                studio
            });
        }
        self.value = value.clone();
        cx.emit(SelectItemEvent::Picked(value));
        cx.notify();
    }

    /// The rows of `list` matching the search (reference Part Studios
    /// expand to their parts; the picked one light blue).
    fn source_rows(&mut self, list: &[RefSource], query: &str, window: &mut Window, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let pal = p();
        let thumbs = self.thumbs.clone();
        let current = split_value(&self.value).0.to_string();
        let expandable = matches!(self.mode, PanelMode::Reference(ReferenceKind::PartStudio));
        let row = |id: SharedString, indent: f32, expander: Option<AnyElement>, key: &str, icon: &'static str, name: String, on: bool, cx: &App| {
            // A picture of the Part Studio or part when rendered, else its icon.
            let picture = match thumbs.get(key) {
                Some(img) => gpui_kit::img(img.clone()).w(px(70.)).h(px(40.)).object_fit(ObjectFit::Contain).into_any_element(),
                None => crate::assets::icon(icon, 30., cx),
            };
            h_flex()
                .id(id)
                .h(px(40.))
                .flex_none()
                .pl(px(indent))
                .when(on, |d| d.bg(gpui_kit::rgb(0xb2ddf6)))
                .when(!on, |d| d.hover(|d| d.bg(p().panel_hover)))
                .cursor_pointer()
                .children(expander)
                .child(div().w(px(70.)).h(px(40.)).flex_none().flex().items_center().justify_center().child(picture))
                .child(div().flex_1().min_w_0().pl(px(4.)).truncate().text_size(px(13.33)).font_weight(FontWeight::LIGHT).text_color(p().text).child(name.clone()))
                .tooltip(crate::ui::tooltip(name))
        };
        let mut out = Vec::new();
        let accepted: Vec<RefSource> = list.iter().filter(|s| self.accepts(s) && (self.other_documents || self.focused.as_ref().is_none_or(|f| *f == s.tab))).cloned().collect();
        for s in &accepted {
            let parts = match &s.what {
                RefSourceKind::PartStudio(parts) if expandable => Some(parts.clone()),
                _ => None,
            };
            let name_hit = query.is_empty() || s.name.to_lowercase().contains(query);
            let part_hits: Vec<(noshape_model::BodyId, String)> = parts.iter().flatten().filter(|(_, n)| name_hit || n.to_lowercase().contains(query)).cloned().collect();
            if !name_hit && part_hits.is_empty() {
                continue;
            }
            let icon = match &s.what {
                RefSourceKind::File(FileKind::Image, _) => "image",
                RefSourceKind::File(FileKind::Cad, _) => "file-cad",
                RefSourceKind::File(..) => "file",
                RefSourceKind::PartStudio(_) => "part-studio",
                RefSourceKind::VariableStudio => "variable-studio",
            };
            let key = s.tab.clone();
            let open = parts.is_some() && (self.expanded.contains(&s.tab) || !query.is_empty());
            // The expander toggles the parts; the rest of the row picks.
            let expander = parts.as_ref().map(|_| {
                let t = s.tab.clone();
                div()
                    .id(SharedString::from(format!("sel-exp-{}", s.tab)))
                    .w(px(18.))
                    .h_full()
                    .flex()
                    .flex_none()
                    .items_center()
                    .pl(px(4.))
                    .child(crate::ui::glyph(if open { "expander-down" } else { "expander-right" }, 10., pal.text))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        cx.stop_propagation();
                        if !this.expanded.remove(&t) {
                            this.expanded.insert(t.clone());
                        }
                        cx.notify();
                    }))
                    .into_any_element()
            });
            let mut r = row(SharedString::from(format!("sel-src-{}", s.tab)), 0., expander, &s.tab, icon, s.name.clone(), current == s.tab, cx);
            // Configured studios: "Set to last viewed configuration" ▾ on the row.
            if s.configuration.is_some() {
                let k2 = s.tab.clone();
                r = r.child(
                    h_flex()
                        .mr(px(10.))
                        .w(px(57.))
                        .h(px(30.))
                        .flex_none()
                        .rounded(px(3.))
                        .border_1()
                        .border_color(gpui_kit::rgb(0xaaaaaa))
                        .child(
                            div()
                                .id(SharedString::from(format!("sel-cfg-last-{}", s.tab)))
                                .w(px(35.))
                                .h_full()
                                .flex()
                                .items_center()
                                .justify_center()
                                .border_r_1()
                                .border_color(gpui_kit::rgb(0xaaaaaa))
                                .tooltip(crate::ui::tooltip("Set to last viewed configuration"))
                                .child(crate::assets::icon("configuration", 20., cx))
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    cx.stop_propagation();
                                    if let Some(v) = k2.parse::<u64>().ok().or_else(|| u64::from_str_radix(&k2, 16).ok()).and_then(|t| crate::release_dialog::last_viewed_config(noshape_model::TabId(t))) {
                                        this.cfg_edit.insert(k2.clone(), v);
                                        this.expanded.insert(k2.clone());
                                    }
                                    cx.notify();
                                })),
                        )
                        .child(div().w(px(20.)).h_full().flex().items_center().justify_center().child(crate::ui::glyph("ui-caret-down", 10., gpui_kit::rgb(0x000000)))),
                );
            }
            out.push(r.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                // Picking a configured studio keeps the configuration shown.
                let cfg = this.cfg_edit.get(&key).cloned().unwrap_or_default();
                this.pick(join_value(&key, &cfg), cx);
            })).into_any_element());
            if open && let Some(cfg) = s.configuration.clone() {
                let tab = s.tab.clone();
                out.push(self.config_editor(&tab, &cfg, window, cx));
            }
            // Live 409 (qa5 DER10): a changed configuration empties the
            // contents until Generate lists them again.
            let pending = s.configuration.as_ref().is_some_and(|c| self.config_changed(&s.tab, c).0);
            if open && !pending {
                for (body, name) in part_hits {
                    let value = crate::fs_reference::part_value(&s.tab, &body);
                    let on = current == value;
                    let studio = s.tab.clone();
                    let icon = if body.index == u32::MAX { "sketch" } else { "part" };
                    out.push(row(SharedString::from(format!("sel-part-{value}")), 36., None, &value, icon, name, on, cx).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        let cfg = this.cfg_edit.get(&studio).cloned().unwrap_or_default();
                        this.pick(join_value(&value, &cfg), cx);
                    })).into_any_element());
                }
            }
        }
        out
    }

    /// A configured Part Studio's inputs under its row (live fs-refcfg-panel:
    /// 40 px indent, Size ▾, ☐ Hole, Len, Txt) and Generate, pale until a
    /// value changes; Generate applies the configuration to the field.
    /// Whether `key`'s configuration inputs were changed since the last
    /// Generate, and the values being edited.
    fn config_changed(&self, key: &str, cfg: &Configuration) -> (bool, ConfigValues) {
        let (base, applied) = split_value(&self.value);
        let applied = if source_of(base) == key { applied } else { ConfigValues::default() };
        let mut effective = cfg.defaults();
        effective.0.extend(applied.0.clone());
        let editing = self.cfg_edit.get(key).cloned().unwrap_or_else(|| effective.clone());
        (editing != effective, editing)
    }

    fn config_editor(&mut self, key: &str, cfg: &Configuration, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let (changed, editing) = self.config_changed(key, cfg);
        // Live 741: labels only as wide as their text, the editor fills the rest.
        let label = |t: String| div().pt(px(7.)).px(px(5.)).flex_none().text_size(px(12.)).line_height(px(16.)).text_color(gpui_kit::rgb(0x666666)).child(t);
        let mut col = v_flex().pl(px(40.)).pr(px(26.));
        for input in &cfg.inputs {
            let current = editing.0.get(&input.id).cloned().unwrap_or_default();
            let k = (key.to_string(), input.id.clone());
            let control: AnyElement = match &input.kind {
                ConfigInputKind::List { options, .. } => {
                    let items: Vec<(SharedString, SharedString)> = options.iter().map(|o| (SharedString::from(o.id.clone()), SharedString::from(o.name.clone()))).collect();
                    let sel = match self.cfg_selects.get(&k) {
                        Some((s, _)) => s.clone(),
                        None => {
                            let s = cx.new(|_| crate::os_select::OsSelect::new(SharedString::from(format!("sel-cfg-{key}-{}", input.id))));
                            let (key2, iid) = (key.to_string(), input.id.clone());
                            let base_vals = editing.clone();
                            let sub = cx.subscribe(&s, move |this: &mut Self, _, ev: &crate::os_select::OsSelectEvent, cx| {
                                let crate::os_select::OsSelectEvent::Picked(id) = ev;
                                this.cfg_edit.entry(key2.clone()).or_insert_with(|| base_vals.clone()).0.insert(iid.clone(), id.to_string());
                                cx.notify();
                            });
                            self.cfg_selects.insert(k.clone(), (s.clone(), sub));
                            s
                        }
                    };
                    sel.update(cx, |s, _| s.set_items(items, current.clone()));
                    h_flex().h(px(28.)).child(label(input.name.clone())).child(div().flex_1().child(sel)).into_any_element()
                }
                ConfigInputKind::Checkbox { .. } => {
                    let on = current == "true";
                    let (key2, iid, base_vals) = (key.to_string(), input.id.clone(), editing.clone());
                    crate::ui::os_checkbox(SharedString::from(format!("sel-cfg-cb-{key}-{}", input.id)), input.name.clone(), on)
                        .h(px(27.))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.cfg_edit.entry(key2.clone()).or_insert_with(|| base_vals.clone()).0.insert(iid.clone(), (!on).to_string());
                            cx.notify();
                        }))
                        .into_any_element()
                }
                ConfigInputKind::Variable { .. } | ConfigInputKind::Text { .. } => {
                    // Quantities read right-aligned ("10 mm"), text left (live 741).
                    let right = matches!(&input.kind, ConfigInputKind::Variable { kind, .. } if *kind != noshape_model::configuration::VariableKind::Text);
                    let field = match self.cfg_fields.get(&k) {
                        Some((f, _)) => f.clone(),
                        None => {
                            let f = cx.new(|cx| InputState::new(window, cx).default_value(current.clone()));
                            let (key2, iid, base_vals) = (key.to_string(), input.id.clone(), editing.clone());
                            let sub = cx.subscribe(&f, move |this: &mut Self, st, ev: &InputEvent, cx| {
                                if matches!(ev, InputEvent::Change) {
                                    let text = st.read(cx).value().to_string();
                                    this.cfg_edit.entry(key2.clone()).or_insert_with(|| base_vals.clone()).0.insert(iid.clone(), text);
                                    cx.notify();
                                }
                            });
                            self.cfg_fields.insert(k.clone(), (f.clone(), sub));
                            f
                        }
                    };
                    h_flex()
                        .h(px(26.))
                        .child(label(input.name.clone()))
                        .child(gpui_kit::base::input::input_frame(div().flex_1().border_b_1().border_color(gpui_kit::rgb(0xd7d7d7)).child(Input::new(&field).appearance(false).text_size(px(13.33)).h(px(25.)).when(right, |i| i.text_align(TextAlign::Right)))))
                        .into_any_element()
                }
            };
            col = col.child(control);
        }
        let key2 = key.to_string();
        col.child(
            div().pt(px(2.)).pb(px(2.)).child(
                div()
                    .id(SharedString::from(format!("sel-cfg-generate-{key}")))
                    .ml(px(3.))
                    .w(px(81.))
                    .h(px(30.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(3.))
                    .border_1()
                    .border_color(gpui_kit::rgb(0x959595))
                    .text_size(px(14.67))
                    .text_color(pal.text)
                    .when(!changed, |d| d.opacity(0.25))
                    .when(changed, |d| d.cursor_pointer().hover(|d| d.bg(pal.panel_hover)))
                    .child("Generate")
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        let Some(cfg) = this.cfg_edit.remove(&key2) else { return };
                        let (base, _) = split_value(&this.value);
                        let base = if source_of(base) == key2 { base.to_string() } else { key2.clone() };
                        this.pick(join_value(&base, &cfg), cx);
                    })),
            ),
        )
        .into_any_element()
    }

    /// The document row: (home ‹ for another document) name and "⑂ Main" /
    /// "⑂ V2", then Create version… and Version graph (live 482, 604).
    fn document_header(&self, name: String, version: String, other: bool, path: Option<PathBuf>, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let graph = self.graph;
        let square = |id: &'static str, icon: &'static str, tip: &'static str, on: bool, cx: &App| {
            div()
                .id(id)
                .size(px(30.))
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .rounded(px(3.))
                .border_1()
                .border_color(gpui_kit::rgb(0xaaaaaa))
                .cursor_pointer()
                .when(on, |d| d.bg(gpui_kit::rgb(0xeaeaea)))
                .hover(|d| d.bg(p().panel_hover))
                .tooltip(crate::ui::tooltip(tip))
                .child(crate::assets::icon(icon, 20., cx))
        };
        let back_tip = self.doc_query.clone().map(|q| format!("Results for \"{q}\"")).or_else(|| self.category.map(|c| FILTERS[c].0.to_string())).unwrap_or_else(|| "Go to document filters".into());
        h_flex()
            .h(px(40.))
            .flex_none()
            .p(px(2.))
            .pl(px(if other { 2. } else { 6. }))
            .gap(px(6.))
            .border_t_1()
            .border_color(gpui_kit::rgb(0xeeeeee))
            // A picked studio shown alone: ‹ goes back to the whole list.
            .when(!other && self.focused.is_some(), |d| {
                d.pl(px(2.)).child(nav("sel-focus-back", "ui-chevron-left", "Back".into()).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.focused = None;
                    cx.notify();
                })))
            })
            .when(other, |d| {
                d.child(nav("sel-home", "home", "Go to document filters".into()).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.other = None;
                    this.category = None;
                    this.doc_query = None;
                    this.graph = false;
                    cx.notify();
                })))
                .child(nav("sel-back", "ui-chevron-left", back_tip).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.other = None;
                    this.graph = false;
                    cx.notify();
                })))
            })
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(div().truncate().text_size(px(14.67)).font_weight(FontWeight::MEDIUM).line_height(px(19.)).child(name))
                    .child(h_flex().gap_1().opacity(0.5).text_size(px(12.)).line_height(px(15.6)).child(crate::ui::glyph("branch", 10., pal.text)).child(version)),
            )
            .child(square("sel-create-version", "version", "Create version…", false, cx).on_click(cx.listener(move |_, _: &ClickEvent, _, cx| cx.emit(SelectItemEvent::CreateVersion(if other { path.clone() } else { None })))))
            .child(square("sel-version-graph", "version-graph", "Version graph", graph, cx).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.graph = !this.graph;
                // Reopening the graph shows versions created meanwhile.
                let open = this.graph;
                if let Some(d) = if other { this.other.as_mut() } else { this.current.as_mut() }
                    && open
                    && let Some((_, versions)) = document_versions(&d.path)
                {
                    let chosen = d.version.and_then(|i| d.versions.get(i)).map(|v| v.microversion);
                    d.version = chosen.and_then(|mv| versions.iter().position(|v| v.microversion == mv));
                    d.versions = versions;
                }
                cx.notify();
            })))
            .into_any_element()
    }

    /// Legend (a ring on a stem = Workspace, a dot on a line = Version),
    /// then Main and the versions on a blue line, Start last with an end
    /// bar; the chosen one light blue. Main can be chosen only in the
    /// current document (live 608, 626).
    fn render_version_graph(&self, other: bool, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let blue = gpui_kit::rgb(BLUE);
        let Some(d) = (if other { self.other.as_ref() } else { self.current.as_ref() }) else { return div().into_any_element() };
        let n = d.versions.len();
        let node = |hollow: bool, first: bool, last: bool| {
            div()
                .w(px(140.))
                .h_full()
                .flex_none()
                .relative()
                .flex()
                .items_center()
                .justify_center()
                .child(div().absolute().left(px(69.)).w(px(2.)).top(px(if first { 18. } else { 0. })).bottom(px(if last { 9. } else { 0. })).bg(blue))
                .when(last, |d| d.child(div().absolute().left(px(64.)).w(px(12.)).h(px(2.)).bottom(px(8.)).bg(blue)))
                .child(div().size(px(if hollow { 13. } else { 9. })).rounded_full().when(hollow, |d| d.border_2()).border_color(blue).bg(if hollow { gpui_kit::rgb(0xffffff) } else { blue }))
        };
        let mut col = v_flex().id("sel-graph").flex_1().min_h_0().overflow_y_scroll().child(
            h_flex()
                .h(px(40.))
                .flex_none()
                .pl(px(6.))
                .gap(px(4.))
                .text_size(px(12.))
                .border_b_1()
                .border_color(gpui_kit::rgb(0xeeeeee))
                .child(v_flex().items_center().child(div().size(px(9.)).rounded_full().border_2().border_color(pal.text)).child(div().w(px(2.)).h(px(5.)).bg(pal.text)))
                .child("Workspace")
                .child(div().ml(px(6.)).w(px(8.)).h(px(14.)).relative().flex().items_center().justify_center().child(div().absolute().left(px(3.)).top_0().bottom_0().w(px(2.)).bg(pal.text)).child(div().size(px(7.)).rounded_full().bg(pal.text)))
                .child("Version"),
        );
        let main_on = d.version.is_none();
        col = col.child(
            h_flex()
                .id("sel-v-main")
                .h(px(36.))
                .flex_none()
                .when(main_on && !other, |r| r.bg(gpui_kit::rgb(0xb2ddf6)))
                .when(!other, |r| r.cursor_pointer())
                .child(node(true, true, n == 0))
                .child(div().flex_1().text_size(px(14.)).when(other, |d| d.italic().text_color(gpui_kit::rgb(0x999999))).child("Main"))
                .tooltip(crate::ui::tooltip("Main"))
                .when(!other, |r| {
                    r.on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        if let Some(d) = this.current.as_mut() {
                            d.version = None;
                            d.sources = None;
                        }
                        this.graph = false;
                        cx.notify();
                    }))
                }),
        );
        for (i, v) in d.versions.iter().enumerate() {
            let on = d.version == Some(i);
            col = col.child(
                h_flex()
                    .id(SharedString::from(format!("sel-v-{i}")))
                    .h(px(36.))
                    .flex_none()
                    .cursor_pointer()
                    .when(on, |r| r.bg(gpui_kit::rgb(0xb2ddf6)))
                    .when(!on, |r| r.hover(|r| r.bg(pal.panel_hover)))
                    .child(node(false, false, i + 1 == n))
                    .child(div().flex_1().text_size(px(14.)).child(v.label.clone()))
                    .tooltip(crate::ui::tooltip(v.label.clone()))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if let Some(d) = if other { this.other.as_mut() } else { this.current.as_mut() } {
                            d.version = Some(i);
                        }
                        this.graph = false;
                        this.load_version_sources(other, cx);
                        cx.notify();
                    })),
            );
        }
        col.into_any_element()
    }

    /// Other documents before a document is opened: the filter and
    /// "Search or paste URL" row, then the filters (live 602) or a filter's /
    /// search's documents (603): folders first, documents with their
    /// picture, name and latest version; those without matching tabs grey.
    fn render_document_browser(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let blue = gpui_kit::rgb(BLUE);
        let (category, query, url) = (self.category, self.doc_query.clone(), self.url.clone());
        let url_empty = url.read(cx).value().is_empty();
        let placeholder = match (category, &query) {
            (Some(c), None) => format!("Search in \"{}\"", FILTERS[c].0),
            _ => "Search or paste URL".to_string(),
        };
        let col = v_flex().flex_1().min_h_0().child(
            h_flex()
                .h(px(39.))
                .flex_none()
                .pt(px(6.))
                .px(px(6.))
                .child(div().id("sel-doc-filters").w(px(32.)).h(px(30.)).flex_none().flex().items_center().justify_center().child(crate::ui::glyph("filter", 20., blue)).tooltip(crate::ui::tooltip("Show advanced search options")))
                .child(
                    div()
                        .ml(px(6.))
                        .flex_1()
                        .relative()
                        .h(px(30.))
                        .rounded_l(px(3.))
                        .border_1()
                        .border_color(gpui_kit::rgb(0xcccccc))
                        .child(crate::ui::input_with_placeholder(&url, placeholder, cx).appearance(false).cleanable(true).text_size(px(12.)).h(px(28.)).pl(px(6.))),
                )
                .child(
                    div()
                        .id("sel-doc-search")
                        .w(px(34.))
                        .h(px(30.))
                        .flex()
                        .flex_none()
                        .items_center()
                        .justify_center()
                        .border_1()
                        .border_color(gpui_kit::rgb(0xaaaaaa))
                        .rounded_r(px(3.))
                        .when(url_empty, |d| d.opacity(0.25))
                        .cursor_pointer()
                        .child(crate::ui::glyph("search", 20., pal.text))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            let text = this.url.read(cx).value().trim().to_string();
                            this.search_documents(text, cx);
                        })),
                ),
        );
        if category.is_none() && query.is_none() {
            // The filters: 32 px rows, 20 px icons.
            let mut list = v_flex().pt(px(10.)).pl(px(10.));
            for (i, (label, icon)) in FILTERS.iter().enumerate() {
                list = list.child(
                    h_flex()
                        .id(SharedString::from(format!("sel-filter-{i}")))
                        .h(px(32.))
                        .pl(px(6.))
                        .gap(px(6.))
                        .cursor_pointer()
                        .text_size(px(14.67))
                        .font_weight(FontWeight::LIGHT)
                        .hover(|d| d.bg(pal.panel_hover))
                        .child(crate::assets::icon(icon, 20., cx))
                        .child(*label)
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.ensure_documents(cx);
                            this.category = Some(i);
                            cx.notify();
                        })),
                );
            }
            return col.child(list).into_any_element();
        }
        let title = query.clone().map(|q| format!("Results for \"{q}\"")).or_else(|| category.map(|c| FILTERS[c].0.to_string())).unwrap_or_default();
        let home = |this: &mut Self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>| {
            this.category = None;
            this.doc_query = None;
            cx.notify();
        };
        let col = col.child(
            h_flex()
                .h(px(40.))
                .flex_none()
                .p(px(2.))
                .gap(px(3.))
                .border_t_1()
                .border_color(gpui_kit::rgb(0xeeeeee))
                .child(nav("sel-docs-home", "home", "Go to document filters".into()).on_click(cx.listener(home)))
                .child(nav("sel-docs-back", "ui-chevron-left", "Filters".into()).on_click(cx.listener(home)))
                .child(div().pl(px(3.)).text_size(px(14.67)).font_weight(FontWeight::MEDIUM).truncate().child(title)),
        );
        let me = self.doc_path.clone();
        let mut docs: Vec<crate::start_library::DocEntry> = self.docs.clone().unwrap_or_default().into_iter().filter(|d| !d.trashed && !d.broken && Some(&d.path) != me.as_ref()).collect();
        let q = query.as_ref().map(|q| q.to_lowercase());
        match (category, &q) {
            (_, Some(q)) => docs.retain(|d| d.name.to_lowercase().contains(q.as_str())),
            // Local documents are all the user's own; none are shared or public.
            (Some(4 | 5), _) => docs.clear(),
            (Some(0 | 2), _) => {
                docs.retain(|d| d.recent_rank.is_some());
                docs.sort_by_key(|d| d.recent_rank);
            }
            _ => docs.sort_by_key(|d| std::cmp::Reverse(d.modified)),
        }
        let mut list = v_flex().id("sel-docs").flex_1().min_h_0().overflow_y_scroll();
        // Owned by me: the folders first (folder outline, bold name).
        if category == Some(1) && q.is_none() {
            let mut dirs: Vec<PathBuf> = docs.iter().map(|d| d.folder().to_path_buf()).collect();
            dirs.sort();
            dirs.dedup();
            if dirs.len() > 1 {
                for (i, dir) in dirs.iter().enumerate() {
                    let name = dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                    list = list.child(
                        h_flex()
                            .id(SharedString::from(format!("sel-dir-{i}")))
                            .h(px(50.))
                            .flex_none()
                            .px(px(2.))
                            .hover(|d| d.bg(gpui_kit::rgb(0xdef1fb)))
                            .tooltip(crate::ui::tooltip(name.clone()))
                            .child(div().w(px(74.)).h(px(40.)).flex_none().flex().items_center().justify_center().child(crate::ui::glyph("folder", 28., gpui_kit::rgb(0x666666))))
                            .child(div().flex_1().min_w_0().truncate().text_size(px(14.67)).font_weight(FontWeight::MEDIUM).child(name)),
                    );
                }
            }
        }
        if docs.is_empty() && self.docs.as_ref().is_some_and(|d| !d.is_empty()) {
            list = list.child(div().pl(px(10.)).h(px(28.)).flex().items_center().text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).child("No documents found"));
        }
        for (i, d) in docs.into_iter().enumerate() {
            let (version, has) = self.doc_info.get(&d.path).cloned().unwrap_or_default();
            let thumb = match d.thumbnail.clone() {
                Some(img) => gpui_kit::img(img).w(px(70.)).h(px(40.)).object_fit(ObjectFit::Contain).into_any_element(),
                None => crate::assets::icon("document", 32., cx),
            };
            let path = d.path.clone();
            let tip = if version.is_empty() { d.name.clone() } else { format!("{} -- {version}", d.name) };
            list = list.child(
                h_flex()
                    .id(SharedString::from(format!("sel-doc-{i}")))
                    .h(px(50.))
                    .flex_none()
                    .px(px(2.))
                    .cursor_pointer()
                    .when(!has, |r| r.opacity(0.35))
                    .hover(|r| r.bg(gpui_kit::rgb(0xdef1fb)))
                    .tooltip(crate::ui::tooltip(tip))
                    .child(div().w(px(74.)).h(px(40.)).flex_none().flex().items_center().justify_center().child(thumb))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(div().truncate().text_size(px(14.67)).font_weight(FontWeight::MEDIUM).line_height(px(19.)).child(d.name.clone()))
                            .when(!version.is_empty(), |c| c.child(div().opacity(0.5).text_size(px(12.)).line_height(px(15.6)).child(version))),
                    )
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.open_other_document(path.clone(), cx))),
            );
        }
        col.child(list).into_any_element()
    }
}

/// The studio a value (`key` or `key#part`, `ext:mv|tab#part|path`) is about.
fn source_of(base: &str) -> String {
    match base.strip_prefix("ext:") {
        Some(rest) => {
            let mut it = rest.splitn(3, '|');
            match (it.next(), it.next(), it.next()) {
                (Some(mv), Some(tab), Some(path)) => format!("ext:{mv}|{}|{path}", tab.split('#').next().unwrap_or(tab)),
                _ => base.to_string(),
            }
        }
        None => base.split('#').next().unwrap_or(base).to_string(),
    }
}

/// A 30 px navigation button (home, ‹) with a dark icon.
fn nav(id: &'static str, icon: &'static str, tip: String) -> Stateful<Div> {
    div()
        .id(id)
        .size(px(30.))
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .rounded(px(3.))
        .cursor_pointer()
        .hover(|d| d.bg(gpui_kit::rgb(0xeaeaea)))
        .tooltip(crate::ui::tooltip(tip))
        .child(crate::ui::glyph(icon, if icon == "ui-chevron-left" { 16. } else { 20. }, gpui_kit::rgb(0x333333)))
}

impl Render for SelectItemPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let (other, graph, mode) = (self.other_documents, self.graph, self.mode);
        let search = self.search.clone();
        let query = search.read(cx).value().trim().to_lowercase();
        let (title, search_placeholder, none) = texts(mode);
        let import_file = matches!(mode, PanelMode::Reference(ReferenceKind::Csv | ReferenceKind::Image));
        let blue = gpui_kit::rgb(BLUE);
        let line = gpui_kit::rgb(0xeeeeee);
        let tab = |id: &'static str, label: &'static str, on: bool, cx: &mut Context<Self>| {
            div()
                .id(id)
                .w(px(150.))
                .h_full()
                .p(px(5.))
                .flex()
                .justify_center()
                .text_size(px(12.))
                .line_height(px(21.6))
                .cursor_pointer()
                .when(on, |d| d.text_color(blue).font_weight(FontWeight::MEDIUM).border_b_2().border_color(blue))
                .when(!on, |d| d.text_color(p().text))
                .child(label)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    let to_other = id == "sel-other";
                    if this.other_documents != to_other {
                        this.other_documents = to_other;
                        this.graph = false;
                        // Live: switching to Other documents clears a reference field.
                        if to_other {
                            this.pick(String::new(), cx);
                        }
                        cx.notify();
                    }
                }))
        };
        let mut body = v_flex().flex_1().min_h_0();
        let shown = if other { self.other.as_ref() } else { self.current.as_ref() };
        if other && shown.is_none() {
            body = body.child(self.render_document_browser(cx));
        } else {
            let (doc_name, doc_path, version_label) = match shown {
                Some(d) => (d.name.clone(), Some(d.path.clone()), d.version.and_then(|i| d.versions.get(i)).map_or_else(|| "Main".to_string(), |v| v.label.clone())),
                None => (self.doc_name.clone(), None, "Main".to_string()),
            };
            body = body.child(self.document_header(doc_name.clone(), version_label, other, doc_path.clone(), cx));
            if graph {
                body = body.child(self.render_version_graph(other, cx));
            } else {
                // The placeholder is italic grey (live 482), drawn over the field.
                body = body.child(
                    h_flex().h(px(42.)).flex_none().pt(px(6.)).px(px(6.)).child(
                        div()
                            .flex_1()
                            .relative()
                            .h(px(30.))
                            .rounded(px(3.))
                            .border_1()
                            .border_color(gpui_kit::rgb(0xcccccc))
                            .child(crate::ui::input_with_placeholder(&search, search_placeholder, cx).appearance(false).cleanable(true).text_size(px(12.)).h(px(28.)).pl(px(5.))),
                    ),
                );
                // At a version with later changes: the blue box (live 604, 627b).
                let (list, changes, loading) = match shown {
                    Some(d) if d.version.is_some() => (d.sources.clone().unwrap_or_default(), d.changes, d.sources.is_none()),
                    _ => (self.sources.clone(), 0, false),
                };
                let mut rows = v_flex().id("sel-list").flex_1().min_h_0().pt(px(1.)).overflow_y_scroll();
                if changes > 0 {
                    let link = format!("Create a version in {doc_name}.");
                    let target = if other { doc_path.clone() } else { None };
                    rows = rows.child(
                        v_flex()
                            .m(px(4.))
                            .p(px(4.))
                            .pl(px(5.))
                            .rounded(px(5.25))
                            .border_1()
                            .border_color(gpui_kit::rgb(0xb2ddf6))
                            .bg(gpui_kit::rgb(0xdef1fb))
                            .text_size(px(14.))
                            .line_height(px(21.6))
                            .text_color(gpui_kit::rgb(0x094174))
                            .child(format!("{changes} change{} since the last version.", if changes == 1 { "" } else { "s" }))
                            .child(
                                div()
                                    .id("sel-create-version-link")
                                    .py(px(4.))
                                    .flex()
                                    .justify_center()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .underline()
                                    .cursor_pointer()
                                    .child(link)
                                    .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| cx.emit(SelectItemEvent::CreateVersion(target.clone())))),
                            ),
                    );
                }
                let items = self.source_rows(&list, &query, window, cx);
                if items.is_empty() && !loading {
                    rows = rows.child(div().p(px(10.)).text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).line_height(px(28.)).text_color(pal.text).child(none));
                }
                body = body.child(rows.children(items));
            }
        }
        v_flex()
            .id("select-item-panel")
            .occlude()
            .w(px(300.))
            .h(px(559.))
            .bg(pal.dialog_bg)
            .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(5.), spread_radius: px(0.), inset: false }])
            .text_color(pal.text)
            .child(
                    h_flex()
                        .h(px(29.))
                        .flex_none()
                        .bg(gpui_kit::rgb(0xffffff))
                        .border_b_1()
                        .border_color(gpui_kit::rgb(0xd7d7d7))
                        .child(div().flex_1().pl(px(6.)).text_size(px(13.)).line_height(px(28.)).font_weight(FontWeight::MEDIUM).child(title))
                        .child(
                            div()
                                .id("sel-close")
                                .size(px(28.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .opacity(0.5)
                                .cursor_pointer()
                                // Live (403): Bootstrap's btn-close, an 11 px black
                                // cross at half opacity.
                                .child(crate::assets::icon("panel-close", 20., cx))
                                .on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(SelectItemEvent::Close))),
                        ),
            )
            .child(h_flex().h(px(32.)).flex_none().border_t_1().border_color(line).child(tab("sel-current", "Current document", !other, cx)).child(tab("sel-other", "Other documents", other, cx)))
            .child(body)
            .when(import_file, |d| {
                d.child(
                    h_flex()
                        .id("sel-import-file")
                        .h(px(23.))
                        .flex_none()
                        .p(px(3.))
                        .gap(px(2.))
                        .text_size(px(14.))
                        .text_color(gpui_kit::rgb(0x666666))
                        .cursor_pointer()
                        .child(crate::ui::glyph("import-file", 16., gpui_kit::rgb(0x666666)))
                        .child("Import…")
                        .on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(SelectItemEvent::ImportFile))),
                )
            })
    }
}
