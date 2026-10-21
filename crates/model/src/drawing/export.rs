//! Drawing export: SVG, DXF (AutoCAD R12 ASCII) and PDF (vector, built-in
//! Helvetica). All take a [`SheetGeometry`] in millimeters.

use super::font;
use super::SheetLayer;
use super::cad_layers::CadLayer;
use super::sheet::{Anchor, ColorSpan, Fill2, FontSpan, LayerSpan, LineStyle, Polyline2, SheetGeometry, Text2, TextFont};
use std::fmt::Write as _;

/// Texts with characters outside the PDF/DXF base fonts (GD&T and other
/// drawing symbols) are exported as strokes of the drawing font.
pub fn needs_strokes(t: &Text2) -> bool {
    t.text.chars().any(|c| !(c.is_ascii() || "Ø°±×µ²³".contains(c)))
}

/// Export Drawing dialog settings that change the geometry.
#[derive(Clone, Debug, PartialEq)]
pub struct ExportOptions {
    /// Every text as outlines (PDF Text "Normal", raster formats, DWG/DXF
    /// "Explode all into polylines"): filled glyphs, or strokes when
    /// `strokes` (DXF has no fills for text).
    pub explode_all: bool,
    pub strokes: bool,
    /// DXF "Explode into polylines (notes)": these text indices only.
    pub explode: Vec<usize>,
    /// Overridden dimensions: "Show underlines".
    pub underline_overrides: bool,
    /// "Exclude off-sheet content" (DWG/DXF).
    pub exclude_off_sheet: bool,
    /// Color: 0 Color, 1 Black and white (white stays white), 2 Grayscale.
    pub color_mode: u8,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self { explode_all: false, strokes: false, explode: Vec::new(), underline_overrides: true, exclude_off_sheet: false, color_mode: 0 }
    }
}

/// Rebuild color spans from per-element colors (consecutive runs).
fn spans_of(lines: &[Option<[u8; 3]>], texts: &[Option<[u8; 3]>], fills: &[Option<[u8; 3]>]) -> Vec<ColorSpan> {
    let mut out = Vec::new();
    let runs = |v: &[Option<[u8; 3]>]| {
        let mut r: Vec<(std::ops::Range<usize>, [u8; 3])> = Vec::new();
        for (i, c) in v.iter().enumerate() {
            let Some(c) = c else { continue };
            match r.last_mut() {
                Some((rg, rc)) if rg.end == i && rc == c => rg.end = i + 1,
                _ => r.push((i..i + 1, *c)),
            }
        }
        r
    };
    for (rg, c) in runs(lines) {
        out.push(ColorSpan { lines: rg, texts: 0..0, fills: 0..0, rgb: c });
    }
    for (rg, c) in runs(texts) {
        out.push(ColorSpan { lines: 0..0, texts: rg, fills: 0..0, rgb: c });
    }
    for (rg, c) in runs(fills) {
        out.push(ColorSpan { lines: 0..0, texts: 0..0, fills: rg, rgb: c });
    }
    out
}

