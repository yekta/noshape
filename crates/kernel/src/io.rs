//! File I/O: STEP, IGES, STL, OBJ, BREP (text and binary, to files and bytes).

use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{KernelError, Result};
use crate::ffi::ffi;
use crate::mesh::{Tessellation, tessellate};
use crate::shape::{Shape, ck, list_to_vec, vec_to_list};
use crate::transform::Transform;

/// Length unit of the kernel's shapes for data exchange. The kernel itself is
/// unitless; STEP/IGES files carry units, so readers/writers convert between
/// the file and the given model unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LengthUnit {
    #[default]
    Millimeter,
    Centimeter,
    Meter,
    Inch,
    Foot,
    Yard,
}

impl LengthUnit {
    /// Millimeters per unit.
    pub fn mm_per_unit(self) -> f64 {
        match self {
            LengthUnit::Millimeter => 1.0,
            LengthUnit::Centimeter => 10.0,
            LengthUnit::Meter => 1000.0,
            LengthUnit::Inch => 25.4,
            LengthUnit::Foot => 304.8,
            LengthUnit::Yard => 914.4,
        }
    }
}

fn path_str(p: &Path) -> Result<&str> {
    p.to_str().ok_or_else(|| KernelError::invalid("path is not valid UTF-8"))
}

fn scaled(s: &Shape, factor: f64) -> Result<Shape> {
    if (factor - 1.0).abs() < 1e-15 {
        return Ok(s.clone());
    }
    s.transformed(&Transform::Scale { center: glam::DVec3::ZERO, factor })
}

/// One shape imported from a STEP file, with optional metadata.
#[derive(Debug, Clone)]
pub struct ImportedShape {
    pub shape: Shape,
    /// Product / part name, if present.
    pub name: Option<String>,
    /// Surface color (sRGB, 0..1), if present.
    pub color: Option<[f32; 3]>,
    /// Opacity (0..1) when the file makes the part translucent.
    pub opacity: Option<f32>,
    /// Colours of single faces (index into the shape's faces, sRGB 0..1)
    /// where the file gives them; they override `color`.
    pub face_colors: Vec<(usize, [f32; 3])>,
}

/// The face colours of shape `i` of an import.
fn face_colors_of(r: &ffi::StepImport, i: usize) -> Vec<(usize, [f32; 3])> {
    (0..r.face_part.len())
        .filter(|&k| r.face_part[k] as usize == i)
        .map(|k| (r.face_index[k] as usize, [r.face_colors[3 * k], r.face_colors[3 * k + 1], r.face_colors[3 * k + 2]]))
        .collect()
}

/// Read a STEP file. Assemblies are flattened into one entry per part
/// instance (with its placement applied), carrying names and colors when
/// available (STEPCAFControl / XCAF). Shapes are converted to `unit`.
pub fn read_step(path: &Path, unit: LengthUnit) -> Result<Vec<ImportedShape>> {
    let r = ck(ffi::read_step(path_str(path)?))?;
    let shapes = list_to_vec(&r.shapes);
    let f = 1.0 / unit.mm_per_unit();
    shapes
        .into_iter()
        .enumerate()
        .map(|(i, s)| {
            let name = r.names.get(i).filter(|n| !n.is_empty()).cloned();
            let color = (r.has_color.get(i).copied().unwrap_or(false))
                .then(|| [r.colors[3 * i], r.colors[3 * i + 1], r.colors[3 * i + 2]]);
            Ok(ImportedShape { shape: scaled(&s, f)?, name, color, opacity: r.opacity.get(i).copied().filter(|&o| o < 1.0), face_colors: face_colors_of(&r, i) })
        })
        .collect()
}

/// A shape to export with optional name and color.
#[derive(Debug, Clone)]
pub struct ExportShape {
    pub shape: Shape,
    pub name: Option<String>,
    pub color: Option<[f32; 3]>,
}

/// Write shapes (one product each, with names and colors) to a STEP AP214
/// file. Shapes are in `unit`; the file is written in millimeters.
pub fn write_step(path: &Path, shapes: &[ExportShape], unit: LengthUnit) -> Result<()> {
    write_step_with(path, shapes, unit, &StepOptions::default())
}

/// STEP application protocol written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StepSchema {
    /// OCCT's default (AP214 IS).
    #[default]
    Default,
    Ap242,
    Ap214,
    Ap203,
}

