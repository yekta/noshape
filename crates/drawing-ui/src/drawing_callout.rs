//! Drawing editor: Onshape's Callout dialog (live drawing/180–185): five
//! property buttons (Part property, BOM table property, Instance property,
//! Flag note, Cut list table property) that insert tokens, the text above,
//! the left | centre | right boxes (the centre "Table: Item No.", focused),
//! the text below, then Insert symbol, Text height, Border shape and Border
//! size. Shown while the Callout tool is active (new callouts take it) or
//! to edit a callout (double-click / Edit…).

use crate::drawing_view::*;
use crate::os_ui::OsDropdownExt as _;
use crate::theme::p;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{Selectable as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::drawing::{AnnotationKind, CalloutBorder, CalloutStyle, TableKind};

/// The text boxes, in [`CalloutDlg::inputs`] order.
const ABOVE: usize = 0;
const LEFT: usize = 1;
const CENTRE: usize = 2;
const RIGHT: usize = 3;
const BELOW: usize = 4;
const HEIGHT: usize = 5;

/// The centre box's default: the BOM item number (a callout's empty text).
const ITEM: &str = "{bom.item}";

/// Border size choices: 0 = Tight Fit.
const SIZES: [(u8, &str); 6] = [(0, "Tight Fit"), (1, "1 Character"), (2, "2 Characters"), (3, "3 Characters"), (4, "4 Characters"), (5, "5 Characters")];

/// Insert symbol choices.
const SYMBOLS: [(&str, &str); 10] = [("Ø", "Diameter"), ("°", "Degree"), ("±", "Plus/minus"), ("⌴", "Counterbore"), ("⌵", "Countersink"), ("↧", "Depth"), ("□", "Square"), ("⌒", "Arc length"), ("∠", "Angle"), ("℄", "Centerline")];

/// A select choice of the dialog: label, checked, what it sets.
type StyleChoice = (String, bool, std::rc::Rc<dyn Fn(&mut CalloutStyle)>);

/// Callout dialog state.
pub struct CalloutDlg {
    /// Editing this callout (else the Callout tool's settings).
    pub edit: Option<u64>,
    pub style: CalloutStyle,
    inputs: Vec<Entity<InputState>>,
    /// The box tokens go into (the last focused, else the centre).
    target: usize,
    /// The values as last read (for previews without a context).
    cached: (String, CalloutStyle),
    /// The focused box (its blue frame).
    focused: Option<usize>,
    /// Once the anchor is picked: the centre's text as the callout shows it ("1"; live workflows-1123 10 s15).
    shown: Option<String>,
}

/// The callout dialog's property tokens: group (its button and the token's
/// label prefix), name, code. Live shows them as "<group>: <name>" tokens
/// ("Table: Item No.", drawing.md A23).
const CALLOUT_TOKENS: [(&str, &str, &str); 12] = [
    ("Part", "Name", "{part.name}"),
    ("Part", "Part number", "{part.number}"),
    ("Part", "Material", "{part.material}"),
    ("Part", "Description", "{part.description}"),
    ("Table", "Item No.", "{bom.item}"),
    ("Table", "Quantity", "{bom.quantity}"),
    ("Table", "Part number", "{bom.number}"),
    ("Table", "Name", "{bom.name}"),
    ("Table", "Material", "{bom.material}"),
    ("Instance", "Reference designator", "{instance.designator}"),
    ("Cut list", "Item", "{cut.item}"),
    ("Cut list", "Quantity", "{cut.quantity}"),
];

/// A callout property code as its token ("Table: Item No.").
fn callout_token(code: &str) -> Option<gpui_kit::component::input::InlineToken> {
    let (g, l, c) = CALLOUT_TOKENS.iter().find(|(_, _, c)| *c == code)?;
    Some(gpui_kit::component::input::InlineToken::new(c.to_string(), c.to_string()).with_label(format!("{g}: {l}")))
}

/// The byte ranges of the callout codes in `text` (BOM, instance and cut
/// list codes aren't sheet fields).
fn callout_codes(text: &str) -> Vec<std::ops::Range<usize>> {
    let mut out = Vec::new();
    for (_, _, c) in CALLOUT_TOKENS.iter().filter(|(g, _, _)| *g != "Part") {
        let mut at = 0;
        while let Some(i) = text[at..].find(c).map(|i| at + i) {
            out.push(i..i + c.len());
            at = i + c.len();
        }
    }
    out.sort_by_key(|r| r.start);
    out
}

impl DrawingEditor {
    fn new_callout_dlg(&mut self, edit: Option<u64>, text: &str, style: CalloutStyle, window: &mut Window, cx: &mut Context<Self>) -> CalloutDlg {
        let h = style.height.unwrap_or(self.drawing.props.ann_type(noshape_model::drawing::sheet::AnnType::Callouts).height.unwrap_or(self.drawing.props.ann_text_height));
        let height = if self.drawing.units_mm { noshape_model::drawing::format::format_number(h, None, 2).to_string() } else { format!("{:.4}", h / 25.4) };
        let vals = [style.above.clone(), style.left.clone(), text.to_string(), style.right.clone(), style.below.clone(), height];
        let inputs = vals
            .into_iter()
            .map(|v| {
                let st = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).default_value(v.clone()));
                // Its properties as tokens.
                let mut c = gpui_kit::component::input::InputContent::new(v.clone());
                for r in crate::drawing_note::field_codes(&v).into_iter().chain(callout_codes(&v)) {
                    if let Some(t) = callout_token(&v[r.clone()])
                        && let Ok(next) = c.clone().with_token(r, t)
                    {
                        c = next;
                    }
                }
                if !c.tokens().is_empty() {
                    st.update(cx, |s, cx| s.set_value(c, window, cx));
                }
                st
            })
            .collect();
        CalloutDlg { edit, cached: (text.to_string(), style.clone()), style, inputs, target: CENTRE, focused: None, shown: None }
    }

    /// Create / drop the dialog with the Callout tool (called from render).
    pub fn sync_callout_dlg(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let active = matches!(self.tool, Tool::Balloon { .. });
        let placing = self.callout_dlg.as_ref().is_some_and(|d| d.edit.is_none());
        if active && self.callout_dlg.is_none() {
            // Live: "Table: Item No." in the centre box, focused.
            let d = self.new_callout_dlg(None, ITEM, CalloutStyle::default(), window, cx);
            d.inputs[CENTRE].update(cx, |s, cx| s.focus(window, cx));
            self.callout_dlg = Some(d);
        } else if !active && placing {
            self.callout_dlg = None;
        }
        // The anchor click shows the centre's text resolved, unfocused; the
        // placement click brings the token back, focused (live workflows-1123 10 s15-s16).
        let anchor = match &self.tool {
            Tool::Balloon { anchor: Some(p) } => Some(p.clone()),
            _ => None,
        };
        let shown = match (&anchor, self.callout_dlg.as_ref().map(|d| d.shown.is_some())) {
            (Some(p), Some(false)) => Some(self.pending_annotation().and_then(|mut a| {
                self.pk().finish_callout(&mut a, p);
                noshape_model::drawing::build::callout_shown_text(&self.drawing, self.sheet, &self.sources, &self.info, &a)
            })),
            _ => None,
        };
        let Some(d) = &mut self.callout_dlg else { return };
        if let Some(t) = shown {
            d.shown = Some(t.unwrap_or_default());
        } else if anchor.is_none() && d.shown.take().is_some() {
            d.inputs[CENTRE].update(cx, |s, cx| s.focus(window, cx));
        }
        d.focused = d.inputs.iter().position(|s| s.read(cx).focus_handle(cx).is_focused(window));
    }

    /// Edit… / double-click on a callout.
    pub fn open_callout_edit(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        let Some((text, style)) = self.current_sheet().annotations.iter().find(|a| a.id == id).and_then(|a| match &a.kind {
            AnnotationKind::Balloon { text, callout, .. } => Some((text.clone(), callout.clone())),
            _ => None,
        }) else {
            return;
        };
        self.tool = Tool::Select;
        self.panel = None;
        let text = if text.trim().is_empty() { ITEM.to_string() } else { text };
        self.callout_dlg = Some(self.new_callout_dlg(Some(id), &text, style, window, cx));
        cx.notify();
    }

    /// The dialog's text (centre box) and style.
    pub fn callout_values(&self, cx: &App) -> Option<(String, CalloutStyle)> {
        let d = self.callout_dlg.as_ref()?;
        let v = |i: usize| d.inputs[i].read(cx).value().to_string();
        let mut s = d.style.clone();
        s.above = v(ABOVE);
        s.left = v(LEFT);
        s.right = v(RIGHT);
        s.below = v(BELOW);
        let inch = !self.drawing.units_mm;
        s.height = v(HEIGHT).trim().trim_end_matches("in").trim_end_matches("mm").trim().parse::<f64>().ok().filter(|h| *h > 0.0).map(|h| if inch { h * 25.4 } else { h });
        // The token alone is the item number: an empty text, as before.
        let centre = v(CENTRE);
        Some((if centre.trim() == ITEM { String::new() } else { centre }, s))
    }

    /// The cached values (refreshed by [`Self::refresh_callout_values`]).
    pub fn callout_values_now(&self) -> Option<(String, CalloutStyle)> {
        self.callout_dlg.as_ref().map(|d| d.cached.clone())
    }

    /// Re-read the dialog into its cache (render, and before placing).
    pub fn refresh_callout_values(&mut self, cx: &App) {
        let v = self.callout_values(cx);
        if let (Some(d), Some(v)) = (&mut self.callout_dlg, v) {
            d.cached = v;
        }
    }

    fn close_callout_dlg(&mut self, ok: bool, cx: &mut Context<Self>) {
        let values = self.callout_values(cx);
        let Some(d) = self.callout_dlg.take() else { return };
        if ok
            && let (Some(id), Some((text, style))) = (d.edit, values)
        {
            self.edit_sheet("Edit callout", cx, |s| {
                if let Some(AnnotationKind::Balloon { text: t, callout, .. }) = s.annotations.iter_mut().find(|a| a.id == id).map(|a| &mut a.kind) {
                    *t = text;
                    // Keep what the dialog doesn't show.
                    let (on_face, stacked) = (callout.on_face, std::mem::take(&mut callout.stacked));
                    *callout = CalloutStyle { on_face, stacked, ..style };
                }
            });
        }
        if d.edit.is_none() {
            self.tool = Tool::Select;
        }
        cx.notify();
    }

    /// Insert a property (as a token: live's grey "Table: Item No." button)
    /// or a symbol into the target box, at its caret.
    fn callout_insert(&mut self, token: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(d) = &self.callout_dlg else { return };
        let input = d.inputs[d.target].clone();
        let token = token.to_string();
        input.update(cx, |s, cx| match callout_token(&token) {
            Some(t) if s.replace_with_token(t.clone(), window, cx).is_ok() => {}
            _ => s.insert(token.clone(), window, cx),
        });
        cx.notify();
    }

    pub fn render_callout_dlg(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        self.refresh_callout_values(cx);
        let d = self.callout_dlg.as_ref()?;
        // (The panel's texts in reading order for the ui-parity runner, as live's recorder reads them: live 2146.)
        crate::ui_script::note_dialog(|| serde_json::json!({ "kind": "drawing-panel", "title": "Callout", "text": ["Callout", "None", "Circle", "Diamond", "Double Circle", "Flag", "Hexagon", "Octagon", "Rectangle", "Split Circle", "Square Circle", "Triangle", "Underline", "Tight Fit", "1 Character", "2 Characters", "3 Characters", "4 Characters", "5 Characters"] }));
        let pal = p();
        let ink = gpui_kit::rgb(0x333333);
        let inputs = d.inputs.clone();
        let style = d.style.clone();
        let editing = d.edit.is_some();
        let (focused, shown) = (d.focused, d.shown.clone());
        let ed = cx.entity();
        // Flag notes and cut lists enable their buttons.
        let sheet = self.current_sheet();
        let has_flags = sheet.annotations.iter().any(|a| matches!(&a.kind, AnnotationKind::Note { style, .. } if !style.flags.is_empty()));
        let has_cut_list = self.drawing.sheets.iter().flat_map(|s| &s.tables).any(|t| matches!(t.kind, TableKind::CutList { .. }));
        // A property button: icon + caret, a list of tokens.
        let prop = |id: &'static str, icon: AnyElement, tip: &'static str, enabled: bool, items: Vec<(&'static str, String)>| {
            let ed = ed.clone();
            let btn = Button::new(id).ghost().xsmall().w(px(45.)).h(px(30.)).dropdown_caret(true).child(icon).tooltip(tip);
            let btn = btn.when(!enabled, |b| b.opacity(0.4));
            if !enabled {
                return btn.into_any_element();
            }
            btn.os_dropdown_menu(SharedString::from(format!("{id}-dd")), false, move |mut m, _, _| {
                for (label, token) in &items {
                    let (e, token) = (ed.clone(), token.clone());
                    m = m.item(crate::os_menu::OsMenuItem::new(*label).on_click(move |_, window, cx| e.update(cx, |t, cx| t.callout_insert(&token, window, cx))));
                }
                m
            })
            .into_any_element()
        };
        let tokens_of = |group: &str| CALLOUT_TOKENS.iter().filter(|(g, _, _)| *g == group).map(|(_, l, c)| (*l, c.to_string())).collect::<Vec<(&str, String)>>();
        let (part_props, bom_props, inst_props) = (tokens_of("Part"), tokens_of("Table"), tokens_of("Instance"));
        // A text box that becomes the token target when clicked (live: 1 px
        // #ddd frame, radius 4, the focused one #1b5faa; its text centred
        // on the first line).
        let boxed = |i: usize, w: f32, h: f32, cx: &mut Context<Self>| {
            let text = match (i == CENTRE, &shown) {
                (true, Some(t)) => h_flex().h(px(20.)).justify_center().child(div().bg(gpui_kit::rgb(0xdddddd)).text_color(gpui_kit::rgb(0x000000)).child(t.clone())).into_any_element(),
                _ => Input::new(&inputs[i]).appearance(false).text_align(TextAlign::Center).h(px(20.)).xsmall().into_any_element(),
            };
            div()
                .w(px(w))
                .h(px(h))
                .px(px(3.))
                .py(px(2.))
                .bg(gpui_kit::rgb(0xffffff))
                .border_1()
                .border_color(gpui_kit::rgb(if focused == Some(i) { 0x1b5faa } else { 0xdddddd }))
                .rounded(px(4.))
                .text_size(px(13.33))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _: &MouseDownEvent, _, _| {
                        if let Some(d) = &mut this.callout_dlg {
                            d.target = i;
                        }
                    }),
                )
                .child(text)
        };
        let select = |id: &'static str, label: String, w: f32, items: Vec<StyleChoice>| {
            let ed = ed.clone();
            let shown = label.clone();
            Button::new(id).label(label).ghost().xsmall().w(px(w)).dropdown_caret(true).os_dropdown_menu(SharedString::from(format!("{id}-dd")), true, move |mut m, _, _| {
                for (l, on, f) in &items {
                    let (e, f) = (ed.clone(), f.clone());
                    m = m.item(crate::os_menu::OsMenuItem::new(l.clone()).checked(*on).on_click(move |_, _, cx| {
                        e.update(cx, |t, cx| {
                            if let Some(d) = &mut t.callout_dlg {
                                f(&mut d.style);
                            }
                            cx.notify();
                        })
                    }));
                }
                m
            }).map(|d| crate::ui_script::mark(d, move || format!("dropdown:{shown}")))
        };
        let borders: Vec<StyleChoice> = CalloutBorder::ALL.iter().map(|&b| (b.name().to_string(), style.border == b, std::rc::Rc::new(move |s: &mut CalloutStyle| s.border = b) as std::rc::Rc<dyn Fn(&mut CalloutStyle)>)).collect();
        let sizes: Vec<StyleChoice> = SIZES.iter().map(|&(n, l)| (l.to_string(), style.chars == n, std::rc::Rc::new(move |s: &mut CalloutStyle| s.chars = n) as std::rc::Rc<dyn Fn(&mut CalloutStyle)>)).collect();
        let size_label = SIZES.iter().find(|s| s.0 == style.chars).map(|s| s.1).unwrap_or("2 Characters");
        let symbols = {
            let ed = ed.clone();
            Button::new("callout-symbol").label("Ø").ghost().xsmall().w(px(45.)).dropdown_caret(true).os_dropdown_menu("callout-symbol-dd", false, move |mut m, _, _| {
                for (sym, name) in SYMBOLS {
                    let e = ed.clone();
                    m = m.item(crate::os_menu::OsMenuItem::new(format!("{sym}  {name}")).on_click(move |_, window, cx| e.update(cx, |t, cx| t.callout_insert(sym, window, cx))));
                }
                m
            })
        };
        Some(
            v_flex()
                .id("dr-callout-dlg")
                .key_context("CalloutDialog")
                .absolute()
                .top(px(DIALOG_TOP))
                .left(px(self.dialog_left()))
                .w(px(364.))
                .bg(pal.dialog_bg)
                .shadow(crate::drawing_chrome::dialog_shadow())
                .text_color(ink)
                .text_size(px(12.))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_move(cx.listener(Self::pointer_off_sheet))
                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    h_flex()
                        .h(px(28.))
                        .pl(px(11.))
                        .border_b_1()
                        .border_color(gpui_kit::rgb(0xd3d7d9))
                        .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Callout"))
                        .child(div().id("callout-ok").w(px(28.)).h(px(27.)).flex().items_center().justify_center().cursor_pointer().bg(gpui_kit::rgb(0x009500)).when(!editing, |d| d.bg(gpui_kit::rgb(crate::drawing_chrome::OK_DISABLED))).child(crate::drawing_chrome::ok_check(18.)).when(editing, |d| d.on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_callout_dlg(true, cx)))))
                        .child(div().id("callout-cancel").w(px(28.)).h(px(27.)).flex().items_center().justify_center().cursor_pointer().child(crate::drawing_chrome::cancel_x(cx)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_callout_dlg(false, cx)))),
                )
                .child(
                    v_flex()
                        .p(px(4.))
                        .gap(px(4.))
                        .child(
                            h_flex()
                                .gap(px(2.))
                                .child(prop("callout-part", crate::assets::icon("callout-part", 18., cx), "Part property", true, part_props))
                                .child(prop("callout-bom", crate::assets::icon("callout-bom", 18., cx), "BOM table property", true, bom_props))
                                .child(prop("callout-inst", crate::assets::icon("callout-instance", 18., cx), "Instance property", true, inst_props))
                                .child(prop("callout-flag", crate::ui::glyph("note", 18., ink).into_any_element(), "Flag note", has_flags, vec![]))
                                .child(prop("callout-cut", crate::ui::glyph("cut-list", 18., ink).into_any_element(), "Cut list table property", has_cut_list, tokens_of("Cut list"))),
                        )
                        .child(boxed(ABOVE, 356., 26., cx))
                        .child(h_flex().gap(px(10.)).child(boxed(LEFT, 83., 66., cx)).child(boxed(CENTRE, 170., 66., cx)).child(boxed(RIGHT, 83., 66., cx)))
                        .child(boxed(BELOW, 356., 26., cx))
                        .child(
                            h_flex()
                                .gap(px(2.))
                                .child(symbols)
                                .child(div().w(px(75.)).child(Input::new(&inputs[HEIGHT]).text_size(px(13.33)).text_color(gpui_kit::rgb(0x555555)).xsmall()))
                                .child(select("callout-border", style.border.name().into(), 115., borders))
                                .child(select("callout-size", size_label.into(), 115., sizes)),
                        ),
                )
                .into_any_element(),
        )
    }
}

