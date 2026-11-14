//! Part and assembly export (the Export modal's writers): every format of
//! Onshape's Export dialog, with its options. Shared by the app's Export
//! modal and the agent API.

use noshape_kernel as k;

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
    /// RHINO only in builds that have it ([`k::RHINO_EXPORT`]; releases do).
    pub fn list(assembly: bool) -> Vec<PartFormat> {
        if assembly {
            vec![Self::Parasolid, Self::Acis, Self::Step, Self::Iges, Self::Pvz, Self::Jt, Self::Gltf, Self::Glb, Self::Obj, Self::ThreeMf, Self::Urdf, Self::Stl]
        } else {
            Self::ALL.into_iter().filter(|f| *f != Self::Rhino || k::RHINO_EXPORT).collect()
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

    pub fn mesh(self) -> bool {
        matches!(self, Self::Gltf | Self::Glb | Self::Obj | Self::ThreeMf | Self::Stl)
    }
}

pub const PARASOLID_VERSIONS: [&str; 25] = ["38.1", "37.1", "37.0", "36.0", "35.1", "34.1", "34.0", "33.1", "33.0", "32.1", "32.0", "31.1", "31.0", "30.1", "30.0", "29.1", "29.0", "28.1", "28.0", "27.1", "27.0", "26.1", "26.0", "25.1", "25.0"];
pub const STEP_PREPROCESSING: [&str; 5] = ["None", "Automatic", "Remove small entities only (advanced)", "Remove small entities only (basic)", "Remove small entities only (advanced) and convert all surfaces to b-surfaces"];
pub const STEP_VERSIONS: [&str; 3] = ["AP242", "AP214", "AP203"];
pub const RHINO_VERSIONS: [&str; 7] = ["8", "7", "6", "5", "4", "3", "2"];
pub const RESOLUTIONS: [&str; 4] = ["Coarse", "Medium", "Fine", "Custom"];
pub const STL_FORMATS: [&str; 2] = ["Text", "Binary"];
/// Units and their size in meters.
pub const UNITS: [(&str, f64); 6] = [("Centimeter", 0.01), ("Foot", 0.3048), ("Inch", 0.0254), ("Meter", 1.0), ("Millimeter", 0.001), ("Yard", 0.9144)];
/// STEP's "Use custom units for export" list: [`UNITS`] without Yard (live qexport); ticking it starts on its first,
/// Centimeter.
pub const STEP_UNITS: usize = 5;

pub const OPTIONS: [&str; 3] = ["Download", "Download and store file in a new tab", "Store file in a new tab"];
/// URDF "Geometry format".
pub const URDF_GEOMETRY: [PartFormat; 4] = [PartFormat::Stl, PartFormat::Gltf, PartFormat::Glb, PartFormat::Obj];

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
    /// "Annotations text height (millimeter)" (live 1746: default 4, 0.0001 to 50).
    pub mbd_text_mm: f64,
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
    /// Options: 0 Download, 1 Download and store file in a new tab, 2 Store
    /// file in a new tab (see [`store_file_tab`]).
    pub options: usize,
    /// Resolution › Custom's fields (live dom g39-export-ps-step).
    pub custom: CustomResolution,
    /// Each part's MBD (index = part; [`studio_pmi`]), written to STEP AP242
    /// as semantic PMI.
    pub pmi: Vec<Option<k::PartPmi>>,
    /// The dialog's values of the formats not shown, given back on
    /// switching to them.
    pub memory: Box<SolidExportPrefs>,
    pub hidden_before_urdf: Option<bool>,
}

/// Resolution › Custom (live 1745): STL's Chordal tolerance · Angular
/// deviation (deg) · Minimum facet width, in the STL units (stored here in
/// mm); OBJ / 3MF / glTF / GLB's Distance tolerance (m) · Angular tolerance
/// (deg) · Maximum chord length (m). Our mesher takes the tolerance and the
/// angle; the facet width and chord length are kept but not applied.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CustomResolution {
    pub chordal_mm: f64,
    pub angular_deg: f64,
    pub min_facet_mm: f64,
    pub distance_m: f64,
    pub angular_tol_deg: f64,
    pub max_chord_m: f64,
}

/// Coarse, Medium and Fine (live 1745: the values Custom is pre-filled
/// with): chordal tolerance (mm), angular deviation (deg), minimum facet
/// width (mm); the other formats' distance tolerance is the same length, the
/// maximum chord length 10 m.
pub const PRESETS: [(f64, f64, f64); 3] = [(0.24, 12.5, 0.635), (0.12, 6.25, 0.254), (0.06, 2.5, 0.0254)];

impl CustomResolution {
    /// Custom pre-filled from a preset (index into [`PRESETS`]).
    pub fn preset(i: usize) -> Self {
        let (chordal, angle, facet) = PRESETS[i.min(2)];
        Self { chordal_mm: chordal, angular_deg: angle, min_facet_mm: facet, distance_m: chordal / 1000.0, angular_tol_deg: angle, max_chord_m: 10.0 }
    }
}

/// Live's first-time Custom: Medium's values.
impl Default for CustomResolution {
    fn default() -> Self {
        Self::preset(1)
    }
}

/// What the Export dialog remembers (live: the user setting
/// `exportSolidOptions`, one for part, Part Studio and Assembly exports,
/// saved on Export only): the format, the options every format shares, and
/// each format's own. glTF, GLB, OBJ, 3MF and URDF share one resolution;
/// STL keeps its own, with its units and encoding.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct SolidExportPrefs {
    pub format: String,
    pub y_up: bool,
    pub individual: bool,
    pub hidden: bool,
    pub options: usize,
    pub step: StepPrefs,
    pub parasolid: VersionPrefs,
    pub rhino: VersionPrefs,
    pub iges_compat: bool,
    pub pvz_group: bool,
    pub gltf_compress: bool,
    pub glb_compress: bool,
    pub stl_resolution: usize,
    pub stl_text: bool,
    pub stl_units: usize,
    pub mesh_resolution: usize,
    pub custom: CustomResolution,
    pub urdf_geometry: usize,
    pub urdf_stl_text: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct VersionPrefs {
    pub version: usize,
    pub latest: bool,
}

