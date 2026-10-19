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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PartFormat {
    Parasolid,
    Acis,
    Step,
    Iges,
    Pvz,
    Jt,
    Rhino,
    Gltf,
    Glb,
    Obj,
    ThreeMf,
    /// Assemblies only: a robot description with its mesh files (zip).
    Urdf,
    Stl,
}

impl PartFormat {
    /// Onshape's order (PARASOLID is the default).
    pub const ALL: [PartFormat; 12] = [Self::Parasolid, Self::Acis, Self::Step, Self::Iges, Self::Pvz, Self::Jt, Self::Rhino, Self::Gltf, Self::Glb, Self::Obj, Self::ThreeMf, Self::Stl];

    /// The Format list: Part Studios as `ALL`; assemblies without RHINO and
    /// with URDF before STL (live dom/dialog-export-partstudio-assembly.txt).
    pub fn list(assembly: bool) -> Vec<PartFormat> {
        if assembly {
            vec![Self::Parasolid, Self::Acis, Self::Step, Self::Iges, Self::Pvz, Self::Jt, Self::Gltf, Self::Glb, Self::Obj, Self::ThreeMf, Self::Urdf, Self::Stl]
        } else {
            Self::ALL.to_vec()
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Parasolid => "PARASOLID",
            Self::Acis => "ACIS",
            Self::Step => "STEP",
            Self::Iges => "IGES",
            Self::Pvz => "PVZ",
            Self::Jt => "JT",
            Self::Rhino => "RHINO",
            Self::Gltf => "GLTF",
            Self::Glb => "GLB",
            Self::Obj => "OBJ",
            Self::ThreeMf => "3MF",
            Self::Urdf => "URDF",
            Self::Stl => "STL",
        }
    }

    pub fn ext(self) -> &'static str {
        match self {
            Self::Parasolid => "x_t",
            Self::Acis => "sat",
            Self::Step => "step",
            Self::Iges => "igs",
            Self::Pvz => "pvz",
            Self::Jt => "jt",
            Self::Rhino => "3dm",
            Self::Gltf => "gltf",
            Self::Glb => "glb",
            Self::Obj => "obj",
            Self::ThreeMf => "3mf",
            Self::Urdf => "zip",
            Self::Stl => "stl",
        }
    }

    fn mesh(self) -> bool {
        matches!(self, Self::Gltf | Self::Glb | Self::Obj | Self::ThreeMf | Self::Stl)
    }
}

const PARASOLID_VERSIONS: [&str; 25] = ["38.1", "37.1", "37.0", "36.0", "35.1", "34.1", "34.0", "33.1", "33.0", "32.1", "32.0", "31.1", "31.0", "30.1", "30.0", "29.1", "29.0", "28.1", "28.0", "27.1", "27.0", "26.1", "26.0", "25.1", "25.0"];
const STEP_PREPROCESSING: [&str; 5] = ["None", "Automatic", "Remove small entities only (advanced)", "Remove small entities only (basic)", "Remove small entities only (advanced) and convert all surfaces to b-surfaces"];
const STEP_VERSIONS: [&str; 3] = ["AP242", "AP214", "AP203"];
const RHINO_VERSIONS: [&str; 7] = ["8", "7", "6", "5", "4", "3", "2"];
const RESOLUTIONS: [&str; 4] = ["Coarse", "Medium", "Fine", "Custom"];
const STL_FORMATS: [&str; 2] = ["Text", "Binary"];
/// Units and their size in meters.
const UNITS: [(&str, f64); 6] = [("Centimeter", 0.01), ("Foot", 0.3048), ("Inch", 0.0254), ("Meter", 1.0), ("Millimeter", 0.001), ("Yard", 0.9144)];
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
const OPTIONS: [&str; 3] = ["Download", "Download and store file in a new tab", "Store file in a new tab"];
/// URDF "Geometry format".
const URDF_GEOMETRY: [PartFormat; 4] = [PartFormat::Stl, PartFormat::Gltf, PartFormat::Glb, PartFormat::Obj];

#[derive(Clone, Debug)]
pub struct ExportSettings {
    pub format: PartFormat,
    pub y_up: bool,
    /// Version index (Parasolid, STEP, Rhino) and "Use latest version".
    pub version: usize,
    pub latest: bool,
    pub preprocessing: usize,
    pub custom_units: bool,
    pub derived_tolerances: bool,
    pub mbd_text_height: bool,
    pub iges_compat: bool,
    pub pvz_group: bool,
    pub resolution: usize,
    pub compress: bool,
    pub stl_text: bool,
    pub units: usize,
    pub individual: bool,
    pub hidden: bool,
    /// URDF: index into `URDF_GEOMETRY`.
    pub urdf_geometry: usize,
}