/// Options of [`write_step_with`].
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct StepOptions {
    pub schema: StepSchema,
    /// Length unit of the file (None: millimetres).
    pub file_unit: Option<LengthUnit>,
}

/// [`write_step`] with the schema and the file's length unit.
pub fn write_step_with(path: &Path, shapes: &[ExportShape], unit: LengthUnit, opts: &StepOptions) -> Result<()> {
    let f = unit.mm_per_unit();
    let mut v = Vec::with_capacity(shapes.len());
    for s in shapes {
        v.push(scaled(&s.shape, f)?);
    }
    let schema = match opts.schema {
        // Onshape's default.
        StepSchema::Default | StepSchema::Ap242 => 1,
        StepSchema::Ap214 => 2,
        StepSchema::Ap203 => 3,
    };
    // UnitsMethods_LengthUnit codes.
    let file_unit = match opts.file_unit {
        None => 0,
        Some(LengthUnit::Inch) => 1,
        Some(LengthUnit::Millimeter) => 2,
        Some(LengthUnit::Foot) => 4,
        Some(LengthUnit::Meter) => 6,
        Some(LengthUnit::Centimeter) => 10,
        // Not an OCCT unit: written in feet, then turned into yards (io.cpp).
        Some(LengthUnit::Yard) => 100,
    };
    // Onshape's layout (live G47): OCCT writes each part's geometry in metres
    // (Onshape's unit without custom units), the file is rebuilt from those.
    let file_unit = if file_unit == 0 { 6 } else { file_unit };
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let mut parts = Vec::with_capacity(v.len());
    for (i, sh) in v.iter().enumerate() {
        let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let tmp = std::env::temp_dir().join(format!("noshape-step-{}-{n}.step", std::process::id()));
        let written = ck(ffi::write_step(&vec_to_list(std::slice::from_ref(sh)), &[], &[], &[], path_str(&tmp)?, schema, file_unit));
        let text = written.and_then(|_| std::fs::read_to_string(&tmp).map_err(|e| KernelError::Io(e.to_string())));
        let _ = std::fs::remove_file(&tmp);
        let name = shapes[i].name.clone().filter(|n| !n.is_empty()).unwrap_or_else(|| format!("Part {}", i + 1));
        parts.push(crate::step_out::Part { name, color: shapes[i].color, occt: text? });
    }
    let file_name = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let text = crate::step_out::write(
        &parts,
        &file_name,
        &crate::step_out::timestamp(now),
        match opts.schema {
            StepSchema::Ap214 => crate::step_out::Schema::Ap214,
            StepSchema::Ap203 => crate::step_out::Schema::Ap203,
            _ => crate::step_out::Schema::Ap242,
        },
    )?;
    std::fs::write(path, text).map_err(|e| KernelError::Io(format!("could not write STEP file {}: {e}", path.display())))
}

/// Convenience: write plain shapes to STEP.
pub fn write_step_shapes(path: &Path, shapes: &[Shape], unit: LengthUnit) -> Result<()> {
    let v: Vec<ExportShape> = shapes
        .iter()
        .map(|s| ExportShape { shape: s.clone(), name: None, color: None })
        .collect();
    write_step(path, &v, unit)
}

/// Read an IGES file into one shape (converted to `unit`).
pub fn read_iges(path: &Path, unit: LengthUnit) -> Result<Shape> {
    let s = Shape::from_res(ffi::read_iges(path_str(path)?))?;
    scaled(&s, 1.0 / unit.mm_per_unit())
}

/// Read an STL (ASCII or binary) or OBJ mesh whose coordinates are in
/// `file_unit` (mesh files carry no unit) as a shape in meters: one
/// planar face per triangle in a connected shell (a solid when closed).
/// Meshes with at most `merge_limit` triangles get coplanar neighbours
/// merged into larger faces.
pub fn read_mesh(path: &Path, file_unit: LengthUnit, merge_limit: u32) -> Result<Shape> {
    let s = Shape::from_res(ffi::read_mesh(path_str(path)?, merge_limit))?;
    scaled(&s, file_unit.mm_per_unit() * 1e-3)
}

