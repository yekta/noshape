//! Runtime builtins that do not need the modeling kernel: math, strings,
//! containers, regular expressions, JSON, printing.

use crate::interp::{Interp, NativeFn, NoticeKind, R};
use crate::value::{Map, Tag, Value, format_number};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock};

pub fn core(name: &str) -> Option<NativeFn> {
    Some(match name {
        "size" => size,
        "resize" => resize,
        "subArray" => sub_array,
        "concatenateArrays" => concatenate_arrays,
        "indexOf" => index_of,
        "keys" => keys,
        "values" => values,
        "mergeMaps" => merge_maps,
        "intersectMaps" => intersect_maps,
        "reverse" => reverse,
        "range" => range,
        "tolerantSort" => tolerant_sort,
        "evaluateSpline" => crate::spline::evaluate_spline,
        "startTimer" => start_timer,
        "printTimer" => print_timer,
        "sqrt" => |i, a| math1(i, a, "sqrt", f64::sqrt),
        "log" => |i, a| math1(i, a, "log", f64::ln),
        "log10" => |i, a| math1(i, a, "log10", f64::log10),
        "exp" => |i, a| math1(i, a, "exp", f64::exp),
        "exp2" => |i, a| math1(i, a, "exp2", f64::exp2),
        "sin" => |i, a| math1(i, a, "sin", f64::sin),
        "cos" => |i, a| math1(i, a, "cos", f64::cos),
        "tan" => |i, a| math1(i, a, "tan", f64::tan),
        "asin" => |i, a| math1(i, a, "asin", f64::asin),
        "acos" => |i, a| math1(i, a, "acos", f64::acos),
        "atan" => |i, a| math1(i, a, "atan", f64::atan),
        "sinh" => |i, a| math1(i, a, "sinh", f64::sinh),
        "cosh" => |i, a| math1(i, a, "cosh", f64::cosh),
        "tanh" => |i, a| math1(i, a, "tanh", f64::tanh),
        "asinh" => |i, a| math1(i, a, "asinh", f64::asinh),
        "acosh" => |i, a| math1(i, a, "acosh", f64::acosh),
        "atanh" => |i, a| math1(i, a, "atanh", f64::atanh),
        "floor" => |i, a| math1(i, a, "floor", f64::floor),
        "ceil" => |i, a| math1(i, a, "ceil", f64::ceil),
        "abs" => |i, a| math1(i, a, "abs", f64::abs),
        "atan2" => |i, a| math2(i, a, "atan2", f64::atan2),
        "hypot" => |i, a| math2(i, a, "hypot", f64::hypot),
        "pow" => |i, a| math2(i, a, "pow", f64::powf),
        "print" => print,
        "report" => report,
        "splitIntoCharacters" => split_chars,
        "length" => length,
        "substring" => substring,
        "startsWith" => |i, a| str2(i, a, |s, p| s.starts_with(p)),
        "endsWith" => |i, a| str2(i, a, |s, p| s.ends_with(p)),
        "indexOfString" => index_of_string,
        "indexOfRegexp" => index_of_regexp,
        "repeatString" => repeat_string,
        "stringToNumber" => string_to_number,
        "match" => regex_match,
        "replace" => regex_replace,
        "splitByRegexp" => split_by_regexp,
        "parseJson" => parse_json,
        "getLanguageVersion" => |_, _| Ok(Value::num(crate::LANGUAGE_VERSION as f64)),
        "isMatrix" => is_matrix,
        "matrixIdentity" => matrix_identity,
        "matrixSum" => |i, a| matrix_cwise(i, a, |x, y| x + y),
        "matrixDifference" => |i, a| matrix_cwise(i, a, |x, y| x - y),
        "matrixCwiseProduct" => |i, a| matrix_cwise(i, a, |x, y| x * y),
        "matrixNegate" => matrix_negate,
        "matrixMultiply" => matrix_multiply,
        "matrixTranspose" => matrix_transpose,
        "matrixInverse" => matrix_inverse,
        "matrixSquaredNorm" => matrix_squared_norm,
        "matrixDeterminant" => matrix_determinant,
        "matrixSvd" => matrix_svd,
        "matrixRotation3d" => matrix_rotation_3d,
        "normalize" => normalize,
        "clusterPoints" => cluster_points,
        _ => return None,
    })
}

// ---------------------------------------------------------------- argument helpers

pub fn arg<'a>(i: &mut Interp, a: &'a [Value], n: usize) -> R<&'a Value> {
    match a.get(n) {
        Some(v) => Ok(v),
        None => Err(i.error(format!("Missing argument {}", n + 1))),
    }
}

