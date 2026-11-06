//! FeatureScript's map: entries sorted by key, as a short vector while small (a plane's three: one allocation of
//! their size, where a B-tree's node held eleven; a 512 × 512 Displacement map's values took 1.5 GB), a B-tree past
//! that. A map of exactly "unit" and "value" (every ValueWithUnits) holds only its two values: a length is its `Arc`'s
//! one allocation, read without a lookup. Iteration, comparison and lookup as a `BTreeMap` (by `Value`'s order).

use crate::value::{Tag, UNIT_KEY, VALUE_KEY, Value};
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
    /// `{ "unit" : u, "value" : v }`: [u, v].
    UnitValue([Value; 2]),
}

/// A key's place in a [`Repr::UnitValue`]: "unit" 0, "value" 1.
fn unit_value_slot(key: &Value) -> Option<usize> {
    if key.same_str(&UNIT_KEY) {
        return Some(0);
    }
    if key.same_str(&VALUE_KEY) {
        return Some(1);
    }
    match key {
        Value::Str(s, Tag::NONE) if &**s == "unit" => Some(0),
        Value::Str(s, Tag::NONE) if &**s == "value" => Some(1),
        _ => None,
    }
}

fn unit_value_keys() -> [&'static Value; 2] {
    [&UNIT_KEY, &VALUE_KEY]
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
            Repr::UnitValue(_) => 2,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// `{ "unit" : unit, "value" : value }`.
    pub fn with_unit_value(unit: Value, value: Value) -> Map {
        Map(Repr::UnitValue([unit, value]))
    }

    /// A ValueWithUnits' unit and value, when this map is exactly those two.
    pub fn unit_value(&self) -> Option<(&Value, &Value)> {
        match &self.0 {
            Repr::UnitValue([u, v]) => Some((u, v)),
            _ => None,
        }
    }

    fn find(v: &[(Value, Value)], key: &Value) -> Result<usize, usize> {
        v.binary_search_by(|(k, _)| k.cmp(key))
    }

    /// The entries as a vector (a unit-and-value map's keys made).
    fn small(&mut self) -> &mut Vec<(Value, Value)> {
        if let Repr::UnitValue([u, v]) = &mut self.0 {
            let (u, v) = (std::mem::take(u), std::mem::take(v));
            self.0 = Repr::Small(vec![(UNIT_KEY.clone(), u), (VALUE_KEY.clone(), v)]);
        }
        match &mut self.0 {
            Repr::Small(v) => v,
            // (Not reached: a big map is no vector.)
            Repr::Big(_) | Repr::UnitValue(_) => unreachable_small(),
        }
    }

    pub fn get(&self, key: &Value) -> Option<&Value> {
        match &self.0 {
            Repr::Small(v) => {
                // (A key by address first: a plane's "origin".)
                if let Some((_, x)) = v.iter().find(|(k, _)| k.same_str(key)) {
                    return Some(x);
                }
                Self::find(v, key).ok().map(|i| &v[i].1)
            }
            Repr::Big(m) => m.get(key),
            Repr::UnitValue(a) => unit_value_slot(key).map(|i| &a[i]),
        }
    }

    /// `m[key]` for an untagged string key, without making the key's value.
    pub fn get_str(&self, key: &str) -> Option<&Value> {
        match &self.0 {
            Repr::Small(v) => {
                // (Where the key sorts in Value's order: untagged values by standard type, strings by their text.)
                let at = |k: &Value| match k {
                    Value::Str(a, t) if t.is_none() => (**a).cmp(key),
                    k if !k.tag().is_none() => std::cmp::Ordering::Greater,
                    k => k.std_type().cmp(&crate::value::StdType::String),
                };
                v.binary_search_by(|(k, _)| at(k)).ok().map(|i| &v[i].1)
            }
            Repr::Big(m) => m.get(&Value::str(key)),
            Repr::UnitValue(a) => match key {
                "unit" => Some(&a[0]),
                "value" => Some(&a[1]),
                _ => None,
            },
        }
    }

    pub fn get_mut(&mut self, key: &Value) -> Option<&mut Value> {
        match &mut self.0 {
            Repr::Small(v) => {
                let i = v.iter().position(|(k, _)| k.same_str(key)).map_or_else(|| Self::find(v, key), Ok);
                i.ok().map(|i| &mut v[i].1)
            }
            Repr::Big(m) => m.get_mut(key),
            Repr::UnitValue(a) => unit_value_slot(key).map(|i| &mut a[i]),
        }
    }

    pub fn contains_key(&self, key: &Value) -> bool {
        self.get(key).is_some()
    }

    pub fn insert(&mut self, key: Value, value: Value) -> Option<Value> {
        match &mut self.0 {
            Repr::Big(m) => return m.insert(key, value),
            Repr::UnitValue(a) => {
                if let Some(i) = unit_value_slot(&key) {
                    return Some(std::mem::replace(&mut a[i], value));
                }
            }
            Repr::Small(_) => {}
        }
        let v = self.small();
        match Self::find(v, &key) {
            Ok(i) => Some(std::mem::replace(&mut v[i].1, value)),
            Err(i) => {
                v.insert(i, (key, value));
                if v.len() > SMALL {
                    self.0 = Repr::Big(std::mem::take(v).into_iter().collect());
                } else if v.len() == 2 && unit_value_slot(&v[0].0) == Some(0) && unit_value_slot(&v[1].0) == Some(1) {
                    let (u, x) = (std::mem::take(&mut v[0].1), std::mem::take(&mut v[1].1));
                    self.0 = Repr::UnitValue([u, x]);
                }
                None
            }
        }
    }

    pub fn remove(&mut self, key: &Value) -> Option<Value> {
        match &mut self.0 {
            Repr::Big(m) => m.remove(key),
            Repr::UnitValue(_) if unit_value_slot(key).is_none() => None,
            _ => {
                let v = self.small();
                Self::find(v, key).ok().map(|i| v.remove(i).1)
            }
        }
    }

    pub fn retain(&mut self, mut f: impl FnMut(&Value, &mut Value) -> bool) {
        match &mut self.0 {
            Repr::Big(m) => m.retain(|k, x| f(k, x)),
            _ => self.small().retain_mut(|(k, x)| f(k, x)),
        }
    }

    pub fn iter(&self) -> Iter<'_> {
        match &self.0 {
            Repr::Small(v) => Iter::Small(v.iter()),
            Repr::Big(m) => Iter::Big(m.iter()),
            Repr::UnitValue(a) => Iter::UnitValue(unit_value_keys().into_iter().zip(a.iter())),
        }
    }

    pub fn iter_mut(&mut self) -> IterMut<'_> {
        match &mut self.0 {
            Repr::Small(v) => IterMut::Small(v.iter_mut()),
            Repr::Big(m) => IterMut::Big(m.iter_mut()),
            Repr::UnitValue(a) => IterMut::UnitValue(unit_value_keys().into_iter().zip(a.iter_mut())),
        }
    }

    pub fn keys(&self) -> impl DoubleEndedIterator<Item = &Value> + ExactSizeIterator {
        self.iter().map(|(k, _)| k)
    }

    pub fn values(&self) -> impl DoubleEndedIterator<Item = &Value> + ExactSizeIterator {
        self.iter().map(|(_, v)| v)
    }
}

