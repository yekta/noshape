//! Configurations › Add configuration input › Configuration variable: the
//! "Variable input" dialog, and a variable input's ⋯ › Edit definition…
//! ("Edit <name>") (live psb 10-20 s, psb/s-700–717). A 220 px feature-style
//! dialog left of the right panel: Name (required), type tabs Length | Angle
//! | Integer | Real | Text, Default / Minimum / Maximum (prefilled 1 / 0 /
//! 100000 in the workspace unit; Text has a Default only). Range errors mark
//! the fields pink with the reason as tooltip, turn the title red and disable
//! ✓. Enter commits; Esc does nothing; ✕ cancels.

use super::PartStudioView;
use crate::theme::p;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::configuration::{ConfigInput, ConfigInputKind, VariableKind};
use noshape_units::{Dim, ExprValue, UnitPrefs};

const KINDS: [(&str, VariableKind); 5] = [("Length", VariableKind::Length), ("Angle", VariableKind::Angle), ("Integer", VariableKind::Integer), ("Real", VariableKind::Real), ("Text", VariableKind::Text)];

pub struct VarInputDialog {
    /// Edit definition…: the input being edited (no Name, no type tabs).
    pub editing: Option<String>,
    /// The edited input's name ("Edit Wid").
    edit_name: String,
    kind: usize,
    name: Entity<InputState>,
    default: Entity<InputState>,
    min: Entity<InputState>,
    max: Entity<InputState>,
    _subs: Vec<Subscription>,
}

/// The dimension of a configuration variable type (`None` for Text).
pub(crate) use noshape_ops::config::kind_dim;

/// Default / Minimum / Maximum of a new variable of `kind` (live 702).
fn prefill(kind: VariableKind, units: &UnitPrefs) -> [String; 3] {
    let unit = match kind {
        VariableKind::Length => Some(units.length.symbol()),
        VariableKind::Angle => Some(units.angle.symbol()),
        _ => None,
    };
    let v = |n: &str| unit.map(|u| format!("{n} {u}")).unwrap_or_else(|| n.to_string());
    if kind == VariableKind::Text { [String::new(), String::new(), String::new()] } else { [v("1"), v("0"), v("100000")] }
}

/// Fields that don't evaluate (Default, Minimum, Maximum).
pub(crate) fn invalid_values(dim: Option<Dim>, values: [&str; 3], units: &UnitPrefs, vars: &dyn noshape_units::VariableScope) -> [bool; 3] {
    let Some(dim) = dim else { return [false; 3] };
    values.map(|v| noshape_units::evaluate_as(v, dim, units, vars).is_err())
}

/// The range error and the fields it marks (Default, Minimum, Maximum)
/// (bundle `validateRange`; live 703–707).
pub(crate) fn range_error(dim: Option<Dim>, values: [&str; 3], units: &UnitPrefs) -> Option<(&'static str, [bool; 3])> {
    let dim = dim?;
    let val = |t: &str| noshape_units::evaluate_as(t, dim, units, &noshape_units::EmptyScope).ok().map(|q| q.value);
    let unit_of = |t: &str| t.split_whitespace().nth(1).map(str::to_string);
    let [d, lo, hi] = values;
    let (Some(dv), Some(lv), Some(hv)) = (val(d), val(lo), val(hi)) else { return None };
    if unit_of(d) != unit_of(lo) || unit_of(lo) != unit_of(hi) {
        Some(("The default value, minimum, and maximum must all be in the same units.", [true, true, true]))
    } else if lv > dv {
        Some(("The default value must be greater or equal to the minimum.", [true, true, false]))
    } else if hv < dv {
        Some(("The default value must be less than or equal to the maximum.", [true, false, true]))
    } else if lv >= hv {
        Some(("The minimum must be less than the maximum.", [false, true, true]))
    } else {
        None
    }
}

impl VarInputDialog {
    fn texts(&self, cx: &App) -> [String; 3] {
        [&self.default, &self.min, &self.max].map(|s| s.read(cx).value().to_string())
    }

