//! My account › Preferences › User export rules (live behaviours "Preferences
//! › User export rules" and "ui2 10-22 05:00", prefs/er-900…931, 1876–1883,
//! dom/preferences-export-rules*): rules per object type and file format that
//! name exported files from `${property}` placeholders.

use crate::os_ui as os;
use crate::preferences_window::PreferencesView;
use crate::theme::p;
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::input::{Input, InputEvent, InputState, Position};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::export_rules::{self as er, ExportElement, ExportRule, ExportRules};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

fn tr(s: &str) -> SharedString {
    crate::i18n::tr(s).into()
}

const BLUE: u32 = 0x1651b0;
const RED: u32 = 0xaa252d;

fn saved_rules() -> Vec<ExportRule> {
    crate::export_naming::rules().rules
}

fn save_rules(rules: Vec<ExportRule>) {
    crate::export_naming::set_rules(&ExportRules { rules });
}

#[derive(Default)]
pub struct ExportRulesPrefsView;

impl Render for ExportRulesPrefsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let rules = saved_rules();
        let help = div()
            .id("er-help")
            .child(crate::assets::icon("help-gray", 18., cx))
            .tooltip(crate::ui::tooltip(tr("Create export rules based on object type, category, and file format to automatically generate metadata-driven file names when you export objects.")))
            .into_any_element();
        let view = cx.entity();
        let body: AnyElement = if rules.is_empty() {
            // Live (prefs/1919): the note 43 px under the rule, the button 107.
            div().pl(px(6.)).pt(px(16.)).text_size(px(14.)).text_color(pal.text).child(tr("You have not added any export rules.")).into_any_element()
        } else {
            // Live 1877: Scope | Format | Convention | [Edit] [Remove], creation order.
            let cell = |w: f32, t: SharedString, bold: bool| div().w(px(w)).flex_none().px(px(10.)).truncate().when(bold, |d| d.font_weight(FontWeight::BOLD)).child(t);
            let small = |id: SharedString, label: SharedString| div().id(id).h(px(24.)).px(px(8.)).flex().items_center().rounded(px(3.)).bg(rgb(BLUE)).text_size(px(12.)).text_color(gpui_kit::rgb_static(0xffffff)).cursor_pointer().child(label);
            let mut table = v_flex().mr(px(15.)).text_size(px(14.)).text_color(pal.text).child(h_flex().h(px(44.)).border_b_1().border_color(rgb(0xdee2e6)).child(cell(180., tr("Scope"), true)).child(cell(220., tr("Format"), true)).child(div().flex_1().px(px(10.)).font_weight(FontWeight::BOLD).child(tr("Convention"))));
            for (i, r) in rules.iter().enumerate() {
                let (v1, v2) = (view.clone(), view.clone());
                table = table.child(
                    h_flex()
                        .h(px(44.))
                        .border_b_1()
                        .border_color(rgb(0xdee2e6))
                        .child(cell(180., tr(r.element.label()), false))
                        .child(cell(220., tr(&r.format), false))
                        .child(div().flex_1().min_w_0().px(px(10.)).truncate().child(r.template.clone()))
                        .child(
                            h_flex()
                                .gap(px(4.))
                                .pr(px(10.))
                                .child(small(SharedString::from(format!("er-edit-{i}")), tr("Edit")).on_click(move |_, window, cx| open_dialog(v1.clone(), Some(i), window, cx)))
                                .child(small(SharedString::from(format!("er-remove-{i}")), tr("Remove")).on_click(move |_, _, cx| {
                                    // Live: removed at once, no confirmation.
                                    let mut rules = saved_rules();
                                    if i < rules.len() {
                                        let gone = rules.remove(i);
                                        save_rules(rules);
                                        crate::prefs_general::toast(crate::i18n::tr("Successfully removed export rule with convention: __convention__").replace("__convention__", &gone.template), cx);
                                        v2.update(cx, |_, cx| cx.notify());
                                    }
                                })),
                        ),
                );
            }
            table.into_any_element()
        };
        let view_add = cx.entity();
        let add_gap = if rules.is_empty() { 53. } else { 26. };
        v_flex()
            // The next section's rule 50 px under the button (live 1919).
            .mb(px(18.))
            .child(PreferencesView::heading(tr("User export rules"), Some(help)))
            .child(
                v_flex()
                    .pl(px(15.))
                    .pt(px(14.))
                    .child(body)
                    .child(h_flex().mt(px(add_gap)).child(os::primary_button("er-add", tr("Add export rule…")).on_click(move |_, window, cx| open_dialog(view_add.clone(), None, window, cx)))),
            )
    }
}

