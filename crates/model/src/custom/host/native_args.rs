//! Every `@native(…)` call in the standard library against the native:
//! where std passes a map (`@alignCanonically(context, { "plane" : plane })`,
//! or a map parameter such as `@evArea(context, arg)`), the native must read
//! that map's keys, not take the argument as the value itself (helix() from a
//! circle failed with "argument 2 should be a Plane").

use super::{FsContext, convert::id_value};
use crate::reference::FeatureId;
use crate::regen::context::Context;
use noshape_featurescript::interp::Interp;
use noshape_featurescript::value::{self, Map, Value};
use noshape_units::UnitPrefs;
use std::collections::BTreeSet;
use std::sync::Arc;

/// An argument of a call site: a map literal's keys (true) or the keys std
/// reads from a map parameter it passes on (false), or another expression.
#[derive(Debug)]
enum Arg {
    Map(Vec<String>, bool),
    Expr(String),
}

#[derive(Debug)]
struct Site {
    file: &'static str,
    line: usize,
    name: String,
    args: Vec<Arg>,
}

/// The source without comments (strings kept as they are).
fn strip_comments(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < b.len() {
        if s[i..].starts_with("//") {
            i = s[i..].find('\n').map_or(b.len(), |j| i + j);
        } else if s[i..].starts_with("/*") {
            let end = s[i + 2..].find("*/").map_or(b.len(), |j| i + 2 + j + 2);
            // Keep the line count.
            out.extend(s[i..end].chars().filter(|c| *c == '\n'));
            i = end;
        } else if b[i] == b'"' {
            let j = string_end(b, i);
            out.push_str(&s[i..j]);
            i = j;
        } else {
            let c = s[i..].chars().next().unwrap();
            out.push(c);
            i += c.len_utf8();
        }
    }
    out
}

/// The index after the string literal starting at `i`.
fn string_end(b: &[u8], i: usize) -> usize {
    let mut j = i + 1;
    while j < b.len() && b[j] != b'"' {
        j += if b[j] == b'\\' { 2 } else { 1 };
    }
    (j + 1).min(b.len())
}

/// The top-level comma-separated parts of `s[open + 1..]` up to the bracket
/// closing `s[open]`.
fn split_top(s: &str, open: usize) -> Vec<String> {
    let b = s.as_bytes();
    let (mut depth, mut parts, mut start, mut j) = (0, Vec::new(), open + 1, open + 1);
    while j < b.len() {
        match b[j] {
            b'"' => {
                j = string_end(b, j);
                continue;
            }
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' if depth == 0 => {
                if !s[start..j].trim().is_empty() {
                    parts.push(s[start..j].trim().to_string());
                }
                return parts;
            }
            b')' | b']' | b'}' => depth -= 1,
            b',' if depth == 0 => {
                parts.push(s[start..j].trim().to_string());
                start = j + 1;
            }
            _ => {}
        }
        j += 1;
    }
    parts
}

/// The keys of a map literal `{ "a" : …, b : … }`.
fn literal_keys(a: &str) -> Option<Vec<String>> {
    if !a.starts_with('{') {
        return None;
    }
    let keys = split_top(a, 0)
        .iter()
        .filter_map(|p| {
            let (k, _) = p.split_once(':')?;
            Some(k.trim().trim_matches('"').to_string())
        })
        .collect();
    Some(keys)
}

