//! Material Library tab (help image custom-mat-library-1): the blue
//! "To edit this material library:" box with Download sample, then the
//! table titled with the tab name: Filter by name; # | Category | Name ▴▾ |
//! Density with a unit select. Tab menu Update… / Export… (workspace)
//! read and write the CSV ([`update`], [`export`]).

use crate::doc_model::{DocEvent, DocModel};
use crate::os_ui as os;
use crate::theme::p;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::TabContent;
use noshape_model::document::TabId;
use noshape_model::material_library::{DENSITY_UNITS, LibraryMaterial, MaterialLibrary};
use std::cell::Cell;
use std::rc::Rc;

pub struct MaterialLibraryView {
    tab: TabId,
    doc: Entity<DocModel>,
    filter: Entity<InputState>,
    /// Index into `DENSITY_UNITS` of the Density column.
    unit: Rc<Cell<usize>>,
    /// Name column sort: None, ascending, descending.
    sort: Option<bool>,
    focus: FocusHandle,
    _subs: Vec<Subscription>,
}

impl Focusable for MaterialLibraryView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl MaterialLibraryView {
    pub fn new(doc: Entity<DocModel>, tab: TabId, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let filter = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).placeholder("Filter by name"));
        let subs = vec![
            cx.subscribe_in(&doc, window, |_: &mut Self, _, _: &DocEvent, _, cx| cx.notify()),
            cx.subscribe_in(&filter, window, |_: &mut Self, _, ev: &InputEvent, _, cx| {
                if matches!(ev, InputEvent::Change) {
                    cx.notify();
                }
            }),
        ];
        Self { tab, doc, filter, unit: Rc::new(Cell::new(0)), sort: None, focus: cx.focus_handle(), _subs: subs }
    }

    fn library<'a>(&self, cx: &'a App) -> Option<&'a MaterialLibrary> {
        match &self.doc.read(cx).session.doc.tab(self.tab)?.content {
            TabContent::MaterialLibrary(l) => Some(l),
            _ => None,
        }
    }
}

/// Tab › Export…: the library as a CSV file.
pub fn export(doc: Entity<DocModel>, tab: TabId, window: &mut Window, cx: &mut App) {
    let d = &doc.read(cx).session.doc;
    let Some(t) = d.tab(tab) else { return };
    let TabContent::MaterialLibrary(lib) = &t.content else { return };
    let (csv, name) = (lib.to_csv(), t.name.clone());
    crate::files::request_save_path("Export", format!("{name}.csv"), "csv", window, cx, move |path, cx| {
        if let Err(e) = std::fs::write(&path, csv) {
            doc.update(cx, |_, cx| cx.emit(DocEvent::Error(format!("Export failed: {e}"))));
        }
    });
}

/// Tab › Update…: choose a CSV file, show its contents (OK / Cancel), then
/// replace the library with it.
pub fn update(doc: Entity<DocModel>, tab: TabId, window: &mut Window, cx: &mut App) {
    let handle = window.window_handle();
    crate::files::request_open_path("Update", "CSV files", &["csv"], window, cx, move |path, cx| {
        let parsed = std::fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|t| MaterialLibrary::from_csv(&t));
        // The in-app prompt calls back inside its window's update: open the
        // contents dialog once that (and the prompt) is done.
        cx.defer(move |cx| match parsed {
            Err(e) => doc.update(cx, |_, cx| cx.emit(DocEvent::Error(format!("Could not read the material library: {e}")))),
            Ok(lib) => {
                let _ = handle.update(cx, |_, window, cx| confirm(doc.clone(), tab, lib, window, cx));
            }
        });
    });
}

/// The file contents (help image custom-mat-library-2): every column, OK / Cancel.
fn confirm(doc: Entity<DocModel>, tab: TabId, lib: MaterialLibrary, window: &mut Window, cx: &mut App) {
    window.open_dialog(cx, move |dialog, _, cx| {
        let pal = p();
        let header: Vec<String> = ["Category", "Name", "Density [kg/m^3]"].iter().map(|s| s.to_string()).chain(lib.extra_columns.iter().cloned()).collect();
        let cell = |t: String, bold: bool| div().w(px(110.)).flex_none().px(px(5.)).py(px(4.)).text_size(px(14.)).text_color(pal.text).when(bold, |d| d.font_weight(FontWeight::BOLD)).child(t);
        let head = h_flex().items_end().children(header.into_iter().map(|h| cell(h, true)));
        let rows = lib.materials.iter().map(|m| {
            h_flex().items_start().child(cell(m.category.clone(), false)).child(cell(m.name.clone(), false)).child(cell(format!("{}", m.density), false)).children(m.extra.iter().map(|e| cell(e.clone(), false)))
        });
        let (doc, lib2) = (doc.clone(), lib.clone());
        os::modal(dialog, "Update", 680.)
            .child(os::modal_body().child(div().id("ml-preview").max_h(px(420.)).overflow_scroll().child(v_flex().child(head).children(rows))))
            .footer(os::modal_footer().child(os::modal_ok(os::primary_button("ok", "OK"))).child(os::modal_cancel(os::cancel_button("cancel", "Cancel", cx))))
            .on_ok(move |_, _, cx| {
                let lib = lib2.clone();
                doc.update(cx, |d, cx| {
                    d.edit_tab(tab, "Update material library", cx, move |c| {
                        if let TabContent::MaterialLibrary(l) = c {
                            *l = lib;
                        }
                    });
                });
                true
            })
    });
}

