//! Onshape's Part Studio "Export" modal (tab › Export…, part › Export…;
//! live document/80–81, dom/dialog-export-partstudio-assembly.txt): File
//! name (+ "View export rules", ?), Format ▾ (PARASOLID, ACIS, STEP, IGES,
//! PVZ, JT, RHINO, GLTF, GLB, OBJ, 3MF, STL), ☐ Export models oriented Y axis
//! up, the format's own options, Options ▾ (Download, Download and store file
//! in a new tab, Store file in a new tab), ☐ Export unique parts as
//! individual files, ☐ Include hidden instances in export; Export | Cancel.

use super::PartStudioView;
use gpui_kit::component::{Disableable as _, WindowExt as _, h_flex};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_kernel as k;
use noshape_model::BodyId;
use std::cell::RefCell;
use std::rc::Rc;
pub use noshape_ops::export::*;

/// Body layout (px, tuned against document/80-81): top padding, the file
/// name field under its label, a label under a field, a field under its
/// label, the first checkbox under a field, between checkboxes, and how much
/// closer a label sits right after a checkbox.
const BODY_PT: f32 = 0.;
const INPUT_MT: f32 = 2.;
const LABEL_MT: f32 = 4.;
const FIELD_MT: f32 = 5.;
const CHECK_MT: f32 = 9.;
const CHECK_GAP: f32 = 12.;
const LABEL_AFTER_CHECK: f32 = 4.;

impl PartStudioView {
    /// The Export modal for `bodies` (all visible parts when empty).
    pub fn open_part_export(&mut self, bodies: Vec<BodyId>, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.entity();
        // Live (dom/dialog-export-parasolid.txt): a part's file is
        // "<studio> - <part>"; the tab's is the studio's name.
        let base = match bodies.as_slice() {
            [b] => format!("{} - {}", self.studio_name(cx), self.display.body(b).map(|d| d.name.clone()).unwrap_or_else(|| "Part".into())),
            _ => self.studio_name(cx),
        };
        // Through the user's export rules (Preferences › User export rules).
        let (element, name, props) = match bodies.as_slice() {
            [b] => (noshape_model::export_rules::ExportElement::Part, self.display.body(b).map(|d| d.name.clone()).unwrap_or_default(), self.studio(cx).and_then(|s| s.part_properties.get(b)).cloned()),
            _ => (noshape_model::export_rules::ExportElement::PartStudio, self.studio_name(cx), None),
        };
        let values = crate::export_naming::values(&self.doc.read(cx).name(), &self.studio_name(cx), &name, props.as_ref());
        let base = crate::export_naming::default_name(element, ExportSettings::default().format.label(), &values, base);
        let parts_only = !bodies.is_empty();
        open_export_modal_with(base, false, parts_only, Rc::new(move |s, name, window, cx| {
            let bodies = bodies.clone();
            view.update(cx, |this, cx| this.run_part_export(s, name, bodies, window, cx));
        }), window, cx);
    }