fn is_ident(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

/// `name.key` and `name["key"]` in `body`.
fn keys_used(body: &str, name: &str) -> Vec<String> {
    let b = body.as_bytes();
    let mut keys = BTreeSet::new();
    for (at, _) in body.match_indices(name) {
        let end = at + name.len();
        if at > 0 && (is_ident(b[at - 1]) || b[at - 1] == b'.') || end >= b.len() {
            continue;
        }
        let rest = &body[end..];
        let key = if let Some(r) = rest.strip_prefix('.') {
            r.bytes().take_while(|c| is_ident(*c)).map(char::from).collect::<String>()
        } else if let Some(r) = rest.strip_prefix("[\"") {
            r.split('"').next().unwrap_or("").to_string()
        } else {
            continue;
        };
        if !key.is_empty() {
            keys.insert(key);
        }
    }
    keys.into_iter().collect()
}

/// Every native call in std with its arguments.
fn std_sites() -> Vec<Site> {
    let mut sites = Vec::new();
    for (file, src) in noshape_featurescript::STD_SOURCES {
        let s = strip_comments(src);
        let b = s.as_bytes();
        // Starts of functions (and their parameter lists) for map parameters.
        let mut funcs: Vec<(usize, String)> = Vec::new();
        for kw in ["function", "predicate"] {
            for (at, _) in s.match_indices(kw) {
                if at > 0 && is_ident(b[at - 1]) || b.get(at + kw.len()).is_some_and(|c| is_ident(*c)) {
                    continue;
                }
                if let Some(open) = s[at..].find('(').map(|o| at + o) {
                    funcs.push((at, split_top(&s, open).join(",")));
                }
            }
        }
        funcs.sort();
        let mut i = 0;
        while let Some(at) = s[i..].find('@').map(|o| i + o) {
            i = at + 1;
            let name: String = s[at + 1..].bytes().take_while(|c| is_ident(*c)).map(char::from).collect();
            let open = at + 1 + name.len();
            if name.is_empty() || b.get(open) != Some(&b'(') {
                continue;
            }
            let (start, params) = funcs.iter().rev().find(|(f, _)| *f < at).map_or((0, ""), |(f, p)| (*f, p.as_str()));
            let map_params: Vec<&str> = params.split(',').filter_map(|p| p.trim().strip_suffix(" is map")).map(str::trim).collect();
            let args = split_top(&s, open)
                .into_iter()
                .map(|a| match literal_keys(&a) {
                    Some(k) => Arg::Map(k, true),
                    None if map_params.contains(&a.as_str()) => Arg::Map(keys_used(&s[start..at], &a), false),
                    None => Arg::Expr(a),
                })
                .collect();
            sites.push(Site { file, line: s[..at].matches('\n').count() + 1, name, args });
        }
    }
    sites
}

/// Natives that take std's map but need none of it: bookkeeping and display
/// natives kept as no-ops, constants, and core functions that take the map
/// whole (`size`, `mergeMaps`' defaults).
const IGNORES_MAP: &[&str] = &[
    // No-ops (display, history and pattern bookkeeping).
    "setErrorEntities",
    "transferSubfeatureErrorDisplay",
    "setFeatureComputedParameter",
    "setFeatureHiddenParameters",
    "setExternalDisambiguation",
    "setHighlightedEntities",
    "setPatternData",
    "setFeaturePatternInstanceData",
    "setDimensionedEntities",
    "addManipulators",
    "addAuxiliaryEntities",
    "addReferenceCSysFrame",
    "clampContextVersion",
    "recordQuery",
    "skSetInitialGuess",
    // Constants: no tolerances, no flattened sheet metal, no feature pattern.
    "getParameterToleranceInfo",
    "getTolerantParameterIds",
    "queryContainsFlattenedSheetMetal",
    "getRemainderPatternTransform",
    // The feature's definition is not needed to start, end or abort it.
    "startFeature",
    "endFeature",
    "abortFeature",
    "clearFeatureStatus",
    // The map as a whole.
    "size",
    "mergeMaps",
];

/// Natives std calls that are not implemented (whatever their arguments).
const MISSING: &[&str] = &[
    "evApproximateBSplineSurface",
    "evFaceCurvatureDerivatives",
    "evFaults",
    "evMaxTolerance",
    "evSheetMetalBendUp",
    "evSheetMetalFormToolBodies",
    "evSheetMetalHoleToolBodies",
    "evTolerances",
    "getHoleAttributes",
    "opBooleanedPattern",
    "opCreateBSplineSurface",
    "opCreateOutline",
    "opDerip",
    "opImportForeign",
    "opMoveCurveBoundary",
    "opOffsetCurveOnFace",
    "opSMFlatOperation",
    "opSplitBySelfShadow",
];

/// A probe value for an argument std passes as `expr` (`args`: the call's
/// arguments, for a sketch's entity ids).
fn probe_value(cx: &Arc<FsContext>, expr: &str, args: &[Arg]) -> Value {
    let id = |n: &str| id_value(&[Arc::from(n)]);
    let on_sketch = matches!(args.first(), Some(Arg::Expr(e)) if e == "sketch");
    match expr {
        "context" => cx.value(),
        "sketch" => {
            let plane = super::convert::plane_value(&super::sketch::canonical_frame(glam::DVec3::ZERO, glam::DVec3::Z));
            let def = Value::map_from([("sketchPlane", plane)]);
            noshape_featurescript::interp::native("newSketch").and_then(|f| f(&mut Interp::new(), vec![cx.value(), id("sketch"), def]).ok()).unwrap_or_default()
        }
        // A sketch entity's id, a JSON string.
        e if on_sketch && e.ends_with("Id") => Value::str(e),
        "s" => Value::str("{}"),
        e if e == "id" || e.starts_with("id +") || e.ends_with("Id") => id(e),
        _ => Value::Undefined,
    }
}

/// Std's map arguments a native does not read as maps: (site, argument
/// number, what the native did instead), the natives std calls that do not
/// exist, and the keys of maps natives read only in part.
fn unread_maps() -> (Vec<String>, BTreeSet<String>, Vec<String>) {
    super::register();
    let mut problems = Vec::new();
    let mut missing = BTreeSet::new();
    let mut unread_keys = Vec::new();
    for site in std_sites() {
        let Some(f) = noshape_featurescript::interp::native(&site.name) else {
            missing.insert(site.name.clone());
            continue;
        };
        if !site.args.iter().any(|a| matches!(a, Arg::Map(..))) || IGNORES_MAP.contains(&site.name.as_str()) {
            continue;
        }
        let cx = FsContext::new(Context::default(), FeatureId(1), "probe".into(), UnitPrefs::default(), Default::default(), None);
        let mut maps = Vec::new();
        let args: Vec<Value> = site
            .args
            .iter()
            .enumerate()
            .map(|(n, a)| match a {
                Arg::Map(keys, literal) => {
                    let mut m = Map::new();
                    for k in keys {
                        m.insert(Value::str(k.as_str()), Value::Undefined);
                    }
                    let v = Value::map(m);
                    maps.push((n, keys.clone(), *literal, v.as_map().map(|m| Arc::as_ptr(m) as usize).unwrap_or_default()));
                    v
                }
                Arg::Expr(e) => probe_value(&cx, e, &site.args),
            })
            .collect();
        value::record_key_reads();
        let out = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(&mut Interp::new(), args)));
        let reads = value::take_key_reads();
        let outcome = match &out {
            Ok(Ok(_)) => "returned".to_string(),
            Ok(Err(e)) => format!("threw {:?}", e.message()),
            Err(_) => "panicked".to_string(),
        };
        for (n, keys, literal, ptr) in maps {
            let read: BTreeSet<&str> = reads.iter().filter(|(p, _)| *p == ptr).map(|(_, k)| k.as_str()).collect();
            if read.is_empty() {
                problems.push(format!("{}:{} @{} argument {}: map {keys:?} not read; {outcome}", site.file, site.line, site.name, n + 1));
            } else if literal && !keys.is_empty() && !keys.iter().any(|k| k.starts_with('(')) && !read.contains("*") && !keys.iter().any(|k| read.contains(k.as_str())) {
                // Only keys std does not pass (a misspelt key).
                problems.push(format!("{}:{} @{} argument {}: reads {read:?}, std passes {keys:?}; {outcome}", site.file, site.line, site.name, n + 1));
            } else if !read.contains("*") {
                let missed: Vec<&String> = keys.iter().filter(|k| !read.contains(k.as_str())).collect();
                if !missed.is_empty() {
                    unread_keys.push(format!("{}:{} @{} argument {}: {missed:?} not read (read {read:?}); {outcome}", site.file, site.line, site.name, n + 1));
                }
            }
        }
    }
    (problems, missing, unread_keys)
}

#[test]
fn natives_read_the_maps_std_passes() {
    let (problems, missing, unread_keys) = unread_maps();
    if std::env::var_os("NATIVE_ARGS_REPORT").is_some() {
        for k in &unread_keys {
            eprintln!("keys: {k}");
        }
    }
    assert!(problems.is_empty(), "{} std calls whose map the native does not read:\n{}", problems.len(), problems.join("\n"));
    let known: BTreeSet<String> = MISSING.iter().map(|s| s.to_string()).collect();
    assert_eq!(missing, known, "natives std calls that do not exist (update MISSING)");
}
