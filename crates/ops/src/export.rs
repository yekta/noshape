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
    /// A part's only (its row's Export…, after STL; not the Part Studio tab's): the Model definitions
    /// Characteristics as CSV ([`inspection_list_csv`], live inspection-list-1118).
    InspectionList,
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

    /// A part row's Format list (live inspection-list-1118): the Part Studio's and Inspection list.
    pub fn part_row_list() -> Vec<PartFormat> {
        let mut list = Self::list(false);
        list.push(Self::InspectionList);
        list
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
            Self::InspectionList => "Inspection list",
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
            Self::InspectionList => "csv",
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
    /// URDF: "Export rigid subassemblies as a single link" (live 1.223; its settings' mergeFastenedComponents).
    pub urdf_single_link: bool,
    /// Options: 0 Download, 1 Download and store file in a new tab, 2 Store
    /// file in a new tab (see [`store_file_tab`]).
    pub options: usize,
    /// Resolution › Custom's fields (live dom g39-export-ps-step).
    pub custom: CustomResolution,
    /// Inspection list: the file's text ([`inspection_list_csv`]).
    pub inspection: String,
    /// Each part's MBD (index = part; [`studio_pmi`]), written to STEP AP242
    /// as semantic PMI.
    pub pmi: Vec<Option<k::PartPmi>>,
    /// The dialog's values of the formats not shown, given back on
    /// switching to them.
    pub memory: Box<SolidExportPrefs>,
    /// The exported tab's name (the dialog's file name): individual files' entries are "<name> - <part>.<ext>".
    pub name: String,
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
    pub urdf_single_link: bool,
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
            urdf_single_link: false,
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
            PartFormat::Urdf => (self.mesh_resolution, self.urdf_geometry, self.urdf_stl_text, self.urdf_single_link) = (s.resolution, s.urdf_geometry, s.stl_text, s.urdf_single_link),
            PartFormat::Parasolid | PartFormat::Rhino | PartFormat::Acis | PartFormat::Jt | PartFormat::InspectionList => {}
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
        (s.iges_compat, s.pvz_group, s.urdf_geometry, s.urdf_single_link) = (self.iges_compat, self.pvz_group, self.urdf_geometry.min(URDF_GEOMETRY.len() - 1), self.urdf_single_link);
        s.compress = if s.format == PartFormat::Glb { self.glb_compress } else { self.gltf_compress };
        let stl = s.format == PartFormat::Stl;
        s.resolution = if stl { self.stl_resolution } else { self.mesh_resolution }.min(CUSTOM_RESOLUTION);
        s.units = if stl { self.stl_units.min(UNITS.len() - 1) } else { p.units.min(STEP_UNITS - 1) };
        s.stl_text = if s.format == PartFormat::Urdf { self.urdf_stl_text } else { self.stl_text };
    }

    /// The Export dialog's settings on opening: the remembered format if
    /// the list has it (else its first) and options.
    pub fn settings(&self, assembly: bool) -> ExportSettings {
        self.settings_in(&PartFormat::list(assembly))
    }

    /// [`Self::settings`] for the Format list `list` (a part row's: [`PartFormat::part_row_list`]).
    pub fn settings_in(&self, list: &[PartFormat]) -> ExportSettings {
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
    /// parts as individual files" (live: even of one part).
    pub fn file_ext(&self) -> &'static str {
        if matches!(self.format, PartFormat::Obj | PartFormat::Urdf) || self.individual { "zip" } else { self.format.ext() }
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
            urdf_single_link: false,
            options: 0,
            custom: CustomResolution::default(),
            inspection: String::new(),
            pmi: Vec::new(),
            memory: Box::default(),
            name: String::new(),
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

/// The parts of Part Studio `tab` in `doc`, a version's document (Revision history's Export: the parts as
/// released), `only` those given (all when empty), for [`write_export`]: (name, shape, colour).
pub fn version_parts(doc: noshape_model::Document, tab: noshape_model::document::TabId, only: &[noshape_model::BodyId]) -> anyhow::Result<Vec<(String, k::Shape, [f32; 4])>> {
    let Some(noshape_model::TabContent::PartStudio(ps)) = doc.tab(tab).map(|t| &t.content) else {
        anyhow::bail!("The Part Studio is not in that version");
    };
    let opts = noshape_model::regen::RegenOptions {
        document_variables: noshape_model::variables::element_variables(&doc, tab),
        derived: Some(crate::derived::DocStudios::new(doc.clone(), Default::default())),
        custom_features: Some(noshape_model::custom::DocumentScripts::from_document(&doc)),
        ..Default::default()
    };
    let r = noshape_model::regen::Regenerator::default().regenerate(ps, &ps.configuration.defaults(), &doc.settings.units, &opts);
    let parts: Vec<_> = r
        .context
        .bodies
        .iter()
        .filter(|b| only.is_empty() || only.contains(&b.id))
        .map(|b| {
            let d = r.display.body(&b.id);
            (d.map(|d| d.name.clone()).unwrap_or_else(|| b.name.clone()), b.shape.clone(), d.map(|d| d.color).unwrap_or([0.7, 0.7, 0.7, 1.0]))
        })
        .collect();
    anyhow::ensure!(only.is_empty() || parts.len() == only.len(), "The released part is not in its version");
    Ok(parts)
}

/// Each part through the format's writer into its own file: (`<name>.<ext>`, bytes), names' spaces `_`.
/// One file per part, named "<tab> - <part>.<ext>" with `tab` (live's individual files), else "<Part_name>.<ext>".
fn part_files(s: &ExportSettings, ext: &str, parts: &[(String, k::Shape, [f32; 4])], tab: Option<&str>) -> anyhow::Result<Vec<(String, Vec<u8>)>> {
    let tmp = |i: usize| std::env::temp_dir().join(format!("noshape-export-{}-{i}.{ext}", std::process::id()));
    let mut used = std::collections::HashSet::new();
    let mut out = Vec::new();
    for (i, p) in parts.iter().enumerate() {
        let mut name = match tab {
            Some(t) => sanitize(&format!("{t} - {}", p.0)),
            None => sanitize(&p.0).replace(' ', "_"),
        };
        while !used.insert(name.clone()) {
            name.push('_');
        }
        let t = tmp(i);
        let one = ExportSettings { pmi: vec![s.pmi.get(i).cloned().flatten()], ..s.clone() };
        write_parts(&one, &t, std::slice::from_ref(p))?;
        out.extend(with_mesh_file(&t, &format!("{name}.{ext}"))?);
    }
    Ok(out)
}

pub fn write_export(s: &ExportSettings, path: &std::path::Path, parts: Vec<(String, k::Shape, [f32; 4])>) -> anyhow::Result<std::path::PathBuf> {
    anyhow::ensure!(!parts.is_empty(), "Selected tab contains no supported geometry for export.");
    if s.format == PartFormat::InspectionList {
        std::fs::write(path, &s.inspection)?;
        return Ok(path.to_path_buf());
    }
    if let Some(msg) = missing_writer(s.format) {
        anyhow::bail!(msg);
    }
    // Intersection edges exact where the faces allow it: a reader refits approximated ones (live's import of a sheet
    // metal part made 61 faces of its 58, PS-INEXACT-INTERSECTION-EDGES).
    let parts: Vec<(String, k::Shape, [f32; 4])> = parts.into_iter().map(|(n, sh, c)| (n, k::exact_edges(&sh).unwrap_or(sh), c)).collect();
    // Live's IGES and ACIS leave a mesh part out beside B-rep parts (c2-hex-bookcase-derived: live.igs and live.sat
    // hold its B-rep part alone).
    let parts = if matches!(s.format, PartFormat::Iges | PartFormat::Acis) && parts.iter().any(|p| !p.1.is_mesh()) { parts.into_iter().filter(|p| !p.1.is_mesh()).collect() } else { parts };
    let parts: Vec<(String, k::Shape, [f32; 4])> = if faceted_meshes(s.format) { parts.into_iter().map(|(n, sh, c)| (n, faceted(sh), c)).collect() } else { parts };
    // Y axis up: the model's +Z becomes +Y.
    let parts: Vec<(String, k::Shape, [f32; 4])> = if s.y_up && s.format != PartFormat::Urdf {
        parts
            .into_iter()
            .map(|(n, sh, c)| Ok((n, sh.transformed(&k::Transform::Rotate { origin: glam::DVec3::ZERO, axis: glam::DVec3::X, angle: -std::f64::consts::FRAC_PI_2 })?, c)))
            .collect::<anyhow::Result<_>>()?
    } else {
        parts
    };
    let files_of = |s: &ExportSettings, ext: &str, tab: Option<&str>| part_files(s, ext, &parts, tab);
    if s.format == PartFormat::Urdf {
        let geo = URDF_GEOMETRY[s.urdf_geometry.min(URDF_GEOMETRY.len() - 1)];
        let ms = ExportSettings { format: geo, y_up: false, individual: false, ..s.clone() };
        let robot = path.file_stem().map(|x| x.to_string_lossy().to_string()).unwrap_or_else(|| "robot".into());
        let meshes = files_of(&ms, geo.ext(), None)?;
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
    // Live (exchange batch 3, 11-20): a zip even of one part, its entries "<tab> - <part>.<ext>".
    if s.individual {
        let tab = if s.name.is_empty() { path.file_stem().map(|x| x.to_string_lossy().to_string()).unwrap_or_default() } else { s.name.clone() };
        let files = files_of(s, s.format.ext(), Some(&tab))?;
        let entries: Vec<(&str, &[u8])> = files.iter().map(|(n, b)| (n.as_str(), b.as_slice())).collect();
        let out = path.with_extension("zip");
        std::fs::write(&out, stored_zip(&entries))?;
        return Ok(out);
    }
    // OBJ: a zip of `<name>.obj` and its `<name>.mtl`, as live downloads it.
    if s.format == PartFormat::Obj {
        let stem = path.file_stem().map(|x| x.to_string_lossy().to_string()).unwrap_or_else(|| "part".into());
        let (defl, ang, min) = mesh_params(s);
        let meshes: Vec<(String, k::Tessellation, [f32; 4])> = parts.iter().map(|(n, sh, c)| Ok((n.clone(), export_mesh(sh, defl, ang, min)?, *c))).collect::<anyhow::Result<_>>()?;
        let (obj, mtl) = obj_and_mtl(&meshes, &stem);
        let (on, mn) = (format!("{stem}.obj"), format!("{stem}.mtl"));
        let out = path.with_extension("zip");
        std::fs::write(&out, stored_zip(&[(on.as_str(), obj.as_bytes()), (mn.as_str(), mtl.as_bytes())]))?;
        return Ok(out);
    }
    write_parts(s, path, &parts)?;
    if s.format == PartFormat::Parasolid && path.with_extension("xmm_txt").is_file() {
        let name = path.file_name().map(|x| x.to_string_lossy().to_string()).unwrap_or_else(|| "part.x_t".into());
        let files = with_mesh_file(path, &name)?;
        let entries: Vec<(&str, &[u8])> = files.iter().map(|(n, b)| (n.as_str(), b.as_slice())).collect();
        let out = path.with_extension("zip");
        std::fs::write(&out, stored_zip(&entries))?;
        return Ok(out);
    }
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
    /// Each instance's rigid subassembly (the outermost one it moves with, as one body): (its path, its name).
    pub rigid: Vec<Option<(Vec<noshape_model::assembly::InstanceId>, String)>>,
    /// Each instance's occurrence path.
    pub paths: Vec<Vec<noshape_model::assembly::InstanceId>>,
    /// The mates and connectors a URDF's joints come from ([`Self::with_kinematics`]).
    pub kinematics: Option<crate::urdf::Kinematics>,
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
        let mut out = AssemblyExport { root: root.to_string(), parts: Vec::new(), instances: Vec::new(), rigid: Vec::new(), paths: Vec::new(), kinematics: None };
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
            out.rigid.push(rigid_subassembly(&result.nodes, o));
            out.paths.push(o.path.clone());
        }
        out
    }

    /// With what a URDF's links and joints need: each instance's density and whether it is fixed, the explicit mate
    /// connectors, the mates (a flexible subassembly's own too) at their solved frames.
    pub fn with_kinematics(mut self, doc: &noshape_model::document::Document, asm: &noshape_model::assembly::Assembly, result: &noshape_model::assembly::eval::AssemblyResult) -> Self {
        use noshape_model::assembly::eval::{frame_matrix, mate_limits};
        use noshape_model::assembly::{InstanceId, MateConnectorRef};
        let index = |path: &[InstanceId]| self.paths.iter().position(|p| p.as_slice() == path);
        let occurrence = |path: &[InstanceId]| result.occurrences.iter().find(|o| o.path.as_slice() == path);
        let mut kin = crate::urdf::Kinematics::default();
        for path in &self.paths {
            let props = occurrence(path).and_then(|o| match &doc.tab(o.studio)?.content {
                noshape_model::TabContent::PartStudio(ps) => ps.part_properties.get(&o.body).map(|p| (p.density, p.mass_override)),
                _ => None,
            });
            kin.density.push(props.unwrap_or_default());
            let top_fixed = asm.instance(path[0]).is_some_and(|i| i.fixed);
            let moves_in_sub = occurrence(path).and_then(|o| o.flex.as_ref()).is_some_and(|(key, _)| result.prepared.is_flexible_child(key));
            kin.fixed.push((top_fixed && !moves_in_sub) || asm.occurrence(path).fixed);
        }
        // A side's instance: the part its connector or entity is on (`prefix`: the subassembly's path from here).
        let side_of = |prefix: &[InstanceId], c: &MateConnectorRef, connectors: &[noshape_model::assembly::AsmMateConnector]| -> Option<Option<usize>> {
            let on = match c.connector {
                Some(id) => &connectors.iter().find(|x| x.id == id)?.on,
                None => c,
            };
            if on.instance == InstanceId::ORIGIN {
                return prefix.is_empty().then_some(None);
            }
            let mut path = prefix.to_vec();
            path.push(on.instance);
            path.extend(on.path.iter().copied());
            // A connector on a subassembly's instance: its first part.
            Some(Some(index(&path).or_else(|| self.paths.iter().position(|p| p.starts_with(&path)))?))
        };
        for c in asm.connectors.iter().filter(|c| !c.suppressed) {
            let (Some(Some(i)), Some(f)) = (side_of(&[], &c.on, &[]), result.connector_frame(c.id)) else { continue };
            kin.connectors.push((c.name.clone(), i, frame_matrix(&f)));
        }
        for st in result.mates.iter().filter(|m| m.ok && !m.inactive) {
            let (Some(m), Some(fa), Some(fb)) = (asm.mate(st.id), st.frame_a, st.frame_b) else { continue };
            let (Some(a), Some(b)) = (side_of(&[], &m.a, &asm.connectors), side_of(&[], &m.b, &asm.connectors)) else { continue };
            kin.mates.push(crate::urdf::JointMate { name: m.name.clone(), kind: m.mate_type, a, b, frame_a: frame_matrix(&fa), frame_b: frame_matrix(&fb), limits: mate_limits(m, &doc.settings.units) });
        }
        for (prefix, m, id) in &result.prepared.flex_mates {
            let Some((fa, fb)) = result.solved.mate_frames.get(id) else { continue };
            let (Some(Some(a)), Some(Some(b))) = (side_of(prefix, &m.a, &[]), side_of(prefix, &m.b, &[])) else { continue };
            kin.mates.push(crate::urdf::JointMate { name: m.name.clone(), kind: m.mate_type, a: Some(a), b: Some(b), frame_a: frame_matrix(fa), frame_b: frame_matrix(fb), limits: mate_limits(m, &doc.settings.units) });
        }
        self.kinematics = Some(kin);
        self
    }

    /// Each instance as a placed part, named after it (formats without
    /// assemblies).
    pub fn placed(&self) -> anyhow::Result<Vec<(String, k::Shape, [f32; 4])>> {
        self.instances.iter().map(|(p, name, t)| Ok((name.clone(), self.parts[*p].1.transformed(&k::Transform::Matrix(*t))?, self.parts[*p].2))).collect()
    }
}

