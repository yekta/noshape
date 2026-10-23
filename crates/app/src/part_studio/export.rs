//! Onshape's Part Studio "Export" modal (tab › Export…, part › Export…;
//! live document/80–81, dom/dialog-export-partstudio-assembly.txt): File
//! name (+ "View export rules", ?), Format ▾ (PARASOLID, ACIS, STEP, IGES,
//! PVZ, JT, RHINO, GLTF, GLB, OBJ, 3MF, STL), ☐ Export models oriented Y axis
//! up, the format's own options, Options ▾ (Download, Download and store file
//! in a new tab, Store file in a new tab), ☐ Export unique parts as
//! individual files, ☐ Include hidden instances in export; Export | Cancel.

use super::PartStudioView;
use gpui_kit::component::{WindowExt as _, h_flex};
use gpui_kit::component::input::{Input, InputState};
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
        let name = if name.is_empty() { self.studio_name(cx) } else { name };
        let ext = s.format.ext();
        let view = cx.entity();
        crate::files::request_save_path("Export", format!("{name}.{ext}"), ext, window, cx, move |path, cx| {
            view.update(cx, |this, cx| {
                let msg = match this.write_part_export(&s, &path, &bodies) {
                    Ok(p) => format!("Exported {}", p.display()),
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
    let st = Rc::new(RefCell::new(ExportSettings::default()));
    window.open_dialog(cx, move |dialog, _, cx| {
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
                rows.push(chk("ex-tol", "Include derived tolerances in export", s.derived_tolerances, |s, v| s.derived_tolerances = v));
                rows.push(chk("ex-mbd", "Use custom annotation text height for MBD export", s.mbd_text_height, |s, v| s.mbd_text_height = v));
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
        rows.push(sel("ex-options", "Options", &OPTIONS, 0, false, |_, _| {}));
        if !parts_only {
            if f != PartFormat::Urdf {
                rows.push(chk("ex-individual", "Export unique parts as individual files", s.individual, |s, v| s.individual = v));
            }
            rows.push(chk("ex-hidden", "Include hidden instances in export", s.hidden, |s, v| s.hidden = v));
        }
        let mut body = crate::os_ui::modal_body()
            .pt(px(BODY_PT))
            .pb(px(22.))
            .gap(px(0.))
            .child(
                h_flex()
                    .gap(px(4.))
                    .items_end()
                    .child(crate::os_ui::field_label("File name"))
                    .child(crate::os_ui::export_rules_link("ex-rules"))
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
                    .border_color(gpui_kit::rgb(0xaaaaaa))
                    .rounded(px(4.))
                    .bg(pal.panel)
                    .child(Input::new(&file).appearance(false).text_size(px(14.))),
            );
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
        let (st2, file2, run2) = (st.clone(), file.clone(), run.clone());
        crate::os_ui::modal(dialog, "Export", 600.)
            .child(body)
            .footer(crate::os_ui::modal_footer().child(crate::os_ui::modal_ok(crate::os_ui::primary_button("ok", "Export"))).child(crate::os_ui::modal_cancel(crate::os_ui::cancel_button("cancel", "Cancel", cx))))
            .on_ok(move |_, window, cx| {
                let s = st2.borrow().clone();
                let name = file2.read(cx).value().trim().to_string();
                let run = run2.clone();
                window.defer(cx, move |window, cx| run(s, name, window, cx));
                true
            })
    });
}


/// A Bootstrap modal checkbox (live document/80): a 14 px box (1 px gray,
/// radius 3; checked blue with a white check), 8 px gap, 14 px `#333` label.
fn modal_checkbox(id: &'static str, label: &'static str, on: bool) -> Stateful<Div> {
    let pal = crate::theme::p();
    h_flex()
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
