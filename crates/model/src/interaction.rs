//! Pure logic behind Part Studio interactions that must behave like
//! Onshape: feature dependencies and reorder validation, the feature list
//! search filter (`:errors`, `:type ...`), mouse-wheel nudging of numeric
//! fields, range selection in lists and "Select other" (entities under the
//! cursor, nearest first). Kept free of UI types so it can be unit tested.

use crate::display::{FeatureStatus, StudioDisplay};
use crate::params::{ParamValue, ParamValues};
use crate::partstudio::{Feature, PartStudio};
use crate::reference::{FeatureId, Reference};
use glam::DVec3;
use std::collections::{BTreeSet, HashSet};

// ------------------------------------------------------------------ dependencies

/// Collect every integer found anywhere in a JSON value (feature ids are
/// serialized as integers; see `partstudio::remap_ids`).
fn collect_ints(v: &serde_json::Value, out: &mut HashSet<u64>) {
    match v {
        serde_json::Value::Number(n) => {
            if let Some(u) = n.as_u64() {
                out.insert(u);
            }
        }
        serde_json::Value::Array(a) => a.iter().for_each(|x| collect_ints(x, out)),
        serde_json::Value::Object(o) => o.values().for_each(|x| collect_ints(x, out)),
        _ => {}
    }
}

/// Features a parameter value refers to: for entity selections the
/// features that created (or split) the selected geometry, not every
/// feature mentioned in its persistent name; for other values (sketch
/// payloads...) every feature id found in it.
fn param_refs(v: &ParamValue, out: &mut HashSet<u64>) {
    fn name(n: &crate::reference::TopoName, out: &mut HashSet<u64>) {
        use crate::reference::TopoName as T;
        match n {
            T::Face { feature, .. } | T::Wire { feature, .. } => {
                out.insert(feature.0);
            }
            T::Split { parent, by, .. } => {
                out.insert(by.0);
                name(parent, out);
            }
            T::Edge { faces, .. } => faces.iter().for_each(|f| name(f, out)),
            T::Vertex { faces, .. } => faces.iter().for_each(|f| name(f, out)),
            T::Boundary { face, .. } => name(face, out),
        }
    }
    fn reference(r: &Reference, out: &mut HashSet<u64>) {
        match r {
            Reference::Face(n) | Reference::Edge(n) | Reference::Vertex(n) => name(n, out),
            Reference::Implicit { on, .. } => reference(on, out),
            r => {
                if let Some(f) = r.depends_on() {
                    out.insert(f.0);
                }
            }
        }
    }
    match v {
        ParamValue::Query(refs) => refs.iter().for_each(|r| reference(r, out)),
        ParamValue::Array(items) => items.iter().flat_map(|i| i.0.values()).for_each(|x| param_refs(x, out)),
        ParamValue::Bool(_) | ParamValue::Enum(_) | ParamValue::Expr(_) | ParamValue::Text(_) | ParamValue::Lookup(_) => {}
        other => {
            if let Ok(j) = serde_json::to_value(other) {
                collect_ints(&j, out);
            }
        }
    }
}

/// `#name` variables used by a feature's expressions.
fn used_variables(f: &Feature) -> BTreeSet<String> {
    fn walk(v: &ParamValue, out: &mut BTreeSet<String>) {
        match v {
            ParamValue::Expr(e) => {
                if let Ok(x) = e.parse() {
                    out.extend(x.variables());
                }
            }
            ParamValue::Text(t) if t.contains('#') => {
                if let Ok(x) = noshape_units::parse(t) {
                    out.extend(x.variables());
                }
            }
            ParamValue::Array(items) => items.iter().flat_map(|i| i.0.values()).for_each(|x| walk(x, out)),
            _ => {}
        }
    }
    let mut out = BTreeSet::new();
    f.params.0.values().for_each(|v| walk(v, &mut out));
    out
}

/// Name a Variable feature defines, if it is one.
fn defined_variable(f: &Feature) -> Option<&str> {
    (f.type_id == "assignVariable").then(|| f.params.text("name")).filter(|n| !n.is_empty())
}

/// Features `features[index]` depends on, among the features *before* it
/// (references to their geometry, sketches, planes and the Variable
/// features defining the `#variables` it uses). In history order.
pub fn feature_dependencies(features: &[Feature], index: usize) -> Vec<FeatureId> {
    let Some(f) = features.get(index) else { return Vec::new() };
    let mut ints = HashSet::new();
    f.params.0.values().for_each(|v| param_refs(v, &mut ints));
    let vars = used_variables(f);
    features[..index]
        .iter()
        .filter(|u| ints.contains(&u.id.0) || defined_variable(u).is_some_and(|n| vars.contains(n)))
        .map(|u| u.id)
        .collect()
}

/// Features after `features[index]` that depend on it (its children).
pub fn feature_dependents(features: &[Feature], index: usize) -> Vec<FeatureId> {
    let Some(f) = features.get(index) else { return Vec::new() };
    (index + 1..features.len()).filter(|&j| feature_dependencies(features, j).contains(&f.id)).map(|j| features[j].id).collect()
}

/// New feature order after dragging `moving` onto the row at `to`
/// (Onshape: dropped above the target when moving up, below it when moving
/// down). `None` when nothing would change.
pub fn reordered(features: &[Feature], moving: &[FeatureId], to: usize) -> Option<Vec<FeatureId>> {
    let ids: Vec<FeatureId> = features.iter().map(|f| f.id).collect();
    let target_pos = to.min(ids.len().checked_sub(1)?);
    let all_above = moving.iter().all(|m| ids.iter().position(|i| i == m).is_some_and(|p| p < target_pos));
    reordered_at(features, moving, to, all_above)
}

/// New feature order after dropping `moving` on the row at `to`: before it,
/// or `after` it (live F33: the half of the row the cursor is in, whichever
/// way the drag goes). `None` when nothing would change.
pub fn reordered_at(features: &[Feature], moving: &[FeatureId], to: usize, after: bool) -> Option<Vec<FeatureId>> {
    let ids: Vec<FeatureId> = features.iter().map(|f| f.id).collect();
    let target = *ids.get(to.min(ids.len().checked_sub(1)?))?;
    let moving: Vec<FeatureId> = ids.iter().copied().filter(|i| moving.contains(i)).collect();
    if moving.is_empty() || moving.contains(&target) {
        return None;
    }
    let mut rest: Vec<FeatureId> = ids.iter().copied().filter(|i| !moving.contains(i)).collect();
    let at = rest.iter().position(|i| *i == target)? + usize::from(after);
    rest.splice(at..at, moving);
    (rest != ids).then_some(rest)
}

/// Check that a new order keeps every feature after the features whose
/// geometry it references. Returns the user-facing error of the first
/// violation. A `#variable` doesn't hold a reorder back (live qa5 pass 8:
/// the move goes through with no message, and the feature then fails with
/// "Variable not found, is suppressed, or has not been set.").
pub fn check_order(features: &[Feature], order: &[FeatureId]) -> Result<(), String> {
    let pos = |id: FeatureId| order.iter().position(|i| *i == id);
    let name = |id: FeatureId| features.iter().find(|f| f.id == id).map(|f| f.name.clone()).unwrap_or_default();
    for (i, f) in features.iter().enumerate() {
        let Some(p) = pos(f.id) else { continue };
        let mut ints = HashSet::new();
        f.params.0.values().for_each(|v| param_refs(v, &mut ints));
        for dep in features[..i].iter().filter(|u| ints.contains(&u.id.0)).map(|u| u.id) {
            if pos(dep).is_some_and(|d| d > p) {
                return Err(format!("Cannot reorder: {} depends on {}.", f.name, name(dep)));
            }
        }
    }
    Ok(())
}

