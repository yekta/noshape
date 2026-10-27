//! `@unpackQuery` (std query.fs `qCompressed(version, string, root)`): the
//! packed queries Part Studio "Show code" writes.
//!
//! Reverse-engineered from live Show code output (dom/fs-packed-query.txt):
//!
//! * `&<hex length>$<base64>`: the base64 is zlib of the plain form, whose
//!   length is the hex number.
//! * plain: `%` then one value. Values: `M<n>` map of n key/value pairs,
//!   `A<n>` array, `S<parts>$<text>` string, `D<number>`, `F` / `T`
//!   booleans, `B<len>$<name>` a type name followed by the value it tags,
//!   `C<i>` a type name seen before, followed by its value, `R<i>` a value
//!   seen before. A string's parts are `.`-joined pieces: `<len>` takes the
//!   next `len` characters of the text, `-<i>` repeats a piece seen before
//!   (`S11.6$FbZU2oH5hP9eBC6_0wireOp` is "FbZU2oH5hP9eBC6_0.wireOp").
//!   All counts and indices are hex.
//! * Two tables: names (type names and string pieces, in order of first
//!   appearance) for `C` and `-<i>`; values (every value as it completes,
//!   maps and arrays included, a tagged value once) for `R`.
//! * `root` (an Id) is prepended to every `Id` value: Show code passes
//!   `newId()`; `makeId("X")` gives operation ids `[X, Top.planeOp]`.

use super::{std_enum, std_tag};
use noshape_featurescript::interp::{Interp, NativeFn, R};
use noshape_featurescript::value::{Map, Value};
use std::collections::HashMap;
use std::sync::LazyLock;

pub const NATIVES: &[(&str, NativeFn)] = &[("unpackQuery", unpack_query)];

fn unpack_query(i: &mut Interp, a: Vec<Value>) -> R<Value> {
    let Some(text) = a.get(1).and_then(|v| v.as_str()) else { return Err(i.error("qCompressed: the query should be a string")) };
    let root: Vec<Value> = a.get(2).and_then(|v| v.as_array().map(|x| x.to_vec())).unwrap_or_default();
    match unpack(&text) {
        Ok(p) => Ok(to_value(&p, &root)),
        Err(e) => Err(i.error(format!("qCompressed: {e}"))),
    }
}

/// A decoded packed value (type names kept as written).
#[derive(Clone, Debug, PartialEq)]
pub enum Packed {
    Map(Vec<(Packed, Packed)>),
    Array(Vec<Packed>),
    Str(String),
    Num(f64),
    Bool(bool),
    Tagged(String, Box<Packed>),
}

/// Decode a packed query string.
pub fn unpack(s: &str) -> Result<Packed, String> {
    let plain;
    let s = if let Some(rest) = s.strip_prefix('&') {
        let (len, data) = rest.split_once('$').ok_or("bad compressed header")?;
        let len = usize::from_str_radix(len, 16).map_err(|_| "bad compressed length")?;
        let bytes = base64(data).ok_or("bad base64")?;
        let out = miniz_oxide::inflate::decompress_to_vec_zlib(&bytes).map_err(|_| "bad zlib data")?;
        plain = String::from_utf8(out).map_err(|_| "bad text")?;
        if plain.len() != len {
            return Err("length mismatch".into());
        }
        plain.as_str()
    } else {
        s
    };
    let body = s.strip_prefix('%').ok_or("a packed query starts with % or &")?;
    let mut d = Decoder { s: body.as_bytes(), pos: 0, names: Vec::new(), values: Vec::new() };
    let v = d.value()?;
    if d.pos != d.s.len() {
        return Err(format!("trailing data at {}", d.pos));
    }
    Ok(v)
}

struct Decoder<'a> {
    s: &'a [u8],
    pos: usize,
    names: Vec<String>,
    values: Vec<Packed>,
}

