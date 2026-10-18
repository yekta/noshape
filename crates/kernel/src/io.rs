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
            Ok(ImportedShape { shape: scaled(&s, f)?, name, color })
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
    let f = unit.mm_per_unit();
    let mut v = Vec::with_capacity(shapes.len());
    let mut names = Vec::new();
    let mut colors = Vec::new();
    let mut has = Vec::new();
    for s in shapes {
        v.push(scaled(&s.shape, f)?);
        names.push(s.name.clone().unwrap_or_default());
        let c = s.color.unwrap_or([0.0; 3]);
        colors.extend_from_slice(&c);
        has.push(s.color.is_some());
    }
    let l = vec_to_list(&v);
    ck(ffi::write_step(&l, &names, &colors, &has, path_str(path)?))
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