impl Default for ExportSettings {
    fn default() -> Self {
        Self {
            format: PartFormat::Parasolid,
            y_up: false,
            version: 0,
            latest: true,
            preprocessing: 0,
            custom_units: false,
            derived_tolerances: false,
            mbd_text_height: false,
            iges_compat: false,
            pvz_group: false,
            resolution: 1,
            compress: false,
            stl_text: false,
            units: 3,
            individual: false,
            hidden: false,
            urdf_geometry: 0,
        }
    }
}

impl PartStudioView {
    /// The Export modal for `bodies` (all visible parts when empty).
    pub fn open_part_export(&mut self, bodies: Vec<BodyId>, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.entity();
        let base = match bodies.as_slice() {
            [b] => self.display.body(b).map(|d| d.name.clone()).unwrap_or_else(|| "Part".into()),
            _ => self.studio_name(cx),
        };
        open_export_modal(base, false, Rc::new(move |s, name, window, cx| {
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

/// Write `parts` (meters, placed; linear RGBA): Y axis up, one file, or
/// with "Export unique parts as individual files" one file per part zipped
/// together (Onshape), or URDF. Returns the written file.
pub fn write_export(s: &ExportSettings, path: &std::path::Path, parts: Vec<(String, k::Shape, [f32; 4])>) -> anyhow::Result<std::path::PathBuf> {
    anyhow::ensure!(!parts.is_empty(), "There are no parts to export");
    if let Some(msg) = missing_writer(s.format) {
        anyhow::bail!(msg);
    }
    // Y axis up: the model's +Z becomes +Y.
    let parts: Vec<(String, k::Shape, [f32; 4])> = if s.y_up && s.format != PartFormat::Urdf {
        parts
            .into_iter()
            .map(|(n, sh, c)| Ok((n, sh.transformed(&k::Transform::Rotate { origin: glam::DVec3::ZERO, axis: glam::DVec3::X, angle: -std::f64::consts::FRAC_PI_2 })?, c)))
            .collect::<anyhow::Result<_>>()?
    } else {
        parts
    };
    let tmp = |i: usize, ext: &str| std::env::temp_dir().join(format!("noshape-export-{}-{i}.{ext}", std::process::id()));
    // Each part through the format's writer into a temporary file.
    let files_of = |s: &ExportSettings, ext: &str| -> anyhow::Result<Vec<(String, Vec<u8>)>> {
        let mut used = std::collections::HashSet::new();
        let mut out = Vec::new();
        for (i, p) in parts.iter().enumerate() {
            let mut name = sanitize(&p.0).replace(' ', "_");
            while !used.insert(name.clone()) {
                name.push('_');
            }
            let t = tmp(i, ext);
            write_parts(s, &t, std::slice::from_ref(p))?;
            out.push((format!("{name}.{ext}"), std::fs::read(&t)?));
            let _ = std::fs::remove_file(&t);
        }
        Ok(out)
    };
    if s.format == PartFormat::Urdf {
        let geo = URDF_GEOMETRY[s.urdf_geometry.min(URDF_GEOMETRY.len() - 1)];
        let ms = ExportSettings { format: geo, y_up: false, individual: false, ..s.clone() };
        let robot = path.file_stem().map(|x| x.to_string_lossy().to_string()).unwrap_or_else(|| "robot".into());
        let meshes = files_of(&ms, geo.ext())?;
        let mut urdf = format!("<?xml version=\"1.0\"?>\n<robot name=\"{}\">\n  <link name=\"base_link\"/>\n", xml_escape(&robot));
        for ((file, _), p) in meshes.iter().zip(&parts) {
            let link = file.rsplit_once('.').map(|x| x.0).unwrap_or(file);
            let c = p.2;
            urdf.push_str(&format!(
                "  <link name=\"{link}\">\n    <visual>\n      <geometry><mesh filename=\"package://{robot}/meshes/{file}\"/></geometry>\n      <material name=\"{link}_color\"><color rgba=\"{} {} {} 1\"/></material>\n    </visual>\n    <collision>\n      <geometry><mesh filename=\"package://{robot}/meshes/{file}\"/></geometry>\n    </collision>\n  </link>\n  <joint name=\"{link}_joint\" type=\"fixed\">\n    <parent link=\"base_link\"/>\n    <child link=\"{link}\"/>\n  </joint>\n",
                c[0].powf(1.0 / 2.2),
                c[1].powf(1.0 / 2.2),
                c[2].powf(1.0 / 2.2)
            ));
        }
        urdf.push_str("</robot>\n");
        let names: Vec<String> = meshes.iter().map(|(f, _)| format!("meshes/{f}")).collect();
        let urdf_name = format!("{robot}.urdf");
        let mut entries: Vec<(&str, &[u8])> = vec![(urdf_name.as_str(), urdf.as_bytes())];
        entries.extend(names.iter().zip(&meshes).map(|(n, (_, b))| (n.as_str(), b.as_slice())));
        let out = path.with_extension("zip");
        std::fs::write(&out, stored_zip(&entries))?;
        return Ok(out);
    }
    if s.individual && parts.len() > 1 {
        let files = files_of(s, s.format.ext())?;
        let entries: Vec<(&str, &[u8])> = files.iter().map(|(n, b)| (n.as_str(), b.as_slice())).collect();
        let out = path.with_extension("zip");
        std::fs::write(&out, stored_zip(&entries))?;
        return Ok(out);
    }
    write_parts(s, path, &parts)?;
    Ok(path.to_path_buf())
}

/// What Export does with the chosen settings and file name.
pub type RunExport = Rc<dyn Fn(ExportSettings, String, &mut Window, &mut App)>;

fn set_part_format(s: &mut ExportSettings, i: usize) {
    s.format = PartFormat::ALL[i];
    s.version = 0;
    s.latest = true;
}

fn set_asm_format(s: &mut ExportSettings, i: usize) {
    s.format = PartFormat::list(true)[i];
    s.version = 0;
    s.latest = true;
    // Onshape checks "Include hidden instances in export" for URDF.
    s.hidden = s.format == PartFormat::Urdf;
}

/// Onshape's Export modal (Part Studio or, with `assembly`, Assembly list).
pub fn open_export_modal(base: String, assembly: bool, run: RunExport, window: &mut Window, cx: &mut App) {
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
        if f != PartFormat::Urdf {
            rows.push(chk("ex-individual", "Export unique parts as individual files", s.individual, |s, v| s.individual = v));
        }
        rows.push(chk("ex-hidden", "Include hidden instances in export", s.hidden, |s, v| s.hidden = v));
        let mut body = crate::os_ui::modal_body()
            .pt(px(BODY_PT))
            .pb(px(22.))
            .gap(px(0.))
            .child(
                h_flex()
                    .gap(px(4.))
                    .items_end()
                    .child(crate::os_ui::field_label("File name"))
                    .child(div().pb(px(1.)).text_size(px(12.)).text_color(gpui_kit::rgb(0x094174)).child("View export rules"))
                    .child(div().flex_1())
                    .child(div().size(px(16.)).rounded_full().bg(gpui_kit::rgb(0x555555)).flex().items_center().justify_center().text_size(px(12.)).font_weight(FontWeight::BOLD).text_color(gpui_kit::rgb(0xffffff)).child("?")),
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

/// The formats whose writer is still to come (kern): Export shows Onshape's
/// flow, then this error.
pub fn missing_writer(f: PartFormat) -> Option<String> {
    let _ = f; // every format has its writer now (kern)
    None
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
                .when(on, |d| d.child(crate::ui::glyph("check", 10., gpui_kit::rgb(0xffffff)))),
        )
        .child(label)
}

fn sanitize(name: &str) -> String {
    name.chars().map(|c| if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') { '_' } else { c }).collect()
}

/// Chord tolerance (meters) of a mesh resolution, from the parts' size.
fn deflection(s: &ExportSettings, parts: &[(String, k::Shape, [f32; 4])]) -> f64 {
    let diag = parts.iter().filter_map(|p| p.1.bounding_box(false).ok()).map(|b| b.diagonal()).fold(0.0f64, f64::max).max(1e-3);
    let rel = match s.resolution {
        0 => 1e-3,
        2 | 3 => 5e-5,
        _ => 2e-4,
    };
    diag * rel
}

fn write_parts(s: &ExportSettings, path: &std::path::Path, parts: &[(String, k::Shape, [f32; 4])]) -> anyhow::Result<()> {
    match s.format {
        PartFormat::Step => {
            let shapes: Vec<k::ExportShape> = parts.iter().map(|(n, sh, c)| k::ExportShape { shape: sh.clone(), name: Some(n.clone()), color: Some([c[0].powf(1.0 / 2.2), c[1].powf(1.0 / 2.2), c[2].powf(1.0 / 2.2)]) }).collect();
            let schema = if s.latest { k::StepSchema::Ap242 } else { [k::StepSchema::Ap242, k::StepSchema::Ap214, k::StepSchema::Ap203].get(s.version).copied().unwrap_or(k::StepSchema::Ap242) };
            let file_unit = if s.custom_units {
                match UNITS[s.units].0 {
                    "Centimeter" => Some(k::LengthUnit::Centimeter),
                    "Foot" => Some(k::LengthUnit::Foot),
                    "Inch" => Some(k::LengthUnit::Inch),
                    "Meter" => Some(k::LengthUnit::Meter),
                    _ => Some(k::LengthUnit::Millimeter),
                }
            } else {
                None
            };
            k::write_step_with(path, &shapes, k::LengthUnit::Meter, &k::StepOptions { schema, file_unit })?;
        }
        PartFormat::Iges => k::write_iges(path, &parts.iter().map(|p| p.1.clone()).collect::<Vec<_>>(), k::LengthUnit::Meter)?,
        // Text transmit file, schema 12006: every Parasolid version reads it, so
        // the Version choice needs no other format.
        PartFormat::Parasolid => k::write_parasolid(path, &parts.iter().map(|p| p.1.clone()).collect::<Vec<_>>(), k::LengthUnit::Meter, false)?,
        PartFormat::Acis => k::write_acis(path, &parts.iter().map(|p| p.1.clone()).collect::<Vec<_>>(), k::LengthUnit::Meter)?,
        PartFormat::Pvz => {
            let shapes: Vec<k::ExportShape> = parts.iter().map(|(n, sh, _)| k::ExportShape { shape: sh.clone(), name: Some(n.clone()), color: None }).collect();
            k::write_pvz(path, &shapes, k::LengthUnit::Meter)?;
        }
        PartFormat::Jt => {
            let shapes: Vec<k::ExportShape> = parts.iter().map(|(n, sh, _)| k::ExportShape { shape: sh.clone(), name: Some(n.clone()), color: None }).collect();
            k::write_jt(path, &shapes, k::LengthUnit::Meter)?;
        }
        PartFormat::Rhino => {
            let shapes: Vec<k::ExportShape> = parts.iter().map(|(n, sh, _)| k::ExportShape { shape: sh.clone(), name: Some(n.clone()), color: None }).collect();
            let version = if s.latest { 8 } else { RHINO_VERSIONS.get(s.version).and_then(|v| v.parse().ok()).unwrap_or(8) };
            k::write_rhino(path, &shapes, k::LengthUnit::Meter, version)?;
        }
        f if f.mesh() => {
            let defl = deflection(s, parts);
            let meshes: Vec<(String, k::Tessellation, [f32; 4])> = parts.iter().map(|(n, sh, c)| Ok((n.clone(), k::tessellate(sh, defl, 0.35, true)?, *c))).collect::<anyhow::Result<_>>()?;
            match f {
                PartFormat::Stl | PartFormat::Obj => {
                    // STL: in the chosen unit; OBJ: meters.
                    let scale = if f == PartFormat::Stl { 1.0 / UNITS[s.units].1 } else { 1.0 } as f32;
                    let mut all = k::Tessellation::default();
                    for (_, m, _) in &meshes {
                        let base = all.positions.len() as u32;
                        all.positions.extend(m.positions.iter().map(|p| [p[0] * scale, p[1] * scale, p[2] * scale]));
                        all.normals.extend_from_slice(&m.normals);
                        all.indices.extend(m.indices.iter().map(|i| i + base));
                    }
                    if f == PartFormat::Stl {
                        k::write_stl_mesh(&all, path, if s.stl_text { k::StlFormat::Ascii } else { k::StlFormat::Binary })?;
                    } else {
                        k::write_obj_mesh(&all, path)?;
                    }
                }
                PartFormat::Gltf | PartFormat::Glb => std::fs::write(path, gltf(&meshes, f == PartFormat::Glb))?,
                _ => std::fs::write(path, three_mf(&meshes))?,
            }
        }
        f => anyhow::bail!("{} export is not available in noshape yet", f.label()),
    }
    Ok(())
}

/// glTF 2.0: one mesh node per part (positions, normals, indices) with its
/// colour as a PBR material; `.glb` packs JSON and buffer into one file,
/// `.gltf` embeds the buffer as a data URI.
fn gltf(meshes: &[(String, k::Tessellation, [f32; 4])], binary: bool) -> Vec<u8> {
    let mut buf: Vec<u8> = Vec::new();
    let (mut views, mut accessors, mut materials, mut gmeshes, mut nodes) = (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for (i, (name, m, c)) in meshes.iter().enumerate() {
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        for p in &m.positions {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        let mut push = |bytes: &[u8], target: u32| {
            while !buf.len().is_multiple_of(4) {
                buf.push(0);
            }
            let off = buf.len();
            buf.extend_from_slice(bytes);
            views.push(serde_json::json!({"buffer": 0, "byteOffset": off, "byteLength": bytes.len(), "target": target}));
            views.len() - 1
        };
        let pos: Vec<u8> = m.positions.iter().flatten().flat_map(|v| v.to_le_bytes()).collect();
        let nor: Vec<u8> = m.normals.iter().flatten().flat_map(|v| v.to_le_bytes()).collect();
        let idx: Vec<u8> = m.indices.iter().flat_map(|v| v.to_le_bytes()).collect();
        let (vp, vn, vi) = (push(&pos, 34962), push(&nor, 34962), push(&idx, 34963));
        let a0 = accessors.len();
        accessors.push(serde_json::json!({"bufferView": vp, "componentType": 5126, "count": m.positions.len(), "type": "VEC3", "min": lo, "max": hi}));
        accessors.push(serde_json::json!({"bufferView": vn, "componentType": 5126, "count": m.normals.len(), "type": "VEC3"}));
        accessors.push(serde_json::json!({"bufferView": vi, "componentType": 5125, "count": m.indices.len(), "type": "SCALAR"}));
        materials.push(serde_json::json!({"name": name, "pbrMetallicRoughness": {"baseColorFactor": c, "metallicFactor": 0.0, "roughnessFactor": 0.5}}));
        gmeshes.push(serde_json::json!({"name": name, "primitives": [{"attributes": {"POSITION": a0, "NORMAL": a0 + 1}, "indices": a0 + 2, "material": i}]}));
        nodes.push(serde_json::json!({"name": name, "mesh": i}));
    }
    while !buf.len().is_multiple_of(4) {
        buf.push(0);
    }
    let mut buffer = serde_json::json!({"byteLength": buf.len()});
    if !binary {
        buffer["uri"] = serde_json::Value::String(format!("data:application/octet-stream;base64,{}", base64(&buf)));
    }
    let doc = serde_json::json!({
        "asset": {"version": "2.0", "generator": "noshape"},
        "scene": 0,
        "scenes": [{"nodes": (0..nodes.len()).collect::<Vec<_>>()}],
        "nodes": nodes,
        "meshes": gmeshes,
        "materials": materials,
        "accessors": accessors,
        "bufferViews": views,
        "buffers": [buffer],
    });
    let mut json = serde_json::to_vec(&doc).unwrap_or_default();
    if !binary {
        return json;
    }
    while !json.len().is_multiple_of(4) {
        json.push(b' ');
    }
    let total = 12 + 8 + json.len() + 8 + buf.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(json.len() as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(&json);
    out.extend_from_slice(&(buf.len() as u32).to_le_bytes());
    out.extend_from_slice(b"BIN\0");
    out.extend_from_slice(&buf);
    out
}

/// 3MF: the core-spec package (a zip with the content types, the root
/// relationship and the model XML, in millimeters), stored uncompressed.
fn three_mf(meshes: &[(String, k::Tessellation, [f32; 4])]) -> Vec<u8> {
    use std::fmt::Write as _;
    let mut model = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<model unit=\"millimeter\" xml:lang=\"en-US\" xmlns=\"http://schemas.microsoft.com/3dmanufacturing/core/2015/02\">\n<resources>\n<basematerials id=\"1\">\n");
    for (name, _, c) in meshes {
        let srgb = |v: f32| (v.max(0.0).powf(1.0 / 2.2) * 255.0).round() as u8;
        let _ = writeln!(model, "<base name=\"{}\" displaycolor=\"#{:02X}{:02X}{:02X}\"/>", xml_escape(name), srgb(c[0]), srgb(c[1]), srgb(c[2]));
    }
    model.push_str("</basematerials>\n");
    for (i, (name, m, _)) in meshes.iter().enumerate() {
        let _ = writeln!(model, "<object id=\"{}\" name=\"{}\" type=\"model\" pid=\"1\" pindex=\"{i}\"><mesh><vertices>", i + 2, xml_escape(name));
        for p in &m.positions {
            let _ = writeln!(model, "<vertex x=\"{}\" y=\"{}\" z=\"{}\"/>", p[0] * 1000.0, p[1] * 1000.0, p[2] * 1000.0);
        }
        model.push_str("</vertices><triangles>\n");
        for t in m.indices.as_chunks::<3>().0 {
            let _ = writeln!(model, "<triangle v1=\"{}\" v2=\"{}\" v3=\"{}\"/>", t[0], t[1], t[2]);
        }
        model.push_str("</triangles></mesh></object>\n");
    }
    model.push_str("</resources>\n<build>\n");
    for i in 0..meshes.len() {
        let _ = writeln!(model, "<item objectid=\"{}\"/>", i + 2);
    }
    model.push_str("</build>\n</model>\n");
    let types = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"model\" ContentType=\"application/vnd.ms-package.3dmanufacturing-3dmodel+xml\"/></Types>";
    let rels = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Target=\"/3D/3dmodel.model\" Id=\"rel0\" Type=\"http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel\"/></Relationships>";
    stored_zip(&[("[Content_Types].xml", types.as_bytes()), ("_rels/.rels", rels.as_bytes()), ("3D/3dmodel.model", model.as_bytes())])
}

fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for (i, shift) in [18, 12, 6, 0].into_iter().enumerate() {
            out.push(if i <= c.len() { T[(n >> shift & 63) as usize] as char } else { '=' });
        }
    }
    out
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// A zip archive with the files stored (no compression).
fn stored_zip(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in files {
        let crc = crc32(data);
        let off = out.len() as u32;
        let header = |sig: u32, central_dir: bool| {
            let mut h = Vec::new();
            h.extend_from_slice(&sig.to_le_bytes());
            if central_dir {
                h.extend_from_slice(&20u16.to_le_bytes());
            }
            h.extend_from_slice(&20u16.to_le_bytes()); // version needed
            h.extend_from_slice(&0u16.to_le_bytes()); // flags
            h.extend_from_slice(&0u16.to_le_bytes()); // stored
            h.extend_from_slice(&0u32.to_le_bytes()); // time, date
            h.extend_from_slice(&crc.to_le_bytes());
            h.extend_from_slice(&(data.len() as u32).to_le_bytes());
            h.extend_from_slice(&(data.len() as u32).to_le_bytes());
            h.extend_from_slice(&(name.len() as u16).to_le_bytes());
            h.extend_from_slice(&0u16.to_le_bytes()); // extra
            if central_dir {
                h.extend_from_slice(&[0u8; 6]); // comment length, disk, internal attributes
                h.extend_from_slice(&0u32.to_le_bytes()); // external attributes
                h.extend_from_slice(&off.to_le_bytes());
            }
            h.extend_from_slice(name.as_bytes());
            h
        };
        out.extend(header(0x0403_4b50, false));
        out.extend_from_slice(data);
        central.extend(header(0x0201_4b50, true));
    }
    let cd_off = out.len() as u32;
    out.extend_from_slice(&central);
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&[0u8; 4]);
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&(central.len() as u32).to_le_bytes());
    out.extend_from_slice(&cd_off.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xedb8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    #[test]
    fn crc32_matches_reference() {
        assert_eq!(super::crc32(b"123456789"), 0xcbf4_3926);
    }

    #[test]
    fn base64_pads() {
        assert_eq!(super::base64(b"ab"), "YWI=");
        assert_eq!(super::base64(b"abc"), "YWJj");
        assert_eq!(super::base64(b"a"), "YQ==");
    }

    #[test]
    fn stored_zip_has_end_record() {
        let z = super::stored_zip(&[("a.txt", b"hello")]);
        assert_eq!(&z[..4], &[0x50, 0x4b, 0x03, 0x04]);
        assert_eq!(&z[z.len() - 22..z.len() - 18], &[0x50, 0x4b, 0x05, 0x06]);
        // Local header 30 + name + data, central entry 46 + name, end record 22.
        assert_eq!(z.len(), 30 + 5 + 5 + 46 + 5 + 22);
    }
}
