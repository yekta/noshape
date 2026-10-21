//! Release management (Onshape "Release…", local: no approvers): the
//! "Create Release candidate" dialog (items with Revision, State and Part
//! number, Release name and notes; Save draft, Apply, Release, Close),
//! "Review Release", "Revision history" and "Create Obsolete candidate".
//! Releasing writes the part numbers to the parts, creates a version named
//! after the release and records the package in the document's release log
//! (store meta "releases", outside the editable history).

use crate::doc_model::{DocEvent, DocModel};
use crate::os_ui as os;
use crate::theme::p;
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::input::{Input, InputEvent, InputState, Textarea, TextareaState};
use gpui_kit::component::{Icon, IconName, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::document::TabId;
use noshape_model::reference::BodyId;
use noshape_model::release::{Issue, ItemKind, NumberingSchemes, Package, PackageKind, ReleaseItem, ReleaseLog};

const META_KEY: &str = "releases";

/// The configuration each Part Studio was last shown in (Add Items ›
/// "Set to last viewed configuration").
static LAST_VIEWED: parking_lot::Mutex<Option<std::collections::HashMap<TabId, noshape_model::configuration::ConfigValues>>> = parking_lot::Mutex::new(None);

pub fn note_viewed_config(tab: TabId, values: &noshape_model::configuration::ConfigValues) {
    LAST_VIEWED.lock().get_or_insert_with(Default::default).insert(tab, values.clone());
}

pub(crate) fn last_viewed_config(tab: TabId) -> Option<noshape_model::configuration::ConfigValues> {
    LAST_VIEWED.lock().as_ref().and_then(|m| m.get(&tab).cloned())
}

/// Release management settings (Onshape's Company settings › Release
/// management; here the account preferences): the revision scheme and the
/// unreleased revision suffix.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ReleaseSettings {
    #[serde(default)]
    pub scheme: noshape_model::release::RevisionScheme,
    #[serde(default = "default_suffix")]
    pub unreleased_suffix: String,
}

fn default_suffix() -> String {
    "*".into()
}

impl Default for ReleaseSettings {
    fn default() -> Self {
        Self { scheme: Default::default(), unreleased_suffix: default_suffix() }
    }
}

fn settings_path() -> std::path::PathBuf {
    crate::files::config_dir().join("release_settings.json")
}

pub fn release_settings() -> ReleaseSettings {
    std::fs::read(settings_path()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

pub fn set_release_settings(s: &ReleaseSettings) {
    let _ = std::fs::write(settings_path(), serde_json::to_vec_pretty(s).unwrap_or_default());
}

fn numbering_path() -> std::path::PathBuf {
    crate::files::config_dir().join("numbering_schemes.json")
}

/// Company settings › Numbering schemes (here the account preferences).
pub fn numbering_schemes() -> NumberingSchemes {
    std::fs::read(numbering_path()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

pub fn set_numbering_schemes(s: &NumberingSchemes) {
    let _ = std::fs::write(numbering_path(), serde_json::to_vec_pretty(s).unwrap_or_default());
}

/// Generates the next number of a sequential scheme (`pick` chooses the
/// scheme and object type) and saves the advanced counter.
pub fn next_number(pick: impl FnOnce(&mut NumberingSchemes) -> Option<String>) -> Option<String> {
    let mut n = numbering_schemes();
    let out = pick(&mut n)?;
    set_numbering_schemes(&n);
    Some(out)
}

/// The document's release log (with the settings' revision scheme and
/// part number uniqueness).
pub fn release_log(doc: &DocModel) -> ReleaseLog {
    let mut log: ReleaseLog = doc.session.store.get_meta(META_KEY).ok().flatten().map(|b| ReleaseLog::from_json(&b)).unwrap_or_default();
    log.scheme = release_settings().scheme;
    log.drawing_reuse = numbering_schemes().drawing_reuse;
    log
}

fn save_log(doc: &mut DocModel, log: &ReleaseLog) -> anyhow::Result<()> {
    doc.session.store.set_meta(META_KEY, Some(&log.to_json()))?;
    Ok(())
}

/// Release items for parts of a Part Studio (their names and part numbers).
/// `config`: the configuration of configured parts (empty: none); the
/// name then carries it ("Part 1 (Long)").
pub fn part_items(doc: &DocModel, tab: TabId, parts: &[BodyId], names: &[String], config: &noshape_model::configuration::ConfigValues) -> Vec<ReleaseItem> {
    let log = release_log(doc);
    let studio = doc.session.doc.tab(tab).and_then(|t| match &t.content {
        noshape_model::document::TabContent::PartStudio(ps) => Some(ps),
        _ => None,
    });
    let props = studio.map(|ps| ps.part_properties.clone());
    // Inputs excluded from properties are released at their defaults.
    let config = &studio.map(|ps| ps.configuration.for_properties(config)).unwrap_or_else(|| config.clone());
    let desc = studio.map(|ps| ps.configuration.describe(config)).unwrap_or_default();
    parts
        .iter()
        .zip(names)
        .map(|(b, name)| {
            let p = props.as_ref().and_then(|p| p.get(b));
            let pn = p.and_then(|p| p.part_number.clone()).unwrap_or_default();
            // Release__typical_release_workflow: a part imported with a
            // revision keeps it in its first release.
            let nrm = p.is_some_and(|p| p.meta.not_revision_managed);
            let imported = p.and_then(|p| p.revision.clone()).filter(|r| !r.trim().is_empty() && log.revisions(&pn).is_empty());
            let name = if desc.is_empty() { name.clone() } else { format!("{name} ({desc})") };
            ReleaseItem { tab, part: Some(b.clone()), name, kind: ItemKind::Part, revision: if nrm { String::new() } else { imported.unwrap_or_else(|| log.next_revision(&pn)) }, part_number: pn, not_revision_managed: nrm, reverted: false, config: if desc.is_empty() { Default::default() } else { config.clone() }, document: None }
        })
        .collect()
}

/// Part number of a tab item: an Assembly's Properties, otherwise the
/// tab's metadata (store meta "tab_part_number:<id>").
pub fn tab_part_number(doc: &DocModel, tab: TabId) -> String {
    match doc.session.doc.tab(tab).map(|t| &t.content) {
        Some(noshape_model::document::TabContent::Assembly(a)) => a.properties.part_number.clone().unwrap_or_default(),
        _ => doc.session.store.get_meta(&format!("tab_part_number:{}", tab.0)).ok().flatten().map(|b| String::from_utf8_lossy(&b).to_string()).unwrap_or_default(),
    }
}

/// The parts of a Part Studio tab (regenerated): ids and names.
fn studio_parts(doc: &DocModel, studio: TabId, config: &noshape_model::configuration::ConfigValues) -> (Vec<BodyId>, Vec<String>) {
    use noshape_model::regen::derived::DerivedProvider as _;
    let provider = doc.derived_provider();
    let ctx = if config.is_empty() { provider.context(&studio.to_string()) } else { provider.context_in(&studio.to_string(), config) };
    let Some(ctx) = ctx else { return (Vec::new(), Vec::new()) };
    ctx.bodies.iter().filter(|b| b.kind == noshape_model::display::BodyKind::Solid).map(|b| (b.id.clone(), b.name.clone())).unzip()
}

/// Release items for a tab (tab menu Release…; Release__typical_release_workflow):
/// a Part Studio's parts; an Assembly and everything in its Bill of
/// Materials (parts and subassemblies); a Drawing and the parts or
/// assemblies its views show; a Variable Studio or file itself.
pub fn tab_items(doc: &DocModel, tab: TabId) -> Vec<ReleaseItem> {
    use noshape_model::document::TabContent;
    let log = release_log(doc);
    let mut out: Vec<ReleaseItem> = Vec::new();
    fn push(out: &mut Vec<ReleaseItem>, items: Vec<ReleaseItem>) {
        for it in items {
            if !out.iter().any(|o| o.same_object(&it)) {
                out.push(it);
            }
        }
    }
    let tab_item = |t: TabId, kind: ItemKind| {
        let name = doc.session.doc.tab(t).map(|x| x.name.clone()).unwrap_or_default();
        let pn = tab_part_number(doc, t);
        ReleaseItem { tab: t, part: None, name, kind, revision: log.next_revision(&pn), part_number: pn, not_revision_managed: false, reverted: false, config: Default::default(), document: None }
    };
    let all_parts = |studio: TabId, config: &noshape_model::configuration::ConfigValues| {
        let (ids, names) = studio_parts(doc, studio, config);
        part_items(doc, studio, &ids, &names, config)
    };
    let one_part = |studio: TabId, b: &BodyId, config: &noshape_model::configuration::ConfigValues| {
        let (ids, names) = studio_parts(doc, studio, config);
        let name = ids.iter().position(|x| x == b).map(|i| names[i].clone()).unwrap_or_else(|| "Part".into());
        part_items(doc, studio, std::slice::from_ref(b), &[name], config)
    };
    // Assemblies recurse into their subassemblies (guarded against cycles).
    fn assembly(doc: &DocModel, a: TabId, seen: &mut Vec<TabId>, out: &mut Vec<ReleaseItem>, tab_item: &dyn Fn(TabId, ItemKind) -> ReleaseItem, all_parts: &dyn Fn(TabId, &noshape_model::configuration::ConfigValues) -> Vec<ReleaseItem>, one_part: &dyn Fn(TabId, &BodyId, &noshape_model::configuration::ConfigValues) -> Vec<ReleaseItem>) {
        use noshape_model::assembly::InstanceSource;
        if seen.contains(&a) {
            return;
        }
        seen.push(a);
        push(out, vec![tab_item(a, ItemKind::Assembly)]);
        let Some(noshape_model::document::TabContent::Assembly(asm)) = doc.session.doc.tab(a).map(|t| &t.content) else { return };
        for inst in &asm.instances {
            match &inst.source {
                // Only the configurations the assembly uses (Help: Releasing a configuration).
                InstanceSource::Part { studio, body, config } => push(out, one_part(*studio, body, config)),
                InstanceSource::PartStudio { studio, config } => push(out, all_parts(*studio, config)),
                InstanceSource::Assembly { assembly: sub, .. } => assembly(doc, *sub, seen, out, tab_item, all_parts, one_part),
                _ => {}
            }
        }
    }
    match doc.session.doc.tab(tab).map(|t| &t.content) {
        Some(TabContent::PartStudio(_)) => push(&mut out, all_parts(tab, &Default::default())),
        Some(TabContent::Assembly(_)) => assembly(doc, tab, &mut Vec::new(), &mut out, &tab_item, &all_parts, &one_part),
        Some(TabContent::Drawing(d)) => {
            push(&mut out, vec![tab_item(tab, ItemKind::Drawing)]);
            for sheet in &d.sheets {
                for v in &sheet.views {
                    let noshape_model::drawing::ViewKind::Base { source, part, config, .. } = &v.kind else { continue };
                    match (doc.session.doc.tab(*source).map(|t| &t.content), part) {
                        (Some(TabContent::Assembly(_)), _) => push(&mut out, vec![tab_item(*source, ItemKind::Assembly)]),
                        (Some(TabContent::PartStudio(_)), Some(b)) => push(&mut out, one_part(*source, b, config)),
                        (Some(TabContent::PartStudio(_)), None) => push(&mut out, all_parts(*source, config)),
                        _ => {}
                    }
                }
            }
        }
        Some(TabContent::VariableStudio(_)) => push(&mut out, vec![tab_item(tab, ItemKind::VariableStudio)]),
        Some(TabContent::File(_)) => push(&mut out, vec![tab_item(tab, ItemKind::File)]),
        _ => {}
    }
    out
}

/// Drawings of the document showing any of `items` (Include associated
/// drawings): a drawing whose base views show the item's part, Part Studio
/// or Assembly.
fn associated_drawings(doc: &DocModel, items: &[ReleaseItem]) -> Vec<ReleaseItem> {
    use noshape_model::document::TabContent;
    let mut out = Vec::new();
    for t in doc.session.doc.ordered_tabs() {
        let TabContent::Drawing(d) = &t.content else { continue };
        let shows = d.sheets.iter().flat_map(|s| s.views.iter()).any(|v| match &v.kind {
            noshape_model::drawing::ViewKind::Base { source, part, .. } => items.iter().any(|it| it.tab == *source && (part.is_none() || it.part.is_none() || it.part == *part)),
            _ => false,
        });
        if shows {
            out.extend(tab_items(doc, t.id).into_iter().filter(|i| i.kind == ItemKind::Drawing));
        }
    }
    out
}

/// "Add Items to Release" (help add-item-to-release-01.png): the current
/// document's Part Studios (with their parts), Assemblies, Files, Drawings
/// and Variable Studios by type, a search box, the picked items at the
/// right with a red ✕, [Add] and [Cancel].
pub struct AddItemsDialog {
    doc: Entity<DocModel>,
    kind: usize,
    search: Entity<InputState>,
    expanded: Vec<TabId>,
    picked: Vec<ReleaseItem>,
    /// Configured Part Studios (Help: Releasing a configuration): the inputs
    /// as edited, and the configuration last generated (its parts are listed).
    pending: std::collections::HashMap<TabId, noshape_model::configuration::ConfigValues>,
    configs: std::collections::HashMap<TabId, noshape_model::configuration::ConfigValues>,
    text_inputs: std::collections::HashMap<(TabId, String), Entity<InputState>>,
    config_menu: Option<TabId>,
    /// "Add Items to Obsolete candidate": released, not obsolete revisions.
    obsolete: bool,
    on_add: AddFn,
    /// Row pictures (release_thumbs) and the ones asked for.
    thumbs: std::collections::HashMap<String, std::sync::Arc<RenderImage>>,
    requested: std::collections::HashSet<String>,
    /// The Other documents tab is active.
    other: bool,
    /// Items come from a version (of this or another document).
    source: Option<add_source::Source>,
    browse: add_source::Browse,
    error: Option<String>,
    _sub: Subscription,
}

/// What [Add] does with the picked items.
type AddFn = std::rc::Rc<dyn Fn(Vec<ReleaseItem>, &mut Window, &mut App)>;

const ADD_KINDS: [(&str, &str); 5] = [("Part Studios", "Search Part Studios"), ("Assemblies", "Search Assemblies"), ("Files", "Search Files"), ("Drawings", "Search Drawings"), ("Variable Studios", "Search Variable Studios")];

pub fn open_add_items(doc: Entity<DocModel>, on_add: impl Fn(Vec<ReleaseItem>, &mut Window, &mut App) + 'static, window: &mut Window, cx: &mut App) {
    open_add_dialog(doc, false, on_add, window, cx);
}

/// "Add Items to Obsolete candidate" (Help: Viewing revision history and
/// obsoleting parts, step 5): the released revisions that can be obsoleted.
pub fn open_add_obsolete_items(doc: Entity<DocModel>, on_add: impl Fn(Vec<ReleaseItem>, &mut Window, &mut App) + 'static, window: &mut Window, cx: &mut App) {
    open_add_dialog(doc, true, on_add, window, cx);
}

fn open_add_dialog(doc: Entity<DocModel>, obsolete: bool, on_add: impl Fn(Vec<ReleaseItem>, &mut Window, &mut App) + 'static, window: &mut Window, cx: &mut App) {
    let view = cx.new(|cx| {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder(ADD_KINDS[0].1));
        let sub = cx.subscribe(&search, |_: &mut AddItemsDialog, _, ev: &InputEvent, cx| {
            if matches!(ev, InputEvent::Change) {
                cx.notify();
            }
        });
        let url = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).placeholder("Search or paste URL"));
        let browse = add_source::Browse { filter: None, query: None, docs: None, url };
        AddItemsDialog { doc, kind: 0, search, expanded: Vec::new(), picked: Vec::new(), pending: Default::default(), configs: Default::default(), text_inputs: Default::default(), config_menu: None, obsolete, on_add: std::rc::Rc::new(on_add), thumbs: Default::default(), requested: Default::default(), other: false, source: None, browse, error: None, _sub: sub }
    });
    window.open_dialog(cx, move |dialog, _, _| release_modal(dialog).w(px(860.)).child(view.clone()));
}

impl AddItemsDialog {
    fn toggle(&mut self, item: ReleaseItem, cx: &mut Context<Self>) {
        match self.picked.iter().position(|p| p.same_object(&item)) {
            Some(i) => {
                self.picked.remove(i);
            }
            None => self.picked.push(item),
        }
        cx.notify();
    }
}

impl AddItemsDialog {
    /// A configured Part Studio's inputs, the "Set to last viewed
    /// configuration ▾" menu (with Reset to default) and [Generate].
    fn render_config_inputs(&mut self, tab: TabId, cfg: &noshape_model::configuration::Configuration, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        use noshape_model::configuration::ConfigInputKind;
        let pal = p();
        let values = self.pending.entry(tab).or_insert_with(|| cfg.defaults()).clone();
        let mut col = v_flex().pl(px(48.)).pr(px(8.)).py(px(4.)).gap(px(4.));
        let menu_open = self.config_menu == Some(tab);
        col = col.child(
            h_flex()
                .gap_2()
                .child(div().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child("Configuration"))
                .child(
                    div()
                        .id(SharedString::from(format!("cfg-menu-{}", tab.0)))
                        .px(px(4.))
                        .cursor_pointer()
                        .child(crate::assets::icon("configuration", 16., cx))
                        .tooltip(crate::ui::tooltip("Set to last viewed configuration"))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.config_menu = if this.config_menu == Some(tab) { None } else { Some(tab) };
                            cx.notify();
                        })),
                ),
        );
        if menu_open {
            let item = |id: &'static str, label: &'static str| div().id(id).px(px(10.)).py(px(4.)).text_size(px(13.)).cursor_pointer().hover(|d| d.bg(pal.row_hover)).child(label);
            let defaults = cfg.defaults();
            col = col.child(
                v_flex()
                    .border_1()
                    .border_color(gpui_kit::rgb(0xcccccc))
                    .bg(gpui_kit::rgb(0xffffff))
                    .child(item("cfg-last", "Set to last viewed configuration").on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if let Some(v) = last_viewed_config(tab) {
                            this.pending.insert(tab, v);
                            this.text_inputs.retain(|(t, _), _| *t != tab);
                        }
                        this.config_menu = None;
                        cx.notify();
                    })))
                    .child(item("cfg-reset", "Reset to default").on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.pending.insert(tab, defaults.clone());
                        this.text_inputs.retain(|(t, _), _| *t != tab);
                        this.config_menu = None;
                        cx.notify();
                    }))),
            );
        }
        // Hidden inputs (visibility conditions) aren't offered.
        let shown = cfg.visibility(&values).0;
        for (input, vis) in cfg.inputs.iter().zip(&shown) {
            if !vis.visible {
                continue;
            }
            let id = input.id.clone();
            let current = values.0.get(&id).cloned().unwrap_or_default();
            let control: AnyElement = match &input.kind {
                ConfigInputKind::List { options, .. } => h_flex()
                    .gap_1()
                    .flex_wrap()
                    .children(options.iter().map(|o| {
                        let (oid, on) = (o.id.clone(), o.id == current);
                        let iid = id.clone();
                        div()
                            .id(SharedString::from(format!("cfg-{}-{}-{}", tab.0, id, o.id)))
                            .px(px(6.))
                            .py(px(2.))
                            .rounded(px(3.))
                            .border_1()
                            .border_color(if on { pal.accent } else { gpui_kit::rgb(0xcccccc) })
                            .when(on, |d| d.bg(pal.row_hover))
                            .text_size(px(12.))
                            .cursor_pointer()
                            .child(o.name.clone())
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.pending.entry(tab).or_default().0.insert(iid.clone(), oid.clone());
                                cx.notify();
                            }))
                    }))
                    .into_any_element(),
                ConfigInputKind::Checkbox { .. } => {
                    let on = current == "true";
                    let iid = id.clone();
                    div()
                        .id(SharedString::from(format!("cfg-{}-{}", tab.0, id)))
                        .size(px(14.))
                        .rounded(px(3.))
                        .border_1()
                        .border_color(if on { pal.accent } else { gpui_kit::rgb(0xcccccc) })
                        .when(on, |d| d.bg(pal.accent))
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.pending.entry(tab).or_default().0.insert(iid.clone(), (!on).to_string());
                            cx.notify();
                        }))
                        .into_any_element()
                }
                _ => {
                    let key = (tab, id.clone());
                    let st = match self.text_inputs.get(&key) {
                        Some(st) => st.clone(),
                        None => {
                            let st = cx.new(|cx| InputState::new(window, cx).default_value(current.clone()));
                            let iid = id.clone();
                            cx.subscribe(&st, move |this: &mut Self, st, ev: &InputEvent, cx| {
                                if matches!(ev, InputEvent::Change) {
                                    let v = st.read(cx).value().to_string();
                                    this.pending.entry(tab).or_default().0.insert(iid.clone(), v);
                                }
                            })
                            .detach();
                            self.text_inputs.insert(key, st.clone());
                            st
                        }
                    };
                    gpui_kit::base::input::input_frame(div().w(px(140.)).border_b_1().border_color(gpui_kit::rgb(0xd7d7d7)).child(Input::new(&st).appearance(false).xsmall()).into_any_element()).into_any_element()
                }
            };
            col = col.child(h_flex().gap_2().child(div().w(px(110.)).text_size(px(12.)).text_color(pal.text_muted).truncate().child(input.name.clone())).child(control));
        }
        col.child(
            h_flex().child(os::secondary_button(SharedString::from(format!("cfg-generate-{}", tab.0)), "Generate", cx).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                let v = this.pending.get(&tab).cloned().unwrap_or_default();
                this.configs.insert(tab, v);
                cx.notify();
            }))),
        )
        .into_any_element()
    }
}