    fn run_part_export(&mut self, s: ExportSettings, name: String, bodies: Vec<BodyId>, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(msg) = missing_writer(s.format) {
            self.flash_info(msg, cx);
            return;
        }
        // The exported bodies' MBD (STEP AP242 semantic PMI), in their order.
        let mut s = s;
        if let (Some(ctx), Some(ps)) = (self.context.as_ref(), self.studio(cx)) {
            let ids: Vec<BodyId> = ctx
                .bodies
                .iter()
                .filter(|b| bodies.is_empty() || bodies.contains(&b.id))
                .filter(|b| !(self.display.body(&b.id).is_some_and(|d| d.hidden) && !s.hidden && bodies.is_empty()))
                .map(|b| b.id.clone())
                .collect();
            s.pmi = noshape_ops::export::studio_pmi(ps, ctx, &ids, &self.doc.read(cx).session.doc.settings.units, s.mbd_text());
        }
        let name = if name.is_empty() { self.studio_name(cx) } else { name };
        // The saved file's own extension (OBJ downloads a .zip, as live).
        let several = if bodies.is_empty() { self.context.as_ref().is_some_and(|c| c.bodies.len() > 1) } else { bodies.len() > 1 };
        let ext = s.file_ext(several);
        // Options › Store file in a new tab: no download.
        if s.options == 2 {
            let msg = match store_export(&self.doc, self.tab, &format!("{name}.{ext}"), |p| self.write_part_export(&s, p, &bodies), cx) {
                Ok(file) => format!("Stored {file}"),
                Err(e) => format!("Export failed: {e:#}"),
            };
            self.flash_info(msg, cx);
            return;
        }
        let view = cx.entity();
        crate::file_dialogs::request_save_path("Export", format!("{name}.{ext}"), ext, window, cx, move |path, cx| {
            view.update(cx, |this, cx| {
                let msg = match this.write_part_export(&s, &path, &bodies) {
                    Ok(p) => {
                        // Download and store file in a new tab.
                        if s.options == 1 {
                            store_written(&this.doc, this.tab, &p, cx);
                        }
                        format!("Exported {}", p.display())
                    }
                    Err(e) => format!("Export failed: {e:#}"),
                };
                this.flash_info(msg, cx);
            });
        });
    }

    /// Write the export: one file, or one per part next to `path`.
    pub fn write_part_export(&self, s: &ExportSettings, path: &std::path::Path, selected: &[BodyId]) -> anyhow::Result<std::path::PathBuf> {
        let ctx = self.context.as_ref().ok_or_else(|| anyhow::anyhow!("Nothing to export yet"))?;
        let parts: Vec<(String, k::Shape, [f32; 4])> = ctx
            .bodies
            .iter()
            .filter(|b| selected.is_empty() || selected.contains(&b.id))
            .filter_map(|b| {
                let d = self.display.body(&b.id);
                if d.is_some_and(|d| d.hidden) && !s.hidden && selected.is_empty() {
                    return None;
                }
                let name = d.map(|d| d.name.clone()).unwrap_or_else(|| b.name.clone());
                let color = d.map(|d| d.color).unwrap_or([0.7, 0.7, 0.7, 1.0]);
                Some((name, b.shape.clone(), color))
            })
            .collect();
        write_export(s, path, parts)
    }
}


/// STEP AP242: only it has the derived tolerances and MBD text height
/// boxes (live 2035: AP214 / AP203 hide both).
fn step_ap242(s: &ExportSettings) -> bool {
    s.format == PartFormat::Step && (s.latest || s.version == 0)
}

/// The annotations text height field is shown (its value checked).
fn mbd_height_shown(s: &ExportSettings) -> bool {
    step_ap242(s) && s.mbd_text_height
}

/// What Export does with the chosen settings and file name.
pub type RunExport = Rc<dyn Fn(ExportSettings, String, &mut Window, &mut App)>;


/// Onshape's Export modal (Part Studio or, with `assembly`, Assembly list).
pub fn open_export_modal(base: String, assembly: bool, run: RunExport, window: &mut Window, cx: &mut App) {
    open_export_modal_with(base, assembly, false, run, window, cx);
}

