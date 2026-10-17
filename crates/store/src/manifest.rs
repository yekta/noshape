//! Manifest: the set of named blobs that make up one document state.

use std::collections::BTreeMap;

use crate::{BlobHash, Result, StoreError};

/// Map from blob name (e.g. `"tab/<uuid>"`, `"document/meta"`) to the hash of
/// its content.
pub type Manifest = BTreeMap<String, BlobHash>;

const MAGIC: &[u8; 4] = b"NSM1";

pub(crate) fn encode(m: &Manifest) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + m.len() * 48);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&(m.len() as u32).to_le_bytes());
    for (name, hash) in m {
        out.extend_from_slice(&(name.len() as u32).to_le_bytes());
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(&hash.0);
    }
    out
}

pub(crate) fn decode(data: &[u8]) -> Result<Manifest> {
    let corrupt = || StoreError::Corrupt("bad manifest".into());
    let mut r = data;
    let mut take = |n: usize| -> Result<&[u8]> {
        if r.len() < n {
            return Err(corrupt());
        }
        let (a, b) = r.split_at(n);
        r = b;
        Ok(a)
    };
    if take(4)? != MAGIC {
        return Err(corrupt());
    }
    let count = u32::from_le_bytes(take(4)?.try_into().expect("4 bytes"));
    let mut m = Manifest::new();
    for _ in 0..count {
        let len = u32::from_le_bytes(take(4)?.try_into().expect("4 bytes")) as usize;
        let name = std::str::from_utf8(take(len)?)
            .map_err(|_| corrupt())?
            .to_string();
        let hash = BlobHash(take(32)?.try_into().expect("32 bytes"));
        m.insert(name, hash);
    }
    Ok(m)
}

/// How a name differs between two manifests.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ChangeKind {
    Added,
    Removed,
    Modified,
}

/// One entry of [`diff`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NameChange {
    pub name: String,
    pub kind: ChangeKind,
}

/// Names that differ from `old` to `new`, sorted by name.
pub fn diff(old: &Manifest, new: &Manifest) -> Vec<NameChange> {
    let mut out = Vec::new();
    for (name, h) in old {
        match new.get(name) {
            None => out.push(NameChange {
                name: name.clone(),
                kind: ChangeKind::Removed,
            }),
            Some(h2) if h2 != h => out.push(NameChange {
                name: name.clone(),
                kind: ChangeKind::Modified,
            }),
            _ => {}
        }
    }
    for name in new.keys() {
        if !old.contains_key(name) {
            out.push(NameChange {
                name: name.clone(),
                kind: ChangeKind::Added,
            });
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let mut m = Manifest::new();
        m.insert("tab/a".into(), BlobHash::of(b"a"));
        m.insert("document/meta".into(), BlobHash::of(b"m"));
        m.insert("ünïcode".into(), BlobHash::of(b"u"));
        assert_eq!(decode(&encode(&m)).unwrap(), m);
        assert_eq!(decode(&encode(&Manifest::new())).unwrap(), Manifest::new());
        assert!(decode(b"NSM1\x05\0\0\0").is_err());
        assert!(decode(b"xx").is_err());
    }

    #[test]
    fn diffs() {
        let mut a = Manifest::new();
        a.insert("keep".into(), BlobHash::of(b"1"));
        a.insert("mod".into(), BlobHash::of(b"1"));
        a.insert("gone".into(), BlobHash::of(b"1"));
        let mut b = a.clone();
        b.remove("gone");
        b.insert("mod".into(), BlobHash::of(b"2"));
        b.insert("new".into(), BlobHash::of(b"2"));
        let d = diff(&a, &b);
        assert_eq!(
            d,
            vec![
                NameChange {
                    name: "gone".into(),
                    kind: ChangeKind::Removed
                },
                NameChange {
                    name: "mod".into(),
                    kind: ChangeKind::Modified
                },
                NameChange {
                    name: "new".into(),
                    kind: ChangeKind::Added
                },
            ]
        );
        assert!(diff(&a, &a).is_empty());
    }
}
