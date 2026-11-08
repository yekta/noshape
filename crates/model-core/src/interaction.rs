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
    // Its in-feature mate connectors' references are its own.
    std::iter::once(f).chain(&f.sub_features).flat_map(|g| g.params.0.values()).for_each(|v| param_refs(v, &mut ints));
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

/// Apply an order to a Part Studio. Folders follow the feature
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

/// A feature list query split into its command (lowercase, `""` for plain
/// text) and argument, quotes taken off (`exact`). `None`: the query doesn't
/// filter, being empty or a command still waiting for its argument (live
/// 10-27: `:type` and `:part` alone list everything).
fn parse_filter(query: &str) -> Option<(String, String, bool)> {
    let q = query.trim();
    if q.is_empty() {
        return None;
    }
    let (cmd, arg) = match q.strip_prefix(':') {
        Some(rest) => {
            let mut it = rest.splitn(2, char::is_whitespace);
            // A bare ":" is an unknown command: it matches nothing (live).
            (it.next().filter(|c| !c.is_empty()).unwrap_or(":").to_lowercase(), it.next().unwrap_or("").trim().to_string())
        }
        None => (String::new(), q.to_string()),
    };
    if arg.is_empty() && matches!(cmd.as_str(), "type" | "name" | "folder" | "variable" | "part" | "partnumber" | "revision") {
        return None;
    }
    // A quoted argument matches exactly (Help: `:part "Part 1"`; live
    // Variable table › Filter feature list puts `:variable "height"`).
    let exact = arg.len() >= 2 && arg.starts_with('"') && arg.ends_with('"');
    let arg = if exact { arg[1..arg.len() - 1].to_string() } else { arg };
    Some((cmd, arg, exact))
}

/// Whether a list row that isn't a feature (a Default geometry row, a part)
/// stays under the query, `None` for no filtering: plain text and `:name`
/// match its name, `:hidden` / `:shown` its visibility (live 10-27:
/// `:hidden` lists the hidden default planes, `:shown` the Origin and the
/// parts); the other commands keep only features.
pub fn filter_row(query: &str, name: &str, hidden: bool) -> Option<bool> {
    let (cmd, arg, exact) = parse_filter(query)?;
    let (name, arg) = (name.to_lowercase(), arg.to_lowercase());
    let name_match = arg.is_empty() || if exact { name == arg } else { name.contains(&arg) };
    Some(match cmd.as_str() {
        "" | "name" => name_match,
        "hidden" => hidden && name_match,
        "shown" => !hidden && name_match,
        _ => false,
    })
}

/// Feature list search (Onshape "Filter by feature name or type name"):
/// plain text matches names and type names; `:errors`, `:warnings`,
/// `:suppressed`, `:hidden`, `:shown`, `:type x`, `:name x`, `:folder x`,
/// `:variable x`, `:part x`, `:partnumber x` and `:revision x` narrow
/// further (the last two by the parts' properties). Returns the matching
/// features, or `None` when the query doesn't filter ([`parse_filter`]).
pub fn filter_features(studio: &PartStudio, display: &StudioDisplay, query: &str) -> Option<HashSet<FeatureId>> {
    let (cmd, arg, exact) = parse_filter(query)?;
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
                // The ray in an edge's reach is in its faces' too: one it doesn't meet comes just behind the edge
                // (live ui-parity face-menu-select-other: through the box's bottom back edge, the bottom face, the
                // Top plane, then the back face).
                if let Reference::Edge(crate::reference::TopoName::Edge { faces, .. }) = r {
                    for f in faces.iter().map(|f| Reference::Face((**f).clone())) {
                        if b.face_refs.contains(&f) && !hits.iter().any(|h| h.reference == f) {
                            hits.push(RayHit { reference: f, depth: depth + edge_tol * 0.5 });
                        }
                    }
                }
            }
        }
    }
    hits.sort_by(|a, b| a.depth.total_cmp(&b.depth));
    hits.truncate(limit);
    hits
}

