//! Configurations panel › input ⋯ › Copy table, Copy input (and the panel's
//! Copy all inputs), Move up / down, Paste inputs, and Input propagation…
//! (Help PartStudio__managing_configurations, PartStudio__input_propagation).
//!
//! Input propagation: the Derived features of this Part Studio whose source
//! has an input with the same id (inputs copied between studios keep it) can
//! have their source configuration follow this input: a configured column
//! "<Derived 1> / Configuration" maps each option to the same option.

use super::PartStudioView;
use crate::os_ui as os;
use crate::theme::p;
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::*;
use noshape_model::FeatureId;
use noshape_model::configuration::{ConfigInput, ConfigInputKind, ConfiguredParam};
use noshape_model::params::ParamValue;
use noshape_model::regen::features::importing::DERIVED_CONFIG_PREFIX;

/// A Derived feature input propagation can configure.
#[derive(Clone)]
pub(super) struct Candidate {
    pub feature: FeatureId,
    pub name: String,
    /// Already propagated.
    pub done: bool,
}

/// The option keys of an input's rows (list options; "true"/"false").
fn row_keys(input: &ConfigInput) -> Vec<String> {
    match &input.kind {
        ConfigInputKind::List { options, .. } => options.iter().map(|o| o.id.clone()).collect(),
        ConfigInputKind::Checkbox { .. } => vec!["true".into(), "false".into()],
        _ => Vec::new(),
    }
}

impl PartStudioView {
    /// The input of another Part Studio tab by id.
    pub(super) fn source_input(&self, tab: &str, input: &str, cx: &App) -> Option<ConfigInput> {
        let doc = &self.doc.read(cx).session.doc;
        doc.tabs.iter().find(|t| t.id.to_string() == tab).and_then(|t| match &t.content {
            noshape_model::document::TabContent::PartStudio(ps) => ps.configuration.inputs.iter().find(|i| i.id == input).cloned(),
            _ => None,
        })
    }

