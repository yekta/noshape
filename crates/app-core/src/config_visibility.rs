//! Onshape's "Configuration visibility conditions" dialog (Help
//! PartStudio__visibility_conditions; bundle templates
//! visibilityConditionsDialog.html, visibilityConditionStatement.html and
//! conditionBuilder.html, their CSS `.edit-config-param-visibility-dialog-container`).
//!
//! Left: one section per input (the first one, which nothing can control,
//! is a plain row "These configuration inputs will always be visible"), each
//! with its "Show <input> When …" conditions and, for lists, option rules
//! ("Show options | range … Always | When … And …"). Right: a live preview
//! of the configuration with ☑ Show hidden inputs. Footer: ?, undo, redo,
//! Save (disabled until something valid changed), Cancel. Only lists and
//! checkboxes above an input can control it.

use crate::os_ui as os;
use crate::theme::p;
use gpui_kit::component::Disableable as _;
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::configuration::{Condition, ConditionTest, ConfigInputKind, ConfigValues, Configuration, OptionRule, ShownOptions, Visibility};
use std::collections::HashSet;
use std::rc::Rc;

type OnSave = Rc<dyn Fn(Configuration, &mut Window, &mut App)>;
type Act = Rc<dyn Fn(&mut VisibilityDialog)>;

/// Bootstrap / Onshape colours used here.
const BORDER: u32 = 0xaaaaaa; // --os-table-border--idle
const HEADER_FILL: u32 = 0xeaeaea; // --os-table-header-fill--idle
const FOCUS: u32 = 0x3e94ff; // --os-focus
const ERROR: u32 = 0xbd3039;
const TITLE: u32 = 0x1651b0;
/// Above the dialog (the kit draws dialogs at popup priority 100).
const POPOVER: usize = 101;

/// A condition: the input's own statement (`rule` None) or an option rule's.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct At {
    input: usize,
    rule: Option<usize>,
    cond: usize,
}

/// The open dropdown.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Menu {
    Filter,
    More,
    CondInput(At),
    CondOp(At),
    CondValues(At),
    CondStart(At),
    CondEnd(At),
    When(usize, usize),
    RuleShow(usize, usize),
    RuleOptions(usize, usize),
    RuleStart(usize, usize),
    RuleEnd(usize, usize),
    Preview(usize),
}

/// A `.simple-select` button: its id, text, width, whether it can open and
/// which menu it opens.
struct SelectSpec {
    id: SharedString,
    label: String,
    width: f32,
    enabled: bool,
    menu: Menu,
}

enum Item {
    /// A choice; closes the menu.
    Choice { label: String, selected: bool, disabled: bool, act: Act },
    /// A checkbox row; the menu stays open.
    Check { label: String, checked: bool, act: Act },
    Separator,
}

pub struct VisibilityDialog {
    cfg: Configuration,
    original: Configuration,
    /// The Inputs filter: one input, or all of them.
    current: Option<String>,
    collapsed: HashSet<String>,
    rules_collapsed: HashSet<(String, usize)>,
    preview: ConfigValues,
    show_hidden: bool,
    menu: Option<Menu>,
    undo: Vec<Configuration>,
    redo: Vec<Configuration>,
    on_save: OnSave,
}

/// Open the dialog for every input (`input` None: the Configurations box's
/// "Modify visibility conditions") or one input (its ⋯ › Visibility
/// conditions… or its visibility icon). `on_save` gets the configuration
/// with the edited conditions.
pub fn open(cfg: Configuration, values: ConfigValues, input: Option<String>, on_save: impl Fn(Configuration, &mut Window, &mut App) + 'static, window: &mut Window, cx: &mut App) {
    let preview = cfg.effective(&values);
    let view = cx.new(|_| VisibilityDialog {
        original: cfg.clone(),
        cfg,
        current: input,
        collapsed: HashSet::new(),
        rules_collapsed: HashSet::new(),
        preview,
        show_hidden: true,
        menu: None,
        undo: Vec::new(),
        redo: Vec::new(),
        on_save: Rc::new(on_save),
    });
    window.open_dialog(cx, move |dialog, _, _| dialog.w(px(1145.)).margin_top(px(40.)).close_button(false).p_0().gap_0().rounded(px(4.)).child(view.clone()));
}

fn blank() -> Condition {
    Condition { input: String::new(), test: ConditionTest::Is(Vec::new()) }
}

impl VisibilityDialog {
    fn name_of(&self, id: &str) -> String {
        self.cfg.inputs.iter().find(|i| i.id == id).map(|i| i.name.clone()).unwrap_or_default()
    }

    fn options_of(&self, id: &str) -> Vec<(String, String)> {
        match self.cfg.inputs.iter().find(|i| i.id == id).map(|i| &i.kind) {
            Some(ConfigInputKind::List { options, .. }) => options.iter().map(|o| (o.id.clone(), o.name.clone())).collect(),
            _ => Vec::new(),
        }
    }

    fn option_name(&self, input: &str, id: &str) -> String {
        self.options_of(input).into_iter().find(|(o, _)| o == id).map(|(_, n)| n).unwrap_or_default()
    }

    fn is_list(&self, k: usize) -> bool {
        matches!(self.cfg.inputs.get(k).map(|i| &i.kind), Some(ConfigInputKind::List { .. }))
    }

    fn is_checkbox_id(&self, id: &str) -> bool {
        matches!(self.cfg.inputs.iter().find(|i| i.id == id).map(|i| &i.kind), Some(ConfigInputKind::Checkbox { .. }))
    }

    /// Inputs nothing can control (bundle getIdsForAlwaysVisibleParams): the
    /// leading configuration variables and the first other input.
    fn always_visible(&self) -> Vec<usize> {
        let mut out = Vec::new();
        for (k, i) in self.cfg.inputs.iter().enumerate() {
            out.push(k);
            if !matches!(i.kind, ConfigInputKind::Variable { .. } | ConfigInputKind::Text { .. }) {
                break;
            }
        }
        out
    }

    fn vis(&self, k: usize) -> Visibility {
        self.cfg.inputs.get(k).and_then(|i| i.visible_when.clone()).unwrap_or_default()
    }

    fn conditions(&self, at: At) -> Vec<Condition> {
        let v = self.vis(at.input);
        match at.rule {
            None => v.conditions,
            Some(r) => v.option_rules.get(r).map(|r| r.conditions.clone()).unwrap_or_default(),
        }
    }

