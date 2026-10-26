//! Feature parameters as JSON, converted through the feature's own
//! [`ParamSpec`] schema (the same one the feature dialog renders), and JSON
//! schemas generated from it.
//!
//! JSON forms: booleans; enum option ids (`"BLIND"`; labels are accepted);
//! numbers as expressions (`"25 mm"`, `"#width / 2"`, `"30 deg"`; a bare
//! number uses the document's units, as typing it into the dialog does);
//! selections as arrays of reference ids; arrays as arrays of objects;
//! lookups as arrays of strings; tab references as a tab id or name.

use crate::error::OpError;
use crate::refs;
use noshape_model::document::Document;
use noshape_model::params::{Hints, NumKind, ParamKind, ParamSpec, ParamValue, ParamValues, QueryFilter, fill_defaults};
use noshape_units::{ExprValue, UnitPrefs};
use serde_json::{Value, json};

/// Find a spec (top level only) by id.
fn spec<'a>(specs: &'a [ParamSpec], id: &str) -> Option<&'a ParamSpec> {
    specs.iter().find(|s| s.id == id)
}

/// Apply JSON `params` onto `values` (only the given keys change).
pub fn apply(specs: &[ParamSpec], values: &mut ParamValues, params: &serde_json::Map<String, Value>, units: &UnitPrefs, doc: &Document) -> Result<(), OpError> {
    for (k, v) in params {
        let Some(s) = spec(specs, k) else {
            // Stored values the schema no longer lists (feature_get shows
            // them) pass back unchanged.
            if values.get(k).is_some_and(|cur| to_json(None, cur) == *v) {
                continue;
            }
            let known: Vec<&str> = specs.iter().filter(|s| !matches!(s.kind, ParamKind::Sketch)).map(|s| s.id).collect();
            return Err(OpError::param(k, &format!("unknown parameter; this feature's parameters are: {}", known.join(", "))));
        };
        let pv = from_json(s, v, units, doc).map_err(|e| e.under(k).with_param_if_none(k))?;
        // Changing a legacy parameter the dialog never shows, beside the one
        // it does (Hole `style` / `styleV2`), would be ignored: say which to
        // use (its current value, as feature_get lists it, passes).
        let same = match (values.get(s.id), &pv) {
            (Some(ParamValue::Expr(a)), ParamValue::Expr(b)) => a.text == b.text,
            (cur, new) => cur == Some(new),
        };
        if !same
            && s.hints.has(Hints::ALWAYS_HIDDEN)
            && let Some(o) = specs.iter().find(|o| o.id != s.id && o.label == s.label && !o.hints.has(Hints::ALWAYS_HIDDEN))
        {
            return Err(OpError::param(k, &format!("a legacy parameter the dialog doesn't show (it would be ignored); use `{}`", o.id)));
        }
        values.set(s.id, pv);
    }
    fill_defaults(specs, values, units);
    Ok(())
}

trait ParamIfNone {
    fn with_param_if_none(self, p: &str) -> Self;
}
impl ParamIfNone for OpError {
    fn with_param_if_none(mut self, p: &str) -> Self {
        if self.param.is_none() {
            self.param = Some(p.to_string());
        }
        self
    }
}

fn kind_name(s: &ParamSpec) -> &'static str {
    match &s.kind {
        ParamKind::Bool { .. } => "boolean",
        ParamKind::Enum { .. } => "enum",
        ParamKind::Num(n) => match n.kind {
            NumKind::Length => "length expression",
            NumKind::Angle => "angle expression",
            NumKind::Real => "number expression",
            NumKind::Integer => "integer expression",
        },
        ParamKind::Text { .. } => "string",
        ParamKind::Query { .. } => "selection",
        ParamKind::Array { .. } => "array",
        ParamKind::Lookup { .. } => "lookup path",
        ParamKind::Sketch => "sketch",
        ParamKind::Reference { .. } => "tab reference",
    }
}