/// Items grouped by the document their pictures are rendered from: the
/// workspace, or the version a linked item comes from.
fn thumb_groups(doc: &DocModel, items: Vec<ReleaseItem>) -> Vec<(noshape_model::Document, Vec<ReleaseItem>)> {
    let mut groups: Vec<(Option<noshape_model::release::LinkedDocument>, Vec<ReleaseItem>)> = Vec::new();
    for it in items {
        match groups.iter_mut().find(|(d, _)| *d == it.document) {
            Some((_, v)) => v.push(it),
            None => groups.push((it.document.clone(), vec![it])),
        }
    }
    groups
        .into_iter()
        .filter_map(|(d, items)| {
            let snapshot = match d {
                None => doc.session.doc.clone(),
                Some(_) => (*add_source::linked_snapshot(doc, &items[0])?).clone(),
            };
            Some((snapshot, items))
        })
        .collect()
}

/// A row of the Add Items list, collected before building elements: a
/// Part Studio with its parts (and configuration), or a tab item.
enum Entry {
    Studio(TabId, String, Vec<ReleaseItem>, Option<noshape_model::configuration::Configuration>),
    Tab(ReleaseItem, &'static str),
}

#[path = "release_add_source.rs"]
mod add_source;
#[path = "release_workflow.rs"]
mod workflow;

impl Render for AddItemsDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        use noshape_model::document::TabContent;
        let pal = p();
        let blue = gpui_kit::rgb(0xb5dcf5);
        let query = self.search.read(cx).value().trim().to_lowercase();
        let matches = |name: &str| query.is_empty() || name.to_lowercase().contains(&query);
        let (mut entries, doc_name, branch) = {
            let doc = self.doc.read(cx);
            let log = release_log(doc);
            let mut entries = Vec::new();
            if self.obsolete {
                // The latest active revision of each released item of the type.
                let want = match self.kind {
                    0 => ItemKind::Part,
                    1 => ItemKind::Assembly,
                    2 => ItemKind::File,
                    3 => ItemKind::Drawing,
                    _ => ItemKind::VariableStudio,
                };
                let mut seen: Vec<(String, noshape_model::configuration::ConfigValues)> = Vec::new();
                for pkg in log.packages.iter().rev().filter(|p| p.kind == PackageKind::Release) {
                    for it in pkg.items.iter().filter(|i| i.kind == want && !i.reverted && !i.not_revision_managed) {
                        let key = (it.part_number.clone(), it.config.clone());
                        if seen.contains(&key) || !matches(&it.name) {
                            continue;
                        }
                        seen.push(key);
                        let active = log.revisions(&it.part_number).last().is_some_and(|e| e.revision == it.revision && e.obsoleted_by.is_none());
                        if active {
                            let icon = match want {
                                ItemKind::Part => "part",
                                ItemKind::Assembly => "assembly",
                                ItemKind::Drawing => "drawing",
                                ItemKind::VariableStudio => "variable-studio",
                                ItemKind::File => "file",
                            };
                            entries.push(Entry::Tab(it.clone(), icon));
                        }
                    }
                }
            }
            for t in doc.session.doc.ordered_tabs().into_iter().filter(|_| !self.obsolete) {
                let tab = t.id;
                match (&t.content, self.kind) {
                    (TabContent::PartStudio(_), 0) => {
                        let config = self.configs.get(&tab).cloned().unwrap_or_default();
                        let (ids, names) = studio_parts(doc, tab, &config);
                        let parts: Vec<ReleaseItem> = part_items(doc, tab, &ids, &names, &config).into_iter().filter(|p| matches(&p.name) || matches(&t.name)).collect();
                        let TabContent::PartStudio(ps) = &t.content else { continue };
                        let cfg = (!ps.configuration.inputs.is_empty()).then(|| ps.configuration.clone());
                        if !parts.is_empty() || cfg.is_some() {
                            entries.push(Entry::Studio(tab, t.name.clone(), parts, cfg));
                        }
                    }
                    (TabContent::Assembly(_), 1) | (TabContent::File(_), 2) | (TabContent::Drawing(_), 3) | (TabContent::VariableStudio(_), 4) if matches(&t.name) => {
                        let (kind, icon) = match &t.content {
                            TabContent::Assembly(_) => (ItemKind::Assembly, "assembly"),
                            TabContent::Drawing(_) => (ItemKind::Drawing, "drawing"),
                            TabContent::VariableStudio(_) => (ItemKind::VariableStudio, "variable-studio"),
                            _ => (ItemKind::File, "file"),
                        };
                        let pn = tab_part_number(doc, tab);
                        entries.push(Entry::Tab(ReleaseItem { tab, part: None, name: t.name.clone(), kind, revision: log.next_revision(&pn), part_number: pn, not_revision_managed: false, reverted: false, config: Default::default(), document: None }, icon));
                    }
                    _ => {}
                }
            }
            let branch = doc.session.store.branch(doc.session.branch).map(|b| b.name).unwrap_or_else(|_| "Main".into());
            (entries, doc.name(), branch)
        };
        // A version (Version graph, Other documents) lists its own items.
        if self.source.is_some() || self.other {
            entries = self.source_entries(self.kind, &matches, cx);
        }
        // A version without items of the type (live assembly/612: "No Part
        // Studios available.").
        let none_available = (self.source.is_some() && entries.is_empty()).then(|| {
            div().pl(px(10.)).h(px(28.)).flex().items_center().text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).text_color(p().text).child(format!("No {} available.", ADD_KINDS[self.kind].0)).into_any_element()
        });
        let is_picked = |picked: &[ReleaseItem], it: &ReleaseItem| picked.iter().any(|p| p.same_object(it));
        // Pictures of the listed rows (a Part Studio row shows the whole studio).
        let linked = self.source.as_ref().and_then(|s| s.linked());
        let studio_item = |tab: TabId, config: noshape_model::configuration::ConfigValues| ReleaseItem { tab, part: None, name: String::new(), kind: ItemKind::Part, part_number: String::new(), revision: String::new(), not_revision_managed: false, reverted: false, config, document: linked.clone() };
        let mut wanted: Vec<ReleaseItem> = self.picked.clone();
        for e in &entries {
            match e {
                Entry::Studio(tab, _, parts, _) => {
                    wanted.push(studio_item(*tab, self.configs.get(tab).cloned().unwrap_or_default()));
                    if self.expanded.contains(tab) {
                        wanted.extend(parts.iter().cloned());
                    }
                }
                Entry::Tab(it, _) => wanted.push(it.clone()),
            }
        }
        let todo: Vec<ReleaseItem> = wanted.into_iter().filter(|it| matches!(it.kind, ItemKind::Part | ItemKind::Assembly | ItemKind::Drawing) && self.requested.insert(crate::release_thumbs::key(it))).collect();
        if !todo.is_empty() && cx.has_global::<crate::gpu::GpuGlobal>() {
            let gpu = cx.global::<crate::gpu::GpuGlobal>().0.clone();
            for (doc, items) in thumb_groups(self.doc.read(cx), todo) {
                let versions = self.doc.read(cx).session.pinned_documents();
                let gpu = gpu.clone();
                cx.spawn(async move |this, cx| {
                    let made = cx.background_spawn(crate::release_thumbs::render(doc, versions, items, gpu)).await;
                    let _ = this.update(cx, |this: &mut Self, cx| {
                        this.thumbs.extend(made);
                        cx.notify();
                    });
                })
                .detach();
            }
        }
        // Help image add-item-to-release-01: the picture, or "Generating
        // preview" on gray until it is made; Variable Studios and files show
        // their icon.
        let thumbs = self.thumbs.clone();
        let picture = |it: &ReleaseItem, w: f32, icon: &'static str, cx: &mut Context<Self>| -> AnyElement {
            let slot = div().w(px(w)).h(px(40.)).flex_none().flex().items_center().justify_center();
            match thumbs.get(&crate::release_thumbs::key(it)) {
                Some(im) => slot.child(img(ImageSource::Render(im.clone())).w(px(w)).h(px(40.)).object_fit(ObjectFit::Contain)).into_any_element(),
                None if matches!(it.kind, ItemKind::Part | ItemKind::Assembly | ItemKind::Drawing) => slot
                    .bg(gpui_kit::rgb(0xeeeeee))
                    .child(div().text_size(px(10.)).line_height(px(11.)).text_center().text_color(gpui_kit::rgb(0x888888)).child("Generating preview"))
                    .into_any_element(),
                None => slot.child(crate::assets::icon(icon, 32., cx)).into_any_element(),
            }
        };
        let mut tree = v_flex();
        for e in entries {
            match e {
                Entry::Studio(tab, name, parts, cfg) => {
                    // A Part Studio row expands to its parts; parts are picked.
                    let open = self.expanded.contains(&tab);
                    tree = tree.child(
                        h_flex()
                            .id(SharedString::from(format!("add-ps-{}", tab.0)))
                            .h(px(42.))
                            .pl(px(4.))
                            .gap(px(14.))
                            .cursor_pointer()
                            .child(div().w(px(16.)).flex().justify_center().child(crate::ui::glyph(if open { "ui-chevron-down" } else { "ui-chevron-right" }, 11., pal.text)))
                            .child(div().w(px(70.)).flex().justify_center().child(picture(&studio_item(tab, self.configs.get(&tab).cloned().unwrap_or_default()), 40., "part-studio", cx)))
                            .child(div().text_size(px(14.)).text_color(pal.text).child(name))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                if let Some(i) = this.expanded.iter().position(|x| *x == tab) {
                                    this.expanded.remove(i);
                                } else {
                                    this.expanded.push(tab);
                                    this.load_source_parts(tab, cx);
                                }
                                cx.notify();
                            })),
                    );
                    if open && let Some(cfg) = &cfg {
                        tree = tree.child(self.render_config_inputs(tab, cfg, window, cx));
                    }
                    if open {
                        for (k, it) in parts.into_iter().enumerate() {
                            let on = is_picked(&self.picked, &it);
                            tree = tree.child(
                                h_flex()
                                    .id(SharedString::from(format!("add-part-{}-{k}", tab.0)))
                                    .h(px(42.))
                                    .pl(px(34.))
                                    .gap(px(4.))
                                    .cursor_pointer()
                                    .when(on, |d| d.bg(blue))
                                    .when(!on, |d| d.hover(|d| d.bg(pal.row_hover)))
                                    .child(picture(&it, 70., "part", cx))
                                    .child(div().text_size(px(14.)).text_color(pal.text).child(it.name.clone()))
                                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.toggle(it.clone(), cx))),
                            );
                        }
                    }
                }
                Entry::Tab(it, icon) => {
                    let on = is_picked(&self.picked, &it);
                    let tab = it.tab;
                    tree = tree.child(
                        h_flex()
                            .id(SharedString::from(format!("add-tab-{}", tab.0)))
                            .h(px(42.))
                            .pl(px(34.))
                            .gap(px(4.))
                            .cursor_pointer()
                            .when(on, |d| d.bg(blue))
                            .when(!on, |d| d.hover(|d| d.bg(pal.row_hover)))
                            .child(picture(&it, 70., icon, cx))
                            .child(div().text_size(px(14.)).text_color(pal.text).child(it.name.clone()))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.toggle(it.clone(), cx))),
                    );
                }
            }
        }
        let picked = self.picked.iter().enumerate().map(|(k, it)| {
            let it2 = it.clone();
            let icon = match it.kind {
                ItemKind::Part => "part",
                ItemKind::Assembly => "assembly",
                ItemKind::Drawing => "drawing",
                ItemKind::VariableStudio => "variable-studio",
                ItemKind::File => "file",
            };
            h_flex()
                .id(SharedString::from(format!("add-picked-{k}")))
                .w(px(300.))
                .h(px(42.))
                .pr(px(12.))
                .gap(px(4.))
                .bg(blue)
                .child(picture(it, 70., icon, cx))
                .child(div().flex_1().text_size(px(14.)).text_color(pal.text).child(it.name.clone()))
                .child(div().id(SharedString::from(format!("add-x-{k}"))).cursor_pointer().child(Icon::new(IconName::Close).with_size(px(18.)).text_color(gpui_kit::rgb(0xbd3039))).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.toggle(it2.clone(), cx))))
        }).collect::<Vec<_>>();
        let tab_button = |i: usize, label: &'static str, cx: &mut Context<Self>| {
            let on = self.kind == i;
            div()
                .id(SharedString::from(format!("add-kind-{i}")))
                .flex_1()
                .py(px(6.))
                .text_size(px(13.))
                .text_center()
                .cursor_pointer()
                .when(on, |d| d.text_color(pal.accent).border_b_2().border_color(pal.accent))
                .when(!on, |d| d.text_color(pal.text))
                .child(label)
                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                    this.kind = i;
                    this.search.update(cx, |s, cx| {
                        s.set_value("", window, cx);
                        s.set_placeholder(ADD_KINDS[i].1, window, cx);
                    });
                    cx.notify();
                }))
        };
        let _ = window;
        // Bundle (release dialog getTitle): "Add Items to Release" /
        // "Add Items to Obsoletion".
        let title = if self.obsolete { "Add Items to Obsoletion" } else { "Add Items to Release" };
        // The select-item widget: Current document | Other documents; the
        // document's title row; the Version graph or the type tabs, search
        // and tree; Other documents first browse the documents.
        let doc_tab = |id: &'static str, label: &'static str, on: bool, other: bool, cx: &mut Context<Self>| {
            div()
                .id(id)
                .flex_1()
                .py(px(6.))
                .text_size(px(13.))
                .text_center()
                .cursor_pointer()
                .when(on, |d| d.text_color(pal.accent).font_weight(FontWeight::MEDIUM).border_b_2().border_color(pal.accent))
                .when(!on, |d| d.text_color(pal.text).border_b_1().border_color(gpui_kit::rgb(0xdddddd)))
                .child(label)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if this.other != other {
                        this.other = other;
                        this.source = None;
                        this.expanded.clear();
                        this.error = None;
                    }
                    cx.notify();
                }))
        };
        let kind_tabs = h_flex().border_b_1().border_color(gpui_kit::rgb(0xdddddd)).children(ADD_KINDS.iter().enumerate().map(|(i, (l, _))| tab_button(i, l, cx)).collect::<Vec<_>>());
        let mut left = v_flex().w(px(404.)).h_full().border_r_1().border_color(gpui_kit::rgb(0xdddddd));
        if !self.obsolete {
            left = left.child(h_flex().child(doc_tab("add-current-doc", "Current document", !self.other, false, cx)).child(doc_tab("add-other-docs", "Other documents", self.other, true, cx)));
        } else {
            left = left.child(h_flex().child(div().w(px(200.)).py(px(6.)).text_size(px(13.)).text_center().text_color(pal.accent).font_weight(FontWeight::MEDIUM).border_b_2().border_color(pal.accent).child("Current document")));
        }
        if self.other && self.source.is_none() {
            left = left.child(div().id("add-browser").flex_1().overflow_y_scroll().child(self.render_browser(cx)));
        } else {
            left = left.child(self.render_title_row(doc_name, branch, cx));
            match self.render_graph(cx) {
                Some(graph) => left = left.child(div().id("add-graph").flex_1().overflow_y_scroll().child(graph)),
                None => {
                    left = left
                        .child(kind_tabs)
                        .child(div().p(px(6.)).child(gpui_kit::base::input::input_frame(div().border_1().rounded(px(3.)).border_color(gpui_kit::rgb(0xcccccc)).child(Input::new(&self.search).appearance(false).small()))))
                        .children(self.render_changes(cx))
                        .children(none_available)
                        .child(div().id("add-tree").flex_1().overflow_y_scroll().child(tree));
                }
            }
        }
        v_flex()
            .w_full()
            .child(h_flex().h(px(56.)).px(px(10.)).child(div().text_size(px(20.)).font_weight(FontWeight::MEDIUM).text_color(pal.text).child(title)))
            .child(
                h_flex()
                    .h(px(416.))
                    .border_t_1()
                    .border_b_1()
                    .border_color(gpui_kit::rgb(0xdddddd))
                    .child(left)
                    .child(v_flex().id("add-picked").flex_1().h_full().overflow_y_scroll().children(picked)),
            )
            .child(
                h_flex()
                    .h(px(58.))
                    .px(px(12.))
                    .gap_2()
                    .justify_end()
                    .child(os::primary_button("add-ok", "Add").tooltip(if self.obsolete { "Add selected items to Obsoletion" } else { "Add selected items to Release" }).on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        let items = std::mem::take(&mut this.picked);
                        let f = this.on_add.clone();
                        window.close_dialog(cx);
                        f(items, window, cx);
                    })))
                    .child(os::cancel_button("add-cancel", "Cancel", cx).tooltip(if self.obsolete { "Cancel adding items and resume editing Obsoletion" } else { "Cancel adding items and resume editing Release" }).on_click(|_, window, cx| window.close_dialog(cx))),
            )
    }
}

