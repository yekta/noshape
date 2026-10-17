//! Feature type registry: schema (parameters, metadata) of every Part Studio
//! feature. Evaluation lives in `crate::eval`; schemas are pure data so the
//! UI can render any feature dialog generically.

use crate::params::ParamSpec;
use indexmap::IndexMap;
use std::sync::LazyLock;

pub mod common;
pub mod core;

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
    let mut map = IndexMap::new();
    for f in all {
        assert!(map.insert(f.id, f).is_none(), "duplicate feature id");
    }
    map
});

pub fn get(id: &str) -> Option<&'static FeatureType> {
    REGISTRY.get(id)
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