    fn condition(&self, at: At) -> Condition {
        self.conditions(at).get(at.cond).cloned().unwrap_or_else(blank)
    }

    /// Lists and checkboxes above input `k`, minus those the other
    /// conditions of the same statement use (getValidConfigParamsInternal).
    fn controllers(&self, at: At) -> Vec<usize> {
        let used: Vec<String> = self.conditions(at).iter().enumerate().filter(|(c, _)| *c != at.cond).map(|(_, c)| c.input.clone()).collect();
        (0..at.input).filter(|&k| matches!(self.cfg.inputs[k].kind, ConfigInputKind::List { .. } | ConfigInputKind::Checkbox { .. }) && !used.contains(&self.cfg.inputs[k].id)).collect()
    }

    /// One undoable change to input `k`'s conditions.
    fn change(&mut self, k: usize, f: impl FnOnce(&mut Visibility)) {
        self.undo.push(self.cfg.clone());
        self.redo.clear();
        let Some(input) = self.cfg.inputs.get_mut(k) else { return };
        let mut v = input.visible_when.take().unwrap_or_default();
        f(&mut v);
        input.visible_when = (!v.conditions.is_empty() || !v.option_rules.is_empty()).then_some(v);
        self.menu = None;
    }

    fn change_cond(&mut self, at: At, f: impl FnOnce(&mut Condition)) {
        self.change(at.input, |v| {
            let list = match at.rule {
                None => &mut v.conditions,
                Some(r) => match v.option_rules.get_mut(r) {
                    Some(r) => &mut r.conditions,
                    None => return,
                },
            };
            if let Some(c) = list.get_mut(at.cond) {
                f(c);
            }
        });
    }

    fn remove_cond(&mut self, at: At) {
        self.change(at.input, |v| match at.rule {
            None => {
                if at.cond < v.conditions.len() {
                    v.conditions.remove(at.cond);
                }
            }
            Some(r) => {
                let Some(rule) = v.option_rules.get_mut(r) else { return };
                // Removing a rule's only condition removes the rule.
                if rule.conditions.len() <= 1 {
                    v.option_rules.remove(r);
                } else {
                    rule.conditions.remove(at.cond);
                }
            }
        });
    }

    fn condition_complete(&self, c: &Condition) -> bool {
        !c.input.is_empty()
            && match &c.test {
                ConditionTest::Is(v) => !v.is_empty(),
                ConditionTest::Between(a, b) => !a.is_empty() && !b.is_empty(),
            }
    }

    /// Every statement is complete (areAllConditionsValid).
    fn valid(&self) -> bool {
        self.cfg.inputs.iter().all(|i| {
            i.visible_when.as_ref().is_none_or(|v| {
                v.conditions.iter().all(|c| self.condition_complete(c))
                    && v.option_rules.iter().all(|r| {
                        let shown = match &r.show {
                            ShownOptions::Unset => false,
                            ShownOptions::Options(o) => !o.is_empty(),
                            ShownOptions::Range(a, b) => !a.is_empty() && !b.is_empty(),
                        };
                        shown && (r.always || (!r.conditions.is_empty() && r.conditions.iter().all(|c| self.condition_complete(c))))
                    })
            })
        })
    }

    /// The configuration without its incomplete conditions and rules.
    fn complete_only(&self) -> Configuration {
        let mut cfg = self.cfg.clone();
        for i in &mut cfg.inputs {
            if let Some(v) = &mut i.visible_when {
                v.conditions.retain(|c| self.condition_complete(c));
                v.option_rules.retain(|r| !matches!(r.show, ShownOptions::Unset) && (r.always || r.conditions.iter().all(|c| self.condition_complete(c))));
            }
        }
        cfg
    }

    fn values_text(&self, c: &Condition) -> String {
        match &c.test {
            ConditionTest::Is(v) if self.is_checkbox_id(&c.input) => v.first().cloned().unwrap_or_default(),
            ConditionTest::Is(v) => self.options_of(&c.input).into_iter().filter(|(o, _)| v.contains(o)).map(|(_, n)| n).collect::<Vec<_>>().join(", "),
            ConditionTest::Between(a, b) => format!("{}, {}", self.option_name(&c.input, a), self.option_name(&c.input, b)),
        }
    }

    /// getTextRepresentation: "Shown when Height is Tall and Width is Wide"
    /// for an input, "when …" / "always" for an option rule; "" while
    /// something is incomplete.
    fn statement_text(&self, conds: &[Condition], input_statement: bool, always: bool) -> String {
        if always {
            return "always".into();
        }
        let mut out = String::new();
        for (k, c) in conds.iter().enumerate() {
            if !self.condition_complete(c) {
                return String::new();
            }
            let name = self.name_of(&c.input);
            let value = self.values_text(c);
            let between = matches!(c.test, ConditionTest::Between(..));
            out.push_str(&match (k, input_statement, between) {
                (0, true, false) => format!("Shown when {name} is {value}"),
                (0, true, true) => format!("Shown when {name} is between {value}"),
                (0, false, false) => format!("when {name} is {value}"),
                (0, false, true) => format!("when {name} is between {value}"),
                (_, _, false) => format!(" and {name} is {value}"),
                (_, _, true) => format!(" and {name} is between {value}"),
            });
        }
        out
    }

    /// A list with option rules where some options have none.
    fn uncovered_options(&self, k: usize) -> bool {
        let v = self.vis(k);
        if v.option_rules.is_empty() {
            return false;
        }
        let ids: Vec<String> = self.options_of(&self.cfg.inputs[k].id).into_iter().map(|(o, _)| o).collect();
        ids.iter().any(|o| {
            !v.option_rules.iter().any(|r| match &r.show {
                ShownOptions::Unset => false,
                ShownOptions::Options(l) => l.contains(o),
                ShownOptions::Range(a, b) => {
                    let at = |x: &str| ids.iter().position(|y| y == x);
                    matches!((at(a), at(b), at(o)), (Some(a), Some(b), Some(i)) if a.min(b) <= i && i <= a.max(b))
                }
            })
        })
    }

    // ---------------------------------------------------------------- widgets

