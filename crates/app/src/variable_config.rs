//! Variable Studio configurations (help "Variable Studio Configurations",
//! images variabletables/config-*): the right edge's Configuration panel
//! button, the panel (the "automatically referenced" notice while "Insert
//! into all Part Studios and Assemblies" is on; "[▦ Configure Variable
//! Studio][▾]" and the illustration; then "Filter inputs", one table per
//! input — option names × configured cells, the shown configuration with a
//! blue bar — and "[Add configuration input][▲]"), the Value / Description
//! cells' Configure / Configure with / Set to / Unconfigure items, and the
//! configured cells' dotted outline. Model: `noshape_model::variables::
//! VsConfiguration`.

use super::*;
use gpui_kit::component::WindowExt as _;
use noshape_model::configuration::{ConfigInput, ConfigInputKind, ListOption, VariableKind};
use noshape_model::variables::{VsConfiguration, VsField};
use noshape_units::ExprValue;

/// The panel's width (live vs-914: x 1347–1919).
const PANEL_W: f32 = 572.;
const NAME_W: f32 = 140.;
const COL_W: f32 = 140.;
const ROW_H: f32 = 31.;
const GRID: u32 = 0xcccccc;
/// A configured cell's outline (help config-configured-values-01).
pub(super) const CONFIGURED: u32 = 0xfab601;

/// A configuration table cell being typed in.
pub(super) struct CfgEdit {
    key: String,
    input: Entity<InputState>,
    _sub: Subscription,
}

type Commit = std::rc::Rc<dyn Fn(&mut VsConfiguration, String)>;

fn new_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let n = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    format!("in{:x}", n & 0xffff_ffff_ffff)
}

/// A new list input's internal ID, as Onshape's "List_8iFesnytxapQpg".
fn list_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    const ALNUM: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let mut n = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0) ^ 0x9e37_79b9_7f4a_7c15;
    let mut out = String::from("List_");
    for _ in 0..14 {
        out.push(ALNUM[(n % 62) as usize] as char);
        n = (n / 62) ^ (n << 7);
    }
    out
}

/// A row's or a named input's internal ID: its name as a FeatureScript
/// identifier, unique among `taken` (live: row "Big" → ID "Big").
fn name_id(name: &str, taken: &[String]) -> String {
    noshape_model::configuration::fs_identifier(name, taken)
}

/// The dotted outline over a configured cell.
pub(super) fn configured_outline() -> Div {
    div().absolute().top(px(2.)).left(px(2.)).right(px(2.)).bottom(px(2.)).border_1().border_dashed().border_color(rgb(CONFIGURED))
}

/// A Value / Description cell's configuration items (live vs-906, 915–921):
/// "Configure" while the studio has one input, else "Configure with <input>"
/// per list / checkbox input and "Set to #<variable>" per configuration
/// variable of the value's type; a configured cell offers "Unconfigure
/// from <input>". Configuring shows "#L1 = 10 mm Value added to
/// Configuration".
pub(super) fn cell_menu(m: crate::os_menu::OsMenu, view: Entity<VariableStudioView>, cfg: &VsConfiguration, cell: (String, VsField, String, Option<VarKind>)) -> crate::os_menu::OsMenu {
    use crate::os_menu::OsMenuItem;
    // The cell: its variable, Value or Description, current text and type.
    let (variable, field, current, var_kind) = cell;
    if let Some(c) = cfg.configured_cell(&variable, field) {
        let name = cfg.inputs.iter().find(|i| i.id == c.input).map(|i| i.name.clone()).unwrap_or_default();
        return m.item(OsMenuItem::new(format!("Unconfigure from {name}")).on_click(move |_, _, cx| {
            view.update(cx, |t, cx| t.edit("Unconfigure variable", cx, |s| s.configuration.unconfigure(&variable, field)))
        }));
    }
    let what = if field == VsField::Value { "Value" } else { "Description" };
    let item = |label: String, input: &ConfigInput| {
        let (view, variable, current, id, input_name) = (view.clone(), variable.clone(), current.clone(), input.id.clone(), input.name.clone());
        OsMenuItem::new(label).on_click(move |_, _, cx| {
            let toast = format!("#{variable} = {current} {what} added to {input_name}");
            view.update(cx, |t, cx| {
                t.edit("Configure variable", cx, |s| s.configuration.configure(&id, &variable, field, &current));
                t.config_open = true;
                t.doc.update(cx, |_, cx| cx.emit(DocEvent::Info(toast)));
            })
        })
    };
    // A configuration variable sets a value of its own type.
    let fits = |k: VariableKind| match var_kind {
        Some(VarKind::Length) => k == VariableKind::Length,
        Some(VarKind::Angle) => k == VariableKind::Angle,
        Some(VarKind::Number) => matches!(k, VariableKind::Integer | VariableKind::Real),
        _ => true,
    };
    // The Configure items first, then Set to (live vs-921).
    let mut m = m;
    for input in &cfg.inputs {
        m = match &input.kind {
            ConfigInputKind::List { .. } | ConfigInputKind::Checkbox { .. } if cfg.inputs.len() == 1 => m.item(item("Configure".into(), input)),
            ConfigInputKind::List { .. } | ConfigInputKind::Checkbox { .. } => m.item(item(format!("Configure with {}", input.name), input)),
            _ => m,
        };
    }
    for input in &cfg.inputs {
        if let ConfigInputKind::Variable { kind, .. } = &input.kind
            && field == VsField::Value
            && fits(*kind)
        {
            m = m.item(item(format!("Set to #{}", input.name), input));
        }
    }
    m
}