/// Write shapes to an IGES (BRep mode) file in millimeters.
pub fn write_iges(path: &Path, shapes: &[Shape], unit: LengthUnit) -> Result<()> {
    let f = unit.mm_per_unit();
    let v = shapes.iter().map(|s| scaled(s, f)).collect::<Result<Vec<_>>>()?;
    let l = vec_to_list(&v);
    ck(ffi::write_iges(&l, path_str(path)?))
}

/// Write shapes to a Parasolid transmit file, one body each: text (.x_t) or
/// neutral binary (.x_b), schema 12006 (read by every Parasolid since V12).
/// Shapes are in `unit`; Parasolid files are in meters.
pub fn write_parasolid(path: &Path, shapes: &[Shape], unit: LengthUnit, binary: bool) -> Result<()> {
    let f = unit.mm_per_unit() * 1e-3;
    let v = shapes.iter().map(|s| scaled(s, f)).collect::<Result<Vec<_>>>()?;
    let l = vec_to_list(&v);
    ck(ffi::write_parasolid(&l, path_str(path)?, binary))
}

/// Write shapes to an ACIS save file (.sat, version 7.0), one body each, in
/// millimetres. Shapes are in `unit`.
pub fn write_acis(path: &Path, shapes: &[Shape], unit: LengthUnit) -> Result<()> {
    let f = unit.mm_per_unit();
    let v = shapes.iter().map(|s| scaled(s, f)).collect::<Result<Vec<_>>>()?;
    let l = vec_to_list(&v);
    ck(ffi::write_acis(&l, path_str(path)?))
}

/// Write shapes to a Rhino .3dm file (one named Brep each, millimetres).
/// `version` is the Rhino version (2..8; 0 = the newest openNURBS writes).
/// Needs a kernel built with openNURBS (OPENNURBS_ROOT): CI and releases are.
pub fn write_rhino(path: &Path, shapes: &[ExportShape], unit: LengthUnit, version: u32) -> Result<()> {
    let f = unit.mm_per_unit();
    let v = shapes.iter().map(|s| scaled(&s.shape, f)).collect::<Result<Vec<_>>>()?;
    let names: Vec<String> = shapes.iter().map(|s| s.name.clone().unwrap_or_default()).collect();
    let l = vec_to_list(&v);
    ck(ffi::write_rhino(&l, &names, path_str(path)?, version))
}

/// Number of Breps in a .3dm file, each checked with openNURBS' validity test
/// (an error names the first invalid one).
pub fn rhino_brep_count(path: &Path) -> Result<u32> {
    ck(ffi::rhino_valid_breps(path_str(path)?))
}

/// Write shapes to a JT file (JT 9.5 like Onshape's: per part a triangle mesh in
/// millimetres and the exact B-rep as an embedded Parasolid body). Shapes are in `unit`.
pub fn write_jt(path: &Path, shapes: &[ExportShape], unit: LengthUnit) -> Result<()> {
    let f = unit.mm_per_unit() * 1e-3;
    let v = shapes.iter().map(|s| scaled(&s.shape, f)).collect::<Result<Vec<_>>>()?;
    let names: Vec<String> = shapes.iter().map(|s| s.name.clone().unwrap_or_default()).collect();
    let l = vec_to_list(&v);
    ck(ffi::write_jt(&l, &names, path_str(path)?))
}

/// Write shapes to a PTC Creo View .pvz file, laid out like Onshape's (one part
/// each: per-face mesh and exact B-rep, in metres). Shapes are in `unit`.
pub fn write_pvz(path: &Path, shapes: &[ExportShape], unit: LengthUnit) -> Result<()> {
    let f = unit.mm_per_unit() * 1e-3;
    let v = shapes.iter().map(|s| scaled(&s.shape, f)).collect::<Result<Vec<_>>>()?;
    let names: Vec<String> = shapes.iter().map(|s| s.name.clone().unwrap_or_default()).collect();
    let l = vec_to_list(&v);
    ck(ffi::write_pvz(&l, &names, path_str(path)?))
}

/// Read a PTC Creo View .pvz file as Onshape imports it: mesh bodies only
/// ([`Shape::is_mesh`]), a compound per placed part instance holding its
/// closed solid and/or open pieces (shells). Returned in `unit` (the file is
/// in metres).
pub fn read_pvz(path: &Path, unit: LengthUnit) -> Result<Shape> {
    let s = Shape::from_res(ffi::read_pvz(path_str(path)?))?;
    scaled(&s, 1e3 / unit.mm_per_unit())
}

