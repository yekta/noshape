//! Hole and thread standard tables: FeatureScript's `holetables.gen.fs`
//! (the embedded standard library), read once with the FS parser. Options
//! keep the source order, which is the order of Onshape's dropdowns (live
//! L58), and leaf values are Onshape's own.
//!
//! A table is a cascade of levels; a *path* is the list of option ids chosen
//! so far (one per level) and is what a [`ParamValue::Lookup`] stores. The
//! leaf of a complete path yields a [`HoleSpec`].
//!
//! [`ParamValue::Lookup`]: crate::params::ParamValue::Lookup

use crate::params::{ParamValue, ParamValues};
use noshape_units::ExprValue;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock};

/// Dimensions resolved from a complete table path. Lengths in meters, angles
/// in radians.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HoleSpec {
    /// Main hole (shaft) diameter: drill size, clearance diameter or, for a
    /// tapped hole without fastener fit, the tap drill diameter. For external
    /// threads the major diameter.
    pub hole_diameter: f64,
    /// Tap drill diameter (tapped holes).
    pub tap_drill_diameter: Option<f64>,
    /// Thread major diameter (tapped holes, external threads).
    pub major_diameter: Option<f64>,
    /// Thread minor diameter (external threads).
    pub minor_diameter: Option<f64>,
    /// Thread pitch (tapped holes, external threads).
    pub pitch: Option<f64>,
    pub cbore_diameter: Option<f64>,
    pub cbore_depth: Option<f64>,
    pub csink_diameter: Option<f64>,
    pub csink_angle: Option<f64>,
    /// Tapered pipe taps: the table's tapped depth and hole depth.
    pub tapped_depth: Option<f64>,
    pub hole_depth: Option<f64>,
}

impl HoleSpec {
    /// Whether the entry describes a threaded hole (FS `showTappedDepth`).
    pub fn is_threaded(&self) -> bool {
        self.pitch.is_some()
    }
}

/// (id, label) option of a level.
pub type Opt = (String, String);

/// Table names used by [`crate::params::ParamKind::Lookup`] parameters.
pub const TABLE_ANSI_HOLE: &str = "hole.ansi";
pub const TABLE_ANSI_HOLE_EX: &str = "hole.ansiEx";
pub const TABLE_ISO_HOLE: &str = "hole.iso";
pub const TABLE_ISO_HOLE_EX: &str = "hole.isoEx";
pub const TABLE_ANSI_THREAD_CLASS: &str = "thread.ansiClass";
pub const TABLE_ISO_THREAD_CLASS: &str = "thread.isoClass";
pub const TABLE_EXTERNAL_THREAD: &str = "externalThread";
/// Generic hole table whose first level is the standard ("ANSI" / "ISO").
pub const TABLE_HOLE: &str = "hole";

pub const ALL_TABLES: &[&str] = &[
    TABLE_HOLE,
    TABLE_ANSI_HOLE,
    TABLE_ANSI_HOLE_EX,
    TABLE_ISO_HOLE,
    TABLE_ISO_HOLE_EX,
    TABLE_ANSI_THREAD_CLASS,
    TABLE_ISO_THREAD_CLASS,
    TABLE_EXTERNAL_THREAD,
];

// ---------------------------------------------------------------------------
// Tables: FeatureScript's holetables.gen.fs, parsed once
// ---------------------------------------------------------------------------

const IN: f64 = 0.0254;
const MM: f64 = 0.001;

/// A lookup-table level: `{ "name", "displayName", "default", "entries" }`.
struct Level {
    /// The level's key in a path map ("size", "pitch", "fit", ...).
    key: String,
    /// Shown name ("Size", "Threads/inch", ...).
    name: String,
    default: Option<String>,
    /// (option, what it leads to), in source order (the dropdown's order).
    entries: Vec<(String, Arc<Entry>)>,
}

enum Entry {
    Level(Level),
    /// Leaf values (`"holeDiameter" : "0.25 in"`).
    Values(Vec<(String, String)>),
    /// A plain string leaf (thread classes) or an empty map ("Custom").
    Leaf,
}

/// Every table constant of `holetables.gen.fs` by name, plus the
/// [`TABLE_EXTERNAL_THREAD`] / [`TABLE_HOLE`] standard levels.
static TABLES: LazyLock<HashMap<String, Arc<Entry>>> = LazyLock::new(load_tables);