    /// Where a configuration variable is used (the Usages table, live 433):
    /// "<feature> <parameter>" for every parameter whose expression names
    /// `#<name>`.
    pub(super) fn variable_usages(&self, name: &str, cx: &App) -> Vec<String> {
        let Some(studio) = self.studio(cx) else { return Vec::new() };
        let needle = format!("#{name}");
        let mut out = Vec::new();
        for f in &studio.features {
            let Some(ty) = f.feature_type() else { continue };
            for (id, v) in f.params.0.iter() {
                if let ParamValue::Expr(e) = v
                    && e.text.split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '#')).any(|w| w == needle)
                {
                    let label = ty.param(id).map(|p| p.label.to_string()).unwrap_or_else(|| id.to_string());
                    out.push(format!("{} {label}", f.name));
                }
            }
        }
        out
    }

    /// A propagated column's parameter header: the source input's name
    /// (live 444: "Derived 1" / "Size").
    pub(super) fn derived_input_name(&self, feature: FeatureId, input: &str, cx: &App) -> String {
        self.studio(cx)
            .and_then(|s| s.feature(feature))
            .and_then(|f| self.source_input(f.params.text("partStudio"), input, cx))
            .map(|i| i.name)
            .unwrap_or_else(|| input.to_string())
    }

    /// Derived features whose source has an input with this input's id and kind.
    pub(super) fn propagation_candidates(&self, input: &str, cx: &App) -> Vec<Candidate> {
        let Some(studio) = self.studio(cx) else { return Vec::new() };
        let Some(own) = studio.configuration.inputs.iter().find(|i| i.id == input) else { return Vec::new() };
        if !matches!(own.kind, ConfigInputKind::List { .. } | ConfigInputKind::Checkbox { .. }) {
            return Vec::new();
        }
        let param = format!("{DERIVED_CONFIG_PREFIX}{input}");
        studio
            .features
            .iter()
            .filter(|f| f.type_id == "importDerived")
            .filter(|f| self.source_input(f.params.text("partStudio"), input, cx).is_some_and(|s| std::mem::discriminant(&s.kind) == std::mem::discriminant(&own.kind)))
            .map(|f| Candidate { feature: f.id, name: f.name.clone(), done: studio.configuration.configured.iter().any(|c| c.feature == f.id && c.param == param) })
            .collect()
    }

    /// Copy table: the input's table as tab-separated text (Help: the
    /// feature names row, the "Name" + parameter row, then one row per
    /// option), for a spreadsheet.
    pub(super) fn copy_config_table(&mut self, input: &str, cx: &mut Context<Self>) {
        let Some(studio) = self.studio(cx).cloned() else { return };
        let Some(inp) = studio.configuration.inputs.iter().find(|i| i.id == input).cloned() else { return };
        let columns: Vec<&ConfiguredParam> = studio.configuration.configured.iter().filter(|c| c.input == input).collect();
        let fname = |c: &ConfiguredParam| studio.feature(c.feature).map(|f| f.name.clone()).unwrap_or_default();
        let plabel = |c: &ConfiguredParam| {
            if let Some(id) = c.param.strip_prefix(DERIVED_CONFIG_PREFIX) {
                return self.derived_input_name(c.feature, id, cx);
            }
            if c.param == noshape_model::configuration::SUPPRESSED_PARAM {
                return "Suppressed".to_string();
            }
            studio.feature(c.feature).and_then(|f| f.feature_type()).and_then(|t| t.param(&c.param)).map(|p| p.label.to_string()).unwrap_or_default()
        };
        let cell = |v: Option<&ParamValue>| match v {
            Some(ParamValue::Expr(e)) => e.text.clone(),
            Some(ParamValue::Bool(b)) => b.to_string(),
            Some(ParamValue::Enum(s)) | Some(ParamValue::Text(s)) => s.clone(),
            Some(ParamValue::Query(q)) => format!("{} selection{}", q.len(), if q.len() == 1 { "" } else { "s" }),
            _ => String::new(),
        };
        // Live (dom/config-copy-clipboard.txt): the table as shown — a list's
        // Name column ("Name\nSmall\nLarge"), a checkbox's Value column
        // ("Value\nfalse\ntrue"), with a feature-name row and parameter
        // headers once features are configured ("\tDerived 1\nName\tSize…");
        // a variable's usages ("Extrude 1 Depth"); nothing for a text input.
        let mut lines = Vec::new();
        match &inp.kind {
            ConfigInputKind::Variable { .. } => lines.extend(self.variable_usages(&inp.name, cx)),
            ConfigInputKind::Text { .. } => return,
            _ => {
                let (head, rows): (&str, Vec<(String, String)>) = match &inp.kind {
                    ConfigInputKind::List { options, .. } => ("Name", options.iter().map(|o| (o.id.clone(), o.name.clone())).collect()),
                    _ => ("Value", vec![("false".into(), "false".into()), ("true".into(), "true".into())]),
                };
                if !columns.is_empty() {
                    lines.push(std::iter::once(String::new()).chain(columns.iter().map(|c| fname(c))).collect::<Vec<_>>().join("\t"));
                }
                lines.push(std::iter::once(head.to_string()).chain(columns.iter().map(|c| plabel(c))).collect::<Vec<_>>().join("\t"));
                for (k, n) in rows {
                    lines.push(std::iter::once(n).chain(columns.iter().map(|c| cell(c.values.get(&k)))).collect::<Vec<_>>().join("\t"));
                }
            }
        }
        if lines.is_empty() {
            return;
        }
        cx.write_to_clipboard(ClipboardItem::new_string(lines.join("\n")));
    }

    /// Copy input (one) / Copy all inputs (`None`).
    pub(super) fn copy_config_inputs(&mut self, input: Option<&str>, cx: &mut Context<Self>) {
        let Some(studio) = self.studio(cx) else { return };
        let inputs: Vec<ConfigInput> = studio.configuration.inputs.iter().filter(|i| input.is_none_or(|id| i.id == id)).cloned().collect();
        // Live: "Size copied." / "Inputs copied." (Onshape's own clipboard).
        let toast = match (input, inputs.as_slice()) {
            (Some(_), [one]) => format!("{} copied.", one.name),
            _ => "Inputs copied.".to_string(),
        };
        crate::config_visibility::copy_inputs(inputs);
        self.flash_success(toast, cx);
    }

    /// Paste inputs: the copied inputs whose ids this studio doesn't have
    /// yet; visibility conditions needing missing inputs are repaired.
    pub(super) fn paste_config_inputs(&mut self, cx: &mut Context<Self>) {
        let copied = crate::config_visibility::copied_inputs();
        let mut repaired = false;
        let mut pasted: Vec<String> = Vec::new();
        self.doc.update(cx, |doc, cx| {
            doc.edit_studio(self.tab, "Paste configuration inputs", cx, |ps| {
                for i in copied {
                    if !ps.configuration.inputs.iter().any(|x| x.id == i.id) {
                        pasted.push(i.name.clone());
                        ps.configuration.inputs.push(i);
                    }
                }
                repaired = !ps.configuration.repair_visibility().is_empty();
            });
        });
        if repaired {
            self.flash_info("Visibility conditions for pasted inputs were updated.".into(), cx);
        } else if !pasted.is_empty() {
            // Live 439: "Size pasted."
            self.flash_success(if pasted.len() == 1 { format!("{} pasted.", pasted[0]) } else { "Inputs pasted.".into() }, cx);
        }
        self.config_dirty(cx);
    }

    pub(super) fn move_config_input(&mut self, input: &str, by: i32, cx: &mut Context<Self>) {
        let id = input.to_string();
        self.edit_config("Move configuration input", cx, |ps| {
            let inputs = &mut ps.configuration.inputs;
            let Some(k) = inputs.iter().position(|i| i.id == id) else { return };
            let to = k as i32 + by;
            if to >= 0 && (to as usize) < inputs.len() {
                inputs.swap(k, to as usize);
            }
        });
    }

    /// The "Input propagation for <studio>" dialog for one input (its ⋯) or
    /// every list / checkbox input (the panel's ⋯).
    pub(super) fn open_input_propagation(&mut self, input: Option<&str>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(studio) = self.studio(cx).cloned() else { return };
        let inputs: Vec<(String, String)> = studio
            .configuration
            .inputs
            .iter()
            .filter(|i| matches!(i.kind, ConfigInputKind::List { .. } | ConfigInputKind::Checkbox { .. }))
            .filter(|i| input.is_none_or(|id| i.id == id))
            .map(|i| (i.id.clone(), i.name.clone()))
            .collect();
        if inputs.is_empty() {
            return;
        }
        // Rows: every Derived feature some input can propagate to; a cell per
        // input (None: that source has no such input).
        let mut rows: Vec<(FeatureId, String, Vec<Option<bool>>)> = Vec::new();
        for (k, (id, _)) in inputs.iter().enumerate() {
            for c in self.propagation_candidates(id, cx) {
                let at = match rows.iter().position(|r| r.0 == c.feature) {
                    Some(at) => at,
                    None => {
                        rows.push((c.feature, c.name.clone(), vec![None; inputs.len()]));
                        rows.len() - 1
                    }
                };
                rows[at].2[k] = Some(c.done);
            }
        }
        let checked = rows.iter().map(|r| r.2.iter().map(|c| c.unwrap_or(false)).collect()).collect();
        let studio_name = self.studio_name(cx);
        let view = cx.entity().downgrade();
        let ids: Vec<String> = inputs.iter().map(|i| i.0.clone()).collect();
        let on_confirm: OnConfirm = std::rc::Rc::new(move |picked: Vec<(usize, FeatureId)>, _: &mut Window, cx: &mut App| {
            let ids = ids.clone();
            let _ = view.update(cx, |this, cx| this.propagate_inputs(&ids, &picked, cx));
        });
        open_propagation_dialog(studio_name, inputs, rows, checked, on_confirm, window, cx);
    }

    /// Configure the Derived features' source configuration with the inputs
    /// (`picked`: input index, feature).
    fn propagate_inputs(&mut self, ids: &[String], picked: &[(usize, FeatureId)], cx: &mut Context<Self>) {
        let Some(studio) = self.studio(cx).cloned() else { return };
        let picked: Vec<(String, Vec<String>, FeatureId)> = picked
            .iter()
            .filter_map(|(k, f)| {
                let input = studio.configuration.inputs.iter().find(|i| i.id == ids[*k])?;
                Some((input.id.clone(), row_keys(input), *f))
            })
            .collect();
        self.edit_config("Input propagation", cx, |ps| {
            for (input, keys, f) in picked {
                let param = format!("{DERIVED_CONFIG_PREFIX}{input}");
                if ps.configuration.configured.iter().any(|c| c.feature == f && c.param == param) {
                    continue;
                }
                let values = keys.iter().map(|k| (k.clone(), ParamValue::Text(k.clone()))).collect();
                ps.configuration.configured.push(ConfiguredParam { input: input.clone(), feature: f, param, values });
            }
        });
        self.config_dirty(cx);
        self.flash_success("Matching parameters successfully configured.".into(), cx);
    }
}