/// The dialog's state across renders.
struct Edit {
    scope: ExportElement,
    format: usize,
    /// The autocomplete list: open, and the highlighted entry.
    complete: Option<usize>,
}

/// "Add export rule" (`editing` None) or "Edit export rule" (live er-902, 911).
fn open_dialog(view: Entity<ExportRulesPrefsView>, editing: Option<usize>, window: &mut Window, cx: &mut App) {
    let existing = editing.and_then(|i| saved_rules().get(i).cloned());
    let scope = existing.as_ref().map_or(ExportElement::Part, |r| r.element);
    let format = existing.as_ref().and_then(|r| scope.formats().iter().position(|f| *f == r.format)).unwrap_or(0);
    let state = Rc::new(RefCell::new(Edit { scope, format, complete: None }));
    let conv = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).default_value(existing.as_ref().map_or("${name}".to_string(), |r| r.template.clone())));
    {
        let st = state.clone();
        window
            .subscribe(&conv, cx, move |input, ev: &InputEvent, window, cx| {
                if let InputEvent::Change = ev {
                    // Typing after `${` opens the completion list (live er-904).
                    let s = input.read(cx);
                    let open = open_token(&s.value(), s.cursor()).is_some();
                    let mut e = st.borrow_mut();
                    e.complete = open.then_some(0);
                    window.refresh();
                }
            })
            .detach();
    }
    let focused = Rc::new(Cell::new(false));
    window.open_dialog(cx, move |dialog, window, cx| {
        if !focused.replace(true) {
            conv.update(cx, |s, cx| s.focus(window, cx));
        }
        let pal = p();
        let (scope, format_ix, complete) = {
            let e = state.borrow();
            (e.scope, e.format, e.complete)
        };
        let text = conv.read(cx).value().to_string();
        let error = er::validate(&text, scope);
        let remaining = er::MAX_TEMPLATE.saturating_sub(text.chars().count());
        let label = |t: &'static str, tip: &'static str, id: &'static str, cx: &App| {
            h_flex()
                .w(px(130.))
                .flex_none()
                .gap(px(4.))
                .text_size(px(14.))
                .font_weight(FontWeight::BOLD)
                .text_color(pal.text)
                .child(tr(t))
                .child(div().id(id).child(crate::assets::icon("help-gray", 16., cx)).tooltip(crate::ui::tooltip(tr(tip))))
        };
        let st1 = state.clone();
        let scope_select = os::select_field("er-scope", ExportElement::ALL.iter().map(|s| crate::i18n::tr(s.label())).collect(), ExportElement::ALL.iter().position(|s| *s == scope).unwrap_or(0), move |i, window, _| {
            let mut e = st1.borrow_mut();
            e.scope = ExportElement::ALL[i];
            e.format = 0;
            window.refresh();
        });
        let st2 = state.clone();
        let format_select = os::select_field("er-format", scope.formats().iter().map(|f| crate::i18n::tr(f)).collect(), format_ix, move |i, window, _| {
            st2.borrow_mut().format = i;
            window.refresh();
        });
        // The convention: a one-line monospace field whose placeholders are
        // blue when valid for the scope and red otherwise (an overlay over
        // the field's own, transparent, text).
        let mono = SharedString::from("Roboto Mono");
        let runs = colored_runs(&text, scope);
        let overlay = StyledText::new(text.clone()).with_highlights(runs);
        let field = div()
            .relative()
            .h(px(34.))
            .rounded(px(3.))
            .border_1()
            .border_color(if error.is_some() { rgb(RED) } else { rgb(0xaaaaaa) })
            .bg(rgb(0xffffff))
            .font_family(mono.clone())
            .text_size(px(14.))
            .child(div().absolute().top_0().bottom_0().left(px(9.)).right(px(6.)).flex().items_center().overflow_hidden().whitespace_nowrap().text_color(pal.text).child(overlay))
            .child(Input::new(&conv).appearance(false).text_color(gpui_kit::transparent_black()).font_family(mono).pl(px(8.)).h(px(32.)));
        // The completion list under the field.
        let candidates = completions(&text, conv.read(cx).cursor(), scope);
        let list = complete.filter(|_| !candidates.is_empty()).map(|sel| {
            let sel = sel.min(candidates.len() - 1);
            let mut col = v_flex().id("er-complete").absolute().top(px(36.)).left_0().w(px(360.)).max_h(px(220.)).overflow_y_scroll().bg(rgb(0xffffff)).border_1().border_color(rgb(0xcccccc)).shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }]).text_size(px(13.)).font_family("Roboto Mono");
            for (k, (name, meta)) in candidates.iter().enumerate() {
                let (c2, n2) = (conv.clone(), name.to_string());
                col = col.child(
                    h_flex()
                        .id(("er-c", k))
                        .h(px(22.))
                        .px(px(6.))
                        .cursor_pointer()
                        .when(k == sel, |d| d.bg(rgb(0xcad6fa)))
                        .child(div().flex_1().child(name.to_string()))
                        .child(div().text_color(rgb(0x999999)).child(tr(meta)))
                        .on_click(move |_, window, cx| accept(&c2, &n2, window, cx)),
                );
            }
            col.into_any_element()
        });
        let (st_keys, conv_keys, cands) = (state.clone(), conv.clone(), candidates.clone());
        let field_box = div()
            .relative()
            .flex_1()
            // Up / Down / Enter / Esc drive the open list; Ctrl+Space opens it.
            .capture_key_down(move |e: &KeyDownEvent, window, cx| {
                let k = e.keystroke.key.as_str();
                let mut s = st_keys.borrow_mut();
                if k == "space" && e.keystroke.modifiers.control {
                    s.complete = Some(0);
                    cx.stop_propagation();
                    window.refresh();
                    return;
                }
                let Some(sel) = s.complete.filter(|_| !cands.is_empty()) else { return };
                match k {
                    "down" => s.complete = Some((sel + 1).min(cands.len() - 1)),
                    "up" => s.complete = Some(sel.saturating_sub(1)),
                    "escape" => s.complete = None,
                    "enter" | "tab" => {
                        drop(s);
                        let name = cands[sel.min(cands.len() - 1)].0.to_string();
                        accept(&conv_keys, &name, window, cx);
                    }
                    _ => return,
                }
                cx.stop_propagation();
                window.refresh();
            })
            .child(field)
            .children(list);
        let row = |l: AnyElement, c: AnyElement| h_flex().items_start().gap(px(10.)).child(l).child(div().flex_1().child(c));
        let (st_ok, conv_ok, view_ok) = (state.clone(), conv.clone(), view.clone());
        os::modal_doc_page(dialog, tr(if editing.is_some() { "Edit export rule" } else { "Add export rule" }), 600.)
            .child(
                os::modal_body()
                    .gap(px(14.))
                    .child(row(label("Scope", "The rule will apply to objects of the selected object type.", "er-scope-help", cx).into_any_element(), scope_select.into_any_element()))
                    .child(row(label("Format", "The rule will apply when exporting applicable objects to the selected file format.", "er-format-help", cx).into_any_element(), format_select.into_any_element()))
                    .child(row(
                        label("Convention", "Create a file naming convention template to use when exporting applicable objects. Use template placeholder expressions of the form ${propertyName} to include an active metadata property in your naming convention. All properties must be valid and active and will be highlighted in blue, while invalid or inactive properties will be highlighted in red.", "er-conv-help", cx).into_any_element(),
                        v_flex()
                            .gap(px(4.))
                            .child(field_box)
                            .child(div().text_size(px(12.)).text_color(pal.text_muted).child(crate::i18n::tr("__n__ characters remaining").replace("__n__", &remaining.to_string())))
                            .when_some(error, |d, e| d.child(div().text_size(px(14.)).text_color(rgb(RED)).child(tr(e))))
                            .into_any_element(),
                    )),
            )
            .footer(
                os::modal_footer()
                    .child(os::modal_ok(os::primary_button("er-ok", tr(if editing.is_some() { "Save changes" } else { "Add" })).when(error.is_some(), |b| b.opacity(0.4))))
                    .child(os::modal_cancel(os::cancel_button("er-cancel", tr("Cancel"), cx))),
            )
            .on_ok(move |_, _, cx| {
                let (scope, format) = {
                    let e = st_ok.borrow();
                    (e.scope, e.scope.formats().get(e.format).copied().unwrap_or(er::ALL_FORMATS).to_string())
                };
                let convention = conv_ok.read(cx).value().to_string();
                if er::validate(&convention, scope).is_some() {
                    return false;
                }
                let mut rules = saved_rules();
                if (ExportRules { rules: rules.clone() }).conflicts(scope, &format, editing) {
                    // Live er-919: the dialog closes with an error toast; nothing is saved.
                    crate::prefs_general::toast(crate::i18n::tr("Unable to save export rule. Another export rule already exists for that combination of company, object type, category, and file format."), cx);
                    return true;
                }
                let rule = ExportRule { element: scope, format, template: convention.clone() };
                match editing {
                    Some(i) if i < rules.len() => rules[i] = rule,
                    _ => rules.push(rule),
                }
                save_rules(rules);
                let msg = if editing.is_some() { "Successfully updated export rule with convention: __convention__" } else { "Successfully added export rule with convention: __convention__" };
                crate::prefs_general::toast(crate::i18n::tr(msg).replace("__convention__", &convention), cx);
                view_ok.update(cx, |_, cx| cx.notify());
                true
            })
    });
}