/// The outermost subassembly instance a part occurrence moves with as one body: one riding on its parent whole (not
/// flexible: locked, patterned, …; its top-level instance when the part isn't in a flexible one), or one whose own
/// instances can't move (no degrees of freedom, one fixed: the rigid assembly icon). (Path, name.)
fn rigid_subassembly(nodes: &[noshape_model::assembly::eval::InstanceNode], o: &noshape_model::assembly::eval::PartOccurrence) -> Option<(Vec<noshape_model::assembly::InstanceId>, String)> {
    // The instance the part's placement follows: its top-level one, or (inside flexible subassemblies) that child.
    let rides = o.flex.as_ref().map_or(1, |f| f.0.len());
    let mut level = nodes;
    for (d, id) in o.path.iter().enumerate().take(rides.min(o.path.len() - 1)) {
        let n = level.iter().find(|n| n.path.last() == Some(id))?;
        if n.kind == noshape_model::assembly::eval::NodeKind::Assembly && (n.rigid || d + 1 == rides) {
            return Some((o.path[..=d].to_vec(), n.name.clone()));
        }
        level = &n.children;
    }
    None
}

/// Write an Assembly's export: STEP, Parasolid and IGES keep the assembly
/// (Onshape: each part once, placed by its instances); the other formats
/// and individual files get the placed parts.
pub fn write_assembly_export(s: &ExportSettings, path: &std::path::Path, a: &AssemblyExport) -> anyhow::Result<std::path::PathBuf> {
    anyhow::ensure!(!a.instances.is_empty(), "Selected tab contains no supported geometry for export.");
    // Y axis up: the model's +Z becomes +Y.
    let up = if s.y_up { glam::DMat4::from_rotation_x(-std::f64::consts::FRAC_PI_2) } else { glam::DMat4::IDENTITY };
    let instances: Vec<k::StepInstance> = a.instances.iter().map(|(p, _, t)| k::StepInstance { part: *p, transform: up * *t }).collect();
    let parts: Vec<(String, k::Shape, [f32; 4])> = a.parts.iter().map(|(n, sh, c)| (n.clone(), if faceted_meshes(s.format) { faceted(sh.clone()) } else { sh.clone() }, *c)).collect();
    // Parasolid keeps the assembly too (live: its x_t re-imports as an
    // Assembly; qa ASM-PARASOLID-FLAT), in every version.
    if s.format == PartFormat::Parasolid && !s.individual {
        k::write_parasolid_assembly(path, &a.root, &parts.iter().map(|(n, sh, c)| step_shape(n, sh, c)).collect::<Vec<_>>(), &instances, k::LengthUnit::Meter, parasolid_version(s))?;
        return Ok(path.to_path_buf());
    }
    // IGES as subfigure instances (live 1755: re-imports as an Assembly).
    if s.format == PartFormat::Iges && !s.individual {
        k::write_iges_assembly(path, &parts.iter().map(|(n, sh, c)| step_shape(n, sh, c)).collect::<Vec<_>>(), &instances, k::LengthUnit::Meter)?;
        return Ok(path.to_path_buf());
    }
    if s.format == PartFormat::Urdf {
        return write_urdf_assembly(s, path, a);
    }
    if s.format != PartFormat::Step || s.individual {
        return write_export(s, path, a.placed()?);
    }
    let shapes: Vec<k::ExportShape> = a.parts.iter().map(|(n, sh, c)| step_shape(n, sh, c)).collect();
    k::write_step_assembly(path, &a.root, &shapes, &instances, k::LengthUnit::Meter, &step_options(s))?;
    Ok(path.to_path_buf())
}

