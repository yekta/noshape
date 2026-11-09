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
//! Import material density after Import appearances (Onshape's list of
//! types, `noshape_ops::import::MATERIAL_DENSITY_TYPES`: not Inventor).
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
    /// The documents page's "Split into multiple documents".
    split: bool,
}

impl Choice {
    fn ops(self) -> noshape_ops::import::ImportChoice {
        noshape_ops::import::ImportChoice { combine: self.combine, appearances: self.appearances, density: self.density, y_up: self.y_up, unit: self.unit, composite: self.composite, join: self.join }
    }
}

/// Show the modal for `paths`; Import adds the tabs (after `after`) and
/// calls `opened` with the first new tab.
/// `keep_files`: also keep each file as a file tab in "CAD Imports" (Import;
/// not when translating an existing file tab).
pub fn open(doc: Entity<DocModel>, paths: Vec<PathBuf>, after: Option<TabId>, keep_files: bool, opened: impl Fn(TabId, &mut Window, &mut App) + 'static, window: &mut Window, cx: &mut App) {
    let doc_unit = doc.read(cx).session.doc.settings.units.length;
    let ok_paths = paths.clone();
    let on_ok = move |c: Choice, window: &mut Window, cx: &mut App| {
        if let Some(first) = import(&doc, &ok_paths, c, after, keep_files, cx) {
            opened(first, window, cx);
        }
    };
    // A CAD file tab's own Import… has no Join adjacent surfaces (live2 2403).
    show(paths, doc_unit, false, keep_files, Rc::new(on_ok), window, cx);
}

/// The documents page's Import files… (live dom/dialog-import-to-onshape.txt):
/// the 590 px modal with three tiles, "Import to a single document", "Split
/// into multiple documents" (each file its own document) and "Combine to a
/// single Part Studio"; Import calls `on_import` with the options and
/// whether to split. The start window builds the documents (start hooks).
pub fn open_for_page(paths: Vec<PathBuf>, on_import: noshape_start_ui::start_hooks::ImportCallback, window: &mut Window, cx: &mut App) {
    let unit = noshape_app_core::user_units::current().units.length;
    show(paths, unit, true, true, Rc::new(move |c: Choice, window: &mut Window, cx: &mut App| on_import(c.ops(), c.split, window, cx)), window, cx);
}

/// What Import does with the options.
type OnOk = Rc<dyn Fn(Choice, &mut Window, &mut App)>;

