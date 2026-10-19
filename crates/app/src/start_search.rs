//! The documents page's advanced search (Onshape: the ▾ inside "Search in
//! <place>"; live documents/110–111, dom/documents-advanced-search.txt).
//!
//! A 360 px panel under the search box: Type, Name, Description | Part
//! number, State, Revision | Add criteria | Results from; ? help, Reset,
//! Search. Search applies a [`Query`] to the documents list: document
//! fields match the store metadata, the rest match what the documents hold
//! (tab types, part properties), read off the UI thread into [`DocFacts`].

use crate::os_menu::{OsMenu, OsMenuItem};
use crate::os_ui as os;
use crate::theme::p;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::document::TabContent;
use std::cell::Cell;
use std::path::Path;
use std::rc::Rc;

/// Type choices, in Onshape's order (live documents/111).
pub const TYPES: [&str; 9] = ["Document", "Part", "Part Studio", "Assembly", "Drawing", "Feature Studio", "Variable Studio", "Folder", "Material Library"];

/// Onshape's release states for the State field.
const STATES: [&str; 6] = noshape_model::partstudio::RELEASE_STATES;

/// "Add criteria" choices (live behaviours.md, advanced search).
const CRITERIA: [&str; 16] = [
    "Creator",
    "Published FeatureScript",
    "Vendor",
    "Project",
    "Product line",
    "Title 1",
    "Title 2",
    "Title 3",
    "Drawn by",
    "Approver",
    "Date drawn",
    "Date approved",
    "Not revision managed",
    "Exclude from all BOMs",
    "Revision description",
    "Library label",
];

/// Yes / no criteria (a checkbox instead of a text field).
fn is_flag(c: &str) -> bool {
    matches!(c, "Published FeatureScript" | "Not revision managed" | "Exclude from all BOMs")
}

const RESULTS_FROM: [&str; 3] = ["Workspaces", "Versions", "All"];
const RESULTS_AT: [&str; 2] = ["Latest", "All"];

/// An applied search.
#[derive(Clone, Default, PartialEq, Debug)]
pub struct Query {
    pub types: Vec<&'static str>,
    pub name: String,
    pub description: String,
    pub part_number: String,
    pub state: Option<&'static str>,
    pub revision: String,
    /// Added criteria: (criterion, text; "true" for flags).
    pub extra: Vec<(&'static str, String)>,
}

impl Query {
    /// Does the query need what's inside the documents?
    pub fn needs_facts(&self) -> bool {
        self.types.iter().any(|t| !matches!(*t, "Document" | "Folder")) || !self.part_number.is_empty() || self.state.is_some() || !self.revision.is_empty() || self.extra.iter().any(|(c, _)| *c != "Creator" && *c != "Library label")
    }