pub(crate) type OnConfirm = std::rc::Rc<dyn Fn(Vec<(usize, FeatureId)>, &mut Window, &mut App)>;

/// Rows of the Input propagation dialog: (row key, reference name, per
/// input: None = can't propagate, Some(already propagated)).
pub(crate) type PropagationRows = Vec<(FeatureId, String, Vec<Option<bool>>)>;

/// Open the Input propagation dialog (live 443) for `owner_name`'s inputs
/// (id, name) over `rows`; ✓ hands the newly ticked (input index, row key)
/// to `on_confirm`. Shared with the assembly (skb: rows are instances, the
/// key their id).
pub(crate) fn open_propagation_dialog(owner_name: String, inputs: Vec<(String, String)>, rows: PropagationRows, checked: Vec<Vec<bool>>, on_confirm: OnConfirm, window: &mut Window, cx: &mut App) {
    let width = 320. + 300. * inputs.len() as f32;
    let dialog = cx.new(|_| PropagationDialog { studio_name: owner_name, inputs, rows, checked, on_confirm });
    window.open_dialog(cx, move |d, _, _| d.w(px(width)).margin_top(px(80.)).close_button(false).p_0().gap_0().rounded(px(4.)).child(dialog.clone()));
}

/// The dialog's state: references (rows), inputs (columns) and the
/// Propagate checkboxes (already propagated ones stay ticked).
struct PropagationDialog {
    studio_name: String,
    /// (id, name) per column; the header shows the name (live 443: "Size").
    inputs: Vec<(String, String)>,
    /// (feature, name, per input: None = can't propagate, Some(done)).
    rows: Vec<(FeatureId, String, Vec<Option<bool>>)>,
    checked: Vec<Vec<bool>>,
    on_confirm: OnConfirm,
}