impl VariableStudioView {
    /// The button at the right edge (open or not), centred on its height
    /// (live document/10 x 1888–1919; help config-panel-02).
    pub(super) fn render_config_tab(&self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        div()
            .id("vs-config-tab")
            .w(px(33.))
            .h(px(34.))
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui_kit::white())
            .border_1()
            .border_r_0()
            .border_color(rgb(0xaaaaaa))
            .cursor_pointer()
            .when(self.config_open, |d| d.bg(pal.panel_active))
            .hover(|d| d.bg(pal.panel_hover))
            .tooltip(crate::ui::tooltip("Configuration panel"))
            .child(crate::assets::icon("configuration", 20., cx))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.config_open = !this.config_open;
                cx.notify();
            }))
            .into_any_element()
    }

    fn cfg_edit(&mut self, desc: &str, cx: &mut Context<Self>, f: impl FnOnce(&mut VsConfiguration)) {
        self.edit(desc, cx, |s| f(&mut s.configuration));
    }

    /// Configure Variable Studio ▾ / Add configuration input ▲: List (made at
    /// once), Checkbox, Configuration variable.
    fn add_input(&mut self, kind: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        // Checkbox / Configuration variable ask for a name first (live
        // vs-911, 913).
        if kind != "List" {
            self.open_input_dialog(kind == "Variable", window, cx);
            return;
        }
        self.cfg_edit("Add configuration input", cx, |c| {
            let n = c.inputs.len() + 1;
            // Live ei-906: "List_…", the first row "Default".
            let id = list_id();
            c.inputs.push(ConfigInput {
                id,
                name: if n == 1 { "Configuration".into() } else { format!("List {n}") },
                kind: ConfigInputKind::List { options: vec![ListOption { id: "Default".into(), name: "Default".into() }], default: "Default".into() },
                visible_when: None,
                excluded: false,
            });
        });
        cx.notify();
    }

    fn kinds_menu(view: Entity<Self>, at: Point<Pixels>, anchor: Anchor, window: &mut Window, cx: &mut App) {
        let menu = crate::os_menu::OsMenu::build(window, cx, move |mut m, _, _| {
            for (label, icon, kind) in [("List", "cfg-filter-list", "List"), ("Checkbox", "cfg-filter-checkbox", "Checkbox"), ("Configuration variable", "cfg-filter-variable", "Variable")] {
                let v = view.clone();
                m = m.item(crate::os_menu::OsMenuItem::new(label).icon_name(icon).on_click(move |_, window, cx| v.update(cx, |this, cx| this.add_input(kind, window, cx))));
            }
            // Live vs-903 / 909: Paste inputs (copied with Copy input).
            let v = view.clone();
            m.item(crate::os_menu::OsMenuItem::new("Paste inputs").icon_name("paste").disabled(crate::config_visibility::copied_inputs().is_empty()).on_click(move |_, _, cx| v.update(cx, |this, cx| this.paste_inputs(cx))))
        });
        crate::os_menu::open_anchored(at, anchor, menu, window, cx);
    }

    /// A text cell of the configuration tables: double-click edits it.
    fn cfg_text(&mut self, key: String, text: String, style: (f32, FontWeight, Hsla), commit: Commit, cx: &mut Context<Self>) -> AnyElement {
        let (size, weight, color) = style;
        if let Some(e) = self.cfg_cell.as_ref().filter(|e| e.key == key) {
            return div().flex_1().min_w_0().child(Input::new(&e.input).appearance(false).text_size(px(size)).font_weight(weight).pl(px(3.))).into_any_element();
        }
        div()
            .id(SharedString::from(format!("vcfg-{key}")))
            .flex_1()
            .min_w_0()
            .truncate()
            .text_size(px(size))
            .font_weight(weight)
            .text_color(color)
            .child(text.clone())
            .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
                if e.click_count() >= 2 {
                    this.start_cfg_text(key.clone(), text.clone(), commit.clone(), window, cx);
                }
            }))
            .into_any_element()
    }

    fn start_cfg_text(&mut self, key: String, current: String, commit: Commit, window: &mut Window, cx: &mut Context<Self>) {
        let input = cx.new(|cx| InputState::new(window, cx).default_value(current));
        input.update(cx, |s, cx| {
            s.focus(window, cx);
            s.select_all(window, cx);
        });
        let k = key.clone();
        let sub = cx.subscribe_in(&input, window, move |this: &mut Self, st, ev: &InputEvent, _, cx| {
            if matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Blur) && this.cfg_cell.as_ref().is_some_and(|e| e.key == k) {
                let text = st.read(cx).value().to_string();
                this.cfg_cell = None;
                let commit = commit.clone();
                this.cfg_edit("Edit configuration", cx, move |c| commit(c, text));
                cx.notify();
            }
        });
        self.cfg_cell = Some(CfgEdit { key, input, _sub: sub });
        cx.notify();
    }

    /// Copy table: the input's table as tab-separated text (header rows,
    /// then a row per option).
    pub(super) fn config_table_text(&self, input: &str, cx: &App) -> String {
        let Some(cfg) = self.studio(cx).map(|s| s.configuration.clone()) else { return String::new() };
        let cols: Vec<&noshape_model::variables::VsConfigured> = cfg.configured.iter().filter(|c| c.input == input).collect();
        let mut out = String::from("Name");
        for c in &cols {
            out.push_str(&format!("\t#{} {}", c.variable, if c.field == VsField::Value { "Value" } else { "Description" }));
        }
        let names: Vec<(String, String)> = match cfg.inputs.iter().find(|i| i.id == input).map(|i| &i.kind) {
            Some(ConfigInputKind::List { options, .. }) => options.iter().map(|o| (o.id.clone(), o.name.clone())).collect(),
            Some(ConfigInputKind::Checkbox { .. }) => vec![("false".into(), "false".into()), ("true".into(), "true".into())],
            _ => Vec::new(),
        };
        for (key, name) in names {
            out.push('\n');
            out.push_str(&name);
            for c in &cols {
                out.push('\t');
                out.push_str(c.values.get(&key).map(String::as_str).unwrap_or(""));
            }
        }
        out
    }

    /// Visibility conditions… (the Part Studio's dialog, on this studio's
    /// inputs).
    fn open_visibility(&mut self, input: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(cfg) = self.studio(cx).map(|s| s.configuration.clone()) else { return };
        let config = noshape_model::configuration::Configuration { inputs: cfg.inputs.clone(), configured: Vec::new() };
        let view = cx.entity();
        crate::config_visibility::open(
            config,
            cfg.active.clone(),
            input,
            move |saved, _, cx| {
                view.update(cx, |t, cx| t.cfg_edit("Edit visibility conditions", cx, |c| c.inputs = saved.inputs.clone()));
            },
            window,
            cx,
        );
    }

    /// Paste inputs (copied with Copy input, here or in a Part Studio).
    fn paste_inputs(&mut self, cx: &mut Context<Self>) {
        let mut inputs = crate::config_visibility::copied_inputs();
        if inputs.is_empty() {
            return;
        }
        self.cfg_edit("Paste configuration inputs", cx, move |c| {
            for i in &mut inputs {
                if c.inputs.iter().any(|x| x.id == i.id) {
                    i.id = new_id();
                }
            }
            c.inputs.append(&mut inputs);
        });
        cx.notify();
    }

    /// A configuration table row's menu (live vs-918): Switch to "<row>",
    /// Set as default, Duplicate row, Move up / down, Delete row, and on a
    /// configured column's cell Delete column.
    fn table_menu(m: crate::os_menu::OsMenu, view: Entity<Self>, input: String, key: String, label: String, column: Option<(String, VsField)>, list: bool) -> crate::os_menu::OsMenu {
        use crate::os_menu::OsMenuItem;
        let act = |label: &'static str, f: Box<dyn Fn(&mut VsConfiguration)>| {
            let view = view.clone();
            let f = std::rc::Rc::new(f);
            OsMenuItem::new(label).on_click(move |_, _, cx| {
                let f = f.clone();
                view.update(cx, |t, cx| t.cfg_edit(label, cx, move |c| f(c)))
            })
        };
        let (i1, k1) = (input.clone(), key.clone());
        let mut m = m.item({
            let view = view.clone();
            OsMenuItem::new(format!("Switch to \"{label}\"")).on_click(move |_, _, cx| {
                let (i, k) = (i1.clone(), k1.clone());
                view.update(cx, |t, cx| t.cfg_edit("Change configuration", cx, move |c| {
                    c.active.0.insert(i, k);
                }))
            })
        });
        if list {
            let (i2, k2) = (input.clone(), key.clone());
            m = m.item(act("Set as default", Box::new(move |c| {
                if let Some(ConfigInputKind::List { default, .. }) = c.inputs.iter_mut().find(|x| x.id == i2).map(|x| &mut x.kind) {
                    *default = k2.clone();
                }
            })));
            let (i3, k3) = (input.clone(), key.clone());
            m = m.item(act("Duplicate row", Box::new(move |c| {
                let mut oid = String::new();
                if let Some(ConfigInputKind::List { options, .. }) = c.inputs.iter_mut().find(|x| x.id == i3).map(|x| &mut x.kind)
                    && let Some(at) = options.iter().position(|o| o.id == k3)
                {
                    let name = format!("{} copy", options[at].name);
                    let taken: Vec<String> = options.iter().map(|o| o.id.clone()).collect();
                    oid = name_id(&name, &taken);
                    options.insert(at + 1, ListOption { id: oid.clone(), name });
                }
                if oid.is_empty() {
                    return;
                }
                for x in c.configured.iter_mut().filter(|x| x.input == i3) {
                    let v = x.values.get(&k3).cloned().unwrap_or_default();
                    x.values.insert(oid.clone(), v);
                }
            })));
            for (label, up) in [("Move up", true), ("Move down", false)] {
                let (i4, k4) = (input.clone(), key.clone());
                m = m.item(act(label, Box::new(move |c| {
                    if let Some(ConfigInputKind::List { options, .. }) = c.inputs.iter_mut().find(|x| x.id == i4).map(|x| &mut x.kind)
                        && let Some(at) = options.iter().position(|o| o.id == k4)
                        && let Some(to) = if up { at.checked_sub(1) } else { (at + 1 < options.len()).then_some(at + 1) }
                    {
                        options.swap(at, to);
                    }
                })));
            }
            let (i5, k5) = (input.clone(), key.clone());
            m = m.item(act("Delete row", Box::new(move |c| {
                if let Some(ConfigInputKind::List { options, default }) = c.inputs.iter_mut().find(|x| x.id == i5).map(|x| &mut x.kind)
                    && options.len() > 1
                {
                    options.retain(|o| o.id != k5);
                    if *default == k5 {
                        *default = options[0].id.clone();
                    }
                }
                for x in c.configured.iter_mut() {
                    x.values.remove(&k5);
                }
                if c.active.0.get(&i5) == Some(&k5) {
                    c.active.0.remove(&i5);
                }
            })));
        }
        if let Some((var, field)) = column {
            m = m.item(act("Delete column", Box::new(move |c| c.unconfigure(&var, field))));
        }
        m
    }

    /// One input: its header (⌄ name, "Default: …", ⋯) and table.
    fn render_input(&mut self, cfg: &VsConfiguration, input: &ConfigInput, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let iid = input.id.clone();
        let folded = self.cfg_folded.contains(&iid);
        let default_text = match &input.kind {
            ConfigInputKind::List { options, default } => options.iter().find(|o| &o.id == default).map(|o| o.name.clone()).unwrap_or_default(),
            ConfigInputKind::Checkbox { default } => default.to_string(),
            ConfigInputKind::Variable { default, .. } => default.text.clone(),
            ConfigInputKind::Text { default } => default.clone(),
        };
        let rename: Commit = {
            let id = iid.clone();
            std::rc::Rc::new(move |c: &mut VsConfiguration, v: String| {
                if let Some(i) = c.inputs.iter_mut().find(|i| i.id == id) {
                    i.name = v.trim().to_string();
                }
            })
        };
        let name_cell = self.cfg_text(format!("in-{iid}"), input.name.clone(), (16., FontWeight::MEDIUM, pal.text.into()), rename, cx);
        let view = cx.entity();
        let (i1, i2) = (iid.clone(), iid.clone());
        let at = cfg.inputs.iter().position(|i| i.id == iid).unwrap_or(0);
        let count = cfg.inputs.len();
        let list = matches!(input.kind, ConfigInputKind::List { .. });
        let has_ids = matches!(input.kind, ConfigInputKind::List { .. } | ConfigInputKind::Checkbox { .. });
        let header = h_flex()
            .h(px(35.))
            .flex_none()
            .px(px(6.))
            .gap(px(4.))
            .bg(rgb(0xeaeaea))
            .border_b_1()
            .border_color(rgb(GRID))
            .child(
                div()
                    .id(SharedString::from(format!("vcfg-fold-{iid}")))
                    .w(px(12.))
                    .cursor_pointer()
                    .child(crate::ui::glyph(if folded { "ui-chevron-right" } else { "ui-chevron-down" }, 10., pal.text))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if !this.cfg_folded.remove(&i1) {
                            this.cfg_folded.insert(i1.clone());
                        }
                        cx.notify();
                    })),
            )
            .child(div().flex_none().max_w(px(180.)).child(name_cell))
            // A configuration variable: "CV (#CV)" (live vs-914).
            .when(matches!(input.kind, ConfigInputKind::Variable { .. }), |d| d.child(div().flex_none().text_size(px(14.67)).font_weight(FontWeight::MEDIUM).text_color(pal.text).child(format!("(#{})", input.name))))
            .child(div().flex_1().min_w_0().truncate().text_size(px(14.67)).font_weight(FontWeight::MEDIUM).text_color(rgb(0x999999)).child(format!("Default: {default_text}")))
            // "Usages ①" once a value is set to it (live vs-922).
            .when(matches!(input.kind, ConfigInputKind::Variable { .. }) && cfg.configured.iter().any(|c| c.input == iid), |d| {
                let n = cfg.configured.iter().filter(|c| c.input == iid).count();
                d.child(h_flex().flex_none().gap(px(4.)).text_size(px(13.)).text_color(pal.text).child("Usages").child(div().min_w(px(16.)).h(px(16.)).px(px(4.)).rounded_full().bg(rgb(0x1651b0)).flex().items_center().justify_center().text_size(px(11.)).text_color(gpui_kit::white()).child(n.to_string())))
            })
            .child(
                div()
                    .id(SharedString::from(format!("vcfg-more-{iid}")))
                    .size(px(25.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .child(crate::assets::icon("more", 20., cx))
                    .on_click(move |e: &ClickEvent, window, cx| {
                        let (v, id) = (view.clone(), i2.clone());
                        // Live ei-900…905: Copy table, Copy input, Move up / down,
                        // Rename, Edit internal IDs…, Visibility conditions…, Input
                        // propagation…, Delete.
                        let menu = crate::os_menu::OsMenu::build(window, cx, move |m, _, _| {
                            use crate::os_menu::OsMenuItem;
                            let (v1, v2, v3, v4, v5, v6, v7) = (v.clone(), v.clone(), v.clone(), v.clone(), v.clone(), v.clone(), v.clone());
                            let (id1, id2, id3, id4, id5, id6) = (id.clone(), id.clone(), id.clone(), id.clone(), id.clone(), id.clone());
                            let moves = [("Move up", at > 0, true), ("Move down", at + 1 < count, false)];
                            let mut m = m.item(OsMenuItem::new("Copy table").on_click(move |_, _, cx| {
                                let text = v1.read(cx).config_table_text(&id1, cx);
                                cx.write_to_clipboard(ClipboardItem::new_string(text));
                            }))
                            .item(OsMenuItem::new("Copy input").on_click(move |_, _, cx| {
                                if let Some(i) = v2.read(cx).studio(cx).and_then(|s| s.configuration.inputs.iter().find(|i| i.id == id2).cloned()) {
                                    crate::config_visibility::copy_inputs(vec![i]);
                                }
                            }));
                            for (label, show, up) in moves {
                                if !show {
                                    continue;
                                }
                                let (v, id) = (v.clone(), id.clone());
                                m = m.item(OsMenuItem::new(label).on_click(move |_, _, cx| {
                                    let id = id.clone();
                                    v.update(cx, |t, cx| {
                                        t.cfg_edit(label, cx, move |c| {
                                            let n = c.inputs.len();
                                            if let Some(at) = c.inputs.iter().position(|i| i.id == id)
                                                && let Some(to) = if up { at.checked_sub(1) } else { (at + 1 < n).then_some(at + 1) }
                                            {
                                                c.inputs.swap(at, to);
                                            }
                                        })
                                    })
                                }));
                            }
                            let m = m.item(OsMenuItem::new("Rename").on_click(move |_, window, cx| {
                                v3.update(cx, |t, cx| {
                                    let name = t.studio(cx).and_then(|s| s.configuration.inputs.iter().find(|i| i.id == id3).map(|i| i.name.clone())).unwrap_or_default();
                                    let id = id3.clone();
                                    let commit: Commit = std::rc::Rc::new(move |c: &mut VsConfiguration, v: String| {
                                        if let Some(i) = c.inputs.iter_mut().find(|i| i.id == id) {
                                            i.name = v.trim().to_string();
                                        }
                                    });
                                    t.start_cfg_text(format!("in-{id3}"), name, commit, window, cx);
                                })
                            }));
                            let m = if has_ids { m.item(OsMenuItem::new(if list { "Edit internal IDs…" } else { "Edit internal ID…" }).on_click(move |_, window, cx| v6.update(cx, |t, cx| t.open_internal_ids(&id6, window, cx)))) } else { m };
                            m.item(OsMenuItem::new("Visibility conditions…").on_click(move |_, window, cx| v4.update(cx, |t, cx| t.open_visibility(Some(id4.clone()), window, cx))))
                            // Live ei-910: a Variable Studio has nothing that references
                            // inputs: a toast, no dialog.
                            .item(OsMenuItem::new("Input propagation…").on_click(move |_, _, cx| {
                                v7.update(cx, |t, cx| t.doc.update(cx, |_, cx| cx.emit(DocEvent::Info("No references in the tab.".into()))))
                            }))
                            .separator()
                            .item(OsMenuItem::new("Delete").on_click(move |_, _, cx| {
                                v5.update(cx, |t, cx| {
                                    t.cfg_edit("Delete configuration input", cx, |c| {
                                        c.inputs.retain(|i| i.id != id5);
                                        c.configured.retain(|x| x.input != id5);
                                        c.active.0.remove(&id5);
                                    })
                                })
                            }))
                        });
                        crate::os_menu::open(e.position(), menu, window, cx);
                    }),
            );
        let mut section = v_flex().border_1().border_color(rgb(GRID)).child(header);
        if folded {
            return section.into_any_element();
        }
        // The rows: list options or the checkbox states.
        let rows: Vec<(String, String)> = match &input.kind {
            ConfigInputKind::List { options, .. } => options.iter().map(|o| (o.id.clone(), o.name.clone())).collect(),
            // Live vs-914: unchecked, then checked.
            ConfigInputKind::Checkbox { .. } => vec![("false".into(), String::new()), ("true".into(), String::new())],
            _ => Vec::new(),
        };
        let columns: Vec<noshape_model::variables::VsConfigured> = cfg.configured.iter().filter(|c| c.input == iid).cloned().collect();
        let input_kind = input.kind.clone();
        let checkbox = matches!(input.kind, ConfigInputKind::Checkbox { .. });
        let notice = |second: bool| {
            v_flex()
                .m(px(6.))
                .px(px(14.))
                .py(px(10.))
                .rounded(px(3.))
                .bg(pal.info_fill)
                .border_1()
                .border_color(pal.info_outline)
                .text_size(px(14.))
                .text_color(pal.info_text)
                .child(div().font_weight(FontWeight::BOLD).child("No variables are configured"))
                .when(second, |d| d.child("Right click a variable value or description to configure them."))
        };
        if rows.is_empty() {
            // A configuration variable: no table (live vs-914); the notice
            // until a value is set to it.
            return section.when(columns.is_empty(), |d| d.child(notice(false))).into_any_element();
        }
        let shown = cfg.value_of(&cfg.active, &iid).unwrap_or_default();
        let cell = |w: f32| h_flex().w(px(w)).flex_none().h(px(ROW_H)).px(px(6.)).border_r_1().border_b_1().border_color(rgb(GRID));
        // Header: "Name" over both rows, each column "#L1" over "Value" /
        // "Description" (help config-configured-values-01).
        let head_cell = |w: f32, h: f32| h_flex().w(px(w)).h(px(h)).flex_none().px(px(6.)).justify_center().bg(rgb(0xeaeaea)).border_r_1().border_b_1().border_color(rgb(GRID)).text_size(px(13.33)).font_weight(FontWeight::BOLD).text_color(pal.text);
        let head_h = if columns.is_empty() { ROW_H } else { 2. * ROW_H };
        let head = h_flex().child(head_cell(NAME_W, head_h).child(if checkbox { "Value" } else { "Name" })).children(columns.iter().map(|c| {
            v_flex().child(head_cell(COL_W, ROW_H).child(format!("#{}", c.variable))).child(head_cell(COL_W, ROW_H).child(if c.field == VsField::Value { "Value" } else { "Description" }))
        }));
        let mut table = v_flex().child(head);
        for (key, label) in &rows {
            let on = *key == shown;
            let (k1, id1) = (key.clone(), iid.clone());
            let rename: Commit = {
                let (id, key) = (iid.clone(), key.clone());
                std::rc::Rc::new(move |c: &mut VsConfiguration, v: String| {
                    if let Some(ConfigInputKind::List { options, .. }) = c.inputs.iter_mut().find(|i| i.id == id).map(|i| &mut i.kind)
                        && let Some(o) = options.iter_mut().find(|o| o.id == key)
                    {
                        o.name = v.trim().to_string();
                    }
                })
            };
            let blue: Hsla = rgb(0x1651b0).into();
            let name = if matches!(input.kind, ConfigInputKind::List { .. }) {
                self.cfg_text(format!("opt-{iid}-{key}"), label.clone(), (13.33, if on { FontWeight::BOLD } else { FontWeight::NORMAL }, if on { blue } else { pal.text.into() }), rename, cx)
            } else {
                // A checkbox row: its state as a greyed box (live vs-914).
                let box_ = if key == "true" {
                    div().opacity(0.5).child(crate::assets::icon("checked-checkbox", 14., cx)).into_any_element()
                } else {
                    div().size(px(12.)).rounded(px(2.)).bg(rgb(0xb0b0b0)).into_any_element()
                };
                div().flex().items_center().child(box_).into_any_element()
            };
            let mut row = h_flex().child(
                cell(NAME_W)
                    .id(SharedString::from(format!("vcfg-row-{iid}-{key}")))
                    .relative()
                    .cursor_pointer()
                    .when(on, |d| d.child(div().absolute().left_0().top_0().bottom_0().w(px(3.)).bg(blue)))
                    .child(name)
                    // Clicking a row shows that configuration in the studio.
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        let (id, k) = (id1.clone(), k1.clone());
                        this.cfg_edit("Change configuration", cx, move |c| {
                            c.active.0.insert(id, k);
                        });
                    }))
                    .os_context_menu({
                        let (view, input, key, label) = (cx.entity(), iid.clone(), key.clone(), if label.is_empty() { key.clone() } else { label.clone() });
                        let list = matches!(input_kind, ConfigInputKind::List { .. });
                        move |m, _, _| Self::table_menu(m, view.clone(), input.clone(), key.clone(), label.clone(), None, list)
                    }),
            );
            for c in &columns {
                let text = c.values.get(key).cloned().unwrap_or_default();
                let commit: Commit = {
                    let (var, field, key) = (c.variable.clone(), c.field, key.clone());
                    std::rc::Rc::new(move |cf: &mut VsConfiguration, v: String| {
                        if let Some(x) = cf.configured.iter_mut().find(|x| x.variable == var && x.field == field) {
                            x.values.insert(key.clone(), v.trim().to_string());
                        }
                    })
                };
                let t = self.cfg_text(format!("val-{iid}-{key}-{}-{:?}", c.variable, c.field), text, (13.33, if on { FontWeight::BOLD } else { FontWeight::NORMAL }, pal.text.into()), commit, cx);
                let cell_menu = {
                    let (view, input, key, label, col) = (cx.entity(), iid.clone(), key.clone(), if label.is_empty() { key.clone() } else { label.clone() }, (c.variable.clone(), c.field));
                    let list = matches!(input_kind, ConfigInputKind::List { .. });
                    move |m, _: &mut Window, _: &mut Context<crate::os_menu::OsMenu>| Self::table_menu(m, view.clone(), input.clone(), key.clone(), label.clone(), Some(col.clone()), list)
                };
                row = row.child(cell(COL_W).id(SharedString::from(format!("vcfg-cell-{iid}-{key}-{}", c.variable))).child(t).os_context_menu(cell_menu));
            }
            table = table.child(row);
        }
        // The italic "Name" row adds an option (lists).
        if matches!(input.kind, ConfigInputKind::List { .. }) {
            let add: Commit = {
                let id = iid.clone();
                std::rc::Rc::new(move |c: &mut VsConfiguration, v: String| {
                    let name = v.trim().to_string();
                    if name.is_empty() {
                        return;
                    }
                    let taken: Vec<String> = match c.inputs.iter().find(|i| i.id == id).map(|i| &i.kind) {
                        Some(ConfigInputKind::List { options, .. }) => options.iter().map(|o| o.id.clone()).collect(),
                        _ => Vec::new(),
                    };
                    let oid = name_id(&name, &taken);
                    // New rows start from the default row's values.
                    let default = match c.inputs.iter().find(|i| i.id == id).map(|i| &i.kind) {
                        Some(ConfigInputKind::List { default, .. }) => default.clone(),
                        _ => String::new(),
                    };
                    if let Some(ConfigInputKind::List { options, .. }) = c.inputs.iter_mut().find(|i| i.id == id).map(|i| &mut i.kind) {
                        options.push(ListOption { id: oid.clone(), name });
                    }
                    for x in c.configured.iter_mut().filter(|x| x.input == id) {
                        let v = x.values.get(&default).cloned().unwrap_or_default();
                        x.values.insert(oid.clone(), v);
                    }
                })
            };
            let key = format!("add-{iid}");
            let add_cell = match self.cfg_cell.as_ref().filter(|e| e.key == key) {
                Some(e) => div().flex_1().child(Input::new(&e.input).appearance(false).text_size(px(13.33)).pl(px(3.))).into_any_element(),
                None => div()
                    .id(SharedString::from(format!("vcfg-{key}")))
                    .size_full()
                    .flex()
                    .items_center()
                    .italic()
                    .text_size(px(13.33))
                    .text_color(rgb(0x999999))
                    .cursor_text()
                    .child("Name")
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.start_cfg_text(key.clone(), String::new(), add.clone(), window, cx)))
                    .into_any_element(),
            };
            table = table.child(h_flex().child(cell(NAME_W).child(add_cell)));
        }
        section = section.child(div().id(SharedString::from(format!("vcfg-table-{iid}"))).overflow_x_scroll().child(table));
        if columns.is_empty() {
            // Live vs-914: a checkbox's notice is the title only.
            section = section.child(notice(!checkbox));
        }
        section.into_any_element()
    }

    /// The Configuration panel (help config-panel-02, config-*-01).
    pub(super) fn render_config_panel(&mut self, studio: &VariableStudio, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let border = rgb(0xaaaaaa);
        let view = cx.entity();
        let header = div()
            .h(px(30.))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .border_b_2()
            .border_color(rgb(0x1651b0))
            .text_size(px(13.33))
            .font_weight(FontWeight::MEDIUM)
            .text_color(rgb(0x1651b0))
            .child("Configurations");
        let link = |id: &'static str, t: &'static str| {
            div()
                .id(id)
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(0x094174))
                .underline()
                .cursor_pointer()
                .child(t)
                .on_click(|_, _, cx| cx.open_url("https://cad.onshape.com/help/Content/VariableStudio/variable_studio_configurations.htm"))
        };
        let empty_art = v_flex()
            .items_center()
            .child(div().mt(px(90.)).child(crate::assets::icon("config-illustration", 160., cx)))
            .child(
                v_flex()
                    .mt(px(18.))
                    .items_center()
                    .text_size(px(14.))
                    .line_height(px(21.))
                    .text_color(rgb(0x666666))
                    .child("Add configurations to create different variants of the values")
                    .child(h_flex().child("produced by this Variable Studio.\u{a0}").child(link("vcfg-help-1", "Learn more about Variable")))
                    .child(link("vcfg-help-2", "Studio configurations.")),
            );
        let body: AnyElement = if studio.insert_into_all && studio.configuration.is_empty() {
            // Help config-panel-02: configuring needs "Insert into all" off.
            v_flex()
                .child(
                    v_flex()
                        .m(px(6.))
                        .px(px(14.))
                        .py(px(12.))
                        .gap(px(12.))
                        .rounded(px(3.))
                        .bg(pal.info_fill)
                        .border_1()
                        .border_color(pal.info_outline)
                        .text_size(px(14.))
                        .text_color(pal.info_text)
                        .child(div().font_weight(FontWeight::BOLD).child("This Variable Studio is automatically referenced in this workspace"))
                        .child("To add configurations, first disable the \"Insert into all Part Studios and Assemblies\" option. This may affect regeneration."),
                )
                .child(empty_art)
                .into_any_element()
        } else if studio.configuration.is_empty() {
            let main = div()
                .id("vcfg-configure")
                .h(px(33.))
                .px(px(10.5))
                .flex()
                .items_center()
                .gap(px(6.))
                .cursor_pointer()
                .hover(|d| d.bg(pal.panel_hover))
                .child(crate::assets::icon("cfg-filter-list", 20., cx))
                .child(div().text_size(px(14.)).text_color(pal.text).child("Configure Variable Studio"))
                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.add_input("List", window, cx)));
            let v = view.clone();
            let caret = div()
                .id("vcfg-configure-more")
                .w(px(26.))
                .h(px(33.))
                .flex()
                .items_center()
                .justify_center()
                .border_l_1()
                .border_color(border)
                .cursor_pointer()
                .hover(|d| d.bg(pal.panel_hover))
                .child(crate::ui::glyph("drawing-tool-caret", 12., pal.text))
                .on_click(move |e: &ClickEvent, window, cx| Self::kinds_menu(v.clone(), e.position(), Anchor::TopLeft, window, cx));
            v_flex()
                .items_center()
                .pt(px(8.))
                .child(h_flex().rounded(px(5.25)).border_1().border_color(border).bg(gpui_kit::white()).child(main).child(caret))
                .child(empty_art)
                .into_any_element()
        } else {
            // Filter inputs (by name) and the three type toggles.
            let filter = self.cfg_filter.get_or_insert_with(|| cx.new(|cx| InputState::new(window, cx).placeholder("Filter inputs"))).clone();
            let q = filter.read(cx).value().to_lowercase();
            let toggle = |id: &'static str, icon: &'static str, kind: u8, this: &Self, cx: &mut Context<Self>| {
                let on = this.cfg_kinds & (1 << kind) != 0;
                div()
                    .id(id)
                    .size(px(26.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(3.))
                    .cursor_pointer()
                    .when(on, |d| d.bg(rgb(0xeaeaea)))
                    .hover(|d| d.bg(pal.panel_hover))
                    .child(crate::assets::icon(icon, 20., cx))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.cfg_kinds ^= 1 << kind;
                        cx.notify();
                    }))
            };
            let tools = h_flex()
                .h(px(36.))
                .flex_none()
                .px(px(6.))
                .gap(px(4.))
                .child(crate::ui::glyph("filter", 16., pal.text))
                .child(div().flex_1().child(crate::os_ui::input_h(Input::new(&filter).text_size(px(13.)), 26.)))
                .child(toggle("vcfg-k-list", "cfg-filter-list", 0, self, cx))
                .child(toggle("vcfg-k-check", "cfg-filter-checkbox", 1, self, cx))
                .child(toggle("vcfg-k-var", "cfg-filter-variable", 2, self, cx))
                // ⋯ (help config-rmb-configure-01): Collapse all / Expand all.
                .child(
                    div()
                        .id("vcfg-more")
                        .size(px(25.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .child(crate::assets::icon("more", 20., cx))
                        .on_click(cx.listener(|this, e: &ClickEvent, window, cx| {
                            let ids: Vec<String> = this.studio(cx).map(|s| s.configuration.inputs.iter().map(|i| i.id.clone()).collect()).unwrap_or_default();
                            let view = cx.entity();
                            let menu = crate::os_menu::OsMenu::build(window, cx, move |m, _, _| {
                                let (v1, v2, ids1) = (view.clone(), view.clone(), ids.clone());
                                m.item(crate::os_menu::OsMenuItem::new("Collapse all").on_click(move |_, _, cx| {
                                    v1.update(cx, |t, cx| {
                                        t.cfg_folded.extend(ids1.iter().cloned());
                                        cx.notify();
                                    })
                                }))
                                .item(crate::os_menu::OsMenuItem::new("Expand all").on_click(move |_, _, cx| {
                                    v2.update(cx, |t, cx| {
                                        t.cfg_folded.clear();
                                        cx.notify();
                                    })
                                }))
                            });
                            crate::os_menu::open(e.position(), menu, window, cx);
                        })),
                );
            let kinds = self.cfg_kinds;
            let cfg = studio.configuration.clone();
            let inputs: Vec<ConfigInput> = cfg
                .inputs
                .iter()
                .filter(|i| q.is_empty() || i.name.to_lowercase().contains(&q))
                .filter(|i| {
                    let k = match i.kind {
                        ConfigInputKind::List { .. } => 0,
                        ConfigInputKind::Checkbox { .. } => 1,
                        _ => 2,
                    };
                    kinds == 0 || kinds & (1 << k) != 0
                })
                .cloned()
                .collect();
            let sections: Vec<AnyElement> = inputs.iter().map(|i| self.render_input(&cfg, i, cx)).collect();
            v_flex().size_full().child(tools).child(v_flex().id("vcfg-inputs").flex_1().min_h_0().overflow_y_scroll().px(px(6.)).gap(px(10.)).children(sections)).into_any_element()
        };
        // Footer: [Add configuration input][▲] once configured; ? at the right.
        let add = (!studio.configuration.is_empty()).then(|| {
            let main = div()
                .id("vcfg-add-input")
                .h(px(28.))
                .px(px(10.))
                .flex()
                .items_center()
                .gap(px(6.))
                .cursor_pointer()
                .hover(|d| d.bg(pal.panel_hover))
                .child(crate::assets::icon("cfg-filter-list", 20., cx))
                .child(div().text_size(px(14.)).text_color(pal.text).child("Add configuration input"))
                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.add_input("List", window, cx)));
            let v = view.clone();
            let caret = div()
                .id("vcfg-add-input-more")
                .w(px(22.))
                .h(px(28.))
                .flex()
                .items_center()
                .justify_center()
                .border_l_1()
                .border_color(border)
                .cursor_pointer()
                .hover(|d| d.bg(pal.panel_hover))
                .child(crate::ui::glyph("ui-caret-up", 10., pal.text))
                .on_click(move |e: &ClickEvent, window, cx| Self::kinds_menu(v.clone(), e.position(), Anchor::BottomLeft, window, cx));
            h_flex().rounded(px(3.)).border_1().border_color(border).bg(pal.panel).child(main).child(caret)
        });
        let footer = h_flex()
            .h(px(40.))
            .flex_none()
            .px(px(8.))
            .bg(rgb(0xf5f5f5))
            .border_t_1()
            .border_color(rgb(0xd7d7d7))
            .child(div().flex_1())
            .children(add)
            .child(div().flex_1())
            .child(div().size(px(14.)).rounded_full().bg(crate::theme::help_dot(0x999999)).flex().items_center().justify_center().text_size(px(10.)).font_weight(FontWeight::BOLD).text_color(crate::theme::p().dialog_bg).child("?"));
        v_flex()
            .w(px(PANEL_W))
            .h_full()
            .flex_none()
            .bg(gpui_kit::white())
            .border_l_1()
            .border_color(rgb(0xcccccc))
            .child(header)
            .child(div().flex_1().min_h_0().child(body))
            .child(footer)
            .into_any_element()
    }
}