/// The geometry an export writes, after [`ExportOptions`].
pub fn prepare(g: &SheetGeometry, o: &ExportOptions) -> SheetGeometry {
    let tone = |c: Option<[u8; 3]>| -> Option<[u8; 3]> {
        let c = c?;
        match o.color_mode {
            1 => (c == [255, 255, 255]).then_some(c),
            2 => {
                let y = (0.299 * c[0] as f64 + 0.587 * c[1] as f64 + 0.114 * c[2] as f64).round() as u8;
                Some([y, y, y])
            }
            _ => Some(c),
        }
    };
    let mut out = g.clone();
    out.colors.clear();
    out.backdrops = g.backdrops.iter().filter_map(|(p, c)| tone(Some(*c)).map(|c| (p.clone(), c))).collect();
    let mut lc: Vec<Option<[u8; 3]>> = (0..g.lines.len()).map(|i| tone(g.line_color(i))).collect();
    let mut fc: Vec<Option<[u8; 3]>> = (0..g.fills.len()).map(|i| tone(g.fill_color(i))).collect();
    let mut tc: Vec<Option<[u8; 3]>> = Vec::new();
    let mut tf: Vec<Option<TextFont>> = Vec::new();
    // Sheet layers per element (exploded text keeps its text's layer).
    let mut ll: Vec<Option<SheetLayer>> = (0..g.lines.len()).map(|i| g.line_sheet_layer(i)).collect();
    let mut fl: Vec<Option<SheetLayer>> = (0..g.fills.len()).map(|i| g.fill_sheet_layer(i)).collect();
    // Line pens (Adjust linestyle); added lines use their style's pen.
    let mut lp: Vec<Option<(f64, Vec<f64>)>> = (0..g.lines.len()).map(|i| g.pens.iter().rev().find(|s| s.lines.contains(&i)).map(|p| (p.width_mm, p.dashes.clone()))).collect();
    let mut tl: Vec<Option<SheetLayer>> = Vec::new();
    // Export layers (`cad_layers`) per element, kept the same way.
    // k: 0 lines, 1 texts, 2 fills.
    fn cad_of(spans: &[super::cad_layers::CadSpan], k: u8, i: usize) -> Option<CadLayer> {
        spans.iter().rev().find(|s| [&s.lines, &s.texts, &s.fills][k as usize].contains(&i)).map(|s| s.layer)
    }
    let mut lk: Vec<Option<CadLayer>> = (0..g.lines.len()).map(|i| cad_of(&g.cad, 0, i)).collect();
    let mut fk: Vec<Option<CadLayer>> = (0..g.fills.len()).map(|i| cad_of(&g.cad, 2, i)).collect();
    let mut tk: Vec<Option<CadLayer>> = Vec::new();
    out.fonts.clear();
    if o.underline_overrides {
        for &i in &g.overridden {
            let Some(t) = g.texts.get(i) else { continue };
            let b = font::text_box(t);
            out.lines.push(Polyline2 { points: vec![b[0], b[1]], style: LineStyle::Thin, closed: false, pick: t.pick });
            lc.push(tone(g.text_color(i)));
            ll.push(g.text_sheet_layer(i));
            lk.push(cad_of(&g.cad, 1, i));
        }
    }
    let texts = std::mem::take(&mut out.texts);
    for (i, t) in texts.into_iter().enumerate() {
        let c = tone(g.text_color(i));
        let layer = g.text_sheet_layer(i);
        let kind = cad_of(&g.cad, 1, i);
        if !(o.explode_all || o.explode.contains(&i)) {
            out.texts.push(t);
            tc.push(c);
            tl.push(layer);
            tk.push(kind);
            tf.push(g.text_font(i).cloned());
            continue;
        }
        if !o.strokes && font::fill_supported(&t.text) {
            let (pts, idx) = font::text_fill_font(&t, g.text_font(i));
            for tri in idx.chunks(3).filter(|c| c.len() == 3) {
                out.fills.push(Fill2 { points: tri.iter().map(|&k| pts[k as usize]).collect() });
                fc.push(c);
                fl.push(layer);
                fk.push(kind);
            }
        } else {
            for points in font::text_strokes(&t).into_iter().filter(|s| s.len() >= 2) {
                out.lines.push(Polyline2 { points, style: LineStyle::Thin, closed: false, pick: t.pick });
                lc.push(c);
                ll.push(layer);
                lk.push(kind);
            }
        }
    }
    out.overridden.clear();
    if o.exclude_off_sheet {
        let inside = |p: &[f64; 2]| p[0] >= -1e-6 && p[1] >= -1e-6 && p[0] <= g.width + 1e-6 && p[1] <= g.height + 1e-6;
        let keep_l: Vec<bool> = out.lines.iter().map(|l| l.points.iter().all(inside)).collect();
        let keep_f: Vec<bool> = out.fills.iter().map(|f| f.points.iter().all(inside)).collect();
        let keep_t: Vec<bool> = out.texts.iter().map(|t| font::text_box(t).iter().all(inside)).collect();
        fn filter<T>(v: &mut Vec<T>, keep: &[bool]) {
            let mut k = keep.iter();
            v.retain(|_| *k.next().unwrap_or(&true));
        }
        filter(&mut lc, &keep_l);
        lp.resize(keep_l.len(), None);
        filter(&mut lp, &keep_l);
        filter(&mut fc, &keep_f);
        filter(&mut tc, &keep_t);
        filter(&mut ll, &keep_l);
        filter(&mut fl, &keep_f);
        filter(&mut tl, &keep_t);
        lk.resize(keep_l.len(), None);
        filter(&mut lk, &keep_l);
        filter(&mut fk, &keep_f);
        filter(&mut tk, &keep_t);
        let mut k = keep_t.iter();
        tf.retain(|_| *k.next().unwrap_or(&true));
        // Hatch regions' line ranges after the removed lines.
        let kept_before: Vec<usize> = std::iter::once(0).chain(keep_l.iter().scan(0, |n, k| {
            *n += *k as usize;
            Some(*n)
        })).collect();
        let at = |i: usize| kept_before.get(i).copied().unwrap_or(*kept_before.last().unwrap_or(&0));
        for h in &mut out.hatches {
            h.lines = at(h.lines.start)..at(h.lines.end);
        }
        let mut k = keep_l.iter();
        out.lines.retain(|_| *k.next().unwrap_or(&true));
        let mut k = keep_f.iter();
        out.fills.retain(|_| *k.next().unwrap_or(&true));
        let mut k = keep_t.iter();
        out.texts.retain(|_| *k.next().unwrap_or(&true));
    }
    out.colors = spans_of(&lc, &tc, &fc);
    out.pens = lp.into_iter().enumerate().filter_map(|(i, p)| p.map(|(width_mm, dashes)| super::sheet::PenSpan { lines: i..i + 1, width_mm, dashes })).collect();
    let one = |i: usize, l: &Option<SheetLayer>, k: u8| l.map(|layer| LayerSpan { lines: if k == 0 { i..i + 1 } else { 0..0 }, texts: if k == 1 { i..i + 1 } else { 0..0 }, fills: if k == 2 { i..i + 1 } else { 0..0 }, layer });
    out.layers = ll.iter().enumerate().filter_map(|(i, l)| one(i, l, 0)).chain(tl.iter().enumerate().filter_map(|(i, l)| one(i, l, 1))).chain(fl.iter().enumerate().filter_map(|(i, l)| one(i, l, 2))).collect();
    let span = |i: usize, l: &Option<CadLayer>, k: u8| l.map(|layer| super::cad_layers::CadSpan { lines: if k == 0 { i..i + 1 } else { 0..0 }, texts: if k == 1 { i..i + 1 } else { 0..0 }, fills: if k == 2 { i..i + 1 } else { 0..0 }, layer });
    out.cad = lk.iter().enumerate().filter_map(|(i, l)| span(i, l, 0)).chain(tk.iter().enumerate().filter_map(|(i, l)| span(i, l, 1))).chain(fk.iter().enumerate().filter_map(|(i, l)| span(i, l, 2))).collect();
    out.fonts = tf.into_iter().enumerate().filter_map(|(i, f)| f.map(|font| FontSpan { texts: i..i + 1, font })).collect();
    out
}