fn load_tables() -> HashMap<String, Arc<Entry>> {
    use noshape_featurescript::ast::{Expr, ExprKind, MapKey, TopKind};
    fn text(e: &Expr) -> Option<&str> {
        if let ExprKind::Str(s) = &*e.kind { Some(s) } else { None }
    }
    fn key(k: &MapKey) -> Option<&str> {
        match k {
            MapKey::Str(s) => Some(s),
            MapKey::Expr(e) => text(e),
        }
    }
    fn resolve(name: &str, consts: &HashMap<&str, &Expr>, memo: &mut HashMap<String, Arc<Entry>>) -> Option<Arc<Entry>> {
        if let Some(e) = memo.get(name) {
            return Some(e.clone());
        }
        let e = convert(consts.get(name)?, consts, memo)?;
        memo.insert(name.to_string(), e.clone());
        Some(e)
    }
    fn convert(e: &Expr, consts: &HashMap<&str, &Expr>, memo: &mut HashMap<String, Arc<Entry>>) -> Option<Arc<Entry>> {
        match &*e.kind {
            ExprKind::Ident(q) => resolve(&q.name, consts, memo),
            ExprKind::Str(_) => Some(Arc::new(Entry::Leaf)),
            ExprKind::Map(items) => {
                let field = |f: &str| items.iter().find(|(k, _)| key(k) == Some(f)).map(|(_, v)| v);
                let Some(entries) = field("entries") else {
                    let values: Vec<(String, String)> = items.iter().filter_map(|(k, v)| Some((key(k)?.to_string(), text(v)?.to_string()))).collect();
                    return Some(Arc::new(if values.is_empty() { Entry::Leaf } else { Entry::Values(values) }));
                };
                let ExprKind::Map(list) = &*entries.kind else { return None };
                let s = |f: &str| field(f).and_then(|v| text(v)).map(str::to_string);
                Some(Arc::new(Entry::Level(Level {
                    key: s("name").unwrap_or_default(),
                    name: s("displayName").unwrap_or_default(),
                    default: s("default"),
                    entries: list.iter().filter_map(|(k, v)| Some((key(k)?.to_string(), convert(v, consts, memo)?))).collect(),
                })))
            }
            _ => None,
        }
    }
    let src = noshape_featurescript::std_source("holetables.gen.fs").unwrap_or_default();
    let module = match noshape_featurescript::parser::parse(src) {
        Ok(m) => m,
        Err(e) => {
            log::error!("holetables.gen.fs: {e:?}");
            return HashMap::new();
        }
    };
    let consts: HashMap<&str, &Expr> = module
        .items
        .iter()
        .filter_map(|it| match &it.kind {
            TopKind::Const { name, init, .. } => Some((&**name, init)),
            _ => None,
        })
        .collect();
    let mut memo = HashMap::new();
    for name in consts.keys() {
        resolve(name, &consts, &mut memo);
    }
    // externalThread.fs's localExternalThreadTable; the generic hole table.
    let standards = |memo: &HashMap<String, Arc<Entry>>, ansi: &str, iso: &str| {
        Arc::new(Entry::Level(Level {
            key: "standard".into(),
            name: "Standard".into(),
            default: None,
            entries: [("ANSI", ansi), ("ISO", iso)].iter().filter_map(|(k, t)| Some((k.to_string(), memo.get(*t)?.clone()))).collect(),
        }))
    };
    let ext = standards(&memo, "ANSI_ExternalThreadTable", "ISO_ExternalThreadTable");
    let hole = standards(&memo, "ANSI_HoleTable", "ISO_HoleTable");
    memo.insert(TABLE_EXTERNAL_THREAD.into(), ext);
    memo.insert(TABLE_HOLE.into(), hole);
    memo
}

fn root(table: &str) -> Option<&'static Entry> {
    let name = match table {
        TABLE_ANSI_HOLE => "ANSI_HoleTable",
        TABLE_ANSI_HOLE_EX => "ANSI_HoleTableEx",
        TABLE_ISO_HOLE => "ISO_HoleTable",
        TABLE_ISO_HOLE_EX => "ISO_HoleTableEx",
        TABLE_ANSI_THREAD_CLASS => "ANSI_ThreadClassHoleTable",
        TABLE_ISO_THREAD_CLASS => "ISO_ThreadClassHoleTable",
        TABLE_EXTERNAL_THREAD | TABLE_HOLE => table,
        _ => return None,
    };
    TABLES.get(name).map(|e| &**e)
}

/// The option of `level` a path element names. Paths saved before the full
/// tables keep the bare pitch ("20 tpi" for "20 tpi (UNC)").
fn find<'a>(level: &'a Level, key: &str) -> Option<&'a (String, Arc<Entry>)> {
    level.entries.iter().find(|(k, _)| k == key).or_else(|| level.entries.iter().find(|(k, _)| k.strip_prefix(key).is_some_and(|r| r.starts_with(" ("))))
}

