//! Consistency checks over every registered feature schema.

use super::*;
use crate::params::{Hints, NumKind, ParamKind, ParamValue, ParamValues, fill_defaults};
use noshape_units::{LengthUnit, UnitPrefs};
use std::collections::HashSet;

/// Feature ids that must be registered (FeatureScript feature names).
const REQUIRED: &[&str] = &[
    // core
    "sketch",
    "extrude",
    "fillet",
    "cPlane",
    "assignVariable",
    // solid
    "revolve",
    "sweep",
    "loft",
    "thicken",
    "enclose",
    "rib",
    "shell",
    "hole",
    "externalThread",
    // detail
    "chamfer",
    "faceBlend",
    "modifyFillet",
    "draft",
    "bodyDraft",
    // boolean
    "booleanBodies",
    "splitPart",
    "deleteBodies",
    // pattern / transform
    "linearPattern",
    "circularPattern",
    "curvePattern",
    "mirror",
    "transform",
    // direct edit
    "deleteFace",
    "moveFace",
    "replaceFace",
    // surface
    "offsetSurface",
    "fill",
    "extendSurface",
    "boundarySurface",
    "ruledSurface",
    "mutualTrim",
    "constrainedSurface",
    // curve
    "helix",
    "fitSpline",
    "projectCurves",
    "bridgingCurve",
    "compositeCurve",
    "intersectionCurve",
    "trimCurve",
    "isocline",
    "offsetCurveOnFace",
    "isoparametricCurve",
    "editCurve",
    "routingCurve",
    // reference / misc
    "mateConnector",
    "compositePart",
    "queryVariable",
    "wrap",
    "decal",
    // sheet metal
    "sheetMetalStart",
    "sheetMetalFlange",
    "sheetMetalHem",
    "sheetMetalTab",
    "sheetMetalBend",
    "sheetMetalJog",
    "sheetMetalFormed",
    "sheetMetalLoft",
    "sheetMetalMakeJoint",
    "sheetMetalCorner",
    "sheetMetalBendRelief",
    "sheetMetalCornerBreak",
    "sheetMetalJoint",
    "sheetMetalEnd",
    "pcbStart",
    "pcbEnd",
    // frames
    "frame",
    "frameTrim",
    "gusset",
    "endcap",
    "cutlist",
    "tag",
    // import
    "importDerived",
    "importForeign",
];

fn unit_prefs() -> [UnitPrefs; 2] {
    let metric = UnitPrefs::default();
    let imperial = UnitPrefs { length: LengthUnit::Inch, ..UnitPrefs::default() };
    [metric, imperial]
}

/// Visit every spec list of a feature: the top level and each array item.
fn spec_lists(specs: &[ParamSpec], out: &mut Vec<(String, Vec<ParamSpec>)>, path: &str) {
    out.push((path.to_string(), specs.to_vec()));
    for s in specs {
        if let ParamKind::Array { item, .. } = &s.kind {
            spec_lists(item, out, &format!("{path}.{}", s.id));
        }
    }
}

fn all_lists(ty: &FeatureType) -> Vec<(String, Vec<ParamSpec>)> {
    let mut out = Vec::new();
    spec_lists(&ty.params, &mut out, ty.id);
    out
}

/// Every parameter list with the parameters in scope around it: array items
/// see their feature's values (the dialog evaluates item visibility on the
/// feature's values overlaid with the item's, like FeatureScript's
/// `definition.isAsymmetric` inside a `vertexSettings` loop).
fn scoped_lists(specs: &[ParamSpec], outer: &[ParamSpec], out: &mut Vec<(String, Vec<ParamSpec>, Vec<ParamSpec>)>, path: &str) {
    out.push((path.to_string(), specs.to_vec(), outer.to_vec()));
    let scope: Vec<ParamSpec> = outer.iter().chain(specs).cloned().collect();
    for s in specs {
        if let ParamKind::Array { item, .. } = &s.kind {
            scoped_lists(item, &scope, out, &format!("{path}.{}", s.id));
        }
    }
}

#[test]
fn required_features_exist() {
    for id in REQUIRED {
        assert!(get(id).is_some(), "missing feature {id}");
    }
}

#[test]
fn feature_ids_unique() {
    // Build the list again without the registry's assert so a duplicate is
    // reported by name.
    let mut seen = HashSet::new();
    let mut n = 0;
    for ty in all() {
        assert!(seen.insert(ty.id), "duplicate feature id {}", ty.id);
        n += 1;
    }
    assert!(n >= REQUIRED.len());
}

#[test]
fn param_ids_unique_per_list() {
    for ty in all() {
        for (path, list) in all_lists(ty) {
            let mut seen = HashSet::new();
            for s in &list {
                assert!(seen.insert(s.id), "{path}: duplicate parameter id {}", s.id);
            }
        }
    }
}

#[test]
fn defaults_fill_and_visibility_runs() {
    for units in unit_prefs() {
        for ty in all() {
            let mut v = ParamValues::default();
            fill_defaults(&ty.params, &mut v, &units);
            for p in &ty.params {
                assert!(v.get(p.id).is_some(), "{}.{} missing default", ty.id, p.id);
                let _ = p.is_visible(&v);
            }
            // Array items: a default item must fill and evaluate as well.
            for (path, list) in all_lists(ty).into_iter().skip(1) {
                let mut item = ParamValues::default();
                fill_defaults(&list, &mut item, &units);
                for p in &list {
                    assert!(item.get(p.id).is_some(), "{path}.{} missing default", p.id);
                    let _ = p.is_visible(&item);
                }
            }
        }
    }
}