/// One parameter value from JSON.
pub fn from_json(s: &ParamSpec, v: &Value, units: &UnitPrefs, doc: &Document) -> Result<ParamValue, OpError> {
    let bad = || OpError::invalid(format!("`{}` ({}) expects a {}; got {v}", s.id, s.label, kind_name(s)));
    Ok(match &s.kind {
        ParamKind::Bool { .. } => ParamValue::Bool(v.as_bool().ok_or_else(bad)?),
        ParamKind::Enum { options, .. } => {
            let t = v.as_str().ok_or_else(bad)?;
            let opt = options.iter().find(|(id, _)| *id == t).or_else(|| options.iter().find(|(id, label)| id.eq_ignore_ascii_case(t) || label.eq_ignore_ascii_case(t)));
            match opt {
                Some((id, _)) => ParamValue::Enum((*id).to_string()),
                None => return Err(OpError::invalid(format!("`{t}` is not an option of `{}`; options: {}", s.id, options.iter().map(|(id, l)| format!("{id} ({l})")).collect::<Vec<_>>().join(", ")))),
            }
        }
        ParamKind::Num(n) => {
            let text = match v {
                Value::String(t) => t.clone(),
                Value::Number(x) => x.to_string(),
                _ => return Err(bad()),
            };
            let e = ExprValue::new(text.trim());
            if let Err(err) = e.parse() {
                return Err(OpError::invalid(format!("`{text}` is not a valid expression: {err}")));
            }
            // Without variables, check the value now (dimension and bounds);
            // with variables the regeneration evaluates it, as in the dialog.
            if e.variables().is_empty() {
                match noshape_units::evaluate_as(&e.text, n.kind.dim(), units, &noshape_units::EmptyScope) {
                    Ok(q) => {
                        let si = q.value;
                        if si < n.min - 1e-12 || si > n.max + 1e-12 {
                            let shown = |x: f64| noshape_units::format_quantity_exact(noshape_units::Quantity::new(x, n.kind.dim()), units);
                            return Err(OpError::invalid(format!("`{text}` is out of range: must be between {} and {}", shown(n.min), shown(n.max))));
                        }
                        if n.kind == NumKind::Integer && (si - si.round()).abs() > 1e-9 {
                            return Err(OpError::invalid(format!("`{text}` must be a whole number")));
                        }
                    }
                    Err(err) => return Err(OpError::invalid(format!("`{text}`: {err}"))),
                }
            }
            ParamValue::Expr(e)
        }
        ParamKind::Text { .. } => ParamValue::Text(v.as_str().ok_or_else(bad)?.to_string()),
        ParamKind::Query { max_picks, .. } => {
            let items: Vec<Value> = match v {
                Value::Array(a) => a.clone(),
                Value::String(_) | Value::Object(_) => vec![v.clone()],
                _ => return Err(bad()),
            };
            if let Some(m) = max_picks
                && items.len() > *m
            {
                return Err(OpError::invalid(format!("`{}` takes at most {m} selection(s); got {}", s.id, items.len())));
            }
            let refs = items.iter().enumerate().map(|(i, r)| refs::parse(r).map_err(|e| e.with_param(format!("[{i}]")))).collect::<Result<Vec<_>, _>>()?;
            ParamValue::Query(refs)
        }
        ParamKind::Array { item, .. } => {
            let arr = v.as_array().ok_or_else(bad)?;
            let mut out = Vec::new();
            for (i, it) in arr.iter().enumerate() {
                let obj = it.as_object().ok_or_else(|| OpError::invalid("array items are objects of item parameters").with_param(format!("[{i}]")))?;
                let mut pv = ParamValues::default();
                fill_defaults(item, &mut pv, units);
                apply(item, &mut pv, obj, units, doc).map_err(|e| e.under(&format!("[{i}]")))?;
                out.push(pv);
            }
            ParamValue::Array(out)
        }
        ParamKind::Lookup { .. } => {
            let arr = v.as_array().ok_or_else(bad)?;
            ParamValue::Lookup(arr.iter().map(|x| x.as_str().map(str::to_string).ok_or_else(bad)).collect::<Result<_, _>>()?)
        }
        ParamKind::Sketch => ParamValue::Sketch(Box::new(serde_json::from_value(v.clone()).map_err(|e| OpError::invalid(format!("sketch data: {e}")))?)),
        ParamKind::Reference { .. } => {
            let t = v.as_str().ok_or_else(bad)?;
            if t.is_empty() {
                ParamValue::Text(String::new())
            } else {
                let tab = crate::doc::find_tab(doc, t)?;
                ParamValue::Text(tab.to_string())
            }
        }
    })
}