/// A stacked instance row being dragged (its index); no ghost.
#[derive(Clone)]
struct StackDrag(usize);

impl Render for StackDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

/// Add stacked callouts (live drawing/191–194): the anchor callout, the
/// picked instances ("Stacked instances", each with ✕), and Add instances
/// from BOM (then clicks on the BOM table's first column add its rows).
pub struct StackDlg {
    pub id: u64,
    pub picks: Vec<(String, noshape_model::BodyId)>,
    pub from_bom: bool,
    /// The Stacked instances list in reorder mode (live request 1682: ⇅
    /// "Reorder items" shows ≡ drag handles and DONE, the × buttons hide).
    pub reorder: bool,
}

impl DrawingEditor {
    /// The source model of a callout's view.
    fn callout_source(&self, id: u64) -> Option<&noshape_model::drawing::views::SourceModel> {
        let a = self.current_sheet().annotations.iter().find(|a| a.id == id)?;
        let (key, _) = noshape_model::drawing::views::view_source(self.current_sheet(), a.kind.view()?)?;
        self.sources.get(&key)
    }

    /// Callout menu › Add stacked callouts… (drawings with a BOM table).
    pub fn open_stack_dlg(&mut self, id: u64, cx: &mut Context<Self>) {
        let Some(AnnotationKind::Balloon { callout, .. }) = self.current_sheet().annotations.iter().find(|a| a.id == id).map(|a| &a.kind) else { return };
        let bodies: Vec<noshape_model::BodyId> = callout.stacked.iter().filter_map(|s| s.body.clone()).collect();
        let name = |b: &noshape_model::BodyId| self.callout_source(id).and_then(|s| s.part(Some(b))).map(|p| p.name.clone()).unwrap_or_default();
        let picks = bodies.iter().map(|b| (name(b), b.clone())).collect();
        // Its view framed while the dialog is open, the instances picked there (live drw-callout-stacked-add).
        let view = self.current_sheet().annotations.iter().find(|a| a.id == id).and_then(|a| a.kind.view());
        self.selection = view.map(Sel::View).into_iter().collect();
        self.stack_dlg = Some(StackDlg { id, picks, from_bom: false, reorder: false });
        self.tool = Tool::Select;
        cx.notify();
    }