    /// Per field: the error tooltip, if any; and whether ✓ is allowed.
    fn errors(&self, units: &UnitPrefs, cx: &App) -> ([Option<&'static str>; 3], bool) {
        let kind = KINDS[self.kind].1;
        let dim = kind_dim(kind);
        let t = self.texts(cx);
        let values = [t[0].as_str(), t[1].as_str(), t[2].as_str()];
        let invalid = invalid_values(dim, values, units, &noshape_units::EmptyScope);
        let range = range_error(dim, values, units);
        let errs = [0, 1, 2].map(|i| if invalid[i] { Some("Enter a valid expression.") } else { range.and_then(|(e, m)| m[i].then_some(e)) });
        let named = self.editing.is_some() || !self.name.read(cx).value().trim().is_empty();
        (errs, named && errs.iter().all(Option::is_none))
    }
}

impl PartStudioView {
    /// Open the dialog: a new variable (`editing` None) or Edit definition…
    pub(super) fn open_variable_input(&mut self, editing: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        let units = self.doc.read(cx).session.doc.settings.units;
        let existing = editing.as_ref().and_then(|id| self.studio(cx).and_then(|s| s.configuration.inputs.iter().find(|i| &i.id == id).cloned()));
        let (kind, values, edit_name) = match &existing {
            Some(ConfigInput { name, kind: ConfigInputKind::Variable { kind, default, min, max }, .. }) => {
                let k = KINDS.iter().position(|(_, v)| v == kind).unwrap_or(0);
                let pre = prefill(*kind, &units);
                (k, [default.text.clone(), min.as_ref().map(|m| m.text.clone()).unwrap_or_else(|| pre[1].clone()), max.as_ref().map(|m| m.text.clone()).unwrap_or_else(|| pre[2].clone())], name.clone())
            }
            Some(_) => return,
            None => (0, prefill(VariableKind::Length, &units), String::new()),
        };
        let field = |v: String, window: &mut Window, cx: &mut Context<Self>| cx.new(|cx| InputState::new(window, cx).default_value(v));
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("Name"));
        let [d, lo, hi] = values;
        let (default, min, max) = (field(d, window, cx), field(lo, window, cx), field(hi, window, cx));
        let subs = [&name, &default, &min, &max]
            .into_iter()
            .map(|st| {
                cx.subscribe(st, |_, _, ev: &InputEvent, cx| {
                    if matches!(ev, InputEvent::Change) {
                        cx.notify();
                    }
                })
            })
            .collect();
        let focus = if editing.is_some() { default.clone() } else { name.clone() };
        focus.update(cx, |s, cx| s.focus(window, cx));
        self.ui.var_input = Some(VarInputDialog { editing, edit_name, kind, name, default, min, max, _subs: subs });
        cx.notify();
    }

    /// ✓ / Enter.
    fn commit_variable_input(&mut self, cx: &mut Context<Self>) {
        let units = self.doc.read(cx).session.doc.settings.units;
        let Some(d) = &self.ui.var_input else { return };
        if !d.errors(&units, cx).1 {
            return;
        }
        let Some(d) = self.ui.var_input.take() else { return };
        let kind = KINDS[d.kind].1;
        let [dv, lo, hi] = d.texts(cx);
        let range = (kind != VariableKind::Text).then(|| (ExprValue::new(lo), ExprValue::new(hi)));
        let default = ExprValue::new(dv);
        match d.editing {
            Some(id) => self.edit_config(noshape_ops::config::EDIT_VARIABLE_INPUT, cx, |ps| noshape_ops::config::edit_variable_input(ps, &id, default, range)),
            None => {
                let name = d.name.read(cx).value().trim().to_string();
                // The dialog's own description (not the Create variable dialog's).
                self.edit_config(noshape_ops::config::ADD_INPUT, cx, |ps| {
                    noshape_ops::config::push_variable_input(ps, name, kind, default, range);
                });
            }
        }
        self.config_dirty(cx);
        cx.notify();
    }

