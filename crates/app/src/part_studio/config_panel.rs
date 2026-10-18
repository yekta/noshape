//! Right-hand Part Studio panel: variable table and configurations
//! (inputs, current configuration, configured feature parameters).

use super::PartStudioView;
use crate::theme::{p, with_alpha};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::{IconName, Selectable as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::configuration::{ConfigInput, ConfigInputKind, ConfiguredParam, ListOption, VariableKind};
use noshape_model::params::{ParamKind, ParamValue};
use noshape_model::partstudio::PartStudio;
use noshape_units::{Dim, ExprValue, Quantity};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RightPanel {
    Variables,
    Configurations,
    SheetMetal,
    CutList,
}

impl PartStudioView {
    pub(super) fn toggle_panel(&mut self, panel: RightPanel, window: &mut Window, cx: &mut Context<Self>) {
        self.right_panel = if self.right_panel == Some(panel) { None } else { Some(panel) };
        let _ = window;
        cx.notify();
    }

    fn edit_config(&mut self, desc: &str, cx: &mut Context<Self>, f: impl FnOnce(&mut PartStudio)) {
        self.doc.update(cx, |doc, cx| {
            doc.edit_studio(self.tab, desc, cx, f);
        });
    }

    /// A text cell bound to a stable key; `commit` runs on Enter/blur.
    fn cell_input(&mut self, key: String, value: String, window: &mut Window, cx: &mut Context<Self>, commit: impl Fn(&mut Self, String, &mut Context<Self>) + 'static) -> Entity<InputState> {
        if let Some((st, last)) = self.panel_inputs.get(&key)
            && *last == value
        {
            return st.clone();
        }
        // (Re)create when the model value changed underneath (undo, other edits).
        if let Some((st, _)) = self.panel_inputs.get(&key).cloned() {
            st.update(cx, |s, cx| s.set_value(value.clone(), window, cx));
            self.panel_inputs.insert(key.clone(), (st.clone(), value));
            return st;
        }
        let st = cx.new(|cx| InputState::new(window, cx).default_value(value.clone()));
        let commit = std::rc::Rc::new(commit);
        let sub = cx.subscribe_in(&st, window, move |this: &mut Self, st, ev: &InputEvent, _, cx| {
            if matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                let v = st.read(cx).value().to_string();
                commit(this, v, cx);
            }
        });
        self.panel_subs.push(sub);
        self.panel_inputs.insert(key, (st.clone(), value));
        st
    }

    pub(super) fn render_right_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let panel = self.right_panel?;
        let pal = p();
        let body = match panel {
            RightPanel::Variables => self.render_variables(cx),
            RightPanel::Configurations => self.render_configurations(window, cx),
            RightPanel::SheetMetal => self.render_sheet_metal_panel(cx),
            RightPanel::CutList => self.render_cut_list_panel(cx),
        };
        Some(
            v_flex()
                .w(px(320.))
                .h_full()
                .flex_none()
                .bg(pal.panel)
                .border_l_1()
                .border_color(pal.border)
                .child(
                    h_flex()
                        .h(px(32.))
                        .px_3()
                        .justify_between()
                        .border_b_1()
                        .border_color(pal.border)
                        .child(div().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(pal.text_muted).child(match panel {
                            RightPanel::Variables => "VARIABLES",
                            RightPanel::Configurations => "CONFIGURATIONS",
                            RightPanel::SheetMetal => "SHEET METAL",
                            RightPanel::CutList => "CUT LIST",
                        }))
                        .child(Button::new("panel-close").icon(IconName::Close).ghost().xsmall().on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.right_panel = None;
                            cx.notify();
                        }))),
                )
                .child(v_flex().id("right-panel-body").flex_1().p_2().gap_2().overflow_y_scroll().child(body))
                .into_any_element(),
        )
    }

    fn render_variables(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let units = self.doc.read(cx).session.doc.settings.units.clone();
        let vars: Vec<(String, Quantity)> = self.display.variables.iter().map(|(k, v)| (k.clone(), *v)).collect();
        if vars.is_empty() {
            return div()
                .text_xs()
                .text_color(pal.text_faint)
                .child("No variables yet. Add a Variable feature (toolbar → Variable) and use it in any field as #name.")
                .into_any_element();
        }
        v_flex()
            .gap_px()
            .children(vars.into_iter().map(|(name, q)| {
                h_flex()
                    .justify_between()
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .hover(|d| d.bg(pal.panel_hover))
                    .child(div().text_sm().text_color(pal.accent_hover).child(format!("#{name}")))
                    .child(div().text_sm().text_color(pal.text).child(noshape_units::format_quantity(q, &units)))
            }))
            .into_any_element()
    }

    fn render_configurations(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let Some(studio) = self.studio(cx).cloned() else { return div().into_any_element() };
        let cfg = studio.configuration.clone();
        let mut col = v_flex().gap_3();

        // Current configuration selector.
        if !cfg.inputs.is_empty() {
            let mut sel = v_flex().gap_1().p_2().rounded_md().bg(pal.app_bg).child(div().text_xs().text_color(pal.text_muted).child("Current configuration"));
            for input in &cfg.inputs {
                let current = cfg.value_of(&self.config_values, &input.id).unwrap_or_default();
                let id = input.id.clone();
                let control: AnyElement = match &input.kind {
                    ConfigInputKind::List { options, .. } => {
                        let label = options.iter().find(|o| o.id == current).map(|o| o.name.clone()).unwrap_or(current.clone());
                        let view = cx.entity();
                        let opts = options.clone();
                        Button::new(SharedString::from(format!("cur-{id}")))
                            .label(label)
                            .outline()
                            .xsmall()
                            .dropdown_caret(true)
                            .dropdown_menu(move |mut m, _, _| {
                                for o in &opts {
                                    let v = view.clone();
                                    let (iid, oid) = (id.clone(), o.id.clone());
                                    m = m.item(PopupMenuItem::new(o.name.clone()).on_click(move |_, w, cx| v.update(cx, |t, cx| t.set_config_value(&iid, oid.clone(), w, cx))));
                                }
                                m
                            })
                            .into_any_element()
                    }
                    ConfigInputKind::Checkbox { .. } => {
                        let on = current == "true";
                        gpui_kit::component::checkbox::Checkbox::new(SharedString::from(format!("cur-{id}")))
                            .checked(on)
                            .small()
                            .on_click(cx.listener(move |this, v: &bool, w, cx| this.set_config_value(&id, v.to_string(), w, cx)))
                            .into_any_element()
                    }
                    ConfigInputKind::Variable { .. } | ConfigInputKind::Text { .. } => {
                        let key = format!("curval-{id}");
                        let iid = id.clone();
                        let st = self.cell_input(key, current.clone(), window, cx, move |this, v, cx| {
                            this.config_values.0.insert(iid.clone(), v);
                            this.config_dirty(cx);
                        });
                        Input::new(&st).xsmall().into_any_element()
                    }
                };
                sel = sel.child(h_flex().gap_2().justify_between().child(div().text_xs().text_color(pal.text).child(input.name.clone())).child(div().w(px(150.)).child(control)));
            }
            col = col.child(sel);
        }

        // Inputs.
        for (ix, input) in cfg.inputs.iter().enumerate() {
            let iid = input.id.clone();
            let name_key = format!("in-name-{iid}");
            let iid2 = iid.clone();
            let name_input = self.cell_input(name_key, input.name.clone(), window, cx, move |this, v, cx| {
                let id = iid2.clone();
                this.edit_config("Rename configuration input", cx, |ps| {
                    if let Some(i) = ps.configuration.inputs.iter_mut().find(|i| i.id == id) {
                        i.name = v;
                    }
                });
            });
            let kind_label = match &input.kind {
                ConfigInputKind::List { .. } => "List",
                ConfigInputKind::Checkbox { .. } => "Checkbox",
                ConfigInputKind::Variable { .. } => "Variable",
                ConfigInputKind::Text { .. } => "Text",
            };
            let iid3 = iid.clone();
            let mut card = v_flex()
                .gap_1p5()
                .p_2()
                .rounded_md()
                .border_1()
                .border_color(pal.border)
                .child(
                    h_flex()
                        .gap_2()
                        .child(div().text_xs().px_1p5().rounded_sm().bg(pal.accent_soft).text_color(pal.accent_hover).child(kind_label))
                        .child(div().flex_1().child(Input::new(&name_input).xsmall()))
                        .child(Button::new(SharedString::from(format!("in-del-{ix}"))).icon(IconName::Delete).ghost().xsmall().on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            let id = iid3.clone();
                            this.edit_config("Delete configuration input", cx, |ps| {
                                ps.configuration.inputs.retain(|i| i.id != id);
                                ps.configuration.configured.retain(|c| c.input != id);
                            });
                        }))),
                );
            match &input.kind {
                ConfigInputKind::List { options, .. } => {
                    for (oi, o) in options.iter().enumerate() {
                        let key = format!("opt-{iid}-{}", o.id);
                        let (a, b) = (iid.clone(), o.id.clone());
                        let st = self.cell_input(key, o.name.clone(), window, cx, move |this, v, cx| {
                            let (a, b) = (a.clone(), b.clone());
                            this.edit_config("Rename configuration", cx, |ps| {
                                if let Some(ConfigInput { kind: ConfigInputKind::List { options, .. }, .. }) = ps.configuration.inputs.iter_mut().find(|i| i.id == a)
                                    && let Some(o) = options.iter_mut().find(|o| o.id == b)
                                {
                                    o.name = v;
                                }
                            });
                        });
                        let (a2, b2) = (iid.clone(), o.id.clone());
                        card = card.child(
                            h_flex()
                                .gap_1()
                                .child(div().w(px(16.)).text_xs().text_color(pal.text_faint).child(format!("{}", oi + 1)))
                                .child(div().flex_1().child(Input::new(&st).xsmall()))
                                .child(Button::new(SharedString::from(format!("opt-del-{iid}-{oi}"))).icon(IconName::Minus).ghost().xsmall().on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    let (a, b) = (a2.clone(), b2.clone());
                                    this.edit_config("Delete configuration", cx, |ps| {
                                        if let Some(ConfigInput { kind: ConfigInputKind::List { options, default }, .. }) = ps.configuration.inputs.iter_mut().find(|i| i.id == a)
                                            && options.len() > 1
                                        {
                                            options.retain(|o| o.id != b);
                                            if *default == b {
                                                *default = options[0].id.clone();
                                            }
                                        }
                                    });
                                }))),
                        );
                    }
                    let a = iid.clone();
                    card = card.child(Button::new(SharedString::from(format!("opt-add-{iid}"))).label("Add configuration").icon(IconName::Plus).ghost().xsmall().on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        let a = a.clone();
                        this.edit_config("Add configuration", cx, |ps| {
                            if let Some(ConfigInput { kind: ConfigInputKind::List { options, .. }, .. }) = ps.configuration.inputs.iter_mut().find(|i| i.id == a) {
                                let n = options.len() + 1;
                                options.push(ListOption { id: format!("opt{}", noshape_rand()), name: format!("Configuration {n}") });
                            }
                        });
                    })));
                }
                ConfigInputKind::Variable { default, kind, .. } => {
                    let key = format!("var-def-{iid}");
                    let a = iid.clone();
                    let st = self.cell_input(key, default.text.clone(), window, cx, move |this, v, cx| {
                        let a = a.clone();
                        this.edit_config("Edit configuration variable", cx, |ps| {
                            if let Some(ConfigInput { kind: ConfigInputKind::Variable { default, .. }, .. }) = ps.configuration.inputs.iter_mut().find(|i| i.id == a) {
                                *default = ExprValue::new(v);
                            }
                        });
                    });
                    card = card.child(
                        h_flex()
                            .gap_2()
                            .child(div().text_xs().text_color(pal.text_muted).child(format!("{kind:?} default")))
                            .child(div().flex_1().child(Input::new(&st).xsmall())),
                    );
                    card = card.child(div().text_xs().text_color(pal.text_faint).child(format!("Use as #{} in any field", input.name)));
                }
                _ => {}
            }
            col = col.child(card);
        }

        // Add input menu.
        let view = cx.entity();
        col = col.child(
            Button::new("add-input").label("Add input").icon(IconName::Plus).outline().xsmall().dropdown_menu(move |m, _, _| {
                let mk = |label: &'static str, kind: fn(usize) -> ConfigInputKind| {
                    let v = view.clone();
                    PopupMenuItem::new(label).on_click(move |_, _, cx| {
                        v.update(cx, |this, cx| {
                            this.edit_config("Add configuration input", cx, |ps| {
                                let n = ps.configuration.inputs.len() + 1;
                                let id = format!("in{}", noshape_rand());
                                ps.configuration.inputs.push(ConfigInput { id, name: format!("{label} {n}"), kind: kind(n), visible_when: None });
                            })
                        })
                    })
                };
                m.item(mk("List", |_| ConfigInputKind::List {
                    options: vec![ListOption { id: "default".into(), name: "Default".into() }, ListOption { id: format!("opt{}", noshape_rand()), name: "Configuration 2".into() }],
                    default: "default".into(),
                }))
                .item(mk("Checkbox", |_| ConfigInputKind::Checkbox { default: false }))
                .item(mk("Variable", |_| ConfigInputKind::Variable { kind: VariableKind::Length, default: ExprValue::new("10 mm"), min: None, max: None }))
                .item(mk("Text", |_| ConfigInputKind::Text { default: String::new() }))
            }),
        );

        // Configured parameters table.
        let list_inputs: Vec<ConfigInput> = cfg.inputs.iter().filter(|i| matches!(i.kind, ConfigInputKind::List { .. } | ConfigInputKind::Checkbox { .. })).cloned().collect();
        if !list_inputs.is_empty() {
            col = col.child(div().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(pal.text_muted).mt_2().child("CONFIGURED PARAMETERS"));
            for (ci, c) in cfg.configured.iter().enumerate() {
                let Some(input) = cfg.inputs.iter().find(|i| i.id == c.input) else { continue };
                let fname = studio.feature(c.feature).map(|f| f.name.clone()).unwrap_or_else(|| "deleted feature".into());
                let plabel = studio
                    .feature(c.feature)
                    .and_then(|f| f.feature_type())
                    .and_then(|t| t.param(&c.param))
                    .map(|p| p.label)
                    .unwrap_or(if c.param == noshape_model::configuration::SUPPRESSED_PARAM { "Suppressed" } else { "?" });
                let mut card = v_flex().gap_1().p_2().rounded_md().border_1().border_color(pal.border).child(
                    h_flex()
                        .justify_between()
                        .child(div().text_xs().text_color(pal.text).child(format!("{fname} › {plabel}")))
                        .child(Button::new(SharedString::from(format!("cp-del-{ci}"))).icon(IconName::Delete).ghost().xsmall().on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.edit_config("Remove configured parameter", cx, |ps| {
                                if ci < ps.configuration.configured.len() {
                                    ps.configuration.configured.remove(ci);
                                }
                            });
                        }))),
                );
                let keys: Vec<(String, String)> = match &input.kind {
                    ConfigInputKind::List { options, .. } => options.iter().map(|o| (o.id.clone(), o.name.clone())).collect(),
                    _ => vec![("true".into(), "Checked".into()), ("false".into(), "Unchecked".into())],
                };
                for (k, label) in keys {
                    let val = c.values.get(&k).cloned();
                    let text = match &val {
                        Some(ParamValue::Expr(e)) => e.text.clone(),
                        Some(ParamValue::Bool(b)) => b.to_string(),
                        Some(ParamValue::Enum(s)) => s.clone(),
                        Some(ParamValue::Text(s)) => s.clone(),
                        _ => String::new(),
                    };
                    let key = format!("cp-{ci}-{k}");
                    let kk = k.clone();
                    let st = self.cell_input(key, text, window, cx, move |this, v, cx| {
                        let kk = kk.clone();
                        this.edit_config("Edit configured value", cx, |ps| {
                            if let Some(cp) = ps.configuration.configured.get_mut(ci) {
                                let new = match cp.values.get(&kk) {
                                    Some(ParamValue::Bool(_)) => ParamValue::Bool(v.trim() == "true" || v.trim() == "1" || v.trim().eq_ignore_ascii_case("yes")),
                                    Some(ParamValue::Enum(_)) => ParamValue::Enum(v.trim().to_string()),
                                    Some(ParamValue::Text(_)) => ParamValue::Text(v),
                                    _ => ParamValue::Expr(ExprValue::new(v)),
                                };
                                cp.values.insert(kk.clone(), new);
                            }
                        });
                    });
                    card = card.child(h_flex().gap_2().child(div().w(px(110.)).text_xs().text_color(pal.text_muted).truncate().child(label)).child(div().flex_1().child(Input::new(&st).xsmall())));
                }
                col = col.child(card);
            }
            // Add a configured parameter: pick feature then parameter.
            let view = cx.entity();
            let feats: Vec<(noshape_model::FeatureId, String, Vec<(String, String)>)> = studio
                .features
                .iter()
                .filter_map(|f| {
                    let t = f.feature_type()?;
                    let params: Vec<(String, String)> = std::iter::once((noshape_model::configuration::SUPPRESSED_PARAM.to_string(), "Suppressed".to_string()))
                        .chain(t.params.iter().filter(|p| matches!(p.kind, ParamKind::Num(_) | ParamKind::Bool { .. } | ParamKind::Enum { .. })).filter(|p| p.is_visible(&f.params) && !p.hints.has(noshape_model::params::Hints::UNCONFIGURABLE)).map(|p| (p.id.to_string(), p.label.to_string())))
                        .collect();
                    Some((f.id, f.name.clone(), params))
                })
                .collect();
            let first_input = list_inputs[0].id.clone();
            col = col.child(
                Button::new("add-configured").label("Configure a parameter").icon(IconName::Plus).outline().xsmall().dropdown_menu(move |mut m, window, cx| {
                    for (fid, fname, params) in &feats {
                        let v = view.clone();
                        let params = params.clone();
                        let fid = *fid;
                        let input = first_input.clone();
                        m = m.submenu(fname.clone(), window, cx, move |mut sm, _, _| {
                            for (pid, plabel) in &params {
                                let v = v.clone();
                                let pid = pid.clone();
                                let input = input.clone();
                                sm = sm.item(PopupMenuItem::new(plabel.clone()).on_click(move |_, _, cx| v.update(cx, |this, cx| this.add_configured(fid, &pid, &input, cx))));
                            }
                            sm
                        });
                    }
                    m
                }),
            );
        }
        col.into_any_element()
    }

    fn add_configured(&mut self, feature: noshape_model::FeatureId, param: &str, input: &str, cx: &mut Context<Self>) {
        let param = param.to_string();
        let input = input.to_string();
        self.edit_config("Configure parameter", cx, |ps| {
            let Some(f) = ps.feature(feature) else { return };
            let current = if param == noshape_model::configuration::SUPPRESSED_PARAM { Some(ParamValue::Bool(f.suppressed)) } else { f.params.get(&param).cloned() };
            let Some(current) = current else { return };
            let keys: Vec<String> = match ps.configuration.inputs.iter().find(|i| i.id == input).map(|i| &i.kind) {
                Some(ConfigInputKind::List { options, .. }) => options.iter().map(|o| o.id.clone()).collect(),
                Some(ConfigInputKind::Checkbox { .. }) => vec!["true".into(), "false".into()],
                _ => return,
            };
            let values = keys.into_iter().map(|k| (k, current.clone())).collect();
            ps.configuration.configured.push(ConfiguredParam { input, feature, param, values });
        });
    }

    fn set_config_value(&mut self, input: &str, value: String, window: &mut Window, cx: &mut Context<Self>) {
        self.config_values.0.insert(input.to_string(), value);
        self.schedule_regen(window, cx);
        cx.notify();
    }

    fn config_dirty(&mut self, cx: &mut Context<Self>) {
        self.needs_regen = true;
        cx.notify();
    }

    /// Small toolbar of panel toggles shown above the view controls.
    pub(super) fn render_panel_toggles(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let active = self.right_panel;
        h_flex()
            .gap_0p5()
            .p_1()
            .rounded_md()
            .bg(with_alpha(pal.panel_raised, 0.85))
            .border_1()
            .border_color(pal.border)
            .child(
                Button::new("panel-vars")
                    .icon(gpui_kit::component::Icon::empty().path(crate::assets::icon_path("variable")))
                    .ghost()
                    .xsmall()
                    .selected(active == Some(RightPanel::Variables))
                    .tooltip("Variable table")
                    .on_click(cx.listener(|this, _: &ClickEvent, w, cx| this.toggle_panel(RightPanel::Variables, w, cx))),
            )
            .child(
                Button::new("panel-config")
                    .icon(gpui_kit::component::Icon::empty().path(crate::assets::icon_path("configuration")))
                    .ghost()
                    .xsmall()
                    .selected(active == Some(RightPanel::Configurations))
                    .tooltip("Configurations")
                    .on_click(cx.listener(|this, _: &ClickEvent, w, cx| this.toggle_panel(RightPanel::Configurations, w, cx))),
            )
            .when(self.has_sheet_metal(), |d| {
                d.child(
                    Button::new("panel-sm")
                        .icon(gpui_kit::component::Icon::empty().path(crate::assets::icon_path("sheet-metal")))
                        .ghost()
                        .xsmall()
                        .selected(active == Some(RightPanel::SheetMetal))
                        .tooltip("Sheet metal: flat pattern and bend table")
                        .on_click(cx.listener(|this, _: &ClickEvent, w, cx| this.toggle_panel(RightPanel::SheetMetal, w, cx))),
                )
            })
            .when(self.has_frames(), |d| {
                d.child(
                    Button::new("panel-cut")
                        .icon(gpui_kit::component::Icon::empty().path(crate::assets::icon_path("cut-list")))
                        .ghost()
                        .xsmall()
                        .selected(active == Some(RightPanel::CutList))
                        .tooltip("Cut list")
                        .on_click(cx.listener(|this, _: &ClickEvent, w, cx| this.toggle_panel(RightPanel::CutList, w, cx))),
                )
            })
            .into_any_element()
    }
}

fn noshape_rand() -> String {
    let u = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    format!("{:x}", (u as u64) ^ 0x9E37_79B9_7F4A_7C15)
}

#[allow(dead_code)]
fn quantity_text(q: Quantity) -> String {
    let _ = Dim::LENGTH;
    format!("{}", q.value)
}