/// Edit properties of a release item: the metadata Properties modal of the
/// part, the assembly, or the tab (drawings, Variable Studios, files keep
/// name, description and part number in the tab's metadata). `on_saved`
/// gets the saved part number.
fn open_item_properties(doc: Entity<DocModel>, item: ReleaseItem, on_saved: impl Fn(String, &mut Window, &mut App) + 'static, window: &mut Window, cx: &mut App) {
    use crate::properties_dialog::{MetaSpec, MetaValues};
    use noshape_model::document::TabContent;
    let d = doc.read(cx);
    let tab = item.tab;
    let content = d.session.doc.tab(tab).map(|t| &t.content);
    let (mut material, mut computed_mass) = (None, None);
    let (category, values) = match (content, &item.part) {
        (Some(TabContent::PartStudio(ps)), Some(b)) => {
            let p = ps.part_properties.get(b).cloned().unwrap_or_default();
            // As the part's own Properties (parts list): material and mass.
            material = Some(p.material.clone().unwrap_or_else(|| "Unassigned".into()));
            if p.material.is_some() {
                use noshape_model::regen::derived::DerivedProvider as _;
                let provider = d.derived_provider();
                let ctx = if item.config.is_empty() { provider.context(&tab.to_string()) } else { provider.context_in(&tab.to_string(), &item.config) };
                let density = |_: &BodyId| p.density;
                computed_mass = ctx.and_then(|c| noshape_model::regen::measure::mass_properties(&c, std::slice::from_ref(b), &density)?.mass).map(|m| format!("{} kg", noshape_units::to_fixed(m, 4)));
            }
            ("Onshape Part", MetaValues { name: item.name.clone(), description: p.description.unwrap_or_default(), part_number: p.part_number.unwrap_or_default(), revision: p.revision.unwrap_or_default(), meta: p.meta, mass_override: p.mass_override.map(|m| format!("{m} kg")), bom: 0 })
        }
        (Some(TabContent::Assembly(a)), _) => {
            let p = &a.properties;
            ("Onshape Assembly", MetaValues { name: item.name.clone(), description: p.description.clone().unwrap_or_default(), part_number: p.part_number.clone().unwrap_or_default(), revision: p.revision.clone().unwrap_or_default(), meta: p.meta.clone(), mass_override: None, bom: 0 })
        }
        _ => {
            let desc = d.session.store.get_meta(&format!("tab_description:{}", tab.0)).ok().flatten().map(|b| String::from_utf8_lossy(&b).to_string()).unwrap_or_default();
            let category = match item.kind {
                ItemKind::Drawing => "Onshape Drawing",
                ItemKind::VariableStudio => "Onshape Variable Studio",
                _ => "Onshape File",
            };
            (category, MetaValues { name: item.name.clone(), description: desc, part_number: tab_part_number(d, tab), ..MetaValues::default() })
        }
    };
    let spec = MetaSpec { category, material, computed_mass, bom_options: None, values };
    crate::properties_dialog::open_properties(
        spec,
        move |v, window, cx| {
            let item = item.clone();
            doc.update(cx, |d, cx| {
                let content_kind = d.session.doc.tab(item.tab).map(|t| match &t.content {
                    TabContent::PartStudio(_) => 0,
                    TabContent::Assembly(_) => 1,
                    _ => 2,
                });
                match (content_kind, &item.part) {
                    (Some(0), Some(b)) => {
                        let b = b.clone();
                        d.edit_studio(item.tab, &format!("Properties : {}", v.name), cx, |ps| {
                            let p = ps.part_properties.entry(b).or_default();
                            p.name = Some(v.name.clone());
                            p.description = Some(v.description.clone()).filter(|s| !s.is_empty());
                            p.part_number = Some(v.part_number.clone()).filter(|s| !s.is_empty());
                            p.revision = Some(v.revision.clone()).filter(|s| !s.is_empty());
                            p.meta = v.meta.clone();
                            p.mass_override = v.mass_override.as_deref().and_then(crate::properties_dialog::parse_mass);
                        });
                    }
                    (Some(1), _) => {
                        d.edit_tab(item.tab, &format!("Properties : {}", v.name), cx, |c| {
                            if let TabContent::Assembly(a) = c {
                                a.properties.description = Some(v.description.clone()).filter(|s| !s.is_empty());
                                a.properties.part_number = Some(v.part_number.clone()).filter(|s| !s.is_empty());
                                a.properties.revision = Some(v.revision.clone()).filter(|s| !s.is_empty());
                                a.properties.meta = v.meta.clone();
                            }
                        });
                    }
                    _ => {
                        let _ = d.session.store.set_meta(&format!("tab_description:{}", item.tab.0), Some(v.description.as_bytes()));
                        let _ = d.session.store.set_meta(&format!("tab_part_number:{}", item.tab.0), Some(v.part_number.as_bytes()));
                    }
                }
            });
            on_saved(v.part_number.clone(), window, cx);
        },
        window,
        cx,
    );
}