    /// Onshape's `.simple-select` (150 × 34, 1 px #aaa, radius 4; 2 px
    /// #3e94ff while its menu is open; red when invalid; half opacity
    /// disabled) with its Bootstrap dropdown under it.
    fn select(&self, spec: SelectSpec, items: impl FnOnce() -> Vec<Item>, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let SelectSpec { id, label, width, enabled, menu } = spec;
        let invalid = false;
        let open = self.menu == Some(menu);
        let button = h_flex()
            .id(id)
            .w(px(width))
            .h(px(34.))
            .m(px(1.))
            .pl(px(10.))
            .pr(px(8.))
            .gap(px(6.))
            .justify_between()
            .rounded(px(4.))
            .border_1()
            .border_color(gpui_kit::rgb(if invalid { ERROR } else { BORDER }))
            .when(open, |d| d.border_2().border_color(gpui_kit::rgb(FOCUS)).pl(px(9.)))
            .text_size(px(14.))
            .text_color(if invalid { gpui_kit::rgb(ERROR) } else { pal.text })
            .when(!enabled, |d| d.opacity(0.5).cursor_not_allowed())
            .when(enabled, |d| d.cursor_pointer())
            .child(div().flex_1().min_w_0().truncate().child(label))
            .child(crate::ui::glyph("ui-caret-down", 9., pal.text))
            .when(enabled, |d| {
                d.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.menu = if this.menu == Some(menu) { None } else { Some(menu) };
                    cx.notify();
                }))
            });
        let mut wrap = div().relative().child(button);
        if open {
            wrap = wrap.child(div().absolute().top(px(36.)).left_0().child(deferred(anchored().child(self.dropdown(width, items(), cx))).with_priority(POPOVER)));
        }
        wrap.into_any_element()
    }

    /// The Bootstrap dropdown-menu: white, 1 px #ccc, radius 4, a shadow,
    /// rows 14 px with #e9ecef hover, at most 175 px tall.
    fn dropdown(&self, width: f32, items: Vec<Item>, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let rows: Vec<AnyElement> = items
            .into_iter()
            .enumerate()
            .map(|(k, it)| match it {
                Item::Separator => div().h(px(1.)).my(px(4.)).bg(gpui_kit::rgb(0xd7d7d7)).into_any_element(),
                Item::Choice { label, selected, disabled, act } => div()
                    .id(("vis-item", k))
                    .px(px(16.))
                    .py(px(4.))
                    .text_size(px(14.))
                    .truncate()
                    .text_color(if disabled { pal.text_faint } else if selected { gpui_kit::rgb(0xffffff) } else { pal.text })
                    .when(selected, |d| d.bg(gpui_kit::rgb(TITLE)))
                    .when(!selected && !disabled, |d| d.hover(|d| d.bg(gpui_kit::rgb(0xe9ecef))))
                    .when(!disabled, |d| d.cursor_pointer())
                    .child(label)
                    .when(!disabled, |d| {
                        d.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            act(this);
                            this.menu = None;
                            cx.notify();
                        }))
                    })
                    .into_any_element(),
                Item::Check { label, checked, act } => crate::ui::os_checkbox(("vis-check", k), label, checked)
                    .w_full()
                    .pl(px(10.))
                    .text_size(px(14.))
                    .text_color(pal.text)
                    .hover(|d| d.bg(gpui_kit::rgb(0xe9ecef)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        act(this);
                        cx.notify();
                    }))
                    .into_any_element(),
            })
            .collect();
        v_flex()
            .id("vis-dropdown")
            .occlude()
            .min_w(px(width.max(85.)))
            .max_h(px(175.))
            .overflow_y_scroll()
            .py(px(4.))
            .bg(gpui_kit::rgb(0xffffff))
            .border_1()
            .border_color(gpui_kit::rgb(0xcccccc))
            .rounded(px(4.))
            .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.18), offset: point(px(0.), px(4.)), blur_radius: px(6.), spread_radius: px(0.), inset: false }])
            .children(rows)
            .on_mouse_down_out(cx.listener(|this, _: &MouseDownEvent, _, cx| {
                this.menu = None;
                cx.notify();
            }))
            .into_any_element()
    }

    /// `.os-add-condition-btn`: a 24 px secondary button with the + icon.
    fn add_button(&self, id: SharedString, label: &'static str, tip: &'static str, enabled: bool, act: Act, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        h_flex()
            .id(id)
            .h(px(24.))
            .flex_none()
            .px(px(10.))
            .gap(px(4.))
            .rounded(px(3.))
            .border_1()
            .border_color(gpui_kit::rgb(BORDER))
            .bg(gpui_kit::rgb(0xffffff))
            .text_size(px(13.))
            .text_color(pal.text)
            .when(!enabled, |d| d.opacity(0.5).cursor_not_allowed())
            .when(enabled, |d| {
                d.cursor_pointer().hover(|d| d.bg(gpui_kit::rgb(0xeaeaea))).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    act(this);
                    cx.notify();
                }))
            })
            .child(crate::assets::icon("add-button", 16., cx))
            .child(label)
            .tooltip(crate::ui::tooltip(tip))
            .into_any_element()
    }

    fn close_x(&self, id: SharedString, act: Act, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id(id)
            .size(px(24.))
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .child(crate::assets::icon("close-x", 16., cx))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                act(this);
                cx.notify();
            }))
            .into_any_element()
    }

    // ------------------------------------------------------------ conditions

    /// conditionBuilder: When/And (or the When | Always select of an option
    /// rule's first row), the controlling input, is | is between, the values.
    fn render_condition(&self, at: At, always: bool, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let c = self.condition(at);
        let rule_row = at.rule.is_some();
        let key = format!("{}-{:?}-{}", at.input, at.rule, at.cond);
        let mut row = h_flex().w_full().min_h(px(38.)).items_center().pl(px(10.)).text_size(px(14.)).text_color(pal.text);
        if rule_row && at.cond == 0 {
            let (k, r) = (at.input, at.rule.unwrap_or(0));
            row = row.child(div().mx(px(8.)).child(self.select(SelectSpec { id: SharedString::from(format!("vis-when-{key}")), label: if always { "Always".into() } else { "When".into() }, width: 85., enabled: true, menu: Menu::When(k, r) }, || {
                    vec![
                        Item::Choice { label: "When".into(), selected: !always, disabled: false, act: Rc::new(move |this: &mut Self| this.set_always(k, r, false)) },
                        Item::Choice { label: "Always".into(), selected: always, disabled: false, act: Rc::new(move |this: &mut Self| this.set_always(k, r, true)) },
                    ]
                }, cx)));
        } else {
            row = row.child(div().w(px(if rule_row { 101. } else { 48. })).mx(px(8.)).child(if at.cond == 0 { "When" } else { "And" }));
        }
        if !always {
            let controllers = self.controllers(at);
            let input_label = if c.input.is_empty() { "Select input".to_string() } else { self.name_of(&c.input) };
            let names: Vec<(String, String)> = controllers.iter().map(|&k| (self.cfg.inputs[k].id.clone(), self.cfg.inputs[k].name.clone())).collect();
            let current = c.input.clone();
            let has_input = !c.input.is_empty();
            row = row.child(div().mx(px(8.)).child(self.select(SelectSpec { id: SharedString::from(format!("vis-in-{key}")), label: input_label, width: 150., enabled: true, menu: Menu::CondInput(at) }, || {
                    if names.is_empty() {
                        return vec![Item::Choice { label: "Select input".into(), selected: false, disabled: true, act: Rc::new(|_: &mut Self| {}) }];
                    }
                    names
                        .into_iter()
                        .map(|(id, name)| {
                            let selected = id == current;
                            Item::Choice { label: name, selected, disabled: false, act: Rc::new(move |this: &mut Self| this.change_cond(at, |c| *c = Condition { input: id.clone(), test: ConditionTest::Is(Vec::new()) })) }
                        })
                        .collect()
                }, cx)));
            let between = matches!(c.test, ConditionTest::Between(..));
            let list = has_input && !self.is_checkbox_id(&c.input);
            row = row.child(div().mx(px(8.)).child(self.select(SelectSpec { id: SharedString::from(format!("vis-op-{key}")), label: if between { "is between".into() } else { "is".into() }, width: 150., enabled: has_input, menu: Menu::CondOp(at) }, || {
                    let mut v = vec![Item::Choice { label: "is".into(), selected: !between, disabled: false, act: Rc::new(move |this: &mut Self| this.change_cond(at, |c| c.test = ConditionTest::Is(Vec::new()))) }];
                    if list {
                        v.push(Item::Choice { label: "is between".into(), selected: between, disabled: false, act: Rc::new(move |this: &mut Self| this.change_cond(at, |c| c.test = ConditionTest::Between(String::new(), String::new()))) });
                    }
                    v
                }, cx)));
            row = row.child(self.render_values(at, &c, &key, cx));
        }
        // A rule's or a later condition's ✕; an input statement's only
        // condition has one too (canRemoveLastCondition).
        row = row.child(div().flex_1()).child(self.close_x(SharedString::from(format!("vis-x-{key}")), Rc::new(move |this: &mut Self| this.remove_cond(at)), cx));
        row.into_any_element()
    }

    fn render_values(&self, at: At, c: &Condition, key: &str, cx: &mut Context<Self>) -> AnyElement {
        let has_input = !c.input.is_empty();
        let input = c.input.clone();
        if !has_input || self.is_checkbox_id(&c.input) {
            let value = match &c.test {
                ConditionTest::Is(v) => v.first().cloned(),
                _ => None,
            };
            let label = value.clone().unwrap_or_else(|| "Select condition".into());
            return self.select(SelectSpec { id: SharedString::from(format!("vis-val-{key}")), label, width: 150., enabled: has_input, menu: Menu::CondValues(at) }, || {
                    if !has_input {
                        return vec![Item::Choice { label: "Select condition".into(), selected: false, disabled: true, act: Rc::new(|_: &mut Self| {}) }];
                    }
                    ["true", "false"]
                        .into_iter()
                        .map(|b| Item::Choice { label: b.into(), selected: value.as_deref() == Some(b), disabled: false, act: Rc::new(move |this: &mut Self| this.change_cond(at, |c| c.test = ConditionTest::Is(vec![b.to_string()]))) })
                        .collect()
                }, cx);
        }
        let options = self.options_of(&input);
        match &c.test {
            ConditionTest::Is(chosen) => {
                let label = if chosen.is_empty() { "Select condition".to_string() } else { self.values_text(c) };
                let chosen = chosen.clone();
                self.select(SelectSpec { id: SharedString::from(format!("vis-val-{key}")), label, width: 150., enabled: true, menu: Menu::CondValues(at) }, || {
                        let all = !options.is_empty() && options.iter().all(|(o, _)| chosen.contains(o));
                        let ids: Vec<String> = options.iter().map(|(o, _)| o.clone()).collect();
                        let mut v = vec![
                            Item::Check { label: "Select all".into(), checked: all, act: Rc::new(move |this: &mut Self| {
                                let ids = ids.clone();
                                this.change_cond(at, |c| c.test = ConditionTest::Is(if all { Vec::new() } else { ids }));
                                this.menu = Some(Menu::CondValues(at));
                            }) },
                            Item::Separator,
                        ];
                        for (o, n) in options {
                            let on = chosen.contains(&o);
                            let input = input.clone();
                            v.push(Item::Check { label: n, checked: on, act: Rc::new(move |this: &mut Self| {
                                let o = o.clone();
                                let order: Vec<String> = this.options_of(&input).into_iter().map(|(x, _)| x).collect();
                                this.change_cond(at, |c| {
                                    if let ConditionTest::Is(list) = &mut c.test {
                                        if on {
                                            list.retain(|x| *x != o);
                                        } else {
                                            list.push(o);
                                            list.sort_by_key(|x| order.iter().position(|y| y == x));
                                        }
                                    }
                                });
                                this.menu = Some(Menu::CondValues(at));
                            }) });
                        }
                        v
                    }, cx)
            }
            ConditionTest::Between(a, b) => {
                let start = if a.is_empty() { "Select range start".to_string() } else { self.option_name(&input, a) };
                let end = if b.is_empty() { "Select range end".to_string() } else { self.option_name(&input, b) };
                let (a, b) = (a.clone(), b.clone());
                let o2 = options.clone();
                h_flex()
                    .child(self.select(SelectSpec { id: SharedString::from(format!("vis-start-{key}")), label: start, width: 150., enabled: true, menu: Menu::CondStart(at) }, || {
                        options
                            .into_iter()
                            .map(|(o, n)| {
                                let selected = o == a;
                                Item::Choice { label: n, selected, disabled: false, act: Rc::new(move |this: &mut Self| {
                                    let o = o.clone();
                                    this.change_cond(at, |c| {
                                        if let ConditionTest::Between(s, _) = &mut c.test {
                                            *s = o;
                                        }
                                    })
                                }) }
                            })
                            .collect()
                    }, cx))
                    .child(div().mx(px(8.)).child(self.select(SelectSpec { id: SharedString::from(format!("vis-end-{key}")), label: end, width: 150., enabled: true, menu: Menu::CondEnd(at) }, || {
                        o2.into_iter()
                            .map(|(o, n)| {
                                let selected = o == b;
                                Item::Choice { label: n, selected, disabled: false, act: Rc::new(move |this: &mut Self| {
                                    let o = o.clone();
                                    this.change_cond(at, |c| {
                                        if let ConditionTest::Between(_, e) = &mut c.test {
                                            *e = o;
                                        }
                                    })
                                }) }
                            })
                            .collect()
                    }, cx)))
                    .into_any_element()
            }
        }
    }

    fn set_always(&mut self, k: usize, r: usize, always: bool) {
        self.change(k, |v| {
            if let Some(rule) = v.option_rules.get_mut(r) {
                rule.always = always;
                rule.conditions = if always { Vec::new() } else { vec![blank()] };
            }
        });
    }

    // ---------------------------------------------------------------- inputs

    fn render_input(&self, k: usize, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let input = &self.cfg.inputs[k];
        let id = input.id.clone();
        let expanded = !self.collapsed.contains(&id);
        let v = self.vis(k);
        let summary = if v.conditions.is_empty() {
            if self.is_list(k) { "Always shown (input is hidden if no options are shown)".to_string() } else { "Always shown".to_string() }
        } else {
            self.statement_text(&v.conditions, true, false)
        };
        let id2 = id.clone();
        let header = h_flex()
            .id(SharedString::from(format!("vis-head-{id}")))
            .h(px(35.))
            .pl(px(10.))
            .gap(px(4.))
            .bg(gpui_kit::rgb(HEADER_FILL))
            .border_b_1()
            .border_color(gpui_kit::rgb(BORDER))
            .when(!expanded, |d| d.rounded_b(px(5.)))
            .child(
                div()
                    .id(SharedString::from(format!("vis-chev-{id}")))
                    .cursor_pointer()
                    .child(crate::ui::glyph(if expanded { "ui-caret-down" } else { "ui-caret-right" }, 10., pal.text))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if !this.collapsed.remove(&id2) {
                            this.collapsed.insert(id2.clone());
                        }
                        cx.notify();
                    })),
            )
            .child(div().ml(px(5.)).text_size(px(16.)).text_color(pal.text).truncate().child(crate::ui::user_text(input.name.clone())))
            .when(self.uncovered_options(k), |d| {
                d.child(div().id(SharedString::from(format!("vis-warn-{id}"))).ml(px(5.)).child(crate::assets::icon("performance-warning", 15., cx)).tooltip(crate::ui::tooltip("Some options do not have an option rule. They will always be hidden.")))
            })
            .when(!expanded, |d| d.child(div().ml(px(10.)).flex_1().min_w_0().truncate().text_size(px(14.)).text_color(pal.text_muted).child(summary)));
        let mut section = v_flex().w_full().mt(px(15.)).border_1().border_color(gpui_kit::rgb(BORDER)).rounded(px(5.)).child(header);
        if !expanded {
            return section.into_any_element();
        }
        let mut body = v_flex().w_full().pb(px(10.));
        let can_add = !self.controllers(At { input: k, rule: None, cond: v.conditions.len() }).is_empty();
        if v.conditions.is_empty() {
            body = body.child(
                div().mx(px(24.)).mt(px(20.)).child(
                    crate::ui::alert(crate::ui::AlertKind::Info)
                        .py(px(12.))
                        .px(px(14.))
                        .text_size(px(14.))
                        .child(div().font_weight(FontWeight::BOLD).child("No conditions created. Input is always shown."))
                        .child(div().ml(px(10.)).child(self.add_button(SharedString::from(format!("vis-addc-{id}")), "Add condition", "Add a visibility condition for this configuration input", can_add, Rc::new(move |this: &mut Self| this.change(k, |v| v.conditions.push(blank()))), cx))),
                ),
            );
        } else {
            body = body
                .child(h_flex().mx(px(24.)).mt(px(10.)).mb(px(8.)).text_size(px(14.)).text_color(pal.text).child(div().mx(px(8.)).child("Show")).child(div().mx(px(8.)).truncate().child(crate::ui::user_text(input.name.clone()))))
                .children((0..v.conditions.len()).map(|c| div().ml(px(14.)).mb(px(4.)).child(self.render_condition(At { input: k, rule: None, cond: c }, false, cx))))
                .child(h_flex().ml(px(36.)).mt(px(4.)).child(self.add_button(SharedString::from(format!("vis-addc-{id}")), "Add condition", "Add a condition to this statement", can_add, Rc::new(move |this: &mut Self| this.change(k, |v| v.conditions.push(blank()))), cx)));
        }
        if self.is_list(k) {
            let can_rule = !self.controllers(At { input: k, rule: Some(usize::MAX), cond: 0 }).is_empty();
            if v.option_rules.is_empty() {
                body = body.child(
                    div().mx(px(24.)).mt(px(20.)).child(
                        crate::ui::alert(crate::ui::AlertKind::Info)
                            .py(px(12.))
                            .px(px(14.))
                            .text_size(px(14.))
                            .child(div().font_weight(FontWeight::BOLD).child("No option rules created. All options are always shown."))
                            .child(div().ml(px(10.)).child(self.add_button(SharedString::from(format!("vis-addr-{id}")), "Add option rule", "Add a visibility rule for the options of this configuration input", can_rule, Rc::new(move |this: &mut Self| this.add_rule(k)), cx))),
                    ),
                );
            } else {
                for r in 0..v.option_rules.len() {
                    body = body.child(div().ml(px(20.)).my(px(16.)).child(self.render_rule(k, r, cx)));
                }
                body = body.child(h_flex().mx(px(24.)).mb(px(12.)).child(self.add_button(SharedString::from(format!("vis-addr-{id}")), "Add option rule", "Add a visibility rule for the options of this configuration input", can_rule, Rc::new(move |this: &mut Self| this.add_rule(k)), cx)));
            }
        }
        section = section.child(body);
        section.into_any_element()
    }

    fn add_rule(&mut self, k: usize) {
        self.change(k, |v| v.option_rules.push(OptionRule { show: ShownOptions::Unset, always: false, conditions: vec![blank()] }));
    }

    /// visibilityConditionStatement for an option rule.
    fn render_rule(&self, k: usize, r: usize, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let input = self.cfg.inputs[k].id.clone();
        let rule = self.vis(k).option_rules[r].clone();
        let expanded = !self.rules_collapsed.contains(&(input.clone(), r));
        let options = self.options_of(&input);
        let key = format!("{k}-{r}");
        let entity = match &rule.show {
            ShownOptions::Unset => "controlled entity",
            ShownOptions::Options(_) => "options",
            ShownOptions::Range(..) => "range",
        };
        let is_options = matches!(rule.show, ShownOptions::Options(_));
        let is_range = matches!(rule.show, ShownOptions::Range(..));
        let mut header = h_flex()
            .h(px(45.))
            .pt(px(10.))
            .pl(px(10.))
            .pr(px(6.))
            .text_size(px(14.))
            .text_color(pal.text)
            .child({
                let key2 = (input.clone(), r);
                div()
                    .id(SharedString::from(format!("vis-rchev-{key}")))
                    .cursor_pointer()
                    .child(crate::ui::glyph(if expanded { "ui-caret-down" } else { "ui-caret-right" }, 10., pal.text))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if !this.rules_collapsed.remove(&key2) {
                            this.rules_collapsed.insert(key2.clone());
                        }
                        cx.notify();
                    }))
            })
            .child(div().mx(px(8.)).child("Show"))
            .child(div().mx(px(8.)).child(self.select(SelectSpec { id: SharedString::from(format!("vis-rshow-{key}")), label: entity.into(), width: 150., enabled: true, menu: Menu::RuleShow(k, r) }, || {
                vec![
                    Item::Choice { label: "options".into(), selected: is_options, disabled: false, act: Rc::new(move |this: &mut Self| this.change(k, |v| v.option_rules[r].show = ShownOptions::Options(Vec::new()))) },
                    Item::Choice { label: "range".into(), selected: is_range, disabled: false, act: Rc::new(move |this: &mut Self| this.change(k, |v| v.option_rules[r].show = ShownOptions::Range(String::new(), String::new()))) },
                ]
            }, cx)));
        match rule.show.clone() {
            ShownOptions::Unset => {}
            ShownOptions::Options(chosen) => {
                let label = if chosen.is_empty() { "Select options".to_string() } else { options.iter().filter(|(o, _)| chosen.contains(o)).map(|(_, n)| n.clone()).collect::<Vec<_>>().join(", ") };
                let opts = options.clone();
                header = header.child(div().mr(px(8.)).child(self.select(SelectSpec { id: SharedString::from(format!("vis-ropts-{key}")), label, width: 150., enabled: true, menu: Menu::RuleOptions(k, r) }, || {
                    let ids: Vec<String> = opts.iter().map(|(o, _)| o.clone()).collect();
                    let all = !ids.is_empty() && ids.iter().all(|o| chosen.contains(o));
                    let mut v = vec![
                        Item::Check { label: "Select all".into(), checked: all, act: Rc::new(move |this: &mut Self| {
                            let ids = ids.clone();
                            this.change(k, |v| v.option_rules[r].show = ShownOptions::Options(if all { Vec::new() } else { ids }));
                            this.menu = Some(Menu::RuleOptions(k, r));
                        }) },
                        Item::Separator,
                    ];
                    for (o, n) in opts {
                        let on = chosen.contains(&o);
                        v.push(Item::Check { label: n, checked: on, act: Rc::new(move |this: &mut Self| {
                            let o = o.clone();
                            let order: Vec<String> = this.options_of(&this.cfg.inputs[k].id).into_iter().map(|(x, _)| x).collect();
                            this.change(k, |v| {
                                if let ShownOptions::Options(list) = &mut v.option_rules[r].show {
                                    if on {
                                        list.retain(|x| *x != o);
                                    } else {
                                        list.push(o);
                                        list.sort_by_key(|x| order.iter().position(|y| y == x));
                                    }
                                }
                            });
                            this.menu = Some(Menu::RuleOptions(k, r));
                        }) });
                    }
                    v
                }, cx)));
            }
            ShownOptions::Range(a, b) => {
                let start = if a.is_empty() { "Select range start".to_string() } else { self.option_name(&input, &a) };
                let end = if b.is_empty() { "Select range end".to_string() } else { self.option_name(&input, &b) };
                let (o1, o2) = (options.clone(), options.clone());
                header = header
                    .child(div().mr(px(8.)).child(self.select(SelectSpec { id: SharedString::from(format!("vis-rstart-{key}")), label: start, width: 150., enabled: true, menu: Menu::RuleStart(k, r) }, || {
                        o1.into_iter()
                            .map(|(o, n)| {
                                let selected = o == a;
                                Item::Choice { label: n, selected, disabled: false, act: Rc::new(move |this: &mut Self| {
                                    let o = o.clone();
                                    this.change(k, |v| {
                                        if let ShownOptions::Range(s, _) = &mut v.option_rules[r].show {
                                            *s = o;
                                        }
                                    })
                                }) }
                            })
                            .collect()
                    }, cx)))
                    .child(div().mr(px(8.)).child(self.select(SelectSpec { id: SharedString::from(format!("vis-rend-{key}")), label: end, width: 150., enabled: true, menu: Menu::RuleEnd(k, r) }, || {
                        o2.into_iter()
                            .map(|(o, n)| {
                                let selected = o == b;
                                Item::Choice { label: n, selected, disabled: false, act: Rc::new(move |this: &mut Self| {
                                    let o = o.clone();
                                    this.change(k, |v| {
                                        if let ShownOptions::Range(_, e) = &mut v.option_rules[r].show {
                                            *e = o;
                                        }
                                    })
                                }) }
                            })
                            .collect()
                    }, cx)));
            }
        }
        if !expanded {
            header = header.child(div().ml(px(8.)).flex_1().min_w_0().truncate().text_color(pal.text_muted).child(self.statement_text(&rule.conditions, false, rule.always)));
        }
        header = header.child(div().flex_1()).child(self.close_x(SharedString::from(format!("vis-rx-{key}")), Rc::new(move |this: &mut Self| this.change(k, |v| {
            v.option_rules.remove(r);
        })), cx));
        let mut stmt = v_flex().w(relative(0.97)).border_1().border_color(gpui_kit::rgb(BORDER)).rounded(px(5.)).child(header);
        if expanded {
            let unset = matches!(rule.show, ShownOptions::Unset);
            let n = if rule.always { 1 } else { rule.conditions.len() };
            let mut body = v_flex().pb(px(10.)).pl(px(10.)).when(unset, |d| d.opacity(0.5));
            for c in 0..n {
                body = body.child(self.render_condition(At { input: k, rule: Some(r), cond: c }, rule.always, cx));
            }
            let can_add = !rule.always && !unset && !self.controllers(At { input: k, rule: Some(r), cond: rule.conditions.len() }).is_empty();
            body = body.child(h_flex().mx(px(12.)).mt(px(4.)).child(self.add_button(SharedString::from(format!("vis-raddc-{key}")), "Add condition", "Add a condition to this statement", can_add, Rc::new(move |this: &mut Self| this.change(k, |v| v.option_rules[r].conditions.push(blank()))), cx)));
            stmt = stmt.child(body);
        }
        stmt.into_any_element()
    }

    // --------------------------------------------------------------- preview

    fn render_preview(&self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let valid = self.valid();
        // Incomplete conditions don't count yet (Onshape disables the preview
        // until every condition is valid).
        let (vis, eff) = self.complete_only().visibility(&self.preview);
        let mut col = v_flex().p(px(25.)).gap(px(2.)).child(div().mb(px(10.)).text_size(px(14.)).font_weight(FontWeight::BOLD).text_color(pal.text).child("Configurations"));
        for (k, (input, v)) in self.cfg.inputs.iter().zip(&vis).enumerate() {
            if !v.visible && !self.show_hidden {
                continue;
            }
            let shown = v.visible && valid;
            let id = input.id.clone();
            let value = eff.0.get(&id).cloned().unwrap_or_default();
            let row = h_flex().min_h(px(34.)).px(px(5.)).pb(px(5.)).gap(px(6.)).text_size(px(13.)).text_color(pal.text).when(!v.visible, |d| d.bg(gpui_kit::rgb(0xeeeeee)).cursor_not_allowed());
            let row = match &input.kind {
                ConfigInputKind::Checkbox { .. } => {
                    let on = value == "true";
                    row.child(crate::ui::os_checkbox(SharedString::from(format!("vis-pv-{id}")), input.name.clone(), on).when(shown, |d| {
                        d.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.preview.0.insert(id.clone(), (!on).to_string());
                            cx.notify();
                        }))
                    }))
                }
                ConfigInputKind::List { options, .. } => {
                    let name = options.iter().find(|o| o.id == value).map(|o| o.name.clone()).unwrap_or_default();
                    let opts: Vec<(String, String, bool)> = options.iter().map(|o| (o.id.clone(), o.name.clone(), v.options.contains(&o.id))).filter(|(_, _, s)| *s || self.show_hidden).collect();
                    let open = self.menu == Some(Menu::Preview(k));
                    let button = h_flex()
                        .id(SharedString::from(format!("vis-pv-{id}")))
                        .flex_1()
                        .min_w(px(100.))
                        .h(px(26.))
                        .justify_between()
                        .border_b_1()
                        .border_color(gpui_kit::rgb(if open { TITLE } else { 0xd7d7d7 }))
                        .when(!v.visible, |d| d.opacity(0.75))
                        .child(div().truncate().child(crate::ui::user_text(name)))
                        .child(crate::ui::glyph("ui-caret-down", 9., pal.text))
                        .when(shown, |d| {
                            d.cursor_pointer().on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.menu = if this.menu == Some(Menu::Preview(k)) { None } else { Some(Menu::Preview(k)) };
                                cx.notify();
                            }))
                        });
                    let mut wrap = div().relative().flex_1().child(button);
                    if open {
                        let items = opts
                            .into_iter()
                            .map(|(o, n, s)| {
                                let id = id.clone();
                                Item::Choice { label: n, selected: o == value, disabled: !s, act: Rc::new(move |this: &mut Self| {
                                    this.preview.0.insert(id.clone(), o.clone());
                                }) }
                            })
                            .collect();
                        wrap = wrap.child(div().absolute().top(px(28.)).left_0().child(deferred(anchored().child(self.dropdown(150., items, cx))).with_priority(POPOVER)));
                    }
                    row.child(div().w(px(70.)).truncate().text_size(px(12.)).text_color(pal.text_muted).child(crate::ui::user_text(input.name.clone()))).child(wrap)
                }
                _ => row.child(div().w(px(70.)).truncate().text_size(px(12.)).text_color(pal.text_muted).child(crate::ui::user_text(input.name.clone()))).child(div().flex_1().h(px(26.)).border_b_1().border_color(gpui_kit::rgb(0xd7d7d7)).child(value)),
            };
            col = col.child(row);
        }
        let show_hidden = self.show_hidden;
        v_flex()
            .w(relative(0.25))
            .h_full()
            .child(div().id("vis-preview").flex_1().overflow_y_scroll().child(col))
            .child(
                h_flex().h(px(25.)).px(px(35.)).border_t_1().border_color(gpui_kit::rgb(BORDER)).child(crate::ui::os_checkbox("vis-show-hidden", "Show hidden inputs", show_hidden).text_size(px(14.)).text_color(pal.text).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.show_hidden = !this.show_hidden;
                    cx.notify();
                }))),
            )
            .into_any_element()
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut cfg = self.cfg.clone();
        for i in &mut cfg.inputs {
            if i.visible_when.as_ref().is_some_and(|v| v.conditions.is_empty() && v.option_rules.is_empty()) {
                i.visible_when = None;
            }
        }
        (self.on_save)(cfg, window, cx);
        window.close_dialog(cx);
    }
}

