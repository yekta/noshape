//! Assembly configurations (live assembly/181-strip-configuration, 190–200,
//! dom/assembly-panel-configuration.txt, assembly-configuration-*.txt):
//! the right strip's Configuration panel (empty: "[▦ Configure Assembly][▾]"
//! with the illustration; configured: tabs Configurations | Configured
//! assembly properties, "Filter inputs", one section per input with
//! "+ Configure assembly features", the table of configurations × configured
//! parameters and the italic "Name" add row, footer "[▦ Add configuration
//! input][▲]"), the "Configurations" box above the instance list, the
//! yellow "Select instances or mates to configure" / "Select parameters to
//! configure" modes, and the "<feature> <param> added to Configuration"
//! toasts. Model: `noshape_model::assembly::config`.

use super::*;
use gpui_kit::component::input::InputEvent;
use std::rc::Rc;
use noshape_model::assembly::config::{self as acfg, ConfigTarget};
use noshape_model::configuration::{ConfigInput, ConfigInputKind};
use noshape_model::params::ParamValue;
use noshape_units::ExprValue;

const NAME_W: f32 = 140.;
const COL_W: f32 = 140.;
const ROW_H: f32 = 34.;

/// "+ Configure assembly features" mode: the input being configured, and
/// the mate whose dialog is open for picking parameters.
#[derive(Clone)]
pub(crate) struct CfgMode {
    pub input: String,
    pub mate: Option<MateId>,
}

/// A configuration table text cell being typed in.
pub(crate) struct CfgEdit {
    pub key: String,
    pub input: Entity<InputState>,
    _sub: Subscription,
}

type CfgCommit = Rc<dyn Fn(&mut AssemblyView, String, &mut Context<AssemblyView>)>;

impl AssemblyView {
    pub(super) fn asm_configuration(&self, cx: &App) -> acfg::AsmConfiguration {
        self.assembly(cx).map(|a| a.configuration.clone()).unwrap_or_default()
    }

    fn set_active_config(&mut self, input: &str, value: String, window: &mut Window, cx: &mut Context<Self>) {
        self.ui.config_values.0.insert(input.to_string(), value);
        self.schedule_eval(window, cx);
        cx.notify();
    }

    /// Configure Assembly ▾ / Add configuration input ▾: List, Checkbox,
    /// Configuration variable (a List is created at once, live 191).
    fn add_asm_config_input(&mut self, kind: &'static str, cx: &mut Context<Self>) {
        self.edit(noshape_ops::config::ADD_INPUT, cx, |a| noshape_ops::asm_config::add_input(a, kind));
        cx.notify();
    }

    fn kinds_menu(view: Entity<Self>, at: Point<Pixels>, anchor: Anchor, window: &mut Window, cx: &mut App) {
        let menu = crate::os_menu::OsMenu::build(window, cx, move |mut m, _, _| {
            for (label, icon, kind) in [("List", "cfg-filter-list", "List"), ("Checkbox", "cfg-filter-checkbox", "Checkbox"), ("Configuration variable", "cfg-filter-variable", "Variable")] {
                let v = view.clone();
                m = m.item(crate::os_menu::OsMenuItem::new(label).icon_name(icon).on_click(move |_, _, cx| v.update(cx, |this, cx| this.add_asm_config_input(kind, cx))));
            }
            m
        });
        crate::os_menu::open_anchored(at, anchor, menu, window, cx);
    }

    // ------------------------------------------------------------ configure mode

    /// "+ Configure assembly features": the yellow selection mode (live 192).
    fn start_configure_features(&mut self, input: String, window: &mut Window, cx: &mut Context<Self>) {
        self.close_mate(window, cx);
        self.ui.cfg_mode = Some(CfgMode { input, mate: None });
        cx.notify();
    }

    /// Done: leave the configure mode (and the mate dialog it opened).
    pub(super) fn finish_configure_features(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.ui.cfg_mode.take().is_some() {
            self.close_mate(window, cx);
            cx.notify();
        }
    }

    /// In configure mode a mate (list row or indicator) opens its dialog in
    /// "Select parameters to configure" mode (live 194). True when handled.
    pub(super) fn configure_pick_mate(&mut self, id: MateId, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(mode) = self.ui.cfg_mode.clone() else { return false };
        let Some(m) = self.assembly(cx).map(|a| a.configured(&self.ui.config_values)).and_then(|a| a.mate(id).cloned()) else { return true };
        self.close_mate(window, cx);
        self.open_mate_dialog(m, Some(id), window, cx);
        let configured: Vec<String> = self.asm_configuration(cx).configured.iter().filter(|c| c.target == ConfigTarget::Mate(id)).map(|c| c.param.clone()).collect();
        if let Some(s) = &self.mate {
            s.dialog.update(cx, |d, cx| {
                d.configuring = Some(configured);
                cx.notify();
            });
        }
        self.ui.cfg_mode = Some(CfgMode { input: mode.input, mate: Some(id) });
        cx.notify();
        true
    }

    /// A parameter clicked in the mate dialog while configuring.
    pub(super) fn configure_mate_param(&mut self, param: &'static str, cx: &mut Context<Self>) {
        let Some(CfgMode { input, mate: Some(id) }) = self.ui.cfg_mode.clone() else { return };
        let Some(asm) = self.assembly(cx) else { return };
        let mate_name = asm.mate(id).map(|m| m.name.clone()).unwrap_or_default();
        let input_name = asm.configuration.inputs.iter().find(|i| i.id == input).map(|i| i.name.clone()).unwrap_or_default();
        let mut added = false;
        self.edit(&noshape_ops::asm_config::configure_mate_description(&mate_name), cx, |a| added = noshape_ops::asm_config::configure(a, &input, ConfigTarget::Mate(id), param));
        if added {
            self.show_cfg_toast(format!("{mate_name} {} added to {input_name}", acfg::param_label(param)), false, cx);
            if let Some(s) = &self.mate {
                s.dialog.update(cx, |d, cx| {
                    if let Some(list) = &mut d.configuring {
                        list.push(param.to_string());
                    }
                    cx.notify();
                });
            }
        }
        cx.notify();
    }

    /// Instance row › Configure suppression (live 199).
    pub(super) fn configure_instance_suppression(&mut self, id: InstanceId, cx: &mut Context<Self>) {
        let Some(asm) = self.assembly(cx) else { return };
        let Some(input) = asm.configuration.inputs.iter().find(|i| matches!(i.kind, ConfigInputKind::List { .. } | ConfigInputKind::Checkbox { .. })).cloned() else { return };
        let name = asm.instance(id).map(|i| i.name.clone()).unwrap_or_default();
        let mut added = false;
        self.edit(&noshape_ops::asm_config::configure_suppression_description(&name), cx, |a| added = noshape_ops::asm_config::configure(a, &input.id, ConfigTarget::Instance(id), acfg::param::UNSUPPRESSED));
        if added {
            self.show_cfg_toast(format!("{name} suppression configured in {}", input.name), false, cx);
        }
        self.panel = Some(SidePanel::Configurations);
        cx.notify();
    }