/// How a previously released item compares with its latest revision (Help:
/// "Reverting items to their last revision").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SinceRelease {
    /// Neither geometry nor metadata changed.
    Unchanged,
    /// Same geometry, other metadata.
    MetadataChanged,
}

/// Compare an item with the document version of its latest revision:
/// geometry = the Part Studio's features, the Assembly's instances (not
/// their positions), the drawing; metadata = the item's properties.
/// None when it was never released or its geometry changed.
fn since_release(doc: &DocModel, log: &ReleaseLog, item: &ReleaseItem) -> Option<SinceRelease> {
    use noshape_model::document::TabContent;
    // A version never changes.
    if item.document.is_some() {
        return None;
    }
    let rev = log.latest_revision(&item.part_number)?;
    let version = log.release_version_of(&item.part_number, &rev)?;
    let mv = doc.session.store.list_versions().ok()?.into_iter().find(|v| v.id.to_string() == version)?.microversion;
    // Both states loaded the same way (loading upgrades saved parameters,
    // so the in-memory document isn't comparable to a stored one).
    let head = doc.session.head().ok()?;
    if head == mv {
        return Some(SinceRelease::Unchanged);
    }
    let load = |m| noshape_model::Document::from_blobs(&doc.session.store.read_all(m).ok()?).ok();
    let (current, old) = (load(head)?, load(mv)?);
    let (now, then) = (current.tab(item.tab)?, old.tab(item.tab)?);
    let json = |v: &dyn erased::Ser| v.to_json();
    let (geometry_same, meta_same) = match (&now.content, &then.content) {
        (TabContent::PartStudio(a), TabContent::PartStudio(b)) => {
            let b_id = item.part.as_ref()?;
            (json(&a.features) == json(&b.features), json(&a.part_properties.get(b_id)) == json(&b.part_properties.get(b_id)))
        }
        (TabContent::Assembly(a), TabContent::Assembly(b)) => {
            // Repositioning is no change: compare the instances without placements.
            let shape = |x: &noshape_model::assembly::Assembly| x.instances.iter().map(|i| (i.id, json(&i.source), i.suppressed)).collect::<Vec<_>>();
            (shape(a) == shape(b), json(&a.properties) == json(&b.properties))
        }
        (TabContent::Drawing(a), TabContent::Drawing(b)) => (json(a) == json(b), now.name == then.name),
        (a, b) => (json(a) == json(b), now.name == then.name),
    };
    match (geometry_same, meta_same) {
        (true, true) => Some(SinceRelease::Unchanged),
        (true, false) => Some(SinceRelease::MetadataChanged),
        _ => None,
    }
}

/// serde values compared as JSON.
mod erased {
    pub trait Ser {
        fn to_json(&self) -> serde_json::Value;
    }
    impl<T: serde::Serialize> Ser for T {
        fn to_json(&self) -> serde_json::Value {
            serde_json::to_value(self).unwrap_or_default()
        }
    }
}

/// A package's items as they are in the workspace now: current names and
/// part numbers, next revisions.
fn refreshed_items(doc: &DocModel, items: &[ReleaseItem]) -> Vec<ReleaseItem> {
    let log = release_log(doc);
    items
        .iter()
        .map(|it| match &it.part {
            // Versions don't change.
            _ if it.document.is_some() => it.clone(),
            Some(b) => {
                let (ids, names) = studio_parts(doc, it.tab, &it.config);
                let name = ids.iter().position(|x| x == b).map(|i| names[i].clone()).unwrap_or_else(|| it.name.clone());
                part_items(doc, it.tab, std::slice::from_ref(b), &[name], &it.config).pop().unwrap_or_else(|| it.clone())
            }
            None => {
                let pn = tab_part_number(doc, it.tab);
                let pn = if pn.is_empty() { it.part_number.clone() } else { pn };
                let name = doc.session.doc.tab(it.tab).map(|t| t.name.clone()).unwrap_or_else(|| it.name.clone());
                ReleaseItem { name, revision: log.next_revision(&pn), part_number: pn, ..it.clone() }
            }
        })
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    /// Create Release candidate (or a saved draft).
    Release,
    /// Review a recorded package (read-only).
    Review,
    /// Create Obsolete candidate.
    Obsolete,
}

/// A state of the form for undo / redo.
#[derive(Clone, PartialEq)]
struct Snapshot {
    items: Vec<ReleaseItem>,
    name: String,
    notes: String,
}

struct Row {
    item: ReleaseItem,
    part_number: Entity<InputState>,
    revision: Entity<InputState>,
    /// The revision was typed by the user (don't re-suggest it).
    revision_touched: bool,
    /// The part number the item had (a different one is "modified").
    initial_pn: String,
    /// Unchanged (or metadata only) since its latest revision: can revert.
    since: Option<SinceRelease>,
}

pub struct ReleaseDialog {
    doc: Entity<DocModel>,
    mode: Mode,
    /// Applied (saved as a draft): the title reads "Review Release".
    applied: bool,
    rows: Vec<Row>,
    name: Entity<InputState>,
    notes: Entity<TextareaState>,
    re_releasable: bool,
    /// Only the items that need attention.
    problems_only: bool,
    /// Opened from the Release drafts panel: the draft's name (Delete draft).
    draft: Option<String>,
    /// The Save draft ▴ menu is open.
    draft_menu: bool,
    /// Item search and the type filter (help: "Searching and filtering the
    /// object list"): empty = every type.
    search: Entity<InputState>,
    filter_open: bool,
    kinds: Vec<ItemKind>,
    /// Form undo / redo (help: "you can use the Undo and Redo buttons or
    /// your operating system hot keys"): snapshots and the current one.
    history: Vec<Snapshot>,
    at: usize,
    restoring: bool,
    /// The field whose typing the newest undo step holds: one step per
    /// field edit, not per keystroke.
    typing: Option<EntityId>,
    /// Row pictures (release_thumbs), by `release_thumbs::key`.
    thumbs: std::collections::HashMap<String, std::sync::Arc<RenderImage>>,
    thumbs_started: bool,
    /// The recorded package (review, draft): its audit log.
    package: Option<Package>,
    /// When the dialog created its candidate (ms since the epoch).
    opened: i64,
    /// The ⋯ menu is open.
    overflow_open: bool,
    /// ⋯ › View workflow: the diagram lightbox, and what had the focus.
    workflow: Option<workflow::Lightbox>,
    workflow_return: Option<FocusHandle>,
    /// Numbering schemes: sequential part numbers / release names (the #
    /// buttons).
    seq_parts: bool,
    seq_names: bool,
    _subs: Vec<Subscription>,
}