/// Read an STL or OBJ file as Onshape imports it: mesh bodies (a compound of
/// a solid of the closed pieces and a shell per open piece). Coordinates are
/// in `file_unit` (mesh files carry none); returned in meters.
/// 3MF, glTF and GLB files carry their units: they come in millimetres, so
/// pass [`LengthUnit::Millimeter`] for them.
pub fn read_mesh_bodies(path: &Path, file_unit: LengthUnit) -> Result<Shape> {
    let s = Shape::from_res(ffi::read_mesh_bodies(path_str(path)?))?;
    scaled(&s, file_unit.mm_per_unit() * 1e-3)
}

/// Read a Parasolid transmit file (.x_t / .x_b, any version with an embedded
/// schema, and schema 12006/13006 files) as Onshape imports it: every part
/// (assembly instances placed) with its name and colour, in `unit`.
pub fn read_parasolid_parts(path: &Path, unit: LengthUnit) -> Result<Vec<ImportedShape>> {
    imported(ck(ffi::read_parasolid_parts(path_str(path)?))?, unit)
}

/// Read a Rhino .3dm file as Onshape imports it (openNURBS builds only):
/// Breps as solids/surfaces, meshes as mesh bodies, curves as wires, blocks
/// placed; each with its name and colour, in `unit`.
pub fn read_rhino_parts(path: &Path, unit: LengthUnit) -> Result<Vec<ImportedShape>> {
    imported(ck(ffi::read_rhino_parts_ffi(path_str(path)?))?, unit)
}

/// Read an ACIS .sat / .sab file (any version, and ShapeManager files) as
/// Onshape imports it: every body with its name and colour, in `unit`.
/// Analytic geometry exactly; spline surfaces and curves from the B-spline
/// data the file stores with them.
pub fn read_acis_parts(path: &Path, unit: LengthUnit) -> Result<Vec<ImportedShape>> {
    imported(ck(ffi::read_sat_parts_ffi(path_str(path)?))?, unit)
}

/// Read a JT file through its exact geometry, the XT B-rep segments (JT 8–10;
/// zlib or LZMA/XZ compressed): every part with its name, in `unit`. Files
/// with tessellation only fail; parts come unplaced.
pub fn read_jt_parts(path: &Path, unit: LengthUnit) -> Result<Vec<ImportedShape>> {
    imported(ck(ffi::read_jt_parts_ffi(path_str(path)?))?, unit)
}

/// Read a native CAD part file as Onshape's translators do, from the exact
/// geometry it embeds: SolidWorks .sldprt (2009 and later), Solid Edge
/// .par/.psm and NX .prt (Parasolid), Inventor .ipt (ShapeManager); every
/// part with its name and colour, in `unit`.
pub fn read_native_parts(path: &Path, unit: LengthUnit) -> Result<Vec<ImportedShape>> {
    imported(ck(ffi::read_native_parts_ffi(path_str(path)?))?, unit)
}

/// Read a native part file in one of its configurations (a SOLIDWORKS
/// configuration id, as an assembly's component names it).
pub fn read_native_parts_configuration(path: &Path, unit: LengthUnit, configuration: Option<i32>) -> Result<Vec<ImportedShape>> {
    imported(ck(ffi::read_native_parts_config_ffi(path_str(path)?, configuration.unwrap_or(-1)))?, unit)
}

/// One component of a native assembly.
#[derive(Debug, Clone, PartialEq)]
pub struct NativeComponent {
    /// The parent sub-assembly (index into the list), `None` at the top.
    pub parent: Option<usize>,
    /// Component name ("Shaft") and instance number (Shaft-1).
    pub name: String,
    pub number: u32,
    /// The referenced file's name ("Shaft.SLDPRT").
    pub file: String,
    pub configuration: String,
    pub configuration_id: Option<i32>,
    /// Placement in its parent, metres.
    pub transform: glam::DMat4,
    pub suppressed: bool,
    pub hidden: bool,
    pub excluded_from_bom: bool,
    /// A sub-assembly (its components follow with `parent` = this).
    pub assembly: bool,
}

