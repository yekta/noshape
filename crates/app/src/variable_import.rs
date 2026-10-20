//! Variable Studio › Variable type "From import" (help: Variable Studio ›
//! "Importing a variable from a CSV table", image csv-variables-02): the
//! Value cell's pencil opens the Variable dialog on its From import tab:
//! Name, CSV file (Select CSV file… → the document's CSV file tabs or
//! Import), ⌄ Row and ⌄ Column (Access type: By index / By index range /
//! By label / All values; Index (0-based), Min / Max index, Value +
//! Regular expression; Result type: Single value / Array / Map + Label
//! column / row index), ☐ Conversion factor, Description; ✓ / ✕.

use crate::doc_model::DocModel;
use crate::os_ui as os;
use crate::theme::p;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::TabContent;
use noshape_model::document::TabId;
use noshape_model::file_tab::FileKind;
use noshape_model::variables::{CsvAccess, CsvAxis, CsvResult, VarKind, VariableStudio};
use std::cell::RefCell;
use std::rc::Rc;

const ACCESS: [(&str, CsvAccess); 4] = [("By index", CsvAccess::ByIndex), ("By index range", CsvAccess::ByIndexRange), ("By label", CsvAccess::ByLabel), ("All values", CsvAccess::All)];
const RESULTS: [(&str, CsvResult); 3] = [("Single value", CsvResult::Single), ("Array", CsvResult::Array), ("Map", CsvResult::Map)];

/// The dialog's text fields of one axis (Row or Column).
struct AxisFields {
    index: Entity<InputState>,
    min: Entity<InputState>,
    max: Entity<InputState>,
    label: Entity<InputState>,
    label_index: Entity<InputState>,
}

impl AxisFields {
    fn new(a: &CsvAxis, window: &mut Window, cx: &mut App) -> Self {
        let num = |v: usize, window: &mut Window, cx: &mut App| cx.new(|cx| InputState::new(window, cx).default_value(v.to_string()));
        Self { index: num(a.index, window, cx), min: num(a.min, window, cx), max: num(a.max, window, cx), label: cx.new(|cx| InputState::new(window, cx).default_value(a.label.clone())), label_index: num(a.label_index, window, cx) }
    }

    /// Read the fields into `a`.
    fn apply(&self, a: &mut CsvAxis, cx: &App) {
        let n = |st: &Entity<InputState>, d: usize| st.read(cx).value().trim().parse().unwrap_or(d);
        a.index = n(&self.index, a.index);
        a.min = n(&self.min, a.min);
        a.max = n(&self.max, a.max);
        a.label = self.label.read(cx).value().to_string();
        a.label_index = n(&self.label_index, a.label_index);
    }
}

