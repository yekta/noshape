//! TrueType fonts embedded in a PDF (Export › PDF, Text: Selectable): live
//! embeds TrueType subsets of the drawing fonts that keep the full `name`
//! table (behaviours "asb (10-24) drawing font files"). Here a subset keeps
//! the glyph ids and the tables a PDF reads; the glyphs not drawn lose their
//! outlines, and the variation and layout tables go (the default instance
//! stays).

use std::collections::{BTreeMap, BTreeSet};

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

/// A subset of `data` with only the glyphs `used` (and the glyphs their
/// composites use, and glyph 0), renumbered from 0 in glyph id order, and
/// live's nine tables (samples/dx-pdf-selectable.pdf: cmap, glyf, head,
/// hhea, hmtx, loca, maxp, name, post; a 4.5 kB subset for 38 glyphs).
/// Returns the font and old → new glyph ids (the PDF's CIDToGIDMap); `used`
/// gives each glyph's character for the subset's cmap. None for a font
/// without TrueType outlines (a CFF font) or one that doesn't parse.
pub fn subset(data: &[u8], used: &BTreeMap<u16, char>) -> Option<(Vec<u8>, BTreeMap<u16, u16>)> {
    let t = tables(data)?;
    let get = |tag: &[u8; 4]| t.iter().find(|(x, _)| x == tag).map(|(_, d)| *d);
    let (glyf, loca, head, hhea, hmtx, maxp) = (get(b"glyf")?, get(b"loca")?, get(b"head")?, get(b"hhea")?, get(b"hmtx")?, get(b"maxp")?);
    let long = u16_at(head, 50)? == 1;
    let n = u16_at(maxp, 4)? as usize;
    let offset = |g: usize| -> Option<usize> { if long { u32_at(loca, 4 * g).map(|v| v as usize) } else { u16_at(loca, 2 * g).map(|v| 2 * v as usize) } };
    let glyph = |g: usize| -> Option<&[u8]> { glyf.get(offset(g)?..offset(g + 1)?) };
    let mut keep: BTreeSet<u16> = used.keys().copied().filter(|g| (*g as usize) < n).collect();
    keep.insert(0);
    let mut todo: Vec<u16> = keep.iter().copied().collect();
    while let Some(g) = todo.pop() {
        for c in components(glyph(g as usize)?) {
            if (c as usize) < n && keep.insert(c) {
                todo.push(c);
            }
        }
    }
    let map: BTreeMap<u16, u16> = keep.iter().enumerate().map(|(i, g)| (*g, i as u16)).collect();
    // Outlines, composites pointing at the new ids.
    let (mut new_glyf, mut offsets) = (Vec::new(), Vec::with_capacity(keep.len() + 1));
    for g in &keep {
        offsets.push(new_glyf.len());
        let mut d = glyph(*g as usize)?.to_vec();
        if d.len() >= 10 && i16::from_be_bytes([d[0], d[1]]) < 0 {
            let mut o = 10;
            while let (Some(flags), Some(gid)) = (u16_at(&d, o), u16_at(&d, o + 2)) {
                let to = map.get(&gid).copied().unwrap_or(0);
                d[o + 2..o + 4].copy_from_slice(&to.to_be_bytes());
                o += 4 + if flags & 1 != 0 { 4 } else { 2 } + if flags & 8 != 0 { 2 } else if flags & 0x40 != 0 { 4 } else if flags & 0x80 != 0 { 8 } else { 0 };
                if flags & 0x20 == 0 {
                    break;
                }
            }
        }
        new_glyf.extend_from_slice(&d);
        while new_glyf.len() % 4 != 0 {
            new_glyf.push(0);
        }
    }
    offsets.push(new_glyf.len());
    let new_loca: Vec<u8> = if long { offsets.iter().flat_map(|o| (*o as u32).to_be_bytes()).collect() } else { offsets.iter().flat_map(|o| ((*o / 2) as u16).to_be_bytes()).collect() };
    // Every kept glyph a long metric (advance, left side bearing).
    let nhm = (u16_at(hhea, 34)? as usize).max(1);
    let mut new_hmtx = Vec::with_capacity(4 * keep.len());
    for g in &keep {
        let g = *g as usize;
        let adv = u16_at(hmtx, 4 * g.min(nhm - 1))?;
        let lsb = if g < nhm { u16_at(hmtx, 4 * g + 2)? } else { u16_at(hmtx, 4 * nhm + 2 * (g - nhm)).unwrap_or(0) };
        new_hmtx.extend_from_slice(&adv.to_be_bytes());
        new_hmtx.extend_from_slice(&lsb.to_be_bytes());
    }
    let count = keep.len() as u16;
    let mut new_hhea = hhea.to_vec();
    new_hhea[34..36].copy_from_slice(&count.to_be_bytes());
    let mut new_maxp = maxp.to_vec();
    new_maxp[4..6].copy_from_slice(&count.to_be_bytes());
    let mut new_head = head.to_vec();
    // checkSumAdjustment is recomputed below.
    new_head[8..12].copy_from_slice(&[0; 4]);
    // post format 3: no glyph names.
    let mut new_post = get(b"post")?.get(..32)?.to_vec();
    new_post[0..4].copy_from_slice(&0x0003_0000u32.to_be_bytes());
    let chars: BTreeMap<u16, u16> = used.iter().filter_map(|(g, c)| Some((u16::try_from(*c as u32).ok()?, *map.get(g)?))).collect();
    let out_tables: Vec<([u8; 4], Vec<u8>)> = vec![
        (*b"cmap", cmap4(&chars)),
        (*b"glyf", new_glyf),
        (*b"head", new_head),
        (*b"hhea", new_hhea),
        (*b"hmtx", new_hmtx),
        (*b"loca", new_loca),
        (*b"maxp", new_maxp),
        (*b"name", get(b"name")?.to_vec()),
        (*b"post", new_post),
    ];
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
    Some((out, map))
}