/// Apply a validated order to a Part Studio. Folders follow the feature
/// dropped next to (a moved feature joins the folder of its new upper
/// neighbor only if its lower neighbor is in the same folder).
pub fn apply_order(ps: &mut PartStudio, order: &[FeatureId]) {
    let mut old = std::mem::take(&mut ps.features);
    let moved: HashSet<FeatureId> = old.iter().enumerate().filter(|(i, f)| order.get(*i) != Some(&f.id)).map(|(_, f)| f.id).collect();
    for id in order {
        if let Some(i) = old.iter().position(|f| f.id == *id) {
            ps.features.push(old.remove(i));
        }
    }
    ps.features.append(&mut old);
    for i in 0..ps.features.len() {
        if !moved.contains(&ps.features[i].id) {
            continue;
        }
        let above = i.checked_sub(1).and_then(|j| ps.features.get(j)).and_then(|f| f.folder);
        let below = ps.features.get(i + 1).and_then(|f| f.folder);
        ps.features[i].folder = if above.is_some() && above == below { above } else { below.filter(|_| ps.features[i].folder == below).or(above.filter(|_| ps.features[i].folder == above)) };
    }
}

// ------------------------------------------------------------------ list selection

/// Onshape list selection: Shift+click selects the range from the anchor
/// (last clicked item) to the clicked item, Ctrl/Cmd+click and plain clicks
/// toggle the clicked item (selection is always additive).
pub fn list_click<T: Clone + PartialEq>(order: &[T], selected: &[T], anchor: Option<&T>, clicked: &T, shift: bool) -> Vec<T> {
    if shift && let Some(a) = anchor {
        let (Some(i), Some(j)) = (order.iter().position(|x| x == a), order.iter().position(|x| x == clicked)) else {
            return vec![clicked.clone()];
        };
        let (lo, hi) = (i.min(j), i.max(j));
        let mut out = selected.to_vec();
        for x in &order[lo..=hi] {
            if !out.contains(x) {
                out.push(x.clone());
            }
        }
        return out;
    }
    let mut out = selected.to_vec();
    match out.iter().position(|x| x == clicked) {
        Some(p) => {
            out.remove(p);
        }
        None => out.push(clicked.clone()),
    }
    out
}

// ------------------------------------------------------------------ feature filter

/// Feature list search (Onshape "Filter by feature name or type name"):
/// plain text matches names and type names; `:errors`, `:warnings`,
/// `:suppressed`, `:hidden`, `:shown`, `:type x`, `:name x`, `:folder x`,
/// `:variable x`, `:part x`, `:partnumber x` and `:revision x` narrow
/// further (the last two by the parts' properties). Returns the matching
/// features, or `None` for an empty query (no filtering).
pub fn filter_features(studio: &PartStudio, display: &StudioDisplay, query: &str) -> Option<HashSet<FeatureId>> {
    let q = query.trim();
    if q.is_empty() {
        return None;
    }
    let (cmd, arg) = match q.strip_prefix(':') {
        Some(rest) => {
            let mut it = rest.splitn(2, char::is_whitespace);
            (it.next().unwrap_or("").to_lowercase(), it.next().unwrap_or("").trim().to_string())
        }
        None => (String::new(), q.to_string()),
    };
    // A quoted argument matches exactly (Help: `:part "Part 1"`; live
    // Variable table › Filter feature list puts `:variable "height"`).
    let exact = arg.len() >= 2 && arg.starts_with('"') && arg.ends_with('"');
    let arg = if exact { arg[1..arg.len() - 1].to_string() } else { arg };
    let arg_l = arg.to_lowercase();
    let status = |id: FeatureId| display.feature_state(id).map(|s| s.status);
    let type_name = |f: &Feature| f.feature_type().map(|t| t.name.to_lowercase()).unwrap_or_else(|| f.type_id.to_lowercase());
    let name_match = |f: &Feature| arg_l.is_empty() || if exact { f.name.to_lowercase() == arg_l } else { f.name.to_lowercase().contains(&arg_l) };
    fn part_number(p: &crate::partstudio::PartProperties) -> Option<&str> {
        p.part_number.as_deref()
    }
    fn revision(p: &crate::partstudio::PartProperties) -> Option<&str> {
        p.revision.as_deref()
    }
    // A part property contains the argument.
    let prop = |b: &crate::display::BodyDisplay, get: fn(&crate::partstudio::PartProperties) -> Option<&str>| studio.part_properties.get(&b.id).and_then(get).is_some_and(|v| v.to_lowercase().contains(&arg_l));
    // Features that create or modify a part the predicate accepts.
    let part_feature = |i: usize, f: &Feature, matches: &dyn Fn(&crate::display::BodyDisplay) -> bool| {
        let creators: HashSet<FeatureId> = display.bodies.iter().filter(|b| matches(b)).map(|b| b.id.feature).collect();
        creators.contains(&f.id)
            || display.bodies.iter().filter(|b| matches(b)).any(|b| b.face_refs.iter().any(|r| r.depends_on() == Some(f.id)))
            || feature_dependencies(&studio.features, i).iter().any(|d| creators.contains(d)) && modifies_bodies(f)
    };
    let mut out = HashSet::new();
    for (i, f) in studio.features.iter().enumerate() {
        let keep = match cmd.as_str() {
            "" => f.name.to_lowercase().contains(&arg_l) || type_name(f).contains(&arg_l) || f.type_id.to_lowercase().contains(&arg_l),
            "errors" | "error" => status(f.id) == Some(FeatureStatus::Error) && name_match(f),
            "warnings" | "warning" => status(f.id) == Some(FeatureStatus::Warning) && name_match(f),
            "suppressed" => f.suppressed && name_match(f),
            "hidden" => f.hidden && name_match(f),
            "shown" => !f.hidden && name_match(f),
            "type" => type_name(f).contains(&arg_l) || f.type_id.to_lowercase().contains(&arg_l),
            "name" => name_match(f),
            "folder" => f.folder.and_then(|id| studio.folder(id)).is_some_and(|fd| fd.name.to_lowercase().contains(&arg_l)),
            // Case-sensitive, like Onshape.
            "variable" => {
                let arg = arg.trim_start_matches('#');
                let hit = |v: &str| if exact { v == arg } else { v.contains(arg) };
                used_variables(f).iter().any(|v| hit(v)) || defined_variable(f).is_some_and(hit)
            }
            "part" => part_feature(i, f, &|b| b.name.to_lowercase().contains(&arg_l)),
            "partnumber" => part_feature(i, f, &|b| prop(b, part_number)),
            "revision" => part_feature(i, f, &|b| prop(b, revision)),
            _ => false,
        };
        if keep {
            out.insert(f.id);
        }
    }
    Some(out)
}