/// Configuration variable types, as the Part Studio's Variable input dialog.
const VAR_KINDS: [(&str, VariableKind); 5] = [("Length", VariableKind::Length), ("Angle", VariableKind::Angle), ("Integer", VariableKind::Integer), ("Real", VariableKind::Real), ("Text", VariableKind::Text)];

/// Add configuration input › Checkbox / Configuration variable (live vs-911,
/// 913): a 220 px feature-style dialog left of the panel.
pub(super) struct InputDialog {
    variable: bool,
    name: Entity<InputState>,
    /// Checkbox: ☑ Default.
    default_on: bool,
    /// Variable: the type tab and Default / Minimum / Maximum.
    kind: usize,
    values: [Entity<InputState>; 3],
}

/// Default / Minimum / Maximum of a new variable of `kind` (1 / 0 / 100000
/// in the workspace unit; Text: empty).
fn var_prefill(kind: VariableKind, units: &noshape_units::UnitPrefs) -> [String; 3] {
    let unit = match kind {
        VariableKind::Length => Some(units.length.symbol()),
        VariableKind::Angle => Some(units.angle.symbol()),
        _ => None,
    };
    let v = |n: &str| unit.map(|u| format!("{n} {u}")).unwrap_or_else(|| n.to_string());
    if kind == VariableKind::Text { [String::new(), String::new(), String::new()] } else { [v("1"), v("0"), v("100000")] }
}

