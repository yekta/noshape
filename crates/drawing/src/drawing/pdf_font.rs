//! TrueType fonts embedded in a PDF (Export › PDF, Text: Selectable): live
//! embeds TrueType subsets of the drawing fonts that keep the full `name`
//! table (behaviours "asb (10-24) drawing font files"). Here a subset keeps
//! the glyph ids and the tables a PDF reads; the glyphs not drawn lose their
//! outlines, and the variation and layout tables go (the default instance
//! stays).

use std::collections::BTreeSet;

fn u16_at(d: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*d.get(o)?, *d.get(o + 1)?]))
}

fn u32_at(d: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_be_bytes([*d.get(o)?, *d.get(o + 1)?, *d.get(o + 2)?, *d.get(o + 3)?]))
}

/// The font's tables: (tag, data).
fn tables(d: &[u8]) -> Option<Vec<([u8; 4], &[u8])>> {
    let n = u16_at(d, 4)? as usize;
    (0..n)
        .map(|i| {
            let r = 12 + 16 * i;
            let tag: [u8; 4] = d.get(r..r + 4)?.try_into().ok()?;
            let (off, len) = (u32_at(d, r + 8)? as usize, u32_at(d, r + 12)? as usize);
            Some((tag, d.get(off..off + len)?))
        })
        .collect()
}

/// The glyph ids a composite glyph uses.
fn components(glyph: &[u8]) -> Vec<u16> {
    let mut out = Vec::new();
    if glyph.len() < 10 || i16::from_be_bytes([glyph[0], glyph[1]]) >= 0 {
        return out;
    }
    let mut o = 10;
    while let (Some(flags), Some(gid)) = (u16_at(glyph, o), u16_at(glyph, o + 2)) {
        out.push(gid);
        o += 4 + if flags & 1 != 0 { 4 } else { 2 };
        o += if flags & 8 != 0 {
            2
        } else if flags & 0x40 != 0 {
            4
        } else if flags & 0x80 != 0 {
            8
        } else {
            0
        };
        if flags & 0x20 == 0 {
            break;
        }
    }
    out
}

fn checksum(d: &[u8]) -> u32 {
    d.chunks(4).fold(0u32, |s, c| {
        let mut b = [0u8; 4];
        b[..c.len()].copy_from_slice(c);
        s.wrapping_add(u32::from_be_bytes(b))
    })
}

/// `data` with only the outlines of `used` glyphs (and the glyphs their
/// composites use, and glyph 0). None for a font without TrueType outlines
/// (a CFF font) or one that doesn't parse.
pub fn subset(data: &[u8], used: &BTreeSet<u16>) -> Option<Vec<u8>> {
    let t = tables(data)?;
    let get = |tag: &[u8; 4]| t.iter().find(|(x, _)| x == tag).map(|(_, d)| *d);
    let (glyf, loca, head) = (get(b"glyf")?, get(b"loca")?, get(b"head")?);
    let long = u16_at(head, 50)? == 1;
    let n = u16_at(get(b"maxp")?, 4)? as usize;
    let offset = |g: usize| -> Option<usize> { if long { u32_at(loca, 4 * g).map(|v| v as usize) } else { u16_at(loca, 2 * g).map(|v| 2 * v as usize) } };
    let glyph = |g: usize| -> Option<&[u8]> { glyf.get(offset(g)?..offset(g + 1)?) };
    let mut keep: BTreeSet<u16> = used.iter().copied().filter(|g| (*g as usize) < n).collect();
    keep.insert(0);
    let mut todo: Vec<u16> = keep.iter().copied().collect();
    while let Some(g) = todo.pop() {
        for c in components(glyph(g as usize)?) {
            if (c as usize) < n && keep.insert(c) {
                todo.push(c);
            }
        }
    }
    let (mut new_glyf, mut offsets) = (Vec::new(), Vec::with_capacity(n + 1));
    for g in 0..n {
        offsets.push(new_glyf.len());
        if keep.contains(&(g as u16)) {
            new_glyf.extend_from_slice(glyph(g)?);
            while new_glyf.len() % 4 != 0 {
                new_glyf.push(0);
            }
        }
    }
    offsets.push(new_glyf.len());
    let new_loca: Vec<u8> = if long { offsets.iter().flat_map(|o| (*o as u32).to_be_bytes()).collect() } else { offsets.iter().flat_map(|o| ((*o / 2) as u16).to_be_bytes()).collect() };
    // The default instance only: a variable font's variation tables go; so
    // do the layout tables (a PDF places each glyph itself) and signatures.
    const DROP: [&[u8; 4]; 12] = [b"gvar", b"fvar", b"avar", b"HVAR", b"MVAR", b"VVAR", b"STAT", b"cvar", b"GSUB", b"GPOS", b"GDEF", b"DSIG"];
    let out_tables: Vec<([u8; 4], Vec<u8>)> = t
        .iter()
        .filter(|(tag, _)| !DROP.contains(&tag))
        .map(|(tag, d)| {
            let body = match tag {
                b"glyf" => new_glyf.clone(),
                b"loca" => new_loca.clone(),
                // checkSumAdjustment is recomputed below.
                b"head" => {
                    let mut h = d.to_vec();
                    h[8..12].copy_from_slice(&[0; 4]);
                    h
                }
                _ => d.to_vec(),
            };
            (*tag, body)
        })
        .collect();
    let count = out_tables.len() as u16;
    let mut out = Vec::new();
    out.extend_from_slice(&data[0..4]);
    out.extend_from_slice(&count.to_be_bytes());
    let pow = 1u16 << (15 - count.leading_zeros().min(15));
    let pow = if pow > count { pow / 2 } else { pow };
    out.extend_from_slice(&(pow * 16).to_be_bytes());
    out.extend_from_slice(&(pow.trailing_zeros() as u16).to_be_bytes());
    out.extend_from_slice(&(count * 16 - pow * 16).to_be_bytes());
    let mut offset = 12 + 16 * out_tables.len();
    let mut head_at = None;
    for (tag, body) in &out_tables {
        if tag == b"head" {
            head_at = Some(offset);
        }
        out.extend_from_slice(tag);
        out.extend_from_slice(&checksum(body).to_be_bytes());
        out.extend_from_slice(&(offset as u32).to_be_bytes());
        out.extend_from_slice(&(body.len() as u32).to_be_bytes());
        offset += body.len().div_ceil(4) * 4;
    }
    for (_, body) in &out_tables {
        out.extend_from_slice(body);
        while out.len() % 4 != 0 {
            out.push(0);
        }
    }
    if let Some(h) = head_at {
        let adj = 0xB1B0_AFBAu32.wrapping_sub(checksum(&out));
        out[h + 8..h + 12].copy_from_slice(&adj.to_be_bytes());
    }
    Some(out)
}