/// Feature types that modify existing parts (for `:part`).
fn modifies_bodies(f: &Feature) -> bool {
    matches!(f.type_id.as_str(), "fillet" | "chamfer" | "shell" | "draft" | "hole" | "booleanBodies" | "splitPart" | "transform" | "deleteFace" | "moveFace" | "replaceFace" | "faceBlend" | "modifyFillet" | "rib" | "mirror" | "linearPattern" | "circularPattern" | "curvePattern")
        || matches!(f.params.enum_("operationType"), "ADD" | "REMOVE" | "INTERSECT")
}

// ------------------------------------------------------------------ numeric fields

/// Onshape mouse-wheel step for numeric fields: 0.1, Ctrl 0.01, Shift 1.
pub fn wheel_step(ctrl: bool, shift: bool) -> f64 {
    if shift {
        1.0
    } else if ctrl {
        0.01
    } else {
        0.1
    }
}

/// Nudge a numeric field's text by `delta` (in the unit the text uses, or
/// the document unit for an expression). A plain number with an optional
/// unit keeps its unit and gets at least as many decimals as the step;
/// expressions are evaluated (with `scope`) and replaced by the result.
/// `min`/`max` clamp the result (in SI). Returns `None` if the text can't
/// be evaluated.
pub fn nudge_value_text(text: &str, delta: f64, dim: noshape_units::Dim, prefs: &noshape_units::UnitPrefs, scope: &dyn noshape_units::VariableScope, min: f64, max: f64) -> Option<String> {
    let t = text.trim();
    let num_end = t.char_indices().find(|(i, c)| !(c.is_ascii_digit() || *c == '.' || (*i == 0 && (*c == '-' || *c == '+')))).map(|(i, _)| i).unwrap_or(t.len());
    let (num, unit) = (&t[..num_end], t[num_end..].trim());
    let step_decimals = decimals_of(delta);
    let clamp_si = |si: f64| si.clamp(min.min(max), max.max(min));
    if let Ok(v) = num.parse::<f64>()
        && (unit.is_empty() || noshape_units::Unit::from_symbol(unit).is_some())
    {
        // Unit factor of the text's unit (or the document unit when bare).
        let factor = match noshape_units::Unit::from_symbol(unit) {
            Some(noshape_units::Unit::Length(u)) => u.meters(),
            Some(noshape_units::Unit::Angle(u)) => u.radians(),
            None => prefs.si_factor(dim),
        };
        let dec = decimals_of(v).max(step_decimals);
        let nv = round_to(clamp_si((v + delta) * factor) / factor, dec);
        let s = format!("{nv:.dec$}");
        let s = if s.starts_with('-') && nv == 0.0 { s[1..].to_string() } else { s };
        return Some(if unit.is_empty() { s } else if t[num_end..].starts_with(' ') { format!("{s} {unit}") } else { format!("{s}{unit}") });
    }
    let q = noshape_units::evaluate_as(t, dim, prefs, scope).ok()?;
    let factor = prefs.si_factor(dim);
    let v = q.value / factor;
    let nv = round_to(clamp_si((v + delta) * factor) / factor, step_decimals.max(prefs.length_precision as usize).min(6));
    let s = format!("{}", nv);
    Some(if dim.is_unitless() {
        s
    } else if dim == noshape_units::Dim::ANGLE {
        format!("{s} {}", prefs.angle.symbol())
    } else {
        format!("{s} {}", prefs.length.symbol())
    })
}

fn decimals_of(v: f64) -> usize {
    let s = format!("{v}");
    s.split_once('.').map(|(_, d)| d.len()).unwrap_or(0).min(6)
}

fn round_to(v: f64, decimals: usize) -> f64 {
    let p = 10f64.powi(decimals as i32);
    (v * p).round() / p
}

/// Value an expression evaluates to, formatted for display next to the
/// field (Onshape shows the evaluated value of expressions). `None` for
/// plain numbers (nothing to show) or invalid text.
pub fn evaluated_hint(text: &str, dim: noshape_units::Dim, prefs: &noshape_units::UnitPrefs, scope: &dyn noshape_units::VariableScope) -> Option<String> {
    let t = text.trim();
    let plain = t.char_indices().all(|(i, c)| c.is_ascii_digit() || c == '.' || c == ' ' || c.is_alphabetic() || (i == 0 && c == '-'));
    if t.is_empty() || (plain && !t.contains('#')) {
        return None;
    }
    let q = noshape_units::evaluate_as(t, dim, prefs, scope).ok()?;
    Some(noshape_units::format_quantity(q, prefs))
}

// ------------------------------------------------------------------ select other

/// One candidate of "Select other": an entity under the cursor.
#[derive(Clone, Debug, PartialEq)]
pub struct RayHit {
    pub reference: Reference,
    /// Distance along the ray (world units).
    pub depth: f64,
}

/// Faces and edges of visible parts under a ray, nearest first (Onshape's
/// Select other list: "all faces and edges, working from the one already
/// highlighted and into the part"). `edge_tol` is the pick distance for
/// edges in world units. At most `limit` entries.
pub fn entities_along_ray(display: &StudioDisplay, origin: DVec3, dir: DVec3, edge_tol: f64, skip: &dyn Fn(&crate::reference::BodyId) -> bool, limit: usize) -> Vec<RayHit> {
    let dir = dir.normalize_or_zero();
    let mut hits: Vec<RayHit> = Vec::new();
    let p = |a: [f32; 3]| DVec3::new(a[0] as f64, a[1] as f64, a[2] as f64);
    for b in display.bodies.iter().filter(|b| !b.hidden && !skip(&b.id)) {
        let m = &b.mesh;
        for (fi, &(first, count)) in m.face_ranges.iter().enumerate() {
            let Some(r) = b.face_refs.get(fi) else { continue };
            let mut best: Option<f64> = None;
            let tri = &m.indices[first as usize..(first + count) as usize];
            for t in tri.as_chunks::<3>().0 {
                let (Some(&a), Some(&bb), Some(&c)) = (m.positions.get(t[0] as usize), m.positions.get(t[1] as usize), m.positions.get(t[2] as usize)) else { continue };
                if let Some(d) = ray_triangle(origin, dir, p(a), p(bb), p(c)) {
                    best = Some(best.map_or(d, |x: f64| x.min(d)));
                }
            }
            if let Some(depth) = best {
                hits.push(RayHit { reference: r.clone(), depth });
            }
        }
        for (ei, pl) in m.edges.iter().enumerate() {
            let Some(r) = b.edge_refs.get(ei) else { continue };
            let mut best: Option<f64> = None;
            for w in pl.windows(2) {
                let (dist, depth) = ray_segment(origin, dir, p(w[0]), p(w[1]));
                if dist <= edge_tol && depth >= 0.0 {
                    best = Some(best.map_or(depth, |x: f64| x.min(depth)));
                }
            }
            if let Some(depth) = best {
                // Edges win ties with the faces they bound.
                hits.push(RayHit { reference: r.clone(), depth: depth - edge_tol * 0.5 });
            }
        }
    }
    hits.sort_by(|a, b| a.depth.total_cmp(&b.depth));
    hits.truncate(limit);
    hits
}