/// An Assembly's URDF as live's zip: `<robot>/urdf/<robot>.urdf`, `<robot>/launch/<robot>.launch` and a mesh per part
/// in its Part Studio's coordinates, `<robot>/meshes/<Part_name>.stl` ([`crate::urdf`]).
fn write_urdf_assembly(s: &ExportSettings, path: &std::path::Path, a: &AssemblyExport) -> anyhow::Result<std::path::PathBuf> {
    let geo = URDF_GEOMETRY[s.urdf_geometry.min(URDF_GEOMETRY.len() - 1)];
    let ms = ExportSettings { format: geo, y_up: false, individual: false, pmi: Vec::new(), ..s.clone() };
    let meshes = part_files(&ms, geo.ext(), &a.parts, None)?;
    let masses: Vec<crate::urdf::PartMass> = a.parts.iter().map(|p| k::mass_properties(&p.1).map_or((0.0, glam::DVec3::ZERO, glam::DMat3::ZERO), |m| (m.mass, m.center, m.inertia))).collect();
    let names: Vec<String> = a.parts.iter().map(|p| p.0.clone()).collect();
    let instances: Vec<(usize, glam::DMat4)> = a.instances.iter().map(|(p, _, t)| (*p, *t)).collect();
    let rigid: Vec<Option<Vec<noshape_model::assembly::InstanceId>>> = a.rigid.iter().map(|r| r.as_ref().map(|r| r.0.clone())).collect();
    let kin = a.kinematics.clone().unwrap_or_else(|| crate::urdf::Kinematics { density: vec![(None, None); instances.len()], ..Default::default() });
    let (links, joints) = crate::urdf::tree(&names, &instances, &rigid, &kin, &masses, s.urdf_single_link);
    let robot = crate::urdf::link_name(&a.root);
    let srgb = |v: f32| crate::appearance::lin_to_srgb(v) as f64 / 255.0;
    let files: Vec<(String, [f64; 3])> = meshes.iter().zip(&a.parts).map(|((f, _), p)| (f.clone(), [srgb(p.2[0]), srgb(p.2[1]), srgb(p.2[2])])).collect();
    let urdf = crate::urdf::urdf_text(&robot, &links, &joints, &files);
    let launch = crate::urdf::launch_text(&robot);
    let (un, ln) = (format!("{robot}/urdf/{robot}.urdf"), format!("{robot}/launch/{robot}.launch"));
    let mesh_names: Vec<String> = meshes.iter().map(|(f, _)| format!("{robot}/meshes/{f}")).collect();
    let mut entries: Vec<(&str, &[u8])> = vec![(ln.as_str(), launch.as_bytes())];
    entries.extend(mesh_names.iter().zip(&meshes).map(|(n, (_, b))| (n.as_str(), b.as_slice())));
    entries.push((un.as_str(), urdf.as_bytes()));
    let out = path.with_extension("zip");
    std::fs::write(&out, stored_zip(&entries))?;
    Ok(out)
}

