//! Blend corpus (noshape's own fillets and chamfers, as the Boolean's corpus): `NOSHAPE_RECORD_BLENDS=<dir>`
//! makes [`crate::fillet`], [`crate::chamfer`] and their variants write each call: `<hash>.brep` (a compound of
//! the shape, a compound of the edges and one of the reference faces: shared sub-shapes stay shared) and
//! `<hash>.txt` (the call and a [`Summary`] of its result). The replay test (`tests/blend_replay.rs`) runs the
//! cases again, OCCT's way and noshape's, and compares.

use crate::bool_corpus::Summary;
use crate::{ChamferSpec, OpResult, Shape, ShapeType};
use std::hash::Hasher;
use std::path::{Path, PathBuf};

/// One blend call.
#[derive(Clone, Debug, PartialEq)]
pub enum Blend {
    /// Constant radius, one per edge (or one for all).
    Fillet { radii: Vec<f64> },
    /// Variable radius along one edge: (relative parameter, radius).
    FilletVariable { law: Vec<(f64, f64)> },
    Chamfer(ChamferSpec),
}

/// A recorded case.
pub struct Case {
    pub name: String,
    pub blend: Blend,
    pub checked: bool,
    pub shape: Shape,
    pub edges: Vec<Shape>,
    pub faces: Vec<Shape>,
    pub summary: Summary,
    /// The kernel's working scale when it was recorded.
    pub scale: f64,
    /// Milliseconds the recorded call took.
    pub ms: f64,
}

/// FNV-1a (stable across runs: the corpus is deduplicated by it).
struct Fnv(u64);

impl Hasher for Fnv {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.0 ^= *b as u64;
            self.0 = self.0.wrapping_mul(0x100_0000_01b3);
        }
    }
}

/// The recording directory, if recording.
pub fn recording() -> Option<PathBuf> {
    std::env::var_os("NOSHAPE_RECORD_BLENDS").map(PathBuf::from)
}

fn describe(blend: &Blend, checked: bool) -> String {
    let mut s = String::new();
    match blend {
        Blend::Fillet { radii } => s += &format!("kind fillet\nradii {}\n", radii.iter().map(|r| format!("{r:e}")).collect::<Vec<_>>().join(" ")),
        Blend::FilletVariable { law } => s += &format!("kind fillet_variable\nlaw {}\n", law.iter().map(|(t, r)| format!("{t:e} {r:e}")).collect::<Vec<_>>().join(" ")),
        Blend::Chamfer(spec) => {
            let (mode, a, b) = match spec {
                ChamferSpec::Equal(d) => ("equal", *d, 0.0),
                ChamferSpec::TwoDistances(a, b) => ("two", *a, *b),
                ChamferSpec::DistanceAngle(d, a) => ("angle", *d, *a),
            };
            s += &format!("kind chamfer\nmode {mode}\nd1 {a:e}\nd2 {b:e}\n");
        }
    }
    s + &format!("checked {}\nscale {:e}\n", checked as u8, crate::working_scale())
}

/// Records one call (when it isn't in the corpus yet): `run` performs it.
pub fn record(dir: &Path, blend: Blend, checked: bool, shape: &Shape, edges: &[Shape], faces: &[Shape], run: impl FnOnce() -> crate::Result<OpResult>) -> crate::Result<OpResult> {
    let inputs = Shape::compound(&[shape.clone(), Shape::compound(edges), Shape::compound(faces)]);
    let Ok(bytes) = inputs.to_brep_bytes() else { return run() };
    let call = describe(&blend, checked);
    let mut h = Fnv(0xcbf2_9ce4_8422_2325);
    h.write(&bytes);
    h.write(call.as_bytes());
    let name = format!("{:016x}", h.finish());
    if dir.join(format!("{name}.txt")).exists() {
        return run();
    }
    let t = std::time::Instant::now();
    let r = run();
    let ms = t.elapsed().as_secs_f64() * 1e3;
    let Ok(res) = &r else { return r };
    let s = Summary::of(res);
    let _ = std::fs::create_dir_all(dir);
    if std::fs::write(dir.join(format!("{name}.brep")), &bytes).is_ok() {
        let text = format!(
            "{call}edges {}\nshape_faces {}\nvalid {}\nvolume {:.17e}\nfaces {}\nedges_out {}\nvertices {}\nhistory {:016x}\nms {ms:.3}\n",
            edges.len(),
            shape.count(ShapeType::Face),
            s.valid as u8,
            s.volume,
            s.faces,
            s.edges,
            s.vertices,
            s.history,
        );
        let _ = std::fs::write(dir.join(format!("{name}.txt")), text);
    }
    r
}