/// Follow `path` from a table's root: the entry reached and, per step, the
/// level and the option taken.
fn walk(table: &str, path: &[String]) -> Option<(&'static Entry, Vec<(&'static Level, &'static str)>)> {
    let mut e = root(table)?;
    let mut steps = Vec::new();
    for p in path {
        let Entry::Level(l) = e else { return None };
        let (k, next) = find(l, p)?;
        steps.push((l, k.as_str()));
        e = next;
    }
    Some((e, steps))
}

/// A table quantity ("0.0469 in", "1/64 in", "2 3/4 in", "82 degree") in
/// meters / radians.
fn quantity(s: &str) -> Option<f64> {
    let (num, unit) = s.trim().rsplit_once(' ')?;
    let scale = match unit {
        "in" => IN,
        "mm" => MM,
        "degree" => std::f64::consts::PI / 180.0,
        _ => return None,
    };
    Some(number(num)? * scale)
}

/// "0.25", "1/64", "2 3/4", "-1".
fn number(s: &str) -> Option<f64> {
    let frac = |f: &str| match f.split_once('/') {
        Some((a, b)) => Some(a.trim().parse::<f64>().ok()? / b.trim().parse::<f64>().ok()?),
        None => f.trim().parse::<f64>().ok(),
    };
    match s.trim().split_once(' ') {
        Some((whole, f)) => Some(whole.parse::<f64>().ok()? + frac(f)?),
        None => frac(s),
    }
}

/// Thread pitch of a pitch option (hole.fs `parsePitch`): "20 tpi (UNC)",
/// "1.00 mm (Coarse)", "11 1/2 tpi".
fn pitch(option: &str) -> Option<f64> {
    let words: Vec<&str> = option.split_whitespace().collect();
    let at = words.iter().position(|w| *w == "tpi" || *w == "mm")?;
    let n = number(&words[..at].join(" "))?;
    match words[at] {
        "tpi" if n > 0.0 => Some(IN / n),
        "mm" => Some(n * MM),
        _ => None,
    }
}

/// Leaf values as hole.fs `updateHoleDefinitionWithStandard` applies them: a
/// tap drill above the hole diameter, or a counterbore / countersink below
/// it (the tables' "-1"), becomes the hole diameter.
fn spec(values: &[(String, String)], steps: &[(&Level, &str)]) -> Option<HoleSpec> {
    let get = |k: &str| values.iter().find(|(n, _)| n == k).and_then(|(_, v)| quantity(v));
    let hole = get("holeDiameter")?;
    let pitch = steps.iter().find(|(l, _)| l.key == "pitch").and_then(|(_, k)| pitch(k));
    let at_least = |v: Option<f64>| v.map(|v| v.max(hole));
    Some(HoleSpec {
        hole_diameter: hole,
        tap_drill_diameter: get("tapDrillDiameter").map(|t| t.min(hole)).or(pitch.map(|_| hole)),
        major_diameter: get("majorDiameter"),
        minor_diameter: get("minorDiameter"),
        pitch,
        cbore_diameter: at_least(get("cBoreDiameter")),
        cbore_depth: get("cBoreDepth"),
        csink_diameter: at_least(get("cSinkDiameter")),
        csink_angle: get("cSinkAngle"),
        tapped_depth: get("tappedDepth"),
        hole_depth: get("holeDepth"),
    })
}