/// The shown planes (default and construction) the ray passes through inside their drawn rectangles, nearest first
/// (Select other lists them among the parts' entities: live ui-parity face-menu-select-other).
pub fn planes_along_ray(display: &StudioDisplay, origin: DVec3, dir: DVec3) -> Vec<RayHit> {
    let dir = dir.normalize_or_zero();
    let mut hits: Vec<RayHit> = Vec::new();
    for pl in display.planes.iter().filter(|p| !p.hidden) {
        let f = &pl.frame;
        let den = dir.dot(f.normal);
        if den.abs() < 1e-12 {
            continue;
        }
        let depth = (f.origin - origin).dot(f.normal) / den;
        if depth < 0.0 {
            continue;
        }
        let q = origin + dir * depth - f.origin;
        let (u, v) = (q.dot(f.x) - pl.center.x, q.dot(f.y) - pl.center.y);
        if u.abs() <= pl.half_width && v.abs() <= pl.half_height {
            hits.push(RayHit { reference: pl.reference.clone(), depth });
        }
    }
    hits.sort_by(|a, b| a.depth.total_cmp(&b.depth));
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
    // std's name template "Routing curve (#curveLength)" (live 1722:
    // "Routing curve (61.825 mm)", "(0 mm)" until it builds).
    if f.type_id == "routingCurve" && !f.custom_name {
        return format!("Routing curve ({})", shown("curveLength").unwrap_or_else(|| "0 mm".into()));
    }
    // std's name template "###name" (live ui-parity query-variable-tool: "#?" until it is named).
    if f.type_id == "queryVariable" && !f.custom_name {
        let name = f.params.text("name");
        return format!("#{}", if name.trim().is_empty() { "?" } else { name.trim() });
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
            // A sketch uses what it projects, and the Origin when a constraint
            // holds it there (live: "Dependencies of Origin" lists the sketch,
            // behaviours request 1663 (b)).
            ParamValue::Sketch(s) => s.external_refs.values().any(|x| x == r) || (matches!(r, Reference::Origin) && constrained_to_origin(s)),
            _ => false,
        })
    }
    features.iter().filter(|f| uses(&f.params, r)).map(|f| f.id).collect()
}

/// A constraint holds the sketch's fixed point at its origin (where the
/// origin's inferences put their constraints, `sketch_mode::origin_point`).
fn constrained_to_origin(s: &crate::sketch_feature::SketchData) -> bool {
    s.sketch.entities().any(|e| e.external && !s.external_refs.contains_key(&e.id.0) && matches!(e.geometry, noshape_sketch::Geometry::Point { pos } if pos.length() < 1e-12) && !s.sketch.constraints_on(e.id).is_empty())
}

/// The sketch a feature was built from (Onshape's "Edit Sketch 1…" entry
/// for an extrude, revolve…): the first sketch its selections reference.
pub fn source_sketch(features: &[Feature], f: &Feature) -> Option<FeatureId> {
    used_sketches(features, f).into_iter().next()
}