impl VariableStudioView {
    fn open_input_dialog(&mut self, variable: bool, window: &mut Window, cx: &mut Context<Self>) {
        let units = self.doc.read(cx).session.doc.settings.units;
        let pre = var_prefill(VariableKind::Length, &units);
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("Name"));
        let values = pre.map(|v| cx.new(|cx| InputState::new(window, cx).default_value(v)));
        name.update(cx, |s, cx| s.focus(window, cx));
        self.cfg_dialog = Some(InputDialog { variable, name, default_on: true, kind: 0, values });
        self.config_open = true;
        cx.notify();
    }

    fn commit_input_dialog(&mut self, cx: &mut Context<Self>) {
        let Some(d) = &self.cfg_dialog else { return };
        let name = d.name.read(cx).value().trim().to_string();
        if name.is_empty() {
            return;
        }
        let kind = if d.variable {
            let (_, k) = VAR_KINDS[d.kind];
            let [def, lo, hi] = [0, 1, 2].map(|i| d.values[i].read(cx).value().to_string());
            let text = k == VariableKind::Text;
            ConfigInputKind::Variable { kind: k, default: ExprValue::new(def), min: (!text).then(|| ExprValue::new(lo)), max: (!text).then(|| ExprValue::new(hi)) }
        } else {
            ConfigInputKind::Checkbox { default: d.default_on }
        };
        self.cfg_dialog = None;
        self.cfg_edit("Add configuration input", cx, move |c| {
            // Live ei-911: a checkbox "CB" has the ID "CB".
            let taken: Vec<String> = c.inputs.iter().map(|i| i.id.clone()).collect();
            let id = name_id(&name, &taken);
            c.inputs.push(ConfigInput { id, name, kind, visible_when: None, excluded: false });
        });
        cx.notify();
    }

    /// The dialog (top right of the table area, left of the panel).
    pub(super) fn render_input_dialog(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let d = self.cfg_dialog.as_ref()?;
        let pal = p();
        let blue = rgb(0x1651b0);
        let ok = !d.name.read(cx).value().trim().is_empty();
        let title = if d.variable { "Variable input" } else { "Checkbox input" };
        let row = |id: &'static str, label: &'static str, st: &Entity<InputState>| {
            h_flex()
                .id(id)
                .h(px(28.))
                .px(px(5.))
                .child(div().w(px(64.)).flex_none().text_size(px(12.)).text_color(rgb(0x666666)).child(label))
                .child(gpui_kit::base::input::input_frame(div().flex_1().min_w_0().border_b_1().border_color(rgb(0xd7d7d7)).child(Input::new(st).appearance(false).text_size(px(13.33)))))
        };
        let mut body = v_flex().child(row("vci-name", "Name", &d.name));
        if d.variable {
            let mut tabs = h_flex();
            for (i, (label, _)) in VAR_KINDS.iter().enumerate() {
                let on = d.kind == i;
                tabs = tabs.child(
                    div()
                        .id(SharedString::from(format!("vci-kind-{i}")))
                        .flex_1()
                        .h(px(26.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .text_size(px(12.))
                        .when(on, |d| d.font_weight(FontWeight::MEDIUM).text_color(blue).border_b_2().border_color(blue))
                        .when(!on, |d| d.text_color(pal.text).border_b_1().border_color(rgb(0xd7d7d7)))
                        .child(*label)
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                            let units = this.doc.read(cx).session.doc.settings.units;
                            let Some(d) = &mut this.cfg_dialog else { return };
                            if d.kind != i {
                                d.kind = i;
                                for (st, v) in d.values.clone().into_iter().zip(var_prefill(VAR_KINDS[i].1, &units)) {
                                    st.update(cx, |s, cx| s.set_value(v, window, cx));
                                }
                                cx.notify();
                            }
                        })),
                );
            }
            let text = VAR_KINDS[d.kind].1 == VariableKind::Text;
            body = body.child(tabs).child(row("vci-default", "Default", &d.values[0])).when(!text, |b| b.child(row("vci-min", "Minimum", &d.values[1])).child(row("vci-max", "Maximum", &d.values[2])));
        } else {
            let on = d.default_on;
            body = body.child(crate::os_ui::checkbox("vci-default-on", "Default", on).pl(px(6.)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                if let Some(d) = &mut this.cfg_dialog {
                    d.default_on = !d.default_on;
                }
                cx.notify();
            })));
        }
        Some(
            v_flex()
                .id("vs-input-dialog")
                .occlude()
                .absolute()
                .top(px(3.))
                .right(px(10.))
                .w(px(220.))
                .bg(gpui_kit::white())
                .shadow_md()
                .text_color(pal.text)
                .on_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| {
                    if e.keystroke.key == "enter" {
                        cx.stop_propagation();
                        this.commit_input_dialog(cx);
                    }
                }))
                .child(
                    h_flex()
                        .h(px(29.))
                        .border_b_1()
                        .border_color(rgb(0xd7d7d7))
                        // Red until it has a name (live vs-913).
                        .child(div().flex_1().pl(px(6.)).text_size(px(13.)).font_weight(FontWeight::MEDIUM).when(!ok, |d| d.text_color(rgb(0xaa252d))).child(title))
                        .child(div().id("vci-ok").size(px(28.)).flex().items_center().justify_center().bg(pal.accept).when(!ok, |d| d.opacity(0.2)).cursor_pointer().child(crate::ui::glyph("check", 16., gpui_kit::white())).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.commit_input_dialog(cx))))
                        .child(div().id("vci-cancel").size(px(28.)).flex().items_center().justify_center().cursor_pointer().child(crate::ui::glyph("close", 14., rgb(0xd52b1e))).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.cfg_dialog = None;
                            cx.notify();
                        }))),
                )
                .child(body)
                .into_any_element(),
        )
    }
}

