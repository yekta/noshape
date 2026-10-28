//! Boolean corpus (docs/BOOLEAN-SPEEDUP.md phase 0): `NOSHAPE_RECORD_BOOLEANS=<dir>`
//! makes [`crate::boolean`] write each call it makes: `<hash>.brep` (a compound
//! of two compounds: the objects, the tools) and `<hash>.txt` (the op, its
//! options and a summary of the result: validity, volume, counts, a hash of
//! the history, time, the path that ran). The hash is of the inputs, op and
//! options, so a corpus recorded over many runs keeps one case per distinct
//! call. The replay test (`tests/bool_replay.rs`) runs the cases again and
//! compares.

use crate::{BooleanOp, BooleanOptions, Glue, OpResult, Shape, ShapeType};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

/// Where an output sub-shape lies: rounded point and size (micrometres).
type Place = (i64, i64, i64, i64);

/// What a Boolean produced, as recorded and compared.
#[derive(Clone, Debug, PartialEq)]
pub struct Summary {
    pub valid: bool,
    pub volume: f64,
    pub faces: usize,
    pub edges: usize,
    pub vertices: usize,
    /// Of the sorted history records, each output by its place (see [`Summary::of`]).
    pub history: u64,
}

impl Summary {
    pub fn of(r: &OpResult) -> Summary {
        // The history with each output named by where it lies, not by its
        // index (a fast path may order the result's faces otherwise; names
        // follow the relations): (arg, input kind, input index, relation,
        // output kind, output signature), sorted.
        let mut sig: std::collections::HashMap<(u8, u32), Place> = std::collections::HashMap::new();
        let mut signature = |kind: ShapeType, i: u32| -> Place {
            *sig.entry((kind as u8, i)).or_insert_with(|| {
                let q = |x: f64| (x * 1e6).round() as i64;
                let Ok(x) = r.shape.sub_shape(kind, i as usize) else { return (0, 0, 0, -1) };
                let (p, m) = match kind {
                    ShapeType::Face => (x.centroid().unwrap_or_default(), x.area().unwrap_or(0.0) * 1e3),
                    ShapeType::Edge => {
                        let m = x.edge_info().ok().and_then(|i| x.point_at(0.5 * (i.first_param + i.last_param)).ok()).unwrap_or_default();
                        (m, x.length().unwrap_or(0.0))
                    }
                    ShapeType::Vertex => (x.vertex_point().unwrap_or_default(), 0.0),
                    _ => (glam::DVec3::ZERO, 0.0),
                };
                (q(p.x), q(p.y), q(p.z), q(m))
            })
        };
        let mut recs: Vec<(u32, u8, u32, u8, u8, Place)> = r
            .history
            .records
            .iter()
            .map(|h| (h.arg, h.input_kind as u8, h.input_index, h.relation as u8, h.output_kind as u8, signature(h.output_kind, h.output_index)))
            .collect();
        recs.sort_unstable();
        let mut hs = Fnv(0xcbf2_9ce4_8422_2325);
        recs.hash(&mut hs);
        Summary {
            valid: r.shape.is_valid(),
            volume: r.shape.volume().unwrap_or(0.0),
            faces: r.shape.count(ShapeType::Face),
            edges: r.shape.count(ShapeType::Edge),
            vertices: r.shape.count(ShapeType::Vertex),
            history: hs.finish(),
        }
    }

    /// Differences from `want` (empty: the same): counts, validity and history
    /// exactly, the volume to 1e-9 relative.
    pub fn differences(&self, want: &Summary) -> Vec<String> {
        let mut d = Vec::new();
        if self.valid != want.valid {
            d.push(format!("valid {} (was {})", self.valid, want.valid));
        }
        if (self.volume - want.volume).abs() > 1e-9 * want.volume.abs().max(1e-30) {
            d.push(format!("volume {:.12e} (was {:.12e})", self.volume, want.volume));
        }
        for (what, a, b) in [("faces", self.faces, want.faces), ("edges", self.edges, want.edges), ("vertices", self.vertices, want.vertices)] {
            if a != b {
                d.push(format!("{what} {a} (was {b})"));
            }
        }
        if self.history != want.history {
            d.push("history differs".into());
        }
        d
    }
}

/// A recorded case.
pub struct Case {
    pub name: String,
    pub op: BooleanOp,
    pub opts: BooleanOptions,
    pub objects: Vec<Shape>,
    pub tools: Vec<Shape>,
    pub summary: Summary,
    /// Milliseconds the recorded call took, and the path that ran.
    pub ms: f64,
    pub path: String,
    /// Faces of the objects (the size bucket).
    pub target_faces: usize,
}

/// FNV-1a: stable across runs and builds (the corpus is deduplicated by it).
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

fn op_name(op: BooleanOp) -> &'static str {
    match op {
        BooleanOp::Fuse => "fuse",
        BooleanOp::Cut => "cut",
        BooleanOp::Common => "common",
    }
}

fn glue_name(g: Glue) -> &'static str {
    match g {
        Glue::Off => "off",
        Glue::Shift => "shift",
        Glue::Full => "full",
        Glue::ShiftChecked => "shift_checked",
    }
}

/// The recording directory, if recording.
pub fn recording() -> Option<PathBuf> {
    std::env::var_os("NOSHAPE_RECORD_BOOLEANS").map(PathBuf::from)
}