    /// A sheet click while the panel is open: an instance in a view, or a
    /// BOM row (first column) in BOM mode. True when used.
    pub fn stack_click(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(d) = &self.stack_dlg else { return false };
        let (id, from_bom) = (d.id, d.from_bom);
        let c = self.cursor;
        let body = if from_bom {
            // The BOM row under the cursor: its item number → its first part.
            let sheet = self.current_sheet();
            let hit = sheet.tables.iter().filter(|t| matches!(t.kind, TableKind::Bom { .. })).find_map(|t| {
                let l = self.built.as_ref()?.table_layout(t.id)?;
                if !l.contains(c, 0.0) || c[0] > *l.xs.get(1)? {
                    return None;
                }
                let t0 = l.title as usize + 1;
                let r = (t0..l.ys.len() - 1).find(|&k| c[1] <= l.ys[k] && c[1] >= l.ys[k + 1])?;
                Some((t.id, r - l.title as usize))
            });
            hit.and_then(|(table, r)| {
                let item: usize = self.cell_text(table, r, 0).trim().parse().ok()?;
                self.callout_source(id)?.bom.iter().find(|e| e.item == item)?.bodies.first().cloned()
            })
        } else {
            // An instance's edge: its part (a point on it carries no
            // reference when it isn't a snap point).
            self.any_point(c).and_then(|p| p.r).map(|r| r.body().clone()).or_else(|| self.pk().hovered_edge().map(|(_, me)| me.body))
        };
        let Some(body) = body else { return true };
        let name = self.callout_source(id).and_then(|s| s.part(Some(&body))).map(|p| p.name.clone()).unwrap_or_default();
        if let Some(d) = &mut self.stack_dlg {
            d.picks.push((name, body));
        }
        cx.notify();
        true
    }