pub fn num_arg(i: &mut Interp, a: &[Value], n: usize) -> R<f64> {
    match a.get(n) {
        Some(Value::Number(x, _)) => Ok(*x),
        Some(other) => Err(i.error(format!("Argument {} should be a number, was {}", n + 1, other.type_description()))),
        None => Err(i.error(format!("Missing argument {}", n + 1))),
    }
}

pub fn str_arg(i: &mut Interp, a: &[Value], n: usize) -> R<Arc<str>> {
    match a.get(n).and_then(|v| v.as_str()) {
        Some(s) => Ok(s),
        None => Err(i.error(format!("Argument {} should be a string", n + 1))),
    }
}

pub fn array_arg(i: &mut Interp, a: &[Value], n: usize) -> R<Arc<Vec<Value>>> {
    match a.get(n) {
        Some(Value::Array(x, _)) => Ok(x.clone()),
        Some(other) => Err(i.error(format!("Argument {} should be an array, was {}", n + 1, other.type_description()))),
        None => Err(i.error(format!("Missing argument {}", n + 1))),
    }
}

pub fn map_arg(i: &mut Interp, a: &[Value], n: usize) -> R<Arc<Map>> {
    match a.get(n) {
        Some(Value::Map(x, _)) => {
            crate::value::note_key_read(x, "*");
            Ok(x.clone())
        }
        Some(other) => Err(i.error(format!("Argument {} should be a map, was {}", n + 1, other.type_description()))),
        None => Err(i.error(format!("Missing argument {}", n + 1))),
    }
}

fn int_arg(i: &mut Interp, a: &[Value], n: usize) -> R<i64> {
    let x = num_arg(i, a, n)?;
    if x.fract() != 0.0 {
        return Err(i.error(format!("Argument {} should be an integer, was {}", n + 1, format_number(x))));
    }
    Ok(x as i64)
}

fn finite(i: &mut Interp, name: &str, x: f64) -> R<Value> {
    if x.is_nan() {
        return Err(i.error(format!("@{name}: Invalid number (NaN)")));
    }
    Ok(Value::num(x))
}

// ---------------------------------------------------------------- containers

fn size(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    Ok(Value::num(match arg(i, &a, 0)? {
        Value::Array(x, _) => x.len(),
        Value::Map(m, _) => m.len(),
        Value::Str(s, _) => s.chars().count(),
        other => return Err(i.error(format!("Cannot take the size of a value of type {}", other.type_description()))),
    } as f64))
}

fn resize(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let (arr, tag) = match arg(i, &a, 0)? {
        Value::Array(x, t) => (x.clone(), *t),
        other => return Err(i.error(format!("Cannot resize a value of type {}", other.type_description()))),
    };
    let n = int_arg(i, &a, 1)?;
    if n < 0 {
        return Err(i.error("Array size must not be negative"));
    }
    let fill = a.get(2).cloned().unwrap_or_default();
    let mut v = Arc::unwrap_or_clone(arr);
    v.resize(n as usize, fill);
    Ok(Value::Array(Arc::new(v), tag))
}

fn sub_array(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let arr = array_arg(i, &a, 0)?;
    let start = int_arg(i, &a, 1)?;
    let end = if a.len() > 2 { int_arg(i, &a, 2)? } else { arr.len() as i64 };
    if start < 0 || end < start || end as usize > arr.len() {
        return Err(i.error(format!("subArray range {start}..{end} is invalid for an array of size {}", arr.len())));
    }
    Ok(Value::array(arr[start as usize..end as usize].to_vec()))
}

fn concatenate_arrays(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let arrays = array_arg(i, &a, 0)?;
    let mut out = Vec::new();
    for x in arrays.iter() {
        match x {
            Value::Array(v, _) => out.extend(v.iter().cloned()),
            other => return Err(i.error(format!("concatenateArrays: element should be an array, was {}", other.type_description()))),
        }
    }
    Ok(Value::array(out))
}

fn index_of(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let arr = array_arg(i, &a, 0)?;
    let v = arg(i, &a, 1)?.clone();
    let start = if a.len() > 2 { int_arg(i, &a, 2)?.max(0) as usize } else { 0 };
    let r = arr.iter().enumerate().skip(start).find(|(_, x)| **x == v).map(|(k, _)| k as f64).unwrap_or(-1.0);
    Ok(Value::num(r))
}