impl VariableStudioView {
    /// Once configured, the "Configurations" box left of the table (live
    /// vs-905, 914): "Configurations [▦▾]", then per input its selector —
    /// a list's select, a variable's value, a checkbox — choosing the
    /// configuration the studio shows.
    pub(super) fn render_config_box(&mut self, studio: &VariableStudio, cx: &mut Context<Self>) -> Option<AnyElement> {
        let cfg = &studio.configuration;
        if cfg.is_empty() {
            return None;
        }
        let pal = p();
        let view = cx.entity();
        let mut col = v_flex().gap(px(2.)).px(px(4.));
        for input in &cfg.inputs {
            let chosen = cfg.value_of(&cfg.active, &input.id).unwrap_or_default();
            let iid = input.id.clone();
            let label = |t: String| div().w(px(72.)).flex_none().text_size(px(11.)).text_color(pal.text_muted).child(t);
            let row: AnyElement = match &input.kind {
                ConfigInputKind::List { options, .. } => {
                    let shown = options.iter().find(|o| o.id == chosen).map(|o| o.name.clone()).unwrap_or_default();
                    let (opts, v) = (options.clone(), view.clone());
                    h_flex()
                        .id(SharedString::from(format!("vcfg-box-{iid}")))
                        .h(px(27.))
                        .child(label(input.name.clone()))
                        .child(h_flex().flex_1().h(px(24.)).border_b_1().border_color(rgb(0xcccccc)).cursor_pointer().text_size(px(13.33)).text_color(pal.text).child(div().flex_1().truncate().child(shown)).child(crate::ui::glyph("drawing-tool-caret", 12., pal.text)))
                        .on_click(move |e: &ClickEvent, window, cx| {
                            let (opts, v, iid, chosen) = (opts.clone(), v.clone(), iid.clone(), chosen.clone());
                            let menu = crate::os_menu::OsMenu::build(window, cx, move |mut m, _, _| {
                                m = m.select_list(140.);
                                for o in &opts {
                                    let (v, iid, oid) = (v.clone(), iid.clone(), o.id.clone());
                                    m = m.item(crate::os_menu::OsMenuItem::new(o.name.clone()).checked(o.id == chosen).on_click(move |_, _, cx| {
                                        let (iid, oid) = (iid.clone(), oid.clone());
                                        v.update(cx, |t, cx| t.cfg_edit("Change configuration", cx, move |c| {
                                            c.active.0.insert(iid, oid);
                                        }))
                                    }));
                                }
                                m
                            });
                            crate::os_menu::open(e.position(), menu, window, cx);
                        })
                        .into_any_element()
                }
                ConfigInputKind::Checkbox { .. } => {
                    let on = chosen == "true";
                    crate::os_ui::checkbox(SharedString::from(format!("vcfg-box-{iid}")), input.name.clone(), on)
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            let iid = iid.clone();
                            this.cfg_edit("Change configuration", cx, move |c| {
                                c.active.0.insert(iid, (!on).to_string());
                            })
                        }))
                        .into_any_element()
                }
                _ => {
                    let commit: Commit = {
                        let iid = iid.clone();
                        std::rc::Rc::new(move |c: &mut VsConfiguration, v: String| {
                            c.active.0.insert(iid.clone(), v.trim().to_string());
                        })
                    };
                    let value = self.cfg_text(format!("box-{iid}"), chosen.clone(), (13.33, FontWeight::NORMAL, pal.text.into()), commit, cx);
                    // Right-aligned (live vs-914 "1 mm").
                    h_flex().h(px(27.)).child(label(input.name.clone())).child(div().flex_1().flex().text_right().border_b_1().border_color(rgb(0xcccccc)).child(value)).into_any_element()
                }
            };
            col = col.child(row);
        }
        let tab = div()
            .id("vcfg-box-panel")
            .h(px(26.))
            .flex()
            .items_center()
            .rounded(px(3.))
            .border_1()
            .border_color(rgb(0xaaaaaa))
            .cursor_pointer()
            .child(div().px(px(4.)).child(crate::assets::icon("configuration", 18., cx)))
            .child(div().w(px(18.)).h_full().flex().items_center().justify_center().border_l_1().border_color(rgb(0xaaaaaa)).child(crate::ui::glyph("drawing-tool-caret", 12., pal.text)))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.config_open = !this.config_open;
                cx.notify();
            }));
        Some(
            v_flex()
                .w(px(205.))
                .h_full()
                .flex_none()
                .bg(gpui_kit::white())
                .border_r_1()
                .border_color(rgb(0xd7d7d7))
                .child(h_flex().h(px(36.)).px(px(4.)).child(div().flex_1().text_size(px(13.33)).font_weight(FontWeight::MEDIUM).text_color(pal.text).child("Configurations")).child(tab))
                .child(col)
                .into_any_element(),
        )
    }
}