impl ReleaseDialog {
    fn new(doc: Entity<DocModel>, mode: Mode, items: Vec<ReleaseItem>, name: String, notes: String, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut subs = Vec::new();
        let read_only = mode != Mode::Release;
        let sinces: Vec<Option<SinceRelease>> = {
            let d = doc.read(cx);
            let log = release_log(d);
            items.iter().map(|it| if mode == Mode::Release { since_release(d, &log, it) } else { None }).collect()
        };
        let rows = items
            .into_iter()
            .zip(sinces)
            .map(|(item, since)| {
                let part_number = cx.new(|cx| InputState::new(window, cx).default_value(item.part_number.clone()));
                let revision = cx.new(|cx| InputState::new(window, cx).default_value(item.revision.clone()));
                if !read_only {
                    subs.push(cx.subscribe_in(&part_number, window, |this: &mut Self, st, ev: &InputEvent, window, cx| {
                        if matches!(ev, InputEvent::Change) {
                            this.part_number_changed(st, window, cx);
                        }
                    }));
                    subs.push(cx.subscribe_in(&revision, window, |this: &mut Self, st, ev: &InputEvent, _, cx| {
                        if matches!(ev, InputEvent::Change) {
                            if let Some(r) = this.rows.iter_mut().find(|r| r.revision == *st) {
                                r.revision_touched = true;
                            }
                            this.record_typing(st.entity_id(), cx);
                        }
                    }));
                }
                Row { initial_pn: item.part_number.clone(), item, part_number, revision, revision_touched: false, since }
            })
            .collect();
        let name = cx.new(|cx| InputState::new(window, cx).default_value(name));
        let notes = cx.new(|cx| TextareaState::new(window, cx).rows(3).default_value(notes));
        subs.push(cx.subscribe(&notes, |this: &mut Self, st, ev: &InputEvent, cx| {
            if matches!(ev, InputEvent::Change) {
                this.record_typing(st.entity_id(), cx);
            }
        }));
        subs.push(cx.subscribe_in(&name, window, |this: &mut Self, st, ev: &InputEvent, _, cx| {
            if matches!(ev, InputEvent::Change) {
                this.record_typing(st.entity_id(), cx);
            }
        }));
        let numbering = numbering_schemes();
        let search = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).placeholder("Search"));
        subs.push(cx.subscribe_in(&search, window, |_: &mut Self, _, ev: &InputEvent, _, cx| {
            if matches!(ev, InputEvent::Change) {
                cx.notify();
            }
        }));
        Self { doc, mode, applied: false, rows, name, notes, re_releasable: false, problems_only: false, draft: None, draft_menu: false, search, filter_open: false, kinds: Vec::new(), history: Vec::new(), at: 0, restoring: false, typing: None, thumbs: Default::default(), thumbs_started: false, package: None, opened: chrono::Utc::now().timestamp_millis(), overflow_open: false, workflow: None, workflow_return: None, seq_parts: numbering.release_objects.sequential, seq_names: numbering.release_names.sequential, _subs: subs }
    }

    /// A new part number suggests its next revision (unless one was typed).
    fn part_number_changed(&mut self, st: &Entity<InputState>, window: &mut Window, cx: &mut Context<Self>) {
        let log = release_log(self.doc.read(cx));
        if let Some(r) = self.rows.iter_mut().find(|r| r.part_number == *st)
            && !r.revision_touched
        {
            let next = log.next_revision(st.read(cx).value().trim());
            r.revision.update(cx, |s, cx| s.set_value(next, window, cx));
            r.revision_touched = false;
        }
        self.record_typing(st.entity_id(), cx);
    }

    /// The row's # button ("Generate next part number"): the next number of
    /// the Release objects scheme for the item's type.
    fn generate_part_number(&mut self, i: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(row) = self.rows.get(i) else { return };
        let ty = row.item.kind.prefix_index();
        if let Some(n) = next_number(|s| s.release_objects.generate(ty)) {
            let st = row.part_number.clone();
            st.update(cx, |s, cx| s.set_value(n, window, cx));
        }
    }

    /// The header's # button ("Generate missing part numbers"). With
    /// "Drawing can reuse part number…", a drawing of one listed part or
    /// assembly shares its number: whichever of the two has one gives it to
    /// the other, else both get a new one (Help Release__typical_release_workflow).
    fn generate_missing_part_numbers(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let reuse = numbering_schemes().drawing_reuse;
        let pn = |r: &Row, cx: &App| r.part_number.read(cx).value().trim().to_string();
        for i in 0..self.rows.len() {
            if let Some(j) = self.drawing_partner(i, cx).filter(|_| reuse) {
                let (a, b) = (pn(&self.rows[i], cx), pn(&self.rows[j], cx));
                let ty = self.rows[j].item.kind.prefix_index();
                let shared = match (a.is_empty(), b.is_empty()) {
                    (true, true) => next_number(|s| s.release_objects.generate(ty)),
                    (true, false) => Some(b),
                    (false, true) => Some(a),
                    (false, false) => None,
                };
                for k in [i, j] {
                    if let Some(n) = &shared
                        && pn(&self.rows[k], cx).is_empty()
                    {
                        let st = self.rows[k].part_number.clone();
                        st.update(cx, |s, cx| s.set_value(n.clone(), window, cx));
                    }
                }
                continue;
            }
            if pn(&self.rows[i], cx).is_empty() {
                self.generate_part_number(i, window, cx);
            }
        }
    }

    /// The listed part or assembly a drawing row shows, when the drawing
    /// shows exactly one object.
    fn drawing_partner(&self, i: usize, cx: &App) -> Option<usize> {
        let r = self.rows.get(i).filter(|r| r.item.kind == ItemKind::Drawing)?;
        let shown: Vec<ReleaseItem> = tab_items(self.doc.read(cx), r.item.tab).into_iter().filter(|it| it.kind != ItemKind::Drawing).collect();
        let [one] = &shown[..] else { return None };
        self.rows.iter().position(|o| o.item.kind != ItemKind::Drawing && o.item.tab == one.tab && o.item.part == one.part)
    }

    /// The audit trail of the candidate's workflow object (Help: "View audit
    /// log to see an audit trail of the workflow objects and their
    /// significant events, for example, creation and transition"). Types are
    /// BTWorkflowAuditEntryType ordinals.
    fn audit_log(&self, cx: &App) -> serde_json::Value {
        let iso = |ms: i64| chrono::DateTime::from_timestamp_millis(ms).map(|t| t.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)).unwrap_or_default();
        let entry = |ms: i64, ty: u32, name: &str, state: &str, transition: Option<&str>| {
            let mut e = serde_json::json!({ "timestamp": iso(ms), "type": ty, "typeName": name, "state": state });
            if let Some(t) = transition {
                e["transition"] = t.into();
            }
            e
        };
        let obsoletion = self.mode == Mode::Obsolete || self.package.as_ref().is_some_and(|p| p.kind == PackageKind::Obsoletion);
        let (id, created) = self.package.as_ref().map_or((format!("{:x}", self.opened), self.opened), |p| (format!("{:024x}", p.id), p.created));
        let mut entries = vec![entry(created, 0, "OBJECT_CREATED", "SETUP", None)];
        let done = self.package.as_ref().filter(|p| p.version.is_some());
        if let Some(p) = done {
            let (transition, state) = if obsoletion { ("OBSOLETE", "OBSOLETED") } else { ("RELEASE", "RELEASED") };
            entries.push(entry(p.created, 7, "TRANSITION_STARTED", "SETUP", Some(transition)));
            entries.push(entry(p.created, 8, "TRANSITION_COMPLETED", state, Some(transition)));
        }
        let state = if done.is_some() { if obsoletion { "OBSOLETED" } else { "RELEASED" } } else { "SETUP" };
        serde_json::json!({
            "objectId": id,
            "objectType": if obsoletion { "OBSOLETION" } else { "RELEASE" },
            "name": self.name.read(cx).value().to_string(),
            "workflow": { "name": if obsoletion { "Onshape Obsoletion" } else { "Onshape Release" }, "isDebug": false, "state": state },
            "entries": entries,
        })
    }

    /// ⋯ › View audit log: Onshape opens the log in a new browser tab.
    fn view_audit_log(&self, cx: &mut Context<Self>) {
        let json = serde_json::to_vec_pretty(&self.audit_log(cx)).unwrap_or_default();
        let path = std::env::temp_dir().join(format!("noshape-auditlog-{}.json", self.package.as_ref().map_or(self.opened as u64, |p| p.id)));
        if std::fs::write(&path, json).is_ok() {
            cx.open_url(&format!("file://{}", path.display()));
        }
    }

    /// Add items (Add Items to Release, associated drawings), skipping
    /// ones already listed.
    fn add_items(&mut self, items: Vec<ReleaseItem>, window: &mut Window, cx: &mut Context<Self>) {
        let mut added = Vec::new();
        for item in items {
            if self.rows.iter().any(|r| r.item.same_object(&item)) {
                continue;
            }
            added.push(item.clone());
            let part_number = cx.new(|cx| InputState::new(window, cx).default_value(item.part_number.clone()));
            let revision = cx.new(|cx| InputState::new(window, cx).default_value(item.revision.clone()));
            self._subs.push(cx.subscribe_in(&part_number, window, |this: &mut Self, st, ev: &InputEvent, window, cx| {
                if matches!(ev, InputEvent::Change) {
                    this.part_number_changed(st, window, cx);
                }
            }));
            self._subs.push(cx.subscribe_in(&revision, window, |this: &mut Self, st, ev: &InputEvent, _, cx| {
                if matches!(ev, InputEvent::Change) {
                    if let Some(r) = this.rows.iter_mut().find(|r| r.revision == *st) {
                        r.revision_touched = true;
                    }
                    this.record_typing(st.entity_id(), cx);
                }
            }));
            let since = {
                let d = self.doc.read(cx);
                since_release(d, &release_log(d), &item)
            };
            self.rows.push(Row { initial_pn: item.part_number.clone(), item, part_number, revision, revision_touched: false, since });
        }
        if self.thumbs_started {
            self.request_thumbs(added, cx);
        }
        self.record(cx);
    }

    /// Render row pictures in the background.
    fn request_thumbs(&mut self, items: Vec<ReleaseItem>, cx: &mut Context<Self>) {
        if items.is_empty() || !cx.has_global::<crate::gpu::GpuGlobal>() {
            return;
        }
        let gpu = cx.global::<crate::gpu::GpuGlobal>().0.clone();
        for (doc, items) in thumb_groups(self.doc.read(cx), items) {
            let versions = self.doc.read(cx).session.pinned_documents();
            let gpu = gpu.clone();
            cx.spawn(async move |this, cx| {
                let made = cx.background_spawn(crate::release_thumbs::render(doc, versions, items, gpu)).await;
                let _ = this.update(cx, |this: &mut Self, cx| {
                    this.thumbs.extend(made);
                    cx.notify();
                });
            })
            .detach();
        }
    }

    /// Revert a row to its last revision (or back): it is listed with that
    /// revision as Released and not released again.
    fn set_reverted(&mut self, i: usize, on: bool, window: &mut Window, cx: &mut Context<Self>) {
        let log = release_log(self.doc.read(cx));
        let Some(r) = self.rows.get_mut(i) else { return };
        r.item.reverted = on;
        let rev = if on { log.latest_revision(&r.item.part_number).unwrap_or_default() } else { log.next_revision(&r.item.part_number) };
        r.revision.update(cx, |s, cx| s.set_value(rev, window, cx));
        r.revision_touched = false;
        self.record(cx);
    }

    fn snapshot(&self, cx: &App) -> Snapshot {
        Snapshot { items: self.items(cx), name: self.name.read(cx).value().to_string(), notes: self.notes.read(cx).value().to_string() }
    }

    /// Note a change of the form (one undo step).
    /// Record typing in `field`: continues the newest step while the same
    /// field is being typed in.
    fn record_typing(&mut self, field: EntityId, cx: &mut Context<Self>) {
        if self.restoring {
            return;
        }
        if self.typing == Some(field) && self.at > 0 && self.at + 1 == self.history.len() {
            self.history[self.at] = self.snapshot(cx);
            cx.notify();
            return;
        }
        self.record(cx);
        self.typing = Some(field);
    }

    fn record(&mut self, cx: &mut Context<Self>) {
        if self.restoring {
            return;
        }
        self.typing = None;
        let snap = self.snapshot(cx);
        if self.history.get(self.at) == Some(&snap) {
            cx.notify();
            return;
        }
        if !self.history.is_empty() {
            self.history.truncate(self.at + 1);
        }
        self.history.push(snap);
        self.at = self.history.len() - 1;
        cx.notify();
    }

    fn undo(&mut self, redo: bool, window: &mut Window, cx: &mut Context<Self>) {
        let to = if redo { self.at + 1 } else { self.at.wrapping_sub(1) };
        let Some(snap) = self.history.get(to).cloned() else { return };
        self.at = to;
        self.typing = None;
        self.restoring = true;
        self.rows.clear();
        self.add_items(snap.items, window, cx);
        self.name.update(cx, |s, cx| s.set_value(snap.name.clone(), window, cx));
        self.notes.update(cx, |s, cx| s.set_value(snap.notes.clone(), window, cx));
        self.restoring = false;
        cx.notify();
    }

    /// Refresh: the items as they are in the workspace now.
    fn refresh(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let fresh = refreshed_items(self.doc.read(cx), &self.items(cx));
        for (row, it) in self.rows.iter_mut().zip(fresh) {
            row.part_number.update(cx, |s, cx| s.set_value(it.part_number.clone(), window, cx));
            row.revision.update(cx, |s, cx| s.set_value(it.revision.clone(), window, cx));
            row.revision_touched = false;
            row.item = it;
        }
        // The pictures show the refreshed items.
        self.thumbs.clear();
        let items = self.rows.iter().map(|r| r.item.clone()).collect();
        self.request_thumbs(items, cx);
        cx.notify();
    }

    /// The items as typed.
    fn items(&self, cx: &App) -> Vec<ReleaseItem> {
        self.rows
            .iter()
            .map(|r| {
                let rev = r.revision.read(cx).value().trim().to_string();
                // Alphabetical labels are capitals; other schemes as typed.
                let rev = if release_settings().scheme == noshape_model::release::RevisionScheme::Alphabetical { rev.to_uppercase() } else { rev };
                ReleaseItem { part_number: r.part_number.read(cx).value().trim().to_string(), revision: rev, ..r.item.clone() }
            })
            .collect()
    }

    fn issues(&self, cx: &App) -> Vec<Vec<Issue>> {
        match self.mode {
            Mode::Release => release_log(self.doc.read(cx)).validate(&self.items(cx)),
            _ => vec![Vec::new(); self.rows.len()],
        }
    }

    fn package(&self, kind: PackageKind, cx: &App) -> Package {
        Package {
            id: 0,
            kind,
            name: self.name.read(cx).value().trim().to_string(),
            notes: self.notes.read(cx).value().to_string(),
            created: chrono::Utc::now().timestamp_millis(),
            version: None,
            items: self.items(cx),
            re_releasable: self.re_releasable,
        }
    }

    /// Save draft / Apply: keep the candidate in the document.
    fn save_draft(&mut self, cx: &mut Context<Self>) -> bool {
        let pkg = self.package(PackageKind::Release, cx);
        if pkg.name.is_empty() {
            cx.notify();
            return false;
        }
        let r = self.doc.update(cx, |doc, _| {
            let mut log = release_log(doc);
            log.save_draft(pkg);
            save_log(doc, &log)
        });
        if let Err(e) = r {
            self.doc.update(cx, |_, cx| cx.emit(DocEvent::Error(format!("{e:#}"))));
            return false;
        }
        self.applied = true;
        cx.notify();
        true
    }

    /// Release (or Obsolete): record the package at a new version.
    fn commit(&mut self, cx: &mut Context<Self>) -> bool {
        let kind = if self.mode == Mode::Obsolete { PackageKind::Obsoletion } else { PackageKind::Release };
        let mut pkg = self.package(kind, cx);
        if pkg.name.is_empty() || (kind == PackageKind::Obsoletion && pkg.notes.trim().is_empty()) || self.issues(cx).iter().any(|v| !v.is_empty()) {
            cx.notify();
            return false;
        }
        let r = self.doc.update(cx, |doc, cx| -> anyhow::Result<()> {
            // The part numbers go onto the parts (one change per Part Studio).
            if kind == PackageKind::Release {
                // Items linked from versions keep theirs (versions don't change).
                let mut tabs: Vec<TabId> = pkg.items.iter().filter(|i| i.document.is_none()).map(|i| i.tab).collect();
                tabs.dedup();
                for tab in tabs {
                    let items: Vec<ReleaseItem> = pkg.items.iter().filter(|i| i.tab == tab && i.part.is_some() && i.document.is_none()).cloned().collect();
                    let changed = doc.session.doc.tab(tab).is_some_and(|t| match &t.content {
                        noshape_model::document::TabContent::PartStudio(ps) => items.iter().any(|i| i.part.as_ref().and_then(|b| ps.part_properties.get(b)).and_then(|p| p.part_number.as_deref()) != Some(i.part_number.as_str())),
                        _ => false,
                    });
                    if changed {
                        doc.edit_studio(tab, &format!("Release {}", pkg.name), cx, |ps| {
                            for i in &items {
                                if let Some(b) = &i.part {
                                    ps.part_properties.entry(b.clone()).or_default().part_number = Some(i.part_number.clone());
                                }
                            }
                        });
                    }
                }
            }
            // Tab items keep their part numbers: an Assembly in its
            // Properties, other tabs in their metadata.
            if kind == PackageKind::Release {
                for i in pkg.items.iter().filter(|i| i.part.is_none() && i.document.is_none()) {
                    if tab_part_number(doc, i.tab) == i.part_number {
                        continue;
                    }
                    let is_assembly = matches!(doc.session.doc.tab(i.tab).map(|t| &t.content), Some(noshape_model::document::TabContent::Assembly(_)));
                    if is_assembly {
                        let pn = i.part_number.clone();
                        doc.edit_tab(i.tab, &format!("Release {}", pkg.name), cx, |c| {
                            if let noshape_model::document::TabContent::Assembly(a) = c {
                                a.properties.part_number = Some(pn);
                            }
                        });
                    } else {
                        doc.session.store.set_meta(&format!("tab_part_number:{}", i.tab.0), Some(i.part_number.as_bytes()))?;
                    }
                }
            }
            // An obsoletion marks the version of the release it obsoletes;
            // a release creates a version named after it.
            let released_at = (kind == PackageKind::Obsoletion).then(|| release_log(doc)).and_then(|log| pkg.items.iter().find_map(|i| log.release_version_of(&i.part_number, &i.revision)));
            pkg.version = Some(match released_at {
                Some(v) => v,
                None => doc.session.create_version(&pkg.name, &pkg.notes)?.to_string(),
            });
            if kind == PackageKind::Release {
                add_source::mark_linked_versions(&pkg)?;
            }
            let mut log = release_log(doc);
            log.record(pkg);
            save_log(doc, &log)?;
            doc.reloaded(cx);
            Ok(())
        });
        if let Err(e) = r {
            self.doc.update(cx, |_, cx| cx.emit(DocEvent::Error(format!("{e:#}"))));
            return false;
        }
        true
    }
}