struct LevelInfo {
    name: &'static str,
    options: Vec<Opt>,
    default: String,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------
/// A lookup table registered at run time (a FeatureScript "Lookup Table"
/// annotation): one level per node.
#[derive(Clone, Debug, PartialEq)]
pub struct DynLevel {
    /// The level's key in a `LookupTablePath` (`"standard"`).
    pub key: String,
    /// Shown name (`"Standard"`).
    pub name: &'static str,
    pub default: String,
    /// (option, its sub-level; `None` for a leaf).
    pub entries: Vec<(String, Option<std::sync::Arc<DynLevel>>)>,
}

static DYNAMIC_TABLES: std::sync::LazyLock<parking_lot::RwLock<std::collections::HashMap<&'static str, std::sync::Arc<DynLevel>>>> = std::sync::LazyLock::new(Default::default);

/// Register (or replace) a run-time lookup table under `name`.
pub fn register_dynamic_table(name: &'static str, root: DynLevel) {
    DYNAMIC_TABLES.write().insert(name, std::sync::Arc::new(root));
}

/// The level reached by `path` in a run-time table.
pub fn dynamic_level(table: &str, path: &[String]) -> Option<std::sync::Arc<DynLevel>> {
    let mut level = DYNAMIC_TABLES.read().get(table)?.clone();
    for key in path {
        let next = level.entries.iter().find(|(k, _)| k == key)?.1.clone()?;
        level = next;
    }
    Some(level)
}

fn level_info(table: &str, path: &[String]) -> Option<LevelInfo> {
    if let Some(l) = dynamic_level(table, path) {
        return Some(LevelInfo { name: l.name, options: l.entries.iter().map(|(k, _)| (k.clone(), k.clone())).collect(), default: l.default.clone() });
    }
    let (Entry::Level(l), _) = walk(table, path)? else { return None };
    let options: Vec<Opt> = l.entries.iter().map(|(k, _)| (k.clone(), k.clone())).collect();
    // A level without a default (Hole type, Standard) starts on its first option.
    let default = l.default.clone().or_else(|| options.first().map(|o| o.0.clone()))?;
    Some(LevelInfo { name: &l.name, options, default })
}

/// Options of the level that follows `path` in the generic hole table
/// (`path[0]` is the standard, "ANSI" or "ISO"). Empty when `path` is a leaf
/// or invalid.
pub fn levels(path: &[String]) -> Vec<Opt> {
    table_levels(TABLE_HOLE, path)
}

/// Leaf values of a complete generic hole-table path (`path[0]` = standard).
pub fn values(path: &[String]) -> Option<HoleSpec> {
    table_values(TABLE_HOLE, path)
}

/// Options of the level following `path` in the named table.
pub fn table_levels(table: &str, path: &[String]) -> Vec<Opt> {
    level_info(table, path).map(|l| l.options).unwrap_or_default()
}

/// Display name of the level following `path` ("Hole type", "Size", ...).
pub fn table_level_name(table: &str, path: &[String]) -> Option<&'static str> {
    level_info(table, path).map(|l| l.name)
}

/// Default option of the level following `path`.
pub fn table_level_default(table: &str, path: &[String]) -> Option<String> {
    level_info(table, path).map(|l| l.default)
}

/// Leaf values of a complete path in the named table. Thread-class tables
/// have no dimensional values and always return `None`.
pub fn table_values(table: &str, path: &[String]) -> Option<HoleSpec> {
    match walk(table, path)? {
        (Entry::Values(v), steps) => spec(v, &steps),
        _ => None,
    }
}

/// Initialise a new Hole like Onshape: the default Standard path's values
/// fill the numeric fields, so the dialog's drill size and diameter agree
/// from the start. Units keep hole.fs's default, Inch, whatever the
/// workspace units (live: a mm document opens a new Hole on the Inch tab).
pub fn init_new_hole(params: &mut ParamValues) {
    sync_hole_standard(params);
    // The fields now hold the table values, but the hole is not marked as
    // synced: until the user edits it, a Standard path set later (by code, or
    // by a dialog that skips the edit logic) still takes the table's values.
    params.set(TABLE_SYNCED_PARAM, ParamValue::Bool(false));
}

/// Hole edit logic after the units system or style changed: the Standard
/// that becomes visible gets its default path if it has none, and its values
/// are copied into the numeric fields (so switching Metric to Inch does not
/// leave a metric diameter behind). Returns whether values were applied.
pub fn sync_hole_standard(params: &mut ParamValues) -> bool {
    let (pid, table) = hole_table_param(params);
    if params.lookup(pid).is_empty() {
        params.set(pid, ParamValue::Lookup(default_path(table)));
    }
    apply_table_to_params(params)
}

/// Path obtained by following every level's default from the top.
pub fn default_path(table: &str) -> Vec<String> {
    let mut path = Vec::new();
    while let Some(d) = table_level_default(table, &path) {
        path.push(d);
        if path.len() > 16 {
            break;
        }
    }
    path
}

/// Whether `path` is a complete, valid path of `table`.
pub fn is_complete(table: &str, path: &[String]) -> bool {
    match table {
        TABLE_ANSI_THREAD_CLASS | TABLE_ISO_THREAD_CLASS => path.len() == 1 && matches!(walk(table, path), Some((Entry::Leaf, _))),
        _ => table_values(table, path).is_some(),
    }
}

