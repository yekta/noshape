//! Variable lookup for expression evaluation.

use std::collections::{BTreeMap, HashMap};

use indexmap::IndexMap;

use crate::Quantity;

/// Something that can resolve `#name` references (name given without `#`).
pub trait VariableScope {
    fn get(&self, name: &str) -> Option<Quantity>;
}

/// A scope with no variables.
#[derive(Clone, Copy, Debug, Default)]
pub struct EmptyScope;

impl VariableScope for EmptyScope {
    fn get(&self, _: &str) -> Option<Quantity> {
        None
    }
}

impl<S: std::hash::BuildHasher> VariableScope for HashMap<String, Quantity, S> {
    fn get(&self, name: &str) -> Option<Quantity> {
        HashMap::get(self, name).copied()
    }
}

impl VariableScope for BTreeMap<String, Quantity> {
    fn get(&self, name: &str) -> Option<Quantity> {
        BTreeMap::get(self, name).copied()
    }
}

impl<S: std::hash::BuildHasher> VariableScope for IndexMap<String, Quantity, S> {
    fn get(&self, name: &str) -> Option<Quantity> {
        IndexMap::get(self, name).copied()
    }
}

impl VariableScope for [(&str, Quantity)] {
    fn get(&self, name: &str) -> Option<Quantity> {
        self.iter().rev().find(|(n, _)| *n == name).map(|(_, q)| *q)
    }
}

impl<T: VariableScope + ?Sized> VariableScope for &T {
    fn get(&self, name: &str) -> Option<Quantity> {
        (**self).get(name)
    }
}

/// Adapts a closure into a scope.
pub struct FnScope<F>(pub F);

impl<F: Fn(&str) -> Option<Quantity>> VariableScope for FnScope<F> {
    fn get(&self, name: &str) -> Option<Quantity> {
        (self.0)(name)
    }
}

/// Looks names up in `inner` first, then in `outer`.
pub struct LayeredScope<'a> {
    pub inner: &'a dyn VariableScope,
    pub outer: &'a dyn VariableScope,
}

impl VariableScope for LayeredScope<'_> {
    fn get(&self, name: &str) -> Option<Quantity> {
        self.inner.get(name).or_else(|| self.outer.get(name))
    }
}