impl Render for VisibilityDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let always = self.always_visible();
        let current_index = self.current.as_ref().and_then(|c| self.cfg.inputs.iter().position(|i| &i.id == c));
        // Header: title, ⋯ (Expand / Collapse all inputs), the Inputs filter, ✕.
        let filter_label = current_index.map(|k| self.cfg.inputs[k].name.clone()).unwrap_or_else(|| "All inputs".into());
        let names: Vec<(String, String)> = self.cfg.inputs.iter().map(|i| (i.id.clone(), i.name.clone())).collect();
        let current = self.current.clone();
        let filter = self.select(SelectSpec { id: "vis-filter".into(), label: filter_label, width: 150., enabled: true, menu: Menu::Filter }, || {
            let mut v = vec![Item::Choice { label: "All inputs".into(), selected: current.is_none(), disabled: false, act: Rc::new(|this: &mut Self| this.current = None) }];
            for (id, n) in names {
                let selected = current.as_deref() == Some(id.as_str());
                v.push(Item::Choice { label: n, selected, disabled: false, act: Rc::new(move |this: &mut Self| this.current = Some(id.clone())) });
            }
            v
        }, cx);
        let expandable: Vec<String> = self.cfg.inputs.iter().enumerate().filter(|(k, _)| !always.contains(k)).map(|(_, i)| i.id.clone()).collect();
        let more = (self.current.is_none() && expandable.len() > 1).then(|| {
            let any_collapsed = expandable.iter().any(|id| self.collapsed.contains(id));
            let any_expanded = expandable.iter().any(|id| !self.collapsed.contains(id));
            let ex = expandable.clone();
            let open = self.menu == Some(Menu::More);
            let mut items = Vec::new();
            if any_collapsed {
                items.push(Item::Choice { label: "Expand all inputs".into(), selected: false, disabled: false, act: Rc::new(|this: &mut Self| this.collapsed.clear()) });
            }
            if any_expanded {
                items.push(Item::Choice { label: "Collapse all inputs".into(), selected: false, disabled: false, act: Rc::new(move |this: &mut Self| this.collapsed.extend(ex.iter().cloned())) });
            }
            let mut wrap = div().relative().child(
                div()
                    .id("vis-more")
                    .size(px(26.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(3.))
                    .cursor_pointer()
                    .hover(|d| d.bg(pal.panel_hover))
                    .child(crate::assets::icon("more", 20., cx))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.menu = if this.menu == Some(Menu::More) { None } else { Some(Menu::More) };
                        cx.notify();
                    })),
            );
            if open {
                wrap = wrap.child(div().absolute().top(px(28.)).left_0().child(deferred(anchored().child(self.dropdown(150., items, cx))).with_priority(POPOVER)));
            }
            wrap
        });
        let header = h_flex()
            .h(px(54.))
            .px(px(10.))
            .border_b_1()
            .border_color(gpui_kit::rgb(os::MODAL_DIVIDER))
            .child(
                h_flex()
                    .w(relative(0.75))
                    .justify_between()
                    .child(h_flex().gap(px(8.)).child(div().text_size(px(20.)).font_weight(FontWeight::MEDIUM).text_color(gpui_kit::rgb(TITLE)).child("Configuration visibility conditions")).children(more))
                    .child(filter),
            )
            .child(div().flex_1())
            .child(
                div()
                    .id("vis-close")
                    .size(px(24.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .opacity(0.5)
                    .cursor_pointer()
                    .hover(|d| d.opacity(0.75))
                    .child(crate::ui::glyph("close", 14., gpui_kit::rgb(0x000000)))
                    .on_click(|_, window, cx| window.close_dialog(cx)),
            );
        // Left: the always-visible row, then one section per other input.
        let mut left = v_flex().pl(px(20.)).pr(px(20.)).pt(px(10.)).pb(px(20.));
        let show_top = current_index.is_none_or(|k| always.contains(&k)) && !self.cfg.inputs.is_empty();
        if show_top {
            let names = match current_index {
                Some(k) => self.cfg.inputs[k].name.clone(),
                None => always.iter().map(|&k| self.cfg.inputs[k].name.clone()).collect::<Vec<_>>().join(", "),
            };
            left = left.child(
                h_flex()
                    .mt(px(15.))
                    .h(px(35.))
                    .pl(px(10.))
                    .pr(px(5.))
                    .justify_between()
                    .bg(gpui_kit::rgb(HEADER_FILL))
                    .border_1()
                    .border_color(gpui_kit::rgb(BORDER))
                    .rounded_b(px(5.))
                    .child(div().ml(px(5.)).text_size(px(16.)).text_color(pal.text).truncate().child(names))
                    .child(div().id("vis-always-info").child(crate::assets::icon("performance-info", 15., cx)).tooltip(crate::ui::tooltip("These configuration inputs will always be visible"))),
            );
        }
        for k in 0..self.cfg.inputs.len() {
            if always.contains(&k) || current_index.is_some_and(|c| c != k) {
                continue;
            }
            left = left.child(self.render_input(k, cx));
        }
        let body = h_flex()
            .h(px(372.))
            .items_start()
            .child(div().id("vis-left").w(relative(0.75)).h_full().overflow_y_scroll().border_r_1().border_color(gpui_kit::rgb(BORDER)).child(left))
            .child(self.render_preview(cx));
        // Footer: ? undo redo … Save Cancel.
        let can_undo = !self.undo.is_empty();
        let can_redo = !self.redo.is_empty();
        let changed = self.cfg != self.original;
        let can_save = changed && self.valid();
        let icon_btn = |id: &'static str, icon: &'static str, tip: &'static str, on: bool, cx: &mut Context<Self>| {
            div()
                .id(id)
                .size(px(28.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(3.))
                .when(!on, |d| d.opacity(0.4))
                .when(on, |d| d.cursor_pointer().hover(|d| d.bg(pal.panel_hover)))
                .child(crate::assets::icon(icon, 20., cx))
                .tooltip(crate::ui::tooltip(tip))
        };
        let footer = h_flex()
            .h(px(64.))
            .px(px(12.))
            .gap(px(4.))
            .border_t_1()
            .border_color(gpui_kit::rgb(os::MODAL_DIVIDER))
            .child(icon_btn("vis-help", "help", "Help", true, cx).mx(px(8.)).on_click(|_, _, cx| cx.open_url("https://cad.onshape.com/help/Content/PartStudio/visibility_conditions.htm")))
            .child(icon_btn("vis-undo", "undo", "Undo", can_undo, cx).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                if let Some(prev) = this.undo.pop() {
                    this.redo.push(std::mem::replace(&mut this.cfg, prev));
                    cx.notify();
                }
            })))
            .child(icon_btn("vis-redo", "redo", "Redo", can_redo, cx).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                if let Some(next) = this.redo.pop() {
                    this.undo.push(std::mem::replace(&mut this.cfg, next));
                    cx.notify();
                }
            })))
            .child(div().flex_1())
            .child(os::primary_button("vis-save", "Save").disabled(!can_save).on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.save(window, cx))))
            .child(os::cancel_button("vis-cancel", "Cancel", cx).on_click(|_, window, cx| window.close_dialog(cx)));
        v_flex().w_full().child(header).child(body).child(footer)
    }
}

/// Inputs copied with Copy input / Copy all inputs, for Paste inputs in any
/// Part Studio or Assembly (Help: "Copied inputs are available until the
/// user logs out").
static COPIED_INPUTS: parking_lot::Mutex<Vec<noshape_model::configuration::ConfigInput>> = parking_lot::Mutex::new(Vec::new());

pub fn copy_inputs(inputs: Vec<noshape_model::configuration::ConfigInput>) {
    *COPIED_INPUTS.lock() = inputs;
}

pub fn copied_inputs() -> Vec<noshape_model::configuration::ConfigInput> {
    COPIED_INPUTS.lock().clone()
}
