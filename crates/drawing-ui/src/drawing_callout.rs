//! Drawing editor: Onshape's Callout dialog (live drawing/180–185): five
//! property buttons (Part property, BOM table property, Instance property,
//! Flag note, Cut list table property) that insert tokens, the text above,
//! the left | centre | right boxes (the centre empty = "Table: Item No."),
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
}

impl DrawingEditor {
    fn new_callout_dlg(&mut self, edit: Option<u64>, text: &str, style: CalloutStyle, window: &mut Window, cx: &mut Context<Self>) -> CalloutDlg {
        let h = style.height.unwrap_or(self.drawing.props.ann_type(noshape_model::drawing::sheet::AnnType::Callouts).height.unwrap_or(self.drawing.props.ann_text_height));
        let height = if self.drawing.units_mm { noshape_model::drawing::format::format_number(h, None, 2).to_string() } else { format!("{:.4}", h / 25.4) };
        let vals = [style.above.clone(), style.left.clone(), text.to_string(), style.right.clone(), style.below.clone(), height];
        let phs = ["", "", "Table: Item No.", "", "", ""];
        let inputs = vals.into_iter().zip(phs).map(|(v, ph)| cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).placeholder(ph).default_value(v))).collect();
        CalloutDlg { edit, cached: (text.to_string(), style.clone()), style, inputs, target: CENTRE }
    }

    /// Create / drop the dialog with the Callout tool (called from render).
    pub fn sync_callout_dlg(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let active = matches!(self.tool, Tool::Balloon { .. });
        let placing = self.callout_dlg.as_ref().is_some_and(|d| d.edit.is_none());
        if active && self.callout_dlg.is_none() {
            self.callout_dlg = Some(self.new_callout_dlg(None, "", CalloutStyle::default(), window, cx));
        } else if !active && placing {
            self.callout_dlg = None;
        }
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
        Some((v(CENTRE), s))
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

    /// Insert a token or symbol into the target box.
    fn callout_insert(&mut self, token: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(d) = &self.callout_dlg else { return };
        let input = d.inputs[d.target].clone();
        let token = token.to_string();
        input.update(cx, |s, cx| {
            let v = format!("{}{token}", s.value());
            s.set_value(v, window, cx);
        });
        cx.notify();
    }

    pub fn render_callout_dlg(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        self.refresh_callout_values(cx);
        let d = self.callout_dlg.as_ref()?;
        let pal = p();
        let ink = gpui_kit::rgb(0x333333);
        let inputs = d.inputs.clone();
        let style = d.style.clone();
        let editing = d.edit.is_some();
        let ed = cx.entity();
        // Flag notes and cut lists enable their buttons.
        let sheet = self.current_sheet();
        let has_flags = sheet.annotations.iter().any(|a| matches!(&a.kind, AnnotationKind::Note { style, .. } if !style.flags.is_empty()));
        let has_cut_list = self.drawing.sheets.iter().flat_map(|s| &s.tables).any(|t| matches!(t.kind, TableKind::CutList { .. }));
        // A property button: icon + caret, a list of tokens.
        let prop = |id: &'static str, icon: &'static str, tip: &'static str, enabled: bool, items: Vec<(&'static str, String)>| {
            let ed = ed.clone();
            let btn = Button::new(id).ghost().xsmall().w(px(45.)).h(px(30.)).dropdown_caret(true).child(crate::ui::glyph(icon, 18., ink)).tooltip(tip);
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
        let part_props: Vec<(&str, String)> = vec![("Name", "{part.name}".into()), ("Part number", "{part.number}".into()), ("Material", "{part.material}".into()), ("Description", "{part.description}".into())];
        let bom_props: Vec<(&str, String)> = vec![("Item No.", "{bom.item}".into()), ("Quantity", "{bom.quantity}".into()), ("Part number", "{bom.number}".into()), ("Name", "{bom.name}".into()), ("Material", "{bom.material}".into())];
        let inst_props: Vec<(&str, String)> = vec![("Reference designator", "{instance.designator}".into())];
        // A text box that becomes the token target when clicked.
        let boxed = |i: usize, w: f32, h: f32, cx: &mut Context<Self>| {
            div()
                .w(px(w))
                .when(h > 30., |d| d.py(px((h - 24.) / 2.)))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _: &MouseDownEvent, _, _| {
                        if let Some(d) = &mut this.callout_dlg {
                            d.target = i;
                        }
                    }),
                )
                .child(Input::new(&inputs[i]).small())
        };
        let select = |id: &'static str, label: String, w: f32, items: Vec<StyleChoice>| {
            let ed = ed.clone();
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
            })
        };
        let borders: Vec<StyleChoice> = CalloutBorder::ALL.iter().map(|&b| (b.name().to_string(), style.border == b, std::rc::Rc::new(move |s: &mut CalloutStyle| s.border = b) as std::rc::Rc<dyn Fn(&mut CalloutStyle)>)).collect();
        let sizes: Vec<StyleChoice> = SIZES.iter().map(|&(n, l)| (l.to_string(), style.chars == n, std::rc::Rc::new(move |s: &mut CalloutStyle| s.chars = n) as std::rc::Rc<dyn Fn(&mut CalloutStyle)>)).collect();
        let size_label = SIZES.iter().find(|s| s.0 == style.chars).map(|s| s.1).unwrap_or("2 Characters");
        let symbols = {
            let ed = ed.clone();
            Button::new("callout-symbol").label("Ø").ghost().xsmall().w(px(40.)).dropdown_caret(true).os_dropdown_menu("callout-symbol-dd", false, move |mut m, _, _| {
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
                .absolute()
                .top(px(DIALOG_TOP))
                .left(px(self.dialog_left()))
                .w(px(364.))
                .bg(pal.dialog_bg)
                .shadow(crate::drawing_chrome::dialog_shadow())
                .text_color(ink)
                .text_size(px(12.))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    h_flex()
                        .h(px(28.))
                        .pl(px(11.))
                        .border_b_1()
                        .border_color(gpui_kit::rgb(0xd3d7d9))
                        .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Callout"))
                        .child(div().id("callout-ok").w(px(28.)).h(px(27.)).flex().items_center().justify_center().cursor_pointer().bg(gpui_kit::rgb(0x009500)).when(!editing, |d| d.bg(gpui_kit::rgb(crate::drawing_chrome::OK_DISABLED))).child(crate::ui::glyph("check", 18., gpui_kit::white())).when(editing, |d| d.on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_callout_dlg(true, cx)))))
                        .child(div().id("callout-cancel").w(px(28.)).h(px(27.)).flex().items_center().justify_center().cursor_pointer().child(crate::drawing_chrome::cancel_x(cx)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_callout_dlg(false, cx)))),
                )
                .child(
                    v_flex()
                        .p(px(4.))
                        .gap(px(4.))
                        .child(
                            h_flex()
                                .gap(px(6.))
                                .child(prop("callout-part", "part", "Part property", true, part_props))
                                .child(prop("callout-bom", "bom", "BOM table property", true, bom_props))
                                .child(prop("callout-inst", "instance", "Instance property", true, inst_props))
                                .child(prop("callout-flag", "note", "Flag note", has_flags, vec![]))
                                .child(prop("callout-cut", "cut-list", "Cut list table property", has_cut_list, vec![("Item", "{cut.item}".into()), ("Quantity", "{cut.quantity}".into())])),
                        )
                        .child(boxed(ABOVE, 356., 24., cx))
                        .child(h_flex().gap(px(8.)).child(boxed(LEFT, 82., 66., cx)).child(boxed(CENTRE, 172., 66., cx)).child(boxed(RIGHT, 82., 66., cx)))
                        .child(boxed(BELOW, 356., 24., cx))
                        .child(
                            h_flex()
                                .gap(px(4.))
                                .child(symbols)
                                .child(div().w(px(70.)).child(Input::new(&inputs[HEIGHT]).text_size(px(13.33)).text_color(gpui_kit::rgb(0x555555)).xsmall()))
                                .child(select("callout-border", style.border.name().into(), 110., borders))
                                .child(select("callout-size", size_label.into(), 116., sizes)),
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
                    .when(!d.reorder, |e| e.tooltip(|w, cx| gpui_kit::component::tooltip::Tooltip::new("Reorder items").build(w, cx)))
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
                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    h_flex()
                        .h(px(28.))
                        .pl(px(11.))
                        .border_b_1()
                        .border_color(gpui_kit::rgb(0xd3d7d9))
                        .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Stacked callouts"))
                        .child(div().id("stack-ok").w(px(28.)).h(px(27.)).flex().items_center().justify_center().cursor_pointer().bg(gpui_kit::rgb(0x009500)).child(crate::ui::glyph("check", 18., gpui_kit::white())).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_stack_dlg(true, cx))))
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