/// An internal ID's problem: empty, not a FeatureScript identifier, or a
/// reserved keyword.
fn id_error(id: &str) -> Option<&'static str> {
    if id.is_empty() {
        return Some("ID cannot be empty");
    }
    let mut chars = id.chars();
    let first_ok = chars.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_');
    if !first_ok || !chars.all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Some("ID must be a valid FeatureScript identifier");
    }
    noshape_model::configuration::FS_KEYWORDS.contains(&id).then_some("ID cannot be a reserved FeatureScript keyword")
}

/// Input ⋯ › Edit internal IDs… (live ei-906…911): a 600 px modal, "Edit
/// internal IDs" (blue, 20 px), Input ID and Row IDs tables (names
/// read-only grey, IDs editable), invalid IDs pink, duplicates in a red box,
/// a valid change in a yellow warning; [Break references and change IDs]
/// (enabled by a valid change) [Cancel].
struct IdsDialog {
    view: WeakEntity<VariableStudioView>,
    studio_name: String,
    input_name: String,
    input_id: String,
    default_display: String,
    others: Vec<String>,
    id: Entity<InputState>,
    rows: Vec<(String, String, Entity<InputState>)>,
}

impl VariableStudioView {
    fn open_internal_ids(&mut self, input: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(cfg) = self.studio(cx).map(|s| s.configuration.clone()) else { return };
        let Some(inp) = cfg.inputs.iter().find(|i| i.id == input).cloned() else { return };
        let default_display = match &inp.kind {
            ConfigInputKind::List { options, default } => options.iter().find(|o| &o.id == default).map(|o| o.name.clone()).unwrap_or_default(),
            ConfigInputKind::Checkbox { default } => default.to_string(),
            _ => String::new(),
        };
        let no_space = |s: &str, _: &mut App| !s.contains(char::is_whitespace);
        let id = cx.new(|cx| InputState::new(window, cx).validate(no_space).default_value(inp.id.clone()));
        let rows = match &inp.kind {
            ConfigInputKind::List { options, .. } => options.iter().map(|o| (o.id.clone(), o.name.clone(), cx.new(|cx| InputState::new(window, cx).validate(no_space).default_value(o.id.clone())))).collect(),
            _ => Vec::new(),
        };
        let studio_name = self.doc.read(cx).session.doc.tab(self.tab).map(|t| t.name.clone()).unwrap_or_default();
        id.update(cx, |s, cx| s.focus(window, cx));
        let others = cfg.inputs.iter().filter(|i| i.id != input).map(|i| i.id.clone()).collect();
        let view = cx.entity().downgrade();
        let dialog = cx.new(|_| IdsDialog { view, studio_name, input_name: inp.name.clone(), input_id: inp.id.clone(), default_display, others, id, rows });
        window.open_dialog(cx, move |d, _, _| d.w(px(598.)).margin_top(px(80.)).close_button(false).p_0().gap_0().rounded(px(4.)).child(dialog.clone()));
    }
}