#[cold]
fn unreachable_small() -> &'static mut Vec<(Value, Value)> {
    // (Map::small is only called on a vector or a unit-and-value map; a leaked empty vector keeps it panic-free.)
    Box::leak(Box::default())
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

type UnitValueIter<'a> = std::iter::Zip<std::array::IntoIter<&'static Value, 2>, std::slice::Iter<'a, Value>>;

pub enum Iter<'a> {
    Small(std::slice::Iter<'a, (Value, Value)>),
    Big(btree_map::Iter<'a, Value, Value>),
    UnitValue(UnitValueIter<'a>),
}

impl<'a> Iterator for Iter<'a> {
    type Item = (&'a Value, &'a Value);
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Iter::Small(i) => i.next().map(|(k, v)| (k, v)),
            Iter::Big(i) => i.next(),
            Iter::UnitValue(i) => i.next(),
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            Iter::Small(i) => i.size_hint(),
            Iter::Big(i) => i.size_hint(),
            Iter::UnitValue(i) => i.size_hint(),
        }
    }
}

impl DoubleEndedIterator for Iter<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        match self {
            Iter::Small(i) => i.next_back().map(|(k, v)| (k, v)),
            Iter::Big(i) => i.next_back(),
            Iter::UnitValue(i) => i.next_back(),
        }
    }
}