/// Items table columns (Help image savedraftbutton-relcandidatedialog,
/// scaled to the 820 px dialog) and its grid lines.
const COL_ITEM: f32 = 270.;
const COL_REVISION: f32 = 140.;
const COL_STATE: f32 = 140.;
const COL_PART_NUMBER: f32 = 150.;
const GRID: u32 = 0xdddddd;

impl Render for ReleaseDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.thumbs_started {
            self.thumbs_started = true;
            let items = self.rows.iter().map(|r| r.item.clone()).collect();
            self.request_thumbs(items, cx);
        }
        let pal = p();
        let read_only = self.mode == Mode::Review;
        let issues = self.issues(cx);
        let attention = issues.iter().filter(|v| !v.is_empty()).count();
        let log = release_log(self.doc.read(cx));
        let red = gpui_kit::rgb(0xbd3039);
        // A header cell: grid line on its left.
        let head = |w: f32| div().w(px(w)).flex_none().h_full().flex().items_center().px(px(10.)).border_l_1().border_color(gpui_kit::rgb(GRID));
        // Bordered 30 px icon buttons (row and header actions).
        let boxed = |id: SharedString, icon: &'static str, tip: &'static str, cx: &mut Context<Self>| {
            div()
                .id(id)
                .size(px(30.))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(3.))
                .border_1()
                .border_color(gpui_kit::rgb(0xaaaaaa))
                .bg(gpui_kit::rgb(0xffffff))
                .cursor_pointer()
                .hover(|d| d.bg(gpui_kit::rgb(0xeaeaea)))
                .child(crate::assets::icon(icon, 18., cx))
                .when(!tip.is_empty(), |d| d.tooltip(crate::ui::tooltip(tip)))
        };
        let small = |id: &'static str, icon: &'static str, tip: &'static str, cx: &mut Context<Self>| {
            div()
                .id(id)
                .size(px(26.))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(3.))
                .cursor_pointer()
                .hover(|d| d.bg(gpui_kit::rgb(0xeaeaea)))
                .child(crate::assets::icon(icon, 18., cx))
                .tooltip(crate::ui::tooltip(tip))
        };
        // Header: search, filter, add buttons | Revision | State | Part number |
        // "N item(s) needs attention" (toggles the problem filter).
        let unchanged = self.rows.iter().any(|r| r.since == Some(SinceRelease::Unchanged) && !r.item.reverted);
        let header = h_flex()
            .h(px(40.))
            .flex_none()
            .border_b_1()
            .border_color(gpui_kit::rgb(GRID))
            .text_size(px(14.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(pal.text)
            .child(
                h_flex()
                    .w(px(COL_ITEM))
                    .flex_none()
                    .h_full()
                    .px(px(6.))
                    .gap(px(4.))
                    .font_weight(FontWeight::NORMAL)
                    .child(gpui_kit::base::input::input_frame(div().flex_1().h(px(28.)).flex().items_center().border_1().rounded(px(3.)).border_color(gpui_kit::rgb(0xcccccc)).child(Input::new(&self.search).appearance(false).small())))
                    .child(small("rel-filter", "filter", "Filter", cx).when(self.filter_open || !self.kinds.is_empty(), |d| d.bg(gpui_kit::rgb(0xeaeaea))).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.filter_open = !this.filter_open;
                        cx.notify();
                    })))
                    .when(self.mode == Mode::Release, |d| {
                        d.child(small("rel-assoc-drawings", "release-drawings", "Include associated drawings from current workspace", cx).on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                            let extra = associated_drawings(this.doc.read(cx), &this.items(cx));
                            this.add_items(extra, window, cx);
                        })))
                        .child(small("rel-add-items", "plus", "Add to Release…", cx).on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                            let me = cx.entity();
                            open_add_items(this.doc.clone(), move |items, window, cx| me.update(cx, |this, cx| this.add_items(items, window, cx)), window, cx);
                        })))
                    })
                    // Obsolete candidate: "Add to Obsolete candidate" (Help step 5).
                    .when(self.mode == Mode::Obsolete, |d| {
                        d.child(small("rel-add-obsolete", "plus", "Add to Obsoletion…", cx).on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                            let me = cx.entity();
                            open_add_obsolete_items(this.doc.clone(), move |items, window, cx| me.update(cx, |this, cx| this.add_items(items, window, cx)), window, cx);
                        })))
                    }),
            )
            .child(head(COL_REVISION).child("Revision"))
            .child(head(COL_STATE).justify_between().child("State").when(self.mode == Mode::Release, |d| {
                // Help: the button on the State header reverts every unchanged item.
                d.child(
                    boxed("rel-revert-all".into(), "undo", "Revert unchanged items to their last revision", cx)
                        .when(!unchanged, |d| d.opacity(0.4).cursor_default())
                        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                            let ids: Vec<usize> = this.rows.iter().enumerate().filter(|(_, r)| r.since == Some(SinceRelease::Unchanged) && !r.item.reverted).map(|(i, _)| i).collect();
                            for i in ids {
                                this.set_reverted(i, true, window, cx);
                            }
                        })),
                )
            }))
            .child(head(COL_PART_NUMBER).justify_between().child("Part number").when(self.seq_parts && self.mode == Mode::Release, |d| {
                // Bundle release-dialog.html: the header's "Generate missing
                // part numbers" (icon partnumber), disabled when none is missing.
                let missing = self.rows.iter().any(|r| !r.item.reverted && r.part_number.read(cx).value().trim().is_empty());
                d.child(
                    boxed("rel-gen-all".into(), "part-number", "Generate missing part numbers", cx)
                        .when(!missing, |d| d.opacity(0.4).cursor_default())
                        .when(missing, |d| d.on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.generate_missing_part_numbers(window, cx)))),
                )
            }))
            .child(div().flex_1().h_full().border_l_1().border_color(gpui_kit::rgb(GRID)));
        // Bundle release-dialog.html (.link-container): the error-filter label
        // "1 item needs attention" / "N items need attention", the filter
        // button (shown while there are errors or it is on) and the ⋯ menu.
        let attention_label = (attention > 0).then(|| {
            div().text_size(px(13.)).text_color(pal.text).whitespace_nowrap().child(if attention == 1 { "1 item needs attention".to_string() } else { format!("{attention} items need attention") })
        });
        let attention_filter = (attention > 0 || self.problems_only).then(|| {
            boxed("rel-attention".into(), "list-error-filter", "Show only items that need attention", cx).when(self.problems_only, |d| d.bg(gpui_kit::rgb(0xd7d7d7))).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.problems_only = !this.problems_only;
                cx.notify();
            }))
        });
        let overflow = div()
            .relative()
            .child(boxed("rel-overflow".into(), "more", "", cx).when(self.overflow_open, |d| d.bg(gpui_kit::rgb(0xd7d7d7))).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.overflow_open = !this.overflow_open;
                cx.notify();
            })))
            .when(self.overflow_open, |d| {
                // Bootstrap dropdown-menu-end: View audit log, View workflow
                // (every candidate has a published workflow; not once discarded).
                // Deferred: the table below would paint over it.
                d.child(div().absolute().top(px(32.)).right_0().child(deferred(anchored().anchor(Anchor::TopRight).child(
                    v_flex()
                        .id("rel-overflow-menu")
                        .occlude()
                        .py(px(4.))
                        .bg(gpui_kit::rgb(0xffffff))
                        .border_1()
                        .border_color(gpui_kit::rgb(0xcccccc))
                        .rounded(px(4.))
                        .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.18), offset: point(px(0.), px(6.)), blur_radius: px(12.), spread_radius: px(0.), inset: false }])
                        .child(
                            div()
                                .id("rel-audit-log")
                                .px(px(16.))
                                .py(px(4.))
                                .text_size(px(14.))
                                .text_color(pal.text)
                                .whitespace_nowrap()
                                .cursor_pointer()
                                .hover(|d| d.bg(gpui_kit::rgb(0xe9ecef)))
                                .child("View audit log")
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.overflow_open = false;
                                    this.view_audit_log(cx);
                                    cx.notify();
                                })),
                        )
                        .child(
                            div()
                                .id("rel-view-workflow")
                                .px(px(16.))
                                .py(px(4.))
                                .text_size(px(14.))
                                .text_color(pal.text)
                                .whitespace_nowrap()
                                .cursor_pointer()
                                .hover(|d| d.bg(gpui_kit::rgb(0xe9ecef)))
                                .child("View workflow")
                                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                    this.overflow_open = false;
                                    this.toggle_workflow(window, cx);
                                })),
                        ),
                // Above the modal (the kit draws dialogs at popup priority 100).
                )).with_priority(101)))
            });
        // Type icons toggle a filter; Clear removes it.
        let filter_row = self.filter_open.then(|| {
            let kind_button = |k: ItemKind, icon: &'static str, cx: &mut Context<Self>| {
                let on = self.kinds.contains(&k);
                div()
                    .id(SharedString::from(format!("rel-kind-{icon}")))
                    .size(px(26.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(3.))
                    .border_1()
                    .border_color(if on { pal.accent } else { gpui_kit::rgb(0xd7d7d7) })
                    .when(on, |d| d.bg(pal.row_hover))
                    .cursor_pointer()
                    .child(crate::assets::icon(icon, 18., cx))
                    .tooltip(crate::ui::tooltip(k.label()))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if let Some(i) = this.kinds.iter().position(|x| *x == k) {
                            this.kinds.remove(i);
                        } else {
                            this.kinds.push(k);
                        }
                        cx.notify();
                    }))
            };
            h_flex()
                .px(px(8.))
                .py(px(4.))
                .gap_1()
                .border_b_1()
                .border_color(gpui_kit::rgb(GRID))
                .child(kind_button(ItemKind::Part, "part", cx))
                .child(kind_button(ItemKind::Assembly, "assembly", cx))
                .child(kind_button(ItemKind::Drawing, "drawing", cx))
                .child(kind_button(ItemKind::VariableStudio, "variable-studio", cx))
                .child(kind_button(ItemKind::File, "file", cx))
                .child(os::secondary_button("rel-kind-clear", "Clear", cx).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.kinds.clear();
                    cx.notify();
                })))
        });
        let query = self.search.read(cx).value().trim().to_lowercase();
        let mut rows_el: Vec<AnyElement> = Vec::new();
        for (i, row) in self.rows.iter().enumerate() {
            let row_issues = issues.get(i).cloned().unwrap_or_default();
            if self.problems_only && row_issues.is_empty() {
                continue;
            }
            if (!query.is_empty() && !row.item.name.to_lowercase().contains(&query)) || (!self.kinds.is_empty() && !self.kinds.contains(&row.item.kind)) {
                continue;
            }
            let pn = row.part_number.read(cx).value().trim().to_string();
            // Release__typical_release_workflow: workspace items are "In
            // progress"; "Released" marks already released (linked) ones.
            let state = match self.mode {
                Mode::Obsolete => "Released",
                Mode::Review => log.state(&pn).label(),
                Mode::Release if row.item.reverted => "Released",
                Mode::Release => "In progress",
            };
            let since = row.since.filter(|_| self.mode == Mode::Release);
            let reverted = row.item.reverted;
            let missing = row_issues.contains(&Issue::MissingPartNumber);
            // Help: "New or modified fields are marked with a yellow triangle
            // in the upper left corner of the field".
            let editing = self.mode == Mode::Release;
            let field = |st: &Entity<InputState>, bad: bool, marked: bool, locked: bool| {
                div()
                    .relative()
                    .w_full()
                    .h(px(28.))
                    .flex()
                    .items_center()
                    .border_1()
                    .rounded(px(3.))
                    .border_color(if bad { red } else { gpui_kit::rgb(0xcccccc) })
                    .child(Input::new(st).appearance(false).small().disabled(read_only || locked || self.mode == Mode::Obsolete))
                    .when(marked, |d| d.child(crate::ui::glyph("ui-corner-mark", 9., gpui_kit::rgb(0xf5b400)).absolute().top_0().left_0()))
            };
            let pn_modified = editing && pn != row.initial_pn;
            let icon = match row.item.kind {
                ItemKind::Part => "part",
                ItemKind::Assembly => "assembly",
                ItemKind::Drawing => "drawing",
                ItemKind::VariableStudio => "variable-studio",
                ItemKind::File => "file",
            };
            let removable = self.mode != Mode::Review;
            let picture = self.thumbs.get(&crate::release_thumbs::key(&row.item)).cloned();
            let main = h_flex()
                .h(px(46.))
                .flex_none()
                .text_size(px(14.))
                .text_color(pal.text)
                .child(
                    h_flex()
                        .relative()
                        .w(px(COL_ITEM))
                        .flex_none()
                        .h_full()
                        .pl(px(20.))
                        .pr(px(10.))
                        .gap(px(8.))
                        // The row's item is new in this candidate (yellow mark, Help image).
                        .when(editing && !reverted, |d| d.child(crate::ui::glyph("ui-corner-mark", 9., gpui_kit::rgb(0xf5b400)).absolute().top_0().left_0()))
                        .child(div().w(px(54.)).h(px(40.)).flex_none().children(picture.map(|im| img(ImageSource::Render(im)).w(px(54.)).h(px(40.)).object_fit(ObjectFit::Contain))))
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .child(h_flex().gap(px(4.)).child(crate::assets::icon(icon, 16., cx)).child({
                                    // The name opens the item's tab (the dialog closes first).
                                    let t = row.item.tab;
                                    div().id(SharedString::from(format!("rel-name-{i}"))).truncate().cursor_pointer().hover(|d| d.underline()).child(row.item.name.clone()).on_click(move |_, window, cx| {
                                        window.close_dialog(cx);
                                        crate::workspace::request_activate_tab(t, window, cx);
                                    })
                                })
                                // Help (add-item-version2): an item linked from another
                                // version shows the version icon after its name.
                                .when_some(row.item.document.as_ref(), |d, l| {
                                    d.child(
                                        div()
                                            .id(SharedString::from(format!("rel-linked-{i}")))
                                            .flex_none()
                                            .child(crate::assets::icon("list-versioned", 14., cx))
                                            .tooltip(crate::ui::tooltip(format!("{} - {}", l.name, l.version_name))),
                                    )
                                }))
                                .child(div().pl(px(20.)).text_size(px(13.)).italic().text_color(pal.text_muted).child(row.item.kind.label())),
                        )
                        .when(removable, |d| {
                            d.child(
                                div()
                                    .id(SharedString::from(format!("rel-x-{i}")))
                                    .flex_none()
                                    .cursor_pointer()
                                    .child(Icon::new(IconName::Close).with_size(px(16.)).text_color(red))
                                    .tooltip(crate::ui::tooltip("Remove"))
                                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                        if i < this.rows.len() {
                                            this.rows.remove(i);
                                        }
                                        this.record(cx);
                                    })),
                            )
                        }),
                )
                .child(div().w(px(COL_REVISION)).flex_none().flex().justify_end().px(px(10.)).child(div().w(px(64.)).child(field(&row.revision, row_issues.iter().any(|x| matches!(x, Issue::InvalidRevision | Issue::RevisionTooLow { .. })), editing && !row.item.not_revision_managed, reverted))))
                .child(h_flex().w(px(COL_STATE)).flex_none().px(px(10.)).gap(px(6.)).child(state).when(since.is_some(), |d| {
                    d.child(
                        boxed(SharedString::from(format!("rel-revert-{i}")), "undo", if reverted { "Use the workspace item" } else { "Revert to last revision" }, cx)
                            .when(reverted, |d| d.bg(gpui_kit::rgb(0xeaeaea)))
                            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.set_reverted(i, !reverted, window, cx))),
                    )
                }))
                .child(div().w(px(COL_PART_NUMBER)).flex_none().px(px(10.)).child(field(&row.part_number, missing, pn_modified, reverted).when(self.seq_parts && editing && !reverted, |d| {
                    // The field's # ("Generate next part number"), greyed once
                    // the item has a number.
                    let empty = pn.trim().is_empty();
                    d.child(
                        div()
                            .id(SharedString::from(format!("rel-gen-{i}")))
                            .w(px(28.))
                            .h_full()
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .border_l_1()
                            .border_color(gpui_kit::rgb(0xcccccc))
                            .child(crate::assets::icon("part-number", 18., cx))
                            .tooltip(crate::ui::tooltip("Generate next part number"))
                            .when(!empty, |d| d.opacity(0.4))
                            .when(empty, |d| d.cursor_pointer().hover(|d| d.bg(gpui_kit::rgb(0xeaeaea))).on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.generate_part_number(i, window, cx)))),
                    )
                })))
                // Help image: per-row Edit properties and Export buttons.
                .child(
                    h_flex()
                        .flex_1()
                        .px(px(10.))
                        .gap(px(6.))
                        .child({
                            let item = row.item.clone();
                            let me = cx.entity();
                            // A row from a version (another document's or this
                            // one's): that version, in a window of its own.
                            let doc_path = self.doc.read(cx).session.path.clone();
                            boxed(SharedString::from(format!("rel-props-{i}")), "properties", "Edit properties", cx).on_click(move |_, window, cx| {
                                if item.document.is_some() {
                                    add_source::run_linked(&item, &doc_path, add_source::LinkedAction::Properties, cx);
                                    return;
                                }
                                let me = me.clone();
                                let doc = me.read(cx).doc.clone();
                                open_item_properties(doc, item.clone(), move |pn, window, cx| me.update(cx, |this, cx| {
                                    if let Some(r) = this.rows.get(i) {
                                        r.part_number.update(cx, |s, cx| s.set_value(pn.clone(), window, cx));
                                    }
                                    cx.notify();
                                }), window, cx)
                            })
                        })
                        .child({
                            let item = row.item.clone();
                            let doc_path = self.doc.read(cx).session.path.clone();
                            boxed(SharedString::from(format!("rel-export-{i}")), "export", "Export", cx).on_click(move |_, window, cx| {
                                if item.document.is_some() {
                                    add_source::run_linked(&item, &doc_path, add_source::LinkedAction::Export, cx);
                                } else {
                                    crate::workspace::request_export(item.tab, item.part.iter().cloned().collect(), window, cx)
                                }
                            })
                        }),
                );
            let note = |text: SharedString, color: Hsla| div().pl(px(COL_ITEM + 10.)).pb(px(4.)).text_size(px(12.)).text_color(color).child(text).into_any_element();
            rows_el.push(
                v_flex()
                    .child(main)
                    .children(row_issues.iter().filter(|x| **x != Issue::MissingPartNumber).map(|x| note(x.message().into(), red.into())))
                    .children(since.filter(|_| !reverted).map(|s| {
                        note(
                            match s {
                                SinceRelease::Unchanged => "Item has not changed since its latest revision.",
                                SinceRelease::MetadataChanged => "Only the item's properties have changed since its latest revision.",
                            }
                            .into(),
                            pal.text_muted.into(),
                        )
                    }))
                    .into_any_element(),
            );
        }
        // The item column's grid line runs the table's full height.
        let list = div()
            .relative()
            .h(px(212.))
            .flex_none()
            .child(div().absolute().top_0().bottom_0().left(px(COL_ITEM)).w(px(1.)).bg(gpui_kit::rgb(GRID)))
            .child(v_flex().id("rel-items").size_full().overflow_y_scroll().children(rows_el));
        let (name_label, notes_label) = match self.mode {
            Mode::Obsolete => ("Obsoletion name", "Obsoletion notes"),
            _ => ("Release name", "Release notes"),
        };
        let name_empty = self.name.read(cx).value().trim().is_empty();
        let label = |text: &'static str, required: bool, empty: bool| {
            h_flex()
                .gap_2()
                .child(h_flex().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child(text).when(required, |d| d.child(div().pl(px(3.)).text_color(red).child("*"))))
                .when(required && empty, |d| d.child(div().text_size(px(12.)).text_color(red).child("Required field must not be empty.")))
        };
        let form = h_flex()
            .items_start()
            .px(px(12.))
            .pt(px(14.))
            .pb(px(14.))
            .child(
                v_flex()
                    .w(px(380.))
                    .gap(px(4.))
                    .child(label(name_label, true, name_empty))
                    .child(
                        h_flex()
                            .gap(px(4.))
                            .child(gpui_kit::base::input::input_frame(div().flex_1().h(px(30.)).flex().items_center().border_1().rounded(px(3.)).border_color(if name_empty { red } else { gpui_kit::rgb(0xcccccc) }).child(Input::new(&self.name).appearance(false).small().disabled(read_only))))
                            // Numbering schemes › Release names: "Generate next
                            // release name" (not for obsoletions).
                            .when(self.seq_names && self.mode == Mode::Release, |d| {
                                d.child(
                                    boxed("rel-gen-name".into(), "part-number", "Generate next release name", cx)
                                        .rounded_full()
                                        .when(!name_empty, |d| d.opacity(0.4).cursor_default())
                                        .when(name_empty, |d| {
                                            d.on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                                if let Some(n) = next_number(|s| s.release_names.generate(0)) {
                                                    this.name.update(cx, |s, cx| s.set_value(n, window, cx));
                                                }
                                            }))
                                        }),
                                )
                            }),
                    )
                    .child(div().pt(px(6.)).child(label(notes_label, self.mode == Mode::Obsolete, false)))
                    .child(Textarea::new(&self.notes).disabled(read_only))
                    .when(self.mode == Mode::Obsolete, |d| {
                        let on = self.re_releasable;
                        d.child(div().pt(px(6.)).child(os::checkbox("rel-rerelease", "Mark revision as re-releasable", on).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.re_releasable = !this.re_releasable;
                            cx.notify();
                        }))))
                    }),
            );
        let blocked = attention > 0 || name_empty;
        let status = (blocked && !read_only).then(|| div().text_size(px(12.)).text_color(pal.text_muted).child("A field required by one or more actions is empty ⓘ"));
        let button = |id: &'static str, text: &'static str, bg: u32, enabled: bool| {
            div()
                .id(id)
                .h(px(34.))
                .flex()
                .items_center()
                .px(px(11.))
                .rounded(px(3.))
                .text_size(px(14.))
                .text_color(gpui_kit::rgb(0xffffff))
                .bg(gpui_kit::rgb(bg))
                .when(!enabled, |d| d.opacity(0.5))
                .when(enabled, |d| d.cursor_pointer())
                .child(text)
        };
        let close = |window: &mut Window, cx: &mut App| window.close_dialog(cx);
        // Help (savedraftbutton-relcandidatedialog): Save draft at the
        // bottom left; a saved draft adds a ▴ menu with Save draft / Delete draft.
        if self.history.is_empty() {
            self.history.push(self.snapshot(cx));
        }
        let (can_undo, can_redo) = (self.at > 0, self.at + 1 < self.history.len());
        // Help image: the available one is a bordered button, the other faded.
        let undo_btn = |id: &'static str, icon: &'static str, tip: &'static str, on: bool, cx: &mut Context<Self>| {
            div()
                .id(id)
                .size(px(30.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(3.))
                .when(!on, |d| d.opacity(0.3))
                .when(on, |d| d.border_1().border_color(gpui_kit::rgb(0xaaaaaa)).cursor_pointer().hover(|d| d.bg(gpui_kit::rgb(0xeaeaea))))
                .child(crate::assets::icon(icon, 18., cx))
                .tooltip(crate::ui::tooltip(tip))
        };
        let mut left = h_flex().gap_1();
        if self.mode == Mode::Release {
            left = left
                .child(undo_btn("rel-undo", "undo", "Undo", can_undo, cx).on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.undo(false, window, cx))))
                .child(undo_btn("rel-redo", "redo", "Redo", can_redo, cx).on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.undo(true, window, cx))));
            left = left.child(os::secondary_button("rel-draft", "Save draft", cx).on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                if this.save_draft(cx) {
                    close(window, cx);
                }
            })));
            if let Some(draft) = self.draft.clone() {
                let open = self.draft_menu;
                left = left.child(
                    div()
                        .relative()
                        .child(os::secondary_button("rel-draft-menu", "▴", cx).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.draft_menu = !this.draft_menu;
                            cx.notify();
                        })))
                        .when(open, |d| {
                            let item = |id: &'static str, label: &'static str| div().id(id).px(px(12.)).py(px(6.)).text_size(px(13.)).text_color(pal.accent).cursor_pointer().hover(|d| d.bg(gpui_kit::rgb(0xf2f2f2))).child(label);
                            d.child(
                                v_flex()
                                    .absolute()
                                    .bottom(px(34.))
                                    .left_0()
                                    .w(px(130.))
                                    .bg(gpui_kit::rgb(0xffffff))
                                    .border_1()
                                    .border_color(gpui_kit::rgb(0xcccccc))
                                    .rounded(px(4.))
                                    .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
                                    .child(item("rel-draft-save", "Save draft").on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                                        if this.save_draft(cx) {
                                            close(window, cx);
                                        }
                                    })))
                                    .child(item("rel-draft-delete", "Delete draft").on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                                        let name = draft.clone();
                                        let r = this.doc.update(cx, |doc, cx| {
                                            let mut log = release_log(doc);
                                            log.delete_draft(&name);
                                            let r = save_log(doc, &log);
                                            cx.notify();
                                            r
                                        });
                                        if let Err(e) = r {
                                            this.doc.update(cx, |_, cx| cx.emit(DocEvent::Error(format!("{e:#}"))));
                                        }
                                        close(window, cx);
                                    }))),
                            )
                        }),
                );
            }
        }
        let mut footer = h_flex().gap_2().justify_end().children(status);
        match self.mode {
            Mode::Release => {
                footer = footer
                    .child(button("rel-apply", "Apply", 0x1651b0, !name_empty).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.save_draft(cx);
                    })))
                    .child(button("rel-release", "Release", 0x2b9d3a, !blocked).on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        if this.commit(cx) {
                            close(window, cx);
                        }
                    })));
            }
            Mode::Obsolete => {
                let ok = !name_empty && !self.notes.read(cx).value().trim().is_empty();
                footer = footer.child(button("rel-obsolete", "Obsolete", 0xbd3039, ok).on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                    if this.commit(cx) {
                        close(window, cx);
                    }
                })));
            }
            // Release__cloning_a_release_package: Clone starts a new candidate
            // from the current workspace (not while viewing a version).
            Mode::Review if self.doc.read(cx).session.viewing.is_none() => {
                footer = footer.child(os::secondary_button("rel-clone", "Clone", cx).on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                    let doc = this.doc.clone();
                    let items = refreshed_items(doc.read(cx), &this.items(cx));
                    let (name, notes) = (this.name.read(cx).value().to_string(), this.notes.read(cx).value().to_string());
                    close(window, cx);
                    open(doc, Mode::Release, items, name, notes, window, cx);
                })));
            }
            Mode::Review => {}
        }
        footer = footer.child(os::cancel_button("rel-close", "Close", cx).on_click(move |_, window, cx| close(window, cx)));
        let title = match self.mode {
            Mode::Release if self.applied => format!("Review Release: {}", self.name.read(cx).value()),
            Mode::Release => "Create Release candidate".into(),
            Mode::Review => format!("Review Release: {}", self.name.read(cx).value()),
            Mode::Obsolete => "Create Obsolete candidate".into(),
        };
        let workflow = self.render_workflow(window, cx);
        v_flex()
            .w_full()
            .children(workflow)
            // Ctrl+Z / Ctrl+Shift+Z / Ctrl+Y (not taken by a text field) undo
            // and redo the form.
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                let k = &ev.keystroke;
                let cmd = k.modifiers.secondary();
                if cmd && k.key == "z" {
                    this.undo(k.modifiers.shift, window, cx);
                    cx.stop_propagation();
                } else if k.modifiers.control && k.key == "y" {
                    this.undo(true, window, cx);
                    cx.stop_propagation();
                }
            }))
            .child(
                h_flex()
                    .h(px(50.))
                    .px(px(10.))
                    .gap(px(10.))
                    .child(div().text_size(px(20.)).font_weight(FontWeight::MEDIUM).text_color(pal.text).whitespace_nowrap().child(title))
                    // Help: Refresh "to make sure you have the latest iteration of the objects".
                    .when(self.mode == Mode::Release, |d| d.child(boxed("rel-refresh".into(), "refresh", "Refresh", cx).on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.refresh(window, cx)))))
                    .child(div().flex_1())
                    .children(attention_label)
                    .children(attention_filter)
                    .child(overflow),
            )
            .child(v_flex().border_t_1().border_b_1().border_color(gpui_kit::rgb(GRID)).child(header).children(filter_row).child(list))
            .child(form)
            .child(h_flex().h(px(56.)).px(px(12.)).gap(px(8.)).border_t_1().border_color(gpui_kit::rgb(os::MODAL_DIVIDER)).child(left).child(div().flex_1().child(footer)))
    }
}