/// The components of a native assembly as a flattened tree: a SOLIDWORKS
/// assembly (.sldasm) in its active configuration, an Inventor assembly
/// (.iam; its documents looked up next to it, see
/// [`read_native_assembly_in`]) or a Solid Edge assembly (.asm).
pub fn read_native_assembly(path: &Path) -> Result<Vec<NativeComponent>> {
    let dir = path.parent().unwrap_or(Path::new("."));
    read_native_assembly_in(path, dir)
}

/// [`read_native_assembly`]; an Inventor assembly's documents are found by
/// their internal ids anywhere under `dir`, and each component's `file` is
/// its path relative to `dir`.
pub fn read_native_assembly_in(path: &Path, dir: &Path) -> Result<Vec<NativeComponent>> {
    let nodes = ck(ffi::read_native_tree_ffi(path_str(path)?, path_str(dir)?))?;
    Ok(nodes
        .into_iter()
        .map(|n| {
            // Row-vector matrix (p' = p M): its rows are glam's columns.
            let mut m = [0.0; 16];
            m.iter_mut().zip(n.transform.iter()).for_each(|(a, b)| *a = *b);
            let mut transform = glam::DMat4::from_cols_array(&m);
            if transform.w_axis.w.abs() > 1e-12 && (transform.w_axis.w - 1.0).abs() > 1e-12 {
                // The last element is a uniform scale.
                let s = transform.w_axis.w;
                transform = glam::DMat4::from_cols(transform.x_axis * s, transform.y_axis * s, transform.z_axis * s, glam::DVec4::new(transform.w_axis.x, transform.w_axis.y, transform.w_axis.z, 1.0));
            }
            NativeComponent {
                parent: usize::try_from(n.parent).ok(),
                name: n.name,
                number: n.number.max(1) as u32,
                file: n.file,
                configuration: n.configuration,
                configuration_id: (n.config_id >= 0).then_some(n.config_id),
                transform,
                suppressed: n.suppressed,
                hidden: n.hidden,
                excluded_from_bom: n.exclude_bom,
                assembly: n.assembly,
            }
        })
        .collect())
}

/// Extract a zip archive's top-level files into `dir`; returns every entry
/// name (entries in folders are listed but not extracted).
pub fn extract_zip(zip: &Path, dir: &Path) -> Result<Vec<String>> {
    Ok(ck(ffi::extract_zip_ffi(path_str(zip)?, path_str(dir)?))?.into_iter().collect())
}

/// Shapes in metres with names and colours, in `unit`.
fn imported(r: ffi::StepImport, unit: LengthUnit) -> Result<Vec<ImportedShape>> {
    let f = 1e3 / unit.mm_per_unit();
    list_to_vec(&r.shapes)
        .into_iter()
        .enumerate()
        .map(|(i, s)| {
            let name = r.names.get(i).filter(|n| !n.is_empty()).cloned();
            let color = (r.has_color.get(i).copied().unwrap_or(false)).then(|| [r.colors[3 * i], r.colors[3 * i + 1], r.colors[3 * i + 2]]);
            Ok(ImportedShape { shape: scaled(&s, f)?, name, color, opacity: r.opacity.get(i).copied().filter(|&o| o < 1.0), face_colors: face_colors_of(&r, i) })
        })
        .collect()
}

/// Read a Parasolid transmit file written by [`write_parasolid`] (schema
/// 12006, analytic and B-spline geometry) as a compound of bodies in `unit`.
pub fn read_parasolid(path: &Path, unit: LengthUnit) -> Result<Shape> {
    let s = Shape::from_res(ffi::read_parasolid(path_str(path)?))?;
    scaled(&s, 1e3 / unit.mm_per_unit())
}

/// Write a shape as BREP text file (without triangulation).
pub fn write_brep(shape: &Shape, path: &Path) -> Result<()> {
    ck(ffi::write_brep_file(shape.raw(), path_str(path)?))
}

/// Read a BREP text file.
pub fn read_brep(path: &Path) -> Result<Shape> {
    Shape::from_res(ffi::read_brep_file(path_str(path)?))
}