/// Möller–Trumbore; distance along the ray (both sides of the triangle).
fn ray_triangle(o: DVec3, d: DVec3, a: DVec3, b: DVec3, c: DVec3) -> Option<f64> {
    let (e1, e2) = (b - a, c - a);
    let pv = d.cross(e2);
    let det = e1.dot(pv);
    if det.abs() < 1e-18 {
        return None;
    }
    let inv = 1.0 / det;
    let tv = o - a;
    let u = tv.dot(pv) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let qv = tv.cross(e1);
    let v = d.dot(qv) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = e2.dot(qv) * inv;
    (t >= 0.0).then_some(t)
}

/// Closest distance between a ray and a segment, and the ray parameter there.
fn ray_segment(o: DVec3, d: DVec3, a: DVec3, b: DVec3) -> (f64, f64) {
    let u = b - a;
    let w = o - a;
    let (aa, bb, cc, dd, ee) = (d.dot(d), d.dot(u), u.dot(u), d.dot(w), u.dot(w));
    let den = aa * cc - bb * bb;
    let mut s = if den.abs() < 1e-18 { 0.0 } else { (bb * ee - cc * dd) / den };
    let mut t = if cc < 1e-18 { 0.0 } else { (ee + s * bb) / cc };
    if !(0.0..=1.0).contains(&t) {
        t = t.clamp(0.0, 1.0);
        s = (a + u * t - o).dot(d) / aa;
    }
    let p1 = o + d * s;
    let p2 = a + u * t;
    (p1.distance(p2), s)
}

/// Name a feature shows in the list and its dialog title: Onshape shows a
/// Variable as "#name = value" ("#?" until it is named) unless renamed.
pub fn display_name(f: &Feature) -> String {
    display_name_computed(f, &|_| None)
}

/// [`display_name`] with the parameters as the dialog shows them:
/// `shown(id)` = a computed value (a Hole's Up to next depth) or a value
/// converted to the workspace units ("6.35 mm" for "0.25 in"), as Onshape
/// fills a templated name ("Ø #holeDiameterV3 ↧ #holeDepthV3").
pub fn display_name_computed(f: &Feature, shown: &dyn Fn(&str) -> Option<String>) -> String {
    if f.type_id == "hole" && !f.custom_name
        && let Some(n) = hole_name(f, shown)
    {
        return n;
    }
    if f.type_id != "assignVariable" || f.custom_name {
        return f.name.clone();
    }
    let name = f.params.text("name");
    // Measured / From import: the evaluated value, "?" until there is one
    // (live: a new Measured variable reads "#? = ?").
    let value = if f.params.is("mode", "MEASURED") || f.params.is("mode", "TABLE") {
        Some(shown("measuredValue").unwrap_or_else(|| "?".into()))
    } else {
        match f.params.get("variableType") {
        Some(ParamValue::Enum(k)) if k == "ANGLE" => f.params.expr("angleValue").map(|e| e.text.clone()),
        Some(ParamValue::Enum(k)) if k == "NUMBER" => f.params.expr("numberValue").map(|e| e.text.clone()),
        Some(ParamValue::Enum(k)) if k == "ANY" => Some(f.params.text("anyValue").to_string()),
            _ => f.params.expr("lengthValue").map(|e| e.text.clone()),
        }
    }
    .unwrap_or_default();
    format!("#{} = {}", if name.trim().is_empty() { "?" } else { name.trim() }, value.trim())
}

/// hole.fs `getFeatureName`: "Ø 0.25 in ↧ 0.5 in", "M6x1 THRU",
/// "Ø 6.6 mm ↧ 10 mm | ⌴Ø 11 mm ↧ 6.5 mm", "Ø 6.6 mm THRU | ⌵Ø 12.4 mm X 90°"
/// (live dom/dialog-hole.txt).
fn hole_name(f: &Feature, shown: &dyn Fn(&str) -> Option<String>) -> Option<String> {
    let p = &f.params;
    let text = |id: &str| shown(id).or_else(|| p.expr(id).map(|e| e.text.trim().to_string())).filter(|t| !t.is_empty());
    let d = text("holeDiameterV3")?;
    let (pid, table) = crate::features::hole_tables::hole_table_param(p);
    let path = crate::features::hole_tables::complete_path(table, p.lookup(pid));
    // A tapped hole is named after its thread: "M6x1", "#10-24".
    let tapped = match path.as_slice() {
        [t, _, size, pitch, ..] if t == "Tapped" => {
            let mut it = pitch.split_whitespace();
            match (it.next(), it.next()) {
                (Some(n), Some("tpi")) => Some(format!("{size}-{n}")),
                (Some(n), Some("mm")) => Some(format!("{size}x{n}")),
                _ => Some(format!("{size}{pitch}")),
            }
        }
        _ => None,
    };
    let mut name = tapped.unwrap_or_else(|| format!("Ø {d}"));
    match p.enum_("endStyleV2") {
        "BLIND" => {
            if let Some(depth) = text("holeDepthV3") {
                name += &format!(" ↧ {depth}");
            }
        }
        "UP_TO_NEXT" | "UP_TO_ENTITY" => {
            if let Some(depth) = shown("holeDepthComputedV3").filter(|t| t != "Multiple") {
                name += &format!(" ↧ {depth}");
            }
        }
        "THROUGH" => name += " THRU",
        _ => {}
    }
    match p.enum_("styleV2") {
        "C_BORE" => {
            name += &format!(" | ⌴Ø {}", text("cBoreDiameterV3").unwrap_or_default());
            name += &format!(" ↧ {}", text("cBoreDepthV3").unwrap_or_default());
        }
        "C_SINK" => {
            name += &format!(" | ⌵Ø {}", text("cSinkDiameterV3").unwrap_or_default());
            let deg = p.expr("cSinkAngleV3").and_then(|e| e.cached.map(|q| q.value.to_degrees()).or_else(|| e.text.trim().trim_end_matches("deg").trim().parse().ok())).unwrap_or(90.0);
            let deg = (deg * 100.0).round() / 100.0;
            name += &format!(" X {deg}°");
        }
        _ => {}
    }
    Some(name)
}