impl PropagationDialog {
    /// Ticked cells not propagated yet.
    fn picked(&self) -> Vec<(usize, FeatureId)> {
        let mut out = Vec::new();
        for (r, row) in self.rows.iter().enumerate() {
            for (k, cell) in row.2.iter().enumerate() {
                if *cell == Some(false) && self.checked[r][k] {
                    out.push((k, row.0));
                }
            }
        }
        out
    }

    /// Set the cells `which` (row, column) selects; done cells stay ticked.
    fn set(&mut self, which: &dyn Fn(usize, usize) -> bool, on: bool) {
        for (r, row) in self.rows.iter().enumerate() {
            for (k, cell) in row.2.iter().enumerate() {
                if which(r, k) && *cell == Some(false) {
                    self.checked[r][k] = on;
                }
            }
        }
    }

    /// All cells `which` selects that can change are ticked.
    fn all(&self, which: &dyn Fn(usize, usize) -> bool) -> bool {
        let mut any = false;
        for (r, row) in self.rows.iter().enumerate() {
            for (k, cell) in row.2.iter().enumerate() {
                if which(r, k) && cell.is_some() {
                    any = true;
                    if !self.checked[r][k] {
                        return false;
                    }
                }
            }
        }
        any
    }

    /// A header / row checkbox: tick or untick the cells it covers.
    fn toggler(which: std::rc::Rc<dyn Fn(usize, usize) -> bool>, cx: &mut Context<Self>) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static + use<> {
        let view = cx.entity().downgrade();
        move |_, _, cx| {
            let which = which.clone();
            let _ = view.update(cx, |this, cx| {
                let on = !this.all(&*which);
                this.set(&*which, on);
                cx.notify();
            });
        }
    }
}