/// Every sketch a feature's selections reference, in order (a loft's
/// profiles; live hides them all when the feature is made).
pub fn used_sketches(features: &[Feature], f: &Feature) -> Vec<FeatureId> {
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
    let mut out: Vec<FeatureId> = Vec::new();
    for id in ids {
        if id != f.id && !out.contains(&id) && features.iter().any(|g| g.id == id && g.type_id == "sketch") {
            out.push(id);
        }
    }
    out
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
pub fn hole_table(ctx: &crate::regen::context::Context, units: &noshape_units::UnitPrefs) -> Vec<crate::custom::CustomTable> {
    use crate::custom::{CustomTable, TableCell, TableColumn, TableRow};
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

/// draft.fs draftEditLogic (flipAlongPull): with the first parting edge
/// picked, the "Switch face" value that drafts the steeper of the edge's two
/// faces (the one nearer parallel to the pull; on a tie the face against the
/// pull). None when the pull or the edge doesn't resolve.
pub fn draft_switch_face(ctx: &crate::regen::context::Context, params: &ParamValues, units: &noshape_units::UnitPrefs) -> Option<bool> {
    use noshape_kernel::ShapeType;
    let vars = std::collections::BTreeMap::new();
    let ev = crate::regen::eval::EvalCtx::new(FeatureId(0), "", params, units, ctx.clone(), &vars);
    let pull = ev.resolve_direction(params.query("pullDirectionEntity").first()?)?.try_normalize()?;
    let &(bi, ei) = ev.resolve_edges(params.query("partingEdges")).first()?;
    let shape = &ctx.bodies[bi].shape;
    let edge = shape.sub_shape(ShapeType::Edge, ei).ok()?;
    let info = edge.edge_info().ok()?;
    let mid = edge.point_at((info.first_param + info.last_param) * 0.5).ok()?;
    let faces = shape.edge_faces().ok()?.get(ei)?.clone();
    // (along the pull, steepness) of each face.
    let data: Vec<(bool, f64)> = faces
        .iter()
        .filter_map(|&f| {
            let face = shape.sub_shape(ShapeType::Face, f as usize).ok()?;
            let c = face.centroid().ok()?;
            let n = face.normal_at_point(c).ok()?.try_normalize()?;
            Some(((c - mid).dot(pull) >= 0.0, 1.0 - n.dot(pull).abs()))
        })
        .collect();
    let along = data.iter().find(|d| d.0)?.1;
    let against = data.iter().find(|d| !d.0)?.1;
    Some(along > against + 1e-9)
}

/// Helix's manipulators as std's helix.fs adds them, keyed for its
/// helixManipulatorChange: Axis or Circle with Height (not Turns and pitch),
/// the height arrow from the helix's start along its axis as picked, its
/// offset the height (negative: Opposite direction); Cylinder / Cone, the
/// direction flip on the axis halfway along the face; Turns and pitch (any
/// axis), the direction flip at the helix's end (its start plus turns ×
/// pitch its way), the last std adds. Empty while the dialog can't place it.
pub fn helix_manipulators(ctx: &crate::regen::context::Context, params: &ParamValues, units: &noshape_units::UnitPrefs, vars: &std::collections::BTreeMap<String, noshape_units::Quantity>) -> Vec<(String, crate::display::FeatureManipulator)> {
    use crate::display::FeatureManipulator as M;
    let ev = crate::regen::eval::EvalCtx::new(FeatureId(0), "", params, units, ctx.clone(), vars);
    let Ok((start, way, _, _, _, face_height)) = crate::regen::features::curves::helix_frame(&ev) else { return Vec::new() };
    let opposite = params.bool("oppositeDirection");
    // std's axis: the way before Opposite direction.
    let axis = if opposite { -way } else { way };
    let flip = |base: DVec3| vec![("directionManipulator".to_string(), M::Flip { base, direction: axis, flipped: opposite, other_direction: None, secondary: false })];
    if params.is("pathType", "TURNS_PITCH") {
        let (Ok(pitch), Ok(turns)) = (ev.length("helicalPitch"), ev.real("revolutions")) else { return Vec::new() };
        return flip(start + way * pitch * turns);
    }
    if let Some(h) = face_height {
        return flip(start + way * h * 0.5);
    }
    if !params.is("endType", "HEIGHT") {
        return Vec::new();
    }
    let Ok(height) = ev.length("height") else { return Vec::new() };
    vec![("heightManipulator".to_string(), M::Linear { base: start, direction: axis, offset: if opposite { -height } else { height }, min: -500.0, max: 500.0, secondary: false, primary: Some("height".into()) })]
}

/// Move curve boundary's distance arrow (moveCurveBoundary.fs: Extend, Blind, with a help point): the first
/// curve's end that grows and its outward tangent there, the distance measured from it (std's arrow, at the
/// extended end with no offset, its tip in the same place).
pub fn move_curve_boundary_anchor(ctx: &crate::regen::context::Context, params: &ParamValues, units: &noshape_units::UnitPrefs) -> Option<(DVec3, DVec3)> {
    let vars = std::collections::BTreeMap::new();
    let ev = crate::regen::eval::EvalCtx::new(FeatureId(0), "", params, units, ctx.clone(), &vars);
    crate::regen::features::curves::extend_curve_anchor(&ev)
}

/// Face blend's side flips as std's faceBlend.fs adds them (addFlipManipulators; caps and limits aren't built
/// here): on each side's first face at its middle by its parameters, along its normal, reversed unless the side is
/// "aligned" (wallsNormalAlignment: sides with a common edge both are when that edge is concave; apart, a side is
/// when its normal at the closest point leans toward the other side), flipped by Flip side 1 / 2.
pub fn face_blend_manipulators(ctx: &crate::regen::context::Context, params: &ParamValues, units: &noshape_units::UnitPrefs) -> Vec<(String, crate::display::FeatureManipulator)> {
    face_blend_manipulators_opt(ctx, params, units).unwrap_or_default()
}

fn face_blend_manipulators_opt(ctx: &crate::regen::context::Context, params: &ParamValues, units: &noshape_units::UnitPrefs) -> Option<Vec<(String, crate::display::FeatureManipulator)>> {
    use crate::display::FeatureManipulator as M;
    let vars = std::collections::BTreeMap::new();
    let ev = crate::regen::eval::EvalCtx::new(FeatureId(0), "", params, units, ctx.clone(), &vars);
    let (s1, s2) = (ev.resolve_faces(params.query("side1")), ev.resolve_faces(params.query("side2")));
    let (&(b1, f1), &(b2, f2)) = (s1.first()?, s2.first()?);
    let (fa, fb) = (ev.face_shape(b1, f1).ok()?, ev.face_shape(b2, f2).ok()?);
    let middle = |f: &noshape_kernel::Shape| -> Option<(DVec3, DVec3)> {
        let [u0, u1, v0, v1] = f.uv_bounds().ok()?;
        let (u, v) = ((u0 + u1) * 0.5, (v0 + v1) * 0.5);
        Some((f.point_at_uv(u, v).ok()?, f.normal_at_uv(u, v).ok()?.normalize()))
    };
    let (ma, mb) = (middle(&fa)?, middle(&fb)?);
    let d = noshape_kernel::distance(&fa, &fb).ok()?;
    let aligned = if b1 == b2 && d.distance <= 1e-9 {
        // A common edge: concave when the other side's middle lies outside the first's plane.
        let n = fa.normal_at_point(d.point_a).ok()?;
        let concave = n.dot(mb.0 - d.point_a) > 0.0;
        [concave, concave]
    } else {
        let v = (d.point_b - d.point_a).normalize();
        [fa.normal_at_point(d.point_a).ok()?.dot(v) >= -1e-5, fb.normal_at_point(d.point_b).ok()?.dot(v) >= -1e-5]
    };
    let flip = |key: &str, (base, n): (DVec3, DVec3), aligned: bool, flipped: bool| (key.to_string(), M::Flip { base, direction: if aligned { n } else { -n }, flipped, other_direction: None, secondary: false });
    Some(vec![flip("side1FlipManipulator", ma, aligned[0], params.bool("flipSide1")), flip("side2FlipManipulator", mb, aligned[1], params.bool("flipSide2"))])
}

/// Mutual trim's keep-side flips as std's mutualTrim.fs adds them (createManipulators): where the two surfaces
/// meet (the middle of the first edge of their section), each in its own surface across that edge, turned away
/// from the other surface's normal, flipped by its Keep opposite side.
pub fn mutual_trim_manipulators(ctx: &crate::regen::context::Context, params: &ParamValues, units: &noshape_units::UnitPrefs) -> Vec<(String, crate::display::FeatureManipulator)> {
    mutual_trim_manipulators_opt(ctx, params, units).unwrap_or_default()
}

fn mutual_trim_manipulators_opt(ctx: &crate::regen::context::Context, params: &ParamValues, units: &noshape_units::UnitPrefs) -> Option<Vec<(String, crate::display::FeatureManipulator)>> {
    use crate::display::FeatureManipulator as M;
    let vars = std::collections::BTreeMap::new();
    let ev = crate::regen::eval::EvalCtx::new(FeatureId(0), "", params, units, ctx.clone(), &vars);
    let body = |q: &str| ev.resolve_bodies(params.query(q)).first().map(|&b| ctx.bodies[b].shape.clone());
    let (a, b) = (body("body1")?, body("body2")?);
    let edge = noshape_kernel::section(&a, &b).ok()?.shape.edges().into_iter().next()?;
    let info = edge.edge_info().ok()?;
    let t = (info.first_param + info.last_param) * 0.5;
    let (p, x) = (edge.point_at(t).ok()?, edge.tangent_at(t).ok()?);
    let normal = |s: &noshape_kernel::Shape| -> Option<DVec3> {
        let v = noshape_kernel::make_vertex(p).ok()?;
        let f = s.faces().into_iter().min_by(|f, g| {
            let d = |x: &noshape_kernel::Shape| noshape_kernel::distance(x, &v).map(|d| d.distance).unwrap_or(f64::MAX);
            d(f).total_cmp(&d(g))
        })?;
        f.normal_at_point(p).ok()
    };
    let (n1, n2) = (normal(&a)?, normal(&b)?);
    let across = |n: DVec3, other: DVec3| {
        let d = n.cross(x).normalize_or_zero();
        if d.dot(other) > 0.0 { -d } else { d }
    };
    let flip = |key: &str, direction: DVec3, flipped: bool| (key.to_string(), M::Flip { base: p, direction, flipped, other_direction: None, secondary: false });
    Some(vec![
        flip("Keep first surface opposite side manipulator", across(n1, n2), params.bool("keepOtherSide1")),
        flip("Keep second surface opposite side manipulator", across(n2, n1), params.bool("keepOtherSide2")),
    ])
}

/// A partial fillet's bound manipulators as std's fillet.fs adds them
/// (addPartialFilletManipulators), keyed as std keys them for its
/// filletManipulatorChange: for each Position or Offset bound, an arrow
/// along the chain's tangent whose tip is the bound's point, its offset the
/// chain length times the bound's fraction (measured from the other end
/// unless Opposite direction).
pub fn partial_fillet_manipulators(ctx: &crate::regen::context::Context, params: &ParamValues, units: &noshape_units::UnitPrefs) -> Vec<(String, crate::display::FeatureManipulator)> {
    let mut out = Vec::new();
    if !params.bool("isPartial") || params.is("blendControlType", "WIDTH") {
        return out;
    }
    let vars = std::collections::BTreeMap::new();
    let ev = crate::regen::eval::EvalCtx::new(FeatureId(0), "", params, units, ctx.clone(), &vars);
    let edges = ev.resolve_edges(params.query("entities"));
    let Some(&(bi, _)) = edges.first() else { return out };
    let mine: Vec<usize> = edges.iter().filter(|(b, _)| *b == bi).map(|(_, e)| *e).collect();
    let shape = &ctx.bodies[bi].shape;
    let Some((_, _, total)) = crate::regen::features::blend_section::partial_point(shape, &mine, 0.0) else { return out };
    let length_of = |id: &str| params.expr(id).and_then(|x| x.cached.map(|q| q.value).or_else(|| noshape_units::evaluate_as(&x.text, noshape_units::Dim::LENGTH, units, &noshape_units::EmptyScope).ok().map(|q| q.value)));
    let real_of = |id: &str| params.expr(id).and_then(|x| x.cached.map(|q| q.value).or_else(|| noshape_units::evaluate_as(&x.text, noshape_units::Dim::UNITLESS, units, &noshape_units::EmptyScope).ok().map(|q| q.value)));
    let opposite = params.bool("partialOppositeParameter");
    let bounds = [("startPartialType", "partialFirstEdgeTotalParameter", "startPartialOffset", false), ("endPartialType", "partialSecondEdgeTotalParameter", "endPartialOffset", true)];
    for (index, (ty, frac, off, second)) in bounds.into_iter().enumerate() {
        if second && !params.bool("secondBound") {
            continue;
        }
        let (fraction, primary) = match params.enum_(ty) {
            "OFFSET" => {
                let Some(d) = length_of(off) else { continue };
                (if second { (total - d) / total } else { d / total }, off)
            }
            "ENTITY" => continue,
            _ => {
                let Some(f) = real_of(frac) else { continue };
                (f, frac)
            }
        };
        let Some((point, tangent, _)) = crate::regen::features::blend_section::partial_point(shape, &mine, fraction) else { continue };
        let param = if opposite { fraction } else { 1.0 - fraction };
        let direction = if opposite { -tangent } else { tangent };
        let offset = total * param;
        out.push((
            format!("partialPointOnEdgeManipulator.{index}"),
            crate::display::FeatureManipulator::Linear { base: point + direction * offset, direction: -direction, offset, min: 0.0, max: total, secondary: false, primary: Some(primary.to_string()) },
        ));
    }
    out
}

/// A parting line Draft's manipulators as std's draft.fs adds them/// A parting line Draft's manipulators as std's draft.fs adds them
/// (addReferenceEntityDraftManipulators), keyed as std keys them so std's
/// draftManipulatorChange applies their moves: on the last parting edge, its
/// two faces ordered more-along-the-pull first; One sided: the flip arrow
/// (quarter way along, into the drafted face) and the angle arc on the face
/// drafted; Symmetric: the arc on the more away face (the along one's
/// negated); Two sided: the angle and second angle arcs. An arc turns about
/// the edge's middle across the face from an inch along (or against) the
/// pull. Empty when the dialog has no edge or pull direction yet.
pub fn draft_parting_manipulators(ctx: &crate::regen::context::Context, params: &ParamValues, units: &noshape_units::UnitPrefs) -> Vec<(String, crate::display::FeatureManipulator)> {
    draft_parting_manipulators_opt(ctx, params, units).unwrap_or_default()
}

fn draft_parting_manipulators_opt(ctx: &crate::regen::context::Context, params: &ParamValues, units: &noshape_units::UnitPrefs) -> Option<Vec<(String, crate::display::FeatureManipulator)>> {
    use crate::display::FeatureManipulator as M;
    use noshape_kernel::{Orientation, ShapeType};
    let vars = std::collections::BTreeMap::new();
    let ev = crate::regen::eval::EvalCtx::new(FeatureId(0), "", params, units, ctx.clone(), &vars);
    let pull = ev.resolve_direction(params.query("pullDirectionEntity").first()?)?.try_normalize()?;
    let &(bi, ei) = ev.resolve_edges(params.query("partingEdges")).last()?;
    let shape = &ctx.bodies[bi].shape;
    let edge = shape.sub_shape(ShapeType::Edge, ei).ok()?;
    let info = edge.edge_info().ok()?;
    let faces: Vec<noshape_kernel::Shape> = shape.edge_faces().ok()?.get(ei)?.iter().filter_map(|&f| shape.sub_shape(ShapeType::Face, f as usize).ok()).collect();
    // A face's point, coedge direction and normal at `frac` along its coedge.
    let at = |face: &noshape_kernel::Shape, frac: f64| -> Option<(DVec3, DVec3, DVec3)> {
        let rev = face.edge_orientation_in(&edge).ok()? == Orientation::Reversed;
        let t = info.first_param + (info.last_param - info.first_param) * if rev { 1.0 - frac } else { frac };
        let p = edge.point_at(t).ok()?;
        let d = edge.tangent_at(t).ok()?.try_normalize()?;
        Some((p, if rev { -d } else { d }, face.normal_at_point(p).ok()?.try_normalize()?))
    };
    struct Data {
        steep: f64,
        half: (DVec3, DVec3, DVec3),
        quarter: (DVec3, DVec3, DVec3),
    }
    let mut data: Vec<(noshape_kernel::Shape, f64)> = Vec::new();
    for f in &faces {
        let (_, d, n) = at(f, 0.5)?;
        data.push((f.clone(), n.dot(d.cross(pull).try_normalize()?)));
    }
    if data.len() == 1 {
        data.push(data[0].clone());
    }
    let tol = 1e-9;
    if data.len() != 2 || (data[0].1 - data[1].1).abs() <= tol {
        return None;
    }
    // More along first; the quarter points of the two meet (0.25 and 0.75 along opposite coedges).
    if data[1].1 > data[0].1 {
        data.swap(0, 1);
    }
    let ordered: Vec<Data> = data.iter().zip([0.25, 0.75]).map(|((f, steep), q)| Some(Data { steep: *steep, half: at(f, 0.5)?, quarter: at(f, q)? })).collect::<Option<_>>()?;
    let angle_of = |id: &str| params.expr(id).and_then(|x| x.cached.map(|q| q.value).or_else(|| noshape_units::evaluate_as(&x.text, noshape_units::Dim::ANGLE, units, &noshape_units::EmptyScope).ok().map(|q| q.value)));
    let max = 89.9f64.to_radians();
    let arc = |d: &Data, angle: f64, flipped: bool, secondary: bool, primary: &str| -> Option<M> {
        if d.steep.abs() <= tol {
            return None;
        }
        let (o, _, n) = d.half;
        let along = d.steep > 0.0;
        Some(M::Angular { axis_origin: o, axis_direction: n.cross(pull).try_normalize()?, rotation_origin: o + pull * if along { 0.0254 } else { -0.0254 }, angle: if flipped { -angle } else { angle }, min: -max, max, secondary, primary: Some(primary.to_string()) })
    };
    let angle = angle_of("angle")?;
    let pull_flip = params.bool("pullDirection");
    let mut out = Vec::new();
    match params.enum_("partingLineSides") {
        "TWO_SIDED" => {
            // The more away face carries one; the other too when one is along and one away.
            let index = if ordered[1].steep <= ordered[0].steep + tol { 1 } else { 0 };
            let second = angle_of("secondAngle")?;
            let first_of = |d: &Data| if d.steep > 0.0 { ("secondAngleManipulator", second, params.bool("secondPullDirection"), true, "secondAngle") } else { ("angleManipulator", angle, pull_flip, false, "angle") };
            let (m, o) = (&ordered[index], &ordered[1 - index]);
            let (k, a, f, sec, p) = first_of(m);
            let added = arc(m, a, f, sec, p);
            let other = (added.is_none() || (m.steep > 0.0) != (o.steep > 0.0)).then(|| {
                let (k, a, f, sec, p) = first_of(o);
                arc(o, a, f, sec, p).map(|x| (k, x))
            });
            out.extend(added.map(|x| (k.to_string(), x)));
            out.extend(other.flatten().map(|(k, x)| (k.to_string(), x)));
        }
        sides => {
            let index = if sides == "ONE_SIDED" { if params.bool("alongPull") { 0 } else { 1 } } else if ordered[1].steep <= ordered[0].steep + tol { 1 } else { 0 };
            let m = &ordered[index];
            if sides == "ONE_SIDED" && faces.len() == 2 {
                let (o, d, n) = m.quarter;
                out.push(("flipManipulator".to_string(), M::Flip { base: o, direction: n.cross(d).try_normalize()?, flipped: false, other_direction: None, secondary: false }));
            }
            let along = m.steep > 0.0;
            let (key, a) = if sides == "SYMMETRIC" && along { ("symmetricAlongAngleManipulator", -angle) } else { ("angleManipulator", angle) };
            out.extend(arc(m, a, pull_flip, false, "angle").map(|x| (key.to_string(), x)));
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use noshape_units::{Dim, EmptyScope, UnitPrefs};

    fn feat(id: u64, ty: &str, name: &str, params: ParamValues) -> Feature {
        Feature { id: FeatureId(id), type_id: ty.into(), name: name.into(), suppressed: false, params, folder: None, custom_name: false, hidden: false, suppression_expr: None, sub_features: Vec::new() }
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
        // Moving Sketch 1 below the extrude (live: done all the same).
        let order = reordered(&ps.features, &[FeatureId(1001)], 4).unwrap();
        assert_eq!(order.last(), Some(&FeatureId(1001)));
        // Several features move together and keep their relative order.
        let order = reordered(&ps.features, &[FeatureId(1004), FeatureId(1003)], 4).unwrap();
        assert_eq!(&order[2..], &[FeatureId(1005), FeatureId(1003), FeatureId(1004)]);
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
        // Live 10-27: ":" alone matches nothing, ":type" / ":part" without
        // an argument don't filter.
        assert!(filter_features(&ps, &d, ":").unwrap().is_empty());
        assert!(filter_features(&ps, &d, ":type").is_none() && filter_features(&ps, &d, ":part ").is_none());
    }

    #[test]
    fn filter_rows_by_name_and_visibility() {
        assert_eq!(filter_row("", "Top", true), None);
        assert_eq!(filter_row("to", "Top", false), Some(true));
        assert_eq!(filter_row("extru", "Part 1", false), Some(false));
        assert_eq!(filter_row(":hidden", "Top", true), Some(true));
        assert_eq!(filter_row(":hidden", "Origin", false), Some(false));
        assert_eq!(filter_row(":shown", "Part 1", false), Some(true));
        assert_eq!(filter_row(":errors", "Part 1", false), Some(false));
        assert_eq!(filter_row(":", "Origin", false), Some(false));
        assert_eq!(filter_row(":type", "Origin", false), None);
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

    /// Select other's planes (live face-menu-select-other): those the ray crosses inside their drawn rectangle, by depth.
    #[test]
    fn planes_along_a_ray() {
        use crate::display::{PlaneDisplay, PlaneFrame};
        use crate::reference::DatumPlane;
        let plane = |d: DatumPlane, origin: DVec3, x: DVec3, y: DVec3| PlaneDisplay { reference: Reference::Plane(d), name: String::new(), frame: PlaneFrame { origin, x, y, normal: x.cross(y) }, half_width: 1.0, half_height: 1.0, center: glam::DVec2::ZERO, hidden: false };
        let d = StudioDisplay { planes: vec![plane(DatumPlane::Top, DVec3::ZERO, DVec3::X, DVec3::Y), plane(DatumPlane::Front, DVec3::new(0.0, 0.0, 2.0), DVec3::X, DVec3::Y), plane(DatumPlane::Right, DVec3::new(5.0, 0.0, 0.0), DVec3::Y, DVec3::Z)], ..Default::default() };
        let hits = planes_along_ray(&d, DVec3::new(0.2, 0.3, 5.0), DVec3::NEG_Z);
        // The z = 2 plane, then z = 0; the x = 5 plane is edge-on to the ray (and its rectangle elsewhere).
        assert_eq!(hits.iter().map(|h| h.reference.clone()).collect::<Vec<_>>(), vec![Reference::Plane(DatumPlane::Front), Reference::Plane(DatumPlane::Top)]);
        assert!((hits[0].depth - 3.0).abs() < 1e-12 && (hits[1].depth - 5.0).abs() < 1e-12);
        // Outside a plane's rectangle: not crossed.
        assert!(planes_along_ray(&d, DVec3::new(3.0, 0.0, 5.0), DVec3::NEG_Z).is_empty());
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

    /// An edge in the ray's reach brings in its face the ray passes beside, just behind the edge (live ui-parity
    /// face-menu-select-other: a box's bottom face, the Top plane, then its back face).
    #[test]
    fn an_edge_in_reach_brings_its_other_face() {
        use crate::display::{BodyDisplay, MeshBuffers};
        use crate::reference::{Role, TopoName};
        // The bottom (z = 0) and the back (y = 1) of a box, sharing the edge y = 1, z = 0.
        let mesh = MeshBuffers {
            positions: vec![[-1.0, -1.0, 0.0], [1.0, -1.0, 0.0], [1.0, 1.0, 0.0], [-1.0, 1.0, 0.0], [-1.0, 1.0, 0.0], [1.0, 1.0, 0.0], [1.0, 1.0, 2.0], [-1.0, 1.0, 2.0]],
            normals: vec![[0.0, 0.0, -1.0], [0.0, 0.0, -1.0], [0.0, 0.0, -1.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0], [0.0, 1.0, 0.0], [0.0, 1.0, 0.0], [0.0, 1.0, 0.0]],
            indices: vec![0, 2, 1, 0, 3, 2, 4, 5, 6, 4, 6, 7],
            face_ranges: vec![(0, 6), (6, 6)],
            edges: vec![vec![[-1.0, 1.0, 0.0], [1.0, 1.0, 0.0]]],
            vertices: vec![],
            mesh_lines: vec![],
        };
        let name = |s: u32| std::sync::Arc::new(TopoName::face(FeatureId(1), Role::New(s)));
        let face = |s: u32| Reference::Face((*name(s)).clone());
        let edge = Reference::Edge(TopoName::edge(name(0), name(1), 0));
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
        // Straight down 0.02 in front of the edge: it meets the bottom, passes beside the back.
        let hits = entities_along_ray(&d, DVec3::new(0.0, 0.98, 5.0), DVec3::NEG_Z, 0.05, &|_| false, 20);
        let refs: Vec<_> = hits.iter().map(|h| h.reference.clone()).collect();
        assert_eq!(refs, vec![edge, face(0), face(1)]);
        // Out of the edge's reach, only the bottom.
        let far = entities_along_ray(&d, DVec3::new(0.0, 0.5, 5.0), DVec3::NEG_Z, 0.05, &|_| false, 20);
        assert_eq!(far.iter().map(|h| h.reference.clone()).collect::<Vec<_>>(), vec![face(0)]);
    }

    #[test]
    fn sketch_curve_click_picks_the_bounded_region() {
        let region = |key: Vec<u32>, area: f64| crate::display::SketchRegionDisplay { key, positions: vec![], indices: vec![], area, imprinted: false };
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

    /// core-workflow 8.4.11: a sketch whose corner is coincident with the origin is a dependent of Origin (live
    /// request 1663 (b)); one whose origin point carries no constraint is not.
    #[test]
    fn features_using_the_origin() {
        use noshape_sketch::{ConstraintKind, Geometry};
        let mut ps = studio();
        let mut data = crate::sketch_feature::SketchData::default();
        let line = data.sketch.add_line(glam::DVec2::new(0.0, 0.0), glam::DVec2::new(40.0, 0.0));
        let o = data.sketch.use_external_point(glam::DVec2::ZERO);
        ps.features[0].params.set("sketch", ParamValue::Sketch(Box::new(data.clone())));
        assert!(features_using(&ps.features, &Reference::Origin).is_empty(), "no constraint on the origin point");
        let Some(Geometry::Line { start, .. }) = data.sketch.entity(line).map(|e| e.geometry.clone()) else { panic!("a line") };
        data.sketch.add_constraint(ConstraintKind::Coincident { a: start, b: o }).unwrap();
        ps.features[0].params.set("sketch", ParamValue::Sketch(Box::new(data)));
        assert_eq!(features_using(&ps.features, &Reference::Origin), vec![FeatureId(1001)]);
        assert!(features_using(&ps.features, &Reference::Plane(crate::reference::DatumPlane::Top)).is_empty());
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
        // Query variable: "###name" (live ui-parity query-variable-tool).
        let mut q = PartStudio::new().new_feature("queryVariable", &noshape_units::UnitPrefs::default()).unwrap();
        assert_eq!(display_name(&q), "#?");
        q.params.set("name", ParamValue::Text("holes".into()));
        assert_eq!(display_name(&q), "#holes");
    }
}