/// `join`: offer Join adjacent surfaces (not for a file tab's own Import…).
fn show(paths: Vec<PathBuf>, doc_unit: noshape_units::LengthUnit, page: bool, join: bool, on_ok: OnOk, window: &mut Window, cx: &mut App) {
    // Native part files get Onshape's five-checkbox variant, mesh files
    // theirs (see the module docs).
    let all_of = |kinds: &[&str]| !paths.is_empty() && paths.iter().all(|p| p.extension().is_some_and(|e| kinds.contains(&e.to_string_lossy().to_lowercase().as_str())));
    let (native, stl, obj) = (all_of(NATIVE_PARTS), all_of(&["stl"]), all_of(&["obj"]));
    // Mesh files and PVZ (imported as meshes) have no Join adjacent surfaces
    // (live "qa5 zip import": PVZ shows appearances, Y up and composite only).
    let (mesh, units_select) = (all_of(&["stl", "obj", "3mf", "pvz"]), stl || obj);
    // Import material density: Onshape's own file-type list, when any file
    // is one (not STEP, SAT or PVZ; qa4 IMP-DENS).
    let density = noshape_ops::import::offers_material_density(&paths);
    let default_unit = if obj { noshape_units::LengthUnit::Meter } else { doc_unit };
    let unit = MESH_UNITS.iter().position(|(_, u)| *u == default_unit).unwrap_or(4);
    let choice = Rc::new(Cell::new(Choice { appearances: true, unit, ..Default::default() }));
    window.open_dialog(cx, move |dialog, window, cx| {
        let pal = p();
        let c = choice.get();
        let (text_system, label_font) = (cx.text_system().clone(), Font { weight: FontWeight::SEMIBOLD, ..window.text_style().font() });
        let tile = |id: &'static str, selected: bool, title: &'static str, note: Option<&'static str>, icon: &'static str, (combine, split): (bool, bool), choice: Rc<Cell<Choice>>| {
            // Live (dom/dialog-import-*.txt): each tile shows one 150 px
            // picture (a file, an arrow, the Part Studio and Assembly tab
            // icons in a box), centred under the 2 px border; our own drawing.
            let picture = h_flex()
                .h(px(150.))
                .flex_none()
                .justify_center()
                .child(crate::assets::icon(icon, 150., cx));
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
                    choice.set(Choice { combine, split, ..choice.get() });
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
                // GPUI has no inline boxes: wrap the label with the ? as a
                // trailing inline element in live's 358 px label box (dom/
                // dialog-import-in-document.txt), so the ? follows the last
                // word ("…non-solid bodies ?") or starts a line of its own.
                .child({
                    let mut wrapper = text_system.line_wrapper(label_font.clone(), px(14.));
                    let fragments = [LineFragment::text(label), LineFragment::element(px(TIP_GAP + TIP_SIZE), 1)];
                    let breaks: Vec<usize> = wrapper.wrap_line(&fragments, px(LABEL_WIDTH), IndentAdjustment::NoIndent).map(|b| b.ix).collect();
                    let mut lines = wrapped_lines(label, &breaks);
                    let last = lines.pop().unwrap_or_default();
                    let line = |t: String| div().text_size(px(14.)).line_height(px(21.)).font_weight(FontWeight::SEMIBOLD).text_color(pal.text).whitespace_nowrap().child(t);
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .children(lines.into_iter().map(line))
                        .child(
                            h_flex()
                                .items_end()
                                .gap(px(TIP_GAP))
                                .when(!last.is_empty(), |d| d.child(line(last)))
                                .child(div().id(SharedString::from(format!("{id}-tip"))).mb(px(1.)).size(px(TIP_SIZE)).flex_none().rounded_full().border_1().border_color(rgb(0xcccccc)).flex().items_center().justify_center().text_size(px(11.)).text_color(rgb(0xaaaaaa)).child("?").tooltip(crate::ui::tooltip(tip))),
                        )
                })
                .on_click(move |_, window, _| {
                    let mut c = choice.get();
                    set(&mut c, !on);
                    choice.set(c);
                    window.refresh();
                })
        };
        let (ch, on_ok) = (choice.clone(), on_ok.clone());
        // Live: the documents page's modal is Bootstrap's modal-lg (590 px).
        os::modal(dialog, "Import to Onshape", if page { 590. } else { 400. })
            .child(
                // Live document/77: the tiles 12 px under the header.
                // Live: the footer rule 27 px under the last checkbox row's text.
                os::modal_body()
                    .pt(px(4.))
                    .pb(px(16.))
                    .gap(px(8.))
                    .when(!native && !stl, |d| {
                        d.child(
                            h_flex()
                                .gap(px(14.))
                                .child(tile("imp-single", !c.combine && !c.split, if page { "Import to a single document" } else { "Import to this document" }, None, "import-tile-single", (false, false), choice.clone()))
                                .when(page, |d| d.child(tile("imp-multi", c.split, "Split into multiple documents", Some("Groups subassemblies with their parts in the same document"), "import-tile-multi", (false, true), choice.clone())))
                                .child(tile("imp-flatten", c.combine, "Combine to a single Part Studio", Some("Use with small assemblies"), "import-tile-combine", (true, false), choice.clone())),
                        )
                    })
                    // Live document/77: the checkbox rows are 34 px apart.
                    .child(v_flex().gap(px(13.)).when(!stl, |d| d.child(check("imp-app", "Import appearances", "Appearances defined on models will be imported.", c.appearances, |c, v| c.appearances = v, choice.clone())))
                    .when(density, |d| d.child(check("imp-dens", "Import material density", "Material density defined on models will be imported.", c.density, |c, v| c.density = v, choice.clone())))
                    .child(check("imp-yup", "Orient imported models with Y Axis Up", "Models built in an environment with Y-axis up, such as SOLIDWORKS, Creo, or Inventor, will keep their original orientation upon import.", c.y_up, |c, v| c.y_up = v, choice.clone()))
                    // Live G41, live2 2403: Combine to a single Part Studio has no composite part.
                    .when(!c.combine, |d| d.child(check("imp-comp", "Create a composite part when importing multiple or non-solid bodies", "Parts within a single Part Studio will become a composite part upon import", c.composite, |c, v| c.composite = v, choice.clone())))
                    .when(!mesh && join, |d| d.child(check("imp-join", "Join adjacent surfaces", "Adjacent surfaces with overlapping edges will be joined upon import.", c.join, |c, v| c.join = v, choice.clone()))))
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
                let (c, on_ok) = (ch.get(), on_ok.clone());
                window.defer(cx, move |window, cx| on_ok(c, window, cx));
                true
            })
    });
}