impl ExactSizeIterator for Iter<'_> {}

pub enum IterMut<'a> {
    Small(std::slice::IterMut<'a, (Value, Value)>),
    Big(btree_map::IterMut<'a, Value, Value>),
    UnitValue(std::iter::Zip<std::array::IntoIter<&'static Value, 2>, std::slice::IterMut<'a, Value>>),
}

impl<'a> Iterator for IterMut<'a> {
    type Item = (&'a Value, &'a mut Value);
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            IterMut::Small(i) => i.next().map(|(k, v)| (&*k, v)),
            IterMut::Big(i) => i.next(),
            IterMut::UnitValue(i) => i.next(),
        }
    }
}

pub enum IntoIter {
    Small(std::vec::IntoIter<(Value, Value)>),
    Big(btree_map::IntoIter<Value, Value>),
    UnitValue(std::iter::Zip<std::array::IntoIter<Value, 2>, std::array::IntoIter<Value, 2>>),
}

impl Iterator for IntoIter {
    type Item = (Value, Value);
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            IntoIter::Small(i) => i.next(),
            IntoIter::Big(i) => i.next(),
            IntoIter::UnitValue(i) => i.next(),
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            IntoIter::Small(i) => i.size_hint(),
            IntoIter::Big(i) => i.size_hint(),
            IntoIter::UnitValue(i) => i.size_hint(),
        }
    }
}

impl DoubleEndedIterator for IntoIter {
    fn next_back(&mut self) -> Option<Self::Item> {
        match self {
            IntoIter::Small(i) => i.next_back(),
            IntoIter::Big(i) => i.next_back(),
            IntoIter::UnitValue(i) => i.next_back(),
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
            Repr::UnitValue(a) => IntoIter::UnitValue([UNIT_KEY.clone(), VALUE_KEY.clone()].into_iter().zip(a)),
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

    /// A unit-and-value map (keys "unit" and "value", however made) and the maps it turns into, every operation on
    /// each: as a BTreeMap.
    #[test]
    fn a_unit_and_value_map_answers_as_a_btree_map() {
        let key = |k: &usize| match k {
            1 => Value::key("unit"),
            2 => Value::str("value"),
            3 => Value::str("unit").with_tag(crate::value::Tag(1)),
            k => Value::num(*k as f64),
        };
        for keys in [&[2][..], &[2, 1], &[1, 2], &[3, 1, 2], &[1, 2, 0], &[4, 0, 2, 1, 3]] {
            for gone in keys.iter().copied().chain([9]) {
                let (mut m, mut b) = (Map::new(), BTreeMap::new());
                for k in keys {
                    let (key, v) = (key(k), Value::str(format!("v{k}")));
                    assert_eq!(m.insert(key.clone(), v.clone()), b.insert(key, v));
                }
                assert!(keys.iter().chain(&[9]).all(|k| m.get(&key(k)) == b.get(&key(k))));
                assert!(["unit", "value", "x"].iter().all(|s| m.get_str(s) == m.get(&Value::str(*s))));
                assert_eq!(m.unit_value().is_some(), m.len() == 2 && keys.contains(&1) && keys.contains(&2));
                for k in keys {
                    if let (Some(x), Some(y)) = (m.get_mut(&key(k)), b.get_mut(&key(k))) {
                        (*x, *y) = (Value::num(0.5), Value::num(0.5));
                    }
                }
                let gone = key(&gone);
                assert_eq!(m.remove(&gone), b.remove(&gone));
                assert!(m.iter().eq(b.iter()) && m.iter().rev().eq(b.iter().rev()) && m.len() == b.len());
                assert!(m.clone().iter_mut().map(|(k, v)| (k.clone(), v.clone())).eq(b.clone().into_iter()));
                m.retain(|k, _| *k != Value::str("value"));
                b.retain(|k, _| *k != Value::str("value"));
                assert_eq!(m.insert(Value::num(7.0), Value::Undefined), b.insert(Value::num(7.0), Value::Undefined));
                assert!(m.clone().into_iter().rev().eq(b.clone().into_iter().rev()) && m.into_iter().eq(b.into_iter()));
            }
        }
    }
}