/// The release dialogs' modal: no title bar (the dialog draws its own
/// title row, Help image savedraftbutton-relcandidatedialog).
fn release_modal(dialog: gpui_kit::component::dialog::Dialog) -> gpui_kit::component::dialog::Dialog {
    dialog.w(px(820.)).margin_top(px(26.)).close_button(false).p_0().gap_0().rounded(px(4.)).border_color(gpui_kit::rgb(os::MODAL_DIVIDER))
}

/// A ReleaseDialog's modal. While the workflow lightbox is open, Esc
/// closes only the lightbox (bundle: preventEscapeFromClosingDialog).
fn release_dialog_modal(dialog: gpui_kit::component::dialog::Dialog, view: &Entity<ReleaseDialog>, cx: &App) -> gpui_kit::component::dialog::Dialog {
    release_modal(dialog).keyboard(view.read(cx).workflow.is_none()).child(view.clone())
}

/// Open "Create Release candidate" for items (parts list or versions panel).
pub fn open_release_candidate(doc: Entity<DocModel>, items: Vec<ReleaseItem>, window: &mut Window, cx: &mut App) {
    // The required Release name starts empty (Onshape fills it only with
    // sequential release numbering set up).
    open(doc, Mode::Release, items, String::new(), String::new(), window, cx);
}