impl Default for VersionPrefs {
    fn default() -> Self {
        Self { version: 0, latest: true }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct StepPrefs {
    pub version: VersionPrefs,
    pub preprocessing: usize,
    pub custom_units: bool,
    pub units: usize,
    pub derived_tolerances: bool,
    pub mbd_text_height: bool,
    pub mbd_text_mm: f64,
}

impl Default for StepPrefs {
    fn default() -> Self {
        Self { version: VersionPrefs::default(), preprocessing: 0, custom_units: false, units: 0, derived_tolerances: false, mbd_text_height: false, mbd_text_mm: MBD_TEXT_MM }
    }
}

/// Live's first Export: Parasolid, Medium, STL in Meter and Binary.
impl Default for SolidExportPrefs {
    fn default() -> Self {
        Self {
            format: PartFormat::Parasolid.label().into(),
            y_up: false,
            individual: false,
            hidden: false,
            options: 0,
            step: StepPrefs::default(),
            parasolid: VersionPrefs::default(),
            rhino: VersionPrefs::default(),
            iges_compat: false,
            pvz_group: false,
            gltf_compress: false,
            glb_compress: false,
            stl_resolution: MEDIUM,
            stl_text: false,
            stl_units: METER,
            mesh_resolution: MEDIUM,
            custom: CustomResolution::default(),
            urdf_geometry: 0,
            urdf_stl_text: false,
        }
    }
}

impl SolidExportPrefs {
    fn path() -> std::path::PathBuf {
        noshape_scene::config_dir().join("export_solid.json")
    }

    /// The remembered options, else live's first-time ones.
    pub fn load() -> Self {
        std::fs::read(Self::path()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
    }

    pub fn save(&self) {
        let p = Self::path();
        if let Ok(t) = serde_json::to_vec_pretty(self) {
            let _ = std::fs::write(p, t);
        }
    }

    fn version(&self, f: PartFormat) -> VersionPrefs {
        match f {
            PartFormat::Step => self.step.version,
            PartFormat::Parasolid => self.parasolid,
            PartFormat::Rhino => self.rhino,
            _ => VersionPrefs::default(),
        }
    }

    /// Keeps `s`'s values: the shared ones and its format's own.
    pub fn keep(&mut self, s: &ExportSettings) {
        self.format = s.format.label().into();
        (self.y_up, self.individual, self.hidden, self.options) = (s.y_up, s.individual, s.hidden, s.options);
        self.custom = s.custom;
        let v = VersionPrefs { version: s.version, latest: s.latest };
        match s.format {
            PartFormat::Step => self.step.version = v,
            PartFormat::Parasolid => self.parasolid = v,
            PartFormat::Rhino => self.rhino = v,
            _ => {}
        }
        match s.format {
            PartFormat::Step => {
                let p = &mut self.step;
                (p.preprocessing, p.custom_units, p.units, p.derived_tolerances, p.mbd_text_height, p.mbd_text_mm) = (s.preprocessing, s.custom_units, s.units, s.derived_tolerances, s.mbd_text_height, s.mbd_text_mm);
            }
            PartFormat::Iges => self.iges_compat = s.iges_compat,
            PartFormat::Pvz => self.pvz_group = s.pvz_group,
            PartFormat::Gltf => (self.gltf_compress, self.mesh_resolution) = (s.compress, s.resolution),
            PartFormat::Glb => (self.glb_compress, self.mesh_resolution) = (s.compress, s.resolution),
            PartFormat::Obj | PartFormat::ThreeMf => self.mesh_resolution = s.resolution,
            PartFormat::Stl => (self.stl_resolution, self.stl_text, self.stl_units) = (s.resolution, s.stl_text, s.units),
            PartFormat::Urdf => (self.mesh_resolution, self.urdf_geometry, self.urdf_stl_text) = (s.resolution, s.urdf_geometry, s.stl_text),
            PartFormat::Parasolid | PartFormat::Rhino | PartFormat::Acis | PartFormat::Jt => {}
        }
    }

    /// Gives `s` the remembered shared values and those of `s.format`.
    pub fn give(&self, s: &mut ExportSettings) {
        (s.y_up, s.individual, s.hidden, s.options) = (self.y_up, self.individual, self.hidden, self.options.min(OPTIONS.len() - 1));
        s.custom = self.custom;
        let v = self.version(s.format);
        (s.version, s.latest) = (v.version, v.latest);
        let p = &self.step;
        (s.preprocessing, s.custom_units, s.derived_tolerances, s.mbd_text_height, s.mbd_text_mm) = (p.preprocessing.min(STEP_PREPROCESSING.len() - 1), p.custom_units, p.derived_tolerances, p.mbd_text_height, p.mbd_text_mm);
        (s.iges_compat, s.pvz_group, s.urdf_geometry) = (self.iges_compat, self.pvz_group, self.urdf_geometry.min(URDF_GEOMETRY.len() - 1));
        s.compress = if s.format == PartFormat::Glb { self.glb_compress } else { self.gltf_compress };
        let stl = s.format == PartFormat::Stl;
        s.resolution = if stl { self.stl_resolution } else { self.mesh_resolution }.min(CUSTOM_RESOLUTION);
        s.units = if stl { self.stl_units.min(UNITS.len() - 1) } else { p.units.min(STEP_UNITS - 1) };
        s.stl_text = if s.format == PartFormat::Urdf { self.urdf_stl_text } else { self.stl_text };
    }

    /// The Export dialog's settings on opening: the remembered format if
    /// the list has it (else its first) and options.
    pub fn settings(&self, assembly: bool) -> ExportSettings {
        let list = PartFormat::list(assembly);
        let mut s = ExportSettings { format: list.iter().copied().find(|f| f.label() == self.format).unwrap_or(list[0]), ..Default::default() };
        self.give(&mut s);
        s.memory = Box::new(self.clone());
        s
    }
}

/// The Export dialog's file name (live qexport): the tab's name, a single part's "<tab> - <part>", cut before any
/// "<" (an instance's "<2>").
pub fn export_base_name(tab: &str, part: Option<&str>) -> String {
    let name = match part {
        Some(p) => format!("{tab} - {p}"),
        None => tab.to_string(),
    };
    name.split('<').next().unwrap_or_default().trim().to_string()
}

/// A unit's short name in Custom's labels (live: "Chordal tolerance (in)").
pub fn unit_abbreviation(i: usize) -> &'static str {
    ["cm", "ft", "in", "m", "mm", "yd"][i.min(5)]
}

/// Index of "Custom" in [`RESOLUTIONS`].
pub const CUSTOM_RESOLUTION: usize = 3;
const MEDIUM: usize = 1;
/// Index of Meter in [`UNITS`].
const METER: usize = 3;

impl ExportSettings {
    /// The extension of the file the export saves (the save prompt's name
    /// and filter): a zip for OBJ (with its .mtl), URDF, and "Export unique
    /// parts as individual files" when there is more than one part.
    pub fn file_ext(&self, several_parts: bool) -> &'static str {
        if matches!(self.format, PartFormat::Obj | PartFormat::Urdf) || (self.individual && several_parts) { "zip" } else { self.format.ext() }
    }
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
            mbd_text_mm: MBD_TEXT_MM,
            iges_compat: false,
            pvz_group: false,
            resolution: MEDIUM,
            compress: false,
            stl_text: false,
            units: METER,
            individual: false,
            hidden: false,
            urdf_geometry: 0,
            options: 0,
            custom: CustomResolution::default(),
            pmi: Vec::new(),
            memory: Box::default(),
            hidden_before_urdf: None,
        }
    }
}

/// Write `parts` (meters, placed; linear RGBA): Y axis up, one file, or
/// with "Export unique parts as individual files" one file per part zipped
/// together (Onshape), or URDF. Returns the written file.
/// The formats a CAD file tab's Export… converts to (live document/94), those this build writes.
pub const FILE_FORMATS: [PartFormat; 10] = [PartFormat::Parasolid, PartFormat::Acis, PartFormat::Iges, PartFormat::Pvz, PartFormat::Jt, PartFormat::Rhino, PartFormat::Gltf, PartFormat::Glb, PartFormat::Obj, PartFormat::ThreeMf];

/// A file's name without its extension (a CAD file's export is named so: live document/94).
pub fn file_stem(file_name: &str) -> String {
    std::path::Path::new(file_name).file_stem().map(|x| x.to_string_lossy().to_string()).unwrap_or_else(|| file_name.to_string())
}