fn keys(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let m = map_arg(i, &a, 0)?;
    Ok(Value::array(m.keys().cloned().collect()))
}

fn values(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let m = map_arg(i, &a, 0)?;
    Ok(Value::array(m.values().cloned().collect()))
}

fn merge_maps(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let (defaults, tag) = match arg(i, &a, 0)? {
        Value::Map(m, t) => (m.clone(), *t),
        other => return Err(i.error(format!("mergeMaps: argument 1 should be a map, was {}", other.type_description()))),
    };
    let m = map_arg(i, &a, 1)?;
    let mut out = Arc::unwrap_or_clone(defaults);
    for (k, v) in m.iter() {
        out.insert(k.clone(), v.clone());
    }
    Ok(Value::Map(Arc::new(out), tag))
}

fn intersect_maps(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let maps = array_arg(i, &a, 0)?;
    let mut iter = maps.iter();
    let Some(Value::Map(first, _)) = iter.next() else { return Ok(Value::map(Map::new())) };
    let mut out = (**first).clone();
    for m in iter {
        let Value::Map(m, _) = m else { return Err(i.error("intersectMaps: every element should be a map")) };
        out = m.iter().filter(|(k, _)| out.contains_key(k)).map(|(k, v)| (k.clone(), v.clone())).collect();
    }
    Ok(Value::map(out))
}

fn reverse(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let arr = array_arg(i, &a, 0)?;
    Ok(Value::array(arr.iter().rev().cloned().collect()))
}

/// The number of a number or of a ValueWithUnits (`{ value, unit }`).
fn magnitude(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n, _) => Some(*n),
        Value::Map(..) => v.get("value").as_number(),
        _ => None,
    }
}

fn with_magnitude(template: &Value, x: f64) -> Value {
    match template {
        Value::Map(m, t) => {
            let mut m = (**m).clone();
            m.insert(Value::str("value"), Value::num(x));
            Value::Map(Arc::new(m), *t)
        }
        _ => Value::num(x),
    }
}

fn range(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let from = arg(i, &a, 0)?.clone();
    let to = arg(i, &a, 1)?.clone();
    let count = int_arg(i, &a, 2)?;
    let (Some(x0), Some(x1)) = (magnitude(&from), magnitude(&to)) else {
        return Err(i.error("range: from and to should be numbers or values with units"));
    };
    if count < 0 {
        return Err(i.error("range: count must not be negative"));
    }
    let mut out = Vec::with_capacity(count as usize);
    for k in 0..count {
        let x = if count == 1 { x0 } else { x0 + (x1 - x0) * k as f64 / (count - 1) as f64 };
        out.push(with_magnitude(&from, x));
    }
    Ok(Value::array(out))
}

fn tolerant_sort(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let vals = array_arg(i, &a, 0)?;
    let tol = magnitude(arg(i, &a, 1)?).unwrap_or(0.0);
    let mut keyed: Vec<(usize, f64)> = Vec::with_capacity(vals.len());
    for (k, v) in vals.iter().enumerate() {
        match magnitude(v) {
            Some(x) => keyed.push((k, x)),
            None => return Err(i.error("tolerantSort: values should be numbers or values with units")),
        }
    }
    // Sort by value, then keep the original order inside runs whose
    // consecutive values are within the tolerance.
    keyed.sort_by(|p, q| p.1.total_cmp(&q.1).then(p.0.cmp(&q.0)));
    let mut out = Vec::with_capacity(keyed.len());
    let mut run: Vec<(usize, f64)> = Vec::new();
    for e in keyed {
        if let Some(last) = run.last()
            && e.1 - last.1 > tol
        {
            run.sort_by_key(|r| r.0);
            out.extend(run.drain(..).map(|r| Value::num(r.0 as f64)));
        }
        run.push(e);
    }
    run.sort_by_key(|r| r.0);
    out.extend(run.into_iter().map(|r| Value::num(r.0 as f64)));
    Ok(Value::array(out))
}

// ---------------------------------------------------------------- math

/// A unit Vector (plain numbers) along a vector of numbers or lengths.
fn normalize(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let v = array_arg(i, &a, 0)?;
    let xs: Option<Vec<f64>> = v.iter().map(magnitude).collect();
    let Some(xs) = xs else { return Err(i.error("normalize: argument should be a vector")) };
    let n = xs.iter().map(|x| x * x).sum::<f64>().sqrt();
    if n == 0.0 {
        return Err(i.error("Cannot normalize a zero vector"));
    }
    Ok(Value::array(xs.into_iter().map(|x| Value::num(x / n)).collect()))
}