impl Render for PropagationDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let grid = gpui_kit::rgb(0xaaaaaa);
        let cell = |w: f32| h_flex().w(px(w)).h(px(36.)).px(px(4.)).border_r_1().border_b_1().border_color(grid).text_size(px(14.)).text_color(pal.text);
        let everything: std::rc::Rc<dyn Fn(usize, usize) -> bool> = std::rc::Rc::new(|_, _| true);
        let mut head = h_flex().bg(gpui_kit::rgb(0xeaeaea)).child(
            cell(298.)
                .justify_center()
                .font_weight(FontWeight::BOLD)
                .relative()
                .child("References")
                .child(div().absolute().right(px(8.)).child(crate::ui::os_checkbox("prop-all-refs", "", self.all(&*everything)).on_click(Self::toggler(everything.clone(), cx)))),
        );
        for (k, (_, name)) in self.inputs.iter().enumerate() {
            let column: std::rc::Rc<dyn Fn(usize, usize) -> bool> = std::rc::Rc::new(move |_, c| c == k);
            head = head.child(
                cell(298.)
                    .justify_center()
                    .font_weight(FontWeight::BOLD)
                    .relative()
                    .child(div().absolute().left(px(8.)).child(crate::ui::os_checkbox(("prop-col", k), "", self.all(&*column)).on_click(Self::toggler(column.clone(), cx))))
                    .child(name.clone()),
            );
        }
        let mut table = v_flex().mx(px(10.)).border_l_1().border_t_1().border_color(grid).child(head);
        for (r, row) in self.rows.iter().enumerate() {
            let this_row: std::rc::Rc<dyn Fn(usize, usize) -> bool> = std::rc::Rc::new(move |rr, _| rr == r);
            // Live 443: the reference cells are grey.
            let mut line = h_flex().child(
                cell(298.)
                    .bg(gpui_kit::rgb(0xeaeaea))
                    .justify_between()
                    .child(row.1.clone())
                    .child(crate::ui::os_checkbox(("prop-ref", r), "", self.all(&*this_row)).on_click(Self::toggler(this_row.clone(), cx))),
            );
            for (k, c) in row.2.iter().enumerate() {
                let one: std::rc::Rc<dyn Fn(usize, usize) -> bool> = std::rc::Rc::new(move |rr, cc| rr == r && cc == k);
                line = line.child(match c {
                    Some(_) => cell(298.).child(crate::ui::os_checkbox(("prop-cfg", r * 64 + k), "Propagate", self.checked[r][k]).on_click(Self::toggler(one, cx))),
                    None => cell(298.),
                });
            }
            table = table.child(line);
        }
        v_flex()
            .w_full()
            .child(
                h_flex()
                    .h(px(64.))
                    .px(px(10.))
                    .gap(px(6.))
                    .child(div().text_size(px(20.)).font_weight(FontWeight::MEDIUM).text_color(gpui_kit::rgb(0x1651b0)).child("Input propagation for"))
                    .child(crate::assets::icon("part-studio", 22., cx))
                    .child(div().text_size(px(20.)).font_weight(FontWeight::MEDIUM).text_color(gpui_kit::rgb(0x1651b0)).child(self.studio_name.clone()))
                    .child(div().flex_1())
                    .child(div().id("prop-close").cursor_pointer().opacity(0.5).child(crate::ui::glyph("close", 14., gpui_kit::rgb(0x000000))).on_click(|_, window, cx| window.close_dialog(cx))),
            )
            .child(div().h(px(1.)).bg(gpui_kit::rgb(os::MODAL_DIVIDER)))
            .child(div().pt(px(10.)).pb(px(18.)).child(table))
            .child(self.render_footer(cx))
    }
}

impl PropagationDialog {
    /// Footer: ?, "N new inputs to propagate", Confirm (disabled at 0).
    fn render_footer(&self, cx: &mut Context<Self>) -> AnyElement {
        use gpui_kit::component::Disableable as _;
        let pal = p();
        let picked = self.picked();
        let n = picked.len();
        let on_confirm = self.on_confirm.clone();
        h_flex()
            .h(px(60.))
            .px(px(12.))
            .gap(px(12.))
            .border_t_1()
            .border_color(gpui_kit::rgb(os::MODAL_DIVIDER))
            .child(crate::assets::icon("help", 20., cx))
            .child(div().flex_1())
            .child(div().text_size(px(14.)).text_color(pal.text).child(format!("{n} new input{} to propagate", if n == 1 { "" } else { "s" })))
            .child(os::primary_button("prop-confirm", "Confirm").disabled(n == 0).on_click(move |_, window, cx| {
                on_confirm(picked.clone(), window, cx);
                window.close_dialog(cx);
            }))
            .into_any_element()
    }
}