/// The cases of a corpus directory, by name.
pub fn cases(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir).map(|r| r.filter_map(|e| e.ok()).filter_map(|e| e.file_name().to_str()?.strip_suffix(".txt").map(str::to_string)).collect()).unwrap_or_default();
    v.sort();
    v
}

/// Loads one case.
pub fn load(dir: &Path, name: &str) -> Option<Case> {
    let text = std::fs::read_to_string(dir.join(format!("{name}.txt"))).ok()?;
    let get = |k: &str| text.lines().find_map(|l| l.strip_prefix(k)?.strip_prefix(' ')).map(str::trim);
    let num = |k: &str| get(k).and_then(|v| v.parse::<f64>().ok());
    let nums = |k: &str| get(k).map(|v| v.split_whitespace().filter_map(|x| x.parse::<f64>().ok()).collect::<Vec<_>>()).unwrap_or_default();
    let blend = match get("kind")? {
        "fillet" => Blend::Fillet { radii: nums("radii") },
        "fillet_variable" => Blend::FilletVariable { law: nums("law").chunks(2).map(|c| (c[0], c[1])).collect() },
        _ => {
            let (a, b) = (num("d1")?, num("d2")?);
            Blend::Chamfer(match get("mode")? {
                "equal" => ChamferSpec::Equal(a),
                "two" => ChamferSpec::TwoDistances(a, b),
                _ => ChamferSpec::DistanceAngle(a, b),
            })
        }
    };
    let bytes = std::fs::read(dir.join(format!("{name}.brep"))).ok()?;
    let parts = Shape::from_brep_bytes(&bytes).ok()?.children();
    let [shape, edges, faces] = parts.as_slice() else { return None };
    let summary = Summary {
        valid: get("valid") == Some("1"),
        volume: num("volume")?,
        faces: num("faces")? as usize,
        edges: num("edges_out")? as usize,
        vertices: num("vertices")? as usize,
        history: u64::from_str_radix(get("history")?, 16).ok()?,
    };
    Some(Case {
        name: name.to_string(),
        blend,
        checked: get("checked") == Some("1"),
        shape: shape.clone(),
        edges: edges.children(),
        faces: faces.children(),
        summary,
        scale: num("scale").unwrap_or(1.0),
        ms: num("ms")?,
    })
}

/// Runs a case's call again.
pub fn run(case: &Case) -> crate::Result<OpResult> {
    let _ = crate::set_working_scale(case.scale);
    match &case.blend {
        Blend::Fillet { radii } => {
            if case.checked {
                crate::fillet_checked(&case.shape, &case.edges, radii)
            } else {
                crate::fillet_radii(&case.shape, &case.edges, radii)
            }
        }
        Blend::FilletVariable { law } => {
            let e = case.edges.first().ok_or_else(|| crate::KernelError::InvalidInput("no edge".into()))?;
            if case.checked {
                crate::fillet_variable_checked(&case.shape, e, law)
            } else {
                crate::fillet_variable(&case.shape, e, law)
            }
        }
        Blend::Chamfer(spec) => {
            if case.checked {
                crate::chamfer_checked(&case.shape, &case.edges, *spec, &case.faces)
            } else {
                crate::chamfer(&case.shape, &case.edges, *spec, &case.faces)
            }
        }
    }
}