/// Thread standard implied by a hole path (FS `threadStandard`): "ANSI" or
/// "ISO" for straight-tapped holes, "UNSET" otherwise.
pub fn thread_standard(table: &str, path: &[String]) -> &'static str {
    let tapped = path.first().map(String::as_str) == Some("Tapped") || path.get(1).map(String::as_str) == Some("Tapped");
    if !tapped {
        return "UNSET";
    }
    match table {
        TABLE_ANSI_HOLE | TABLE_ANSI_HOLE_EX => "ANSI",
        TABLE_ISO_HOLE | TABLE_ISO_HOLE_EX => "ISO",
        TABLE_HOLE => match path.first().map(String::as_str) {
            Some("ANSI") => "ANSI",
            Some("ISO") => "ISO",
            _ => "UNSET",
        },
        _ => "UNSET",
    }
}

// ---------------------------------------------------------------------------
// Hole dialog edit logic
// ---------------------------------------------------------------------------

/// Numeric Hole parameters driven by the Standard table.
pub const HOLE_TABLE_DIMENSIONS: &[&str] = &["holeDiameterV3", "tapDrillDiameterV3", "cBoreDiameterV3", "cBoreDepthV3", "cSinkDiameterV3", "cSinkAngleV3"];

/// Hidden Hole parameter set by [`apply_table_to_params`]: the numeric
/// parameters hold the table values (or the user's later edits) and the
/// evaluator uses them as they are.
pub const TABLE_SYNCED_PARAM: &str = "tableSynced";

/// The Standard lookup parameter of a Hole that is visible for its units and
/// style, as (parameter id, table name).
pub fn hole_table_param(params: &ParamValues) -> (&'static str, &'static str) {
    match (params.is("unitsSystem", "INCH"), params.is("styleV2", "SIMPLE")) {
        (true, true) => ("ansiHoleTableEx", TABLE_ANSI_HOLE_EX),
        (true, false) => ("ansiHoleTable", TABLE_ANSI_HOLE),
        (false, true) => ("isoHoleTableEx", TABLE_ISO_HOLE_EX),
        (false, false) => ("isoHoleTable", TABLE_ISO_HOLE),
    }
}

/// `path` with each element as the table's option it names (see [`find`]);
/// unknown elements are kept as they are.
pub fn canonical_path(table: &str, path: &[String]) -> Vec<String> {
    match walk(table, path) {
        Some((_, steps)) => steps.iter().map(|(_, k)| k.to_string()).collect(),
        None => path.to_vec(),
    }
}

/// Complete a (possibly partial) table path with each level's default.
pub fn complete_path(table: &str, path: &[String]) -> Vec<String> {
    let mut p = canonical_path(table, path);
    while let Some(d) = table_level_default(table, &p) {
        p.push(d);
        if p.len() > 16 {
            break;
        }
    }
    p
}

fn fmt_num(x: f64, decimals: usize) -> String {
    let s = format!("{x:.decimals$}");
    let s = if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s };
    if s == "-0" { "0".into() } else { s }
}

/// Hole edit logic (Onshape's behavior when the Standard changes): copy the
/// table values of the Hole's current Standard path into its numeric
/// parameters, so the dialog shows them and later edits start from them.
///
/// Call it from the Hole dialog whenever the visible Standard lookup
/// (see [`hole_table_param`]), the units system or the style changes; do not
/// call it when the user edits a numeric parameter (edits win until the next
/// Standard change). Sets: `holeDiameterV3`, `tapDrillDiameterV3`,
/// `cBoreDiameterV3`, `cBoreDepthV3`, `cSinkDiameterV3`, `cSinkAngleV3`
/// (as literal expressions in mm / in and degrees), the hidden
/// `majorDiameter`, `threadStandard`, `showTappedDepth`, `hasClearance` and
/// [`TABLE_SYNCED_PARAM`]. Returns false (and changes nothing) when no
/// Standard is selected or its path has no values.
pub fn apply_table_to_params(params: &mut ParamValues) -> bool {
    let (pid, table) = hole_table_param(params);
    let raw = params.lookup(pid).to_vec();
    if raw.is_empty() {
        return false;
    }
    let path = complete_path(table, &raw);
    let Some(spec) = table_values(table, &path) else { return false };
    let inch = params.is("unitsSystem", "INCH");
    let len = |m: f64| ParamValue::Expr(ExprValue::new(if inch { format!("{} in", fmt_num(m / IN, 5)) } else { format!("{} mm", fmt_num(m * 1000.0, 4)) }));
    params.set("holeDiameterV3", len(spec.hole_diameter));
    if let Some(td) = spec.tap_drill_diameter {
        params.set("tapDrillDiameterV3", len(td));
    }
    if let Some(v) = spec.cbore_diameter {
        params.set("cBoreDiameterV3", len(v));
    }
    if let Some(v) = spec.cbore_depth {
        params.set("cBoreDepthV3", len(v));
    }
    if let Some(v) = spec.csink_diameter {
        params.set("cSinkDiameterV3", len(v));
    }
    if let Some(a) = spec.csink_angle {
        params.set("cSinkAngleV3", ParamValue::Expr(ExprValue::new(format!("{} deg", fmt_num(a.to_degrees(), 4)))));
    }
    if let Some(v) = spec.major_diameter {
        params.set("majorDiameter", len(v));
    }
    // Tapered pipe taps carry their depths.
    if let Some(v) = spec.tapped_depth {
        params.set("tappedDepthV3", len(v));
    }
    if let Some(v) = spec.hole_depth {
        params.set("holeDepthV3", len(v));
    }
    params.set("threadStandard", ParamValue::Enum(thread_standard(table, &path).into()));
    params.set("showTappedDepth", ParamValue::Bool(spec.is_threaded()));
    params.set("hasClearance", ParamValue::Bool(spec.tap_drill_diameter.is_some_and(|t| (t - spec.hole_diameter).abs() > 1e-12)));
    params.set(TABLE_SYNCED_PARAM, ParamValue::Bool(true));
    true
}

