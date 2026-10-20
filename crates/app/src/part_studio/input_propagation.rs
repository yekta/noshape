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
            if c.param.starts_with(DERIVED_CONFIG_PREFIX) {
                return "Configuration".to_string();
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
        let mut lines = Vec::new();
        match &inp.kind {
            ConfigInputKind::Variable { default, .. } => {
                lines.push("Name\tDefault value".to_string());
                lines.push(format!("#{}\t{}", inp.name, default.text));
            }
            _ => {
                lines.push(std::iter::once(String::new()).chain(columns.iter().map(|c| fname(c))).collect::<Vec<_>>().join("\t"));
                lines.push(std::iter::once("Name".to_string()).chain(columns.iter().map(|c| plabel(c))).collect::<Vec<_>>().join("\t"));
                let names: Vec<(String, String)> = match &inp.kind {
                    ConfigInputKind::List { options, .. } => options.iter().map(|o| (o.id.clone(), o.name.clone())).collect(),
                    _ => vec![("true".into(), "Checked".into()), ("false".into(), "Unchecked".into())],
                };
                for (k, n) in names {
                    lines.push(std::iter::once(n).chain(columns.iter().map(|c| cell(c.values.get(&k)))).collect::<Vec<_>>().join("\t"));
                }
            }
        }
        cx.write_to_clipboard(ClipboardItem::new_string(lines.join("\n")));
    }

    /// Copy input (one) / Copy all inputs (`None`).
    pub(super) fn copy_config_inputs(&mut self, input: Option<&str>, cx: &mut Context<Self>) {
        let Some(studio) = self.studio(cx) else { return };
        let inputs = studio.configuration.inputs.iter().filter(|i| input.is_none_or(|id| i.id == id)).cloned().collect();
        crate::config_visibility::copy_inputs(inputs);
    }

    /// Paste inputs: the copied inputs whose ids this studio doesn't have
    /// yet; visibility conditions needing missing inputs are repaired.
    pub(super) fn paste_config_inputs(&mut self, cx: &mut Context<Self>) {
        let copied = crate::config_visibility::copied_inputs();
        let mut repaired = false;
        self.doc.update(cx, |doc, cx| {
            doc.edit_studio(self.tab, "Paste configuration inputs", cx, |ps| {
                for i in copied {
                    if !ps.configuration.inputs.iter().any(|x| x.id == i.id) {
                        ps.configuration.inputs.push(i);
                    }
                }
                repaired = !ps.configuration.repair_visibility().is_empty();
            });
        });
        if repaired {
            self.flash_info("Visibility conditions for pasted inputs were updated.".into(), cx);
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

    /// The "Input propagation for <studio>" dialog.
    pub(super) fn open_input_propagation(&mut self, input: &str, window: &mut Window, cx: &mut Context<Self>) {
        let rows = self.propagation_candidates(input, cx);
        if rows.is_empty() {
            return;
        }
        let studio_name = self.studio_name(cx);
        let view = cx.entity().downgrade();
        let input = input.to_string();
        let on_confirm: OnConfirm = std::rc::Rc::new(move |features: Vec<FeatureId>, _: &mut Window, cx: &mut App| {
            let input = input.clone();
            let _ = view.update(cx, |this, cx| this.propagate_input(&input, &features, cx));
        });
        let dialog = cx.new(|_| PropagationDialog { studio_name, checked: rows.iter().map(|r| r.done).collect(), rows, on_confirm });
        window.open_dialog(cx, move |d, _, _| d.w(px(620.)).margin_top(px(80.)).close_button(false).p_0().gap_0().rounded(px(4.)).child(dialog.clone()));
    }

    /// Configure the Derived features' source configuration with this input.
    fn propagate_input(&mut self, input: &str, features: &[FeatureId], cx: &mut Context<Self>) {
        let Some(own) = self.studio(cx).and_then(|s| s.configuration.inputs.iter().find(|i| i.id == input).cloned()) else { return };
        let keys = row_keys(&own);
        let param = format!("{DERIVED_CONFIG_PREFIX}{input}");
        let (input, features) = (input.to_string(), features.to_vec());
        self.edit_config("Input propagation", cx, |ps| {
            for f in features {
                if ps.configuration.configured.iter().any(|c| c.feature == f && c.param == param) {
                    continue;
                }
                let values = keys.iter().map(|k| (k.clone(), ParamValue::Text(k.clone()))).collect();
                ps.configuration.configured.push(ConfiguredParam { input: input.clone(), feature: f, param: param.clone(), values });
            }
        });
        self.config_dirty(cx);
    }
}

type OnConfirm = std::rc::Rc<dyn Fn(Vec<FeatureId>, &mut Window, &mut App)>;

/// The dialog's state: references and their Propagate checkboxes.
struct PropagationDialog {
    studio_name: String,
    rows: Vec<Candidate>,
    checked: Vec<bool>,
    on_confirm: OnConfirm,
}

impl PropagationDialog {
    fn new_count(&self) -> usize {
        self.rows.iter().zip(&self.checked).filter(|(r, c)| **c && !r.done).count()
    }

    /// The header checkboxes: tick or untick every row (done rows stay ticked).
    fn toggle_all(cx: &mut Context<Self>) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static + use<> {
        let view = cx.entity().downgrade();
        move |_, _, cx| {
            let _ = view.update(cx, |this, cx| {
                let on = !this.checked.iter().all(|c| *c);
                for (c, r) in this.checked.iter_mut().zip(&this.rows) {
                    *c = on || r.done;
                }
                cx.notify();
            });
        }
    }

    /// A row's checkboxes (rows already propagated stay ticked).
    fn toggle_row(k: usize, done: bool, cx: &mut Context<Self>) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static + use<> {
        let view = cx.entity().downgrade();
        move |_, _, cx| {
            if !done {
                let _ = view.update(cx, |this, cx| {
                    this.checked[k] = !this.checked[k];
                    cx.notify();
                });
            }
        }
    }
}

impl Render for PropagationDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let grid = gpui_kit::rgb(0xaaaaaa);
        let all = self.checked.iter().all(|c| *c);
        let cell = |w: f32| h_flex().w(px(w)).h(px(36.)).px(px(4.)).border_r_1().border_b_1().border_color(grid).text_size(px(14.)).text_color(pal.text);
        let toggle_all = Self::toggle_all;
        let mut table = v_flex().mx(px(10.)).border_l_1().border_t_1().border_color(grid).child(
            h_flex()
                .bg(gpui_kit::rgb(0xeaeaea))
                .child(cell(298.).justify_center().font_weight(FontWeight::BOLD).relative().child("References").child(div().absolute().right(px(8.)).child(crate::ui::os_checkbox("prop-all-refs", "", all).on_click(toggle_all(cx)))))
                .child(cell(298.).justify_center().font_weight(FontWeight::BOLD).relative().child(div().absolute().left(px(8.)).child(crate::ui::os_checkbox("prop-all-cfg", "", all).on_click(toggle_all(cx)))).child("Configuration")),
        );
        for (k, r) in self.rows.iter().enumerate() {
            let on = self.checked[k];
            let done = r.done;
            let toggle = move |cx: &mut Context<Self>| Self::toggle_row(k, done, cx);
            table = table.child(
                h_flex()
                    .child(cell(298.).justify_between().child(r.name.clone()).child(crate::ui::os_checkbox(("prop-ref", k), "", on).on_click(toggle(cx))))
                    .child(cell(298.).child(crate::ui::os_checkbox(("prop-cfg", k), "Propagate", on).on_click(toggle(cx)))),
            );
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
        let n = self.new_count();
        let picked: Vec<FeatureId> = self.rows.iter().zip(&self.checked).filter(|(r, c)| **c && !r.done).map(|(r, _)| r.feature).collect();
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