/// A cmap table with one (3, 1) format 4 subtable: each character its own
/// segment to its glyph.
fn cmap4(chars: &BTreeMap<u16, u16>) -> Vec<u8> {
    let mut segs: Vec<(u16, u16)> = chars.iter().filter(|(c, _)| **c != 0xFFFF).map(|(c, g)| (*c, *g)).collect();
    segs.push((0xFFFF, 0));
    let n = segs.len() as u16;
    let pow = 1u16 << (15 - n.leading_zeros().min(15));
    let pow = if pow > n { pow / 2 } else { pow };
    let mut sub = Vec::new();
    let w = |v: &mut Vec<u8>, x: u16| v.extend_from_slice(&x.to_be_bytes());
    w(&mut sub, 4);
    w(&mut sub, 16 + 8 * n);
    w(&mut sub, 0);
    w(&mut sub, 2 * n);
    w(&mut sub, 2 * pow);
    w(&mut sub, pow.trailing_zeros() as u16);
    w(&mut sub, 2 * n - 2 * pow);
    for (c, _) in &segs {
        w(&mut sub, *c);
    }
    w(&mut sub, 0);
    for (c, _) in &segs {
        w(&mut sub, *c);
    }
    for (c, g) in &segs {
        w(&mut sub, if *c == 0xFFFF { 1 } else { g.wrapping_sub(*c) });
    }
    for _ in &segs {
        w(&mut sub, 0);
    }
    let mut out = Vec::new();
    for x in [0u16, 1, 3, 1] {
        w(&mut out, x);
    }
    out.extend_from_slice(&12u32.to_be_bytes());
    out.extend_from_slice(&sub);
    out
}

/// The PDF's CIDToGIDMap stream data: for each CID (the original glyph id)
/// up to the highest used, its glyph in the subset (big-endian u16).
pub fn cid_to_gid(map: &BTreeMap<u16, u16>) -> Vec<u8> {
    let top = map.keys().next_back().copied().unwrap_or(0) as usize;
    let mut out = vec![0u8; 2 * (top + 1)];
    for (cid, gid) in map {
        let i = 2 * *cid as usize;
        out[i..i + 2].copy_from_slice(&gid.to_be_bytes());
    }
    out
}

/// A ToUnicode CMap: glyph id → the character it draws.
pub fn to_unicode(map: &BTreeMap<u16, char>) -> String {
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
    /// Live's anatomy (samples/dx-pdf-selectable.pdf): the drawn glyphs
    /// renumbered from 0, nine tables, a few kB.
    #[test]
    fn subset_renumbers_the_glyphs_drawn_with_lives_tables() {
        let face = noshape_sketch::text::drawing_embed_face("Noto Sans", false, false).unwrap();
        let glyphs = noshape_sketch::text::drawing_glyphs("Ab", face.data);
        let used: super::BTreeMap<u16, char> = glyphs[0].iter().map(|g| (g.0, g.1)).collect();
        let (sub, map) = super::subset(face.data, &used).unwrap();
        assert!(sub.len() < 4000, "{} of {}", sub.len(), face.data.len());
        let names = |d: &[u8]| super::tables(d).unwrap().iter().map(|t| t.0).collect::<Vec<_>>();
        assert_eq!(names(&sub), [*b"cmap", *b"glyf", *b"head", *b"hhea", *b"hmtx", *b"loca", *b"maxp", *b"name", *b"post"]);
        // Glyph 0 stays 0; the drawn ones follow in order.
        assert_eq!(map.get(&0), Some(&0));
        assert_eq!(map.len(), 3);
        let t = super::tables(&sub).unwrap();
        let get = |tag: &[u8; 4]| t.iter().find(|x| &x.0 == tag).unwrap().1;
        assert_eq!(super::u16_at(get(b"maxp"), 4), Some(3));
        assert_eq!(get(b"hmtx").len(), 12);
        // The cmap maps 'A' to its new glyph.
        let a = used.iter().find(|(_, c)| **c == 'A').unwrap().0;
        let cmap = get(b"cmap");
        assert!(cmap.windows(2).any(|w| w == [0, b'A']) && map[a] > 0);
        assert_eq!(super::checksum(&sub), 0xB1B0_AFBA, "whole-font checksum");
        let cg = super::cid_to_gid(&map);
        assert_eq!(cg.len(), 2 * (*map.keys().last().unwrap() as usize + 1));
        assert_eq!(super::u16_at(&cg, 2 * *a as usize), Some(map[a]));
    }
}