/// Groups of indices of points within `tolerance` of each other.
fn cluster_points(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let pts = array_arg(i, &a, 0)?;
    let tol = num_arg(i, &a, 1)?;
    let coords: Vec<Vec<f64>> = pts.iter().map(|p| p.as_array().map(|c| c.iter().filter_map(magnitude).collect()).unwrap_or_default()).collect();
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut assigned = vec![false; coords.len()];
    for k in 0..coords.len() {
        if assigned[k] {
            continue;
        }
        let mut g = vec![k];
        assigned[k] = true;
        let mut j = 0;
        while j < g.len() {
            let c = g[j];
            for m in 0..coords.len() {
                if !assigned[m] && coords[c].len() == coords[m].len() && coords[c].iter().zip(&coords[m]).map(|(x, y)| (x - y) * (x - y)).sum::<f64>().sqrt() <= tol {
                    assigned[m] = true;
                    g.push(m);
                }
            }
            j += 1;
        }
        g.sort_unstable();
        groups.push(g);
    }
    Ok(Value::array(groups.into_iter().map(|g| Value::array(g.into_iter().map(|x| Value::num(x as f64)).collect())).collect()))
}

fn math1(i: &mut Interp, a: Vec<Value>, name: &str, f: fn(f64) -> f64) -> R<Value> {
    let x = num_arg(i, &a, 0)?;
    let r = f(x);
    finite(i, name, r)
}

fn math2(i: &mut Interp, a: Vec<Value>, name: &str, f: fn(f64, f64) -> f64) -> R<Value> {
    let x = num_arg(i, &a, 0)?;
    let y = num_arg(i, &a, 1)?;
    let r = f(x, y);
    finite(i, name, r)
}

// ---------------------------------------------------------------- output

fn print(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let s = match a.first() {
        Some(v) => v.as_str().map(|s| s.to_string()).unwrap_or_else(|| crate::value::to_display(v)),
        None => String::new(),
    };
    i.output.push_str(&s);
    Ok(Value::Undefined)
}

/// Timers outlive a feature (std debug.fs: a timer set in a prior feature
/// keeps running).
static TIMERS: std::sync::LazyLock<parking_lot::Mutex<std::collections::HashMap<String, std::time::Instant>>> = std::sync::LazyLock::new(Default::default);

fn start_timer(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let name = str_arg(i, &a, 0)?;
    TIMERS.lock().insert(name.to_string(), std::time::Instant::now());
    Ok(Value::Undefined)
}

/// Prints the elapsed whole milliseconds.
fn print_timer(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let name = str_arg(i, &a, 0)?;
    let Some(start) = TIMERS.lock().get(&*name).copied() else {
        return Err(i.error(format!("@printTimer: Unknown timer: \"{name}\"")));
    };
    // Live: `Timer "t1": 0 ms` (whole milliseconds).
    i.output.push_str(&format!("Timer \"{name}\": {} ms\n", start.elapsed().as_millis()));
    Ok(Value::Undefined)
}

fn report(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let s = a.first().map(|v| v.as_str().map(|s| s.to_string()).unwrap_or_else(|| crate::value::to_display(v))).unwrap_or_default();
    i.notice(NoticeKind::Warning, s);
    Ok(Value::Undefined)
}

// ---------------------------------------------------------------- strings

fn split_chars(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let s = str_arg(i, &a, 0)?;
    Ok(Value::array(s.chars().map(|c| Value::str(c.to_string())).collect()))
}

fn length(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let s = str_arg(i, &a, 0)?;
    Ok(Value::num(s.chars().count() as f64))
}

fn substring(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let s = str_arg(i, &a, 0)?;
    let chars: Vec<char> = s.chars().collect();
    let start = int_arg(i, &a, 1)?;
    let end = if a.len() > 2 { int_arg(i, &a, 2)? } else { chars.len() as i64 };
    if start < 0 || end < start || end as usize > chars.len() {
        return Err(i.error(format!("substring range {start}..{end} is invalid for a string of length {}", chars.len())));
    }
    Ok(Value::str(chars[start as usize..end as usize].iter().collect::<String>()))
}

fn str2(i: &mut Interp, a: Vec<Value>, f: fn(&str, &str) -> bool) -> R<Value> {
    let s = str_arg(i, &a, 0)?;
    let p = str_arg(i, &a, 1)?;
    Ok(Value::bool(f(&s, &p)))
}

fn char_index(s: &str, byte: usize) -> f64 {
    s[..byte].chars().count() as f64
}