impl Shape {
    /// Serialize to BREP text (BRepTools::Write, no triangulation).
    pub fn to_brep_bytes(&self) -> Result<Vec<u8>> {
        ck(ffi::brep_to_bytes(self.raw()))
    }
    /// Deserialize BREP text.
    pub fn from_brep_bytes(bytes: &[u8]) -> Result<Shape> {
        Shape::from_res(ffi::brep_from_bytes(bytes))
    }
    /// Serialize to compact binary BREP (BinTools). Used for caching
    /// regenerated shapes; `with_triangles` also stores meshes.
    pub fn to_bin_bytes(&self, with_triangles: bool) -> Result<Vec<u8>> {
        ck(ffi::bin_to_bytes(self.raw(), with_triangles))
    }
    /// Deserialize binary BREP.
    pub fn from_bin_bytes(bytes: &[u8]) -> Result<Shape> {
        Shape::from_res(ffi::bin_from_bytes(bytes))
    }
}

/// STL output format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum StlFormat {
    #[default]
    Binary,
    Ascii,
}

fn tri_normal(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> [f32; 3] {
    let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
    let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if l > 0.0 { [n[0] / l, n[1] / l, n[2] / l] } else { [0.0, 0.0, 0.0] }
}

/// Write a tessellation as STL.
pub fn write_stl_mesh(mesh: &Tessellation, path: &Path, format: StlFormat) -> Result<()> {
    let f = std::fs::File::create(path)?;
    let mut w = std::io::BufWriter::new(f);
    let tri = |i: usize| {
        let a = mesh.positions[mesh.indices[3 * i] as usize];
        let b = mesh.positions[mesh.indices[3 * i + 1] as usize];
        let c = mesh.positions[mesh.indices[3 * i + 2] as usize];
        (a, b, c)
    };
    let n = mesh.triangle_count();
    match format {
        StlFormat::Binary => {
            let mut header = [0u8; 80];
            header[..17].copy_from_slice(b"noshape-kernel st");
            w.write_all(&header)?;
            w.write_all(&(n as u32).to_le_bytes())?;
            for i in 0..n {
                let (a, b, c) = tri(i);
                for v in [tri_normal(a, b, c), a, b, c] {
                    for x in v {
                        w.write_all(&x.to_le_bytes())?;
                    }
                }
                w.write_all(&[0, 0])?;
            }
        }
        StlFormat::Ascii => {
            writeln!(w, "solid noshape")?;
            for i in 0..n {
                let (a, b, c) = tri(i);
                let nn = tri_normal(a, b, c);
                writeln!(w, "facet normal {} {} {}", nn[0], nn[1], nn[2])?;
                writeln!(w, " outer loop")?;
                for v in [a, b, c] {
                    writeln!(w, "  vertex {} {} {}", v[0], v[1], v[2])?;
                }
                writeln!(w, " endloop")?;
                writeln!(w, "endfacet")?;
            }
            writeln!(w, "endsolid noshape")?;
        }
    }
    w.flush()?;
    Ok(())
}

/// Tessellate (linear deflection `deflection`, 0.5 rad angular) and write STL.
pub fn write_stl(shape: &Shape, path: &Path, deflection: f64, format: StlFormat) -> Result<()> {
    let m = tessellate(shape, deflection, 0.5, true)?;
    write_stl_mesh(&m, path, format)
}

/// Write a tessellation as Wavefront OBJ (one group per face, with normals).
pub fn write_obj_mesh(mesh: &Tessellation, path: &Path) -> Result<()> {
    let f = std::fs::File::create(path)?;
    let mut w = std::io::BufWriter::new(f);
    writeln!(w, "# noshape-kernel OBJ export")?;
    for p in &mesh.positions {
        writeln!(w, "v {} {} {}", p[0], p[1], p[2])?;
    }
    for n in &mesh.normals {
        writeln!(w, "vn {} {} {}", n[0], n[1], n[2])?;
    }
    for (fi, r) in mesh.faces.iter().enumerate() {
        if r.index_count == 0 {
            continue;
        }
        writeln!(w, "g face{fi}")?;
        for t in mesh.face_indices(fi).as_chunks::<3>().0 {
            let (a, b, c) = (t[0] + 1, t[1] + 1, t[2] + 1);
            writeln!(w, "f {a}//{a} {b}//{b} {c}//{c}")?;
        }
    }
    w.flush()?;
    Ok(())
}

/// Tessellate and write OBJ.
pub fn write_obj(shape: &Shape, path: &Path, deflection: f64) -> Result<()> {
    let m = tessellate(shape, deflection, 0.5, true)?;
    write_obj_mesh(&m, path)
}