    /// Whether the instance row menu offers Configure suppression.
    pub(super) fn can_configure_suppression(&self, id: InstanceId, cx: &App) -> bool {
        self.assembly(cx).is_some_and(|a| {
            a.configuration.inputs.iter().any(|i| matches!(i.kind, ConfigInputKind::List { .. } | ConfigInputKind::Checkbox { .. }))
                && !a.configuration.configured.iter().any(|c| c.target == ConfigTarget::Instance(id) && c.param == acfg::param::UNSUPPRESSED)
        })
    }

    /// The yellow banner of the configure mode.
    pub(super) fn render_cfg_banner(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let mode = self.ui.cfg_mode.as_ref()?;
        let text = if mode.mate.is_some() { "Select parameters to configure" } else { "Select instances or mates to configure" };
        Some(super::positions::yellow_banner(text.into(), cx.listener(|this, _: &ClickEvent, window, cx| this.finish_configure_features(window, cx))))
    }

    /// The blue "ⓘ <text> ×" message bubble at the top of the graphics area.
    /// A configuration toast (info, or green success) that fades after
    /// 4 s like the Part Studio's info / success bubbles.
    pub(super) fn show_cfg_toast(&mut self, text: String, ok: bool, cx: &mut Context<Self>) {
        self.ui.cfg_toast = Some(text.clone());
        self.ui.cfg_toast_ok = ok;
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(std::time::Duration::from_secs(4)).await;
            let _ = this.update(cx, |this: &mut Self, cx| {
                if this.ui.cfg_toast.as_deref() == Some(text.as_str()) {
                    this.ui.cfg_toast = None;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub(super) fn render_cfg_toast(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let text = self.ui.cfg_toast.clone()?;
        let view = cx.entity();
        Some(
            div()
                .absolute()
                .top(px(9.))
                .left_0()
                .right_0()
                // Centered on the window (live), not the graphics area.
                .pr(px(crate::ui::RAIL_W + if self.ui.list_collapsed { 0. } else { crate::ui::left_panel_w() }))
                .flex()
                .justify_center()
                .child(crate::os_ui::message_bubble("cfg-toast", if self.ui.cfg_toast_ok { crate::os_ui::BubbleKind::Success } else { crate::os_ui::BubbleKind::Info }, text, move |_, _, cx| {
                    view.update(cx, |this, cx| {
                        this.ui.cfg_toast = None;
                        cx.notify();
                    })
                }, cx))
                .into_any_element(),
        )
    }

    // ------------------------------------------------------------ text cells

    fn cfg_cell_text(&mut self, key: String, text: String, (size, weight): (Pixels, FontWeight), double: bool, commit: CfgCommit, cx: &mut Context<Self>) -> AnyElement {
        if let Some(e) = self.ui.cfg_edit.as_ref().filter(|e| e.key == key) {
            return div().flex_1().min_w_0().child(Input::new(&e.input).appearance(false).text_size(size).font_weight(weight).pl(px(3.))).into_any_element();
        }
        div()
            .id(SharedString::from(format!("acfg-{key}")))
            .flex_1()
            .min_w_0()
            .truncate()
            .text_size(size)
            .font_weight(weight)
            .child(text.clone())
            .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
                if !double || e.click_count() >= 2 {
                    this.edit_cfg_cell(key.clone(), text.clone(), commit.clone(), window, cx);
                }
            }))
            .into_any_element()
    }

    fn edit_cfg_cell(&mut self, key: String, current: String, commit: CfgCommit, window: &mut Window, cx: &mut Context<Self>) {
        let input = cx.new(|cx| InputState::new(window, cx).default_value(current));
        input.update(cx, |s, cx| {
            s.focus(window, cx);
            s.select_all(window, cx);
        });
        let k = key.clone();
        let sub = cx.subscribe_in(&input, window, move |this: &mut Self, st, ev: &InputEvent, window, cx| {
            let done = match ev {
                InputEvent::PressEnter { .. } => {
                    window.focus(&this.focus, cx);
                    true
                }
                InputEvent::Blur => true,
                _ => false,
            };
            if done && this.ui.cfg_edit.as_ref().is_some_and(|e| e.key == k) {
                let text = st.read(cx).value().to_string();
                this.ui.cfg_edit = None;
                commit(this, text, cx);
                cx.notify();
            }
        });
        self.ui.cfg_edit = Some(CfgEdit { key, input, _sub: sub });
        cx.notify();
    }

    // ------------------------------------------------------------ panel

    fn render_asm_config_empty(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let border = rgb(0xaaaaaa);
        let view = cx.entity();
        let main = div()
            .id("acfg-configure")
            .h(px(33.))
            .px(px(10.5))
            .flex()
            .items_center()
            .gap(px(6.))
            .cursor_pointer()
            .hover(|d| d.bg(pal.panel_hover))
            .child(crate::assets::icon("cfg-filter-list", 20., cx))
            .child(div().text_size(px(14.)).text_color(pal.text).child("Configure Assembly"))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.add_asm_config_input("List", cx)));
        let caret = div()
            .id("acfg-configure-more")
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
            .on_click(move |e: &ClickEvent, window, cx| Self::kinds_menu(view.clone(), e.position(), Anchor::TopLeft, window, cx));
        let link = |id: &'static str, t: &'static str| {
            div()
                .id(id)
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(0x094174))
                .underline()
                .cursor_pointer()
                .child(t)
                .on_click(|_, _, cx| cx.open_url("https://cad.onshape.com/help/Content/Assembly/assembly_configurations.htm"))
        };
        v_flex()
            .items_center()
            .pt(px(8.))
            .child(h_flex().rounded(px(5.25)).border_1().border_color(border).bg(crate::theme::p().panel).child(main).child(caret))
            .child(div().mt(px(105.)).child(crate::assets::icon("config-illustration", 160., cx)))
            .child(
                v_flex()
                    .mt(px(18.))
                    .items_center()
                    // Live dom assembly-panel-configuration: 14 px #666 on 21 px
                    // lines in a 400 px box (its line breaks).
                    .text_size(px(14.))
                    .line_height(px(21.))
                    .text_color(rgb(0x666666))
                    .child("Add configurations to create multiple variants of an assembly")
                    .child("using instance configurations, patterns, mates, mate connectors,")
                    .child(h_flex().child("or suppression states.\u{a0}").child(link("acfg-help-1", "Learn more about assembly")))
                    .child(link("acfg-help-2", "configurations.")),
            )
            .into_any_element()
    }

    /// Column header of a configured parameter: the feature name over the
    /// parameter (two-level header, live 196).
    fn cfg_column_names(&self, target: ConfigTarget, param: &str, cx: &App) -> (String, String) {
        let asm = self.assembly(cx);
        let name = match target {
            ConfigTarget::Instance(id) => asm.and_then(|a| a.instance(id)).map(|i| i.name.clone()),
            ConfigTarget::Mate(id) => asm.and_then(|a| a.mate(id)).map(|m| m.name.clone()),
        };
        (name.unwrap_or_else(|| "Deleted".into()), self.cfg_param_label(target, param, cx))
    }

    /// A column's parameter label; a propagated input reads as the
    /// instance's own input name.
    fn cfg_param_label(&self, target: ConfigTarget, param: &str, cx: &App) -> String {
        match (target, param.strip_prefix(acfg::param::INPUT_PREFIX)) {
            (ConfigTarget::Instance(id), Some(input)) => self.instance_source_input(id, input, cx).map(|i| i.name).unwrap_or_else(|| input.to_string()),
            _ => acfg::param_label(param).to_string(),
        }
    }

    /// The configuration inputs of an instance's source (its Part Studio or
    /// subassembly, of this document or another one).
    fn instance_source_inputs(&self, id: InstanceId, cx: &App) -> Vec<ConfigInput> {
        let Some(inst) = self.assembly(cx).and_then(|a| a.instance(id)) else { return Vec::new() };
        let of_tab = |doc: &noshape_model::Document, tab: noshape_model::document::TabId| match doc.tab(tab).map(|t| &t.content) {
            Some(TabContent::PartStudio(ps)) => ps.configuration.inputs.clone(),
            Some(TabContent::Assembly(a)) => a.configuration.inputs.clone(),
            _ => Vec::new(),
        };
        match &inst.source {
            InstanceSource::Part { studio, .. } | InstanceSource::PartStudio { studio, .. } | InstanceSource::MirroredPart { studio, .. } | InstanceSource::Sketch { studio, .. } => of_tab(&self.doc.read(cx).session.doc, *studio),
            InstanceSource::Assembly { assembly, .. } => of_tab(&self.doc.read(cx).session.doc, *assembly),
            InstanceSource::External { key, .. } => {
                use noshape_model::assembly::eval::ExternalStudios as _;
                super::AppExternal.studio(key).map(|(d, t)| of_tab(&d, t)).unwrap_or_default()
            }
            InstanceSource::Standard { .. } => Vec::new(),
        }
    }

    fn instance_source_input(&self, id: InstanceId, input: &str, cx: &App) -> Option<ConfigInput> {
        self.instance_source_inputs(id, cx).into_iter().find(|i| i.id == input)
    }

    /// The Configuration visibility conditions dialog (psb's
    /// `config_visibility`) for every input, or one input (input ⋯).
    fn open_asm_visibility(&mut self, input: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        let cfg = noshape_model::configuration::Configuration { inputs: self.asm_configuration(cx).inputs, configured: Vec::new() };
        let values = self.ui.config_values.clone();
        let view = cx.entity().downgrade();
        crate::config_visibility::open(
            cfg,
            values,
            input,
            move |edited, window, cx| {
                let _ = view.update(cx, |this, cx| {
                    this.edit(noshape_ops::config::EDIT_VISIBILITY, cx, |a| {
                        for e in &edited.inputs {
                            noshape_ops::asm_config::set_visibility(a, &e.id, e.visible_when.clone());
                        }
                    });
                    this.schedule_eval(window, cx);
                });
            },
            window,
            cx,
        );
    }

    /// Input propagation… (input ⋯; live part-studio/443, help Input
    /// propagation): psb's dialog over this assembly's instances whose source
    /// has an input with the same id and kind; ✓ configures the instance's
    /// input to follow the assembly's (`param::INPUT_PREFIX`).
    fn open_asm_input_propagation(&mut self, input: Option<&str>, window: &mut Window, cx: &mut Context<Self>) {
        let cfg = self.asm_configuration(cx);
        let inputs: Vec<(String, String)> = cfg
            .inputs
            .iter()
            .filter(|i| matches!(i.kind, ConfigInputKind::List { .. } | ConfigInputKind::Checkbox { .. }))
            .filter(|i| input.is_none_or(|id| i.id == id))
            .map(|i| (i.id.clone(), i.name.clone()))
            .collect();
        if inputs.is_empty() {
            return;
        }
        let instances: Vec<(InstanceId, String)> = self.assembly(cx).map(|a| a.instances.iter().map(|i| (i.id, i.name.clone())).collect()).unwrap_or_default();
        let Some(asm) = self.assembly(cx).cloned() else { return };
        let mut rows: crate::part_studio::input_propagation::PropagationRows = Vec::new();
        for (iid, name) in instances {
            let theirs = self.instance_source_inputs(iid, cx);
            let cells: Vec<Option<bool>> = inputs
                .iter()
                .map(|(id, _)| {
                    let own = cfg.inputs.iter().find(|i| &i.id == id)?;
                    noshape_ops::asm_config::propagation_state(&asm, own, &theirs, iid)
                })
                .collect();
            if cells.iter().any(|c| c.is_some()) {
                rows.push((noshape_model::reference::FeatureId(iid.0), name, cells));
            }
        }
        let checked = rows.iter().map(|r| r.2.iter().map(|c| c.unwrap_or(false)).collect()).collect();
        let name = self.doc.read(cx).session.doc.tab(self.tab).map(|t| t.name.clone()).unwrap_or_default();
        let view = cx.entity().downgrade();
        let ids: Vec<String> = inputs.iter().map(|i| i.0.clone()).collect();
        let on_confirm: crate::part_studio::input_propagation::OnConfirm = Rc::new(move |picked, _: &mut Window, cx: &mut App| {
            let ids = ids.clone();
            let _ = view.update(cx, |this, cx| this.propagate_asm_inputs(&ids, &picked, cx));
        });
        crate::part_studio::input_propagation::open_propagation_dialog(name, inputs, rows, checked, on_confirm, window, cx);
    }

    /// Configure the picked instances' inputs to follow the assembly's (each
    /// option to the option with the same id; a checkbox to its state).
    fn propagate_asm_inputs(&mut self, ids: &[String], picked: &[(usize, noshape_model::reference::FeatureId)], cx: &mut Context<Self>) {
        let cfg = self.asm_configuration(cx);
        let picked: Vec<(String, InstanceId)> = picked.iter().filter_map(|(k, f)| cfg.inputs.iter().find(|i| i.id == ids[*k]).map(|i| (i.id.clone(), InstanceId(f.0)))).collect();
        if picked.is_empty() {
            return;
        }
        self.edit(noshape_ops::config::INPUT_PROPAGATION, cx, |a| noshape_ops::asm_config::propagate_inputs(a, &picked));
        self.show_cfg_toast("Matching parameters successfully configured.".into(), true, cx);
    }

    fn render_asm_config_input(&mut self, cfg: &acfg::AsmConfiguration, input: &ConfigInput, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let grid = rgb(0xaaaaaa);
        let iid = input.id.clone();
        let folded = self.ui.cfg_folded.contains(&iid);
        let current = cfg.value_of(&self.ui.config_values, &iid).unwrap_or_default();
        let default_text = match &input.kind {
            ConfigInputKind::List { options, default } => options.iter().find(|o| &o.id == default).map(|o| o.name.clone()).unwrap_or_default(),
            ConfigInputKind::Checkbox { default } => if *default { "Checked".into() } else { "Unchecked".into() },
            ConfigInputKind::Variable { default, .. } => default.text.clone(),
            ConfigInputKind::Text { default } => default.clone(),
        };
        let configurable = matches!(input.kind, ConfigInputKind::List { .. } | ConfigInputKind::Checkbox { .. });
        let picking = self.ui.cfg_mode.as_ref().is_some_and(|m| m.input == iid);
        let view = cx.entity();
        let (i1, i2, i3) = (iid.clone(), iid.clone(), iid.clone());
        let rename: CfgCommit = Rc::new(move |this, v, cx| {
            let id = i2.clone();
            this.edit(noshape_ops::config::RENAME_INPUT, cx, |a| noshape_ops::asm_config::rename_input(a, &id, &v));
        });
        let name_cell = self.cfg_cell_text(format!("in-name-{iid}"), input.name.clone(), (px(16.), FontWeight::MEDIUM), true, rename.clone(), cx);
        let header = h_flex()
            .h(px(35.))
            .flex_none()
            .px(px(6.))
            .gap(px(4.))
            .bg(rgb(0xeaeaea))
            .border_t_1()
            .border_b_1()
            .border_l_1()
            .border_color(grid)
            .child(
                div()
                    .id(SharedString::from(format!("acfg-fold-{iid}")))
                    .w(px(12.))
                    .cursor_pointer()
                    .child(crate::ui::glyph(if folded { "ui-chevron-right" } else { "ui-chevron-down" }, 10., pal.text))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if !this.ui.cfg_folded.remove(&i1) {
                            this.ui.cfg_folded.insert(i1.clone());
                        }
                        cx.notify();
                    })),
            )
            .child(div().flex_none().max_w(px(160.)).child(name_cell))
            .child(div().flex_1().min_w_0().truncate().text_size(px(14.67)).font_weight(FontWeight::MEDIUM).text_color(rgb(0x999999)).child(format!("Default: {default_text}")))
            .when(configurable, |d| {
                d.child(
                    h_flex()
                        .id(SharedString::from(format!("acfg-configure-{iid}")))
                        .h(px(23.))
                        .pl(px(6.))
                        .pr(px(10.))
                        .gap(px(2.))
                        .bg(if picking { pal.panel_active } else { pal.panel })
                        .border_1()
                        .border_color(grid)
                        .rounded(px(3.5))
                        .cursor_pointer()
                        .when(picking, |d| d.opacity(0.5))
                        .hover(|d| d.bg(pal.panel_hover))
                        .tooltip(crate::ui::tooltip("Choose instances and mate parameters to be driven by this configuration input"))
                        .child(crate::ui::glyph("plus", 14., pal.text))
                        .child(div().text_size(px(13.)).text_color(pal.text).child("Configure assembly features"))
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                            if this.ui.cfg_mode.is_none() {
                                this.start_configure_features(i3.clone(), window, cx);
                            }
                        })),
                )
            })
            .child({
                // Input ⋯: Copy table, Copy input, Rename, Visibility
                // conditions…, Input propagation…, Delete (live 200).
                let (v, iid) = (view.clone(), iid.clone());
                div()
                    .id(SharedString::from(format!("acfg-more-{iid}")))
                    .size(px(25.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .child(crate::assets::icon("more", 20., cx))
                    .on_click(move |e: &ClickEvent, window, cx| {
                        let (v, iid) = (v.clone(), iid.clone());
                        let menu = crate::os_menu::OsMenu::build(window, cx, move |m, _, _| {
                            let (v1, v2, v3, r, d, c) = (v.clone(), v.clone(), v.clone(), iid.clone(), iid.clone(), iid.clone());
                            m.list_no_icons()
                                .item(crate::os_menu::OsMenuItem::new("Copy table").on_click(move |_, _, cx| v3.update(cx, |this, cx| this.copy_config_table(&c, cx))))
                                .item(crate::os_menu::OsMenuItem::new("Copy input").on_click(move |_, _, cx| v1.update(cx, |this, cx| this.duplicate_config_input(&r, cx))))
                                .item(crate::os_menu::OsMenuItem::new("Rename").on_click({
                                    let (v, r) = (v.clone(), iid.clone());
                                    move |_, window, cx| {
                                        v.update(cx, |this, cx| {
                                            let name = this.asm_configuration(cx).inputs.iter().find(|i| i.id == r).map(|i| i.name.clone()).unwrap_or_default();
                                            let rr = r.clone();
                                            this.edit_cfg_cell(
                                                format!("in-name-{r}"),
                                                name,
                                                Rc::new(move |this, v, cx| {
                                                    let id = rr.clone();
                                                    this.edit(noshape_ops::config::RENAME_INPUT, cx, |a| noshape_ops::asm_config::rename_input(a, &id, &v));
                                                }),
                                                window,
                                                cx,
                                            );
                                        })
                                    }
                                }))
                                // psb's dialogs, for this input (live 200).
                                .item(crate::os_menu::OsMenuItem::new("Visibility conditions…").on_click({
                                    let (v, r) = (v.clone(), iid.clone());
                                    move |_, window, cx| v.update(cx, |this, cx| this.open_asm_visibility(Some(r.clone()), window, cx))
                                }))
                                .item(crate::os_menu::OsMenuItem::new("Input propagation…").on_click({
                                    let (v, r) = (v.clone(), iid.clone());
                                    move |_, window, cx| v.update(cx, |this, cx| this.open_asm_input_propagation(Some(&r), window, cx))
                                }))
                                .separator()
                                .item(crate::os_menu::OsMenuItem::new("Delete").on_click(move |_, _, cx| {
                                    let d = d.clone();
                                    v2.update(cx, |this, cx| {
                                        this.edit(noshape_ops::config::DELETE_INPUT, cx, |a| a.delete_config_input(&d));
                                        this.ui.config_values.0.remove(&d);
                                        cx.notify();
                                    })
                                }))
                        });
                        crate::os_menu::open_anchored(e.position(), Anchor::TopRight, menu, window, cx);
                    })
            });
        let mut section = v_flex().mb(px(10.)).child(header);
        if folded {
            return section.into_any_element();
        }
        let cell = |w: f32| div().w(px(w)).h(px(ROW_H)).flex_none().px(px(3.)).flex().items_center().border_r_1().border_b_1().border_color(grid).text_size(px(14.));
        let head = |w: f32| cell(w).bg(rgb(0xeaeaea)).justify_center().font_weight(FontWeight::BOLD);
        // Column widths: live's grips (`ui::col_grip`), per input table.
        let table = format!("asm-config-{iid}");
        let name_w = crate::ui::col_w(&table, "name", NAME_W);
        let grip = |col: &str, default: f32| crate::ui::col_grip(&table, col, default);
        match &input.kind {
            ConfigInputKind::Variable { default, .. } => {
                let a = iid.clone();
                let commit: CfgCommit = Rc::new(move |this, v, cx| {
                    let a = a.clone();
                    this.edit(noshape_ops::config::EDIT_VARIABLE_INPUT, cx, |asm| noshape_ops::asm_config::set_variable_default(asm, &a, &v));
                });
                let value = self.cfg_cell_text(format!("var-def-{iid}"), default.text.clone(), (px(13.33), FontWeight::NORMAL), false, commit, cx);
                let value_w = crate::ui::col_w(&table, "default", COL_W);
                section = section.child(h_flex().border_l_1().border_color(grid).child(head(name_w).relative().child("Name").child(grip("name", NAME_W))).child(head(value_w).relative().child("Default value").child(grip("default", COL_W))));
                section = section.child(h_flex().border_l_1().border_color(grid).child(cell(name_w).child(format!("#{}", input.name))).child(cell(value_w).child(value)));
            }
            ConfigInputKind::Text { .. } => {}
            _ => {
                let rows: Vec<(String, String)> = match &input.kind {
                    ConfigInputKind::List { options, .. } => options.iter().map(|o| (o.id.clone(), o.name.clone())).collect(),
                    _ => vec![("true".into(), "Checked".into()), ("false".into(), "Unchecked".into())],
                };
                let columns: Vec<(usize, acfg::AsmConfiguredParam)> = cfg.configured.iter().cloned().enumerate().filter(|(_, c)| c.input == iid).collect();
                let col_key = |c: &acfg::AsmConfiguredParam| format!("{:?}-{:?}", c.target, c.param);
                let col_ws: Vec<f32> = columns.iter().map(|(_, c)| crate::ui::col_w(&table, &col_key(c), COL_W)).collect();
                // Two-level header: feature name spanning its parameters.
                let two_level = !columns.is_empty();
                let mut hrow = h_flex().items_start().border_l_1().border_color(grid).child(head(name_w).relative().h(px(if two_level { ROW_H * 2. } else { ROW_H })).child("Name").child(grip("name", NAME_W)));
                let mut i = 0;
                while i < columns.len() {
                    let t = columns[i].1.target;
                    let span = columns[i..].iter().take_while(|(_, c)| c.target == t).count();
                    let (fname, _) = self.cfg_column_names(t, &columns[i].1.param, cx);
                    let mut sub = h_flex();
                    for (k, (_, c)) in columns[i..i + span].iter().enumerate() {
                        sub = sub.child(head(col_ws[i + k]).relative().child(div().truncate().child(self.cfg_param_label(c.target, &c.param, cx))).child(grip(&col_key(c), COL_W)));
                    }
                    hrow = hrow.child(v_flex().child(head(col_ws[i..i + span].iter().sum()).child(div().truncate().child(fname))).child(sub));
                    i += span;
                }
                section = section.child(hrow);
                for (rk, rname) in &rows {
                    let active = *rk == current;
                    let is_list = matches!(input.kind, ConfigInputKind::List { .. });
                    let name_el: AnyElement = if is_list {
                        let (a, b) = (iid.clone(), rk.clone());
                        let commit: CfgCommit = Rc::new(move |this, v, cx| {
                            let (a, b) = (a.clone(), b.clone());
                            let v = v.trim().to_string();
                            if v.is_empty() {
                                return;
                            }
                            this.edit(noshape_ops::config::RENAME_OPTION, cx, |asm| noshape_ops::asm_config::rename_option(asm, &a, &b, &v));
                        });
                        self.cfg_cell_text(format!("opt-{iid}-{rk}"), rname.clone(), (px(13.33), if active { FontWeight::MEDIUM } else { FontWeight::NORMAL }), true, commit, cx)
                    } else {
                        rname.clone().into_any_element()
                    };
                    let (ia, rka, da, db, vr) = (iid.clone(), rk.clone(), iid.clone(), rk.clone(), view.clone());
                    let mut row = h_flex()
                        .id(SharedString::from(format!("acfg-row-{iid}-{rk}")))
                        .border_l_1()
                        .border_color(grid)
                        .child(
                            cell(name_w)
                                .relative()
                                .cursor_pointer()
                                .pl(px(6.))
                                .when(active, |d| d.text_color(rgb(0x1651b0)).child(div().absolute().left_0().top_0().bottom_0().w(px(3.)).bg(rgb(0x1651b0))))
                                .child(name_el),
                        )
                        // Clicking a row makes it the current configuration.
                        .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
                            if e.click_count() == 1 {
                                this.set_active_config(&ia, rka.clone(), window, cx);
                            }
                        }))
                        .when(is_list, |d| {
                            d.os_context_menu(move |m, _, _| {
                                let (v, a, b) = (vr.clone(), da.clone(), db.clone());
                                m.item(crate::os_menu::OsMenuItem::new("Delete").icon_name("menu-delete").on_click(move |_, _, cx| {
                                    let (a, b) = (a.clone(), b.clone());
                                    v.update(cx, |this, cx| {
                                        this.edit(noshape_ops::config::DELETE_OPTION, cx, |asm| noshape_ops::asm_config::delete_option(asm, &a, &b));
                                    })
                                }))
                            })
                        });
                    for (k, (ci, c)) in columns.iter().enumerate() {
                        let el = self.cfg_value_cell(*ci, c, rk, cx);
                        row = row.child(cell(col_ws[k]).child(el));
                    }
                    section = section.child(row);
                }
                // The italic "Name" row adds a configuration (List).
                if matches!(input.kind, ConfigInputKind::List { .. }) {
                    let a = iid.clone();
                    let key = format!("opt-new-{iid}");
                    let commit: CfgCommit = Rc::new(move |this, v, cx| {
                        let name = v.trim().to_string();
                        if name.is_empty() {
                            return;
                        }
                        let a = a.clone();
                        this.edit(noshape_ops::config::ADD_OPTION, cx, |asm| noshape_ops::asm_config::add_option(asm, &a, &name));
                    });
                    let editing = self.ui.cfg_edit.as_ref().is_some_and(|e| e.key == key);
                    let el = self.cfg_cell_text(key, String::new(), (px(13.33), FontWeight::NORMAL), false, commit, cx);
                    section = section.child(
                        h_flex().border_l_1().border_color(grid).child(
                            cell(name_w)
                                .relative()
                                .child(el)
                                .when(!editing, |d| d.child(div().absolute().left(px(6.)).top_0().bottom_0().flex().items_center().italic().text_size(px(14.)).text_color(pal.text_faint).child("Name"))),
                        ),
                    );
                }
                if columns.is_empty() {
                    section = section.child(
                        div().pt(px(5.)).pr(px(5.)).child(
                            v_flex()
                                .p(px(14.))
                                .rounded(px(5.25))
                                .bg(pal.info_fill)
                                .border_1()
                                .border_color(pal.info_outline)
                                .text_size(px(14.))
                                .line_height(px(21.))
                                .text_color(pal.info_text)
                                .child(div().font_weight(FontWeight::BOLD).child("No assembly features are configured"))
                                .child("Right click patterns, mates, or instances to configure them."),
                        ),
                    );
                }
            }
        }
        section.into_any_element()
    }

    /// A configured value: a checkbox (Unsuppressed, Offset) or an
    /// editable expression.
    fn cfg_value_cell(&mut self, ci: usize, c: &acfg::AsmConfiguredParam, key: &str, cx: &mut Context<Self>) -> AnyElement {
        let val = c.values.get(key).cloned();
        let kk = key.to_string();
        if let Some(ParamValue::Bool(b)) = val {
            return div()
                .flex_1()
                .flex()
                .justify_center()
                .child(crate::ui::os_checkbox(SharedString::from(format!("acv-{ci}-{key}")), "", b).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    let kk = kk.clone();
                    this.edit(noshape_ops::config::EDIT_CONFIGURED_VALUE, cx, |a| noshape_ops::asm_config::set_cell(a, ci, &kk, ParamValue::Bool(!b)));
                })))
                .into_any_element();
        }
        // A propagated input: the instance's option, chosen from its own list.
        if let (Some(ParamValue::Text(t)), Some(input), ConfigTarget::Instance(iid)) = (&val, c.param.strip_prefix(acfg::param::INPUT_PREFIX), c.target) {
            let options: Vec<(String, String)> = match self.instance_source_input(iid, input, cx).map(|i| i.kind) {
                Some(ConfigInputKind::List { options, .. }) => options.into_iter().map(|o| (o.id, o.name)).collect(),
                Some(ConfigInputKind::Checkbox { .. }) => vec![("true".into(), "Checked".into()), ("false".into(), "Unchecked".into())],
                _ => Vec::new(),
            };
            let label = options.iter().find(|(id, _)| id == t).map(|(_, n)| n.clone()).unwrap_or_else(|| t.clone());
            let view = cx.entity();
            return div()
                .id(SharedString::from(format!("acv-sel-{ci}-{key}")))
                .flex_1()
                .h_full()
                .px(px(3.))
                .flex()
                .items_center()
                .cursor_pointer()
                .child(h_flex().w_full().child(div().flex_1().truncate().text_size(px(14.)).child(label)).child(crate::os_ui::glyph(crate::os_ui::GLYPH_CARET, 8., rgb(0x333333))))
                .os_dropdown_menu(SharedString::from(format!("acv-dd-{ci}-{key}")), true, move |mut menu, _, _| {
                    for (id, name) in options.clone() {
                        let (v, kk) = (view.clone(), kk.clone());
                        menu = menu.item(crate::os_menu::OsMenuItem::new(name).on_click(move |_, _, cx| {
                            let (kk, id) = (kk.clone(), id.clone());
                            v.update(cx, |this, cx| {
                                this.edit(noshape_ops::config::EDIT_CONFIGURED_VALUE, cx, |a| noshape_ops::asm_config::set_cell(a, ci, &kk, ParamValue::Text(id)));
                            })
                        }));
                    }
                    menu
                })
                .into_any_element();
        }
        let text = match &val {
            Some(ParamValue::Expr(e)) => e.text.clone(),
            _ => String::new(),
        };
        let commit: CfgCommit = Rc::new(move |this, v, cx| {
            let kk = kk.clone();
            this.edit(noshape_ops::config::EDIT_CONFIGURED_VALUE, cx, |a| noshape_ops::asm_config::set_cell(a, ci, &kk, ParamValue::Expr(ExprValue::new(v.trim()))));
        });
        self.cfg_cell_text(format!("cv-{ci}-{key}"), text, (px(13.33), FontWeight::NORMAL), false, commit, cx)
    }

    fn copy_config_table(&mut self, input: &str, cx: &mut Context<Self>) {
        let cfg = self.asm_configuration(cx);
        let Some(inp) = cfg.inputs.iter().find(|i| i.id == input) else { return };
        let rows: Vec<(String, String)> = match &inp.kind {
            ConfigInputKind::List { options, .. } => options.iter().map(|o| (o.id.clone(), o.name.clone())).collect(),
            ConfigInputKind::Checkbox { .. } => vec![("true".into(), "Checked".into()), ("false".into(), "Unchecked".into())],
            _ => Vec::new(),
        };
        let cols: Vec<&acfg::AsmConfiguredParam> = cfg.configured.iter().filter(|c| c.input == input).collect();
        let mut out = String::from("Name");
        for c in &cols {
            let (f, p) = self.cfg_column_names(c.target, &c.param, cx);
            out.push_str(&format!("\t{f} {p}"));
        }
        out.push('\n');
        for (k, n) in rows {
            out.push_str(&n);
            for c in &cols {
                let v = match c.values.get(&k) {
                    Some(ParamValue::Bool(b)) => b.to_string(),
                    Some(ParamValue::Expr(e)) => e.text.clone(),
                    _ => String::new(),
                };
                out.push('\t');
                out.push_str(&v);
            }
            out.push('\n');
        }
        cx.write_to_clipboard(ClipboardItem::new_string(out));
    }

    fn duplicate_config_input(&mut self, input: &str, cx: &mut Context<Self>) {
        self.edit(noshape_ops::asm_config::COPY_INPUT, cx, |a| noshape_ops::asm_config::duplicate_input(a, input));
    }

    fn render_add_asm_config_input(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let border = rgb(0xaaaaaa);
        let view = cx.entity();
        let main = div()
            .id("acfg-add-input")
            .h(px(28.))
            .px(px(10.))
            .flex()
            .items_center()
            .gap(px(6.))
            .cursor_pointer()
            .hover(|d| d.bg(pal.panel_hover))
            .child(crate::assets::icon("cfg-filter-list", 20., cx))
            .child(div().text_size(px(14.)).text_color(pal.text).child("Add configuration input"))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.add_asm_config_input("List", cx)));
        let caret = div()
            .id("acfg-add-input-more")
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
            .on_click(move |e: &ClickEvent, window, cx| Self::kinds_menu(view.clone(), e.position(), Anchor::BottomLeft, window, cx));
        h_flex().rounded(px(3.)).border_1().border_color(border).bg(pal.panel).child(main).child(caret).into_any_element()
    }

    /// The docked 712 px Configuration panel.
    /// The Configurations panel's "Filter inputs" field (made on first use).
    pub(super) fn ensure_cfg_filter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.ui.cfg_filter.is_some() {
            return;
        }
        let f = cx.new(|cx| InputState::new(window, cx));
        self.ui.cfg_filter_sub = Some(cx.subscribe(&f, |_, _, ev: &InputEvent, cx| {
            if matches!(ev, InputEvent::Change) {
                cx.notify();
            }
        }));
        self.ui.cfg_filter = Some(f);
    }

    pub(super) fn render_asm_config_panel(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let cfg = self.asm_configuration(cx);
        let empty = cfg.inputs.is_empty();
        let tab = |id: &'static str, label: &'static str, on: bool, cx: &mut Context<Self>| {
            div()
                .id(id)
                .flex_1()
                .h(px(26.))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .text_size(px(13.33))
                .font_weight(FontWeight::MEDIUM)
                .text_color(if on { rgb(0x1651b0) } else { rgb(0x333333) })
                .when(on, |d| d.border_b_2().border_color(rgb(0x1651b0)))
                .child(label)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.ui.cfg_props_tab = id == "acfg-tab-props";
                    cx.notify();
                }))
        };
        let title = if empty {
            div()
                .h(px(26.))
                .flex()
                .items_center()
                .justify_center()
                .border_b_2()
                .border_color(rgb(0x1651b0))
                .text_size(px(13.33))
                .font_weight(FontWeight::MEDIUM)
                .text_color(rgb(0x1651b0))
                .child("Configurations")
                .into_any_element()
        } else {
            h_flex().child(tab("acfg-tab-configs", "Configurations", !self.ui.cfg_props_tab, cx)).child(tab("acfg-tab-props", "Configured assembly properties", self.ui.cfg_props_tab, cx)).into_any_element()
        };
        let body: AnyElement = if empty {
            self.render_asm_config_empty(cx)
        } else if self.ui.cfg_props_tab {
            self.render_cfg_properties(&cfg, cx)
        } else {
            // Filter row as in the Part Studio panel (the same Onshape
            // component): funnel, "Filter inputs" (name filter), the kind
            // toggles "Show only … inputs", and ⋯ (Copy all inputs,
            // Collapse / Expand all inputs).
            let query = self.ui.cfg_filter.as_ref().map(|f| f.read(cx).value().trim().to_lowercase()).unwrap_or_default();
            let filter_el: AnyElement = match &self.ui.cfg_filter {
                Some(f) => div()
                    .flex_1()
                    .child(gpui_kit::base::input::input_frame(
                        div()
                            .relative()
                            .h(px(24.))
                            .border_1()
                            .border_color(pal.border_strong)
                            .rounded(px(2.))
                            .flex()
                            .items_center()
                            .child(crate::ui::input_with_placeholder(f, "Filter inputs", cx).appearance(false).small().text_size(px(13.)).pl(px(6.))),
                    ))
                    .into_any_element(),
                None => div().flex_1().into_any_element(),
            };
            let kinds = [("cfg-filter-list", "list"), ("cfg-filter-checkbox", "checkbox"), ("cfg-filter-variable", "configuration variable")];
            let mut row = h_flex().h(px(34.)).gap(px(6.)).px(px(4.)).child(crate::ui::glyph("filter", 16., pal.text)).child(filter_el);
            for (k, (icon, what)) in kinds.into_iter().enumerate() {
                let only = self.ui.cfg_only_kinds[k];
                row = row.child(
                    div()
                        .id(SharedString::from(format!("acfg-kind-{k}")))
                        .size(px(26.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(3.))
                        .cursor_pointer()
                        .when(only, |d| d.bg(pal.panel_active))
                        .hover(|d| d.bg(pal.panel_hover))
                        .tooltip(crate::ui::tooltip(format!("Show only {what} inputs")))
                        .child(crate::assets::icon(icon, 20., cx))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.ui.cfg_only_kinds[k] = !this.ui.cfg_only_kinds[k];
                            cx.notify();
                        })),
                );
            }
            let ids: Vec<String> = cfg.inputs.iter().map(|i| i.id.clone()).collect();
            let all_folded = !ids.is_empty() && ids.iter().all(|id| self.ui.cfg_folded.contains(id));
            let view = cx.entity();
            row = row.child(
                div().id("acfg-panel-more").size(px(26.)).flex().items_center().justify_center().cursor_pointer().child(crate::assets::icon("more", 20., cx)).on_click(move |e: &ClickEvent, window, cx| {
                    let (view, ids) = (view.clone(), ids.clone());
                    let menu = crate::os_menu::OsMenu::build(window, cx, move |m, _, _| {
                        let (v1, v2, ids) = (view.clone(), view.clone(), ids.clone());
                        let several = ids.len() > 1;
                        let mut m = m.list_no_icons().item(crate::os_menu::OsMenuItem::new("Copy all inputs").on_click(move |_, _, cx| {
                            v1.update(cx, |this, cx| {
                                crate::config_visibility::copy_inputs(this.asm_configuration(cx).inputs.clone());
                                this.show_cfg_toast("Inputs copied.".into(), true, cx);
                            })
                        }));
                        if several {
                            m = m.item(crate::os_menu::OsMenuItem::new(if all_folded { "Expand all inputs" } else { "Collapse all inputs" }).on_click(move |_, _, cx| {
                                let ids = ids.clone();
                                v2.update(cx, |this, cx| {
                                    if all_folded {
                                        this.ui.cfg_folded.clear();
                                    } else {
                                        this.ui.cfg_folded.extend(ids);
                                    }
                                    cx.notify();
                                })
                            }));
                        }
                        // Live (Part Studio panel 434): then Visibility
                        // conditions… and Input propagation… for every input.
                        let (v3, v4) = (view.clone(), view.clone());
                        m.item(crate::os_menu::OsMenuItem::new("Visibility conditions…").on_click(move |_, window, cx| v3.update(cx, |this, cx| this.open_asm_visibility(None, window, cx))))
                            .item(crate::os_menu::OsMenuItem::new("Input propagation…").on_click(move |_, window, cx| v4.update(cx, |this, cx| this.open_asm_input_propagation(None, window, cx))))
                    });
                    crate::os_menu::open(e.position(), menu, window, cx);
                }),
            );
            let mut col = v_flex().child(row);
            let only = self.ui.cfg_only_kinds;
            for input in cfg.inputs.clone() {
                let kind_ix = match input.kind {
                    ConfigInputKind::List { .. } => 0,
                    ConfigInputKind::Checkbox { .. } => 1,
                    _ => 2,
                };
                if (only.iter().any(|o| *o) && !only[kind_ix]) || (!query.is_empty() && !input.name.to_lowercase().contains(&query)) {
                    continue;
                }
                col = col.child(self.render_asm_config_input(&cfg, &input, cx));
            }
            col.into_any_element()
        };
        crate::ui::right_resizable(v_flex().id("acfg-pane").relative(), 0.)
            .w(px(crate::ui::right_panel_w()))
            .h_full()
            .occlude()
            .bg(rgb(0xffffff))
            .text_color(rgb(0x333333))
            .child(div().pt(px(4.)).child(title))
            .child(v_flex().id("acfg-body").flex_1().min_h_0().overflow_y_scroll().child(body))
            .child(
                h_flex()
                    .h(px(40.))
                    .flex_none()
                    .bg(rgb(0xeeeeee))
                    .border_t_1()
                    .border_l_1()
                    .border_color(rgb(0xcccccc))
                    .child(div().flex_1().flex().justify_center().when(!empty, |d| d.child(self.render_add_asm_config_input(cx))))
                    .child(div().pr(px(10.)).child(div().size(px(14.)).rounded_full().flex().items_center().justify_center().text_size(px(11.)).font_weight(FontWeight::BOLD).child(crate::ui::glyph("help-dot", 15.6, crate::theme::help_dot(0xaaaaaa))))),
            )
            .into_any_element()
    }

    /// "Configured assembly properties": per configuration, the assembly's
    /// name in it.
    fn render_cfg_properties(&self, cfg: &acfg::AsmConfiguration, cx: &App) -> AnyElement {
        let grid = rgb(0xaaaaaa);
        let name = self.doc.read(cx).session.doc.tab(self.tab).map(|t| t.name.clone()).unwrap_or_default();
        let cell = |w: f32| div().w(px(w)).h(px(34.)).flex_none().px(px(3.)).flex().items_center().border_r_1().border_b_1().border_color(grid).text_size(px(14.));
        let mut col = v_flex().pt(px(6.)).child(h_flex().border_l_1().border_t_1().border_color(grid).child(cell(180.).bg(rgb(0xeaeaea)).justify_center().font_weight(FontWeight::BOLD).child("Configuration")).child(cell(180.).bg(rgb(0xeaeaea)).justify_center().font_weight(FontWeight::BOLD).child("Name")));
        if let Some(ConfigInput { kind: ConfigInputKind::List { options, .. }, .. }) = cfg.inputs.first() {
            for o in options {
                col = col.child(h_flex().border_l_1().border_color(grid).child(cell(180.).child(crate::ui::user_text(o.name.clone()))).child(cell(180.).child(crate::ui::user_text(name.clone()))));
            }
        }
        col.into_any_element()
    }

    /// The "Configurations [▦][▾]" box above the instance list (live 191,
    /// dom/panel-configuration-left-box.txt): one "Configuration ▾ Default"
    /// row per input.
    pub(super) fn render_asm_config_box(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let pal = p();
        let cfg = self.asm_configuration(cx);
        if cfg.inputs.is_empty() {
            return None;
        }
        let border = rgb(0xaaaaaa);
        let view = cx.entity();
        let split = h_flex()
            .rounded(px(3.))
            .border_1()
            .border_color(border)
            .child(
                div()
                    .id("acfgbox-panel")
                    .w(px(35.))
                    .h(px(28.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .hover(|d| d.bg(pal.panel_hover))
                    .child(crate::assets::icon("configuration-visibiliity", 20., cx))
                    .tooltip(crate::ui::tooltip("Modify visibility conditions"))
                    // Live: the Configuration visibility conditions dialog for
                    // every input (psb's config_visibility).
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.open_asm_visibility(None, window, cx))),
            )
            .child(
                div()
                    .id("acfgbox-more")
                    .w(px(20.))
                    .h(px(28.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .border_l_1()
                    .border_color(border)
                    .cursor_pointer()
                    .hover(|d| d.bg(pal.panel_hover))
                    .child(crate::ui::glyph("ui-dropdown-arrow", 10., gpui_kit::rgb(0x666666)))
                    .on_click(move |e: &ClickEvent, window, cx| Self::kinds_menu(view.clone(), e.position(), Anchor::TopLeft, window, cx)),
            );
        let mut col = v_flex()
            .flex_none()
            .px(px(4.))
            .pt(px(4.))
            .pb(px(6.))
            .border_b_1()
            .border_color(pal.border)
            .child(h_flex().h(px(30.)).justify_between().child(div().text_size(px(14.)).font_weight(FontWeight::BOLD).text_color(pal.text).child("Configurations")).child(split));
        // Visibility conditions: hidden inputs and options don't show.
        let (shown, effective) = cfg.inputs_only().visibility(&self.ui.config_values);
        for (input, vis) in cfg.inputs.iter().zip(&shown) {
            if !vis.visible {
                continue;
            }
            let current = effective.0.get(&input.id).cloned().unwrap_or_default();
            let iid = input.id.clone();
            let control: AnyElement = match &input.kind {
                ConfigInputKind::List { options, .. } => {
                    let options: Vec<_> = options.iter().filter(|o| vis.options.contains(&o.id)).collect();
                    let ix = options.iter().position(|o| o.id == current).unwrap_or(0);
                    let ids: Vec<String> = options.iter().map(|o| o.id.clone()).collect();
                    let v = cx.entity();
                    crate::os_ui::user_select_field(SharedString::from(format!("acfgbox-sel-{iid}")), options.iter().map(|o| o.name.clone()).collect(), ix, move |i, window, cx| {
                        let (iid, id) = (iid.clone(), ids[i].clone());
                        v.update(cx, |this, cx| this.set_active_config(&iid, id, window, cx));
                    })
                    .h(px(26.))
                    .border_0()
                    .into_any_element()
                }
                ConfigInputKind::Checkbox { .. } => {
                    let on = current == "true";
                    crate::ui::os_checkbox(SharedString::from(format!("acfgbox-cb-{iid}")), "", on)
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.set_active_config(&iid, (!on).to_string(), window, cx)))
                        .into_any_element()
                }
                ConfigInputKind::Variable { .. } | ConfigInputKind::Text { .. } => div().text_size(px(14.)).text_color(pal.text).child(current).into_any_element(),
            };
            col = col.child(
                h_flex()
                    .min_h(px(28.))
                    .gap(px(4.))
                    .child(div().flex_none().pl(px(3.)).text_size(px(12.)).text_color(pal.text_muted).truncate().child(crate::ui::user_text(input.name.clone())))
                    .child(div().flex_1().min_w_0().child(control)),
            );
        }
        Some(col.into_any_element())
    }
}