/// Size whose major diameter matches `diameter` (m) within `tol` (external
/// thread auto-sizing). Returns the default path for that size.
/// Onshape's hole edit logic for a blind tapped hole (live L49/L50): Depth,
/// Tapped depth and Tap clearance stay linked, Depth = Tapped depth + Tap
/// clearance × pitch. A new size sets the clearance to 0 (Depth = Tapped
/// depth); a new Depth moves the clearance (or the tapped depth when the
/// Depth is less than it); a new clearance moves the tapped depth; a new
/// tapped depth moves the Depth. Returns the parameters to change.
pub fn link_tapped_depths(prev: &ParamValues, now: &ParamValues, units: &noshape_units::UnitPrefs) -> Vec<(&'static str, ParamValue)> {
    use noshape_units::{Dim, Quantity};
    if !now.bool("showTappedDepth") || !now.is("endStyleV2", "BLIND") {
        return Vec::new();
    }
    let (pid, table) = hole_table_param(now);
    let Some(pitch) = table_values(table, &complete_path(table, now.lookup(pid))).and_then(|s| s.pitch).filter(|p| *p > 0.0) else { return Vec::new() };
    let empty = std::collections::BTreeMap::<String, Quantity>::new();
    let val = |v: &ParamValues, id: &str, dim: Dim| -> Option<f64> {
        let e = v.expr(id)?;
        e.cached.map(|q| q.value).or_else(|| noshape_units::evaluate_as(&e.text, dim, units, &empty).ok().map(|q| q.value))
    };
    let (Some(depth), Some(tapped), Some(clear)) = (val(now, "holeDepthV3", Dim::LENGTH), val(now, "tappedDepthV3", Dim::LENGTH), val(now, "tapClearance", Dim::UNITLESS)) else { return Vec::new() };
    let changed = |id: &str, dim: Dim| match (val(prev, id, dim), val(now, id, dim)) {
        (Some(a), Some(b)) => (a - b).abs() > 1e-12,
        _ => false,
    };
    let len = |m: f64| ParamValue::Expr(ExprValue::new(noshape_units::format_quantity(Quantity::new(m.max(0.0), Dim::LENGTH), units)));
    let count = |c: f64| ParamValue::Expr(ExprValue::new(noshape_units::format_number(c.max(0.0), 6)));
    let close = |a: f64, b: f64| (a - b).abs() < 1e-9;
    if prev.lookup(pid) != now.lookup(pid) {
        let mut out = Vec::new();
        if !close(clear, 0.0) {
            out.push(("tapClearance", count(0.0)));
        }
        if !close(depth, tapped) {
            out.push(("holeDepthV3", len(tapped)));
        }
        return out;
    }
    // std hole.fs adjustDepthAndThreadParameters: of the two fields the
    // user isn't editing, the later one in (Depth, Tapped depth, Tap
    // clearance) whose new value stays positive (the clearance may be 0);
    // none: nothing changes (std then warns HOLE_INCONSISTENT_TAP_INFO).
    let candidates = [
        ("holeDepthV3", tapped + clear * pitch, false, Dim::LENGTH),
        ("tappedDepthV3", depth - clear * pitch, false, Dim::LENGTH),
        ("tapClearance", (depth - tapped) / pitch, true, Dim::UNITLESS),
    ];
    let edited: Vec<&str> = candidates.iter().filter(|c| changed(c.0, c.3)).map(|c| c.0).collect();
    if edited.is_empty() {
        return Vec::new();
    }
    let pick = candidates.iter().rfind(|c| !edited.contains(&c.0) && (c.1 > 0.0 || (c.2 && c.1 >= 0.0)));
    match pick {
        Some(&(id, v, _, dim)) if dim == Dim::LENGTH => vec![(id, len(v))],
        Some(&(id, v, _, _)) => vec![(id, count(v))],
        None => Vec::new(),
    }
}