    /// Folders only (Type = Folder).
    pub fn folders_only(&self) -> bool {
        !self.types.is_empty() && self.types.iter().all(|t| *t == "Folder")
    }
}

/// What a document holds, for the content criteria.
#[derive(Clone, Default, Debug)]
pub struct DocFacts {
    /// Type names of its tabs (as [`TYPES`]), plus "Part" when it has parts.
    pub kinds: Vec<&'static str>,
    pub parts: Vec<noshape_model::partstudio::PartProperties>,
}

/// Read a document's facts (blocking; call off the UI thread).
pub fn facts(path: &Path) -> Option<DocFacts> {
    let store = noshape_store::Document::open_read_only(path).ok()?;
    let head = store.head(store.default_branch().ok()?).ok()?;
    let doc = noshape_model::document::Document::from_blobs(&store.read_all(head).ok()?).ok()?;
    let mut f = DocFacts::default();
    for t in &doc.tabs {
        let k = match &t.content {
            TabContent::PartStudio(ps) => {
                f.parts.extend(ps.part_properties.values().cloned());
                if !ps.part_properties.is_empty() || !ps.features.is_empty() {
                    f.kinds.push("Part");
                }
                "Part Studio"
            }
            TabContent::Assembly(_) => "Assembly",
            TabContent::Drawing(_) => "Drawing",
            TabContent::FeatureStudio(_) => "Feature Studio",
            TabContent::VariableStudio(_) => "Variable Studio",
            TabContent::MaterialLibrary(_) => "Material Library",
            TabContent::File(_) => continue,
        };
        f.kinds.push(k);
    }
    Some(f)
}

fn contains(hay: &str, needle: &str) -> bool {
    needle.trim().is_empty() || hay.to_lowercase().contains(&needle.trim().to_lowercase())
}

/// Does a document match? `labels`: its document labels; `facts` is read
/// when [`Query::needs_facts`].
pub fn matches(q: &Query, name: &str, description: &str, owner: &str, labels: &[String], facts: Option<&DocFacts>) -> bool {
    if q.folders_only() || !contains(name, &q.name) || !contains(description, &q.description) {
        return false;
    }
    let kinds: Vec<&str> = q.types.iter().copied().filter(|t| !matches!(*t, "Document" | "Folder")).collect();
    let needs = q.needs_facts();
    let f = match facts {
        Some(f) => Some(f),
        None if needs => return false,
        None => None,
    };
    if !kinds.is_empty() && !f.is_some_and(|f| kinds.iter().any(|k| f.kinds.contains(k))) {
        return false;
    }
    // Part criteria: one part must match them all.
    let part_crit: Vec<(&str, &str)> = q.extra.iter().filter(|(c, _)| !matches!(*c, "Creator" | "Library label" | "Published FeatureScript")).map(|(c, v)| (*c, v.as_str())).collect();
    if !q.part_number.is_empty() || q.state.is_some() || !q.revision.is_empty() || !part_crit.is_empty() {
        let part_ok = |pp: &noshape_model::partstudio::PartProperties| {
            let m = &pp.meta;
            contains(pp.part_number.as_deref().unwrap_or(""), &q.part_number)
                && (q.part_number.is_empty() || pp.part_number.is_some())
                && q.state.is_none_or(|s| m.state() == s)
                && contains(pp.revision.as_deref().unwrap_or(""), &q.revision)
                && (q.revision.is_empty() || pp.revision.is_some())
                && part_crit.iter().all(|(c, v)| {
                    let field = |o: &Option<String>| o.as_deref().is_some_and(|s| contains(s, v));
                    match *c {
                        "Vendor" => field(&m.vendor),
                        "Project" => field(&m.project),
                        "Product line" => field(&m.product_line),
                        "Title 1" => field(&m.title1),
                        "Title 2" => field(&m.title2),
                        "Title 3" => field(&m.title3),
                        "Not revision managed" => m.not_revision_managed,
                        "Exclude from all BOMs" => m.exclude_from_bom,
                        // Drawing and release fields: no part holds them.
                        _ => false,
                    }
                })
        };
        if !f.is_some_and(|f| f.parts.iter().any(part_ok)) {
            return false;
        }
    }
    q.extra.iter().all(|(c, v)| match *c {
        "Creator" => contains(owner, v),
        "Library label" => labels.iter().any(|l| contains(l, v)),
        "Published FeatureScript" => f.is_some_and(|f| f.kinds.contains(&"Feature Studio")),
        _ => true,
    })
}

/// Events to the documents page.
pub enum SearchEvent {
    Search(Query),
    Reset,
}

/// The panel's editing state.
pub struct AdvancedSearch {
    types: Vec<&'static str>,
    name: Entity<InputState>,
    description: Entity<InputState>,
    part_number: Entity<InputState>,
    revision: Entity<InputState>,
    state: Option<&'static str>,
    extra: Vec<(&'static str, Entity<InputState>, bool)>,
    results_from: usize,
    results_at: usize,
}

impl EventEmitter<SearchEvent> for AdvancedSearch {}

impl AdvancedSearch {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let field = |ph: &str, window: &mut Window, cx: &mut Context<Self>| {
            let ph = ph.to_string();
            cx.new(|cx| InputState::new(window, cx).placeholder(ph))
        };
        Self {
            types: Vec::new(),
            name: field("Enter name", window, cx),
            description: field("Enter description", window, cx),
            part_number: field("Enter part number", window, cx),
            revision: field("Enter revision", window, cx),
            state: None,
            extra: Vec::new(),
            results_from: 0,
            results_at: 0,
        }
    }

    fn query(&self, cx: &App) -> Query {
        let v = |s: &Entity<InputState>| s.read(cx).value().trim().to_string();
        Query {
            types: self.types.clone(),
            name: v(&self.name),
            description: v(&self.description),
            part_number: v(&self.part_number),
            state: self.state,
            revision: v(&self.revision),
            extra: self.extra.iter().filter_map(|(c, st, on)| if is_flag(c) { on.then(|| (*c, "true".to_string())) } else { Some((*c, v(st))).filter(|e| !e.1.is_empty()) }).collect(),
        }
    }

    fn reset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for st in [&self.name, &self.description, &self.part_number, &self.revision] {
            st.update(cx, |s, cx| s.set_value("", window, cx));
        }
        self.types.clear();
        self.state = None;
        self.extra.clear();
        self.results_from = 0;
        self.results_at = 0;
        cx.emit(SearchEvent::Reset);
        cx.notify();
    }