/// One parameter value as JSON (the same forms [`from_json`] accepts).
pub fn to_json(s: Option<&ParamSpec>, v: &ParamValue) -> Value {
    match v {
        ParamValue::Bool(b) => json!(b),
        ParamValue::Enum(e) => json!(e),
        ParamValue::Expr(e) => json!(e.text),
        ParamValue::Text(t) => json!(t),
        ParamValue::Query(q) => Value::Array(q.iter().map(|r| json!(refs::to_id(r))).collect()),
        ParamValue::Array(items) => {
            let item_specs = match s.map(|s| &s.kind) {
                Some(ParamKind::Array { item, .. }) => Some(item.clone()),
                _ => None,
            };
            Value::Array(items.iter().map(|it| values_to_json(item_specs.as_deref().map(|v| &v[..]), it)).collect())
        }
        ParamValue::Lookup(l) => json!(l),
        ParamValue::Sketch(_) => json!("<sketch: use sketch_get>"),
    }
}

pub fn values_to_json(specs: Option<&[ParamSpec]>, values: &ParamValues) -> Value {
    let mut m = serde_json::Map::new();
    for (k, v) in &values.0 {
        let s = specs.and_then(|ss| spec(ss, k));
        if matches!(v, ParamValue::Sketch(_)) {
            continue;
        }
        m.insert(k.clone(), to_json(s, v));
    }
    Value::Object(m)
}

/// The ids of the parameters the dialog currently shows.
pub fn visible_ids(specs: &[ParamSpec], values: &ParamValues) -> Vec<&'static str> {
    specs.iter().filter(|s| s.is_visible(values)).map(|s| s.id).collect()
}

fn filter_names(f: QueryFilter) -> Vec<&'static str> {
    const NAMES: [(QueryFilter, &str); 18] = [
        (QueryFilter::FACE, "face"),
        (QueryFilter::PLANAR_FACE, "planar face"),
        (QueryFilter::EDGE, "edge"),
        (QueryFilter::LINEAR_EDGE, "linear edge"),
        (QueryFilter::CIRCULAR_EDGE, "circular edge"),
        (QueryFilter::VERTEX, "vertex"),
        (QueryFilter::SOLID_BODY, "part (solid body)"),
        (QueryFilter::SHEET_BODY, "surface (sheet body)"),
        (QueryFilter::WIRE_BODY, "curve (wire body)"),
        (QueryFilter::SKETCH_REGION, "sketch region"),
        (QueryFilter::SKETCH_EDGE, "sketch curve"),
        (QueryFilter::SKETCH_POINT, "sketch point"),
        (QueryFilter::PLANE, "plane"),
        (QueryFilter::MATE_CONNECTOR, "mate connector"),
        (QueryFilter::FEATURE, "feature"),
        (QueryFilter::CYLINDRICAL_FACE, "cylindrical face"),
        (QueryFilter::SKETCH, "sketch"),
        (QueryFilter::POINT_BODY, "point body"),
    ];
    NAMES.iter().filter(|(q, _)| f.intersects(*q)).map(|(_, n)| *n).collect()
}

/// When a parameter with a visibility rule is shown: with the feature's
/// defaults, and which single enum / boolean change from them flips it
/// ("shown; hidden when endStyleV2 is THROUGH"). `None` (array items, whose
/// rules read their item's values): the general note.
fn visibility_note(s: &ParamSpec, all: &[ParamSpec], defaults: Option<&ParamValues>) -> String {
    let Some(defaults) = defaults else { return " (shown only for some values of the other parameters; ignored while hidden)".into() };
    let shown = s.is_visible(defaults);
    let mut flips: Vec<(&str, Vec<String>)> = Vec::new();
    for p in all.iter().filter(|p| p.id != s.id) {
        let alternatives: Vec<(String, ParamValue)> = match (&p.kind, defaults.get(p.id)) {
            (ParamKind::Enum { options, .. }, Some(ParamValue::Enum(cur))) => options.iter().filter(|(id, _)| id != cur).map(|(id, _)| (id.to_string(), ParamValue::Enum(id.to_string()))).collect(),
            (ParamKind::Bool { .. }, Some(ParamValue::Bool(b))) => vec![((!b).to_string(), ParamValue::Bool(!b))],
            _ => continue,
        };
        let changed: Vec<String> = alternatives
            .into_iter()
            .filter(|(_, v)| {
                let mut probe = defaults.clone();
                probe.set(p.id, v.clone());
                s.is_visible(&probe) != shown
            })
            .map(|(name, _)| name)
            .collect();
        if !changed.is_empty() {
            flips.push((p.id, changed));
        }
    }
    let when = flips.iter().map(|(id, vs)| format!("{id} is {}", vs.join(" or "))).collect::<Vec<_>>().join(", or when ");
    match (shown, when.is_empty()) {
        (true, true) => " (shown with the defaults; hidden for some other combinations of the other parameters, and ignored while hidden)".into(),
        (true, false) => format!(" (shown with the defaults; hidden, and ignored, when {when})"),
        (false, true) => " (hidden with the defaults, and ignored while hidden; shown only for some combinations of the other parameters)".into(),
        (false, false) => format!(" (hidden with the defaults, and ignored while hidden; shown when {when})"),
    }
}