impl IdsDialog {
    fn renames(&self, cx: &App) -> Vec<(String, String)> {
        self.rows.iter().map(|(old, _, st)| (old.clone(), st.read(cx).value().to_string())).filter(|(a, b)| a != b).collect()
    }

    /// The first duplicate ID.
    fn duplicate(&self, cx: &App) -> Option<String> {
        let id = self.id.read(cx).value().to_string();
        if id != self.input_id && self.others.contains(&id) {
            return Some(id);
        }
        let mut seen = HashSet::new();
        self.rows.iter().map(|(_, _, st)| st.read(cx).value().to_string()).find(|v| !seen.insert(v.clone()))
    }
}

impl Render for IdsDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let grid = rgb(0xaaaaaa);
        let multiple = !self.rows.is_empty();
        let new_id = self.id.read(cx).value().to_string();
        let renames = self.renames(cx);
        let (id_change, row_change) = (new_id != self.input_id, !renames.is_empty());
        let errors = id_error(&new_id).is_some() || self.rows.iter().any(|(_, _, st)| id_error(&st.read(cx).value()).is_some());
        let duplicate = self.duplicate(cx);
        let can_confirm = duplicate.is_none() && (id_change || row_change) && !errors;
        let cell = |w: f32| h_flex().w(px(w)).h(px(34.)).flex_none().px(px(4.)).border_r_1().border_b_1().border_color(grid).text_size(px(14.)).text_color(pal.text);
        let head = |w: f32, t: &str| cell(w).bg(rgb(0xeaeaea)).justify_center().font_weight(FontWeight::BOLD).child(t.to_string());
        // Invalid: pink (the Input ID's cell also says why on hover).
        let id_cell = |st: &Entity<InputState>, tip: bool, cx: &App| {
            let err = id_error(&st.read(cx).value());
            cell(366.)
                .when(err.is_some(), |d| d.bg(rgb(0xf9ebec)))
                .child(div().id(SharedString::from(format!("vid-{}", st.entity_id()))).flex_1().when_some(err.filter(|_| tip), |d, e| d.tooltip(crate::ui::tooltip(e))).child(Input::new(st).appearance(false).text_size(px(13.33))))
        };
        let table = |title: &str, name_head: &str, rows: Vec<(String, AnyElement)>| {
            let mut t = v_flex().border_l_1().border_t_1().border_color(grid).child(h_flex().child(head(200., name_head)).child(head(366., "Internal ID")));
            for (name, editor) in rows {
                t = t.child(h_flex().child(cell(200.).bg(rgb(0xf5f5f5)).text_color(pal.text_muted).child(name)).child(editor));
            }
            v_flex().gap(px(4.)).when(multiple, |d| d.child(div().text_size(px(14.)).font_weight(FontWeight::BOLD).text_color(rgb(0x1651b0)).child(title.to_string()))).child(t)
        };
        let id_rows = vec![(self.input_name.clone(), id_cell(&self.id, true, cx).into_any_element())];
        let option_rows: Vec<(String, AnyElement)> = self.rows.iter().map(|(_, name, st)| (name.clone(), id_cell(st, false, cx).into_any_element())).collect();
        let title = if multiple { "Edit internal IDs" } else { "Edit internal ID" };
        let info = if multiple { format!("This dialog will change the internal identifiers of the input {}.", self.input_name) } else { format!("This dialog will change the internal identifier of the input {}.", self.input_name) };
        let warning_text = if can_confirm && row_change {
            Some(format!("This change will break all references to {} in any assemblies, drawings, and other tabs which use a row of {} whose ID has changed.", self.studio_name, self.input_name))
        } else if can_confirm && id_change {
            Some(format!("After this change, all assemblies, drawings, and other tabs which reference {0} will revert {0}'s {1} to the default value \"{2}\".", self.studio_name, self.input_name, self.default_display))
        } else {
            None
        };
        let warning = warning_text.map(|text| h_flex().gap(px(12.)).px(px(16.)).py(px(14.)).rounded(px(4.)).bg(pal.warn_fill).border_1().border_color(pal.warn_outline).text_size(px(13.)).text_color(pal.text).child(crate::assets::icon("warning", 24., cx)).child(div().flex_1().min_w_0().child(text)));
        // Live ei-907: duplicates in a red box.
        let dup_box = duplicate.map(|d| div().px(px(16.)).py(px(12.)).rounded(px(4.)).bg(rgb(0xf8d7da)).border_1().border_color(rgb(0xf5c2c7)).text_size(px(13.)).text_color(rgb(0x842029)).child(format!("IDs must be unique. Cannot use duplicate ID \"{d}\".")));
        let (view, input_id) = (self.view.clone(), self.input_id.clone());
        v_flex()
            .w_full()
            .child(
                v_flex()
                    .px(px(16.))
                    .pt(px(16.))
                    .pb(px(12.))
                    .gap(px(8.))
                    .child(div().text_size(px(20.)).font_weight(FontWeight::MEDIUM).text_color(rgb(0x1651b0)).child(title))
                    .child(div().text_size(px(14.)).text_color(pal.text).child(info)),
            )
            .child(div().h(px(1.)).bg(rgb(crate::os_ui::MODAL_DIVIDER)))
            .child(v_flex().p(px(16.)).gap(px(12.)).child(table("Input ID", "Name", id_rows)).when(multiple, |d| d.child(table("Row IDs", "Row name", option_rows))))
            .child(
                v_flex()
                    .px(px(16.))
                    .py(px(12.))
                    .gap(px(8.))
                    .border_t_1()
                    .border_color(rgb(crate::os_ui::MODAL_DIVIDER))
                    .children(dup_box)
                    .children(warning)
                    .child(
                        h_flex()
                            .justify_end()
                            .gap(px(8.))
                            .child(gpui_kit::component::Disableable::disabled(crate::os_ui::primary_button("vid-ok", if multiple { "Break references and change IDs" } else { "Break references and change ID" }), !can_confirm).on_click(move |_, window, cx| {
                                let (old, new_id, renames) = (input_id.clone(), new_id.clone(), renames.clone());
                                let _ = view.update(cx, |t, cx| t.cfg_edit("Edit internal IDs", cx, move |c| c.change_ids(&old, &new_id, &renames)));
                                window.close_dialog(cx);
                            }))
                            .child(crate::os_ui::cancel_button("vid-cancel", "Cancel", cx).on_click(|_, window, cx| window.close_dialog(cx))),
                    ),
            )
    }
}