/// The callout Onshape draws beside a hole while its Hole dialog is open
/// (live dialogs/hole-30, hole-35): "Ø6.35 THRU", with a fit
/// "Ø6.350 H7 +0.015/−0.000 THRU" (deviations stacked, see
/// [`crate::drawing::format::stacked`]), then counterbore / countersink
/// lines. Live (part-studio/431–439): one callout per feature, at its
/// middle hole, without a count; a blind depth reads "↧5".
pub fn hole_callout_3d(holes: &[crate::regen::context::HoleRecord], feature: FeatureId, units: &noshape_units::UnitPrefs) -> Option<(crate::regen::context::HoleRecord, Vec<String>)> {
    use crate::drawing::format::stacked;
    let mine: Vec<&crate::regen::context::HoleRecord> = holes.iter().filter(|h| h.feature == feature).collect();
    let hole = *mine.get(mine.len() / 2)?;
    let factor = units.si_factor(noshape_units::Dim::LENGTH);
    let len = |m: f64| noshape_units::format_number(m / factor, units.length_precision);
    // Toleranced values keep the workspace precision's zeros ("6.350").
    let fixed = |m: f64, p: u8| format!("{:.*}", usize::from(p), m / factor);
    let depth = |d: Option<f64>| match (hole.through || hole.exits, d) {
        (true, _) => " THRU".to_string(),
        (false, Some(d)) => format!(" ↧{}", len(d)),
        _ => String::new(),
    };
    let mut out = Vec::new();
    match &hole.thread {
        Some(t) => {
            let class = hole.thread_class.as_ref().map(|c| format!(" - {c}")).unwrap_or_default();
            out.push(format!("{t}{class}{}", depth(hole.tapped_depth.or(hole.depth))));
            if let Some(td) = hole.tap_drill {
                out.push(format!("Ø{}{}", len(td), depth(hole.depth)));
            }
        }
        None => match &hole.diameter_fit {
            Some(fit) => {
                let p = fit.precision.unwrap_or(units.length_precision);
                let class = fit.class.as_ref().map(|c| format!(" {c}")).unwrap_or_default();
                let dev = |m: f64, lower: bool| {
                    let s = fixed(m.abs(), p);
                    if m < 0.0 || (lower && m == 0.0) { format!("−{s}") } else { format!("+{s}") }
                };
                let devs = fit.deviations.map(|(u, l)| format!(" {}", stacked(&dev(u, false), &dev(l, true)))).unwrap_or_default();
                out.push(format!("Ø{}{class}{devs}{}", fixed(hole.diameter, p), depth(hole.depth)));
            }
            None => out.push(format!("Ø{}{}", len(hole.diameter), depth(hole.depth))),
        },
    }
    if let Some((d, dd)) = hole.cbore {
        out.push(format!("⌴ Ø{} ↧{}", len(d), len(dd)));
    }
    if let Some((d, ang)) = hole.csink {
        out.push(format!("⌵ Ø{} X {}°", len(d), noshape_units::format_number(ang.to_degrees(), 1)));
    }
    Some((hole.clone(), out))
}

/// Features whose selections reference `r` (Show dependencies of a default
/// plane or the origin).
pub fn features_using(features: &[Feature], r: &Reference) -> Vec<FeatureId> {
    fn uses(values: &ParamValues, r: &Reference) -> bool {
        values.0.values().any(|v| match v {
            ParamValue::Query(refs) => refs.contains(r),
            ParamValue::Array(items) => items.iter().any(|i| uses(i, r)),
            _ => false,
        })
    }
    features.iter().filter(|f| uses(&f.params, r)).map(|f| f.id).collect()
}

/// The sketch a feature was built from (Onshape's "Edit Sketch 1…" entry
/// for an extrude, revolve…): the first sketch its selections reference.
pub fn source_sketch(features: &[Feature], f: &Feature) -> Option<FeatureId> {
    fn walk(values: &ParamValues, out: &mut Vec<FeatureId>) {
        for v in values.0.values() {
            match v {
                ParamValue::Query(refs) => out.extend(refs.iter().filter_map(|r| match r {
                    Reference::SketchRegion { sketch, .. } | Reference::SketchEntity { sketch, .. } | Reference::Sketch(sketch) => Some(*sketch),
                    _ => None,
                })),
                ParamValue::Array(items) => items.iter().for_each(|i| walk(i, out)),
                _ => {}
            }
        }
    }
    let mut ids = Vec::new();
    walk(&f.params, &mut ids);
    ids.into_iter().find(|id| *id != f.id && features.iter().any(|g| g.id == *id && g.type_id == "sketch"))
}

/// Onshape: clicking a sketch curve while a region field is active picks
/// the region bounded by that curve; a curve between two regions (a hole's
/// circle) gives the smaller one. Region keys list their boundary as
/// `[ordinal, (entity << 1) | forward, ...]`.
pub fn region_for_curve(regions: &[crate::display::SketchRegionDisplay], entity: u32) -> Option<&crate::display::SketchRegionDisplay> {
    regions
        .iter()
        .filter(|r| r.key.iter().skip(1).any(|k| k >> 1 == entity))
        .min_by(|a, b| a.area.total_cmp(&b.area))
}