/// Formats whose writers take B-rep faces only: a mesh part (an imported STL) goes as its triangles' planar faces
/// (their writers crashed on a mesh face's missing surface). STEP keeps it as a tessellated solid, Parasolid's writer
/// decides (a solid mesh in its .xmm_txt in the latest version, as live).
fn faceted_meshes(f: PartFormat) -> bool {
    !f.mesh() && !matches!(f, PartFormat::Step | PartFormat::Urdf | PartFormat::Parasolid)
}

/// A Parasolid file and the .xmm_txt its writer put beside it for its mesh bodies (Onshape's Parasolid of a Part
/// Studio with a mesh part comes zipped with it, live c2-hex-bookcase-derived): (entry name, bytes) each, the files
/// removed; the file alone when there is none.
fn with_mesh_file(file: &std::path::Path, name: &str) -> anyhow::Result<Vec<(String, Vec<u8>)>> {
    let mut out = vec![(name.to_string(), std::fs::read(file)?)];
    let _ = std::fs::remove_file(file);
    let mesh = file.with_extension("xmm_txt");
    if mesh.is_file() {
        let stem = name.rsplit_once('.').map_or(name, |x| x.0);
        out.push((format!("{stem}.xmm_txt"), std::fs::read(&mesh)?));
        let _ = std::fs::remove_file(&mesh);
    }
    Ok(out)
}