/// Live's checkbox label box (dom/dialog-import-in-document.txt: 358 px) and
/// the ? after the label text.
const LABEL_WIDTH: f32 = 358.;
const TIP_GAP: f32 = 8.;
const TIP_SIZE: f32 = 18.;

/// The lines of `label` broken at `breaks` (the byte index each new line
/// starts at; the ? element sits at `label.len()`). The last entry is the
/// text before the ?, empty when the ? wraps onto a line of its own.
fn wrapped_lines(label: &str, breaks: &[usize]) -> Vec<String> {
    let mut lines = Vec::new();
    let mut start = 0;
    for &b in breaks.iter().filter(|&&b| b > 0 && b <= label.len()) {
        lines.push(label[start..b].trim_end().to_string());
        start = b;
    }
    lines.push(label[start..].trim_end().to_string());
    lines
}

/// Translate the files and add their tabs (shared with the agent API:
/// `noshape_ops::import`).
fn import(doc: &Entity<DocModel>, paths: &[PathBuf], c: Choice, after: Option<TabId>, keep_files: bool, cx: &mut App) -> Option<TabId> {
    use noshape_ops::import as imp;
    let units = doc.read(cx).session.doc.settings.units;
    let choice = c.ops();
    let (inputs, errors) = imp::read_inputs(paths, &choice, keep_files, &units);
    for e in errors {
        // The toast, and the reason in the Notifications bell (live IM3).
        crate::notifications::push(cx, format!("Import {} failed", import_name(&e).unwrap_or_default()), e.clone(), true);
        doc.update(cx, |_, cx| cx.emit(DocEvent::Error(failed_toast(&e))));
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
        let ok = errors.is_empty();
        for e in errors {
            crate::notifications::push(cx, format!("Import {} failed", import_name(&e).unwrap_or_default()), e.clone(), true);
            cx.emit(DocEvent::Error(failed_toast(&e)));
        }
        // Live's bell entry: "Import <file>" with its steps done.
        if ok && first.is_some() {
            for p in paths {
                let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                crate::notifications::push(cx, format!("Import {name}"), format!("Uploaded · Translated · Post-processing done · Created {name}"), false);
            }
        }
        // Live: "onshape-cone.sat has been imported. Open document ×".
        if let ([p], true, Some(_)) = (paths, ok, first) {
            let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            cx.emit(DocEvent::Info(format!("{name} has been imported.")));
        }
    });
    first
}

/// Live's toast for a file that didn't translate ("g47-3parts-unique.zip
/// uploaded but failed to translate."; the reason goes to its Notifications
/// bell, behaviours "qa5 zip import"); the agent API keeps the reason.
fn failed_toast(e: &str) -> String {
    match import_name(e) {
        Some(name) => format!("{name} uploaded but failed to translate."),
        None => e.to_string(),
    }
}

/// The file an import error is about.
fn import_name(e: &str) -> Option<String> {
    if let Some((n, _)) = e.split_once(" was not translated.") {
        Some(n.to_string())
    } else if let Some(rest) = e.strip_prefix("Import failed: ") {
        let path = rest.split_once(": ").map_or(rest, |(p, _)| p);
        Some(std::path::Path::new(path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.to_string()))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::wrapped_lines;

    #[::core::prelude::v1::test]
    fn failed_imports_toast_as_live() {
        assert_eq!(super::failed_toast("g47-3parts-unique.zip was not translated. Translation is not supported for STEP (.stp) files."), "g47-3parts-unique.zip uploaded but failed to translate.");
        assert_eq!(super::failed_toast("Import failed: /tmp/x/tri.jt: no geometry"), "tri.jt uploaded but failed to translate.");
        assert_eq!(super::failed_toast("Something else"), "Something else");
    }

    #[::core::prelude::v1::test]
    fn the_question_mark_follows_the_last_word() {
        let label = "Create a composite part when importing multiple or non-solid bodies";
        // Live: "…multiple or" / "non-solid bodies ?".
        assert_eq!(wrapped_lines(label, &[51]), ["Create a composite part when importing multiple or", "non-solid bodies"]);
        // One line with room for the ?.
        assert_eq!(wrapped_lines("Join adjacent surfaces", &[]), ["Join adjacent surfaces"]);
        // The ? alone on the next line.
        assert_eq!(wrapped_lines("Join adjacent surfaces", &[22]), ["Join adjacent surfaces", ""]);
    }
}