/// A six-letter subset tag ("ABCDEF+") from the glyphs kept.
pub fn subset_tag(used: &BTreeSet<u16>, k: usize) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325 ^ k as u64;
    for g in used {
        h = (h ^ *g as u64).wrapping_mul(0x100_0000_01b3);
    }
    (0..6).map(|i| (b'A' + ((h >> (5 * i)) % 26) as u8) as char).collect()
}

/// A ToUnicode CMap: glyph id → the character it draws.
pub fn to_unicode(map: &std::collections::BTreeMap<u16, char>) -> String {
    let mut s = String::from("/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n/CMapName /Adobe-Identity-UCS def\n/CMapType 2 def\n1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n");
    let pairs: Vec<(u16, char)> = map.iter().map(|(g, c)| (*g, *c)).collect();
    for chunk in pairs.chunks(100) {
        s.push_str(&format!("{} beginbfchar\n", chunk.len()));
        for (g, c) in chunk {
            let mut u = [0u16; 2];
            let hex: String = c.encode_utf16(&mut u).iter().map(|x| format!("{x:04X}")).collect();
            s.push_str(&format!("<{g:04X}> <{hex}>\n"));
        }
        s.push_str("endbfchar\n");
    }
    s.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
    s
}

#[cfg(test)]
mod tests {
    #[test]
    fn subset_keeps_the_glyphs_drawn_and_every_table() {
        let face = noshape_sketch::text::drawing_embed_face("Noto Sans", false, false).unwrap();
        let glyphs = noshape_sketch::text::drawing_glyphs("Ab", face.data);
        let used: std::collections::BTreeSet<u16> = glyphs[0].iter().map(|g| g.0).collect();
        let sub = super::subset(face.data, &used).unwrap();
        assert!(sub.len() * 2 < face.data.len(), "{} of {}", sub.len(), face.data.len());
        let names = |d: &[u8]| super::tables(d).unwrap().iter().map(|t| t.0).collect::<Vec<_>>();
        assert_eq!(names(&sub), names(face.data).into_iter().filter(|t| !matches!(t, b"gvar" | b"fvar" | b"avar" | b"HVAR" | b"MVAR" | b"VVAR" | b"STAT" | b"cvar" | b"GSUB" | b"GPOS" | b"GDEF" | b"DSIG")).collect::<Vec<_>>());
        // The kept glyphs' outlines are the original's; another glyph has none.
        let (a, b) = (super::tables(face.data).unwrap(), super::tables(&sub).unwrap());
        let get = |t: &Vec<([u8; 4], &[u8])>, tag: &[u8; 4]| t.iter().find(|x| &x.0 == tag).unwrap().1.len();
        assert!(get(&b, b"glyf") < get(&a, b"glyf") / 20);
        assert_eq!(super::checksum(&sub), 0xB1B0_AFBA, "whole-font checksum");
    }
}
