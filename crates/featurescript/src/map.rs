//! FeatureScript's map: entries sorted by key, in place while two or fewer (a ValueWithUnits: the map's only
//! allocation is its value's `Arc`), then a short vector (a plane's three; a B-tree's node held eleven, a 512 × 512
//! Displacement map's values took 1.5 GB), a B-tree past that. Iteration, comparison and lookup as a `BTreeMap` (by
//! `Value`'s order).

use crate::value::Value;
use std::collections::BTreeMap;
use std::collections::btree_map;

/// Entries kept in a vector up to this many.
const SMALL: usize = 16;
/// Entries kept in place up to this many (a ValueWithUnits' two: a million points' lengths are each one allocation
/// of their size).
const INLINE: usize = 2;
const EMPTY: (Value, Value) = (Value::Undefined, Value::Undefined);

#[derive(Clone, Debug)]
pub struct Map(Repr);

#[derive(Clone, Debug)]
enum Repr {
    Small(Small),
    Big(BTreeMap<Value, Value>),
}

/// A small map's sorted entries: the first `INLINE` in place (the slots past the count hold undefined), more in a
/// vector.
#[derive(Clone, Debug)]
enum Small {
    Inline(u8, [(Value, Value); INLINE]),
    Heap(Vec<(Value, Value)>),
}

impl Small {
    fn as_slice(&self) -> &[(Value, Value)] {
        match self {
            Small::Inline(n, a) => &a[..*n as usize],
            Small::Heap(v) => v,
        }
    }

    fn as_mut_slice(&mut self) -> &mut [(Value, Value)] {
        match self {
            Small::Inline(n, a) => &mut a[..*n as usize],
            Small::Heap(v) => v,
        }
    }

    fn insert(&mut self, i: usize, e: (Value, Value)) {
        match self {
            Small::Inline(n, a) if (*n as usize) < INLINE => {
                let end = *n as usize;
                a[end] = e;
                a[i..=end].rotate_right(1);
                *n += 1;
            }
            Small::Inline(..) => {
                let mut v = self.take_all();
                v.insert(i, e);
                *self = Small::Heap(v);
            }
            Small::Heap(v) => v.insert(i, e),
        }
    }

    fn remove(&mut self, i: usize) -> (Value, Value) {
        match self {
            Small::Inline(n, a) => {
                let end = *n as usize;
                a[i..end].rotate_left(1);
                *n -= 1;
                std::mem::take(&mut a[end - 1])
            }
            Small::Heap(v) => v.remove(i),
        }
    }

    fn retain(&mut self, mut f: impl FnMut(&mut (Value, Value)) -> bool) {
        match self {
            Small::Inline(n, a) => {
                let mut kept = 0;
                for i in 0..*n as usize {
                    if f(&mut a[i]) {
                        a.swap(kept, i);
                        kept += 1;
                    }
                }
                for x in &mut a[kept..] {
                    *x = EMPTY;
                }
                *n = kept as u8;
            }
            Small::Heap(v) => v.retain_mut(f),
        }
    }

    fn take_all(&mut self) -> Vec<(Value, Value)> {
        match std::mem::replace(self, Small::Inline(0, [EMPTY; INLINE])) {
            Small::Inline(n, a) => a.into_iter().take(n as usize).collect(),
            Small::Heap(v) => v,
        }
    }
}

impl Default for Map {
    fn default() -> Self {
        Map::new()
    }
}

impl Map {
    pub const fn new() -> Self {
        Map(Repr::Small(Small::Inline(0, [EMPTY; INLINE])))
    }

    pub fn len(&self) -> usize {
        match &self.0 {
            Repr::Small(v) => v.as_slice().len(),
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
            Repr::Small(v) => {
                let v = v.as_slice();
                // (A key by address first: a ValueWithUnits' "value", a plane's "origin".)
                if let Some((_, x)) = v.iter().find(|(k, _)| k.same_str(key)) {
                    return Some(x);
                }
                Self::find(v, key).ok().map(|i| &v[i].1)
            }
            Repr::Big(m) => m.get(key),
        }
    }

    /// `m[key]` for an untagged string key, without making the key's value.
    pub fn get_str(&self, key: &str) -> Option<&Value> {
        match &self.0 {
            Repr::Small(v) => {
                let v = v.as_slice();
                // (Where the key sorts in Value's order: untagged values by standard type, strings by their text.)
                let at = |k: &Value| match k {
                    Value::Str(a, t) if t.is_none() => (**a).cmp(key),
                    k if !k.tag().is_none() => std::cmp::Ordering::Greater,
                    k => k.std_type().cmp(&crate::value::StdType::String),
                };
                v.binary_search_by(|(k, _)| at(k)).ok().map(|i| &v[i].1)
            }
            Repr::Big(m) => m.get(&Value::str(key)),
        }
    }

