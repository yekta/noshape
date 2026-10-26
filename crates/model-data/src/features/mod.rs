//! Feature type registry: schema (parameters, metadata) of every Part Studio
//! feature. Evaluation lives in `crate::eval`; schemas are pure data so the
//! UI can render any feature dialog generically.

use crate::params::ParamSpec;
use indexmap::IndexMap;
use std::sync::LazyLock;

pub mod boolean;
pub mod common;
pub mod core;
pub mod curve;
pub mod detail;
pub mod direct;
pub mod frames;
pub mod hole_tables;
pub mod importing;
pub mod pattern;
pub mod refgeom;
pub mod sheetmetal;
pub mod solid;
pub mod surface;
pub mod wrap;
#[cfg(test)]
mod tests_schema;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Category {
    Sketch,
    Solid,
    Detail,
    Pattern,
    Transform,
    Direct,
    Surface,
    Curve,
    Reference,
    SheetMetal,
    Frame,
    Variable,
    Import,
    /// Features defined by a Feature Studio of the document (`custom:` ids).
    Custom,
}

impl Category {
    pub fn label(self) -> &'static str {
        match self {
            Category::Sketch => "Sketch",
            Category::Solid => "Create",
            Category::Detail => "Detail",
            Category::Pattern => "Pattern",
            Category::Transform => "Transform",
            Category::Direct => "Direct edit",
            Category::Surface => "Surface",
            Category::Curve => "Curves",
            Category::Reference => "Reference",
            Category::SheetMetal => "Sheet metal",
            Category::Frame => "Frames",
            Category::Variable => "Variables",
            Category::Import => "Import",
            Category::Custom => "Custom features",
        }
    }
}

pub struct FeatureType {
    pub id: &'static str,
    pub name: &'static str,
    pub category: Category,
    /// Default keyboard shortcut (Onshape conventions), e.g. "shift-e".
    pub shortcut: Option<&'static str>,
    /// Icon name used by the UI (see app icons).
    pub icon: &'static str,
    pub description: &'static str,
    pub params: Vec<ParamSpec>,
    /// Shown in the main toolbar (vs. only in search / menus).
    pub in_toolbar: bool,
}

impl FeatureType {
    pub fn new(id: &'static str, name: &'static str, category: Category, params: Vec<ParamSpec>) -> Self {
        Self { id, name, category, shortcut: None, icon: id, description: "", params, in_toolbar: true }
    }
    pub fn shortcut(mut self, s: &'static str) -> Self {
        self.shortcut = Some(s);
        self
    }
    pub fn icon(mut self, s: &'static str) -> Self {
        self.icon = s;
        self
    }
    pub fn describe(mut self, s: &'static str) -> Self {
        self.description = s;
        self
    }
    pub fn hidden(mut self) -> Self {
        self.in_toolbar = false;
        self
    }
    pub fn param(&self, id: &str) -> Option<&ParamSpec> {
        find_param(&self.params, id)
    }
}

fn find_param<'a>(specs: &'a [ParamSpec], id: &str) -> Option<&'a ParamSpec> {
    for s in specs {
        if s.id == id {
            return Some(s);
        }
        if let crate::params::ParamKind::Array { item, .. } = &s.kind
            && let Some(p) = find_param(item, id)
        {
            return Some(p);
        }
    }
    None
}

static REGISTRY: LazyLock<IndexMap<&'static str, FeatureType>> = LazyLock::new(|| {
    let mut all: Vec<FeatureType> = Vec::new();
    core::register(&mut all);
    solid::register(&mut all);
    detail::register(&mut all);
    boolean::register(&mut all);
    pattern::register(&mut all);
    direct::register(&mut all);
    surface::register(&mut all);
    curve::register(&mut all);
    refgeom::register(&mut all);
    wrap::register(&mut all);
    sheetmetal::register(&mut all);
    frames::register(&mut all);
    importing::register(&mut all);
    let mut map = IndexMap::new();
    for f in all {
        let id = f.id;
        assert!(map.insert(id, f).is_none(), "duplicate feature id {id}");
    }
    map
});

/// Schema of a feature type: built-in, or a custom feature of a compiled
/// Feature Studio (`custom:<tab>:<name>`, see [`crate::custom`]).
pub fn get(id: &str) -> Option<&'static FeatureType> {
    REGISTRY.get(id).or_else(|| CUSTOM_LOOKUP.get().and_then(|f| f(id)))
}

/// Schemas of custom features: compiled Feature Studios live in noshape-model, which registers its lookup here
/// ([`set_custom_lookup`]) before it compiles any studio.
static CUSTOM_LOOKUP: std::sync::OnceLock<fn(&str) -> Option<&'static FeatureType>> = std::sync::OnceLock::new();

/// Register the custom feature schema lookup (noshape-model's `custom::lookup`); later calls are ignored.
pub fn set_custom_lookup(f: fn(&str) -> Option<&'static FeatureType>) {
    let _ = CUSTOM_LOOKUP.set(f);
}

pub fn all() -> impl Iterator<Item = &'static FeatureType> {
    REGISTRY.values()
}

pub fn by_category(c: Category) -> impl Iterator<Item = &'static FeatureType> {
    REGISTRY.values().filter(move |f| f.category == c)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::{ParamValues, fill_defaults};

    #[test]
    fn registry_builds_and_defaults_fill() {
        let units = noshape_units::UnitPrefs::default();
        for ty in all() {
            let mut v = ParamValues::default();
            fill_defaults(&ty.params, &mut v, &units);
            for p in &ty.params {
                assert!(v.get(p.id).is_some(), "{}.{} missing default", ty.id, p.id);
                // Visibility predicates must not panic on defaults.
                let _ = p.is_visible(&v);
            }
        }
        assert!(get("extrude").is_some());
    }
}