#[test]
fn enum_defaults_are_options() {
    for ty in all() {
        for (path, list) in all_lists(ty) {
            for s in &list {
                if let ParamKind::Enum { options, default } = &s.kind {
                    assert!(!options.is_empty(), "{path}.{}: no options", s.id);
                    assert!(options.iter().any(|o| o.0 == *default), "{path}.{}: default {default} not an option", s.id);
                    let ids: HashSet<_> = options.iter().map(|o| o.0).collect();
                    assert_eq!(ids.len(), options.len(), "{path}.{}: duplicate option ids", s.id);
                }
            }
        }
    }
}

#[test]
fn numeric_defaults_within_bounds() {
    for ty in all() {
        for (path, list) in all_lists(ty) {
            for s in &list {
                if let ParamKind::Num(n) = &s.kind {
                    assert!(n.min <= n.max, "{path}.{}: min > max", s.id);
                    for d in [n.default_metric, n.default_imperial] {
                        // Allow float noise from unit conversion.
                        let eps = 1e-9 * (1.0 + d.abs());
                        assert!(d >= n.min - eps && d <= n.max + eps, "{path}.{}: default {d} outside [{}, {}]", s.id, n.min, n.max);
                    }
                    if n.kind == NumKind::Integer {
                        assert_eq!(n.default_metric.fract(), 0.0, "{path}.{}: integer default", s.id);
                    }
                }
            }
        }
    }
}

#[test]
fn lookup_tables_resolve() {
    for ty in all() {
        for (path, list) in all_lists(ty) {
            for s in &list {
                if let ParamKind::Lookup { table } = &s.kind {
                    let d = hole_tables::default_path(table);
                    assert!(!d.is_empty() && hole_tables::is_complete(table, &d), "{path}.{}: table {table} has no complete default path", s.id);
                }
            }
        }
    }
}

#[test]
fn metadata_sane() {
    for ty in all() {
        assert!(!ty.name.is_empty() && !ty.description.is_empty(), "{}: name/description", ty.id);
        assert!(!ty.params.is_empty(), "{}: no params", ty.id);
        for (path, list) in all_lists(ty) {
            for s in &list {
                assert!(!s.label.is_empty(), "{path}.{}: empty label", s.id);
                assert!(s.id.chars().all(|c| c.is_ascii_alphanumeric()), "{path}.{}: bad id", s.id);
            }
        }
    }
    let shortcuts: Vec<_> = all().filter_map(|t| t.shortcut).collect();
    let unique: HashSet<_> = shortcuts.iter().collect();
    assert_eq!(unique.len(), shortcuts.len(), "duplicate shortcuts {shortcuts:?}");
}

/// Tiny deterministic PRNG (xorshift) so the test needs no extra crates.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// Randomize every enum and boolean (including hidden state parameters) and
/// check that each non-hidden parameter is visible in at least one sample.
/// Catches visibility predicates that test misspelled ids or impossible
/// combinations.
/// Asserts that every parameter of `list` shows for some choice of the
/// booleans and enums of `list` and of the `outer` parameters in scope.
fn check_reachable(path: &str, list: &[ParamSpec], outer: &[ParamSpec], units: &UnitPrefs, rng: &mut Rng) {
    let mut base = ParamValues::default();
    fill_defaults(outer, &mut base, units);
    fill_defaults(list, &mut base, units);
    let mut seen: HashSet<&str> = HashSet::new();
    for sample in 0..4000 {
        let mut v = base.clone();
        if sample > 0 {
            for s in outer.iter().chain(list) {
                match &s.kind {
                    ParamKind::Bool { .. } => v.set(s.id, ParamValue::Bool(rng.below(2) == 1)),
                    ParamKind::Enum { options, .. } => v.set(s.id, ParamValue::Enum(options[rng.below(options.len())].0.to_string())),
                    _ => {}
                }
            }
        }
        for s in list {
            if s.is_visible(&v) {
                seen.insert(s.id);
            }
        }
        if seen.len() == list.len() {
            break;
        }
    }
    for s in list {
        if !s.hints.has(Hints::ALWAYS_HIDDEN) {
            assert!(seen.contains(s.id), "{path}.{} is never visible", s.id);
        }
    }
}

#[test]
fn every_parameter_can_become_visible() {
    let units = UnitPrefs::default();
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    for ty in all() {
        let mut lists = Vec::new();
        scoped_lists(&ty.params, &[], &mut lists, ty.id);
        for (path, list, outer) in lists {
            check_reachable(&path, &list, &outer, &units, &mut rng);
        }
    }
}

#[test]
fn categories_populated() {
    for c in [
        Category::Solid,
        Category::Detail,
        Category::Pattern,
        Category::Transform,
        Category::Direct,
        Category::Surface,
        Category::Curve,
        Category::Reference,
        Category::SheetMetal,
        Category::Frame,
        Category::Variable,
        Category::Import,
    ] {
        assert!(by_category(c).next().is_some(), "no feature in {c:?}");
    }
}

#[test]
#[should_panic(expected = "never visible")]
fn reachability_check_detects_dead_params() {
    let list = vec![ParamSpec::bool("flag", "Flag", false), ParamSpec::bool("dead", "Dead", false).visible(|v| v.is("misspelled", "X"))];
    check_reachable("test", &list, &[], &UnitPrefs::default(), &mut Rng(1));
}