    pub fn get_mut(&mut self, key: &Value) -> Option<&mut Value> {
        match &mut self.0 {
            Repr::Small(v) => {
                let v = v.as_mut_slice();
                let i = v.iter().position(|(k, _)| k.same_str(key)).map_or_else(|| Self::find(v, key), Ok);
                i.ok().map(|i| &mut v[i].1)
            }
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
        match Self::find(v.as_slice(), &key) {
            Ok(i) => Some(std::mem::replace(&mut v.as_mut_slice()[i].1, value)),
            Err(i) => {
                v.insert(i, (key, value));
                if v.as_slice().len() > SMALL {
                    self.0 = Repr::Big(v.take_all().into_iter().collect());
                }
                None
            }
        }
    }

    pub fn remove(&mut self, key: &Value) -> Option<Value> {
        match &mut self.0 {
            Repr::Small(v) => Self::find(v.as_slice(), key).ok().map(|i| v.remove(i).1),
            Repr::Big(m) => m.remove(key),
        }
    }

    pub fn retain(&mut self, mut f: impl FnMut(&Value, &mut Value) -> bool) {
        match &mut self.0 {
            Repr::Small(v) => v.retain(|(k, x)| f(k, x)),
            Repr::Big(m) => m.retain(|k, x| f(k, x)),
        }
    }

    pub fn iter(&self) -> Iter<'_> {
        match &self.0 {
            Repr::Small(v) => Iter::Small(v.as_slice().iter()),
            Repr::Big(m) => Iter::Big(m.iter()),
        }
    }

    pub fn iter_mut(&mut self) -> IterMut<'_> {
        match &mut self.0 {
            Repr::Small(v) => IterMut::Small(v.as_mut_slice().iter_mut()),
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
    Inline(std::iter::Take<std::array::IntoIter<(Value, Value), INLINE>>),
    Small(std::vec::IntoIter<(Value, Value)>),
    Big(btree_map::IntoIter<Value, Value>),
}

impl Iterator for IntoIter {
    type Item = (Value, Value);
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            IntoIter::Inline(i) => i.next(),
            IntoIter::Small(i) => i.next(),
            IntoIter::Big(i) => i.next(),
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            IntoIter::Inline(i) => i.size_hint(),
            IntoIter::Small(i) => i.size_hint(),
            IntoIter::Big(i) => i.size_hint(),
        }
    }
}

impl DoubleEndedIterator for IntoIter {
    fn next_back(&mut self) -> Option<Self::Item> {
        match self {
            IntoIter::Inline(i) => i.next_back(),
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
            Repr::Small(Small::Inline(n, a)) => IntoIter::Inline(a.into_iter().take(n as usize)),
            Repr::Small(Small::Heap(v)) => IntoIter::Small(v.into_iter()),
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

    /// A string key found by its text as by its value, among keys of every kind and size of map.
    #[test]
    fn a_string_key_is_found_by_its_text() {
        let keys = [Value::num(1.0), Value::str("b"), Value::str("a"), Value::bool(true), Value::array(vec![]), Value::str("c").with_tag(crate::value::Tag(1)), Value::str("ab"), Value::Undefined];
        for n in 1..=keys.len() {
            let mut m = Map::new();
            for (i, k) in keys.iter().take(n).enumerate() {
                m.insert(k.clone(), Value::num(i as f64));
            }
            for s in ["a", "b", "c", "ab", "zz", ""] {
                assert_eq!(m.get_str(s), m.get(&Value::str(s)), "{n} keys, {s:?}");
            }
        }
    }

    /// A map of two or fewer entries (held in place) and one that grew past them, every operation on each size.
    #[test]
    fn a_small_map_answers_as_a_btree_map() {
        for keys in [&[2][..], &[2, 1], &[3, 1, 2], &[3, 1, 2, 0], &[4, 0, 2, 1, 3]] {
            for gone in keys.iter().copied().chain([9]) {
                let (mut m, mut b) = (Map::new(), BTreeMap::new());
                for k in keys {
                    let (key, v) = (Value::num(*k as f64), Value::str(format!("v{k}")));
                    assert_eq!(m.insert(key.clone(), v.clone()), b.insert(key, v));
                }
                let key = Value::num(gone as f64);
                assert_eq!(m.remove(&key), b.remove(&key));
                assert!(m.iter().eq(b.iter()) && m.iter().rev().eq(b.iter().rev()) && m.len() == b.len());
                m.retain(|k, _| k.as_number().is_some_and(|x| x != 1.0));
                b.retain(|k, _| k.as_number().is_some_and(|x| x != 1.0));
                assert_eq!(m.insert(Value::num(7.0), Value::Undefined), b.insert(Value::num(7.0), Value::Undefined));
                assert!(m.clone().into_iter().rev().eq(b.clone().into_iter().rev()) && m.into_iter().eq(b.into_iter()));
            }
        }
    }
}
