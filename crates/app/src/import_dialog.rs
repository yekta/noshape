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
use noshape_model::{PartStudio, TabContent};
use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;

/// Native part files (their own dialog variant).
const NATIVE_PARTS: &[&str] = &["sldprt", "par", "psm", "prt", "ipt"];
/// Mesh files without units: Onshape asks for them (live G35, G35b).
const MESH_FILES: &[&str] = &["stl", "obj"];
/// The Units select, as Onshape lists units in its STL dialogs.
const MESH_UNITS: [(&str, noshape_units::LengthUnit); 6] = [
    ("Centimeter", noshape_units::LengthUnit::Centimeter),
    ("Foot", noshape_units::LengthUnit::Foot),
    ("Inch", noshape_units::LengthUnit::Inch),
    ("Meter", noshape_units::LengthUnit::Meter),
    ("Millimeter", noshape_units::LengthUnit::Millimeter),
    ("Yard", noshape_units::LengthUnit::Yard),
];

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
    let (mesh, units_select) = (all_of(&["stl", "obj", "3mf"]), stl || obj);
    // Native assemblies (a zip; live G38h) keep the tiles and add Import
    // material density like native parts.
    let zip = all_of(&["zip"]);
    let density = native || zip;
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

/// Translate the files and add their Part Studio tab(s).
fn import(doc: &Entity<DocModel>, paths: &[PathBuf], c: Choice, after: Option<TabId>, keep_files: bool, cx: &mut App) -> Option<TabId> {
    let units = doc.read(cx).session.doc.settings.units;
    let mut read: Vec<(String, String, crate::workspace::Foreign)> = Vec::new();
    // Native assemblies (a zip named like the root assembly; live G38).
    let mut assemblies: Vec<noshape_model::native_assembly::AssemblyZip> = Vec::new();
    for path in paths {
        if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("zip")) {
            match noshape_model::native_assembly::read_assembly_zip(path) {
                Ok(a) => assemblies.push(a),
                // Onshape keeps the zip as a file tab and reports it.
                Err(e) if e == noshape_model::native_assembly::STEP_IN_ZIP => {
                    let msg = format!("{} was not translated. {e}", crate::workspace::import_names(path).1);
                    doc.update(cx, |_, cx| cx.emit(DocEvent::Error(msg)));
                }
                Err(e) => doc.update(cx, |_, cx| cx.emit(DocEvent::Error(format!("Import failed: {}: {e:#}", path.display())))),
            }
            continue;
        }
        // Mesh files in the unit chosen in the dialog.
        let units = if MESH_FILES.iter().any(|e| path.extension().is_some_and(|x| x.eq_ignore_ascii_case(e))) { noshape_units::UnitPrefs { length: MESH_UNITS[c.unit.min(MESH_UNITS.len() - 1)].1, ..units } } else { units };
        match crate::workspace::read_foreign(path, &units) {
            Ok(foreign) => {
                let (name, file) = crate::workspace::import_names(path);
                read.push((name, file, foreign));
            }
            Err(e) => doc.update(cx, |_, cx| cx.emit(DocEvent::Error(format!("Import failed: {}: {e:#}", path.display())))),
        }
    }
    // A file Onshape couldn't translate still gets its file tab.
    if read.is_empty() && assemblies.is_empty() && !keep_files {
        return None;
    }
    let mut first = None;
    let mut label = read.iter().map(|r| r.1.clone()).chain(assemblies.iter().map(|a| format!("{}.zip", a.zip))).collect::<Vec<_>>().join(", ");
    if label.is_empty() {
        label = paths.iter().map(|p| crate::workspace::import_names(p).1).collect::<Vec<_>>().join(", ");
    }
    let mut errors: Vec<String> = Vec::new();
    let files: Vec<noshape_model::file_tab::FileTab> = if keep_files {
        paths.iter().filter_map(|p| Some(noshape_model::file_tab::FileTab::new(&crate::workspace::import_names(p).1, &std::fs::read(p).ok()?, noshape_model::file_tab::now()))).collect()
    } else {
        Vec::new()
    };
    doc.update(cx, |doc, cx| {
        doc.edit_document(&format!("Import {label}"), cx, |d| {
            // Onshape keeps the imported files in a "CAD Imports" folder.
            if !files.is_empty() {
                let folder = crate::file_tab_view::cad_imports_folder(d);
                for f in files {
                    crate::file_tab_view::add_file_tab(d, f, Some(folder), None);
                }
            }
            let mut at = after;
            for a in &assemblies {
                // Combine: one Part Studio, no Assembly (live G41 (5)).
                if c.combine {
                    match noshape_model::native_assembly::add_native_assembly_combined(d, &a.root, &a.components, &a.files, c.appearances, &units) {
                        Ok(studio) => {
                            crate::workspace::place_after(d, studio, at);
                            at = Some(studio);
                            first.get_or_insert(studio);
                        }
                        Err(e) => errors.push(format!("Import failed: {}: {e}", a.root)),
                    }
                    continue;
                }
                match noshape_model::native_assembly::add_native_zip(d, a, c.appearances, &units) {
                    Ok(noshape_model::native_assembly::NativeTabs::Assembly { studio, assembly }) => {
                        crate::workspace::place_after(d, assembly, at);
                        crate::workspace::place_after(d, studio, Some(assembly));
                        at = Some(studio);
                        first.get_or_insert(assembly);
                    }
                    Ok(noshape_model::native_assembly::NativeTabs::Studio(studio)) => {
                        crate::workspace::place_after(d, studio, at);
                        at = Some(studio);
                        first.get_or_insert(studio);
                    }
                    Err(e) => errors.push(format!("Import failed: {}: {e}", a.root)),
                }
            }
            let groups: Vec<Vec<&(String, String, crate::workspace::Foreign)>> = if c.combine { vec![read.iter().collect()] } else { read.iter().map(|r| vec![r]).collect() };
            for g in groups {
                let mut ps = PartStudio::new();
                for (_, file, foreign) in &g {
                    crate::workspace::add_import_feature(&mut ps, file, foreign.clone(), &units);
                    if let Some(f) = ps.features.last_mut() {
                        f.params.set("importAppearances", noshape_model::ParamValue::Bool(c.appearances));
                        f.params.set("specifyMaterialData", noshape_model::ParamValue::Bool(c.density));
                        f.params.set("importMaterialDensity", noshape_model::ParamValue::Bool(c.density));
                        f.params.set("yAxisIsUp", noshape_model::ParamValue::Bool(c.y_up));
                        f.params.set("createComposite", noshape_model::ParamValue::Bool(c.composite));
                        f.params.set("joinAdjacentSurfaces", noshape_model::ParamValue::Bool(c.join));
                    }
                }
                let base = g[0].0.clone();
                let name = if d.tabs.iter().any(|t| t.name == base) { d.next_tab_name(&base) } else { base };
                let id = d.add_tab(&name, TabContent::PartStudio(ps));
                crate::workspace::place_after(d, id, at);
                at = Some(id);
                first.get_or_insert(id);
            }
        });
        for e in errors {
            cx.emit(DocEvent::Error(e));
        }
    });
    first
}