    /// Open a select list of `items` at the pointer; `pick` gets the index.
    fn pick_from(items: Vec<String>, current: Option<usize>, at: Point<Pixels>, view: Entity<Self>, pick: fn(&mut Self, usize, &mut Window, &mut Context<Self>), window: &mut Window, cx: &mut App) {
        let menu = OsMenu::build(window, cx, move |mut m, _, _| {
            m = m.select_list(238.);
            for (i, label) in items.iter().enumerate() {
                let v = view.clone();
                m = m.item(OsMenuItem::new(label.clone()).checked(current == Some(i)).on_click(move |_, w, cx| v.update(cx, |t, cx| {
                    pick(t, i, w, cx);
                    cx.notify();
                })));
            }
            m
        });
        crate::os_menu::open(at, menu, window, cx);
    }
}

/// 30 px field, 1 px `#ccc` border, radius 3, italic gray placeholder.
fn field_box(id: impl Into<ElementId>) -> Stateful<Div> {
    h_flex().id(id).w(px(238.)).h(px(30.)).px(px(4.)).border_1().border_color(rgb(0xcccccc)).rounded(px(3.)).bg(rgb(0xffffff)).text_size(px(12.))
}

impl Render for AdvancedSearch {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let view = cx.entity();
        let row = |label: &'static str, field: AnyElement| h_flex().h(px(40.)).px(px(4.)).child(div().w(px(100.)).flex_none().text_size(px(12.)).text_color(pal.text).child(label)).child(field);
        let input = |st: &Entity<InputState>| field_box(SharedString::from(format!("adv-{}", st.entity_id()))).child(Input::new(st).appearance(false).text_size(px(12.)).italic()).into_any_element();
        // Select lists hang under their field: each field records its bounds.
        let anchors: Vec<Rc<Cell<Bounds<Pixels>>>> = (0..5).map(|_| Rc::default()).collect();
        let under = |a: &Rc<Cell<Bounds<Pixels>>>| {
            let b = a.get();
            point(b.left(), b.bottom())
        };
        let select = |id: &'static str, text: String, placeholder: bool, anchor: &Rc<Cell<Bounds<Pixels>>>| {
            let probe = anchor.clone();
            field_box(id)
                .relative()
                .child(canvas(move |b, _, _| probe.set(b), |_, _, _, _| {}).absolute().top_0().left_0().size_full())
                .pl(px(8.))
                .cursor_pointer()
                .child(div().flex_1().min_w_0().truncate().when(placeholder, |d| d.italic().text_color(pal.text_faint)).child(text))
                .child(os::glyph(os::GLYPH_CARET, 9., rgb(0x666666)))
        };
        let types_text = if self.types.is_empty() { "All".to_string() } else { self.types.join(", ") };
        let v1 = view.clone();
        let a0 = anchors[0].clone();
        let type_field = select("adv-type", types_text, self.types.is_empty(), &anchors[0])
            .on_click(move |_: &ClickEvent, window, cx| {
                // Multiselect: each pick toggles a type.
                let cur = v1.read(cx).types.clone();
                let v = v1.clone();
                let menu = OsMenu::build(window, cx, move |mut m, _, _| {
                    m = m.select_list(238.);
                    for t in TYPES {
                        let v = v.clone();
                        m = m.item(OsMenuItem::new(t).checked(cur.contains(&t)).on_click(move |_, _, cx| v.update(cx, |s, cx| {
                            if let Some(i) = s.types.iter().position(|x| *x == t) {
                                s.types.remove(i);
                            } else {
                                s.types.push(t);
                            }
                            cx.notify();
                        })));
                    }
                    m
                });
                crate::os_menu::open(under(&a0), menu, window, cx);
            })
            .into_any_element();
        let v2 = view.clone();
        let state = self.state;
        let a1 = anchors[1].clone();
        let state_field = select("adv-state", state.unwrap_or("Any state").to_string(), state.is_none(), &anchors[1])
            .on_click(move |_: &ClickEvent, window, cx| {
                let items = std::iter::once("Any state".to_string()).chain(STATES.iter().map(|s| s.to_string())).collect();
                let cur = Some(state.and_then(|s| STATES.iter().position(|x| *x == s)).map_or(0, |i| i + 1));
                AdvancedSearch::pick_from(items, cur, under(&a1), v2.clone(), |t, i, _, _| t.state = if i == 0 { None } else { Some(STATES[i - 1]) }, window, cx)
            })
            .into_any_element();
        let v3 = view.clone();
        let a2 = anchors[2].clone();
        let add_field = select("adv-add", "Select...".into(), false, &anchors[2])
            .on_click(move |_: &ClickEvent, window, cx| {
                let have: Vec<&str> = v3.read(cx).extra.iter().map(|e| e.0).collect();
                let items: Vec<String> = CRITERIA.iter().filter(|c| !have.contains(c)).map(|c| c.to_string()).collect();
                AdvancedSearch::pick_from(
                    items,
                    None,
                    under(&a2),
                    v3.clone(),
                    |t, i, window, cx| {
                        let have: Vec<&str> = t.extra.iter().map(|e| e.0).collect();
                        if let Some(c) = CRITERIA.iter().copied().filter(|c| !have.contains(c)).nth(i) {
                            let st = cx.new(|cx| InputState::new(window, cx).placeholder(format!("Enter {}", c.to_lowercase())));
                            t.extra.push((c, st, true));
                        }
                    },
                    window,
                    cx,
                )
            })
            .into_any_element();
        let (from, at) = (self.results_from, self.results_at);
        let v4 = view.clone();
        let a3 = anchors[3].clone();
        let from_field = select("adv-from", RESULTS_FROM[from].into(), false, &anchors[3])
            .on_click(move |_: &ClickEvent, window, cx| AdvancedSearch::pick_from(RESULTS_FROM.iter().map(|s| s.to_string()).collect(), Some(from), under(&a3), v4.clone(), |t, i, _, _| t.results_from = i, window, cx))
            .into_any_element();
        let v5 = view.clone();
        let a4 = anchors[4].clone();
        let at_field = select("adv-at", RESULTS_AT[at].into(), false, &anchors[4])
            .on_click(move |_: &ClickEvent, window, cx| AdvancedSearch::pick_from(RESULTS_AT.iter().map(|s| s.to_string()).collect(), Some(at), under(&a4), v5.clone(), |t, i, _, _| t.results_at = i, window, cx))
            .into_any_element();
        let group = || v_flex().mx(px(6.)).pt(px(10.)).border_t_1().border_color(rgb(0xcccccc));
        let mut added = v_flex();
        for (i, (c, st, on)) in self.extra.iter().enumerate() {
            let c: &'static str = c;
            let field = if is_flag(c) {
                os::checkbox(SharedString::from(format!("adv-flag-{i}")), "", *on)
                    .on_click(cx.listener(move |t, _: &ClickEvent, _, cx| {
                        if let Some(e) = t.extra.get_mut(i) {
                            e.2 = !e.2;
                        }
                        cx.notify();
                    }))
                    .into_any_element()
            } else {
                input(st)
            };
            added = added.child(
                h_flex()
                    .child(row(c, field))
                    .child(div().id(SharedString::from(format!("adv-x-{i}"))).cursor_pointer().child(os::glyph(os::GLYPH_CLOSE, 12., pal.text_faint)).on_click(cx.listener(move |t, _: &ClickEvent, _, cx| {
                        if i < t.extra.len() {
                            t.extra.remove(i);
                        }
                        cx.notify();
                    }))),
            );
        }
        v_flex()
            .id("adv-search")
            .occlude()
            .w(px(360.))
            .bg(rgb(0xffffff))
            .border_1()
            .border_color(rgb(0xd7d7d7))
            .rounded(px(4.))
            .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(6.)), blur_radius: px(12.), spread_radius: px(0.), inset: false }])
            .pt(px(8.))
            .text_color(pal.text)
            .child(v_flex().mx(px(6.)).child(row("Type", type_field)).child(row("Name", input(&self.name))).child(row("Description", input(&self.description))))
            .child(group().child(row("Part number", input(&self.part_number))).child(row("State", state_field)).child(row("Revision", input(&self.revision))))
            .child(group().child(added).child(row("Add criteria", add_field)))
            .child(group().child(row("Results from", from_field)).child(row("", at_field)))
            .child(
                h_flex()
                    .p(px(10.))
                    .gap(px(3.))
                    .child(div().opacity(0.6).child(crate::assets::icon("help", 20., cx)))
                    .child(div().flex_1())
                    .child(os::secondary_button("adv-reset", "Reset", cx).on_click(cx.listener(|t, _: &ClickEvent, window, cx| t.reset(window, cx))))
                    .child(os::primary_button("adv-go", "Search").on_click(cx.listener(|t, _: &ClickEvent, _, cx| {
                        let q = t.query(cx);
                        cx.emit(SearchEvent::Search(q));
                    }))),
            )
    }
}