fn byte_of_char(s: &str, ch: usize) -> usize {
    s.char_indices().nth(ch).map(|(b, _)| b).unwrap_or(s.len())
}

fn index_of_string(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let s = str_arg(i, &a, 0)?;
    let sub = str_arg(i, &a, 1)?;
    let start = if a.len() > 2 { int_arg(i, &a, 2)?.max(0) as usize } else { 0 };
    let b = byte_of_char(&s, start);
    Ok(Value::num(s[b..].find(&*sub).map(|k| char_index(&s, b + k)).unwrap_or(-1.0)))
}

/// Compiled regular expressions (Java syntax is close to Rust's for the
/// patterns FeatureScript uses).
fn regex(i: &mut Interp, pattern: &str, anchored: bool) -> R<regex::Regex> {
    static CACHE: LazyLock<Mutex<HashMap<(String, bool), regex::Regex>>> = LazyLock::new(Default::default);
    let key = (pattern.to_string(), anchored);
    if let Some(r) = CACHE.lock().get(&key) {
        return Ok(r.clone());
    }
    let src = if anchored { format!("^(?:{pattern})$") } else { pattern.to_string() };
    match regex::Regex::new(&src) {
        Ok(r) => {
            let mut c = CACHE.lock();
            if c.len() > 512 {
                c.clear();
            }
            c.insert(key, r.clone());
            Ok(r)
        }
        Err(e) => Err(i.error(format!("Invalid regular expression \"{pattern}\": {e}"))),
    }
}

fn index_of_regexp(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let s = str_arg(i, &a, 0)?;
    let p = str_arg(i, &a, 1)?;
    let start = if a.len() > 2 { int_arg(i, &a, 2)?.max(0) as usize } else { 0 };
    let re = regex(i, &p, false)?;
    let b = byte_of_char(&s, start);
    Ok(Value::num(re.find(&s[b..]).map(|m| char_index(&s, b + m.start())).unwrap_or(-1.0)))
}

fn repeat_string(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let s = str_arg(i, &a, 0)?;
    let n = int_arg(i, &a, 1)?;
    if n < 0 {
        return Err(i.error("repeatString: count must not be negative"));
    }
    Ok(Value::str(s.repeat(n as usize)))
}

fn string_to_number(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let s = str_arg(i, &a, 0)?;
    let t = s.trim();
    let v = match t {
        "inf" | "+inf" => Some(f64::INFINITY),
        "-inf" => Some(f64::NEG_INFINITY),
        _ if !t.is_empty() && t.chars().all(|c| c.is_ascii_digit() || matches!(c, '.' | 'e' | 'E' | '+' | '-')) => t.parse::<f64>().ok(),
        _ => None,
    };
    match v {
        Some(x) => Ok(Value::num(x)),
        None => Err(i.error(format!("Cannot convert \"{s}\" to a number"))),
    }
}

fn regex_match(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let s = str_arg(i, &a, 0)?;
    let p = str_arg(i, &a, 1)?;
    let re = regex(i, &p, true)?;
    let mut m = Map::new();
    match re.captures(&s) {
        Some(c) => {
            m.insert(Value::str("hasMatch"), Value::bool(true));
            let caps = c.iter().map(|g| g.map(|g| Value::str(g.as_str())).unwrap_or_default()).collect();
            m.insert(Value::str("captures"), Value::array(caps));
        }
        None => {
            m.insert(Value::str("hasMatch"), Value::bool(false));
            m.insert(Value::str("captures"), Value::array(vec![]));
        }
    }
    Ok(Value::map(m))
}

/// Replacement text: `$1` groups; backslashes are literal (std's own
/// `replace(result, "\\|$", ")\\b)")` must leave `)\b)`), except `\$`, a
/// literal dollar.
fn java_replacement(r: &str) -> String {
    let mut out = String::new();
    let mut chars = r.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&'$') => {
                chars.next();
                out.push_str("$$");
            }
            '\\' => out.push('\\'),
            '$' => {
                let mut digits = String::new();
                while let Some(d) = chars.peek().filter(|d| d.is_ascii_digit()) {
                    digits.push(*d);
                    chars.next();
                }
                if digits.is_empty() {
                    out.push_str("$$");
                } else {
                    out.push_str(&format!("${{{digits}}}"));
                }
            }
            other => out.push(other),
        }
    }
    out
}