/// The Part Studio's Hole table (live L59, part-studio/2181): one table per
/// part ("Part 1"), columns Tag | Size | Drill angle | Qty, one row per
/// distinct hole (A, B, … in feature order) counting the identical ones;
/// Size is the callout, one line per part of it ("⌀5 mm ↧ 2 mm",
/// "⌵⌀20 mm X 90 deg", "M6x1.00 ↧ 1 mm"); Drill angle only when the tip
/// stays in the part.
pub fn hole_table(ctx: &crate::regen::context::Context, units: &noshape_units::UnitPrefs) -> Vec<crate::custom::table::CustomTable> {
    use crate::custom::table::{CustomTable, TableCell, TableColumn, TableRow};
    let factor = units.si_factor(noshape_units::Dim::LENGTH);
    let unit = if units.length == noshape_units::LengthUnit::Inch { "in" } else { "mm" };
    let len = |m: f64| format!("{} {unit}", noshape_units::format_number(m / factor, units.length_precision));
    let deg = |a: f64| format!("{} deg", noshape_units::format_number(a.to_degrees(), 3));
    let size = |h: &crate::regen::context::HoleRecord| -> String {
        let through = h.through || h.exits;
        let depth = |d: Option<f64>| if through { " THRU".to_string() } else { d.map(|d| format!(" ↧ {}", len(d))).unwrap_or_default() };
        let mut lines = vec![format!("⌀{}{}", len(h.diameter), depth(h.depth))];
        if let Some((d, depth)) = h.cbore {
            lines.push(format!("⌴⌀{} ↧ {}", len(d), len(depth)));
        }
        if let Some((d, a)) = h.csink {
            lines.push(format!("⌵⌀{} X {}", len(d), deg(a)));
        }
        if let Some(t) = &h.thread {
            let t = t.trim_end_matches(" mm");
            let tapped = match h.tapped_depth {
                Some(d) if !h.through => format!(" ↧ {}", len(d)),
                _ => " THRU".into(),
            };
            lines.push(format!("{t}{tapped}"));
        }
        lines.join("\n")
    };
    let mut out = Vec::new();
    for body in &ctx.bodies {
        let mine: Vec<&crate::regen::context::HoleRecord> = ctx.holes.iter().filter(|h| h.parts.contains(&body.id)).collect();
        if mine.is_empty() {
            continue;
        }
        let mut rows: Vec<(String, String, usize)> = Vec::new();
        for h in mine {
            let angle = match h.tip_angle {
                Some(a) if !(h.through || h.exits) => deg(a),
                _ => String::new(),
            };
            let s = size(h);
            match rows.iter_mut().find(|r| r.0 == s && r.1 == angle) {
                Some(r) => r.2 += 1,
                None => rows.push((s, angle, 1)),
            }
        }
        let col = |id: &str, name: &str| TableColumn { id: id.into(), name: name.into(), alignment: None };
        let cell = |t: String| TableCell { text: t, error: None, info: None };
        out.push(CustomTable {
            title: body.name.clone(),
            title_suffix: None,
            columns: vec![col("tag", "Tag"), col("size", "Size"), col("angle", "Drill angle"), col("qty", "Qty")],
            rows: rows
                .into_iter()
                .enumerate()
                .map(|(i, (s, a, n))| TableRow {
                    cells: vec![("tag".into(), cell(((b'A' + (i % 26) as u8) as char).to_string())), ("size".into(), cell(s)), ("angle".into(), cell(a)), ("qty".into(), cell(n.to_string()))],
                    entities: Vec::new(),
                })
                .collect(),
            entities: Vec::new(),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use noshape_units::{Dim, EmptyScope, UnitPrefs};

    fn feat(id: u64, ty: &str, name: &str, params: ParamValues) -> Feature {
        Feature { id: FeatureId(id), type_id: ty.into(), name: name.into(), suppressed: false, params, folder: None, custom_name: false, hidden: false, suppression_expr: None }
    }

    fn studio() -> PartStudio {
        let mut ext = ParamValues::default();
        ext.set("entities", ParamValue::Query(vec![Reference::SketchRegion { sketch: FeatureId(1001), key: vec![0, 2] }]));
        ext.set("depth", ParamValue::Expr(noshape_units::ExprValue::new("#d")));
        let mut var = ParamValues::default();
        var.set("name", ParamValue::Text("d".into()));
        let mut fil = ParamValues::default();
        fil.set("entities", ParamValue::Query(vec![Reference::Body(crate::reference::BodyId { feature: FeatureId(1003), index: 0 })]));
        let mut ps = PartStudio::new();
        ps.features = vec![
            feat(1001, "sketch", "Sketch 1", ParamValues::default()),
            feat(1002, "assignVariable", "Variable 1", var),
            feat(1003, "extrude", "Extrude 1", ext),
            feat(1004, "fillet", "Fillet 1", fil),
            feat(1005, "sketch", "Sketch 2", ParamValues::default()),
        ];
        ps
    }

    #[test]
    fn dependencies_follow_references_and_variables() {
        let ps = studio();
        assert_eq!(feature_dependencies(&ps.features, 2), vec![FeatureId(1001), FeatureId(1002)]);
        assert_eq!(feature_dependencies(&ps.features, 3), vec![FeatureId(1003)]);
        assert!(feature_dependencies(&ps.features, 4).is_empty());
        assert_eq!(feature_dependents(&ps.features, 0), vec![FeatureId(1003)]);
    }

    #[test]
    fn edge_selections_depend_on_the_creating_features_only() {
        use crate::reference::{Role, Source, TopoName};
        use std::sync::Arc;
        let mut ps = studio();
        // An extrude side face is named after the sketch curve it came from;
        // a fillet on its edge depends on the extrude, not on the sketch.
        let side = |e: u32| Arc::new(TopoName::face(FeatureId(1003), Role::Side(Source::Sketch { sketch: FeatureId(1001), entity: e, segment: 0 })));
        let edge = Reference::Edge(TopoName::edge(side(1), side(2), 0));
        ps.features[3].params.set("entities", ParamValue::Query(vec![edge]));
        assert_eq!(feature_dependencies(&ps.features, 3), vec![FeatureId(1003)]);
    }

    #[test]
    fn reorder_rules() {
        let ps = studio();
        // Moving Sketch 2 to the top is fine.
        let order = reordered(&ps.features, &[FeatureId(1005)], 0).unwrap();
        assert_eq!(order[0], FeatureId(1005));
        assert!(check_order(&ps.features, &order).is_ok());
        // Moving Extrude 1 above its sketch is refused with a message.
        let order = reordered(&ps.features, &[FeatureId(1003)], 0).unwrap();
        let err = check_order(&ps.features, &order).unwrap_err();
        assert!(err.contains("Extrude 1") && err.contains("Sketch 1"), "{err}");
        // Moving Sketch 1 below the extrude breaks the extrude.
        let order = reordered(&ps.features, &[FeatureId(1001)], 4).unwrap();
        assert_eq!(order.last(), Some(&FeatureId(1001)));
        assert!(check_order(&ps.features, &order).is_err());
        // Several features move together and keep their relative order.
        let order = reordered(&ps.features, &[FeatureId(1004), FeatureId(1003)], 4).unwrap();
        assert_eq!(&order[2..], &[FeatureId(1005), FeatureId(1003), FeatureId(1004)]);
        assert!(check_order(&ps.features, &order).is_ok());
        // A Variable moves below the feature using it (live qa5 pass 8).
        let order = reordered(&ps.features, &[FeatureId(1002)], 4).unwrap();
        assert!(check_order(&ps.features, &order).is_ok());
        // Live F33: the cursor's half of the hovered row decides, either way.
        assert_eq!(reordered_at(&ps.features, &[FeatureId(1001)], 3, false).unwrap()[2], FeatureId(1001), "down, upper half: before");
        assert_eq!(reordered_at(&ps.features, &[FeatureId(1001)], 3, true).unwrap()[3], FeatureId(1001), "down, lower half: after");
        assert_eq!(reordered_at(&ps.features, &[FeatureId(1005)], 1, false).unwrap()[1], FeatureId(1005), "up, upper half: before");
        assert_eq!(reordered_at(&ps.features, &[FeatureId(1005)], 1, true).unwrap()[2], FeatureId(1005), "up, lower half: after");
        // Dropping onto itself is a no-op.
        assert!(reordered(&ps.features, &[FeatureId(1003)], 2).is_none());
        let mut ps2 = ps.clone();
        apply_order(&mut ps2, &order);
        assert_eq!(ps2.features.iter().map(|f| f.id).collect::<Vec<_>>(), order);
    }

    /// Live (ui 10-25 c): a folder's Add expression… puts the expression on
    /// every feature inside it (none on the folder); unpacked, they keep it.
    #[test]
    fn a_folders_dynamic_suppression_goes_on_its_features() {
        use crate::display::FeatureStatus;
        use crate::regen::{RegenOptions, Regenerator};
        let mut ps = crate::demo::plate_studio();
        let ext = ps.features.iter().find(|f| f.type_id == "extrude").map(|f| f.id).expect("extrude");
        let run = |ps: &PartStudio| {
            let mut r = Regenerator::new();
            r.regenerate(ps, &Default::default(), &noshape_units::UnitPrefs::default(), &RegenOptions::default()).display
        };
        let inner = ps.new_folder(&[ext], None);
        let outer = ps.wrap_folder(inner, "Outer".into()).unwrap();
        ps.set_folder_suppression_expr(outer, Some("2 mm < 1 mm".into()));
        assert_eq!(ps.feature(ext).and_then(|f| f.suppression_expr.as_deref()), Some("2 mm < 1 mm"));
        assert_eq!(run(&ps).feature_state(ext).map(|s| s.status), Some(FeatureStatus::Suppressed));
        ps.dissolve_folder(outer);
        ps.dissolve_folder(inner);
        assert_eq!(ps.feature(ext).and_then(|f| f.suppression_expr.as_deref()), Some("2 mm < 1 mm"));
        assert_eq!(run(&ps).feature_state(ext).map(|s| s.status), Some(FeatureStatus::Suppressed));
        let f = ps.new_folder(&[ext], None);
        ps.set_folder_suppression_expr(f, None);
        assert_eq!(ps.feature(ext).and_then(|f| f.suppression_expr.clone()), None);
    }

    #[test]
    fn dynamic_suppression_and_boolean_expressions() {
        use crate::display::FeatureStatus;
        use crate::regen::{RegenOptions, Regenerator};
        let mut ps = crate::demo::plate_studio();
        let ext = ps.features.iter().position(|f| f.type_id == "extrude").expect("extrude");
        let id = ps.features[ext].id;
        let run = |ps: &PartStudio| {
            let mut r = Regenerator::new();
            r.regenerate(ps, &Default::default(), &noshape_units::UnitPrefs::default(), &RegenOptions::default()).display
        };
        // "Unsuppressed if": false suppresses, true keeps the feature.
        ps.features[ext].suppression_expr = Some("2 mm < 1 mm".into());
        assert_eq!(run(&ps).feature_state(id).map(|s| s.status), Some(FeatureStatus::Suppressed));
        ps.features[ext].suppression_expr = Some("2 mm > 1 mm".into());
        assert_eq!(run(&ps).feature_state(id).map(|s| s.status), Some(FeatureStatus::Ok));
        ps.features[ext].suppression_expr = Some("#nope > 1".into());
        assert_eq!(run(&ps).feature_state(id).map(|s| s.status), Some(FeatureStatus::Error));
        // A checkbox converted to an expression is evaluated.
        ps.features[ext].suppression_expr = None;
        ps.features[ext].params.set("oppositeDirection", ParamValue::Expr(noshape_units::ExprValue::new("3 > 2")));
        let d = run(&ps);
        assert_eq!(d.feature_state(id).map(|s| s.status), Some(FeatureStatus::Ok));
        let z = d.bodies[0].bounds.unwrap();
        assert!(z.0.z < -1e-6, "flipped extrude goes below the plane: {z:?}");
    }

    /// Live (part-studio/585–590): Add folder with nothing selected makes an
    /// empty "New folder (0)" at the rollback bar; with a selection it wraps
    /// it, collapsed.
    #[test]
    fn new_folder_empty_at_rollback_or_wrapping_the_selection() {
        let mut ps = studio();
        let last = ps.features.last().unwrap().id;
        let empty = ps.new_folder(&[], Some("New folder".into()));
        assert_eq!(ps.folder_count(empty), 0);
        assert_eq!(ps.folder(empty).and_then(|f| f.after), Some(last));
        assert_eq!(ps.empty_folders_after(Some(last)).len(), 1);
        let wrap = ps.new_folder(&[FeatureId(1003)], Some("New folder".into()));
        assert_eq!(ps.folder_count(wrap), 1);
        assert!(ps.folder(wrap).unwrap().collapsed);
    }

    /// Live (ui 10-25 b (1)): Boss 1 and Boss 2 selected → "New folder (3)"
    /// with Boss sketch 2 in between, in place.
    #[test]
    fn a_new_folder_takes_the_run_between_the_selected_features() {
        let mut ps = studio();
        let order: Vec<FeatureId> = ps.features.iter().map(|f| f.id).collect();
        let f = ps.new_folder(&[order[1], order[3]], None);
        assert_eq!(ps.features.iter().map(|f| f.id).collect::<Vec<_>>(), order);
        assert_eq!(ps.folder_count(f), 3);
        assert!(ps.features[1..=3].iter().all(|x| x.folder == Some(f)));
        // A folder inside the run moves in whole.
        let inner = ps.new_folder(&[order[2]], None);
        let outer_parent = ps.folder(inner).and_then(|x| x.parent);
        assert_eq!(outer_parent, Some(f));
        ps.dissolve_folder(f);
        let g = ps.new_folder(&[order[1], order[3]], None);
        assert_eq!(ps.folder(inner).and_then(|x| x.parent), Some(g));
        assert_eq!(ps.folder_count(g), 2);
    }

    /// Live (ui 10-25 b (2)): a folder's Add selection to folder… wraps it;
    /// Delete takes the folder with everything in it.
    #[test]
    fn folders_wrap_and_delete_with_their_contents() {
        let mut ps = studio();
        let inner = ps.new_folder(&[FeatureId(1003), FeatureId(1004)], None);
        let outer = ps.wrap_folder(inner, "New folder".into()).unwrap();
        assert_eq!(ps.folder(inner).and_then(|f| f.parent), Some(outer));
        assert_eq!(ps.folder_features(outer), vec![FeatureId(1003), FeatureId(1004)]);
        ps.delete_folder_tree(outer);
        assert!(ps.folders.is_empty());
        assert_eq!(ps.features.iter().map(|f| f.id).collect::<Vec<_>>(), vec![FeatureId(1001), FeatureId(1002), FeatureId(1005)]);
    }

    #[test]
    fn folders_nest_and_unpack_into_their_parent() {
        let mut ps = studio();
        let outer = ps.new_folder(&[FeatureId(1003), FeatureId(1004), FeatureId(1005)], None);
        let inner = ps.new_folder(&[FeatureId(1004), FeatureId(1005)], None);
        assert_eq!(ps.folder(inner).and_then(|f| f.parent), Some(outer));
        assert_eq!(ps.folder_chain(Some(inner)), vec![outer, inner]);
        ps.dissolve_folder(inner);
        assert_eq!(ps.feature(FeatureId(1005)).and_then(|f| f.folder), Some(outer));
        ps.dissolve_folder(outer);
        assert_eq!(ps.feature(FeatureId(1005)).and_then(|f| f.folder), None);
    }

    #[test]
    fn list_click_is_additive_and_shift_selects_ranges() {
        let order = [1, 2, 3, 4, 5];
        assert_eq!(list_click(&order, &[], None, &3, false), vec![3]);
        assert_eq!(list_click(&order, &[3], Some(&3), &5, false), vec![3, 5]);
        assert_eq!(list_click(&order, &[3, 5], Some(&5), &3, false), vec![5]);
        assert_eq!(list_click(&order, &[2], Some(&2), &4, true), vec![2, 3, 4]);
        assert_eq!(list_click(&order, &[4], Some(&4), &1, true), vec![4, 1, 2, 3]);
    }

    #[test]
    fn feature_filter_commands() {
        let mut ps = studio();
        ps.features[3].suppressed = true;
        let d = StudioDisplay::default();
        assert!(filter_features(&ps, &d, "  ").is_none());
        let by_name = filter_features(&ps, &d, "sketch").unwrap();
        assert!(by_name.contains(&FeatureId(1001)) && by_name.contains(&FeatureId(1005)) && by_name.len() == 2);
        // Type names match too ("Extrude").
        assert_eq!(filter_features(&ps, &d, ":type extr").unwrap().len(), 1);
        assert_eq!(filter_features(&ps, &d, ":suppressed").unwrap(), [FeatureId(1004)].into_iter().collect());
        assert_eq!(filter_features(&ps, &d, ":variable d").unwrap(), [FeatureId(1002), FeatureId(1003)].into_iter().collect());
        // Quoted = exact: `"d"` matches `#d` only.
        assert_eq!(filter_features(&ps, &d, ":variable \"d\"").unwrap(), [FeatureId(1002), FeatureId(1003)].into_iter().collect());
        assert!(filter_features(&ps, &d, ":variable \"dd\"").unwrap().is_empty());
        assert_eq!(filter_features(&ps, &d, "\"Variable 1\"").unwrap(), [FeatureId(1002)].into_iter().collect());
        assert!(filter_features(&ps, &d, ":errors").unwrap().is_empty());
    }

    #[test]
    fn part_number_and_revision_filters() {
        let mut ps = crate::demo::plate_studio();
        let r = crate::regen::Regenerator::new().regenerate(&ps, &Default::default(), &UnitPrefs::default(), &crate::regen::RegenOptions::default());
        let d = r.display;
        let body = d.bodies[0].id.clone();
        let props = ps.part_properties.entry(body).or_default();
        props.part_number = Some("PN-100".into());
        props.revision = Some("B".into());
        let by_pn = filter_features(&ps, &d, ":partnumber pn-1").unwrap();
        // The extrude that makes the part and the fillet that changes it.
        let names: HashSet<&str> = ps.features.iter().filter(|f| by_pn.contains(&f.id)).map(|f| f.type_id.as_str()).collect();
        assert!(names.contains("extrude") && names.contains("fillet"), "{names:?}");
        assert_eq!(filter_features(&ps, &d, ":revision b").unwrap(), by_pn);
        assert!(filter_features(&ps, &d, ":partnumber zz").unwrap().is_empty());
    }

    #[test]
    fn wheel_nudges_numeric_text() {
        let mm = UnitPrefs::default();
        let len = Dim::LENGTH;
        let n = |t: &str, d: f64| nudge_value_text(t, d, len, &mm, &EmptyScope, -10.0, 10.0);
        assert_eq!(n("25 mm", 0.1).as_deref(), Some("25.1 mm"));
        assert_eq!(n("25mm", -1.0).as_deref(), Some("24mm"));
        assert_eq!(n("1 in", 0.01).as_deref(), Some("1.01 in"));
        assert_eq!(n("2.5", 0.1).as_deref(), Some("2.6"));
        assert_eq!(n("0.05 mm", -0.1).as_deref(), Some("-0.05 mm"));
        // Expressions are evaluated and replaced.
        let r = n("10 mm + 5 mm", 1.0).unwrap();
        assert!(r.starts_with("16"), "{r}");
        // Clamped to the field's range (SI).
        assert_eq!(nudge_value_text("1", -1.0, Dim::UNITLESS, &mm, &EmptyScope, 1.0, 100.0).as_deref(), Some("1"));
        assert!(n("#nope", 0.1).is_none());
        assert_eq!(wheel_step(false, false), 0.1);
        assert_eq!(wheel_step(true, false), 0.01);
        assert_eq!(wheel_step(false, true), 1.0);
    }

    #[test]
    fn evaluated_hint_only_for_expressions() {
        let mm = UnitPrefs::default();
        assert!(evaluated_hint("25 mm", Dim::LENGTH, &mm, &EmptyScope).is_none());
        let h = evaluated_hint("10 mm * 2", Dim::LENGTH, &mm, &EmptyScope).unwrap();
        assert!(h.starts_with("20"), "{h}");
        let mut vars = std::collections::BTreeMap::new();
        vars.insert("w".to_string(), noshape_units::Quantity::new(0.004, Dim::LENGTH));
        let h = evaluated_hint("#w / 2", Dim::LENGTH, &mm, &vars).unwrap();
        assert!(h.starts_with('2'), "{h}");
    }

    #[test]
    fn ray_hits_sorted_by_depth() {
        use crate::display::{BodyDisplay, MeshBuffers};
        use crate::reference::{Role, TopoName};
        // Two parallel squares at z = 0 and z = 1, viewed from above.
        let sq = |z: f32| vec![[-1.0, -1.0, z], [1.0, -1.0, z], [1.0, 1.0, z], [-1.0, 1.0, z]];
        let mut positions = sq(0.0);
        positions.extend(sq(1.0));
        let mesh = MeshBuffers {
            positions,
            normals: vec![[0.0, 0.0, 1.0]; 8],
            indices: vec![0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7],
            face_ranges: vec![(0, 6), (6, 6)],
            edges: vec![vec![[-1.0, 0.0, 1.0], [1.0, 0.0, 1.0]]],
            vertices: vec![],
            mesh_lines: vec![],
        };
        let face = |s: u32| Reference::Face(TopoName::face(FeatureId(1), Role::New(s)));
        let edge = Reference::Edge(TopoName::edge(std::sync::Arc::new(TopoName::face(FeatureId(1), Role::New(1))), std::sync::Arc::new(TopoName::face(FeatureId(1), Role::New(2))), 0));
        let body = BodyDisplay {
            id: crate::reference::BodyId { feature: FeatureId(1), index: 0 },
            name: "Part 1".into(),
            kind: crate::display::BodyKind::Solid,
            color: [1.0; 4],
            hidden: false,
            mesh: std::sync::Arc::new(mesh),
            coarser: Vec::new(),
            levels_from: 0,
            face_refs: vec![face(0), face(1)],
            edge_refs: vec![edge.clone()],
            vertex_refs: vec![],
            bounds: None,
            composite: None,
            is_mesh: false,
            face_colors: Vec::new(),
        };
        let d = StudioDisplay { bodies: vec![body], ..Default::default() };
        let hits = entities_along_ray(&d, DVec3::new(0.0, 0.01, 5.0), DVec3::NEG_Z, 0.05, &|_| false, 20);
        let refs: Vec<_> = hits.iter().map(|h| h.reference.clone()).collect();
        assert_eq!(refs, vec![edge, face(1), face(0)]);
        assert!(entities_along_ray(&d, DVec3::new(0.0, 0.01, 5.0), DVec3::NEG_Z, 0.05, &|_| true, 20).is_empty());
    }

    #[test]
    fn sketch_curve_click_picks_the_bounded_region() {
        let region = |key: Vec<u32>, area: f64| crate::display::SketchRegionDisplay { key, positions: vec![], indices: vec![], area };
        // Rectangle (entities 1-4) with a hole (circle 7): the outer region is
        // bounded by both, the disc by the circle only.
        let regions = vec![region(vec![0, 2, 4, 6, 8, 15], 80.0), region(vec![1, 14], 3.0)];
        assert_eq!(region_for_curve(&regions, 2).map(|r| r.area), Some(80.0));
        assert_eq!(region_for_curve(&regions, 7).map(|r| r.area), Some(3.0));
        assert!(region_for_curve(&regions, 9).is_none());
    }

    #[test]
    fn source_sketch_of_an_extrude() {
        let ps = studio();
        assert_eq!(source_sketch(&ps.features, &ps.features[2]), Some(FeatureId(1001)));
        assert_eq!(source_sketch(&ps.features, &ps.features[3]), None);
    }

    #[test]
    fn features_using_a_plane() {
        let mut ps = studio();
        ps.features[0].params.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(crate::reference::DatumPlane::Top)]));
        assert_eq!(features_using(&ps.features, &Reference::Plane(crate::reference::DatumPlane::Top)), vec![FeatureId(1001)]);
        assert!(features_using(&ps.features, &Reference::Plane(crate::reference::DatumPlane::Front)).is_empty());
    }

    #[test]
    fn variables_show_their_assignment() {
        let mut ps = studio();
        assert_eq!(display_name(&ps.features[2]), "Extrude 1");
        let v = &mut ps.features[1];
        v.params.set("lengthValue", ParamValue::Expr(noshape_units::ExprValue::new("25 mm")));
        assert_eq!(display_name(v), "#d = 25 mm");
        v.params.set("name", ParamValue::Text(String::new()));
        assert_eq!(display_name(v), "#? = 25 mm");
        v.custom_name = true;
        assert_eq!(display_name(v), "Variable 1");
    }
}