/// Open the dialog for variable `i` of the Variable Studio `tab`.
pub fn open(doc: Entity<DocModel>, tab: TabId, i: usize, window: &mut Window, cx: &mut App) {
    let Some(var) = studio(&doc, tab, cx).and_then(|s| s.variables.get(i)).cloned() else { return };
    let imp = Rc::new(RefCell::new(var.import.clone().unwrap_or_default()));
    let name = cx.new(|cx| InputState::new(window, cx).default_value(var.name.clone()));
    let desc = cx.new(|cx| InputState::new(window, cx).default_value(var.description.clone()));
    let conv_on = Rc::new(std::cell::Cell::new(imp.borrow().conversion.is_some()));
    let conv = cx.new(|cx| InputState::new(window, cx).default_value(imp.borrow().conversion.clone().unwrap_or_else(|| "1 in".into())));
    let (row, col) = (Rc::new(AxisFields::new(&imp.borrow().row, window, cx)), Rc::new(AxisFields::new(&imp.borrow().column, window, cx)));
    let open_sections = Rc::new(std::cell::Cell::new((true, true)));
    window.open_dialog(cx, move |dialog, _, cx| {
        let pal = p();
        let state = imp.borrow().clone();
        let file_name = state.file.and_then(|t| doc.read(cx).session.doc.tab(t)).filter(|t| matches!(t.content, TabContent::File(_))).map(|t| t.name.clone());
        let preview = noshape_model::variables::import_value(&doc.read(cx).session.doc, &state).map(|v| v.to_string()).unwrap_or_else(|_| "?".into());
        let title = format!("#{} = {}", name.read(cx).value(), preview);
        let underline = |st: &Entity<InputState>| gpui_kit::base::input::input_frame(div().flex_1().border_b_1().border_color(rgb(0xcccccc)).child(Input::new(st).appearance(false).h(px(26.)).text_size(px(14.))));
        let num_row = |label: &'static str, st: &Entity<InputState>| h_flex().h(px(30.)).pl(px(18.)).gap(px(8.)).child(div().text_size(px(13.33)).text_color(pal.text_muted).child(label)).child(div().w(px(60.)).ml_auto().child(Input::new(st).appearance(false).h(px(26.)).text_size(px(14.)).text_right()));
        let select = |id: &'static str, opts: Vec<String>, cur: usize, set: Rc<dyn Fn(usize)>| os::select_field(id, opts, cur, move |k, w, _| {
            set(k);
            w.refresh();
        });
        let axis_section = |is_row: bool, fields: &Rc<AxisFields>, a: &CsvAxis| {
            let (title, sec_id, label_index_label) = if is_row { ("Row", "imp-row", "Label column index") } else { ("Column", "imp-col", "Label row index") };
            let open = if is_row { open_sections.get().0 } else { open_sections.get().1 };
            let os2 = open_sections.clone();
            let header = h_flex()
                .id(sec_id)
                .h(px(32.))
                .gap(px(6.))
                .cursor_pointer()
                .child(os::glyph(if open { os::GLYPH_CHEVRON_DOWN } else { os::GLYPH_CHEVRON_RIGHT }, 14., pal.text))
                .child(div().text_size(px(14.)).text_color(pal.text).child(title))
                .on_click(move |_, w, _| {
                    let (r, c) = os2.get();
                    os2.set(if is_row { (!r, c) } else { (r, !c) });
                    w.refresh();
                });
            let mut body = v_flex().pl(px(10.)).gap(px(2.));
            if open {
                let (i1, i2) = (imp.clone(), imp.clone());
                body = body.child(div().pl(px(18.)).child(select(
                    if is_row { "imp-row-access" } else { "imp-col-access" },
                    ACCESS.iter().map(|x| x.0.to_string()).collect(),
                    ACCESS.iter().position(|x| x.1 == a.access).unwrap_or(0),
                    Rc::new(move |k| {
                        let mut s = i1.borrow_mut();
                        let ax = if is_row { &mut s.row } else { &mut s.column };
                        ax.access = ACCESS[k].1;
                    }),
                )));
                match a.access {
                    CsvAccess::ByIndex => body = body.child(num_row("Index (0-based)", &fields.index)),
                    CsvAccess::ByIndexRange => body = body.child(num_row("Min index", &fields.min)).child(num_row("Max index", &fields.max)),
                    CsvAccess::ByLabel => {
                        let i3 = imp.clone();
                        let regex = a.regex;
                        body = body.child(h_flex().h(px(30.)).pl(px(18.)).gap(px(8.)).child(div().text_size(px(13.33)).text_color(pal.text_muted).child("Value")).child(underline(&fields.label))).child(div().pl(px(14.)).child(os::checkbox(if is_row { "imp-row-re" } else { "imp-col-re" }, "Regular expression", regex).on_click(move |_, w, _| {
                            let mut s = i3.borrow_mut();
                            let ax = if is_row { &mut s.row } else { &mut s.column };
                            ax.regex = !regex;
                            w.refresh();
                        })));
                    }
                    CsvAccess::All => {}
                }
                body = body.child(
                    h_flex()
                        .h(px(34.))
                        .pl(px(18.))
                        .gap(px(8.))
                        .child(div().text_size(px(13.33)).text_color(pal.text_muted).child("Result type"))
                        .child(div().w(px(120.)).ml_auto().child(select(
                            if is_row { "imp-row-res" } else { "imp-col-res" },
                            RESULTS.iter().map(|x| x.0.to_string()).collect(),
                            RESULTS.iter().position(|x| x.1 == a.result).unwrap_or(0),
                            Rc::new(move |k| {
                                let mut s = i2.borrow_mut();
                                let ax = if is_row { &mut s.row } else { &mut s.column };
                                ax.result = RESULTS[k].1;
                            }),
                        ))),
                );
                if a.result == CsvResult::Map {
                    body = body.child(num_row(label_index_label, &fields.label_index));
                }
            }
            v_flex().child(header).child(body)
        };
        let (d1, imp1) = (doc.clone(), imp.clone());
        let file_field = h_flex()
            .id("imp-file")
            .h(px(34.))
            .px(px(8.))
            .gap(px(8.))
            .border_1()
            .border_color(rgb(0xcccccc))
            .cursor_pointer()
            .child(os::glyph(os::GLYPH_DOCUMENT, 18., pal.text_muted))
            .child(div().text_size(px(14.)).text_color(if file_name.is_some() { pal.text } else { pal.text_faint }).child(file_name.clone().unwrap_or_else(|| "Select CSV file...".into())))
            .on_click(move |_, window, cx| {
                let (doc, imp) = (d1.clone(), imp1.clone());
                crate::file_tab_view::pick_file(d1.clone(), "Select CSV file", &[FileKind::Csv], &["csv"], window, cx, move |f, window, cx| {
                    // The picked (or just imported) CSV file tab.
                    let id = doc.read(cx).session.doc.tabs.iter().find(|t| matches!(&t.content, TabContent::File(x) if *x == f)).map(|t| t.id);
                    imp.borrow_mut().file = id;
                    window.refresh();
                });
            });
        let conv_on2 = conv_on.clone();
        let conversion_on = conv_on.get();
        let body = v_flex()
            .p(px(10.))
            .gap(px(6.))
            .child(h_flex().gap(px(8.)).child(div().text_size(px(13.33)).text_color(pal.text_muted).child("Name")).child(underline(&name)))
            .child(div().text_size(px(13.33)).text_color(pal.text_muted).child("CSV file"))
            .child(file_field)
            .child(axis_section(true, &row, &state.row))
            .child(axis_section(false, &col, &state.column))
            .child(os::checkbox("imp-conv", "Conversion factor", conversion_on).on_click(move |_, w, _| {
                conv_on2.set(!conv_on2.get());
                w.refresh();
            }))
            .when(conversion_on, |d| d.child(div().pl(px(18.)).child(underline(&conv))))
            .child(h_flex().gap(px(8.)).child(div().text_size(px(13.33)).text_color(pal.text_muted).child("Description")).child(underline(&desc)));
        let tabs = h_flex()
            .h(px(32.))
            .border_b_1()
            .border_color(rgb(0xcccccc))
            .text_size(px(14.))
            .children(["Assigned", "Measured", "From import"].into_iter().map(|t| {
                let on = t == "From import";
                div().flex_1().h_full().flex().items_center().justify_center().text_color(if on { pal.accent } else { pal.text }).when(on, |d| d.border_b_2().border_color(pal.accent)).child(t)
            }));
        let (doc2, imp2, row2, col2, name2, desc2, conv2, conv_on3) = (doc.clone(), imp.clone(), row.clone(), col.clone(), name.clone(), desc.clone(), conv.clone(), conv_on.clone());
        dialog
            .w(px(260.))
            .p_0()
            .close_button(false)
            .title(
                h_flex()
                    .w_full()
                    .h(px(36.))
                    .pl(px(8.))
                    .gap(px(4.))
                    .child(div().flex_1().min_w_0().truncate().text_size(px(14.)).font_weight(FontWeight::BOLD).text_color(pal.text).child(title))
                    .child(os::modal_ok(div().id("imp-ok").size(px(36.)).flex().items_center().justify_center().bg(rgb(0x45b649)).cursor_pointer().child(gpui_kit::component::Icon::new(gpui_kit::component::IconName::Check).size(px(20.)).text_color(rgb(0xffffff)))))
                    .child(os::modal_cancel(div().id("imp-cancel").size(px(36.)).flex().items_center().justify_center().cursor_pointer().child(os::glyph(os::GLYPH_CLOSE, 18., rgb(0xd0021b))))),
            )
            .child(tabs)
            .child(body)
            .on_ok(move |_, _, cx| {
                let mut s = imp2.borrow().clone();
                row2.apply(&mut s.row, cx);
                col2.apply(&mut s.column, cx);
                s.conversion = conv_on3.get().then(|| conv2.read(cx).value().trim().to_string()).filter(|c| !c.is_empty());
                let (vname, vdesc) = (name2.read(cx).value().trim().to_string(), desc2.read(cx).value().to_string());
                doc2.update(cx, |d, cx| {
                    d.edit_tab(tab, "Edit variable", cx, |c| {
                        if let TabContent::VariableStudio(vs) = c
                            && let Some(v) = vs.variables.get_mut(i)
                        {
                            if !vname.is_empty() {
                                v.name = vname;
                            }
                            v.description = vdesc;
                            v.kind = Some(VarKind::FromImport);
                            v.import = Some(s);
                        }
                    });
                });
                true
            })
    });
}

fn studio<'a>(doc: &Entity<DocModel>, tab: TabId, cx: &'a App) -> Option<&'a VariableStudio> {
    match &doc.read(cx).session.doc.tab(tab)?.content {
        TabContent::VariableStudio(v) => Some(v),
        _ => None,
    }
}