fn regex_replace(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let s = str_arg(i, &a, 0)?;
    let p = str_arg(i, &a, 1)?;
    let r = str_arg(i, &a, 2)?;
    let re = regex(i, &p, false)?;
    Ok(Value::str(re.replace_all(&s, java_replacement(&r).as_str()).into_owned()))
}

fn split_by_regexp(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let s = str_arg(i, &a, 0)?;
    let p = str_arg(i, &a, 1)?;
    let mut parts: Vec<String> = if p.is_empty() {
        let mut v: Vec<String> = s.chars().map(|c| c.to_string()).collect();
        v.insert(0, String::new());
        v
    } else {
        let re = regex(i, &p, false)?;
        re.split(&s).map(|x| x.to_string()).collect()
    };
    // Like Java's String.split: trailing empty strings are removed.
    while parts.len() > 1 && parts.last().is_some_and(|x| x.is_empty()) {
        parts.pop();
    }
    Ok(Value::array(parts.into_iter().map(Value::str).collect()))
}

fn json_to_value(j: &serde_json::Value, units: Option<&Map>) -> Value {
    match j {
        serde_json::Value::Null => Value::Undefined,
        serde_json::Value::Bool(b) => Value::bool(*b),
        serde_json::Value::Number(n) => Value::num(n.as_f64().unwrap_or(0.0)),
        serde_json::Value::String(s) => {
            if let Some(u) = units
                && let Some(v) = string_with_units(s, u)
            {
                return v;
            }
            Value::str(s.as_str())
        }
        serde_json::Value::Array(a) => Value::array(a.iter().map(|x| json_to_value(x, units)).collect()),
        serde_json::Value::Object(o) => {
            let mut m = Map::new();
            for (k, v) in o {
                let v = json_to_value(v, units);
                if !v.is_undefined() {
                    m.insert(Value::str(k.as_str()), v);
                }
            }
            Value::map(m)
        }
    }
}

/// `"3 inch"` -> 3 * inch, with the unit looked up in `units`.
fn string_with_units(s: &str, units: &Map) -> Option<Value> {
    let (num, unit) = s.trim().split_once(char::is_whitespace)?;
    let x: f64 = num.parse().ok()?;
    let u = units.get(&Value::str(unit.trim()))?;
    let v = magnitude(u)?;
    Some(with_magnitude(u, x * v))
}

fn parse_json(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let s = str_arg(i, &a, 0)?;
    let opts = a.get(1).cloned().unwrap_or_default();
    let units = opts.get("stringToUnitMap").as_map().cloned();
    match serde_json::from_str::<serde_json::Value>(&s) {
        Ok(j) => Ok(json_to_value(&j, units.as_deref())),
        Err(e) => Err(i.error(format!("Invalid JSON: {e}"))),
    }
}

// ---------------------------------------------------------------- matrices

type Mat = Vec<Vec<f64>>;

fn to_mat(v: &Value) -> Option<Mat> {
    let rows = v.as_array()?;
    let mut out = Vec::with_capacity(rows.len());
    let mut width = None;
    for r in rows.iter() {
        let r = r.as_array()?;
        let row: Option<Vec<f64>> = r.iter().map(|x| x.as_number()).collect();
        let row = row?;
        if *width.get_or_insert(row.len()) != row.len() {
            return None;
        }
        out.push(row);
    }
    Some(out)
}

fn from_mat(m: Mat) -> Value {
    Value::array(m.into_iter().map(|r| Value::array(r.into_iter().map(Value::num).collect())).collect())
}

fn mat_arg(i: &mut Interp, a: &[Value], n: usize) -> R<Mat> {
    match a.get(n).and_then(to_mat) {
        Some(m) => Ok(m),
        None => Err(i.error(format!("Argument {} should be a matrix", n + 1))),
    }
}

fn is_matrix(_: &mut Interp, a: Vec<Value>) -> R<Value> {
    Ok(Value::bool(a.first().and_then(to_mat).is_some_and(|m| !m.is_empty() && !m[0].is_empty())))
}

fn matrix_identity(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let n = int_arg(i, &a, 0)?.max(0) as usize;
    Ok(from_mat((0..n).map(|r| (0..n).map(|c| if r == c { 1.0 } else { 0.0 }).collect()).collect()))
}

fn matrix_cwise(i: &mut Interp, a: Vec<Value>, f: fn(f64, f64) -> f64) -> R<Value> {
    let x = mat_arg(i, &a, 0)?;
    let y = mat_arg(i, &a, 1)?;
    if x.len() != y.len() || x.first().map(|r| r.len()) != y.first().map(|r| r.len()) {
        return Err(i.error("Matrix sizes do not match"));
    }
    Ok(from_mat(x.iter().zip(&y).map(|(r, s)| r.iter().zip(s).map(|(p, q)| f(*p, *q)).collect()).collect()))
}