/// Review a recorded package.
pub fn open_review(doc: Entity<DocModel>, package: &Package, window: &mut Window, cx: &mut App) {
    let package = package.clone();
    let view = cx.new(|cx| {
        let mut d = ReleaseDialog::new(doc, Mode::Review, package.items.clone(), package.name.clone(), package.notes.clone(), window, cx);
        d.package = Some(package);
        d
    });
    window.open_dialog(cx, move |dialog, _, cx| release_dialog_modal(dialog, &view, cx));
}

fn open(doc: Entity<DocModel>, mode: Mode, items: Vec<ReleaseItem>, name: String, notes: String, window: &mut Window, cx: &mut App) {
    let view = cx.new(|cx| ReleaseDialog::new(doc, mode, items, name, notes, window, cx));
    window.open_dialog(cx, move |dialog, _, cx| release_dialog_modal(dialog, &view, cx));
}

/// A saved draft from the Release drafts panel: "Review Release", editable.
pub fn open_draft(doc: Entity<DocModel>, draft: &Package, window: &mut Window, cx: &mut App) {
    let name = draft.name.clone();
    let view = cx.new(|cx| {
        let mut d = ReleaseDialog::new(doc, Mode::Release, draft.items.clone(), draft.name.clone(), draft.notes.clone(), window, cx);
        d.package = Some(draft.clone());
        d.applied = true;
        d.draft = Some(name);
        d
    });
    window.open_dialog(cx, move |dialog, _, cx| release_dialog_modal(dialog, &view, cx));
}

/// The left panel "Release drafts" (help releasedraftspanel.png): one row
/// per saved candidate (part thumbnail, name, "by <user> <time>");
/// double-click opens it.
pub struct ReleaseDraftsPanel {
    doc: Entity<DocModel>,
    selected: Option<String>,
    _sub: Subscription,
}

impl ReleaseDraftsPanel {
    pub fn new(doc: Entity<DocModel>, cx: &mut Context<Self>) -> Self {
        let sub = cx.observe(&doc, |_, _, cx| cx.notify());
        Self { doc, selected: None, _sub: sub }
    }
}

impl Render for ReleaseDraftsPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let log = release_log(self.doc.read(cx));
        let user = std::env::var("USER").or_else(|_| std::env::var("USERNAME")).unwrap_or_default();
        let rows = log.drafts.iter().rev().enumerate().map(|(i, d)| {
            let selected = self.selected.as_deref() == Some(d.name.as_str());
            let (name, pkg, doc) = (d.name.clone(), d.clone(), self.doc.clone());
            let icon = match d.items.first().map(|i| i.kind) {
                Some(ItemKind::Assembly) => "assembly",
                Some(ItemKind::Drawing) => "drawing",
                _ => "part",
            };
            h_flex()
                .id(SharedString::from(format!("draft-{i}")))
                .px(px(12.))
                .py(px(8.))
                .gap(px(16.))
                .cursor_pointer()
                .when(selected, |d| d.bg(gpui_kit::rgb(0xbfe1f5)))
                .when(!selected, |d| d.hover(|d| d.bg(pal.row_hover)))
                .child(crate::assets::icon(icon, 40., cx))
                .child(
                    v_flex()
                        .text_size(px(14.))
                        .text_color(pal.text)
                        .child(d.name.clone())
                        .child(h_flex().gap_1().text_size(px(13.)).child("by").child(div().font_weight(FontWeight::SEMIBOLD).child(user.clone())).child(div().text_color(pal.text_muted).child(crate::start_library::format_onshape_time(d.created)))),
                )
                .on_click(cx.listener(move |this, ev: &ClickEvent, window, cx| {
                    this.selected = Some(name.clone());
                    if ev.click_count() >= 2 {
                        open_draft(doc.clone(), &pkg, window, cx);
                    }
                    cx.notify();
                }))
        });
        v_flex()
            .size_full()
            .bg(pal.panel)
            .child(
                h_flex()
                    .h(px(37.))
                    .flex_none()
                    .pl(px(5.))
                    .pr(px(6.))
                    .child(div().flex_1().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child("Release drafts"))
                    .child(crate::ui::icon_button("drafts-close", "close", 30., false, cx).tooltip("Close").on_click(|_, window, cx| window.dispatch_action(Box::new(crate::actions::ToggleReleaseDrafts), cx))),
            )
            .child(v_flex().id("drafts-list").flex_1().overflow_y_scroll().children(rows))
    }
}

/// "Revision history" of a part number: its revisions, newest first, with
/// View release and Obsolete revision.
pub fn open_revision_history(doc: Entity<DocModel>, part_number: String, window: &mut Window, cx: &mut App) {
    window.open_dialog(cx, move |dialog, _, cx| {
        let pal = p();
        let log = release_log(doc.read(cx));
        let versions = doc.read(cx).session.store.list_versions().unwrap_or_default();
        let doc_name = doc.read(cx).name();
        let mut rows = v_flex().gap(px(8.));
        for (k, e) in log.revisions(&part_number).into_iter().rev().enumerate() {
            let pkg = log.packages[e.release].clone();
            // The released version (its author is the releasing user).
            let version = pkg.version.as_ref().and_then(|v| versions.iter().find(|x| x.id.to_string() == *v)).cloned();
            let obsoletion = e.obsoleted_by.map(|i| log.packages[i].clone());
            let obsolete = e.obsoleted_by.is_some() || e.auto_obsolete;
            let item = pkg.items.iter().find(|i| i.part_number == part_number).cloned();
            let (d1, d2) = (doc.clone(), doc.clone());
            let pkg2 = pkg.clone();
            rows = rows.child(
                h_flex()
                    .gap_3()
                    .py(px(6.))
                    .border_b_1()
                    .border_color(gpui_kit::rgb(0xeeeeee))
                    .child(div().text_size(px(14.)).text_color(if obsolete { pal.text_muted } else { gpui_kit::rgb(0x2b9d3a) }).child(if obsolete { "△" } else { "▲" }))
                    .child(div().w(px(40.)).text_size(px(20.)).font_weight(FontWeight::BOLD).text_color(pal.text).child(e.revision.clone()))
                    .child(
                        v_flex()
                            .flex_1()
                            .text_size(px(12.))
                            .text_color(pal.text)
                            .child(h_flex().gap_2().child(div().font_weight(FontWeight::BOLD).child(pkg.name.clone())).child(div().text_color(pal.text_muted).child(crate::start_library::format_onshape_time(pkg.created))))
                            .when_some(version.as_ref().map(|v| v.author.clone()).filter(|a| !a.is_empty()), |d, a| d.child(div().text_color(pal.text_muted).child(format!("User: {a}"))))
                            .child(div().text_color(pal.text_muted).child(format!("Document: {doc_name}")))
                            // Help: "Click the version name … to open that version".
                            .child(h_flex().gap_1().text_color(pal.text_muted).child("Version:").child(
                                div()
                                    .id(SharedString::from(format!("rh-ver-{k}")))
                                    .text_color(pal.accent)
                                    .cursor_pointer()
                                    .child(version.as_ref().map(|v| v.name.clone()).unwrap_or_else(|| pkg.name.clone()))
                                    .on_click({
                                        let (d, mv) = (doc.clone(), version.as_ref().map(|v| v.microversion));
                                        move |_, window, cx| {
                                            window.close_dialog(cx);
                                            if let Some(mv) = mv {
                                                d.update(cx, |doc, cx| match doc.session.view_microversion(Some(mv)) {
                                                    Ok(()) => doc.reloaded(cx),
                                                    Err(e) => cx.emit(DocEvent::Error(format!("{e:#}"))),
                                                });
                                            }
                                        }
                                    }),
                            ))
                            .child(div().text_color(pal.text_muted).child(format!("Type: {}", item.as_ref().map_or("Part", |i| i.kind.label()))))
                            .when(obsolete, |d| d.child(div().text_color(pal.text_muted).child(if e.auto_obsolete && e.obsoleted_by.is_none() { "Obsoleted by a later revision" } else { "Obsolete" }))),
                    )
                    // Help: "An Export button that exports the entities involved in
                    // the release": the released version, read-only, then Export.
                    .child({
                        let (d, mv, it) = (doc.clone(), version.as_ref().map(|v| v.microversion), item.clone());
                        os::secondary_button(SharedString::from(format!("rh-export-{k}")), "Export", cx).on_click(move |_, window, cx| {
                            window.close_dialog(cx);
                            if let Some(mv) = mv {
                                d.update(cx, |doc, cx| match doc.session.view_microversion(Some(mv)) {
                                    Ok(()) => doc.reloaded(cx),
                                    Err(e) => cx.emit(DocEvent::Error(format!("{e:#}"))),
                                });
                            }
                            if let Some(it) = &it {
                                crate::workspace::request_export(it.tab, it.part.iter().cloned().collect(), window, cx);
                            }
                        })
                    })
                    .child(os::secondary_button(SharedString::from(format!("rh-view-{k}")), "View release", cx).on_click(move |_, window, cx| {
                        window.close_dialog(cx);
                        open_review(d1.clone(), &pkg2, window, cx);
                    }))
                    // Help: once obsoleted, "the Obsolete revision button has
                    // become a View obsoletion button".
                    .when_some(obsoletion, |d, o| {
                        let d3 = doc.clone();
                        d.child(os::secondary_button(SharedString::from(format!("rh-viewobs-{k}")), "View obsoletion", cx).on_click(move |_, window, cx| {
                            window.close_dialog(cx);
                            open_review(d3.clone(), &o, window, cx);
                        }))
                    })
                    .when(e.obsoleted_by.is_none(), |d| {
                        let item = item.clone();
                        d.child(div().id(SharedString::from(format!("rh-obs-{k}"))).px(px(10.)).py(px(4.)).rounded(px(3.)).cursor_pointer().text_size(px(13.)).text_color(gpui_kit::rgb(0xffffff)).bg(gpui_kit::rgb(0xbd3039)).child("Obsolete revision").on_click(move |_, window, cx| {
                            window.close_dialog(cx);
                            if let Some(it) = item.clone() {
                                let name = format!("Obsolete {} {}", it.part_number, it.revision);
                                open(d2.clone(), Mode::Obsolete, vec![it], name, String::new(), window, cx);
                            }
                        }))
                    }),
            );
        }
        os::modal(dialog, "", 640.)
            .title(v_flex().child(div().text_size(px(16.)).font_weight(FontWeight::SEMIBOLD).child("Revision history")).child(div().text_size(px(12.)).text_color(pal.text_muted).child(part_number.clone())))
            .child(rows)
            .footer(os::modal_footer().child(os::modal_cancel(os::cancel_button("rh-close", "Close", cx))))
    });
}
