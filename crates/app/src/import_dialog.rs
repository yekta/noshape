//! "+" › Import… inside a document (live document/76–77,
//! dom/dialog-import-in-document.txt): the OS picker (several files), then
//! the 400 px "Import to Noshape" modal: two option tiles (180 × 230,
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
//! a Units select defaulting to Millimeter; OBJ keeps the tiles and Import
//! appearances, its Units default to Meter; 3MF is OBJ without Units. None of
//! them has Join adjacent surfaces. IGES adds ☐ IGES postprocessing after
//! Join adjacent surfaces. A native assembly zip (live G38h): the tiles and
//! the five checkboxes, no Units. Every import starts from these defaults:
//! nothing is remembered (live2 feel/importexport.json). A bare assembly file
//! (live upload-only-1118: .iam, .catproduct, .sldasm, .asm) gets live's info
//! box ("It looks like you want to import an assembly…" + Learn more), ☐ Upload
//! only and ☐ Import material density; Import stays disabled until Upload only
//! is ticked, then keeps the files as they are, as file tabs after the active tab.

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
pub(crate) struct Choice {
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
    /// Split's "Create one document per part or assembly".
    per_part: bool,
    /// IGES files: "IGES postprocessing".
    iges_post: bool,
    /// A bare assembly's "Upload only".
    upload_only: bool,
}

impl Choice {
    fn ops(self) -> noshape_ops::import::ImportChoice {
        // The composite part's box shows only on Import to a single document / this document.
        let composite = self.composite && !self.combine && !self.split;
        noshape_ops::import::ImportChoice { combine: self.combine, appearances: self.appearances, density: self.density, y_up: self.y_up, unit: self.unit, composite, join: self.join, iges_post: self.iges_post, upload_only: self.upload_only }
    }
}

/// Show the modal for `paths`; Import adds the tabs (after `after`; the view stays, as live's).
/// `keep_files`: also keep each file as a file tab in "CAD Imports" (Import;
/// not when translating an existing file tab).
pub fn open(doc: Entity<DocModel>, paths: Vec<PathBuf>, after: Option<TabId>, keep_files: bool, window: &mut Window, cx: &mut App) {
    let ok_paths = paths.clone();
    let on_ok = move |c: Choice, _: &mut Window, cx: &mut App| {
        import(&doc, &ok_paths, c, after, keep_files, cx);
    };
    // A CAD file tab's own Import… has no Join adjacent surfaces (live2 2403).
    show(paths, false, keep_files, Rc::new(on_ok), window, cx);
}

/// The documents page's Import files… (live dom/dialog-import-to-onshape.txt):
/// the 590 px modal with three tiles, "Import to a single document", "Split
/// into multiple documents" (each file its own document) and "Combine to a
/// single Part Studio"; Import calls `on_import` with the options and
/// whether to split. The start window builds the documents (start hooks).
pub fn open_for_page(paths: Vec<PathBuf>, on_import: noshape_start_ui::start_hooks::ImportCallback, window: &mut Window, cx: &mut App) {
    show(
        paths,
        true,
        true,
        Rc::new(move |c: Choice, window: &mut Window, cx: &mut App| {
            use noshape_start_ui::start_hooks::PageImport;
            let how = match (c.split, c.per_part) {
                (false, _) => PageImport::Single,
                (true, false) => PageImport::Split,
                (true, true) => PageImport::PerPart,
            };
            on_import(c.ops(), how, window, cx)
        }),
        window,
        cx,
    );
}

/// What Import does with the options.
type OnOk = Rc<dyn Fn(Choice, &mut Window, &mut App)>;

