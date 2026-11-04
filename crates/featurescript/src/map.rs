//! FeatureScript's map: entries sorted by key, as a short vector while small (a ValueWithUnits' two, a plane's
//! three: one allocation of their size, where a B-tree's node held eleven; a 512 × 512 Displacement map's values
//! took 1.5 GB), a B-tree past that. Iteration, comparison and lookup as a `BTreeMap` (by `Value`'s order).

use crate::value::Value;
use std::collections::BTreeMap;
use std::collections::btree_map;

/// Entries kept in a vector up to this many.
const SMALL: usize = 16;

#[derive(Clone, Debug)]
pub struct Map(Repr);

#[derive(Clone, Debug)]
enum Repr {
    Small(Vec<(Value, Value)>),
    Big(BTreeMap<Value, Value>),
}

impl Default for Map {
    fn default() -> Self {
        Map::new()
    }
}

impl Map {
    pub const fn new() -> Self {
        Map(Repr::Small(Vec::new()))
    }

    pub fn len(&self) -> usize {
        match &self.0 {
            Repr::Small(v) => v.len(),
            Repr::Big(m) => m.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn find(v: &[(Value, Value)], key: &Value) -> Result<usize, usize> {
        v.binary_search_by(|(k, _)| k.cmp(key))
    }

    pub fn get(&self, key: &Value) -> Option<&Value> {
        match &self.0 {
            Repr::Small(v) => Self::find(v, key).ok().map(|i| &v[i].1),
            Repr::Big(m) => m.get(key),
        }
    }

    pub fn get_mut(&mut self, key: &Value) -> Option<&mut Value> {
        match &mut self.0 {
            Repr::Small(v) => Self::find(v, key).ok().map(|i| &mut v[i].1),
            Repr::Big(m) => m.get_mut(key),
        }
    }

    pub fn contains_key(&self, key: &Value) -> bool {
        self.get(key).is_some()
    }

    pub fn insert(&mut self, key: Value, value: Value) -> Option<Value> {
        let v = match &mut self.0 {
            Repr::Big(m) => return m.insert(key, value),
            Repr::Small(v) => v,
        };
        match Self::find(v, &key) {
            Ok(i) => Some(std::mem::replace(&mut v[i].1, value)),
            Err(i) => {
                v.insert(i, (key, value));
                if v.len() > SMALL {
                    self.0 = Repr::Big(std::mem::take(v).into_iter().collect());
                }
                None
            }
        }
    }

    pub fn remove(&mut self, key: &Value) -> Option<Value> {
        match &mut self.0 {
            Repr::Small(v) => Self::find(v, key).ok().map(|i| v.remove(i).1),
            Repr::Big(m) => m.remove(key),
        }
    }

    pub fn retain(&mut self, mut f: impl FnMut(&Value, &mut Value) -> bool) {
        match &mut self.0 {
            Repr::Small(v) => v.retain_mut(|(k, x)| f(k, x)),
            Repr::Big(m) => m.retain(|k, x| f(k, x)),
        }
    }

    pub fn iter(&self) -> Iter<'_> {
        match &self.0 {
            Repr::Small(v) => Iter::Small(v.iter()),
            Repr::Big(m) => Iter::Big(m.iter()),
        }
    }

    pub fn iter_mut(&mut self) -> IterMut<'_> {
        match &mut self.0 {
            Repr::Small(v) => IterMut::Small(v.iter_mut()),
            Repr::Big(m) => IterMut::Big(m.iter_mut()),
        }
    }

    pub fn keys(&self) -> impl DoubleEndedIterator<Item = &Value> + ExactSizeIterator {
        self.iter().map(|(k, _)| k)
    }

    pub fn values(&self) -> impl DoubleEndedIterator<Item = &Value> + ExactSizeIterator {
        self.iter().map(|(_, v)| v)
    }
}

impl PartialEq for Map {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len() && self.iter().eq(other.iter())
    }
}

impl Eq for Map {}

impl Extend<(Value, Value)> for Map {
    fn extend<I: IntoIterator<Item = (Value, Value)>>(&mut self, iter: I) {
        for (k, v) in iter {
            self.insert(k, v);
        }
    }
}