    pub(super) fn render_variable_input(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let units = self.doc.read(cx).session.doc.settings.units;
        let d = self.ui.var_input.as_ref()?;
        let pal = p();
        let (errs, ok) = d.errors(&units, cx);
        let blue = gpui_kit::rgb(0x1651b0);
        let title = match &d.editing {
            Some(_) => format!("Edit {}", d.edit_name),
            None => "Variable input".to_string(),
        };
        let tab = |i: usize, label: &'static str, on: bool, cx: &mut Context<Self>| {
            div()
                .id(SharedString::from(format!("vi-kind-{i}")))
                .flex_1()
                .h(px(26.))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .text_size(px(12.))
                .when(on, |d| d.font_weight(FontWeight::MEDIUM).text_color(blue).border_b_2().border_color(blue))
                .when(!on, |d| d.text_color(pal.text).border_b_1().border_color(gpui_kit::rgb(0xd7d7d7)))
                .child(label)
                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                    let units = this.doc.read(cx).session.doc.settings.units;
                    let Some(d) = &mut this.ui.var_input else { return };
                    if d.kind == i {
                        return;
                    }
                    // Each type starts from its own values (live 702).
                    d.kind = i;
                    let pre = prefill(KINDS[i].1, &units);
                    for (st, v) in [d.default.clone(), d.min.clone(), d.max.clone()].into_iter().zip(pre) {
                        st.update(cx, |s, cx| s.set_value(v, window, cx));
                    }
                    cx.notify();
                }))
        };
        // A field: pink with a red underline and the reason as tooltip.
        let row = |id: &'static str, label: &'static str, st: &Entity<InputState>, err: Option<&'static str>| {
            h_flex()
                .id(id)
                .h(px(28.))
                .px(px(5.))
                .child(div().w(px(64.)).flex_none().text_size(px(12.)).text_color(gpui_kit::rgb(0x666666)).child(label))
                .child(gpui_kit::base::input::input_frame(
                    div()
                        .flex_1()
                        .min_w_0()
                        .border_b_1()
                        .border_color(if err.is_some() { gpui_kit::rgb(0xbd3039) } else { gpui_kit::rgb(0xd7d7d7) })
                        .when(err.is_some(), |d| d.bg(gpui_kit::rgb(0xf9ebec)))
                        .child(Input::new(st).small().appearance(false).text_size(px(13.33))),
                ))
                // An empty message: the error look without a tooltip.
                .when_some(err.filter(|e| !e.is_empty()), |d, e| d.tooltip(crate::ui::tooltip(e)))
        };
        let text = KINDS[d.kind].1 == VariableKind::Text;
        let mut kinds = h_flex();
        if d.editing.is_none() {
            for (i, (label, _)) in KINDS.iter().enumerate() {
                kinds = kinds.child(tab(i, label, d.kind == i, cx));
            }
        }
        let d = self.ui.var_input.as_ref()?;
        let name_missing = d.editing.is_none() && d.name.read(cx).value().trim().is_empty();
        Some(
            v_flex()
                .id("variable-input")
                .occlude()
                .absolute()
                .top(px(3.))
                .right(px(0.))
                .w(px(220.))
                .bg(gpui_kit::rgb(0xffffff))
                .shadow_md()
                .text_color(pal.text)
                // Enter commits; Esc leaves the dialog open (live).
                .on_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| match e.keystroke.key.as_str() {
                    "enter" => {
                        cx.stop_propagation();
                        this.commit_variable_input(cx);
                    }
                    "escape" => cx.stop_propagation(),
                    _ => {}
                }))
                .child(
                    h_flex()
                        .h(px(29.))
                        .border_b_1()
                        .border_color(gpui_kit::rgb(0xd7d7d7))
                        .child(div().flex_1().pl(px(6.)).text_size(px(13.)).font_weight(FontWeight::MEDIUM).when(!ok, |d| d.text_color(gpui_kit::rgb(0xaa252d))).child(title))
                        .child(div().id("vi-ok").size(px(28.)).flex().items_center().justify_center().bg(pal.accept).when(!ok, |d| d.opacity(0.2)).cursor_pointer().child(crate::ui::glyph("check", 16., gpui_kit::rgb_static(0xffffff))).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.commit_variable_input(cx))))
                        .child(div().id("vi-cancel").size(px(28.)).flex().items_center().justify_center().cursor_pointer().child(crate::ui::glyph("close", 14., gpui_kit::rgb(0xd52b1e))).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.ui.var_input = None;
                            cx.notify();
                        }))),
                )
                .when(d.editing.is_none(), |c| c.child(row("vi-name", "Name", &d.name, name_missing.then_some(""))))
                .child(kinds)
                .child(row("vi-default", "Default", &d.default, errs[0]))
                .when(!text, |c| c.child(row("vi-min", "Minimum", &d.min, errs[1])).child(row("vi-max", "Maximum", &d.max, errs[2])))
                .into_any_element(),
        )
    }
}