/// SVG font attributes of a text.
fn svg_font(f: Option<&TextFont>) -> String {
    match f {
        None => "font-family=\"Helvetica, Arial, sans-serif\"".into(),
        Some(f) => {
            let generic = if f.family.contains("Serif") { "serif" } else if f.family.contains("Mono") { "monospace" } else { "sans-serif" };
            format!("font-family=\"{}, {generic}\"{}{}", esc_xml(&f.family), if f.bold { " font-weight=\"bold\"" } else { "" }, if f.italic { " font-style=\"italic\"" } else { "" })
        }
    }
}

/// PDF base-14 font resource (F1–F12) of a text.
fn pdf_font(f: Option<&TextFont>) -> usize {
    let Some(f) = f else { return 1 };
    let base = if f.family.contains("Serif") { 4 } else if f.family.contains("Mono") { 8 } else { 0 };
    base + 1 + usize::from(f.bold) + 2 * usize::from(f.italic)
}

const PDF_FONTS: [&str; 12] = ["Helvetica", "Helvetica-Bold", "Helvetica-Oblique", "Helvetica-BoldOblique", "Times-Roman", "Times-Bold", "Times-Italic", "Times-BoldItalic", "Courier", "Courier-Bold", "Courier-Oblique", "Courier-BoldOblique"];