impl FromIterator<(Value, Value)> for Map {
    fn from_iter<I: IntoIterator<Item = (Value, Value)>>(iter: I) -> Self {
        let mut m = Map::new();
        m.extend(iter);
        m
    }
}

impl<const N: usize> From<[(Value, Value); N]> for Map {
    fn from(entries: [(Value, Value); N]) -> Self {
        entries.into_iter().collect()
    }
}

pub enum Iter<'a> {
    Small(std::slice::Iter<'a, (Value, Value)>),
    Big(btree_map::Iter<'a, Value, Value>),
}

impl<'a> Iterator for Iter<'a> {
    type Item = (&'a Value, &'a Value);
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Iter::Small(i) => i.next().map(|(k, v)| (k, v)),
            Iter::Big(i) => i.next(),
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            Iter::Small(i) => i.size_hint(),
            Iter::Big(i) => i.size_hint(),
        }
    }
}

impl DoubleEndedIterator for Iter<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        match self {
            Iter::Small(i) => i.next_back().map(|(k, v)| (k, v)),
            Iter::Big(i) => i.next_back(),
        }
    }
}

impl ExactSizeIterator for Iter<'_> {}

pub enum IterMut<'a> {
    Small(std::slice::IterMut<'a, (Value, Value)>),
    Big(btree_map::IterMut<'a, Value, Value>),
}

impl<'a> Iterator for IterMut<'a> {
    type Item = (&'a Value, &'a mut Value);
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            IterMut::Small(i) => i.next().map(|(k, v)| (&*k, v)),
            IterMut::Big(i) => i.next(),
        }
    }
}

pub enum IntoIter {
    Small(std::vec::IntoIter<(Value, Value)>),
    Big(btree_map::IntoIter<Value, Value>),
}

impl Iterator for IntoIter {
    type Item = (Value, Value);
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            IntoIter::Small(i) => i.next(),
            IntoIter::Big(i) => i.next(),
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            IntoIter::Small(i) => i.size_hint(),
            IntoIter::Big(i) => i.size_hint(),
        }
    }
}

impl DoubleEndedIterator for IntoIter {
    fn next_back(&mut self) -> Option<Self::Item> {
        match self {
            IntoIter::Small(i) => i.next_back(),
            IntoIter::Big(i) => i.next_back(),
        }
    }
}

impl ExactSizeIterator for IntoIter {}

impl IntoIterator for Map {
    type Item = (Value, Value);
    type IntoIter = IntoIter;
    fn into_iter(self) -> IntoIter {
        match self.0 {
            Repr::Small(v) => IntoIter::Small(v.into_iter()),
            Repr::Big(m) => IntoIter::Big(m.into_iter()),
        }
    }
}

impl<'a> IntoIterator for &'a Map {
    type Item = (&'a Value, &'a Value);
    type IntoIter = Iter<'a>;
    fn into_iter(self) -> Iter<'a> {
        self.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Small and big maps answer as a BTreeMap of the same entries: lookups, order, replacing and removing.
    #[test]
    fn a_map_answers_as_a_btree_map() {
        let mut m = Map::new();
        let mut b = BTreeMap::new();
        for k in [5, 3, 9, 1, 3, 20, 7, 11, 2, 40, 33, 8, 13, 17, 21, 4, 6, 0, 99, 50] {
            let (key, v) = (Value::num(k as f64), Value::str(format!("v{k}")));
            assert_eq!(m.insert(key.clone(), v.clone()), b.insert(key, v));
            assert!(m.iter().eq(b.iter()) && m.len() == b.len());
        }
        for k in [3, 99, 1000] {
            let key = Value::num(k as f64);
            assert_eq!(m.get(&key), b.get(&key));
            assert_eq!(m.remove(&key), b.remove(&key));
        }
        assert!(m.iter().rev().eq(b.iter().rev()));
        m.retain(|k, _| k.as_number().is_some_and(|x| x < 10.0));
        b.retain(|k, _| k.as_number().is_some_and(|x| x < 10.0));
        assert!(m.iter().eq(b.iter()) && m.into_iter().eq(b.into_iter()));
    }
}