/// JSON schema of one parameter (`all` and `defaults`: its feature's
/// parameters and default values, to say when it is shown).
pub fn spec_schema(s: &ParamSpec, units: &UnitPrefs, all: &[ParamSpec], defaults: Option<&ParamValues>) -> Value {
    let mut desc = s.label.to_string();
    if !s.description.is_empty() {
        desc = format!("{desc}. {}", s.description);
    }
    if s.visible.is_some() {
        desc.push_str(&visibility_note(s, all, defaults));
    }
    let mut v = match &s.kind {
        ParamKind::Bool { default } => json!({ "type": "boolean", "default": default }),
        ParamKind::Enum { options, default } => {
            desc = format!("{desc}. Options: {}", options.iter().map(|(id, l)| format!("{id} = {l}")).collect::<Vec<_>>().join("; "));
            json!({ "type": "string", "enum": options.iter().map(|(id, _)| *id).collect::<Vec<_>>(), "default": default })
        }
        ParamKind::Num(n) => {
            let what = match n.kind {
                NumKind::Length => "Length expression with units, e.g. \"25 mm\", \"1 in\", \"#width / 2\"",
                NumKind::Angle => "Angle expression, e.g. \"30 deg\", \"#a * 2\"",
                NumKind::Real => "Number expression, e.g. \"1.5\", \"#ratio\"",
                NumKind::Integer => "Integer expression, e.g. \"4\", \"#count\"",
            };
            let d = noshape_model::params::default_expr(n, units).text;
            let range = |x: f64| noshape_units::format_quantity_exact(noshape_units::Quantity::new(x, n.kind.dim()), units);
            desc = format!("{desc}. {what}; range {} to {}", range(n.min), range(n.max));
            json!({ "type": ["string", "number"], "default": d })
        }
        ParamKind::Text { default } => json!({ "type": "string", "default": default }),
        ParamKind::Query { filter, max_picks } => {
            desc = format!("{desc}. Selection of reference ids; accepts: {}", filter_names(*filter).join(", "));
            let mut v = json!({ "type": "array", "items": { "type": "string" } });
            if let Some(m) = max_picks {
                v["maxItems"] = json!(m);
            }
            v
        }
        ParamKind::Array { item, item_label } => {
            let mut props = serde_json::Map::new();
            for it in item.iter() {
                props.insert(it.id.into(), spec_schema(it, units, item, None));
            }
            desc = format!("{desc}. List of {item_label} items");
            json!({ "type": "array", "items": { "type": "object", "properties": props, "additionalProperties": false } })
        }
        ParamKind::Lookup { table } => {
            desc = format!("{desc}. Lookup path in table `{table}`, one string per level");
            json!({ "type": "array", "items": { "type": "string" } })
        }
        ParamKind::Sketch => json!({ "type": "object" }),
        ParamKind::Reference { kind } => {
            desc = format!("{desc}. Id or name of a {kind:?} tab");
            json!({ "type": "string" })
        }
    };
    v["description"] = Value::String(desc);
    v
}

/// JSON schema of a feature's parameter object (`defaults`: a new
/// feature's values).
pub fn feature_schema(specs: &[ParamSpec], units: &UnitPrefs, defaults: Option<&ParamValues>) -> Value {
    let mut props = serde_json::Map::new();
    for s in specs.iter().filter(|s| !matches!(s.kind, ParamKind::Sketch)) {
        props.insert(s.id.into(), spec_schema(s, units, specs, defaults));
    }
    json!({ "type": "object", "properties": props, "additionalProperties": false })
}