/// `join`: offer Join adjacent surfaces (not for a file tab's own Import…).
fn show(paths: Vec<PathBuf>, page: bool, join: bool, on_ok: OnOk, window: &mut Window, cx: &mut App) {
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
    // IGES files: IGES postprocessing (live2 feel/importexport).
    let iges = noshape_ops::import::offers_iges_postprocessing(&paths);
    // A bare assembly file: only Upload only and Import material density (live upload-only-1118).
    let bare = !page && noshape_ops::import::has_bare_assembly(&paths);
    // The same starting choices every time (nothing is remembered: live2 feel/importexport); STL in Millimeter.
    let unit = noshape_ops::import::ImportChoice::defaults(&paths).unit;
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
            // The suite's targets: a card by its title, a checkbox by its label (ui-parity README).
            v_flex()
                .id(id)
                .map(|d| noshape_app_core::ui_script::mark(d, move || format!("button:{title}")))
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
        // `muted`: grey text right after the label (live: "…assembly(not recommended)", no space).
        let label_width = if page { PAGE_LABEL_WIDTH } else { LABEL_WIDTH };
        let check = |id: &'static str, label: &'static str, muted: Option<&'static str>, tip: &'static str, on: bool, set: fn(&mut Choice, bool), choice: Rc<Cell<Choice>>| {
            // (Live's reader reads the muted part with the label: "…assembly(not recommended)".)
            noshape_app_core::ui_script::note_modal_field(|| serde_json::json!({ "label": format!("{label}{}", muted.unwrap_or("")), "kind": "checkbox", "value": on }));
            h_flex()
                .id(id)
                .debug_selector(move || id.to_string())
                .map(|d| noshape_app_core::ui_script::mark(d, move || format!("checkbox:{label}")))
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
                // trailing inline element in live's label box (dom/
                // dialog-import-in-document.txt), so the ? follows the last
                // word ("…non-solid bodies ?") or starts a line of its own.
                .child({
                    let mut wrapper = text_system.line_wrapper(label_font.clone(), px(14.));
                    let text = format!("{label}{}", muted.unwrap_or(""));
                    // (No ? without a tooltip: live's Upload only.)
                    let tip_room = if tip.is_empty() { 0. } else { TIP_GAP + TIP_SIZE };
                    let fragments = [LineFragment::text(&text), LineFragment::element(px(tip_room), 1)];
                    let breaks: Vec<usize> = wrapper.wrap_line(&fragments, px(label_width), IndentAdjustment::NoIndent).map(|b| b.ix).collect();
                    let mut lines = wrapped_lines(&text, label.len(), &breaks);
                    let last = lines.pop().unwrap_or_default();
                    // The muted part regular weight and grey (live).
                    let line = |(bold, grey): (String, String)| {
                        h_flex()
                            .text_size(px(14.))
                            .line_height(px(21.))
                            .whitespace_nowrap()
                            .when(!bold.is_empty(), |d| d.child(div().font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child(bold)))
                            .when(!grey.is_empty(), |d| d.child(div().text_color(pal.text_muted).child(grey)))
                    };
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .children(lines.into_iter().map(line))
                        .child(
                            h_flex()
                                .items_end()
                                .gap(px(TIP_GAP))
                                .when(!last.0.is_empty() || !last.1.is_empty(), |d| d.child(line(last)))
                                .when(!tip.is_empty(), |d| d.child(div().id(SharedString::from(format!("{id}-tip"))).mb(px(1.)).size(px(TIP_SIZE)).flex_none().rounded_full().border_1().border_color(rgb(0xcccccc)).flex().items_center().justify_center().text_size(px(11.)).text_color(rgb(0xaaaaaa)).child("?").tooltip(crate::ui::tooltip(tip)))),
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
        os::modal(dialog, "Import to Noshape", if page { 590. } else { 400. })
            .child(
                // Live document/77: the tiles 12 px under the header.
                // Live: the footer rule 27 px under the last checkbox row's text.
                os::modal_body()
                    .pt(px(4.))
                    .pb(px(16.))
                    .gap(px(8.))
                    .when(bare, |d| {
                        d.child(
                            // Live: help (cshid=importfiles).
                            os::info_box_link("imp-learn", noshape_ops::import::BARE_ASSEMBLY_NOTE, "Learn more", |_, cx| noshape_app_core::help::open("Import and export", cx)),
                        )
                        .child(
                            v_flex()
                                .gap(px(13.))
                                .child(check("imp-upload", "Upload only", None, "", c.upload_only, |c, v| c.upload_only = v, choice.clone()))
                                .when(density, |d| d.child(check("imp-dens", "Import material density", None, "Material density defined on models will be imported.", c.density, |c, v| c.density = v, choice.clone()))),
                        )
                    })
                    .when(!bare && !native && !stl, |d| {
                        d.child(
                            h_flex()
                                .gap(px(14.))
                                .child(tile("imp-single", !c.combine && !c.split, if page { "Import to a single document" } else { "Import to this document" }, None, "import-tile-single", (false, false), choice.clone()))
                                .when(page, |d| d.child(tile("imp-multi", c.split, "Split into multiple documents", Some("Groups subassemblies with their parts in the same document"), "import-tile-multi", (false, true), choice.clone())))
                                .child(tile("imp-flatten", c.combine, "Combine to a single Part Studio", Some("Use with small assemblies"), "import-tile-combine", (true, false), choice.clone())),
                        )
                    })
                    // Live document/77: the checkbox rows are 34 px apart.
                    .when(!bare, |d| d.child(v_flex().gap(px(13.)).when(!stl, |d| d.child(check("imp-app", "Import appearances", None, "Appearances defined on models will be imported.", c.appearances, |c, v| c.appearances = v, choice.clone())))
                    .when(density, |d| d.child(check("imp-dens", "Import material density", None, "Material density defined on models will be imported.", c.density, |c, v| c.density = v, choice.clone())))
                    .child(check("imp-yup", "Orient imported models with Y Axis Up", None, "Models built in an environment with Y-axis up, such as SOLIDWORKS, Creo, or Inventor, will keep their original orientation upon import.", c.y_up, |c, v| c.y_up = v, choice.clone()))
                    // The cards swap the list (live2 2403): Split has one document per part or assembly in the composite
                    // part's place, Combine to a single Part Studio neither (live G41).
                    .when(c.split, |d| d.child(check("imp-per-part", "Create one document per part or assembly", Some("(not recommended)"), "Each part and assembly is given its own document. For large assemblies, this option can create a large number of documents that can be difficult to manage.", c.per_part, |c, v| c.per_part = v, choice.clone())))
                    .when(!c.combine && !c.split, |d| d.child(check("imp-comp", "Create a composite part when importing multiple or non-solid bodies", None, "Parts within a single Part Studio will become a composite part upon import", c.composite, |c, v| c.composite = v, choice.clone())))
                    .when(!mesh && join, |d| d.child(check("imp-join", "Join adjacent surfaces", None, "Adjacent surfaces with overlapping edges will be joined upon import.", c.join, |c, v| c.join = v, choice.clone())))
                    .when(iges, |d| d.child(check("imp-iges", "IGES postprocessing", None, "Repairs the translation of linear edges (slower).", c.iges_post, |c, v| c.iges_post = v, choice.clone())))))
                    // Live dom/dialog-import-stl.txt: "Units" (14 px / 600) over a 212 px select.
                    .when(units_select && !bare, |d| {
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
            // A bare assembly's Import is disabled until Upload only is ticked (live upload-only-1118).
            .footer(os::modal_footer().pb(px(10.)).child(os::modal_ok(div().when(bare && !c.upload_only, |d| d.opacity(0.4)).child(os::primary_button("ok", "Import")))).child(os::modal_cancel(os::cancel_button("cancel", "Cancel", cx))))
            .on_ok(move |_, window, cx| {
                let (c, on_ok) = (ch.get(), on_ok.clone());
                if bare && !c.upload_only {
                    return false;
                }
                window.defer(cx, move |window, cx| on_ok(c, window, cx));
                true
            })
    });
}

/// Live's checkbox label box (dom/dialog-import-in-document.txt: 358 px; the documents page's 590 px modal 548 px:
/// its labels on one line, live2 2403) and the ? after the label text.
const LABEL_WIDTH: f32 = 358.;
const PAGE_LABEL_WIDTH: f32 = 548.;
const TIP_GAP: f32 = 8.;
const TIP_SIZE: f32 = 18.;

/// The lines of `text` broken at `breaks` (the byte index each new line
/// starts at; the ? element sits at `text.len()`), each as its part within the
/// first `bold` bytes and the rest. The last entry is the text before the ?,
/// empty when the ? wraps onto a line of its own.
fn wrapped_lines(text: &str, bold: usize, breaks: &[usize]) -> Vec<(String, String)> {
    let mut starts = vec![0];
    starts.extend(breaks.iter().copied().filter(|&b| b > 0 && b <= text.len()));
    let ends = starts.iter().skip(1).copied().chain([text.len()]);
    starts
        .iter()
        .zip(ends)
        .map(|(&a, b)| {
            let line = text[a..b].trim_end();
            let cut = bold.clamp(a, a + line.len()) - a;
            (line[..cut].to_string(), line[cut..].to_string())
        })
        .collect()
}

/// Translate the files and add their tabs (shared with the agent API:
/// `noshape_ops::import`).
/// A CAD file tab's Update… (live 2430): the new file imported at once with the dialog's defaults (Import to this
/// document, appearances), its tabs right after the active tab, the same toast; no modal.
pub fn reimport(doc: &Entity<DocModel>, path: PathBuf, cx: &mut App) {
    let after = doc.read(cx).session.active_tab;
    import(doc, &[path], Choice { appearances: true, ..Default::default() }, after, false, cx);
}

/// The files read on a background thread (the dialog closes at once; live shows no progress either, the tabs come when
/// translated), then their tabs added after `after` and live's bubble: "<file> has been imported. Open document", or
/// for a file that didn't translate a red "<file> uploaded but failed to translate. Open document" with the file kept in CAD
/// Imports (live import-export-1118); the reason goes to the Notifications bell (live IM3).
pub(crate) fn import(doc: &Entity<DocModel>, paths: &[PathBuf], c: Choice, after: Option<TabId>, keep_files: bool, cx: &mut App) {
    use noshape_ops::import as imp;
    let units = doc.read(cx).session.doc.settings.units;
    let choice = c.ops();
    let (doc, paths) = (doc.downgrade(), paths.to_vec());
    cx.spawn(async move |cx| {
        let read = {
            let paths = paths.clone();
            cx.background_spawn(async move { imp::read_inputs(&paths, &choice, keep_files, &units) }).await
        };
        cx.update(|cx| {
            let Some(doc) = doc.upgrade() else { return };
            let (inputs, mut errors) = read;
            let mut first = None;
            doc.update(cx, |doc, cx| {
                if !inputs.is_empty() {
                    let description = imp::description(&inputs.label);
                    let mut failed = Vec::new();
                    doc.edit_document(&description, cx, |d| (first, failed) = imp::apply(d, inputs, &choice, after, &units));
                    errors.extend(failed);
                }
                for e in &errors {
                    crate::notifications::push(cx, format!("Import {} failed", import_name(e).unwrap_or_default()), e.clone(), true);
                }
                // Live's bell entry: "Import <file>" with its steps done.
                if errors.is_empty() && first.is_some() && !choice.upload_only {
                    for p in &paths {
                        let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                        crate::notifications::push(cx, format!("Import {name}"), format!("Uploaded · Translated · Post-processing done · Created {name}"), false);
                    }
                }
                // The bubble's link shows what the import made: the new tab, or the kept file of one that failed.
                // A file that didn't translate gets live's red bubble (rr-import-1118: alert-danger, stays until ×).
                let (notice, tab, failed) = match (paths.as_slice(), errors.first()) {
                    ([p], None) => (format!("{} has been imported.", file_name(p)), first, false),
                    (_, Some(e)) => (failed_toast(e), import_name(e).and_then(|n| kept_file(&doc.session.doc, &n)), true),
                    _ => return,
                };
                let Some(tab) = tab else {
                    cx.emit(if failed { DocEvent::Error(notice) } else { DocEvent::Info(notice) });
                    return;
                };
                let me = cx.entity().downgrade();
                let open = move |_: &mut Window, cx: &mut App| {
                    let _ = me.update(cx, |_, cx| cx.emit(DocEvent::Ui(noshape_ops::UiRequest::ActivateTab(tab))));
                };
                match failed {
                    true => doc.error_with_link(notice, "Open document", open, cx),
                    false => doc.info_with_link(notice, "Open document", open, cx),
                }
            });
        });
    })
    .detach();
}

fn file_name(p: &std::path::Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// The newest file tab named `name` (an import keeps its files in CAD Imports).
fn kept_file(d: &noshape_model::Document, name: &str) -> Option<TabId> {
    d.tabs.iter().rev().find(|t| t.name == name && matches!(t.content, noshape_model::document::TabContent::File(_))).map(|t| t.id)
}

/// Live's toast for a file that didn't translate ("g47-3parts-unique.zip
/// uploaded but failed to translate."; the reason goes to its Notifications
/// bell, behaviours "qa5 zip import"); the import's error already reads so unless live refused the file.
fn failed_toast(e: &str) -> String {
    match import_name(e) {
        Some(name) => format!("{name} uploaded but failed to translate."),
        None => e.to_string(),
    }
}

/// The file an import error is about.
fn import_name(e: &str) -> Option<String> {
    if let Some((n, _)) = e.split_once(" was not translated.").or_else(|| e.split_once(" uploaded but failed to translate.")) {
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
        assert_eq!(super::failed_toast("bad.step uploaded but failed to translate."), "bad.step uploaded but failed to translate.");
        assert_eq!(super::import_name("bad.step uploaded but failed to translate.").as_deref(), Some("bad.step"));
        assert_eq!(super::failed_toast("Something else"), "Something else");
    }

    #[::core::prelude::v1::test]
    fn the_question_mark_follows_the_last_word() {
        let plain = |text: &str, breaks: &[usize]| wrapped_lines(text, text.len(), breaks).into_iter().map(|(b, g)| b + &g).collect::<Vec<_>>();
        let label = "Create a composite part when importing multiple or non-solid bodies";
        // Live: "…multiple or" / "non-solid bodies ?".
        assert_eq!(plain(label, &[51]), ["Create a composite part when importing multiple or", "non-solid bodies"]);
        // One line with room for the ?.
        assert_eq!(plain("Join adjacent surfaces", &[]), ["Join adjacent surfaces"]);
        // The ? alone on the next line.
        assert_eq!(plain("Join adjacent surfaces", &[22]), ["Join adjacent surfaces", ""]);
        // A muted part after the label, on its line or wrapped (live2 2403: "…assembly(not recommended)").
        let (label, text) = ("Create one document per part or assembly", "Create one document per part or assembly(not recommended)");
        let pair = |b: &str, g: &str| (b.to_string(), g.to_string());
        assert_eq!(wrapped_lines(text, label.len(), &[]), [pair(label, "(not recommended)")]);
        assert_eq!(wrapped_lines(text, label.len(), &[32]), [pair("Create one document per part or", ""), pair("assembly", "(not recommended)")]);
        assert_eq!(wrapped_lines(text, label.len(), &[45]), [pair("Create one document per part or assembly", "(not"), pair("", "recommended)")]);
    }
}