/// A CAD file tab's parts as its Export… writes them (read through a scratch copy under its own name: the readers go
/// by its extension).
pub fn cad_file_parts(f: &noshape_model::file_tab::FileTab, units: &noshape_units::UnitPrefs) -> anyhow::Result<Vec<(String, k::Shape, [f32; 4])>> {
    let bytes = f.bytes().ok_or_else(|| anyhow::anyhow!("the file's contents are missing"))?;
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!("noshape-file-export-{}-{}", std::process::id(), NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
    let path = dir.join(&f.file_name);
    std::fs::create_dir_all(&dir)?;
    std::fs::write(&path, &bytes)?;
    let parts = crate::import::file_parts(&path, units);
    let _ = std::fs::remove_dir_all(&dir);
    parts
}

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
            let one = ExportSettings { pmi: vec![s.pmi.get(i).cloned().flatten()], ..s.clone() };
            write_parts(&one, &t, std::slice::from_ref(p))?;
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
    // OBJ: a zip of `<name>.obj` and its `<name>.mtl`, as live downloads it.
    if s.format == PartFormat::Obj {
        let stem = path.file_stem().map(|x| x.to_string_lossy().to_string()).unwrap_or_else(|| "part".into());
        let (defl, ang, stl, min) = mesh_params(s);
        let meshes: Vec<(String, k::Tessellation, [f32; 4])> = parts.iter().map(|(n, sh, c)| Ok((n.clone(), export_mesh(sh, defl, ang, stl, min)?, *c))).collect::<anyhow::Result<_>>()?;
        let (obj, mtl) = obj_and_mtl(&meshes, &stem);
        let (on, mn) = (format!("{stem}.obj"), format!("{stem}.mtl"));
        let out = path.with_extension("zip");
        std::fs::write(&out, stored_zip(&[(on.as_str(), obj.as_bytes()), (mn.as_str(), mtl.as_bytes())]))?;
        return Ok(out);
    }
    write_parts(s, path, &parts)?;
    Ok(path.to_path_buf())
}

/// OBJ as live writes it (samples/l30-export/…-obj.zip): in metres, one
/// group per part with its welded vertices and shared normals, its
/// material, then one `o meshN` per face; the materials in `<stem>.mtl`,
/// named by their colour.
fn obj_and_mtl(meshes: &[(String, k::Tessellation, [f32; 4])], stem: &str) -> (String, String) {
    use std::fmt::Write as _;
    let mut obj = format!("# OBJ generated by Noshape {} Units = meters\nmtllib {stem}.mtl\n", k::VERSION);
    let mut mtl = String::new();
    let mut materials: Vec<String> = Vec::new();
    let (mut v0, mut n0, mut mesh_k) = (0u32, 0u32, 0usize);
    for (name, m, c) in meshes {
        let rgb = [0, 1, 2].map(|i| crate::appearance::lin_to_srgb(c[i]) as f32 / 255.0);
        let mat = format!("{:.6}_{:.6}_{:.6}_0.000000_0.000000", rgb[0], rgb[1], rgb[2]);
        if !materials.contains(&mat) {
            let _ = write!(mtl, "newmtl {mat}\nKa 0.0000 0.0000 0.0000\nKd {:.4} {:.4} {:.4}\nKs 0.0000 0.0000 0.0000\nTf 0.0000 0.0000 0.0000\nd 1\nNs 0\n", rgb[0], rgb[1], rgb[2]);
            materials.push(mat.clone());
        }
        let _ = writeln!(obj, "g {name}");
        let (positions, remap) = m.welded_positions();
        for p in &positions {
            let _ = writeln!(obj, "v {} {} {} ", p[0], p[1], p[2]);
        }
        // Normals once each (smooth faces share theirs).
        let mut normal_at: std::collections::HashMap<[u32; 3], u32> = std::collections::HashMap::new();
        let mut normals: Vec<[f32; 3]> = Vec::new();
        let nmap: Vec<u32> = m
            .normals
            .iter()
            .map(|n| {
                *normal_at.entry(n.map(|c| (c + 0.0).to_bits())).or_insert_with(|| {
                    normals.push(*n);
                    normals.len() as u32 - 1
                })
            })
            .collect();
        for n in &normals {
            let _ = writeln!(obj, "vn {} {} {} ", n[0], n[1], n[2]);
        }
        let _ = writeln!(obj, "usemtl {mat}");
        for fi in 0..m.faces.len() {
            if m.faces[fi].index_count == 0 {
                continue;
            }
            let _ = writeln!(obj, "o mesh{mesh_k}");
            mesh_k += 1;
            for t in m.face_indices(fi).as_chunks::<3>().0 {
                let [a, b, cc] = t.map(|i| remap[i as usize] + 1 + v0);
                if a == b || b == cc || a == cc {
                    continue;
                }
                let [na, nb, nc] = t.map(|i| nmap[i as usize] + 1 + n0);
                let _ = writeln!(obj, "f {a}//{na} {b}//{nb} {cc}//{nc} ");
            }
        }
        v0 += positions.len() as u32;
        n0 += normals.len() as u32;
    }
    (obj, mtl)
}

/// An Assembly to export: the root's name, each distinct part once in its
/// own coordinates (meters; linear RGBA), and the instances placing them.
pub struct AssemblyExport {
    pub root: String,
    pub parts: Vec<(String, k::Shape, [f32; 4])>,
    /// (part index, instance name, placement in meters).
    pub instances: Vec<(usize, String, glam::DMat4)>,
}

impl AssemblyExport {
    /// The solved assembly's part occurrences (`ids`: those instances, None:
    /// all; hidden ones only with "Include hidden instances in export").
    pub fn of(root: &str, result: &noshape_model::assembly::eval::AssemblyResult, ids: Option<&[noshape_model::assembly::InstanceId]>, include_hidden: bool) -> Self {
        let paths: Option<Vec<Vec<noshape_model::assembly::InstanceId>>> = ids.map(|ids| ids.iter().map(|i| vec![*i]).collect());
        Self::of_occurrences(root, result, paths.as_deref(), include_hidden)
    }

    /// [`Self::of`] for occurrences given by path (a part inside a
    /// subassembly: its path from the top-level instance).
    pub fn of_occurrences(root: &str, result: &noshape_model::assembly::eval::AssemblyResult, paths: Option<&[Vec<noshape_model::assembly::InstanceId>]>, include_hidden: bool) -> Self {
        let mut keys: Vec<(noshape_model::document::TabId, noshape_model::BodyId, noshape_model::configuration::ConfigValues)> = Vec::new();
        let mut out = AssemblyExport { root: root.to_string(), parts: Vec::new(), instances: Vec::new() };
        for o in result.occurrences.iter().filter(|o| (include_hidden || !o.hidden) && paths.is_none_or(|ps| ps.iter().any(|p| o.path.starts_with(p)))) {
            let key = (o.studio, o.body.clone(), o.config.clone());
            let part = match keys.iter().position(|k| *k == key) {
                Some(i) => i,
                None => {
                    let Some(b) = o.context.bodies.iter().find(|b| b.id == o.body) else { continue };
                    let c = o.color;
                    out.parts.push((o.body_name.clone(), b.shape.clone(), [c[0], c[1], c[2], 1.0]));
                    keys.push(key);
                    keys.len() - 1
                }
            };
            out.instances.push((part, o.instance_name.clone(), o.transform));
        }
        out
    }

    /// Each instance as a placed part, named after it (formats without
    /// assemblies).
    pub fn placed(&self) -> anyhow::Result<Vec<(String, k::Shape, [f32; 4])>> {
        self.instances.iter().map(|(p, name, t)| Ok((name.clone(), self.parts[*p].1.transformed(&k::Transform::Matrix(*t))?, self.parts[*p].2))).collect()
    }
}

