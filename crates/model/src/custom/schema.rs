//! Run-time feature schemas for custom features.
//!
//! The feature registry and the generic dialog work with
//! `&'static FeatureType` / `&'static str`. Custom schemas are therefore
//! interned: every distinct string and every distinct schema is leaked once
//! and reused, so recompiling a Feature Studio (or editing only function
//! bodies) allocates nothing new. Only edits to a declaration leak a new,
//! small `FeatureType`.
//!
//! The *active* table maps type ids to the schema of the most recently
//! compiled source of their Feature Studio; [`crate::features::get`] falls
//! back to it.

use super::compile::{DeclKind, ParamDecl, VisibleValue};
use crate::document::TabId;
use crate::features::{Category, FeatureType};
use crate::params::{NumKind, NumSpec, ParamKind, ParamSpec, QueryFilter};
use parking_lot::{Mutex, RwLock};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

static STRINGS: LazyLock<Mutex<HashSet<&'static str>>> = LazyLock::new(Default::default);
type Options = &'static [(&'static str, &'static str)];
static OPTIONS: LazyLock<Mutex<HashMap<Vec<(String, String)>, Options>>> = LazyLock::new(Default::default);
static SCHEMAS: LazyLock<Mutex<HashMap<String, &'static FeatureType>>> = LazyLock::new(Default::default);
static ACTIVE: LazyLock<RwLock<HashMap<String, &'static FeatureType>>> = LazyLock::new(Default::default);

fn intern(s: &str) -> &'static str {
    let mut set = STRINGS.lock();
    if let Some(x) = set.get(s) {
        return x;
    }
    let leaked: &'static str = Box::leak(s.to_string().into_boxed_str());
    set.insert(leaked);
    leaked
}

fn intern_options(opts: &[(String, String)]) -> Options {
    let mut map = OPTIONS.lock();
    if let Some(x) = map.get(opts) {
        return x;
    }
    let v: Vec<(&'static str, &'static str)> = opts.iter().map(|(a, b)| (intern(a), intern(b))).collect();
    let leaked: Options = Box::leak(v.into_boxed_slice());
    map.insert(opts.to_vec(), leaked);
    leaked
}

fn param_spec(d: &ParamDecl) -> ParamSpec {
    let num = |kind: NumKind, default: f64, min: f64, max: f64| NumSpec { kind, default_metric: default, default_imperial: default, min, max };
    let id = intern(&d.id);
    let label = intern(&d.label);
    let mut spec = match &d.kind {
        DeclKind::Query { filter, max_picks } => match max_picks {
            Some(1) => ParamSpec::query1(id, label, QueryFilter(*filter)),
            _ => ParamSpec::query(id, label, QueryFilter(*filter)),
        },
        DeclKind::Length { default, min, max } => ParamSpec::num(id, label, num(NumKind::Length, *default, *min, *max)),
        DeclKind::Angle { default, min, max } => ParamSpec::num(id, label, num(NumKind::Angle, *default, *min, *max)),
        DeclKind::Real { default, min, max } => ParamSpec::num(id, label, num(NumKind::Real, *default, *min, *max)),
        DeclKind::Int { default, min, max } => ParamSpec::num(id, label, num(NumKind::Integer, *default as f64, *min as f64, *max as f64)),
        DeclKind::Bool { default } => ParamSpec::bool(id, label, *default),
        DeclKind::Enum { options, default } => {
            let opts = intern_options(options);
            ParamSpec::enum_(id, label, opts).default_enum(intern(default))
        }
        DeclKind::Text { default } => ParamSpec::text(id, label, intern(default)),
    };
    if let DeclKind::Query { max_picks: Some(n), .. } = &d.kind
        && let ParamKind::Query { max_picks, .. } = &mut spec.kind
    {
        *max_picks = Some(*n);
    }
    if !d.description.is_empty() {
        spec = spec.describe(intern(&d.description));
    }
    if let Some((other, value)) = &d.visible_when {
        let other = other.clone();
        spec = match value.clone() {
            VisibleValue::Bool(b) => spec.visible(move |v| v.bool(&other) == b),
            VisibleValue::Option(o) => spec.visible(move |v| v.enum_(&other) == o),
        };
    }
    spec
}

/// The (interned) feature type for a declaration.
pub(super) fn build_type(type_id: &str, name: &str, icon: &str, description: &str, params: &[ParamDecl]) -> &'static FeatureType {
    let key = format!("{type_id}\u{1}{name}\u{1}{icon}\u{1}{description}\u{1}{params:?}");
    if let Some(t) = SCHEMAS.lock().get(&key) {
        return t;
    }
    let mut t = FeatureType::new(intern(type_id), intern(name), Category::Custom, params.iter().map(param_spec).collect()).icon(intern(icon));
    if !description.is_empty() {
        t = t.describe(intern(description));
    }
    let leaked: &'static FeatureType = Box::leak(Box::new(t));
    SCHEMAS.lock().insert(key, leaked);
    leaked
}

/// Make `types` the active custom features of a Feature Studio tab
/// (replacing what it declared before).
pub(super) fn activate(tab: TabId, types: &[&'static FeatureType]) {
    let prefix = super::type_id(tab, "");
    let mut active = ACTIVE.write();
    let unchanged = active.keys().filter(|k| k.starts_with(&prefix)).count() == types.len() && types.iter().all(|t| active.get(t.id).is_some_and(|x| std::ptr::eq(*x, *t)));
    if unchanged {
        return;
    }
    active.retain(|k, _| !k.starts_with(&prefix));
    for t in types {
        active.insert(t.id.to_string(), t);
    }
}

/// Schema of an active custom feature type.
pub fn lookup(type_id: &str) -> Option<&'static FeatureType> {
    if !super::is_custom(type_id) {
        return None;
    }
    ACTIVE.read().get(type_id).copied()
}