fn faceted(sh: k::Shape) -> k::Shape {
    if !sh.is_mesh() {
        return sh;
    }
    sh.faceted().unwrap_or(sh)
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
    k::StepOptions { schema, file_unit, pmi: s.pmi.clone(), all_bsurfaces: s.preprocessing == STEP_PREPROCESSING.len() - 1 }
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

/// A part row's Format list's `i`th ([`PartFormat::part_row_list`]).
pub fn set_part_row_format(s: &mut ExportSettings, i: usize) {
    switch_format(s, PartFormat::part_row_list()[i]);
}

/// A part's Inspection list (live inspection-list-1118): its Part Studio's Model definitions › Characteristics rows
/// (toleranced parameters of the features before the rollback bar; datums get none) under the table's header, as
/// live writes it: UTF-8 with a BOM, every field quoted, CRLF.
/// `"Type","Nominal value","Tolerance","Upper limit","Lower limit"` then e.g. `"Distance","10.000","±0.100","10.100","9.900"`.
pub fn inspection_list_csv(ps: &noshape_model::PartStudio, units: &noshape_units::UnitPrefs) -> String {
    let row = |cells: &[&str]| cells.iter().map(|c| format!("\"{}\"", c.replace('"', "\"\""))).collect::<Vec<_>>().join(",") + "\r\n";
    let mut out = String::from("\u{feff}");
    out += &row(&["Type", "Nominal value", "Tolerance", "Upper limit", "Lower limit"]);
    let standard = ps.default_tolerances.as_deref();
    for c in noshape_model::annotations::characteristics(ps, None, ps.rollback_index()).iter().filter(|c| !c.rolled_back) {
        let factor = units.si_factor(c.dim);
        let decimals = usize::from(if c.dim == noshape_units::Dim::ANGLE { units.angle_precision } else { units.length_precision });
        let [nominal, tol, upper, lower] = c.cells(factor, decimals, standard);
        out += &row(&[c.kind, &nominal, &tol, &upper, &lower]);
    }
    out
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
/// angular deviation, the others' distance / angular tolerance), and an STL's shortest edge segment (meters, its
/// Minimum facet width; 0 for the others).
/// Every mesh format as Onshape's Export dialog facets it (live 2994, 3052), whatever the file's unit (the same mesh,
/// scaled): Medium's 0.12 mm chord and 6.25° cut clamp-hms' R5 rim in 58 segments, ceil(sweep / angle) on every
/// circle and torus where the chord doesn't bind. As BRepMesh's deflection twice the chord (half Parasolid's) and its
/// angle twice the segments' turn; an STL's minimum facet width floors the segments (0.7-0.85 × it where chord or
/// angle would go finer). OBJ / 3MF / glTF the same way (c2-housing-draft's 3MF: live 1258 / 2340 / 6188 triangles
/// at Coarse / Medium / Fine, ours 1132 / 2156 / 5710; BRepMesh at the dialog's values gave 2160 / 4332 / 11512).
fn mesh_params(s: &ExportSettings) -> (f64, f64, f64) {
    let c = if s.resolution == CUSTOM_RESOLUTION { s.custom } else { CustomResolution::preset(s.resolution) };
    let stl = s.format == PartFormat::Stl || (s.format == PartFormat::Urdf && URDF_GEOMETRY[s.urdf_geometry.min(3)] == PartFormat::Stl);
    let (lin, ang, min) = if stl {
        (c.chordal_mm * 2e-3, 2.0 * c.angular_deg, 0.7 * c.min_facet_mm * 1e-3)
    } else {
        (2.0 * c.distance_m.max(1e-7), 2.0 * c.angular_tol_deg, 0.0)
    };
    (lin.max(1e-10), ang.to_radians().clamp(1e-3, 1.5), min)
}

/// A part's export mesh (STL, OBJ, 3MF…): a closed surface, consistently wound, at Onshape's facet tolerances
/// ([`k::tessellate_export`]).
/// The chord no longer than a twentieth of the part's box: a chord past the part's size limits nothing (its
/// segments follow the angle), and BRepMesh's mesh broke there (motor-mount-jack at a 100 mm chord and Medium's
/// minimum facet width: a hole filled, +41 % volume).
fn export_mesh(sh: &k::Shape, defl: f64, ang: f64, min: f64) -> anyhow::Result<k::Tessellation> {
    let size = sh.bounding_box(false).map_or(f64::INFINITY, |b| b.max.distance(b.min));
    Ok(k::tessellate_export(sh, defl.min(0.05 * size).max(1e-10), ang, min)?)
}

fn write_parts(s: &ExportSettings, path: &std::path::Path, parts: &[(String, k::Shape, [f32; 4])]) -> anyhow::Result<()> {
    match s.format {
        PartFormat::Step => {
            let shapes: Vec<k::ExportShape> = parts.iter().map(|(n, sh, c)| step_shape(n, sh, c)).collect();
            k::write_step_with(path, &shapes, k::LengthUnit::Meter, &step_options(s))?;
        }
        PartFormat::Iges => k::write_iges_parts_as(path, &parts.iter().map(|(n, sh, c)| step_shape(n, sh, c)).collect::<Vec<_>>(), k::LengthUnit::Meter, s.iges_compat)?,
        // Text transmit file in the chosen version (live G36: the body in that
        // version's layouts; "Use latest version" = 38.1).
        // Named and coloured as Onshape's (TYSA_NAME / TYSA_COLOUR_2).
        PartFormat::Parasolid => {
            // Meshes came with Parasolid 28.1: live refuses a mesh part in an older version.
            let first_mesh_version = PARASOLID_VERSIONS.iter().position(|v| *v == "28.1");
            if PARASOLID_VERSIONS.iter().position(|v| *v == parasolid_version(s)) > first_mesh_version && parts.iter().any(|p| p.1.is_mesh() && p.1.count(k::ShapeType::Solid) > 0) {
                anyhow::bail!("The selected Parasolid version does not support the current geometry contents.");
            }
            k::write_parasolid_parts(path, &parts.iter().map(|(n, sh, c)| step_shape(n, sh, c)).collect::<Vec<_>>(), k::LengthUnit::Meter, false, parasolid_version(s))?
        }
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
            let (defl, ang, min) = mesh_params(s);
            let meshes: Vec<(String, k::Tessellation, [f32; 4])> = parts.iter().map(|(n, sh, c)| Ok((n.clone(), export_mesh(sh, defl, ang, min)?, *c))).collect::<anyhow::Result<_>>()?;
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
                PartFormat::Gltf | PartFormat::Glb => std::fs::write(path, gltf(&meshes, &parts.iter().zip(&meshes).map(|(p, m)| face_uvs(&p.1, &m.1)).collect::<Vec<_>>(), f == PartFormat::Glb, s.compress)?)?,
                _ => std::fs::write(path, three_mf(&meshes))?,
            }
        }
        f => anyhow::bail!("{} export is not available in Noshape yet", f.label()),
    }
    Ok(())
}