/// Write an Assembly's export: STEP, Parasolid and IGES keep the assembly
/// (Onshape: each part once, placed by its instances); the other formats
/// and individual files get the placed parts.
pub fn write_assembly_export(s: &ExportSettings, path: &std::path::Path, a: &AssemblyExport) -> anyhow::Result<std::path::PathBuf> {
    anyhow::ensure!(!a.instances.is_empty(), "There are no parts to export");
    // Y axis up: the model's +Z becomes +Y.
    let up = if s.y_up { glam::DMat4::from_rotation_x(-std::f64::consts::FRAC_PI_2) } else { glam::DMat4::IDENTITY };
    let instances: Vec<k::StepInstance> = a.instances.iter().map(|(p, _, t)| k::StepInstance { part: *p, transform: up * *t }).collect();
    // Parasolid keeps the assembly too (live: its x_t re-imports as an
    // Assembly; qa ASM-PARASOLID-FLAT), in every version.
    if s.format == PartFormat::Parasolid && !s.individual {
        k::write_parasolid_assembly(path, &a.root, &a.parts.iter().map(|(n, sh, c)| step_shape(n, sh, c)).collect::<Vec<_>>(), &instances, k::LengthUnit::Meter, parasolid_version(s))?;
        return Ok(path.to_path_buf());
    }
    // IGES as subfigure instances (live 1755: re-imports as an Assembly).
    if s.format == PartFormat::Iges && !s.individual {
        k::write_iges_assembly(path, &a.parts.iter().map(|(n, sh, c)| step_shape(n, sh, c)).collect::<Vec<_>>(), &instances, k::LengthUnit::Meter)?;
        return Ok(path.to_path_buf());
    }
    if s.format != PartFormat::Step || s.individual {
        return write_export(s, path, a.placed()?);
    }
    let shapes: Vec<k::ExportShape> = a.parts.iter().map(|(n, sh, c)| step_shape(n, sh, c)).collect();
    k::write_step_assembly(path, &a.root, &shapes, &instances, k::LengthUnit::Meter, &step_options(s))?;
    Ok(path.to_path_buf())
}

/// The dialog's Parasolid version (live G36: "Use latest version" = 38.1).
fn parasolid_version(s: &ExportSettings) -> &'static str {
    if s.latest { PARASOLID_VERSIONS[0] } else { PARASOLID_VERSIONS.get(s.version).copied().unwrap_or(PARASOLID_VERSIONS[0]) }
}

/// A part as the STEP writer takes it: Onshape writes the colour's 8-bit sRGB values.
fn step_shape(n: &str, sh: &k::Shape, c: &[f32; 4]) -> k::ExportShape {
    let srgb = |v: f32| crate::appearance::lin_to_srgb(v) as f32 / 255.0;
    k::ExportShape { shape: sh.clone(), name: Some(n.to_string()), color: Some([srgb(c[0]), srgb(c[1]), srgb(c[2])]) }
}

/// The dialog's STEP version and units.
fn step_options(s: &ExportSettings) -> k::StepOptions {
    let schema = if s.latest { k::StepSchema::Ap242 } else { [k::StepSchema::Ap242, k::StepSchema::Ap214, k::StepSchema::Ap203].get(s.version).copied().unwrap_or(k::StepSchema::Ap242) };
    let file_unit = if s.custom_units {
        match UNITS[s.units].0 {
            "Centimeter" => Some(k::LengthUnit::Centimeter),
            "Foot" => Some(k::LengthUnit::Foot),
            "Inch" => Some(k::LengthUnit::Inch),
            "Meter" => Some(k::LengthUnit::Meter),
            "Yard" => Some(k::LengthUnit::Yard),
            _ => Some(k::LengthUnit::Millimeter),
        }
    } else {
        None
    };
    k::StepOptions { schema, file_unit, pmi: s.pmi.clone() }
}

/// The Part Studio Format list's row `i` ([`PartFormat::list`]).
/// Format: keeps the shown format's values, shows the new one's.
fn switch_format(s: &mut ExportSettings, f: PartFormat) {
    let mut memory = std::mem::take(&mut s.memory);
    memory.keep(s);
    s.format = f;
    memory.give(s);
    s.memory = memory;
}

pub fn set_part_format(s: &mut ExportSettings, i: usize) {
    switch_format(s, PartFormat::list(false)[i]);
}

/// URDF checks "Include hidden instances in export"; leaving it gives back
/// the value it had before (live onFormatChanged).
pub fn set_asm_format(s: &mut ExportSettings, i: usize) {
    switch_format(s, PartFormat::list(true)[i]);
    if let Some(h) = s.hidden_before_urdf.take() {
        s.hidden = h;
    }
    if s.format == PartFormat::Urdf {
        s.hidden_before_urdf = Some(s.hidden);
        s.hidden = true;
    }
}

/// The formats whose writer is still to come (kern): Export shows Onshape's
/// flow, then this error.
pub fn missing_writer(f: PartFormat) -> Option<String> {
    let _ = f; // every format has its writer now (kern)
    None
}