/// The `${…` being typed before the caret: its start (after `${`) and text.
fn open_token(text: &str, caret: usize) -> Option<(usize, String)> {
    let before = text.get(..caret)?;
    let at = before.rfind("${")?;
    let typed = &before[at + 2..];
    (!typed.contains('}')).then(|| (at + 2, typed.to_string()))
}

/// The scope's variables matching what is typed after `${` (live: fuzzy).
fn completions(text: &str, caret: usize, scope: ExportElement) -> Vec<(&'static str, &'static str)> {
    let typed = open_token(text, caret).map(|(_, t)| t.to_lowercase()).unwrap_or_default();
    scope
        .variables()
        .iter()
        .filter(|(n, _)| {
            let n = n.to_lowercase();
            let mut it = n.chars();
            typed.chars().all(|c| it.any(|x| x == c))
        })
        .copied()
        .collect()
}

/// Complete the token before the caret with `name}`.
fn accept(input: &Entity<InputState>, name: &str, window: &mut Window, cx: &mut App) {
    let (text, caret) = {
        let s = input.read(cx);
        (s.value().to_string(), s.cursor())
    };
    let Some((start, _)) = open_token(&text, caret) else { return };
    // Replace the rest of the name under the caret and its `}` too.
    let rest = text[caret..].trim_start_matches(|c: char| c.is_alphanumeric() || c == '.' || c == '_');
    let rest = rest.strip_prefix('}').unwrap_or(rest);
    let new = format!("{}{}}}{}", &text[..start], name, rest);
    let at = (start + name.len() + 1) as u32;
    input.update(cx, |s, cx| {
        s.set_value(new, window, cx);
        s.set_cursor_position(Position { line: 0, character: at }, window, cx);
    });
}

/// Colour runs for the convention: valid placeholders blue, others red.
fn colored_runs(text: &str, scope: ExportElement) -> Vec<(std::ops::Range<usize>, HighlightStyle)> {
    er::placeholders(text, scope)
        .into_iter()
        .map(|p| (p.range, HighlightStyle { color: Some(if p.valid { rgb(BLUE) } else { rgb(RED) }.into()), ..Default::default() }))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::prelude::v1::test;

    #[test]
    fn completion_follows_the_typed_token() {
        assert_eq!(open_token("A_${na", 6), Some((4, "na".into())));
        assert_eq!(open_token("A_${name}", 9), None);
        let c = completions("${pn", 4, ExportElement::Part);
        assert!(c.iter().any(|(n, _)| *n == "partNumber"));
        assert!(!completions("${partNumber", 12, ExportElement::PartStudio).iter().any(|(n, _)| *n == "partNumber"));
    }
}
