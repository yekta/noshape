//! "+" › Import… inside a document (live document/76–77,
//! dom/dialog-import-in-document.txt): the OS picker (several files), then
//! the 400 px "Import to Onshape" modal: two option tiles (180 × 230,
//! selected `#eaeaea` with a 2 px `#b2ddf6` border) "Import to this
//! document" / "Combine to a single Part Studio" (+ "Use with small
//! assemblies"), then ☑ Import appearances, ☐ Orient imported models with Y
//! Axis Up, ☐ Create a composite part when importing multiple or non-solid
//! bodies, ☐ Join adjacent surfaces (each with a ? tooltip); Import / Cancel.
//! Native part files (SOLIDWORKS, Solid Edge, NX, Inventor; live G38b,
//! dom/dialog-import-sldprt.txt, dialog-import-nxhub.txt): no tiles, and ☐
//! Import material density after Import appearances.
//! Mesh files (live G35, G35b): STL has no tiles, no Import appearances and
//! a Units select defaulting to the document's unit; OBJ keeps the tiles and
//! Import appearances, its Units default to Meter; 3MF is OBJ without Units.
//! None of them has Join adjacent surfaces. A native assembly zip (live
//! G38h): the tiles and the five checkboxes, no Units.

use crate::doc_model::{DocEvent, DocModel};
use crate::os_ui as os;
use crate::theme::p;
use gpui_kit::component::{WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::document::TabId;
use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;

// Shared with the agent API.
use noshape_ops::import::{MESH_UNITS, NATIVE_PARTS};

#[derive(Clone, Copy, Default)]
struct Choice {
    combine: bool,
    appearances: bool,
    density: bool,
    y_up: bool,
    /// Mesh files: index into [`MESH_UNITS`].
    unit: usize,
    composite: bool,
    join: bool,
}

/// Show the modal for `paths`; Import adds the tabs (after `after`) and
/// calls `opened` with the first new tab.
/// `keep_files`: also keep each file as a file tab in "CAD Imports" (Import;
/// not when translating an existing file tab).
pub fn open(doc: Entity<DocModel>, paths: Vec<PathBuf>, after: Option<TabId>, keep_files: bool, opened: impl Fn(TabId, &mut Window, &mut App) + 'static, window: &mut Window, cx: &mut App) {
    // Native part files get Onshape's five-checkbox variant, mesh files
    // theirs (see the module docs).
    let all_of = |kinds: &[&str]| !paths.is_empty() && paths.iter().all(|p| p.extension().is_some_and(|e| kinds.contains(&e.to_string_lossy().to_lowercase().as_str())));
    let (native, stl, obj) = (all_of(NATIVE_PARTS), all_of(&["stl"]), all_of(&["obj"]));
    // Mesh files and PVZ (imported as meshes) have no Join adjacent surfaces
    // (live "qa5 zip import": PVZ shows appearances, Y up and composite only).
    let (mesh, units_select) = (all_of(&["stl", "obj", "3mf", "pvz"]), stl || obj);
    // Native assemblies (a zip; live G38h) keep the tiles and add Import
    // material density like native parts; STEP and Parasolid files have it
    // too, SAT and PVZ not (live "qa5 zip import", "api (10-23) import parity").
    let zip = all_of(&["zip"]);
    let density = native || zip || all_of(&["step", "stp", "x_t", "x_b", "xmt_txt", "xmt_bin"]);
    let doc_unit = doc.read(cx).session.doc.settings.units.length;
    let default_unit = if obj { noshape_units::LengthUnit::Meter } else { doc_unit };
    let unit = MESH_UNITS.iter().position(|(_, u)| *u == default_unit).unwrap_or(4);
    let choice = Rc::new(Cell::new(Choice { appearances: true, unit, ..Default::default() }));
    let opened = Rc::new(opened);
    window.open_dialog(cx, move |dialog, _, cx| {
        let pal = p();
        let c = choice.get();
        let tile = |id: &'static str, selected: bool, title: &'static str, note: Option<&'static str>, both: bool, combine: bool, choice: Rc<Cell<Choice>>| {
            // Live (dom/dialog-import-*.txt): each tile shows one 150 px
            // picture (a file, an arrow, the Part Studio and Assembly tab
            // icons in a box), centred under the 2 px border; our own drawing.
            let picture = h_flex()
                .h(px(150.))
                .flex_none()
                .justify_center()
                .child(crate::assets::icon(if both { "import-tile-single" } else { "import-tile-combine" }, 150., cx));
            v_flex()
                .id(id)
                .w(px(180.))
                .h(px(230.))
                .flex_none()
                .rounded(px(5.))
                .border_2()
                .border_color(rgb(if selected { 0xb2ddf6 } else { 0xcccccc }))
                .when(selected, |d| d.bg(rgb(0xeaeaea)))
                .cursor_pointer()
                .child(picture)
                .child(div().px(px(4.)).text_center().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child(title))
                .children(note.map(|n| div().text_center().text_size(px(10.)).text_color(pal.text).child(n)))
                .on_click(move |_, window, _| {
                    choice.set(Choice { combine, ..choice.get() });
                    window.refresh();
                })
        };
        // Bootstrap form-check (live DOM): 14 px box (radius 3.5, 1 px #aaa;
        // checked #0d6efd), 14 px / 600 #333 label that wraps, then the ?.
        let check = |id: &'static str, label: &'static str, tip: &'static str, on: bool, set: fn(&mut Choice, bool), choice: Rc<Cell<Choice>>| {
            h_flex()
                .id(id)
                .w_full()
                .items_start()
                .gap(px(7.))
                .cursor_pointer()
                .child(
                    div()
                        .mt(px(4.))
                        .size(px(14.))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(3.5))
                        .border_1()
                        .border_color(rgb(if on { 0x0d6efd } else { 0xaaaaaa }))
                        .bg(rgb(if on { 0x0d6efd } else { 0xffffff }))
                        .when(on, |d| d.child(gpui_kit::component::Icon::new(gpui_kit::component::IconName::Check).size(px(11.)).text_color(gpui_kit::rgb_static(0xffffff)))),
                )
                // GPUI has no inline boxes: the ? sits after the label's last line.
                .child(
                    h_flex()
                        .flex_1()
                        .min_w_0()
                        .items_end()
                        .gap(px(8.))
                        .child(div().flex_shrink(1.).min_w_0().text_size(px(14.)).line_height(px(21.)).font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child(label))
                        .child(div().id(SharedString::from(format!("{id}-tip"))).mb(px(1.)).size(px(18.)).flex_none().rounded_full().border_1().border_color(rgb(0xcccccc)).flex().items_center().justify_center().text_size(px(11.)).text_color(rgb(0xaaaaaa)).child("?").tooltip(crate::ui::tooltip(tip))),
                )
                .on_click(move |_, window, _| {
                    let mut c = choice.get();
                    set(&mut c, !on);
                    choice.set(c);
                    window.refresh();
                })
        };
        let (doc, paths, ch, opened) = (doc.clone(), paths.clone(), choice.clone(), opened.clone());
        os::modal(dialog, "Import to Onshape", 400.)
            .child(
                // Live document/77: the tiles 12 px under the header.
                // Live: the footer rule 27 px under the last checkbox row's text.
                os::modal_body()
                    .pt(px(4.))
                    .pb(px(16.))
                    .gap(px(8.))
                    .when(!native && !stl, |d| d.child(h_flex().gap(px(14.)).child(tile("imp-single", !c.combine, "Import to this document", None, true, false, choice.clone())).child(tile("imp-flatten", c.combine, "Combine to a single Part Studio", Some("Use with small assemblies"), false, true, choice.clone()))))
                    // Live document/77: the checkbox rows are 34 px apart.
                    .child(v_flex().gap(px(13.)).when(!stl, |d| d.child(check("imp-app", "Import appearances", "Appearances defined on models will be imported.", c.appearances, |c, v| c.appearances = v, choice.clone())))
                    .when(density, |d| d.child(check("imp-dens", "Import material density", "Material density defined on models will be imported.", c.density, |c, v| c.density = v, choice.clone())))
                    .child(check("imp-yup", "Orient imported models with Y Axis Up", "Models built in an environment with Y-axis up, such as SOLIDWORKS, Creo, or Inventor, will keep their original orientation upon import.", c.y_up, |c, v| c.y_up = v, choice.clone()))
                    // Live G41: Combine hides it for an assembly zip.
                    .when(!(zip && c.combine), |d| d.child(check("imp-comp", "Create a composite part when importing multiple or non-solid bodies", "Parts within a single Part Studio will become a composite part upon import", c.composite, |c, v| c.composite = v, choice.clone())))
                    .when(!mesh, |d| d.child(check("imp-join", "Join adjacent surfaces", "Adjacent surfaces with overlapping edges will be joined upon import.", c.join, |c, v| c.join = v, choice.clone()))))
                    // Live dom/dialog-import-stl.txt: "Units" (14 px / 600) over a 212 px select.
                    .when(units_select, |d| {
                        let ch = choice.clone();
                        d.child(
                            v_flex().gap(px(6.)).child(div().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child("Units")).child(div().w(px(212.)).child(crate::os_ui::select_field(
                                "imp-units",
                                MESH_UNITS.iter().map(|(n, _)| n.to_string()).collect(),
                                c.unit,
                                move |i, _, _| ch.set(Choice { unit: i, ..ch.get() }),
                            ))),
                        )
                    }),
            )
            // Live document/77: 13 px over the buttons, 10 under them.
            .footer(os::modal_footer().pb(px(10.)).child(os::modal_ok(os::primary_button("ok", "Import"))).child(os::modal_cancel(os::cancel_button("cancel", "Cancel", cx))))
            .on_ok(move |_, window, cx| {
                let (doc, paths, c, opened) = (doc.clone(), paths.clone(), ch.get(), opened.clone());
                window.defer(cx, move |window, cx| {
                    if let Some(first) = import(&doc, &paths, c, after, keep_files, cx) {
                        opened(first, window, cx);
                    }
                });
                true
            })
    });
}

/// Translate the files and add their tabs (shared with the agent API:
/// `noshape_ops::import`).
fn import(doc: &Entity<DocModel>, paths: &[PathBuf], c: Choice, after: Option<TabId>, keep_files: bool, cx: &mut App) -> Option<TabId> {
    use noshape_ops::import as imp;
    let units = doc.read(cx).session.doc.settings.units;
    let choice = imp::ImportChoice { combine: c.combine, appearances: c.appearances, density: c.density, y_up: c.y_up, unit: c.unit, composite: c.composite, join: c.join };
    let (inputs, errors) = imp::read_inputs(paths, &choice, keep_files, &units);
    for e in errors {
        doc.update(cx, |_, cx| cx.emit(DocEvent::Error(e)));
    }
    // A file Onshape couldn't translate still gets its file tab.
    if inputs.is_empty() {
        return None;
    }
    let description = imp::description(&inputs.label);
    let mut first = None;
    let mut errors = Vec::new();
    doc.update(cx, |doc, cx| {
        doc.edit_document(&description, cx, |d| (first, errors) = imp::apply(d, inputs, &choice, after, &units));
        for e in errors {
            cx.emit(DocEvent::Error(e));
        }
    });
    first
}