/// glTF 2.0 as live's: a node and mesh per part, a primitive per face (positions, normals, the face's (u, v) scaled to
/// 0..1, indices), a double-sided material per colour named by its components; `.glb` packs JSON and buffer into one
/// file, `.gltf` embeds the buffer as a data URI. `compress`: each primitive a Draco buffer
/// (KHR_draco_mesh_compression), as live's Compress.
fn gltf(meshes: &[(String, k::Tessellation, [f32; 4])], uvs: &[Vec<[f32; 2]>], binary: bool, compress: bool) -> anyhow::Result<Vec<u8>> {
    fn bounds<const N: usize>(values: &[[f32; N]]) -> ([f32; N], [f32; N]) {
        values.iter().fold(([f32::MAX; N], [f32::MIN; N]), |(lo, hi), p| (std::array::from_fn(|k| lo[k].min(p[k])), std::array::from_fn(|k| hi[k].max(p[k]))))
    }
    let mut buf: Vec<u8> = Vec::new();
    let (mut views, mut accessors, mut materials, mut gmeshes, mut nodes) = (Vec::new(), Vec::new(), Vec::<serde_json::Value>::new(), Vec::new(), Vec::new());
    let mut primitives_so_far = 0;
    for (i, (name, m, c)) in meshes.iter().enumerate() {
        let srgb = |v: f32| f32::from(crate::appearance::lin_to_srgb(v)) / 255.0;
        let rgb = [srgb(c[0]), srgb(c[1]), srgb(c[2])];
        let material_name = format!("{:.6}_{:.6}_{:.6}_0.000000_0.000000", rgb[0], rgb[1], rgb[2]);
        let material = materials.iter().position(|x| x["name"] == material_name.as_str()).unwrap_or_else(|| {
            materials.push(serde_json::json!({"doubleSided": true, "name": material_name, "pbrMetallicRoughness": {"baseColorFactor": [rgb[0], rgb[1], rgb[2], c[3]], "metallicFactor": 0.0}}));
            materials.len() - 1
        });
        let mesh_name = format!("mesh{primitives_so_far}_mesh");
        let mut primitives = Vec::new();
        for r in m.faces.iter().filter(|r| r.index_count > 0) {
            let at = format!("mesh{primitives_so_far}");
            primitives_so_far += 1;
            let vs = r.first_vertex as usize..(r.first_vertex + r.vertex_count) as usize;
            let (pos, nor) = (&m.positions[vs.clone()], &m.normals[vs.clone()]);
            let tex = uvs.get(i).and_then(|u| u.get(vs.clone())).map_or_else(|| vec![[0.0; 2]; pos.len()], <[_]>::to_vec);
            let idx: Vec<u32> = m.indices[r.first_index as usize..(r.first_index + r.index_count) as usize].iter().map(|v| v - r.first_vertex).collect();
            let mut push = |bytes: &[u8], view_name: Option<(&str, u32)>| {
                while !buf.len().is_multiple_of(4) {
                    buf.push(0);
                }
                let mut view = serde_json::json!({"buffer": 0, "byteLength": bytes.len(), "byteOffset": buf.len()});
                buf.extend_from_slice(bytes);
                // (A Draco buffer's view has no name or target.)
                if let Some((kind, target)) = view_name {
                    (view["name"], view["target"]) = (format!("{at}_{kind}_bufferView").into(), target.into());
                }
                views.push(view);
                views.len() - 1
            };
            let a0 = accessors.len();
            let mut primitive = serde_json::json!({"attributes": {"NORMAL": a0 + 1, "POSITION": a0, "TEXCOORD_0": a0 + 2}, "indices": a0 + 3, "material": material});
            let ((plo, phi), (nlo, nhi), (tlo, thi)) = (bounds(pos), bounds(nor), bounds(&tex));
            let imax = idx.iter().copied().max().unwrap_or(0);
            let mut list = [
                serde_json::json!({"componentType": 5126, "count": pos.len(), "max": phi, "min": plo, "name": format!("{at}_coord_accessor"), "type": "VEC3"}),
                serde_json::json!({"componentType": 5126, "count": nor.len(), "max": nhi, "min": nlo, "name": format!("{at}_norm_accessor"), "type": "VEC3"}),
                serde_json::json!({"componentType": 5126, "count": tex.len(), "max": thi, "min": tlo, "name": format!("{at}_texcoord_accessor"), "type": "VEC2"}),
                serde_json::json!({"componentType": 5125, "count": idx.len(), "max": [imax], "min": [0], "name": format!("{at}_idx_accessor"), "type": "SCALAR"}),
            ];
            if compress {
                // The accessors hold the decoded counts and no data (KHR_draco_mesh_compression).
                let (data, points, ids) = draco_mesh(pos, nor, &tex, &idx)?;
                let view = push(&data, None);
                for a in &mut list[..3] {
                    a["count"] = points.into();
                }
                primitive["extensions"] = serde_json::json!({"KHR_draco_mesh_compression": {"attributes": {"NORMAL": ids[1], "POSITION": ids[0], "TEXCOORD_0": ids[2]}, "bufferView": view}});
            } else {
                let bytes = |v: &mut dyn Iterator<Item = f32>| v.flat_map(f32::to_le_bytes).collect::<Vec<u8>>();
                let vp = push(&bytes(&mut pos.iter().flatten().copied()), Some(("coord", 34962)));
                let vn = push(&bytes(&mut nor.iter().flatten().copied()), Some(("norm", 34962)));
                let vt = push(&bytes(&mut tex.iter().flatten().copied()), Some(("texcoord", 34962)));
                let vi = push(&idx.iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<u8>>(), Some(("idx", 34963)));
                for (a, v) in list.iter_mut().zip([vp, vn, vt, vi]) {
                    a["bufferView"] = v.into();
                }
            }
            accessors.extend(list);
            primitives.push(primitive);
        }
        gmeshes.push(serde_json::json!({"name": mesh_name, "primitives": primitives}));
        nodes.push(serde_json::json!({"mesh": i, "name": name}));
    }
    while !buf.len().is_multiple_of(4) {
        buf.push(0);
    }
    let mut buffer = serde_json::json!({"byteLength": buf.len(), "name": "Total_buffer"});
    if !binary {
        buffer["uri"] = serde_json::Value::String(format!("data:application/octet-stream;base64,{}", base64(&buf)));
    }
    let mut doc = serde_json::json!({
        "accessors": accessors,
        "asset": {"generator": "Noshape", "version": "2.0"},
        "bufferViews": views,
        "buffers": [buffer],
        "materials": materials,
        "meshes": gmeshes,
        "nodes": nodes,
        "scene": 0,
        "scenes": [{"name": "Root", "nodes": (0..nodes.len()).collect::<Vec<_>>()}],
    });
    if compress {
        (doc["extensionsUsed"], doc["extensionsRequired"]) = (serde_json::json!(["KHR_draco_mesh_compression"]), serde_json::json!(["KHR_draco_mesh_compression"]));
    }
    let mut json = serde_json::to_vec(&doc).unwrap_or_default();
    if !binary {
        return Ok(json);
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
    Ok(out)
}

/// Each vertex's face (u, v) scaled to 0..1 over its face, as live's glTF TEXCOORD_0 (none when the mesh's faces aren't
/// the shape's).
fn face_uvs(sh: &k::Shape, m: &k::Tessellation) -> Vec<[f32; 2]> {
    let faces = sh.faces();
    if faces.len() != m.faces.len() {
        return Vec::new();
    }
    let mut out = vec![[0.0f32; 2]; m.positions.len()];
    for (face, r) in faces.iter().zip(&m.faces) {
        let vs = r.first_vertex as usize..(r.first_vertex + r.vertex_count) as usize;
        let mut uv: Vec<[f64; 2]> = Vec::with_capacity(vs.len());
        let mut near: Option<glam::DVec2> = None;
        for p in &m.positions[vs.clone()] {
            let p = glam::DVec3::new(p[0].into(), p[1].into(), p[2].into());
            let hit = near.and_then(|seed| face.project_point_near(p, seed).ok()).or_else(|| face.project_point(p).ok());
            let q = hit.map_or([0.0; 2], |h| [h.u, h.v]);
            near = hit.map(|h| glam::DVec2::new(h.u, h.v));
            uv.push(q);
        }
        // A closed face's seam: a triangle across it has a vertex at either end of the period; move those at the start
        // to the end.
        if let Ok(b) = face.uv_bounds() {
            for (k, (lo, hi)) in [(0, (b[0], b[1])), (1, (b[2], b[3]))] {
                if ((hi - lo) - std::f64::consts::TAU).abs() > 1e-6 {
                    continue;
                }
                for t in m.indices[r.first_index as usize..(r.first_index + r.index_count) as usize].as_chunks::<3>().0 {
                    let at = t.map(|v| (v - r.first_vertex) as usize);
                    let (min, max) = at.iter().fold((f64::MAX, f64::MIN), |(a, b), &v| (a.min(uv[v][k]), b.max(uv[v][k])));
                    if max - min > std::f64::consts::PI {
                        for v in at {
                            if uv[v][k] - lo < std::f64::consts::PI {
                                uv[v][k] += std::f64::consts::TAU;
                            }
                        }
                    }
                }
            }
        }
        let (lo, hi) = uv.iter().fold(([f64::MAX; 2], [f64::MIN; 2]), |(lo, hi), q| ([lo[0].min(q[0]), lo[1].min(q[1])], [hi[0].max(q[0]), hi[1].max(q[1])]));
        let unit = |x: f64, k: usize| if hi[k] - lo[k] > 0.0 { ((x - lo[k]) / (hi[k] - lo[k])) as f32 } else { 0.0 };
        for (o, q) in out[vs].iter_mut().zip(&uv) {
            *o = [unit(q[0], 0), unit(q[1], 1)];
        }
    }
    out
}

/// A face's mesh as a Draco buffer (EdgeBreaker, unquantized, as live's): its bytes, its decoded point count and its
/// position, normal and texture-coordinate attributes' ids.
fn draco_mesh(positions: &[[f32; 3]], normals: &[[f32; 3]], uvs: &[[f32; 2]], indices: &[u32]) -> anyhow::Result<(Vec<u8>, usize, [u32; 3])> {
    use draco_core::{DataType, GeometryAttributeType, PointAttribute, PointIndex};
    let mut mesh = draco_core::Mesh::new();
    let flat3 = |v: &[[f32; 3]]| v.iter().flatten().flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>();
    let attributes = [
        (GeometryAttributeType::Position, 3, flat3(positions)),
        (GeometryAttributeType::Normal, 3, flat3(normals)),
        (GeometryAttributeType::TexCoord, 2, uvs.iter().flatten().flat_map(|x| x.to_le_bytes()).collect()),
    ];
    for (kind, comps, bytes) in attributes {
        let mut att = PointAttribute::new();
        att.init(kind, comps, DataType::Float32, false, positions.len());
        att.buffer_mut().write(0, &bytes);
        mesh.add_attribute(att);
    }
    mesh.set_num_points(positions.len());
    for t in indices.as_chunks::<3>().0 {
        mesh.add_face([PointIndex(t[0]), PointIndex(t[1]), PointIndex(t[2])]);
    }
    // Lossless, as live's (its decoded values are the floats it meshed: -0.0004, -0.012).
    let options = draco_core::EncoderOptions::new();
    let mut encoder = draco_core::MeshEncoder::new();
    encoder.set_mesh(mesh);
    let mut out = draco_core::EncoderBuffer::new();
    let info = encoder.encode_with_info(&options, &mut out).map_err(|e| anyhow::anyhow!("the mesh couldn't be compressed: {e}"))?;
    let id = |kind| info.attributes.iter().find(|a| a.attribute_type == kind).map_or(0, |a| a.unique_id);
    Ok((out.data().to_vec(), info.num_encoded_points, [id(GeometryAttributeType::Position), id(GeometryAttributeType::Normal), id(GeometryAttributeType::TexCoord)]))
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

pub(crate) fn base64(data: &[u8]) -> String {
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
pub(crate) fn stored_zip(files: &[(&str, &[u8])]) -> Vec<u8> {
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

    /// Live (exchange batch 3, 11-20): unique parts as individual files are a zip even of one part, its entries
    /// "<tab> - <part>.<ext>".
    #[test]
    fn individual_files_zip_one_part_named_by_tab() {
        use noshape_kernel as k;
        let block = k::make_box(&k::Plane::xy(), 0.04, 0.03, 0.01).unwrap();
        let s = super::ExportSettings { format: super::PartFormat::Stl, individual: true, name: "Part Studio 1".into(), ..Default::default() };
        let dir = std::env::temp_dir().join(format!("noshape-individual-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(s.file_ext(), "zip");
        let out = super::write_export(&s, &dir.join("out.zip"), vec![("Part 1".into(), block, [0.5, 0.5, 0.5, 1.0])]).unwrap();
        let z = std::fs::read(&out).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(&z[..4], &[0x50, 0x4b, 0x03, 0x04]);
        let n = usize::from(u16::from_le_bytes([z[26], z[27]]));
        assert_eq!(String::from_utf8_lossy(&z[30..30 + n]), "Part Studio 1 - Part 1.stl");
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