fn matrix_negate(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let x = mat_arg(i, &a, 0)?;
    Ok(from_mat(x.into_iter().map(|r| r.into_iter().map(|p| -p).collect()).collect()))
}

fn matrix_multiply(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    // number * Matrix (std matrix.fs operator*).
    if let (Some(Value::Number(s, _)), Some(m)) = (a.first(), a.get(1)) {
        let Some(m) = to_mat(m) else { return Err(i.error("Argument 2 should be a matrix")) };
        return Ok(from_mat(m.into_iter().map(|r| r.into_iter().map(|p| p * s).collect()).collect()));
    }
    let x = mat_arg(i, &a, 0)?;
    match a.get(1) {
        Some(Value::Number(s, _)) => Ok(from_mat(x.into_iter().map(|r| r.into_iter().map(|p| p * s).collect()).collect())),
        Some(Value::Array(v, _)) if v.iter().all(|x| matches!(x, Value::Number(..))) => {
            // Matrix times a column vector.
            let y: Vec<f64> = v.iter().filter_map(|x| x.as_number()).collect();
            if x.first().map(|r| r.len()) != Some(y.len()) {
                return Err(i.error("Matrix and vector sizes do not match for multiplication"));
            }
            Ok(Value::array(x.iter().map(|r| Value::num(r.iter().zip(&y).map(|(p, q)| p * q).sum())).collect()))
        }
        _ => {
            let y = mat_arg(i, &a, 1)?;
            let inner = x.first().map(|r| r.len()).unwrap_or(0);
            if inner != y.len() {
                return Err(i.error("Matrix sizes do not match for multiplication"));
            }
            let cols = y.first().map(|r| r.len()).unwrap_or(0);
            Ok(from_mat(x.iter().map(|r| (0..cols).map(|c| (0..inner).map(|k| r[k] * y[k][c]).sum()).collect()).collect()))
        }
    }
}

fn matrix_transpose(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let x = mat_arg(i, &a, 0)?;
    let cols = x.first().map(|r| r.len()).unwrap_or(0);
    Ok(from_mat((0..cols).map(|c| x.iter().map(|r| r[c]).collect()).collect()))
}

fn matrix_inverse(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let x = mat_arg(i, &a, 0)?;
    let n = x.len();
    if x.iter().any(|r| r.len() != n) {
        return Err(i.error("Matrix must be square"));
    }
    // Gauss-Jordan with partial pivoting.
    let mut m: Mat = x.iter().enumerate().map(|(r, row)| row.iter().cloned().chain((0..n).map(|c| if c == r { 1.0 } else { 0.0 })).collect()).collect();
    for col in 0..n {
        let piv = (col..n).max_by(|p, q| m[*p][col].abs().total_cmp(&m[*q][col].abs())).unwrap_or(col);
        if m[piv][col].abs() < 1e-300 {
            return Err(i.error("Matrix is singular"));
        }
        m.swap(col, piv);
        let d = m[col][col];
        for v in m[col].iter_mut() {
            *v /= d;
        }
        for r in 0..n {
            if r != col {
                let f = m[r][col];
                if f != 0.0 {
                    let pivot = m[col].clone();
                    for (x, t) in m[r].iter_mut().zip(&pivot) {
                        *x -= f * t;
                    }
                }
            }
        }
    }
    Ok(from_mat(m.into_iter().map(|r| r[n..].to_vec()).collect()))
}

fn matrix_squared_norm(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let x = mat_arg(i, &a, 0)?;
    Ok(Value::num(x.iter().flatten().map(|v| v * v).sum()))
}

fn determinant(m: &Mat) -> f64 {
    let n = m.len();
    let mut a = m.clone();
    let mut det = 1.0;
    for col in 0..n {
        let piv = (col..n).max_by(|p, q| a[*p][col].abs().total_cmp(&a[*q][col].abs())).unwrap_or(col);
        if a[piv][col] == 0.0 {
            return 0.0;
        }
        if piv != col {
            a.swap(piv, col);
            det = -det;
        }
        det *= a[col][col];
        for r in col + 1..n {
            let f = a[r][col] / a[col][col];
            let pivot = a[col].clone();
            for (x, t) in a[r].iter_mut().zip(&pivot).skip(col) {
                *x -= f * t;
            }
        }
    }
    det
}