impl Decoder<'_> {
    fn peek(&self) -> Option<u8> {
        self.s.get(self.pos).copied()
    }

    fn hex(&mut self) -> Result<usize, String> {
        let start = self.pos;
        while self.peek().is_some_and(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)) {
            self.pos += 1;
        }
        let t = std::str::from_utf8(&self.s[start..self.pos]).unwrap_or_default();
        usize::from_str_radix(t, 16).map_err(|_| format!("expected a hex number at {start}"))
    }

    fn take(&mut self, n: usize) -> Result<String, String> {
        let end = self.pos + n;
        let t = self.s.get(self.pos..end).ok_or("string runs past the end")?;
        self.pos = end;
        String::from_utf8(t.to_vec()).map_err(|_| "bad text".into())
    }

    fn expect(&mut self, c: u8) -> Result<(), String> {
        if self.peek() != Some(c) {
            return Err(format!("expected '{}' at {}", c as char, self.pos));
        }
        self.pos += 1;
        Ok(())
    }

    fn name(&mut self, i: usize) -> Result<String, String> {
        self.names.get(i).cloned().ok_or_else(|| format!("unknown name {i:x}"))
    }

    fn value(&mut self) -> Result<Packed, String> {
        let c = self.peek().ok_or("unexpected end")?;
        self.pos += 1;
        let v = match c {
            b'B' => {
                let n = self.hex()?;
                self.expect(b'$')?;
                let t = self.take(n)?;
                self.names.push(t.clone());
                let inner = self.untracked()?;
                Packed::Tagged(t, Box::new(inner))
            }
            b'C' => {
                let i = self.hex()?;
                let t = self.name(i)?;
                let inner = self.untracked()?;
                Packed::Tagged(t, Box::new(inner))
            }
            b'R' => {
                let i = self.hex()?;
                return self.values.get(i).cloned().ok_or_else(|| format!("unknown value {i:x}"));
            }
            _ => {
                self.pos -= 1;
                self.untracked()?
            }
        };
        self.values.push(v.clone());
        Ok(v)
    }

    /// A value's body (not yet in the value table).
    fn untracked(&mut self) -> Result<Packed, String> {
        let c = self.peek().ok_or("unexpected end")?;
        self.pos += 1;
        Ok(match c {
            b'M' => {
                let n = self.hex()?;
                let mut m = Vec::with_capacity(n);
                for _ in 0..n {
                    let k = self.value()?;
                    let v = self.value()?;
                    m.push((k, v));
                }
                Packed::Map(m)
            }
            b'A' => {
                let n = self.hex()?;
                let mut a = Vec::with_capacity(n);
                for _ in 0..n {
                    a.push(self.value()?);
                }
                Packed::Array(a)
            }
            b'S' => {
                // Pieces: lengths of new pieces or `-<index>` of known ones.
                let mut pieces: Vec<Result<String, usize>> = Vec::new();
                loop {
                    if self.peek() == Some(b'-') {
                        self.pos += 1;
                        let i = self.hex()?;
                        pieces.push(Ok(self.name(i)?));
                    } else {
                        pieces.push(Err(self.hex()?));
                    }
                    match self.peek() {
                        Some(b'.') => self.pos += 1,
                        _ => break,
                    }
                }
                self.expect(b'$')?;
                let mut parts = Vec::new();
                for p in pieces {
                    parts.push(match p {
                        Ok(s) => s,
                        Err(n) => {
                            let s = self.take(n)?;
                            self.names.push(s.clone());
                            s
                        }
                    });
                }
                Packed::Str(parts.join("."))
            }
            b'D' => {
                let start = self.pos;
                while self.peek().is_some_and(|c| c.is_ascii_digit() || matches!(c, b'.' | b'-' | b'+' | b'e')) {
                    self.pos += 1;
                }
                let t = std::str::from_utf8(&self.s[start..self.pos]).unwrap_or_default();
                Packed::Num(t.parse().map_err(|_| format!("bad number at {start}"))?)
            }
            b'F' => Packed::Bool(false),
            b'T' => Packed::Bool(true),
            other => return Err(format!("unknown token '{}' at {}", other as char, self.pos - 1)),
        })
    }
}