pub fn external_thread_for_diameter(standard_id: &str, diameter: f64, tol: f64) -> Option<Vec<String>> {
    let std = vec![standard_id.to_string()];
    let (Entry::Level(sizes), _) = walk(TABLE_EXTERNAL_THREAD, &std)? else { return None };
    sizes.entries.iter().find_map(|(size, _)| {
        let path = complete_path(TABLE_EXTERNAL_THREAD, &[standard_id.to_string(), size.clone()]);
        let major = table_values(TABLE_EXTERNAL_THREAD, &path)?.major_diameter?;
        ((major - diameter).abs() <= tol).then_some(path)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn default_paths_are_complete() {
        for t in ALL_TABLES {
            let path = default_path(t);
            assert!(!path.is_empty(), "{t}");
            assert!(is_complete(t, &path), "{t}: {path:?}");
        }
        assert_eq!(default_path(TABLE_ANSI_HOLE), p(&["Drilled", "1/4 (0.25)"]));
        assert_eq!(default_path(TABLE_EXTERNAL_THREAD), p(&["ANSI", "1/4", "20 tpi"]));
    }

    #[test]
    fn every_level_default_is_an_option() {
        // Walk every path of every table (bounded) and check defaults.
        fn walk(table: &str, path: &mut Vec<String>, budget: &mut usize) {
            let Some(info) = level_info(table, path) else {
                assert!(is_complete(table, path), "{table}: dead end {path:?}");
                return;
            };
            assert!(info.options.iter().any(|o| o.0 == info.default), "{table} {path:?}: default {}", info.default);
            for (id, _) in info.options {
                if *budget == 0 {
                    return;
                }
                *budget -= 1;
                path.push(id);
                walk(table, path, budget);
                path.pop();
            }
        }
        for t in ALL_TABLES {
            let mut budget = 20_000;
            walk(t, &mut Vec::new(), &mut budget);
        }
    }

    #[test]
    fn table_values_copied_into_hole_params() {
        let mut v = ParamValues::default();
        v.set("unitsSystem", ParamValue::Enum("METRIC".into()));
        v.set("styleV2", ParamValue::Enum("C_BORE".into()));
        assert!(!apply_table_to_params(&mut v), "no standard selected");
        v.set("isoHoleTable", ParamValue::Lookup(p(&["Tapped", "Straight tap", "M5"])));
        assert!(apply_table_to_params(&mut v));
        assert_eq!(v.expr("holeDiameterV3").unwrap().text, "4.2 mm");
        assert_eq!(v.expr("tapDrillDiameterV3").unwrap().text, "4.2 mm");
        assert_eq!(v.expr("cBoreDiameterV3").unwrap().text, "9.75 mm");
        assert_eq!(v.enum_("threadStandard"), "ISO");
        assert!(v.bool("showTappedDepth") && !v.bool("hasClearance") && v.bool(TABLE_SYNCED_PARAM));
        // Inch clearance hole with a fit.
        let mut v = ParamValues::default();
        v.set("unitsSystem", ParamValue::Enum("INCH".into()));
        v.set("styleV2", ParamValue::Enum("SIMPLE".into()));
        v.set("ansiHoleTableEx", ParamValue::Lookup(p(&["Tapped", "Straight tap", "1/4", "20 tpi", "Close"])));
        assert!(apply_table_to_params(&mut v));
        assert_eq!(v.expr("holeDiameterV3").unwrap().text, "0.257 in");
        assert_eq!(v.expr("tapDrillDiameterV3").unwrap().text, "0.201 in");
        assert!(v.bool("hasClearance"));
    }

    #[test]
    fn known_values() {
        let m5 = values(&p(&["ISO", "Tapped", "Straight tap", "M5", "0.80 mm", "None"])).unwrap();
        assert!((m5.hole_diameter - 4.2e-3).abs() < 1e-9);
        assert!((m5.pitch.unwrap() - 0.8e-3).abs() < 1e-12);
        assert!((m5.cbore_diameter.unwrap() - 9.75e-3).abs() < 1e-9);
        let q = values(&p(&["ANSI", "Tapped", "Straight tap", "1/4", "20 tpi", "Close"])).unwrap();
        assert!((q.hole_diameter - 0.257 * IN).abs() < 1e-9);
        assert!((q.tap_drill_diameter.unwrap() - 0.201 * IN).abs() < 1e-9);
        assert_eq!(thread_standard(TABLE_ANSI_HOLE, &p(&["Tapped", "Straight tap"])), "ANSI");
        assert_eq!(thread_standard(TABLE_ISO_HOLE, &p(&["Clearance", "M5"])), "UNSET");
        let ext = external_thread_for_diameter("ISO", 8e-3, 1e-6).unwrap();
        assert_eq!(ext, p(&["ISO", "M8", "1.25 mm"]));
        assert!(table_values(TABLE_EXTERNAL_THREAD, &ext).unwrap().minor_diameter.unwrap() < 8e-3);
    }

    /// Live L58: the dropdowns list the whole standard in Onshape's order.
    #[test]
    fn lists_in_live_order() {
        let ids = |t: &str, path: &[&str]| table_levels(t, &p(path)).into_iter().map(|o| o.0).collect::<Vec<_>>();
        assert_eq!(ids(TABLE_ANSI_HOLE_EX, &[]), p(&["Drilled", "Clearance", "Tapped", "PEM®"]));
        assert_eq!(ids(TABLE_ANSI_HOLE, &[]), p(&["Drilled", "Clearance", "Tapped"]));
        let sizes = ids(TABLE_ISO_HOLE, &["Tapped", "Straight tap"]);
        assert_eq!(sizes[..8], p(&["M1", "M1.1", "M1.2", "M1.4", "M1.6", "M1.8", "M2", "M2.2"]));
        assert_eq!(sizes.len(), 42);
        assert_eq!(sizes.last().unwrap(), "M68");
        assert_eq!(ids(TABLE_ISO_HOLE, &["Tapped", "Straight tap", "M6"]), p(&["0.75 mm (Fine)", "1.00 mm (Coarse)"]));
        assert_eq!(default_path(TABLE_ISO_HOLE_EX).len(), 2);
        assert_eq!(complete_path(TABLE_ISO_HOLE, &p(&["Tapped", "Straight tap", "M6"]))[3], "1.00 mm (Coarse)");
        let ansi = ids(TABLE_ANSI_HOLE, &["Tapped", "Straight tap"]);
        assert_eq!(ansi[..12], p(&["#0", "#1", "#2", "#3", "#4", "#5", "#6", "#8", "#10", "#12", "1/4", "5/16"]));
        assert!(ansi.contains(&"1 1/16".to_string()) && ansi.last().unwrap() == "6");
        assert_eq!(ids(TABLE_ANSI_HOLE, &["Tapped", "Straight tap", "1/4"]), p(&["20 tpi (UNC)", "24 tpi", "27 tpi", "28 tpi (UNF)", "32 tpi (UNEF)", "36 tpi", "40 tpi", "48 tpi", "56 tpi"]));
        let drills = ids(TABLE_ANSI_HOLE, &["Drilled"]);
        assert_eq!(drills[..3], p(&["#80 (0.0135)", "#79 (0.0145)", "1/64 (0.015625)"]));
        let at = drills.iter().position(|d| d == "1/4 (0.25)").unwrap();
        assert_eq!(drills[at - 1..at + 3], p(&["D (0.246)", "1/4 (0.25)", "E (0.25)", "F (0.257)"]));
        assert_eq!(drills.last().unwrap(), "1 (1.0)");
        let metric = ids(TABLE_ISO_HOLE, &["Drilled"]);
        assert_eq!(metric[..3], p(&["0.05", "0.1", "0.2"]));
        let ten = metric.iter().position(|d| d == "10").unwrap();
        assert_eq!(metric[ten + 1], "10.5");
        // Paths saved with the bare pitch still resolve.
        assert_eq!(table_values(TABLE_ISO_HOLE, &p(&["Tapped", "Straight tap", "M6", "1.00 mm", "None"])), table_values(TABLE_ISO_HOLE, &p(&["Tapped", "Straight tap", "M6", "1.00 mm (Coarse)", "None"])));
        let m6 = table_values(TABLE_ISO_HOLE, &p(&["Tapped", "Straight tap", "M6", "0.75 mm (Fine)", "None"])).unwrap();
        assert!((m6.pitch.unwrap() - 0.75e-3).abs() < 1e-12 && (m6.tap_drill_diameter.unwrap() - 5.25e-3).abs() < 1e-9);
        let d = table_values(TABLE_ANSI_HOLE, &p(&["Drilled", "1/64 (0.015625)"])).unwrap();
        assert!((d.hole_diameter - IN / 64.0).abs() < 1e-12 && d.pitch.is_none());
    }
}