fn matrix_determinant(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let x = mat_arg(i, &a, 0)?;
    if x.iter().any(|r| r.len() != x.len()) {
        return Err(i.error("Matrix must be square"));
    }
    Ok(Value::num(determinant(&x)))
}

/// One-sided Jacobi SVD: `{ u, s, v }` with `m = u * diag(s) * v^T`.
fn matrix_svd(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let x = mat_arg(i, &a, 0)?;
    let rows = x.len();
    let cols = x.first().map(|r| r.len()).unwrap_or(0);
    let mut u = x.clone();
    let mut v: Mat = (0..cols).map(|r| (0..cols).map(|c| if r == c { 1.0 } else { 0.0 }).collect()).collect();
    for _ in 0..60 {
        let mut off = 0.0;
        for p in 0..cols {
            for q in p + 1..cols {
                let (mut alpha, mut beta, mut gamma) = (0.0, 0.0, 0.0);
                for row in u.iter() {
                    alpha += row[p] * row[p];
                    beta += row[q] * row[q];
                    gamma += row[p] * row[q];
                }
                off += gamma * gamma;
                if gamma.abs() < 1e-300 {
                    continue;
                }
                let zeta = (beta - alpha) / (2.0 * gamma);
                let t = zeta.signum() / (zeta.abs() + (1.0 + zeta * zeta).sqrt());
                let t = if zeta == 0.0 { 1.0 } else { t };
                let c = 1.0 / (1.0 + t * t).sqrt();
                let s = c * t;
                for row in u.iter_mut() {
                    let (a1, b1) = (row[p], row[q]);
                    row[p] = c * a1 - s * b1;
                    row[q] = s * a1 + c * b1;
                }
                for row in v.iter_mut() {
                    let (a1, b1) = (row[p], row[q]);
                    row[p] = c * a1 - s * b1;
                    row[q] = s * a1 + c * b1;
                }
            }
        }
        if off < 1e-30 {
            break;
        }
    }
    let mut s = vec![0.0; cols];
    for (c, sv) in s.iter_mut().enumerate() {
        *sv = (0..rows).map(|r| u[r][c] * u[r][c]).sum::<f64>().sqrt();
        if *sv > 1e-300 {
            for row in u.iter_mut() {
                row[c] /= *sv;
            }
        }
    }
    // Sort singular values decreasing.
    let mut order: Vec<usize> = (0..cols).collect();
    order.sort_by(|p, q| s[*q].total_cmp(&s[*p]));
    let u2: Mat = u.iter().map(|r| order.iter().map(|&c| r[c]).collect()).collect();
    let v2: Mat = v.iter().map(|r| order.iter().map(|&c| r[c]).collect()).collect();
    let s2: Vec<Value> = order.iter().map(|&c| Value::num(s[c])).collect();
    let _ = i;
    let mut m = Map::new();
    m.insert(Value::str("u"), from_mat(u2));
    m.insert(Value::str("s"), Value::array(s2));
    m.insert(Value::str("v"), from_mat(v2));
    Ok(Value::map(m))
}

fn matrix_rotation_3d(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    // (axis as 3 numbers, angle in radians)
    let axis = array_arg(i, &a, 0)?;
    let ang = num_arg(i, &a, 1)?;
    let ax: Vec<f64> = axis.iter().filter_map(magnitude).collect();
    if ax.len() != 3 {
        return Err(i.error("matrixRotation3d: axis should have 3 components"));
    }
    let n = (ax[0] * ax[0] + ax[1] * ax[1] + ax[2] * ax[2]).sqrt();
    if n == 0.0 {
        return Err(i.error("matrixRotation3d: axis is zero"));
    }
    let (x, y, z) = (ax[0] / n, ax[1] / n, ax[2] / n);
    let (s, c) = ang.sin_cos();
    let t = 1.0 - c;
    Ok(from_mat(vec![
        vec![t * x * x + c, t * x * y - s * z, t * x * z + s * y],
        vec![t * x * y + s * z, t * y * y + c, t * y * z - s * x],
        vec![t * x * z - s * y, t * y * z + s * x, t * z * z + c],
    ]))
}

/// Untagged `{ "key" : value }` helper for natives.
pub fn tagged_map(entries: Vec<(&str, Value)>, tag: Tag) -> Value {
    let mut m = Map::new();
    for (k, v) in entries {
        if !v.is_undefined() {
            m.insert(Value::str(k), v);
        }
    }
    Value::Map(Arc::new(m), tag)
}