    fn close_stack_dlg(&mut self, ok: bool, cx: &mut Context<Self>) {
        let Some(d) = self.stack_dlg.take() else { return };
        self.selection.clear();
        if ok {
            let stacked: Vec<noshape_model::drawing::StackedCallout> = d.picks.iter().map(|(_, b)| noshape_model::drawing::StackedCallout { text: String::new(), body: Some(b.clone()) }).collect();
            self.edit_sheet("Add stacked callouts", cx, |s| {
                if let Some(AnnotationKind::Balloon { callout, .. }) = s.annotations.iter_mut().find(|a| a.id == d.id).map(|a| &mut a.kind) {
                    callout.stacked = stacked;
                }
            });
        }
        cx.notify();
    }

    pub fn render_stack_dlg(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let d = self.stack_dlg.as_ref()?;
        let pal = p();
        let ink = gpui_kit::rgb(0x333333);
        let anchor = self.current_sheet().annotations.iter().find(|a| a.id == d.id).and_then(|a| {
            // Its reference, else the part stored when it was placed on an edge.
            let stored = match &a.kind {
                AnnotationKind::Balloon { callout, .. } => callout.part.clone(),
                _ => None,
            };
            let body = a.refs.iter().find(|r| r.slot == 0).map(|r| r.point.body().clone()).or(stored)?;
            self.callout_source(d.id)?.part(Some(&body)).map(|p| p.name.clone())
        });
        let mut list = v_flex()
            .mx(px(4.))
            .p(px(4.))
            .border_1()
            .border_color(pal.accent)
            .bg(gpui_kit::rgb(0xe4f1fb))
            .child(h_flex().child(div().flex_1().text_size(px(10.)).text_color(pal.text_muted).child("Stacked instances")).child(
                // Live request 1682: ⇅ ("Reorder items") switches the list to
                // reorder mode, DONE back.
                div()
                    .id("stack-reorder")
                    .text_size(px(11.))
                    .text_color(if d.reorder { pal.accent } else { pal.text_muted })
                    .cursor_pointer()
                    .child(if d.reorder { "DONE" } else { "⇅" })
                    .when(!d.reorder, |e| e.tooltip(crate::ui::tooltip("Reorder items")))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        if let Some(d) = &mut this.stack_dlg {
                            d.reorder = !d.reorder;
                        }
                        cx.notify();
                    })),
            ));
        let reorder = d.reorder;
        for (i, (name, _)) in d.picks.iter().enumerate() {
            let row = h_flex().id(("stack-row", i)).h(px(20.)).gap(px(4.));
            // Reorder mode: a ≡ handle, the row dragged onto another moves
            // there; else its × removes it.
            let row = if reorder {
                row.cursor_grab()
                    .on_drag(StackDrag(i), |d, _, _, cx| cx.new(|_| d.clone()))
                    .on_drop(cx.listener(move |this, from: &StackDrag, _, cx| {
                        if let Some(d) = &mut this.stack_dlg
                            && from.0 < d.picks.len()
                            && i < d.picks.len()
                        {
                            let x = d.picks.remove(from.0);
                            d.picks.insert(i, x);
                        }
                        cx.notify();
                    }))
                    .child(div().text_color(pal.text_muted).child("≡"))
                    .child(div().flex_1().text_size(px(12.)).child(crate::ui::user_text(name.clone())))
            } else {
                row.child(div().flex_1().text_size(px(12.)).child(crate::ui::user_text(name.clone()))).child(
                    div().id(("stack-x", i)).px(px(4.)).cursor_pointer().text_color(pal.text_muted).child("×").on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if let Some(d) = &mut this.stack_dlg
                            && i < d.picks.len()
                        {
                            d.picks.remove(i);
                        }
                        cx.notify();
                    })),
                )
            };
            list = list.child(row);
        }
        // Its texts for the ui-parity runner (live2 drw-callout-stacked-add).
        crate::ui_script::note_dialog(|| {
            let mut text: Vec<String> = vec!["Stacked callouts".into(), "Anchor:".into()];
            text.extend(anchor.clone());
            text.extend(["Add instances from BOM".to_string(), "Stacked instances".to_string()]);
            text.extend(d.picks.iter().map(|(n, _)| n.clone()));
            serde_json::json!({ "kind": "drawing-panel", "title": "Stacked callouts", "text": text })
        });
        Some(
            v_flex()
                .id("dr-stack-dlg")
                .absolute()
                .top(px(DIALOG_TOP))
                .left(px(self.dialog_left()))
                .w(px(200.))
                .pb(px(4.))
                .gap(px(4.))
                .bg(pal.dialog_bg)
                .shadow(crate::drawing_chrome::dialog_shadow())
                .text_color(ink)
                .text_size(px(12.))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_move(cx.listener(Self::pointer_off_sheet))
                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    h_flex()
                        .h(px(28.))
                        .pl(px(11.))
                        .border_b_1()
                        .border_color(gpui_kit::rgb(0xd3d7d9))
                        .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Stacked callouts"))
                        .child(div().id("stack-ok").w(px(28.)).h(px(27.)).flex().items_center().justify_center().cursor_pointer().bg(gpui_kit::rgb(0x009500)).child(crate::drawing_chrome::ok_check(18.)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_stack_dlg(true, cx))))
                        .child(div().id("stack-cancel").w(px(28.)).h(px(27.)).flex().items_center().justify_center().cursor_pointer().child(crate::drawing_chrome::cancel_x(cx)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_stack_dlg(false, cx)))),
                )
                .child(h_flex().px(px(8.)).gap(px(10.)).child(div().text_size(px(11.)).child("Anchor:")).child(div().text_size(px(11.)).child(anchor.unwrap_or_default())))
                .child(
                    div().mx(px(4.)).child(Button::new("stack-from-bom").label("Add instances from BOM").small().w_full().when(d.from_bom, |b| b.selected(true)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        if let Some(d) = &mut this.stack_dlg {
                            d.from_bom = !d.from_bom;
                        }
                        cx.notify();
                    }))),
                )
                .child(list)
                .into_any_element(),
        )
    }

    /// The yellow "Select BOM instances" banner with Done (BOM mode).
    pub fn render_stack_banner(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.stack_dlg.as_ref().is_some_and(|d| d.from_bom) {
            return None;
        }
        Some(
            h_flex()
                .absolute()
                .top(px(DIALOG_TOP))
                .left(px(0.))
                .right(px(0.))
                .justify_center()
                .child(
                    h_flex()
                        .gap(px(12.))
                        .px(px(14.))
                        .h(px(32.))
                        .bg(gpui_kit::rgb(0xfff3c4))
                        .border_1()
                        .border_color(gpui_kit::rgb(0xe8c547))
                        .text_size(px(12.))
                        .child("Select BOM instances")
                        .child(Button::new("stack-bom-done").label("Done").small().on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            if let Some(d) = &mut this.stack_dlg {
                                d.from_bom = false;
                            }
                            cx.notify();
                        }))),
                )
                .into_any_element(),
        )
    }
}

#[cfg(test)]
impl DrawingEditor {
    /// The Callout dialog's centre box: its text, the focused box, the text shown once the anchor is picked.
    pub(crate) fn callout_centre_state(&self, cx: &App) -> Option<(String, Option<usize>, Option<String>)> {
        let d = self.callout_dlg.as_ref()?;
        Some((d.inputs[CENTRE].read(cx).value().to_string(), d.focused, d.shown.clone()))
    }
}