impl Render for MaterialLibraryView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let name = self.doc.read(cx).session.doc.tab(self.tab).map(|t| t.name.clone()).unwrap_or_default();
        let q = self.filter.read(cx).value().trim().to_lowercase();
        let (_, per_unit) = DENSITY_UNITS[self.unit.get().min(DENSITY_UNITS.len() - 1)];
        let mut mats: Vec<LibraryMaterial> = self.library(cx).map(|l| l.materials.clone()).unwrap_or_default();
        mats.retain(|m| q.is_empty() || m.name.to_lowercase().contains(&q) || m.category.to_lowercase().contains(&q));
        if let Some(asc) = self.sort {
            mats.sort_by_key(|m| m.name.to_lowercase());
            if !asc {
                mats.reverse();
            }
        }
        let doc = self.doc.clone();
        let bullet = |t: AnyElement| h_flex().items_start().gap(px(8.)).child("•").child(t);
        let link = |id: &'static str, t: &'static str| div().id(id).text_color(rgb(0x094174)).underline().cursor_pointer().child(t);
        let info = v_flex()
            .px(px(14.))
            .pt(px(12.))
            .pb(px(14.))
            .rounded(px(3.))
            .bg(pal.info_fill)
            .border_1()
            .border_color(pal.info_outline)
            .text_size(px(14.))
            .text_color(pal.info_text)
            .gap(px(2.))
            .child("To edit this material library:")
            .child(v_flex().pl(px(12.)).child(bullet(div().child("Right click on the tab and click Export to download a CSV file, or download a sample.").into_any_element())).child(bullet(div().child("Edit the CSV file to add new or remove materials, then save your changes.").into_any_element())).child(bullet(div().child("Right click on this tab and click Update, then choose your saved CSV file.").into_any_element())).child(bullet(
                h_flex()
                    .child(link("ml-version", "Create a version").on_click(move |_, window, cx| crate::history_panel::open_create_version_in(doc.clone(), window, cx)))
                    .child("\u{a0}of this document to make this material library available for use.")
                    .into_any_element(),
            )))
            .child(
                h_flex()
                    .pt(px(24.))
                    .child(div().flex_1().font_weight(FontWeight::SEMIBOLD).child(link("ml-learn", "Learn more about custom materials").on_click(|_, _, cx| cx.open_url("https://cad.onshape.com/help/Content/PartStudio/customizing_part_materials.htm"))))
                    .child(os::primary_button("ml-sample", "Download sample").on_click(|_, window, cx| {
                        let csv = MaterialLibrary::sample().to_csv();
                        crate::files::request_save_path("Download sample", "Material library sample.csv".into(), "csv", window, cx, move |path, _| {
                            let _ = std::fs::write(path, csv);
                        });
                    })),
            );
        let unit_cell = div().w(px(90.)).child(os::select_field("ml-unit", DENSITY_UNITS.iter().map(|u| u.0.to_string()).collect(), self.unit.get(), {
            let u = self.unit.clone();
            move |i, window, _| {
                u.set(i);
                window.refresh();
            }
        }));
        let th = |w: Option<f32>| {
            let d = h_flex().h(px(38.)).px(px(10.)).border_r_1().border_color(rgb(0xaaaaaa)).text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).text_color(pal.text);
            match w {
                Some(w) => d.w(px(w)).flex_none(),
                None => d.flex_1(),
            }
        };
        let sort = self.sort;
        let header = h_flex()
            .border_t_1()
            .border_color(rgb(0xaaaaaa))
            .bg(rgb(0xffffff))
            .child(th(Some(64.)).child("#"))
            .child(th(Some(130.)).child("Category"))
            .child(
                th(None)
                    .id("ml-sort")
                    .cursor_pointer()
                    .child(div().flex_1().child("Name"))
                    .child(os::glyph(if sort == Some(false) { os::GLYPH_CARET } else { os::GLYPH_CARET_UP }, 8., if sort.is_some() { pal.text } else { pal.text_faint }))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.sort = match this.sort {
                            None => Some(true),
                            Some(true) => Some(false),
                            Some(false) => None,
                        };
                        cx.notify();
                    })),
            )
            .child(th(Some(170.)).border_r_0().gap(px(8.)).child(div().flex_1().child("Density")).child(unit_cell));
        let rows = mats.iter().enumerate().map(|(i, m)| {
            let td = |w: Option<f32>, t: String| {
                let d = div().h(px(34.)).px(px(10.)).flex().items_center().text_size(px(14.)).text_color(pal.text).truncate().child(t);
                match w {
                    Some(w) => d.w(px(w)).flex_none(),
                    None => d.flex_1().min_w_0(),
                }
            };
            h_flex()
                .border_t_1()
                .border_color(rgb(0xdddddd))
                .child(td(Some(64.), (i + 1).to_string()))
                .child(td(Some(130.), m.category.clone()))
                .child(td(None, m.name.clone()))
                .child(td(Some(170.), format!("{}", (m.density / per_unit * 1e4).round() / 1e4)))
        });
        let table = v_flex()
            .border_1()
            .border_color(rgb(0xaaaaaa))
            .child(
                h_flex()
                    .h(px(36.))
                    .px(px(6.))
                    .bg(rgb(0xeaeaea))
                    .child(div().flex_1().text_size(px(16.)).text_color(pal.text).child(crate::ui::user_text(name)))
                    .child(div().w(px(170.)).child(Input::new(&self.filter).h(px(26.)).text_size(px(14.)))),
            )
            .child(header)
            .children(rows);
        div()
            .id("material-library")
            .size_full()
            .track_focus(&self.focus)
            .bg(rgb(0xffffff))
            .overflow_y_scroll()
            .child(v_flex().w(px(660.)).mx_auto().pt(px(48.)).pb(px(40.)).gap(px(14.)).child(info).child(table))
    }
}