/// [`open_export_modal`]; `parts_only`: exporting parts (a parts-list row,
/// a release row), which has no "Export unique parts as individual files"
/// or "Include hidden instances in export" (live dom/dialog-export-jt.txt
/// vs -jt-tab.txt).
pub fn open_export_modal_with(base: String, assembly: bool, parts_only: bool, run: RunExport, window: &mut Window, cx: &mut App) {
    let file = cx.new(|cx| InputState::new(window, cx).default_value(base));
    // The last export's format and options (live: exportSolidOptions).
    let st = Rc::new(RefCell::new(noshape_ops::export::SolidExportPrefs::load().settings(assembly)));
    // Resolution › Custom's fields (live 1745): STL's three, their lengths
    // in the STL units, then the other mesh formats' three.
    let custom: Rc<Vec<(&'static str, Entity<InputState>)>> = Rc::new(CUSTOM_FIELDS.iter().map(|f| (f.id, cx.new(|cx| InputState::new(window, cx).default_value(custom_text(&st.borrow(), f))))).collect());
    // A field that isn't a number disables Export (live: no message).
    let invalid = Rc::new(std::cell::Cell::new(false));
    for (f, (_, input)) in CUSTOM_FIELDS.iter().zip(custom.iter()) {
        let (st, invalid) = (st.clone(), invalid.clone());
        window
            .subscribe(input, cx, move |input, ev: &InputEvent, window, cx| {
                let text = input.read(cx).value().trim().to_string();
                let v = text.parse::<f64>().ok().filter(|v| v.is_finite());
                match ev {
                    InputEvent::Change => {
                        invalid.set(v.is_none() && !text.is_empty());
                        if let Some(v) = v.filter(|v| *v > 0.0) {
                            let scale = if f.stl_length { stl_mm(&st.borrow()) } else { 1.0 };
                            (f.set)(&mut st.borrow_mut().custom, v * scale);
                        }
                        window.refresh();
                    }
                    // Live: 0, -1, empty or text go back to the last value.
                    InputEvent::Blur if v.is_none_or(|v| v <= 0.0) => {
                        invalid.set(false);
                        let shown = custom_text(&st.borrow(), f);
                        input.update(cx, |i, cx| i.set_value(shown, window, cx));
                    }
                    _ => {}
                }
            })
            .detach();
    }
    // STEP › "Annotations text height (millimeter)" (live 1746): 0.0001 to 50,
    // else a message and Export disabled.
    let mbd_input = cx.new(|cx| InputState::new(window, cx).default_value(noshape_model::drawing::format::format_number(st.borrow().mbd_text_mm, None, 4)));
    let mbd_bad = Rc::new(std::cell::Cell::new(false));
    {
        let (st, mbd_bad) = (st.clone(), mbd_bad.clone());
        window
            .subscribe(&mbd_input, cx, move |input, ev: &InputEvent, window, cx| {
                if !matches!(ev, InputEvent::Change) {
                    return;
                }
                match input.read(cx).value().trim().parse::<f64>().ok().filter(|v| (0.0001..=50.0).contains(v)) {
                    Some(v) => {
                        st.borrow_mut().mbd_text_mm = v;
                        mbd_bad.set(false);
                    }
                    None => mbd_bad.set(true),
                }
                window.refresh();
            })
            .detach();
    }
    // Custom is pre-filled from the preset chosen before it in the same
    // format's list; STL's lengths follow its units.
    let shown_as = Rc::new(std::cell::Cell::new((st.borrow().format, st.borrow().resolution, st.borrow().units)));
    window.open_dialog(cx, move |dialog, window, cx| {
        {
            let (format, res, units) = shown_as.get();
            let mut s = st.borrow_mut();
            let prefill = s.format == format && s.resolution == noshape_ops::export::CUSTOM_RESOLUTION && res != noshape_ops::export::CUSTOM_RESOLUTION;
            if prefill {
                s.custom = noshape_ops::export::CustomResolution::preset(res);
            }
            shown_as.set((s.format, s.resolution, s.units));
            if prefill || units != s.units {
                let s = s.clone();
                for (f, (_, input)) in CUSTOM_FIELDS.iter().zip(custom.iter()) {
                    input.update(cx, |i, cx| i.set_value(custom_text(&s, f), window, cx));
                }
            }
        }
        let s = st.borrow().clone();
        let f = s.format;
        let pal = crate::theme::p();
        // The rows below the file name, in order; spacing depends on the
        // neighbours (live document/80-81): a field is 34 px with its label
        // above, the first checkbox under a field 12 px down, consecutive
        // checkboxes 39 px apart, a label right after a checkbox 2 px closer.
        enum Row {
            Select { id: &'static str, label: &'static str, opts: Vec<String>, cur: usize, disabled: bool, set: fn(&mut ExportSettings, usize) },
            Check { id: &'static str, label: &'static str, on: bool, set: fn(&mut ExportSettings, bool) },
            Number { label: String, input: Entity<InputState>, error: Option<&'static str> },
        }
        let sel = |id, label, opts: &[&str], cur, disabled, set| Row::Select { id, label, opts: opts.iter().map(|o| o.to_string()).collect(), cur, disabled, set };
        let chk = |id, label, on, set| Row::Check { id, label, on, set };
        let formats = PartFormat::list(assembly);
        let labels: Vec<&str> = formats.iter().map(|f| f.label()).collect();
        let units: Vec<&str> = UNITS.iter().map(|u| u.0).collect();
        let set_format: fn(&mut ExportSettings, usize) = if assembly { set_asm_format } else { set_part_format };
        let mut rows = vec![sel("ex-format", "Format", &labels, formats.iter().position(|x| *x == f).unwrap_or(0), false, set_format)];
        if f != PartFormat::Urdf {
            rows.push(chk("ex-yup", "Export models oriented Y axis up", s.y_up, |s, v| s.y_up = v));
        }
        match f {
            PartFormat::Parasolid => {
                rows.push(sel("ex-version", "Version", &PARASOLID_VERSIONS, s.version, s.latest, |s, i| s.version = i));
                rows.push(chk("ex-latest", "Use latest version", s.latest, |s, v| s.latest = v));
            }
            PartFormat::Step => {
                rows.push(sel("ex-pre", "Preprocessing", &STEP_PREPROCESSING, s.preprocessing, false, |s, i| s.preprocessing = i));
                rows.push(sel("ex-version", "Version", &STEP_VERSIONS, s.version, s.latest, |s, i| s.version = i));
                rows.push(chk("ex-latest", "Use latest version", s.latest, |s, v| s.latest = v));
                rows.push(chk("ex-custom-units", "Use custom units for export", s.custom_units, |s, v| s.custom_units = v));
                if s.custom_units {
                    rows.push(sel("ex-units", "Units", &units, s.units, false, |s, i| s.units = i));
                }
                if step_ap242(&s) {
                    rows.push(chk("ex-tol", "Include derived tolerances in export", s.derived_tolerances, |s, v| s.derived_tolerances = v));
                    rows.push(chk("ex-mbd", "Use custom annotation text height for MBD export", s.mbd_text_height, |s, v| s.mbd_text_height = v));
                }
                if mbd_height_shown(&s) {
                    let error = mbd_bad.get().then_some("Valid annotations text height in range from 0.0001 to 50 is required.");
                    rows.push(Row::Number { label: "Annotations text height (millimeter)".into(), input: mbd_input.clone(), error });
                }
            }
            PartFormat::Iges => rows.push(chk("ex-iges", "Use compatibility mode for surfaces and curves", s.iges_compat, |s, v| s.iges_compat = v)),
            PartFormat::Pvz => rows.push(chk("ex-pvz", "Group composite parts into a single part file", s.pvz_group, |s, v| s.pvz_group = v)),
            PartFormat::Rhino => {
                rows.push(sel("ex-version", "Version", &RHINO_VERSIONS, s.version, s.latest, |s, i| s.version = i));
                rows.push(chk("ex-latest", "Use latest version", s.latest, |s, v| s.latest = v));
            }
            PartFormat::Gltf | PartFormat::Glb => {
                rows.push(sel("ex-res", "Resolution", &RESOLUTIONS, s.resolution, false, |s, i| s.resolution = i));
                rows.push(chk("ex-compress", "Compress", s.compress, |s, v| s.compress = v));
            }
            PartFormat::Obj | PartFormat::ThreeMf => rows.push(sel("ex-res", "Resolution", &RESOLUTIONS, s.resolution, false, |s, i| s.resolution = i)),
            PartFormat::Stl => {
                rows.push(sel("ex-stl", "STL Format", &STL_FORMATS, if s.stl_text { 0 } else { 1 }, false, |s, i| s.stl_text = i == 0));
                rows.push(sel("ex-units", "Units", &units, s.units, false, |s, i| s.units = i));
                rows.push(sel("ex-res", "Resolution", &RESOLUTIONS, s.resolution, false, |s, i| s.resolution = i));
            }
            PartFormat::Urdf => {
                let geo: Vec<&str> = URDF_GEOMETRY.iter().map(|g| g.label()).collect();
                rows.push(sel("ex-geo", "Geometry format", &geo, s.urdf_geometry, false, |s, i| s.urdf_geometry = i));
                if URDF_GEOMETRY[s.urdf_geometry.min(3)] == PartFormat::Stl {
                    rows.push(sel("ex-stl", "STL Format", &STL_FORMATS, if s.stl_text { 0 } else { 1 }, false, |s, i| s.stl_text = i == 0));
                }
                rows.push(sel("ex-res", "Resolution", &RESOLUTIONS, s.resolution, false, |s, i| s.resolution = i));
            }
            PartFormat::Acis | PartFormat::Jt => {}
        }
        // Resolution › Custom: its three fields under the list (STL's, else
        // the tessellated formats').
        if s.resolution == noshape_ops::export::CUSTOM_RESOLUTION
            && let Some(at) = rows.iter().position(|r| matches!(r, Row::Select { id: "ex-res", .. }))
        {
            let stl = f == PartFormat::Stl || (f == PartFormat::Urdf && URDF_GEOMETRY[s.urdf_geometry.min(3)] == PartFormat::Stl);
            let range = if stl { 0..3 } else { 3..6 };
            for (k, i) in range.enumerate() {
                let f = &CUSTOM_FIELDS[i];
                let label = if f.stl_length { format!("{} ({})", f.label, noshape_ops::export::unit_abbreviation(s.units)) } else { f.label.to_string() };
                rows.insert(at + 1 + k, Row::Number { label, input: custom[i].1.clone(), error: None });
            }
        }
        rows.push(sel("ex-options", "Options", &OPTIONS, s.options, false, |s, i| s.options = i));
        if !parts_only {
            if f != PartFormat::Urdf {
                rows.push(chk("ex-individual", "Export unique parts as individual files", s.individual, |s, v| s.individual = v));
            }
            rows.push(chk("ex-hidden", "Include hidden instances in export", s.hidden, |s, v| s.hidden = v));
        }
        // Live (dom g39-export-*): an empty name is an error under the field;
        // OBJ adds a note on spaces in names.
        let empty = file.read(cx).value().trim().is_empty();
        let mut body = crate::os_ui::modal_body()
            .pt(px(BODY_PT))
            .pb(px(22.))
            .gap(px(0.))
            .child(
                h_flex()
                    .gap(px(4.))
                    .items_end()
                    .child(crate::os_ui::field_label("File name"))
                    .child(crate::part_studio_hooks::export_rules_link("ex-rules"))
                    .child(div().flex_1())
                    .child(div().size(px(16.)).rounded_full().bg(gpui_kit::rgb(0x555555)).flex().items_center().justify_center().text_size(px(12.)).font_weight(FontWeight::BOLD).text_color(gpui_kit::rgb_static(0xffffff)).child("?")),
            )
            .child(
                div()
                    .mt(px(INPUT_MT))
                    .h(px(34.))
                    .px(px(1.))
                    .flex()
                    .items_center()
                    .border_1()
                    .border_color(gpui_kit::rgb(if empty { 0xdc3545 } else { 0xaaaaaa }))
                    .rounded(px(4.))
                    .bg(pal.panel)
                    .child(Input::new(&file).appearance(false).text_size(px(14.))),
            )
            .when(empty, |d| d.child(div().mt(px(4.)).text_size(px(12.)).text_color(gpui_kit::rgb(0xdc3545)).child("Valid file name is required.")))
            .when(f == PartFormat::Obj, |d| d.child(div().mt(px(4.)).text_size(px(12.)).text_color(pal.text_muted).child("(OBJ names using spaces may not be supported by other systems)")));
        let mut prev_check = false;
        for row in rows {
            let st = st.clone();
            body = match row {
                Row::Select { id, label, opts, cur, disabled, set } => {
                    let mt = if prev_check { LABEL_MT - LABEL_AFTER_CHECK } else { LABEL_MT };
                    prev_check = false;
                    body.child(
                        div().mt(px(mt)).child(crate::os_ui::field_label(label)).child(
                            div()
                                .mt(px(FIELD_MT))
                                .relative()
                                .child(crate::os_ui::select_field(id, opts, cur, move |i, w, _| {
                                    set(&mut st.borrow_mut(), i);
                                    w.refresh();
                                }))
                                // Live: "Version" is grayed out while "Use latest
                                // version" is checked.
                                .when(disabled, |d| d.child(div().absolute().inset_0().rounded(px(4.)).bg(gpui_kit::rgba(0x00000011)).occlude())),
                        ),
                    )
                }
                Row::Number { label, input, error } => {
                    let mt = if prev_check { LABEL_MT - LABEL_AFTER_CHECK } else { LABEL_MT };
                    prev_check = false;
                    let border = if error.is_some() { 0xdc3545 } else { 0xaaaaaa };
                    body.child(
                        div()
                            .mt(px(mt))
                            .child(crate::os_ui::field_label(label))
                            .child(div().mt(px(FIELD_MT)).h(px(34.)).px(px(1.)).flex().items_center().border_1().border_color(gpui_kit::rgb(border)).rounded(px(4.)).bg(pal.panel).child(Input::new(&input).appearance(false).text_size(px(14.))))
                            .when_some(error, |d, e| d.child(div().mt(px(4.)).text_size(px(12.)).text_color(gpui_kit::rgb(0xdc3545)).child(e))),
                    )
                }
                Row::Check { id, label, on, set } => {
                    let mt = if prev_check { CHECK_GAP } else { CHECK_MT };
                    prev_check = true;
                    body.child(div().mt(px(mt)).child(modal_checkbox(id, label, on).on_click(move |_, w, _| {
                        set(&mut st.borrow_mut(), !on);
                        w.refresh();
                    })))
                }
            };
        }
        let (st2, file2, run2, invalid2, mbd_bad2) = (st.clone(), file.clone(), run.clone(), invalid.clone(), mbd_bad.clone());
        crate::os_ui::modal(dialog, "Export", 600.)
            .child(body)
            .footer(crate::os_ui::modal_footer().child(crate::os_ui::modal_ok(crate::os_ui::primary_button("ok", "Export").disabled(invalid.get() || (mbd_height_shown(&s) && mbd_bad.get())))).child(crate::os_ui::modal_cancel(crate::os_ui::cancel_button("cancel", "Cancel", cx))))
            .on_ok(move |_, window, cx| {
                let s = st2.borrow().clone();
                let name = file2.read(cx).value().trim().to_string();
                if name.is_empty() || invalid2.get() || (mbd_height_shown(&s) && mbd_bad2.get()) {
                    return false;
                }
                let mut prefs = (*s.memory).clone();
                prefs.keep(&s);
                prefs.save();
                let run = run2.clone();
                window.defer(cx, move |window, cx| run(s, name, window, cx));
                true
            })
    });
}


/// One of Resolution › Custom's fields (live 1745).
struct CustomField {
    id: &'static str,
    label: &'static str,
    /// STL's chordal tolerance and facet width: shown in the STL units.
    stl_length: bool,
    get: fn(&noshape_ops::export::CustomResolution) -> f64,
    set: fn(&mut noshape_ops::export::CustomResolution, f64),
}

const CUSTOM_FIELDS: [CustomField; 6] = [
    CustomField { id: "ex-chordal", label: "Chordal tolerance", stl_length: true, get: |c| c.chordal_mm, set: |c, v| c.chordal_mm = v },
    CustomField { id: "ex-angdev", label: "Angular deviation (deg)", stl_length: false, get: |c| c.angular_deg, set: |c, v| c.angular_deg = v },
    CustomField { id: "ex-minfacet", label: "Minimum facet width", stl_length: true, get: |c| c.min_facet_mm, set: |c, v| c.min_facet_mm = v },
    CustomField { id: "ex-distance", label: "Distance tolerance (m)", stl_length: false, get: |c| c.distance_m, set: |c, v| c.distance_m = v },
    CustomField { id: "ex-angtol", label: "Angular tolerance (deg)", stl_length: false, get: |c| c.angular_tol_deg, set: |c, v| c.angular_tol_deg = v },
    CustomField { id: "ex-maxchord", label: "Maximum chord length (m)", stl_length: false, get: |c| c.max_chord_m, set: |c, v| c.max_chord_m = v },
];

/// Millimetres per STL unit.
fn stl_mm(s: &ExportSettings) -> f64 {
    noshape_ops::export::UNITS[s.units].1 * 1000.0
}

/// A Custom field's text (live: 1 mm at Inch shows 0.03937).
fn custom_text(s: &ExportSettings, f: &CustomField) -> String {
    let v = (f.get)(&s.custom) / if f.stl_length { stl_mm(s) } else { 1.0 };
    noshape_model::drawing::format::format_number(v, None, 6)
}

/// A Bootstrap modal checkbox (live document/80): a 14 px box (1 px gray,
/// radius 3; checked blue with a white check), 8 px gap, 14 px `#333` label.
pub(super) fn modal_checkbox(id: &'static str, label: &'static str, on: bool) -> Stateful<Div> {
    let pal = crate::theme::p();
    h_flex()
        .debug_selector(move || id.into())
        .id(id)
        .h(px(27.))
        .gap(px(8.))
        .cursor_pointer()
        .text_size(px(14.))
        .text_color(pal.text)
        .child(
            div()
                .size(px(14.))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(3.))
                .border_1()
                .border_color(if on { pal.accent } else { gpui_kit::rgb(0xadadad) })
                .bg(if on { pal.accent } else { gpui_kit::rgb(0xffffff) })
                .when(on, |d| d.child(crate::ui::glyph("check", 10., gpui_kit::rgb_static(0xffffff)))),
        )
        .child(label)
}

/// Options › Store file in a new tab: write the export to a temporary file
/// (`write` returns the file written: a .zip for individual files) and keep
/// it as a file tab after `tab` (shared with the agent API:
/// `noshape_ops::export::store_file_tab`). Returns the stored file's name.
pub fn store_export(doc: &Entity<crate::doc_model::DocModel>, tab: noshape_model::document::TabId, file: &str, write: impl FnOnce(&std::path::Path) -> anyhow::Result<std::path::PathBuf>, cx: &mut App) -> anyhow::Result<String> {
    let dir = std::env::temp_dir().join(format!("noshape-store-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let written = write(&dir.join(file))?;
    let name = store_written(doc, tab, &written, cx);
    let _ = std::fs::remove_file(&written);
    Ok(name)
}

/// Keep an exported file (already written to `path`) as a file tab after `tab`.
pub fn store_written(doc: &Entity<crate::doc_model::DocModel>, tab: noshape_model::document::TabId, path: &std::path::Path, cx: &mut App) -> String {
    let name = path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
    if let Ok(bytes) = std::fs::read(path) {
        doc.update(cx, |d, cx| d.edit_document(&noshape_ops::export::store_description(&name), cx, |d| {
            noshape_ops::export::store_file_tab(d, &name, &bytes, tab);
        }));
    }
    name
}