/// A case's name (a hash of its inputs, op and options) and its inputs as BREP.
fn case_of(op: BooleanOp, objects: &[Shape], tools: &[Shape], opts: &BooleanOptions) -> Option<(String, Vec<u8>)> {
    let inputs = Shape::compound(&[Shape::compound(objects), Shape::compound(tools)]);
    let bytes = inputs.to_brep_bytes().ok()?;
    let mut h = Fnv(0xcbf2_9ce4_8422_2325);
    h.write(&bytes);
    h.write(op_name(op).as_bytes());
    h.write(format!("{} {} {} {} {}", opts.fuzzy, glue_name(opts.glue), opts.unify, opts.parallel, opts.obb).as_bytes());
    Some((format!("{:016x}", h.finish()), bytes))
}

/// Records one call (when it isn't in the corpus yet): `run` performs it.
pub fn record(dir: &Path, op: BooleanOp, objects: &[Shape], tools: &[Shape], opts: &BooleanOptions, run: impl FnOnce() -> crate::Result<OpResult>) -> crate::Result<OpResult> {
    let Some((name, bytes)) = case_of(op, objects, tools, opts) else { return run() };
    if dir.join(format!("{name}.txt")).exists() {
        return run();
    }
    let t = std::time::Instant::now();
    let r = run();
    let ms = t.elapsed().as_secs_f64() * 1e3;
    let Ok(res) = &r else { return r };
    write_case(dir, &name, &bytes, &Call { op, objects, tools, opts }, res, ms, &crate::last_boolean_path());
    r
}

/// One Boolean call: op, arguments, options.
pub struct Call<'a> {
    pub op: BooleanOp,
    pub objects: &'a [Shape],
    pub tools: &'a [Shape],
    pub opts: &'a BooleanOptions,
}

/// Records a call made elsewhere (the kernel's own Booleans outside `boolean`: Shell's, the slabs'),
/// with its time and path.
pub fn record_result(dir: &Path, call: &Call, res: &OpResult, ms: f64, path: &str) {
    let Some((name, bytes)) = case_of(call.op, call.objects, call.tools, call.opts) else { return };
    if !dir.join(format!("{name}.txt")).exists() {
        write_case(dir, &name, &bytes, call, res, ms, path);
    }
}

fn write_case(dir: &Path, name: &str, bytes: &[u8], call: &Call, res: &OpResult, ms: f64, path: &str) {
    let (op, objects, tools, opts) = (call.op, call.objects, call.tools, call.opts);
    let s = Summary::of(res);
    let _ = std::fs::create_dir_all(dir);
    if std::fs::write(dir.join(format!("{name}.brep")), bytes).is_ok() {
        let text = format!(
            "op {}\nfuzzy {:e}\nglue {}\nunify {}\nparallel {}\nobb {}\nobjects {}\ntools {}\ntarget_faces {}\nvalid {}\nvolume {:.17e}\nfaces {}\nedges {}\nvertices {}\nhistory {:016x}\nms {:.3}\npath {}\n",
            op_name(op),
            opts.fuzzy,
            glue_name(opts.glue),
            opts.unify as u8,
            opts.parallel as u8,
            opts.obb as u8,
            objects.len(),
            tools.len(),
            objects.iter().map(|o| o.count(ShapeType::Face)).sum::<usize>(),
            s.valid as u8,
            s.volume,
            s.faces,
            s.edges,
            s.vertices,
            s.history,
            ms,
            path,
        );
        let _ = std::fs::write(dir.join(format!("{name}.txt")), text);
    }
}

/// Writes `s` as the case's expected result (NOSHAPE_BOOL_REBASE: a corpus
/// compares replays with replays; a few cases come out otherwise when OCCT
/// gets them inside a regeneration).
pub fn rebase(dir: &Path, name: &str, s: &Summary) -> std::io::Result<()> {
    let p = dir.join(format!("{name}.txt"));
    let text = std::fs::read_to_string(&p)?;
    let set = |k: &str, v: String| format!("{k} {v}");
    let out: Vec<String> = text
        .lines()
        .map(|l| match l.split_once(' ').map(|x| x.0) {
            Some("valid") => set("valid", (s.valid as u8).to_string()),
            Some("volume") => set("volume", format!("{:.17e}", s.volume)),
            Some("faces") => set("faces", s.faces.to_string()),
            Some("edges") => set("edges", s.edges.to_string()),
            Some("vertices") => set("vertices", s.vertices.to_string()),
            Some("history") => set("history", format!("{:016x}", s.history)),
            _ => l.to_string(),
        })
        .collect();
    std::fs::write(p, out.join("\n") + "\n")
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
    let flag = |k: &str| get(k) == Some("1");
    let op = match get("op")? {
        "fuse" => BooleanOp::Fuse,
        "cut" => BooleanOp::Cut,
        _ => BooleanOp::Common,
    };
    let glue = match get("glue")? {
        "shift" => Glue::Shift,
        "full" => Glue::Full,
        "shift_checked" => Glue::ShiftChecked,
        _ => Glue::Off,
    };
    let opts = BooleanOptions { fuzzy: num("fuzzy")?, glue, unify: flag("unify"), parallel: flag("parallel"), obb: flag("obb") };
    let bytes = std::fs::read(dir.join(format!("{name}.brep"))).ok()?;
    let inputs = Shape::from_brep_bytes(&bytes).ok()?;
    let parts = inputs.children();
    let [objects, tools] = parts.as_slice() else { return None };
    let summary = Summary {
        valid: flag("valid"),
        volume: num("volume")?,
        faces: num("faces")? as usize,
        edges: num("edges")? as usize,
        vertices: num("vertices")? as usize,
        history: u64::from_str_radix(get("history")?, 16).ok()?,
    };
    Some(Case {
        name: name.to_string(),
        op,
        opts,
        objects: objects.children(),
        tools: tools.children(),
        summary,
        ms: num("ms")?,
        path: get("path").unwrap_or("").to_string(),
        target_faces: num("target_faces")? as usize,
    })
}