/// "#rrggbb" of an element color (black ink by default).
fn hex(c: Option<[u8; 3]>) -> String {
    let c = c.unwrap_or([0, 0, 0]);
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

/// The nearest AutoCAD color index (DXF R12 has no true color): 7 is
/// black / white.
fn aci(c: Option<[u8; 3]>) -> u8 {
    let Some(c) = c else { return 7 };
    const TABLE: [(u8, [u8; 3]); 9] = [(1, [255, 0, 0]), (2, [255, 255, 0]), (3, [0, 255, 0]), (4, [0, 255, 255]), (5, [0, 0, 255]), (6, [255, 0, 255]), (7, [0, 0, 0]), (8, [128, 128, 128]), (9, [192, 192, 192])];
    let d = |a: [u8; 3]| (0..3).map(|i| (a[i] as i32 - c[i] as i32).pow(2)).sum::<i32>();
    TABLE.iter().min_by_key(|(_, rgb)| d(*rgb)).map(|t| t.0).unwrap_or(7)
}

fn stroke_width(t: &Text2) -> f64 {
    (t.height * 0.09).max(0.18)
}

fn esc_xml(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

pub fn to_svg(g: &SheetGeometry) -> String {
    let mut s = String::new();
    let _ = writeln!(
        s,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}mm" height="{h}mm" viewBox="0 0 {w} {h}">"#,
        w = g.width,
        h = g.height
    );
    let _ = writeln!(s, r#"<rect width="{}" height="{}" fill="white"/>"#, g.width, g.height);
    for im in &g.images {
        let _ = writeln!(
            s,
            r#"<image x="{:.4}" y="{:.4}" width="{:.4}" height="{:.4}" preserveAspectRatio="none" href="data:{};base64,{}"/>"#,
            im.pos[0],
            g.height - im.pos[1] - im.size[1],
            im.size[0],
            im.size[1],
            im.mime(),
            im.data_base64
        );
    }
    // Flip Y: sheet coordinates are Y-up.
    let _ = writeln!(s, r#"<g transform="translate(0 {}) scale(1 -1)" fill="none" stroke="black" stroke-linecap="round" stroke-linejoin="round">"#, g.height);
    for (pts, c) in &g.backdrops {
        let pts: Vec<String> = pts.iter().map(|p| format!("{:.4},{:.4}", p[0], p[1])).collect();
        let _ = writeln!(s, r#"<polygon points="{}" fill="{}" stroke="none"/>"#, pts.join(" "), hex(Some(*c)));
    }
    for (li, l) in g.lines.iter().enumerate() {
        if l.points.len() < 2 {
            continue;
        }
        let pts: Vec<String> = l.points.iter().map(|p| format!("{:.4},{:.4}", p[0], p[1])).collect();
        let tag = if l.closed { "polygon" } else { "polyline" };
        let stroke = g.line_color(li).map(|c| format!(r#" stroke="{}""#, hex(Some(c)))).unwrap_or_default();
        let (width_mm, dash) = g.line_pen(li);
        let dash_attr = if dash.is_empty() {
            String::new()
        } else {
            format!(r#" stroke-dasharray="{}""#, dash.iter().map(|d| d.to_string()).collect::<Vec<_>>().join(" "))
        };
        let _ = writeln!(s, r#"<{tag} points="{}" stroke-width="{}"{dash_attr}{stroke}/>"#, pts.join(" "), width_mm);
    }
    for (fi, f) in g.fills.iter().enumerate() {
        let pts: Vec<String> = f.points.iter().map(|p| format!("{:.4},{:.4}", p[0], p[1])).collect();
        let _ = writeln!(s, r#"<polygon points="{}" fill="{}" stroke="none"/>"#, pts.join(" "), hex(g.fill_color(fi)));
    }
    let _ = writeln!(s, "</g>");
    for (ti, t) in g.texts.iter().enumerate().filter(|(_, t)| needs_strokes(t)) {
        let w = stroke_width(t);
        for st in font::text_strokes(t) {
            let pts: Vec<String> = st.iter().map(|p| format!("{:.4},{:.4}", p[0], g.height - p[1])).collect();
            let _ = writeln!(s, r#"<polyline points="{}" fill="none" stroke="{}" stroke-width="{w:.3}" stroke-linecap="round" stroke-linejoin="round"/>"#, pts.join(" "), hex(g.text_color(ti)));
        }
    }
    for (ti, t) in g.texts.iter().enumerate().filter(|(_, t)| !needs_strokes(t)) {
        let (x, y) = (t.pos[0], g.height - t.pos[1]);
        let anchor = match t.anchor {
            Anchor::BottomLeft | Anchor::TopLeft => "start",
            Anchor::BottomCenter | Anchor::Center => "middle",
        };
        let baseline = match t.anchor {
            Anchor::Center => r#" dominant-baseline="central""#,
            Anchor::TopLeft => r#" dominant-baseline="hanging""#,
            _ => "",
        };
        let _ = writeln!(
            s,
            r#"<text x="{x:.4}" y="{y:.4}" {} font-size="{:.3}" fill="{}" text-anchor="{anchor}"{baseline} transform="rotate({:.3} {x:.4} {y:.4})">{}</text>"#,
            svg_font(g.text_font(ti)),
            t.height * 1.4,
            hex(g.text_color(ti)),
            -t.angle.to_degrees(),
            esc_xml(&t.text)
        );
    }
    let _ = writeln!(s, "</svg>");
    s
}

/// DXF R12 (AC1009): LINE/POLYLINE/TEXT/SOLID entities on per-style layers, in mm.
pub fn to_dxf(g: &SheetGeometry) -> String {
    to_dxf_units(g, 4)
}

/// DXF in a unit: `insunits` 1 inch, 2 foot, 4 mm, 5 cm, 6 m (Export
/// Drawing › Units); coordinates and text heights are converted.
pub fn to_dxf_units(g: &SheetGeometry, insunits: u8) -> String {
    let k = match insunits {
        1 => 1.0 / 25.4,
        2 => 1.0 / 304.8,
        5 => 0.1,
        6 => 0.001,
        _ => 1.0,
    };
    let scaled;
    let g = if k == 1.0 {
        g
    } else {
        let mut c = g.clone();
        let f = |p: &mut [f64; 2]| {
            p[0] *= k;
            p[1] *= k;
        };
        c.lines.iter_mut().flat_map(|l| l.points.iter_mut()).for_each(f);
        c.fills.iter_mut().flat_map(|l| l.points.iter_mut()).for_each(f);
        for t in &mut c.texts {
            f(&mut t.pos);
            t.height *= k;
        }
        scaled = c;
        &scaled
    };
    let mut s = String::new();
    let pair = |s: &mut String, code: i32, v: &str| {
        let _ = writeln!(s, "{code:>3}\n{v}");
    };
    pair(&mut s, 0, "SECTION");
    pair(&mut s, 2, "HEADER");
    pair(&mut s, 9, "$ACADVER");
    pair(&mut s, 1, "AC1009");
    pair(&mut s, 9, "$INSUNITS");
    pair(&mut s, 70, &insunits.to_string());
    pair(&mut s, 0, "ENDSEC");
    // Tables: line types and layers.
    pair(&mut s, 0, "SECTION");
    pair(&mut s, 2, "TABLES");
    // Onshape's line types (inches, in the file's units) and its layers in
    // capitals (R12 / R14, live qa5-sm-flat.dxf), the used ones.
    let line_types: [(&str, &str, &[f64]); 5] = [
        ("CONTINUOUS", "Solid line", &[]),
        ("DASHED", "Dashed line __ __ __ __ __", &[0.5, -0.25]),
        ("ONS_PHANTOM", "Onshape Phantom line ______ __ __ ______", &[0.25, -0.05, 0.05, -0.05, 0.05, -0.05]),
        ("CENTER", "Center line ____ _ ____ _ ____ _", &[0.45, -0.09, 0.09, -0.09]),
        ("HIDDEN", "Hidden line ____ ____ ____ ____", &[0.1, -0.05]),
    ];
    pair(&mut s, 0, "TABLE");
    pair(&mut s, 2, "LTYPE");
    pair(&mut s, 70, &line_types.len().to_string());
    for (name, description, pattern) in line_types {
        pair(&mut s, 0, "LTYPE");
        pair(&mut s, 2, name);
        pair(&mut s, 70, "0");
        pair(&mut s, 3, description);
        pair(&mut s, 72, "65");
        pair(&mut s, 73, &pattern.len().to_string());
        let to_units = |inch: f64| inch * 25.4 * k;
        let total: f64 = pattern.iter().map(|x| to_units(x.abs())).sum();
        pair(&mut s, 40, &format!("{total}"));
        for p in pattern {
            pair(&mut s, 49, &format!("{}", to_units(*p)));
        }
    }
    pair(&mut s, 0, "ENDTAB");
    let mut used: std::collections::BTreeSet<CadLayer> = (0..g.lines.len()).map(|i| g.cad_line_layer(i)).collect();
    used.extend((0..g.texts.len()).map(|i| g.cad_text_layer(i)));
    used.extend((0..g.fills.len()).map(|i| g.cad_fill_layer(i)));
    pair(&mut s, 0, "TABLE");
    pair(&mut s, 2, "LAYER");
    pair(&mut s, 70, &used.len().to_string());
    for l in &used {
        pair(&mut s, 0, "LAYER");
        pair(&mut s, 2, l.name(true));
        pair(&mut s, 70, "0");
        pair(&mut s, 62, "7");
        pair(&mut s, 6, &l.line_type().to_uppercase());
    }
    pair(&mut s, 0, "ENDTAB");
    pair(&mut s, 0, "ENDSEC");

    pair(&mut s, 0, "SECTION");
    pair(&mut s, 2, "ENTITIES");
    for (li, l) in g.lines.iter().enumerate() {
        let color = g.line_color(li).map(|c| aci(Some(c)));
        if l.points.len() == 2 && !l.closed {
            pair(&mut s, 0, "LINE");
            pair(&mut s, 8, g.cad_line_layer(li).name(true));
            if let Some(c) = color {
                pair(&mut s, 62, &c.to_string());
            }
            pair(&mut s, 10, &format!("{:.6}", l.points[0][0]));
            pair(&mut s, 20, &format!("{:.6}", l.points[0][1]));
            pair(&mut s, 11, &format!("{:.6}", l.points[1][0]));
            pair(&mut s, 21, &format!("{:.6}", l.points[1][1]));
        } else if l.points.len() >= 2 {
            pair(&mut s, 0, "POLYLINE");
            pair(&mut s, 8, g.cad_line_layer(li).name(true));
            if let Some(c) = color {
                pair(&mut s, 62, &c.to_string());
            }
            pair(&mut s, 66, "1");
            pair(&mut s, 70, if l.closed { "1" } else { "0" });
            for p in &l.points {
                pair(&mut s, 0, "VERTEX");
                pair(&mut s, 8, g.cad_line_layer(li).name(true));
                pair(&mut s, 10, &format!("{:.6}", p[0]));
                pair(&mut s, 20, &format!("{:.6}", p[1]));
            }
            pair(&mut s, 0, "SEQEND");
        }
    }
    for (fi, f) in g.fills.iter().enumerate() {
        if f.points.len() >= 3 {
            pair(&mut s, 0, "SOLID");
            pair(&mut s, 8, g.cad_fill_layer(fi).name(true));
            if let Some(c) = g.fill_color(fi) {
                pair(&mut s, 62, &aci(Some(c)).to_string());
            }
            let p = &f.points;
            let pts = [p[0], p[1], p[2], p[2]];
            for (i, q) in pts.iter().enumerate() {
                pair(&mut s, 10 + i as i32, &format!("{:.6}", q[0]));
                pair(&mut s, 20 + i as i32, &format!("{:.6}", q[1]));
            }
        }
    }
    for (ti, t) in g.texts.iter().enumerate().filter(|(_, t)| needs_strokes(t)) {
        for st in font::text_strokes(t) {
            if st.len() < 2 {
                continue;
            }
            pair(&mut s, 0, "POLYLINE");
            pair(&mut s, 8, g.cad_text_layer(ti).name(true));
            if let Some(c) = g.text_color(ti) {
                pair(&mut s, 62, &aci(Some(c)).to_string());
            }
            pair(&mut s, 66, "1");
            pair(&mut s, 70, "0");
            for p in &st {
                pair(&mut s, 0, "VERTEX");
                pair(&mut s, 8, g.cad_text_layer(ti).name(true));
                pair(&mut s, 10, &format!("{:.6}", p[0]));
                pair(&mut s, 20, &format!("{:.6}", p[1]));
            }
            pair(&mut s, 0, "SEQEND");
        }
    }
    for (ti, t) in g.texts.iter().enumerate().filter(|(_, t)| !needs_strokes(t)) {
        pair(&mut s, 0, "TEXT");
        pair(&mut s, 8, g.cad_text_layer(ti).name(true));
        if let Some(c) = g.text_color(ti) {
            pair(&mut s, 62, &aci(Some(c)).to_string());
        }
        pair(&mut s, 10, &format!("{:.6}", t.pos[0]));
        pair(&mut s, 20, &format!("{:.6}", t.pos[1]));
        pair(&mut s, 40, &format!("{:.4}", t.height));
        pair(&mut s, 1, &t.text);
        pair(&mut s, 50, &format!("{:.4}", t.angle.to_degrees()));
        let (h, v) = match t.anchor {
            Anchor::BottomLeft => (0, 0),
            Anchor::BottomCenter => (1, 0),
            Anchor::Center => (1, 2),
            Anchor::TopLeft => (0, 3),
        };
        if h != 0 || v != 0 {
            pair(&mut s, 72, &h.to_string());
            pair(&mut s, 73, &v.to_string());
            pair(&mut s, 11, &format!("{:.6}", t.pos[0]));
            pair(&mut s, 21, &format!("{:.6}", t.pos[1]));
        }
    }
    pair(&mut s, 0, "ENDSEC");
    pair(&mut s, 0, "EOF");
    s
}

/// PDF content stream of one sheet.
fn pdf_content(g: &SheetGeometry) -> String {
    let pt = 72.0 / 25.4;
    let mut content = String::new();
    content.push_str("1 J 1 j 0 0 0 RG 0 0 0 rg\n");
    let rgb = |c: Option<[u8; 3]>| {
        let c = c.unwrap_or([0, 0, 0]);
        format!("{:.3} {:.3} {:.3}", c[0] as f64 / 255.0, c[1] as f64 / 255.0, c[2] as f64 / 255.0)
    };
    for (pts, c) in &g.backdrops {
        let _ = writeln!(content, "{} rg", rgb(Some(*c)));
        for (i, p) in pts.iter().enumerate() {
            let _ = writeln!(content, "{:.3} {:.3} {}", p[0] * pt, p[1] * pt, if i == 0 { "m" } else { "l" });
        }
        content.push_str("f\n");
    }
    for (li, l) in g.lines.iter().enumerate() {
        if l.points.len() < 2 {
            continue;
        }
        let _ = writeln!(content, "{} RG", rgb(g.line_color(li)));
        let (width_mm, dash) = g.line_pen(li);
        if dash.is_empty() {
            content.push_str("[] 0 d\n");
        } else {
            let d: Vec<String> = dash.iter().map(|x| format!("{:.3}", x * pt)).collect();
            let _ = writeln!(content, "[{}] 0 d", d.join(" "));
        }
        let _ = writeln!(content, "{:.3} w", width_mm * pt);
        for (i, p) in l.points.iter().enumerate() {
            let _ = writeln!(content, "{:.3} {:.3} {}", p[0] * pt, p[1] * pt, if i == 0 { "m" } else { "l" });
        }
        content.push_str(if l.closed { "s\n" } else { "S\n" });
    }
    content.push_str("[] 0 d\n");
    for (fi, f) in g.fills.iter().enumerate() {
        let _ = writeln!(content, "{} rg", rgb(g.fill_color(fi)));
        for (i, p) in f.points.iter().enumerate() {
            let _ = writeln!(content, "{:.3} {:.3} {}", p[0] * pt, p[1] * pt, if i == 0 { "m" } else { "l" });
        }
        content.push_str("f\n");
    }
    for (ti, t) in g.texts.iter().enumerate().filter(|(_, t)| needs_strokes(t)) {
        let _ = writeln!(content, "{:.3} w {} RG", stroke_width(t) * pt, rgb(g.text_color(ti)));
        for st in font::text_strokes(t) {
            for (i, p) in st.iter().enumerate() {
                let _ = writeln!(content, "{:.3} {:.3} {}", p[0] * pt, p[1] * pt, if i == 0 { "m" } else { "l" });
            }
            content.push_str("S\n");
        }
    }
    for (ti, t) in g.texts.iter().enumerate().filter(|(_, t)| !needs_strokes(t)) {
        let size = t.height * 1.4 * pt;
        let _ = writeln!(content, "{} rg", rgb(g.text_color(ti)));
        // Approximate Helvetica advance: 0.5 em per char.
        let width = t.text.chars().count() as f64 * size * 0.5;
        let (dx, dy) = match t.anchor {
            Anchor::BottomLeft => (0.0, 0.0),
            Anchor::BottomCenter => (-width / 2.0, 0.0),
            Anchor::Center => (-width / 2.0, -size * 0.35),
            Anchor::TopLeft => (0.0, -size * 0.75),
        };
        let (c, s) = (t.angle.cos(), t.angle.sin());
        let x = t.pos[0] * pt + dx * c - dy * s;
        let y = t.pos[1] * pt + dx * s + dy * c;
        let text: String = t
            .text
            .chars()
            .map(|ch| match ch {
                '(' => "\\(".to_string(),
                ')' => "\\)".to_string(),
                '\\' => "\\\\".to_string(),
                'Ø' => "\\330".to_string(),
                '°' => "\\260".to_string(),
                '±' => "\\261".to_string(),
                c if c.is_ascii() => c.to_string(),
                _ => "?".to_string(),
            })
            .collect();
        let _ = writeln!(content, "BT /F{} {size:.3} Tf {c:.5} {s:.5} {:.5} {c:.5} {x:.3} {y:.3} Tm ({text}) Tj ET", pdf_font(g.text_font(ti)), -s);
    }
    content
}

/// Single-page vector PDF.
pub fn to_pdf(g: &SheetGeometry) -> Vec<u8> {
    to_pdf_pages(std::slice::from_ref(g))
}

/// Multi-page vector PDF: one page per sheet.
pub fn to_pdf_pages(pages: &[SheetGeometry]) -> Vec<u8> {
    to_pdf_pages_version(pages, "1.4")
}

/// [`to_pdf_pages`] with the header version (Export Drawing › Version).
pub fn to_pdf_pages_version(pages: &[SheetGeometry], version: &str) -> Vec<u8> {
    let pt = 72.0 / 25.4;
    let mut objects: Vec<Vec<u8>> = vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        format!("<< /Type /Pages /Kids [{}] /Count {} >>", (0..pages.len()).map(|i| format!("{} 0 R", 4 + 2 * i)).collect::<Vec<_>>().join(" "), pages.len()).into_bytes(),
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>".to_vec(),
    ];
    // Fonts F2–F12 follow the pages, then the image XObjects (decoded RGB
    // supplied by the caller).
    let font0 = 4 + 2 * pages.len();
    let fonts: String = (2..=12).map(|k| format!(" /F{k} {} 0 R", font0 + k - 2)).collect();
    let mut next = font0 + 11;
    let mut images: Vec<Vec<u8>> = Vec::new();
    for (i, g) in pages.iter().enumerate() {
        let mut draw = String::new();
        let mut xobjects = String::new();
        for im in &g.images {
            let Some((w, h, rgb)) = &im.rgb else { continue };
            let n = next;
            next += 1;
            let _ = write!(xobjects, " /Im{n} {n} 0 R");
            let _ = writeln!(draw, "q {:.3} 0 0 {:.3} {:.3} {:.3} cm /Im{n} Do Q", im.size[0] * pt, im.size[1] * pt, im.pos[0] * pt, im.pos[1] * pt);
            let mut obj = format!("<< /Type /XObject /Subtype /Image /Width {w} /Height {h} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Length {} >>\nstream\n", rgb.len()).into_bytes();
            obj.extend_from_slice(rgb);
            obj.extend_from_slice(b"\nendstream");
            images.push(obj);
        }
        let content = draw + pdf_content(g).as_str();
        let (w, h) = (g.width * pt, g.height * pt);
        let xo = if xobjects.is_empty() { String::new() } else { format!(" /XObject <<{xobjects} >>") };
        objects.push(format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {w:.3} {h:.3}] /Resources << /Font << /F1 3 0 R{fonts} >>{xo} >> /Contents {} 0 R >>", 5 + 2 * i).into_bytes());
        objects.push(format!("<< /Length {} >>\nstream\n{}endstream", content.len(), content).into_bytes());
    }
    for name in &PDF_FONTS[1..] {
        objects.push(format!("<< /Type /Font /Subtype /Type1 /BaseFont /{name} /Encoding /WinAnsiEncoding >>").into_bytes());
    }
    objects.extend(images);
    let mut out: Vec<u8> = format!("%PDF-{version}\n").into_bytes();
    out.extend_from_slice(b"%\xE2\xE3\xCF\xD3\n");
    let mut offsets = Vec::new();
    for (i, o) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        out.extend_from_slice(o);
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref = out.len();
    let mut tail = format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1);
    for o in offsets {
        let _ = writeln!(tail, "{o:010} 00000 n ");
    }
    let _ = write!(tail, "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objects.len() + 1);
    out.extend_from_slice(tail.as_bytes());
    out
}

/// A zip archive of stored (uncompressed) files: several sheets exported
/// at once download as one zip.
pub fn zip_store(files: &[(String, Vec<u8>)]) -> Vec<u8> {
    fn crc32(data: &[u8]) -> u32 {
        let mut c = !0u32;
        for &b in data {
            c ^= b as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 { (c >> 1) ^ 0xEDB8_8320 } else { c >> 1 };
            }
        }
        !c
    }
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in files {
        let (crc, len, off) = (crc32(data), data.len() as u32, out.len() as u32);
        let n = name.as_bytes();
        let head = |sig: u32, central: bool| {
            let mut h = Vec::new();
            h.extend_from_slice(&sig.to_le_bytes());
            if central {
                h.extend_from_slice(&20u16.to_le_bytes());
            }
            // Version 2.0, UTF-8 names, stored, no time.
            for v in [20u16, 0x0800, 0, 0, 0x21] {
                h.extend_from_slice(&v.to_le_bytes());
            }
            for v in [crc, len, len] {
                h.extend_from_slice(&v.to_le_bytes());
            }
            h.extend_from_slice(&(n.len() as u16).to_le_bytes());
            h.extend_from_slice(&0u16.to_le_bytes());
            if central {
                for v in [0u16, 0, 0] {
                    h.extend_from_slice(&v.to_le_bytes());
                }
                h.extend_from_slice(&0u32.to_le_bytes());
                h.extend_from_slice(&off.to_le_bytes());
            }
            h.extend_from_slice(n);
            h
        };
        out.extend(head(0x0403_4b50, false));
        out.extend_from_slice(data);
        central.extend(head(0x0201_4b50, true));
    }
    let (cd_off, cd_len) = (out.len() as u32, central.len() as u32);
    out.extend(central);
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    for v in [0u16, 0, files.len() as u16, files.len() as u16] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(&cd_len.to_le_bytes());
    out.extend_from_slice(&cd_off.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drawing::{DimOrientation, Sheet};

    fn sample() -> SheetGeometry {
        let sheet = Sheet::default();
        let (w, h) = sheet.size.size_mm();
        let mut g = SheetGeometry::new(w, h);
        g.add_border_and_title(&sheet);
        g.rect(50.0, 50.0, 150.0, 120.0, LineStyle::Visible);
        g.line([50.0, 85.0], [150.0, 85.0], LineStyle::Hidden);
        g.linear_dimension([50.0, 120.0], [150.0, 120.0], 10.0, DimOrientation::Horizontal, "100", 1);
        g
    }

    #[test]
    fn svg_is_well_formed_enough() {
        let s = to_svg(&sample());
        assert!(s.starts_with("<svg") && s.trim_end().ends_with("</svg>"));
        assert!(s.contains("stroke-dasharray"));
    }

    #[test]
    fn dxf_has_sections_and_entities() {
        let d = to_dxf(&sample());
        assert!(d.contains("ENTITIES") && d.contains("LINE") && d.contains("TEXT") && d.trim_end().ends_with("EOF"));
    }

    #[test]
    fn export_options_explode_underline_units() {
        let mut g = sample();
        g.overridden.push(g.texts.len() - 1);
        let n_text = g.texts.len();
        let p = prepare(&g, &ExportOptions { explode_all: true, strokes: true, ..Default::default() });
        assert!(p.texts.is_empty() && p.lines.len() > g.lines.len() + n_text);
        let u = prepare(&g, &ExportOptions::default());
        assert_eq!(u.lines.len(), g.lines.len() + 1);
        assert_eq!(u.texts.len(), n_text);
        let d = to_dxf_units(&g, 1);
        assert!(d.contains("$INSUNITS\n 70\n1"));
        let pdf = to_pdf_pages_version(&[g], "1.5");
        assert!(pdf.starts_with(b"%PDF-1.5\n"));
    }

    #[test]
    fn colors_survive_prepare_and_reach_writers() {
        let mut g = sample();
        let m = g.mark();
        g.rect(10.0, 10.0, 20.0, 20.0, LineStyle::Thin);
        g.texts.push(Text2 { pos: [15.0, 15.0], height: 2.5, text: "7".into(), angle: 0.0, anchor: Anchor::Center, pick: 0 });
        g.color_since(m, [255, 0, 0]);
        let red = |s: &str| s.contains("#ff0000");
        assert!(red(&to_svg(&g)));
        // Exploded text keeps its color (as fills), grayscale turns it gray.
        let p = prepare(&g, &ExportOptions { explode_all: true, ..Default::default() });
        assert!(p.fill_color(p.fills.len() - 1) == Some([255, 0, 0]));
        let gray = prepare(&g, &ExportOptions { color_mode: 2, ..Default::default() });
        assert_eq!(gray.line_color(gray.lines.len() - 1), Some([76, 76, 76]));
        let bw = prepare(&g, &ExportOptions { color_mode: 1, ..Default::default() });
        assert!(bw.colors.is_empty());
        assert!(to_dxf(&g).contains(" 62\n1\n"));
        assert!(String::from_utf8_lossy(&to_pdf(&g)).contains("1.000 0.000 0.000 RG"));
    }

    #[test]
    fn images_reach_svg_and_pdf() {
        let mut g = sample();
        let png = crate::sketch_feature::base64_encode(&[0x89, b'P', b'N', b'G', 13, 10, 26, 10, 0, 0, 0, 0]);
        g.images.push(super::super::sheet::GeomImage { pos: [10.0, 20.0], size: [30.0, 15.0], data_base64: png.into(), rgb: Some((2, 1, std::sync::Arc::new(vec![255, 0, 0, 0, 0, 255]))) });
        let svg = to_svg(&g);
        assert!(svg.contains("data:image/png;base64,") && svg.contains(&format!(r#"y="{:.4}""#, g.height - 35.0)));
        let pdf = to_pdf(&g);
        let text = String::from_utf8_lossy(&pdf);
        assert!(text.contains("/XObject << /Im17 17 0 R >>") && text.contains("/Subtype /Image /Width 2 /Height 1"));
        let sx = text.rfind("startxref\n").unwrap();
        let off: usize = text[sx + 10..].lines().next().unwrap().trim().parse().unwrap();
        assert_eq!(&pdf[off..off + 4], b"xref");
    }

    #[test]
    fn zip_store_lists_files() {
        let z = zip_store(&[("a.txt".into(), b"hello".to_vec()), ("b.txt".into(), b"world!".to_vec())]);
        assert_eq!(&z[..4], b"PK\x03\x04");
        // crc32("hello") = 0x3610a686, at offset 14 of the first header.
        assert_eq!(u32::from_le_bytes(z[14..18].try_into().unwrap()), 0x3610_a686);
        let end = z.len() - 22;
        assert_eq!(&z[end..end + 4], b"PK\x05\x06");
        assert_eq!(u16::from_le_bytes(z[end + 10..end + 12].try_into().unwrap()), 2);
    }

    #[test]
    fn pdf_has_valid_structure() {
        let p = to_pdf(&sample());
        let s = String::from_utf8_lossy(&p);
        assert!(s.starts_with("%PDF-1.4"));
        assert!(s.contains("startxref") && s.trim_end().ends_with("%%EOF"));
        // xref offset points at "xref".
        let sx = s.rfind("startxref\n").unwrap();
        let off: usize = s[sx + 10..].lines().next().unwrap().trim().parse().unwrap();
        assert_eq!(&p[off..off + 4], b"xref");
    }
}

#[cfg(test)]
mod visual {
    #[test]
    #[ignore]
    fn write_sample_svg() {
        let g = super::tests_sample();
        std::fs::write("/tmp/out/sheet.svg", super::to_svg(&g)).unwrap();
        std::fs::write("/tmp/out/sheet.pdf", super::to_pdf(&g)).unwrap();
    }
}

#[cfg(test)]
fn tests_sample() -> SheetGeometry {
    use crate::drawing::{DimOrientation, Sheet};
    let sheet = Sheet::default();
    let (w, h) = sheet.size.size_mm();
    let mut g = SheetGeometry::new(w, h);
    g.add_border_and_title(&sheet);
    g.rect(50.0, 50.0, 150.0, 120.0, LineStyle::Visible);
    g.line([50.0, 85.0], [150.0, 85.0], LineStyle::Hidden);
    g.line([100.0, 40.0], [100.0, 130.0], LineStyle::Centerline);
    g.linear_dimension([50.0, 120.0], [150.0, 120.0], 10.0, DimOrientation::Horizontal, "100", 1);
    g.linear_dimension([150.0, 50.0], [150.0, 120.0], 10.0, DimOrientation::Vertical, "70", 2);
    g
}