fn base64(s: &str) -> Option<Vec<u8>> {
    let val = |c: u8| -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => return None,
        } as u32)
    };
    let mut out = Vec::new();
    let (mut acc, mut bits) = (0u32, 0);
    for c in s.bytes().filter(|c| !c.is_ascii_whitespace() && *c != b'=') {
        acc = (acc << 6) | val(c)?;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

/// Module defining each standard library type or enum (by name).
static TYPE_MODULES: LazyLock<HashMap<String, &'static str>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    for (file, src) in noshape_featurescript::STD_SOURCES {
        for line in src.lines() {
            let t = line.trim_start();
            let rest = t.strip_prefix("export enum ").or_else(|| t.strip_prefix("export type "));
            if let Some(name) = rest.and_then(|r| r.split(|c: char| !c.is_alphanumeric() && c != '_').next()) {
                m.entry(name.to_string()).or_insert(*file);
            }
        }
    }
    m
});

fn to_value(p: &Packed, root: &[Value]) -> Value {
    match p {
        Packed::Map(entries) => {
            let mut m = Map::new();
            for (k, v) in entries {
                m.insert(to_value(k, root), to_value(v, root));
            }
            Value::map(m)
        }
        Packed::Array(a) => Value::array(a.iter().map(|x| to_value(x, root)).collect()),
        Packed::Str(s) => Value::str(s.as_str()),
        Packed::Num(n) => Value::num(*n),
        Packed::Bool(b) => Value::bool(*b),
        Packed::Tagged(t, inner) => {
            let module = TYPE_MODULES.get(t.as_str()).copied();
            if let (Packed::Str(member), Some(module)) = (&**inner, module)
                && let Some(e) = std_enum(module, t, member)
            {
                return e;
            }
            let mut v = to_value(inner, root);
            if t == "Id"
                && let Value::Array(a, _) = &v
            {
                v = Value::array(root.iter().cloned().chain(a.iter().cloned()).collect());
            }
            match module {
                Some(module) => v.with_tag(std_tag(module, t)),
                None => v,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tagged_str(t: &str, x: &str) -> Packed {
        Packed::Tagged(t.into(), Box::new(Packed::Str(x.into())))
    }

    fn s(x: &str) -> Packed {
        Packed::Str(x.into())
    }

    fn map(entries: Vec<(&str, Packed)>) -> Packed {
        Packed::Map(entries.into_iter().map(|(k, v)| (s(k), v)).collect())
    }

    fn query(entries: Vec<(&str, Packed)>) -> Packed {
        Packed::Tagged("Query".into(), Box::new(map(entries)))
    }

    fn id(x: &str) -> Packed {
        Packed::Tagged("Id".into(), Box::new(Packed::Array(vec![s(x)])))
    }

    fn dummy(entity: &str, op: &str) -> Packed {
        query(vec![("entityType", tagged_str("EntityType", entity)), ("historyType", s("CREATION")), ("operationId", id(op)), ("queryType", s("DUMMY"))])
    }

    #[test]
    fn default_plane() {
        let p = unpack("%B5$QueryM4Sa$entityTypeBa$EntityTypeS4$FACESb$historyTypeS8$CREATIONSb$operationIdB2$IdA1S3.7$TopplaneOpS9$queryTypeS5$DUMMY").unwrap();
        assert_eq!(p, dummy("FACE", "Top.planeOp"));
    }

    #[test]
    fn union_of_plane_and_origin() {
        let p = unpack("%B5$QueryM2S9$queryTypeB9$QueryTypeS5$UNIONSa$subqueriesA2C0M4Sa$entityTypeBa$EntityTypeS4$FACESb$historyTypeS8$CREATIONSb$operationIdB2$IdA1S3.7$TopplaneOpR0S5$DUMMYC0M4R3C6S6$VERTEXR5R6R7CbA1S6.7$OriginpointOpR0Ra").unwrap();
        let want = query(vec![("queryType", tagged_str("QueryType", "UNION")), ("subqueries", Packed::Array(vec![dummy("FACE", "Top.planeOp"), dummy("VERTEX", "Origin.pointOp")]))]);
        assert_eq!(p, want);
    }

    #[test]
    fn region_query_compressed() {
        // Extrude 1's sketch region (live, zlib form) and its decoded text.
        let packed = "&1f3$eJydkFFPwjAUhf/M9VFCC2PwuHUFGoXhbX3AF7KtjTQGNrui2b+3DEM0mpj40vSec79ze3uTRvBwMq5bRZJQ0LYtDqV9PhXe1ses8EVCVvSno7rGyCmofJPf54ttuJqjt96aNhknlA1DWgnaOPtm9NzVh14pLk3dGU4L4NdCjoFnCx6QvW197brPeIY8USJfB6NujOsnC51SEDohkpDBBObl0yOtl9F+MzMpm+yG79aZvJEzeD1v1QdpkHdcseWOr5VQW2mgfTG+2l8eILSk4IYY4QRjnOKMVSH8Vg9isIfG2aPHSsYgVhsUa5WRy3o47s8rhCVWqNGcs8gXmdCgkxGS6G+O/pMb/cphxOLwsfOE8e/eB3T+mcU=";
        let plain = "%B5$QueryM5S12$disambiguationDataA1M2S12$disambiguationTypeS8$TOPOLOGYS8$entitiesA4A2C0M5Sb$derivedFromC0M5Sa$entityTypeBa$EntityTypeS4$EDGESb$historyTypeS8$CREATIONSb$operationIdB2$IdA1S11.6$FbZU2oH5hP9eBC6_0wireOpS9$queryTypeSd$SKETCH_ENTITYSe$sketchEntityIdS2$r0R5R6R7R8R9CcA1S-d.7$imprintRcS7$IMPRINTD1A2C0M5R4C0M5R5R6R7R8R9RbRcRdReS2$r1R5R6R7R8R9R12RcR13R15A2C0M5R4C0M5R5R6R7R8R9RbRcRdReS2$r2R5R6R7R8R9R12RcR13R15A2C0M5R4C0M5R5R6R7R8R9RbRcRdReS2$r3R5R6R7R8R9R12RcR13R15R5C7S4$FACER7R8R9R12RcR13";
        let a = unpack(packed).unwrap();
        assert_eq!(a, unpack(plain).unwrap());
        let edge = |r: &str| {
            let from = query(vec![("entityType", tagged_str("EntityType", "EDGE")), ("historyType", s("CREATION")), ("operationId", id("FbZU2oH5hP9eBC6_0.wireOp")), ("queryType", s("SKETCH_ENTITY")), ("sketchEntityId", s(r))]);
            let imprint = query(vec![("derivedFrom", from), ("entityType", tagged_str("EntityType", "EDGE")), ("historyType", s("CREATION")), ("operationId", id("FbZU2oH5hP9eBC6_0.imprint")), ("queryType", s("IMPRINT"))]);
            Packed::Array(vec![imprint, Packed::Num(1.0)])
        };
        let want = query(vec![
            ("disambiguationData", Packed::Array(vec![map(vec![("disambiguationType", s("TOPOLOGY")), ("entities", Packed::Array(vec![edge("r0"), edge("r1"), edge("r2"), edge("r3")]))])])),
            ("entityType", tagged_str("EntityType", "FACE")),
            ("historyType", s("CREATION")),
            ("operationId", id("FbZU2oH5hP9eBC6_0.imprint")),
            ("queryType", s("IMPRINT")),
        ]);
        assert_eq!(a, want);
    }

    #[test]
    fn cap_edge_with_boolean() {
        let p = unpack("%B5$QueryM6S12$disambiguationDataA1M2S12$disambiguationTypeS13$ORIGINAL_DEPENDENCYS9$originalsA1C0M5Sa$entityTypeBa$EntityTypeS4$EDGESb$historyTypeS8$CREATIONSb$operationIdB2$IdA1S11.6$FbZU2oH5hP9eBC6_0wireOpS9$queryTypeSd$SKETCH_ENTITYSe$sketchEntityIdS2$r2R4R5R6R7S7$isStartFR8CbA1S11.9$FyUE18XOUbMicpL_0opExtrudeRbS8$CAP_EDGE").unwrap();
        let Packed::Tagged(_, m) = p else { panic!() };
        let Packed::Map(m) = *m else { panic!() };
        assert_eq!(m.len(), 6);
        assert!(m.contains(&(s("isStart"), Packed::Bool(false))));
        assert!(m.contains(&(s("operationId"), id("FyUE18XOUbMicpL_0.opExtrude"))));
        assert!(m.contains(&(s("queryType"), s("CAP_EDGE"))));
    }
}