fn sanitize(name: &str) -> String {
    name.chars().map(|c| if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') { '_' } else { c }).collect()
}

/// The mesher's linear (meters) and angular (radians) deflection, the preset's or Custom's (STL's chordal tolerance /
/// angular deviation, the others' distance / angular tolerance), whether it is an STL's, and an STL's shortest edge
/// segment (meters, its Minimum facet width; 0 for the others).
/// An STL's lengths as Onshape's Export dialog facets them (live 2994, 3052), whatever the file's unit (the same mesh,
/// scaled): Medium's 0.12 mm chord and 6.25° cut clamp-hms' R5 rim in 58 segments, ceil(sweep / angle) on every
/// circle and torus where the chord doesn't bind. As BRepMesh's deflection twice the chord (half Parasolid's) and its
/// angle twice the segments' turn; the minimum facet width floors the segments (0.7-0.85 × it where chord or angle
/// would go finer).
fn mesh_params(s: &ExportSettings) -> (f64, f64, bool, f64) {
    let c = if s.resolution == CUSTOM_RESOLUTION { s.custom } else { CustomResolution::preset(s.resolution) };
    let stl = s.format == PartFormat::Stl || (s.format == PartFormat::Urdf && URDF_GEOMETRY[s.urdf_geometry.min(3)] == PartFormat::Stl);
    let (lin, ang, min) = if stl {
        (c.chordal_mm * 2e-3, 2.0 * c.angular_deg, 0.7 * c.min_facet_mm * 1e-3)
    } else {
        (c.distance_m.max(1e-7), c.angular_tol_deg, 0.0)
    };
    (lin.max(1e-10), ang.to_radians().clamp(1e-3, 1.5), stl, min)
}

/// A part's export mesh (STL, OBJ, 3MF…): a closed surface, consistently wound; an STL's at Onshape's facet
/// tolerances ([`k::tessellate_export`]).
/// An STL's chord no longer than a twentieth of the part's box: a chord past the part's size limits nothing (its
/// segments follow the angle), and BRepMesh's mesh broke there (motor-mount-jack at a 100 mm chord and Medium's
/// minimum facet width: a hole filled, +41 % volume).
fn export_mesh(sh: &k::Shape, defl: f64, ang: f64, stl: bool, min: f64) -> anyhow::Result<k::Tessellation> {
    if !stl {
        return Ok(k::tessellate_closed(sh, defl, ang)?);
    }
    let size = sh.bounding_box(false).map_or(f64::INFINITY, |b| b.max.distance(b.min));
    Ok(k::tessellate_export(sh, defl.min(0.05 * size).max(1e-10), ang, min)?)
}

fn write_parts(s: &ExportSettings, path: &std::path::Path, parts: &[(String, k::Shape, [f32; 4])]) -> anyhow::Result<()> {
    match s.format {
        PartFormat::Step => {
            let shapes: Vec<k::ExportShape> = parts.iter().map(|(n, sh, c)| step_shape(n, sh, c)).collect();
            k::write_step_with(path, &shapes, k::LengthUnit::Meter, &step_options(s))?;
        }
        PartFormat::Iges => k::write_iges_parts(path, &parts.iter().map(|(n, sh, c)| step_shape(n, sh, c)).collect::<Vec<_>>(), k::LengthUnit::Meter)?,
        // Text transmit file in the chosen version (live G36: the body in that
        // version's layouts; "Use latest version" = 38.1).
        // Named and coloured as Onshape's (TYSA_NAME / TYSA_COLOUR_2).
        PartFormat::Parasolid => k::write_parasolid_parts(path, &parts.iter().map(|(n, sh, c)| step_shape(n, sh, c)).collect::<Vec<_>>(), k::LengthUnit::Meter, false, parasolid_version(s))?,
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
            let (defl, ang, stl, min) = mesh_params(s);
            let meshes: Vec<(String, k::Tessellation, [f32; 4])> = parts.iter().map(|(n, sh, c)| Ok((n.clone(), export_mesh(sh, defl, ang, stl, min)?, *c))).collect::<anyhow::Result<_>>()?;
            match f {
                PartFormat::Stl | PartFormat::Obj => {
                    // STL: in the chosen unit; OBJ: meters.
                    let scale = if f == PartFormat::Stl { 1.0 / UNITS[s.units].1 } else { 1.0 } as f32;
                    let mut all = k::Tessellation::default();
                    for (_, m, _) in &meshes {
                        let base = all.positions.len() as u32;
                        let first = all.indices.len() as u32;
                        all.faces.extend(m.faces.iter().map(|r| k::FaceRange { first_index: r.first_index + first, index_count: r.index_count, first_vertex: r.first_vertex + base, vertex_count: r.vertex_count }));
                        all.positions.extend(m.positions.iter().map(|p| [p[0] * scale, p[1] * scale, p[2] * scale]));
                        all.normals.extend_from_slice(&m.normals);
                        all.indices.extend(m.indices.iter().map(|i| i + base));
                    }
                    if f == PartFormat::Stl {
                        k::write_stl_mesh(&all, path, if s.stl_text { k::StlFormat::Ascii } else { k::StlFormat::Binary })?;
                    } else {
                        let stem = path.file_stem().map(|x| x.to_string_lossy().to_string()).unwrap_or_else(|| "part".into());
                        std::fs::write(path, obj_and_mtl(&meshes, &stem).0)?;
                    }
                }
                PartFormat::Gltf | PartFormat::Glb => std::fs::write(path, gltf(&meshes, f == PartFormat::Glb))?,
                _ => std::fs::write(path, three_mf(&meshes))?,
            }
        }
        f => anyhow::bail!("{} export is not available in Noshape yet", f.label()),
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
        "asset": {"version": "2.0", "generator": "Noshape"},
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
    // As live's (samples/l30-export/…-live.3mf): metres, the parts'
    // colours in a material colour group.
    let mut model = String::from("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<model xmlns=\"http://schemas.microsoft.com/3dmanufacturing/core/2015/02\" unit=\"meter\" xml:lang=\"en-US\" xmlns:m=\"http://schemas.microsoft.com/3dmanufacturing/material/2015/02\">\n<resources>\n<m:colorgroup id=\"1\">\n");
    for (_, _, c) in meshes {
        let srgb = |v: f32| crate::appearance::lin_to_srgb(v);
        let _ = writeln!(model, "<m:color color=\"#{:02X}{:02X}{:02X}FF\"/>", srgb(c[0]), srgb(c[1]), srgb(c[2]));
    }
    model.push_str("</m:colorgroup>\n");
    for (i, (name, m, _)) in meshes.iter().enumerate() {
        let _ = writeln!(model, "<object id=\"{}\" name=\"{}\" type=\"model\" pid=\"1\" pindex=\"{i}\"><mesh><vertices>", i + 2, xml_escape(name));
        // 3MF Core: a manifold mesh by index (each edge in two triangles):
        // the faces' vertices welded.
        let (positions, remap) = m.welded_positions();
        for p in &positions {
            let _ = writeln!(model, "<vertex x=\"{:.8}\" y=\"{:.8}\" z=\"{:.8}\"/>", p[0], p[1], p[2]);
        }
        model.push_str("</vertices><triangles>\n");
        for t in m.welded_triangles(&remap) {
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

/// A zip archive with the files stored (no compression), dated now (UTC).
fn stored_zip(files: &[(&str, &[u8])]) -> Vec<u8> {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    stored_zip_at(files, dos_time(secs))
}

/// MS-DOS (time, date) of a Unix time: 2-second steps, years from 1980.
fn dos_time(unix: u64) -> (u16, u16) {
    let (days, rem) = ((unix / 86_400) as i64, unix % 86_400);
    // Civil date from days since 1970-01-01 (H. Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    if y < 1980 {
        return (0, (1 << 5) | 1);
    }
    let time = (((rem / 3600) << 11) | ((rem % 3600 / 60) << 5) | ((rem % 60) / 2)) as u16;
    let date = (((y - 1980).min(127) << 9) | (m << 5) | d) as u16;
    (time, date)
}

fn stored_zip_at(files: &[(&str, &[u8])], (time, date): (u16, u16)) -> Vec<u8> {
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
            h.extend_from_slice(&time.to_le_bytes());
            h.extend_from_slice(&date.to_le_bytes());
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

/// Export Options › "Store file in a new tab" (and "Download and store"):
/// the exported file becomes a file tab directly after the exported tab
/// (live: "Part 1 Drawing 1.dwt" right after the drawing; the exported tab
/// stays active). One edit, [`store_description`].
pub fn store_file_tab(d: &mut noshape_model::document::Document, file_name: &str, bytes: &[u8], after: noshape_model::document::TabId) -> noshape_model::document::TabId {
    crate::import::add_file_tab(d, noshape_model::file_tab::FileTab::new(file_name, bytes, noshape_model::file_tab::now()), None, Some(after))
}

/// The history entry of [`store_file_tab`]: "Store <file>".
pub fn store_description(file_name: &str) -> String {
    format!("Store {file_name}")
}

/// MBD text height in STEP (live 1746: Onshape's default, 4 mm).
pub const MBD_TEXT_MM: f64 = 4.0;

impl ExportSettings {
    /// The MBD text height written to STEP (mm).
    pub fn mbd_text(&self) -> f64 {
        if self.mbd_text_height { self.mbd_text_mm } else { MBD_TEXT_MM }
    }
}

/// The MBD of a Part Studio's `bodies` for STEP AP242 (Model definitions,
/// live 1746): datums on their faces and dimensions whose references are
/// faces of the same body, valued from the model (`mbd::measure`), with
/// their Symmetrical / Deviation / Limits tolerances (typed in the
/// document's unit), each drawn in its plane with `text_mm` high text; one
/// entry per body, None without any.
pub fn studio_pmi(ps: &noshape_model::PartStudio, ctx: &noshape_model::regen::context::Context, bodies: &[noshape_model::BodyId], units: &noshape_units::UnitPrefs, text_mm: f64) -> Vec<Option<k::PartPmi>> {
    use noshape_model::annotations::{AnnotationKind, Measure};
    use noshape_model::drawing::mbd::MbdShape;
    use noshape_model::Reference;
    bodies
        .iter()
        .map(|id| {
            let body = ctx.bodies.iter().find(|b| &b.id == id)?;
            let faces = |r: &Reference| -> Vec<u32> {
                match r {
                    Reference::Face(n) => body.find_faces(n).into_iter().map(|i| i as u32 + 1).collect(),
                    _ => Vec::new(),
                }
            };
            let shapes = body.shape.faces();
            let cylinder = |f: &[u32]| f.first().and_then(|&i| shapes.get(i as usize - 1)).and_then(|s| s.face_surface().ok()).is_some_and(|s| matches!(s, k::Surface::Cylinder { .. }));
            let mut pmi = k::PartPmi::default();
            for a in &ps.annotations {
                let frame = Frame::new(a.at, a.normal, a.up);
                match &a.kind {
                    AnnotationKind::Datum { letter } => {
                        let f = faces(&a.face);
                        if !f.is_empty() {
                            pmi.datums.push(k::PmiDatum { faces: f, letter: letter.clone(), presentation: Some(datum_look(a, letter, text_mm)) });
                        }
                    }
                    AnnotationKind::Dimension { measure, refs, tolerance } => {
                        let fa = refs.first().map(&faces).unwrap_or_default();
                        let fb = refs.get(1).map(&faces).unwrap_or_default();
                        let kind = match measure {
                            Measure::Angle => k::PmiKind::Angle,
                            Measure::Radius => k::PmiKind::Radius,
                            _ if cylinder(&fa) && cylinder(&fb) => k::PmiKind::AxisDistance,
                            _ => k::PmiKind::Distance,
                        };
                        if fa.is_empty() || (kind != k::PmiKind::Radius && fb.is_empty()) {
                            continue;
                        }
                        let Some((shape, value)) = noshape_model::drawing::mbd::measure(ctx, a) else { continue };
                        let angle = kind == k::PmiKind::Angle;
                        // Deviations typed in the document unit (degrees for angles).
                        let unit = if angle { std::f64::consts::PI / 180.0 } else { units.length.meters() };
                        let dim = if angle { noshape_units::Dim::ANGLE } else { noshape_units::Dim::LENGTH };
                        let dev = |s: &str| s.trim().parse::<f64>().map(|v| v * unit).unwrap_or_else(|_| noshape_units::evaluate_as(s, dim, units, &noshape_units::EmptyScope).map(|q| q.value).unwrap_or(0.0));
                        let (u, l) = (dev(&tolerance.upper), dev(&tolerance.lower));
                        let tol = match tolerance.kind.as_str() {
                            "Symmetrical" => Some((-u, u)),
                            "Deviation" | "Limits" => Some((-l, u)),
                            _ => None,
                        };
                        let prefix = match kind {
                            k::PmiKind::Angle => "ANGLE",
                            k::PmiKind::Radius => "RADIUS",
                            k::PmiKind::Diameter => "DIAMETER",
                            k::PmiKind::AxisDistance => "AXIS_DISTANCE",
                            k::PmiKind::Distance => "DISTANCE",
                        };
                        let text = dimension_text(kind, value, tol);
                        let look = match shape {
                            MbdShape::Linear { a: p, b: q } => frame.linear(p, q, &text, text_mm),
                            MbdShape::Radius { center, radius, .. } => frame.radial(center, radius, &text, text_mm),
                            MbdShape::Angle { vertex, a: p, b: q } => frame.angular(vertex, p, q, &text, text_mm),
                        };
                        pmi.dims.push(k::PmiDimension { kind, a: fa, b: fb, value, tolerance: tol, name: format!("{prefix}.noshape{}", a.id), precision: 3, presentation: Some(look) });
                    }
                }
            }
            (pmi != k::PartPmi::default()).then_some(pmi)
        })
        .collect()
}

/// A dimension's text as the presentation shows it: "30±0.1", "R3",
/// "90°", "+0.1/-0.05" deviations.
fn dimension_text(kind: k::PmiKind, value: f64, tol: Option<(f64, f64)>) -> String {
    let angle = kind == k::PmiKind::Angle;
    let shown = |v: f64| {
        let v = if angle { v.to_degrees() } else { v * 1000.0 };
        let t = format!("{v:.3}");
        let t = t.trim_end_matches('0').trim_end_matches('.').to_string();
        if t == "-0" { "0".into() } else { t }
    };
    let deg = if angle { "°" } else { "" };
    let mut t = format!("{}{}{deg}", if kind == k::PmiKind::Radius { "R" } else { "" }, shown(value));
    match tol {
        Some((l, u)) if (l + u).abs() < 1e-12 => t.push_str(&format!("±{}{deg}", shown(u))),
        Some((l, u)) => t.push_str(&format!(" +{}{deg}/{}{deg}", shown(u), shown(l))),
        None => {}
    }
    t
}

/// An annotation's plane (live 1746: through its text position, its normal
/// and in-plane up): 2D points in mm map to 3D metres.
struct Frame {
    origin: glam::DVec3,
    x: glam::DVec3,
    y: glam::DVec3,
    z: glam::DVec3,
}

/// Arrowheads as live draws them: 3 mm sides at 45°, 2.12 mm long and 1.41 mm wide.
const ARROW: (f64, f64) = (3.0 * std::f64::consts::FRAC_1_SQRT_2, std::f64::consts::FRAC_1_SQRT_2);

impl Frame {
    fn new(at: [f64; 3], normal: [f64; 3], up: [f64; 3]) -> Self {
        let z = glam::DVec3::from_array(normal).try_normalize().unwrap_or(glam::DVec3::Z);
        let up = glam::DVec3::from_array(up);
        let y = (up - z * up.dot(z)).try_normalize().unwrap_or_else(|| z.any_orthonormal_vector());
        Self { origin: glam::DVec3::from_array(at), x: y.cross(z), y, z }
    }
    /// A point's 2D coordinates (mm) in the plane.
    fn uv(&self, p: glam::DVec3) -> glam::DVec2 {
        let d = (p - self.origin) * 1000.0;
        glam::DVec2::new(d.dot(self.x), d.dot(self.y))
    }
    fn at(&self, p: glam::DVec2) -> [f64; 3] {
        (self.origin + (self.x * p.x + self.y * p.y) / 1000.0).to_array()
    }
    fn look(&self, label: &str, g: Look) -> k::PmiPresentation {
        k::PmiPresentation {
            label: label.into(),
            origin: self.origin.to_array(),
            normal: self.z.to_array(),
            x: self.x.to_array(),
            triangles: g.tris.iter().flatten().map(|p| self.at(*p)).collect(),
            lines: g.lines.iter().flatten().map(|p| self.at(*p)).collect(),
        }
    }
    /// A linear dimension: extension lines from the measured points to a
    /// dimension line through the text, arrows at its ends, broken for the
    /// text.
    fn linear(&self, p: glam::DVec3, q: glam::DVec3, text: &str, h: f64) -> k::PmiPresentation {
        let (p, q) = (self.uv(p), self.uv(q));
        let d = (q - p).try_normalize().unwrap_or(glam::DVec2::X);
        let n = d.perp();
        let (pa, qa) = (p + n * -p.dot(n), q + n * -q.dot(n));
        let mut g = Look::default();
        g.lines.push([p, pa]);
        g.lines.push([q, qa]);
        // The dimension line (through the text, along `d`), broken for it.
        let half = 0.5 * noshape_model::drawing::font::text_width(text, h) + 0.5 * h;
        let (s, e) = (pa.dot(d), qa.dot(d));
        let (lo, hi) = (s.min(e), s.max(e));
        if -half > lo {
            g.lines.push([d * lo, d * -half]);
        }
        if half < hi {
            g.lines.push([d * half, d * hi]);
        }
        // Outward at both ends (live: tips on the extension lines).
        g.arrow(pa, (pa - qa).try_normalize().unwrap_or(-d));
        g.arrow(qa, (qa - pa).try_normalize().unwrap_or(d));
        g.text(text, glam::DVec2::ZERO, upright(d), h);
        self.look("linear dimension", g)
    }
    /// A radius: a leader from the arc to the text, its arrow on the arc.
    fn radial(&self, center: glam::DVec3, r: f64, text: &str, h: f64) -> k::PmiPresentation {
        let c = self.uv(center);
        let dir = (-c).try_normalize().unwrap_or(glam::DVec2::X);
        let on_arc = c + dir * (r * 1000.0);
        let half = 0.5 * noshape_model::drawing::font::text_width(text, h) + 0.5 * h;
        let end = -dir * half;
        let mut g = Look::default();
        g.lines.push([on_arc, end]);
        g.arrow(on_arc, (on_arc - end).try_normalize().unwrap_or(-dir));
        g.text(text, glam::DVec2::ZERO, upright(dir), h);
        self.look("radial dimension", g)
    }
    /// An angle: an arc through the text between the legs, arrows at its ends.
    fn angular(&self, vertex: glam::DVec3, a: glam::DVec3, b: glam::DVec3, text: &str, h: f64) -> k::PmiPresentation {
        let v = self.uv(vertex);
        let (a, b) = (self.uv(a) - v, self.uv(b) - v);
        let r = (-v).length().max(8.0);
        let (t0, mut t1) = (a.y.atan2(a.x), b.y.atan2(b.x));
        if t1 < t0 {
            t1 += std::f64::consts::TAU;
        }
        if t1 - t0 > std::f64::consts::PI {
            t1 -= std::f64::consts::TAU;
        }
        let steps = 24;
        let pt = |t: f64| v + glam::DVec2::new(t.cos(), t.sin()) * r;
        let mut g = Look::default();
        for k in 0..steps {
            let (s0, s1) = (t0 + (t1 - t0) * k as f64 / steps as f64, t0 + (t1 - t0) * (k + 1) as f64 / steps as f64);
            g.lines.push([pt(s0), pt(s1)]);
        }
        let sense = (t1 - t0).signum();
        let tangent = |t: f64| glam::DVec2::new(-t.sin(), t.cos()) * sense;
        g.arrow(pt(t0), -tangent(t0));
        g.arrow(pt(t1), tangent(t1));
        let mid = (t0 + t1) * 0.5;
        g.text(text, pt(mid) + glam::DVec2::new(mid.cos(), mid.sin()) * h, upright(tangent(mid)), h);
        self.look("angular dimension", g)
    }
}

/// A datum's symbol (live 1746 "datum"): a filled triangle on the face, a
/// stem out along its normal to a box with the letter.
fn datum_look(a: &noshape_model::annotations::Annotation, letter: &str, h: f64) -> k::PmiPresentation {
    let n = glam::DVec3::from_array(a.normal);
    let up = glam::DVec3::from_array(a.up);
    // The symbol's plane holds the face normal: its "up" is the normal.
    let side = up.cross(n).try_normalize().unwrap_or_else(|| n.any_orthonormal_vector());
    let frame = Frame::new(a.at, side.cross(n).to_array(), n.to_array());
    let mut g = Look::default();
    let (w, t) = (1.5 * h / 4.0 * 2.0, 2.6 * h / 4.0);
    g.tris.push([glam::DVec2::new(-w / 2.0, 0.0), glam::DVec2::new(w / 2.0, 0.0), glam::DVec2::new(0.0, t)]);
    let (stem, size) = (2.0 * h, 1.6 * h);
    g.lines.push([glam::DVec2::new(0.0, t), glam::DVec2::new(0.0, t + stem)]);
    let (x0, x1, y0, y1) = (-size / 2.0, size / 2.0, t + stem, t + stem + size);
    for [p, q] in [[(x0, y0), (x1, y0)], [(x1, y0), (x1, y1)], [(x1, y1), (x0, y1)], [(x0, y1), (x0, y0)]] {
        g.lines.push([glam::DVec2::new(p.0, p.1), glam::DVec2::new(q.0, q.1)]);
    }
    g.text(letter, glam::DVec2::new(0.0, (y0 + y1) / 2.0), 0.0, h);
    frame.look("datum", g)
}

/// Text along `d` reads left to right or bottom to top.
fn upright(d: glam::DVec2) -> f64 {
    let a = d.y.atan2(d.x);
    if a > std::f64::consts::FRAC_PI_2 + 1e-9 || a <= -std::f64::consts::FRAC_PI_2 + 1e-9 { a + std::f64::consts::PI } else { a }
}

/// An annotation's drawing in its plane (mm).
#[derive(Default)]
struct Look {
    tris: Vec<[glam::DVec2; 3]>,
    lines: Vec<[glam::DVec2; 2]>,
}

impl Look {
    /// A filled arrowhead with its tip at `tip`, pointing along `dir`.
    fn arrow(&mut self, tip: glam::DVec2, dir: glam::DVec2) {
        let back = tip - dir * ARROW.0;
        let n = dir.perp() * ARROW.1;
        self.tris.push([back + n, tip, back - n]);
    }
    /// Text centred on `at`, `h` mm high, as filled glyphs (the drawing
    /// font, as the drawing's PDF outlines it).
    fn text(&mut self, text: &str, at: glam::DVec2, angle: f64, h: f64) {
        use noshape_model::drawing::sheet::{Anchor, Text2};
        let t = Text2 { pos: at.to_array(), height: h, text: text.into(), angle, anchor: Anchor::Center, pick: 0 };
        let (pts, idx) = noshape_model::drawing::font::text_fill_font(&t, None);
        for tri in idx.chunks(3).filter(|c| c.len() == 3) {
            self.tris.push([0, 1, 2].map(|j| glam::DVec2::from_array(pts[tri[j] as usize])));
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn crc32_matches_reference() {
        assert_eq!(super::crc32(b"123456789"), 0xcbf4_3926);
    }

    /// Live qexport: the Export dialog's first-time values and its file name.
    #[test]
    fn export_dialog_first_time_values_and_name() {
        use super::*;
        let p = SolidExportPrefs::default();
        let s = p.settings(false);
        assert_eq!(s.format, PartFormat::Parasolid);
        assert!(s.latest && !s.y_up && !s.custom_units);
        let mut step = ExportSettings { format: PartFormat::Step, ..Default::default() };
        p.give(&mut step);
        assert_eq!(UNITS[step.units].0, "Centimeter", "ticking Use custom units starts on the list's first");
        assert!(!UNITS[..STEP_UNITS].iter().any(|u| u.0 == "Yard"));
        let mut stl = ExportSettings { format: PartFormat::Stl, ..Default::default() };
        p.give(&mut stl);
        assert_eq!((UNITS[stl.units].0, stl.stl_text, stl.resolution), ("Meter", false, 1));
        // Presets: chordal, angular, STL facet width (0.025 / 0.01 / 0.001 in), max chord 10 m.
        assert_eq!(PRESETS, [(0.24, 12.5, 0.635), (0.12, 6.25, 0.254), (0.06, 2.5, 0.0254)]);
        assert_eq!((CustomResolution::default(), CustomResolution::default().max_chord_m), (CustomResolution::preset(1), 10.0));
        assert_eq!(export_base_name("Part Studio 1", Some("Bolt <3>")), "Part Studio 1 - Bolt");
        assert_eq!(export_base_name("Part Studio 1", None), "Part Studio 1");
    }

    #[test]
    fn base64_pads() {
        assert_eq!(super::base64(b"ab"), "YWI=");
        assert_eq!(super::base64(b"abc"), "YWJj");
        assert_eq!(super::base64(b"a"), "YQ==");
    }

    /// qa EXPORT-MESH-SHARED: a 3MF body is one closed mesh by index (every
    /// edge in exactly two triangles, once each way), so slicers find no
    /// open edges; the faces' shared points are one vertex.
    #[test]
    fn three_mf_bodies_are_manifold_by_index() {
        use noshape_kernel as k;
        use std::collections::HashMap;
        let block = k::make_box(&k::Plane::xy(), 0.04, 0.03, 0.01).unwrap();
        let pin = k::make_cylinder(&k::Plane::new(glam::DVec3::new(0.02, 0.015, -0.01), glam::DVec3::Z, glam::DVec3::X), 0.005, 0.03, std::f64::consts::TAU).unwrap();
        let part = k::cut(&block, &pin).unwrap().shape;
        let m = k::tessellate(&part, 0.0001, 0.35, true).unwrap();
        let xml = String::from_utf8_lossy(&super::three_mf(&[("Part 1".into(), m.clone(), [0.5, 0.5, 0.5, 1.0])])).to_string();
        let attr = |l: &str, k: &str| -> u32 { l.split(&format!("{k}=\"")).nth(1).unwrap().split('"').next().unwrap().parse().unwrap() };
        let tris: Vec<[u32; 3]> = xml.lines().filter(|l| l.starts_with("<triangle ")).map(|l| [attr(l, "v1"), attr(l, "v2"), attr(l, "v3")]).collect();
        let verts = xml.lines().filter(|l| l.starts_with("<vertex ")).count();
        assert!(verts < m.positions.len(), "welded: {verts} of {}", m.positions.len());
        let mut directed: HashMap<(u32, u32), u32> = HashMap::new();
        for t in &tris {
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                *directed.entry((a, b)).or_default() += 1;
            }
        }
        let bad: Vec<_> = directed.iter().filter(|((a, b), n)| **n != 1 || directed.get(&(*b, *a)) != Some(&1)).collect();
        assert!(bad.is_empty(), "{} of {} directed edges unmatched", bad.len(), directed.len());
    }

    /// qa RHINO-GREEN: RHINO is offered only by a build that can write it;
    /// the dialog's rows and the formats stay aligned either way.
    #[test]
    fn rhino_is_listed_only_when_built_in() {
        use super::PartFormat;
        let list = PartFormat::list(false);
        assert_eq!(list.contains(&PartFormat::Rhino), noshape_kernel::RHINO_EXPORT);
        for (i, f) in list.iter().enumerate() {
            let mut s = super::ExportSettings::default();
            super::set_part_format(&mut s, i);
            assert_eq!(s.format, *f);
        }
    }

    #[test]
    fn zip_entries_are_dated() {
        // 2026-10-27 12:54:31 UTC.
        let (time, date) = super::dos_time(1_793_105_671);
        assert_eq!((date >> 9, (date >> 5) & 15, date & 31), (46, 10, 27));
        assert_eq!((time >> 11, (time >> 5) & 63, (time & 31) * 2), (12, 54, 30));
        let z = super::stored_zip_at(&[("a.txt", b"hi")], (time, date));
        assert_eq!(&z[10..14], &[time.to_le_bytes(), date.to_le_bytes()].concat()[..]);
    }

    #[test]
    fn stored_zip_has_end_record() {
        let z = super::stored_zip(&[("a.txt", b"hello")]);
        assert_eq!(&z[..4], &[0x50, 0x4b, 0x03, 0x04]);
        assert_eq!(&z[z.len() - 22..z.len() - 18], &[0x50, 0x4b, 0x05, 0x06]);
        // Local header 30 + name + data, central entry 46 + name, end record 22.
        assert_eq!(z.len(), 30 + 5 + 5 + 46 + 5 + 22);
    }

    /// Live's exportSolidOptions: each format keeps its own version, units
    /// and resolution while the dialog switches between them, and the kept
    /// ones come back on opening.
    #[test]
    fn each_format_keeps_its_own_choices() {
        use super::*;
        let row = |f: PartFormat| PartFormat::list(false).iter().position(|x| *x == f).unwrap();
        let mut s = SolidExportPrefs::default().settings(false);
        // (The units held are STEP's, unseen until its custom units are ticked: Centimeter, live qexport.)
        assert_eq!((s.format, s.latest, s.resolution, s.units, s.stl_text), (PartFormat::Parasolid, true, 1, 0, false), "live's first Export");
        set_part_format(&mut s, row(PartFormat::Step));
        (s.version, s.latest, s.custom_units, s.units) = (1, false, true, 4);
        set_part_format(&mut s, row(PartFormat::Stl));
        assert_eq!((s.units, s.resolution), (3, 1), "STL's own units");
        (s.resolution, s.stl_text) = (2, true);
        set_part_format(&mut s, row(PartFormat::Obj));
        assert_eq!(s.resolution, 1, "OBJ shares glTF's resolution, not STL's");
        set_part_format(&mut s, row(PartFormat::Parasolid));
        assert!(s.latest, "Parasolid's own version");
        set_part_format(&mut s, row(PartFormat::Step));
        assert_eq!((s.version, s.latest, s.custom_units, s.units), (1, false, true, 4));
        let mut kept = (*s.memory).clone();
        kept.keep(&s);
        let kept: SolidExportPrefs = serde_json::from_str(&serde_json::to_string(&kept).unwrap()).unwrap();
        let mut again = kept.settings(false);
        assert_eq!((again.format, again.version, again.latest, again.units), (PartFormat::Step, 1, false, 4), "reopened on STEP AP214 in mm");
        set_part_format(&mut again, row(PartFormat::Stl));
        assert_eq!((again.resolution, again.stl_text, again.units), (2, true, 3));
        let urdf = SolidExportPrefs { format: "URDF".into(), ..Default::default() };
        assert_eq!(urdf.settings(false).format, PartFormat::Parasolid, "a format the list lacks: its first");
        assert_eq!(urdf.settings(true).format, PartFormat::Urdf);
    }

    /// Live onFormatChanged: URDF checks "Include hidden instances in
    /// export"; leaving it gives back the earlier value.
    #[test]
    fn urdf_checks_include_hidden_until_left() {
        use super::*;
        let row = |f: PartFormat| PartFormat::list(true).iter().position(|x| *x == f).unwrap();
        let mut s = SolidExportPrefs::default().settings(true);
        set_asm_format(&mut s, row(PartFormat::Urdf));
        assert!(s.hidden);
        set_asm_format(&mut s, row(PartFormat::Step));
        assert!(!s.hidden);
        s.hidden = true;
        set_asm_format(&mut s, row(PartFormat::Urdf));
        set_asm_format(&mut s, row(PartFormat::Gltf));
        assert!(s.hidden);
    }
}
