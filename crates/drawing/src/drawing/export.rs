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

/// [`needs_strokes`], or text `ti` is in an SHX shape font (live's PDF draws
/// those as strokes, Type 3 fonts).
fn stroked(g: &SheetGeometry, ti: usize, t: &Text2) -> bool {
    needs_strokes(t) || g.text_font(ti).is_some_and(|f| super::sheet::is_shx(&f.family))
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

/// Consecutive runs of equal per-element values: one span each (an
/// element's lookup scans the spans, so one span per element made exporting
/// a drawing of many elements quadratic: a 1000-row BOM's exploded text).
fn runs<T: PartialEq + Clone>(v: &[Option<T>]) -> Vec<(std::ops::Range<usize>, T)> {
    let mut r: Vec<(std::ops::Range<usize>, T)> = Vec::new();
    for (i, c) in v.iter().enumerate() {
        let Some(c) = c else { continue };
        match r.last_mut() {
            Some((rg, rc)) if rg.end == i && rc == c => rg.end = i + 1,
            _ => r.push((i..i + 1, c.clone())),
        }
    }
    r
}

/// [`runs`] of per-line, per-text and per-fill values as spans made by `mk`
/// (its ranges: lines, texts, fills; two of them empty).
fn kind_runs<T: PartialEq + Clone, S>(v: [&[Option<T>]; 3], mk: impl Fn([std::ops::Range<usize>; 3], T) -> S) -> Vec<S> {
    let mut out = Vec::new();
    for (k, list) in v.into_iter().enumerate() {
        for (r, x) in runs(list) {
            let mut ranges = [0..0, 0..0, 0..0];
            ranges[k] = r;
            out.push(mk(ranges, x));
        }
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
    let mut lp: Vec<Option<(f64, Vec<f64>, bool)>> = (0..g.lines.len()).map(|i| g.pens.iter().rev().find(|s| s.lines.contains(&i)).map(|p| (p.width_mm, p.dashes.clone(), p.centered))).collect();
    let mut tl: Vec<Option<SheetLayer>> = Vec::new();
    // Export layers (`cad_layers`) per element, kept the same way.
    // k: 0 lines, 1 texts, 2 fills.
    fn cad_of(spans: &[super::cad_layers::CadSpan], k: u8, i: usize) -> Option<CadLayer> {
        spans.iter().rev().find(|s| [&s.lines, &s.texts, &s.fills][k as usize].contains(&i)).map(|s| s.layer)
    }
    let mut lk: Vec<Option<CadLayer>> = (0..g.lines.len()).map(|i| cad_of(&g.cad, 0, i)).collect();
    let mut fk: Vec<Option<CadLayer>> = (0..g.fills.len()).map(|i| cad_of(&g.cad, 2, i)).collect();
    let mut tk: Vec<Option<CadLayer>> = Vec::new();
    // Text styles of the kept texts (exploded text has none).
    let mut ts: Vec<Option<super::cad_layers::CadTextStyle>> = Vec::new();
    // The MTEXT each kept text is part of (its index, width, height).
    let mut tm: Vec<Option<(usize, f64, f64, bool)>> = Vec::new();
    // CAD objects (dimensions) per element, kept the same way.
    let (mut lo, text_obj, mut fo) = g.cad_members_of();
    let mut to: Vec<Option<usize>> = Vec::new();
    out.fonts.clear();
    if o.underline_overrides {
        for &i in &g.overridden {
            let Some(t) = g.texts.get(i) else { continue };
            let b = font::text_box(t);
            out.lines.push(Polyline2 { points: vec![b[0], b[1]], style: LineStyle::Thin, closed: false, pick: t.pick });
            lc.push(tone(g.text_color(i)));
            ll.push(g.text_sheet_layer(i));
            lk.push(cad_of(&g.cad, 1, i));
            lo.push(text_obj[i]);
        }
    }
    let texts = std::mem::take(&mut out.texts);
    for (i, t) in texts.into_iter().enumerate() {
        let c = tone(g.text_color(i));
        let layer = g.text_sheet_layer(i);
        let kind = cad_of(&g.cad, 1, i);
        let obj = text_obj[i];
        if !(o.explode_all || o.explode.contains(&i)) {
            out.texts.push(t);
            tc.push(c);
            tl.push(layer);
            tk.push(kind);
            ts.push(g.cad_text_style(i));
            tm.push(g.cad_mtext.iter().rposition(|m| m.texts.contains(&i)).map(|k| (k, g.cad_mtext[k].width, g.cad_mtext[k].height, g.cad_mtext[k].wrapped)));
            to.push(obj);
            tf.push(g.text_font(i).cloned());
            continue;
        }
        if !o.strokes && !font::text_is_stroked(&t, g.text_font(i)) {
            let (pts, idx) = font::text_fill_font(&t, g.text_font(i));
            for tri in idx.chunks(3).filter(|c| c.len() == 3) {
                out.fills.push(Fill2 { points: tri.iter().map(|&k| pts[k as usize]).collect() });
                fc.push(c);
                fl.push(layer);
                fk.push(kind);
                fo.push(obj);
            }
        } else if !o.strokes && !g.text_font(i).is_some_and(|f| super::sheet::is_shx(&f.family)) {
            // A symbol with no outline in our fonts (GD&T ⌖ …): its strokes
            // filled, as live's outlined text.
            for st in font::text_strokes_font(&t, g.text_font(i)) {
                for quad in stroke_quads(&st, stroke_width(&t) / 2.0) {
                    out.fills.push(Fill2 { points: quad.to_vec() });
                    fc.push(c);
                    fl.push(layer);
                    fk.push(kind);
                    fo.push(obj);
                }
            }
        } else {
            for points in font::text_strokes_font(&t, g.text_font(i)).into_iter().filter(|s| s.len() >= 2) {
                out.lines.push(Polyline2 { points, style: LineStyle::Thin, closed: false, pick: t.pick });
                lc.push(c);
                ll.push(layer);
                lk.push(kind);
                lo.push(obj);
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
        filter(&mut ts, &keep_t);
        filter(&mut tm, &keep_t);
        lo.resize(keep_l.len(), None);
        filter(&mut lo, &keep_l);
        filter(&mut fo, &keep_f);
        filter(&mut to, &keep_t);
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
        let kept: Vec<usize> = keep_f.iter().scan(0, |n, k| {
            let i = *n;
            *n += *k as usize;
            Some(i)
        }).collect();
        out.outlined_fills = out.outlined_fills.iter().filter(|&&i| keep_f.get(i).copied().unwrap_or(true)).map(|&i| kept.get(i).copied().unwrap_or(i)).collect();
        let mut k = keep_t.iter();
        out.texts.retain(|_| *k.next().unwrap_or(&true));
    }
    out.colors = kind_runs([&lc, &tc, &fc], |[lines, texts, fills], rgb| ColorSpan { lines, texts, fills, rgb });
    out.pens = runs(&lp).into_iter().map(|(lines, (width_mm, dashes, centered))| super::sheet::PenSpan { centered, lines, width_mm, dashes }).collect();
    out.layers = kind_runs([&ll, &tl, &fl], |[lines, texts, fills], layer| LayerSpan { lines, texts, fills, layer });
    out.cad = kind_runs([&lk, &tk, &fk], |[lines, texts, fills], layer| super::cad_layers::CadSpan { lines, texts, fills, layer });
    out.cad_members = kind_runs([&lo, &to, &fo], |[lines, texts, fills], obj| super::cad_objects::CadMember { lines, texts, fills, obj });
    out.cad_text_styles = runs(&ts).into_iter().map(|(texts, style)| super::cad_layers::CadTextStyleSpan { texts, style }).collect();
    out.cad_mtext = runs(&tm).into_iter().map(|(texts, (_, width, height, wrapped))| super::cad_layers::CadMtext { texts, width, height, wrapped }).collect();
    out.fonts = runs(&tf).into_iter().map(|(texts, font)| FontSpan { texts, font }).collect();
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
pub(crate) fn aci(c: Option<[u8; 3]>) -> u8 {
    let Some(c) = c else { return 7 };
    const TABLE: [(u8, [u8; 3]); 9] = [(1, [255, 0, 0]), (2, [255, 255, 0]), (3, [0, 255, 0]), (4, [0, 255, 255]), (5, [0, 0, 255]), (6, [255, 0, 255]), (7, [0, 0, 0]), (8, [128, 128, 128]), (9, [192, 192, 192])];
    let d = |a: [u8; 3]| (0..3).map(|i| (a[i] as i32 - c[i] as i32).pow(2)).sum::<i32>();
    TABLE.iter().min_by_key(|(_, rgb)| d(*rgb)).map(|t| t.0).unwrap_or(7)
}

/// A stroke `r` thick on each side as filled quads, one per segment (all
/// turning the same way, so a nonzero fill makes their union).
fn stroke_quads(points: &[[f64; 2]], r: f64) -> Vec<[[f64; 2]; 4]> {
    points
        .windows(2)
        .filter_map(|w| {
            let (a, b) = (w[0], w[1]);
            let len = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
            if len < 1e-9 {
                return None;
            }
            let (u, n) = ([(b[0] - a[0]) / len * r, (b[1] - a[1]) / len * r], [-(b[1] - a[1]) / len * r, (b[0] - a[0]) / len * r]);
            Some([[a[0] - u[0] + n[0], a[1] - u[1] + n[1]], [b[0] + u[0] + n[0], b[1] + u[1] + n[1]], [b[0] + u[0] - n[0], b[1] + u[1] - n[1]], [a[0] - u[0] - n[0], a[1] - u[1] - n[1]]])
        })
        .collect()
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
            let phase = g.line_dash_phase(li);
            let offset = if phase != 0.0 { format!(r#" stroke-dashoffset="{phase}""#) } else { String::new() };
            format!(r#" stroke-dasharray="{}"{offset}"#, dash.iter().map(|d| d.to_string()).collect::<Vec<_>>().join(" "))
        };
        let _ = writeln!(s, r#"<{tag} points="{}" stroke-width="{}"{dash_attr}{stroke}/>"#, pts.join(" "), width_mm);
    }
    // Consecutive fills of one colour (an exploded text's triangles) are one
    // path, as in the PDF: a raster export of a 1000-row BOM parsed 250,000
    // polygons.
    let colors: Vec<String> = (0..g.fills.len()).map(|fi| hex(g.fill_color(fi))).collect();
    for (fi, f) in g.fills.iter().enumerate() {
        if fi == 0 || colors[fi - 1] != colors[fi] {
            s.push_str("<path d=\"");
        }
        if f.points.len() >= 3 {
            for (i, p) in f.points.iter().enumerate() {
                let _ = write!(s, "{}{:.4},{:.4} ", if i == 0 { "M" } else { "L" }, p[0], p[1]);
            }
            s.push('Z');
        }
        if colors.get(fi + 1) != Some(&colors[fi]) {
            let _ = writeln!(s, r#"" fill="{}" stroke="none"/>"#, colors[fi]);
        }
    }
    let _ = writeln!(s, "</g>");
    for (ti, t) in g.texts.iter().enumerate().filter(|(ti, t)| stroked(g, *ti, t)) {
        let w = stroke_width(t);
        for st in font::text_strokes_font(t, g.text_font(ti)) {
            let pts: Vec<String> = st.iter().map(|p| format!("{:.4},{:.4}", p[0], g.height - p[1])).collect();
            let _ = writeln!(s, r#"<polyline points="{}" fill="none" stroke="{}" stroke-width="{w:.3}" stroke-linecap="round" stroke-linejoin="round"/>"#, pts.join(" "), hex(g.text_color(ti)));
        }
    }
    for (ti, t) in g.texts.iter().enumerate().filter(|(ti, t)| !stroked(g, *ti, t)) {
        let (x, y) = (t.pos[0], g.height - t.pos[1]);
        let anchor = match t.anchor {
            Anchor::BottomLeft | Anchor::TopLeft => "start",
            Anchor::BottomCenter | Anchor::Center | Anchor::TopCenter => "middle",
        };
        let baseline = match t.anchor {
            Anchor::Center => r#" dominant-baseline="central""#,
            Anchor::TopLeft | Anchor::TopCenter => r#" dominant-baseline="hanging""#,
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
    // Translucent overlays over everything (a release watermark).
    if !g.overlays.is_empty() {
        let _ = writeln!(s, r#"<g transform="translate(0 {}) scale(1 -1)" stroke="none">"#, g.height);
        for (pts, c, a) in &g.overlays {
            let pts: Vec<String> = pts.iter().map(|p| format!("{:.4},{:.4}", p[0], p[1])).collect();
            let _ = writeln!(s, r#"<polygon points="{}" fill="{}" fill-opacity="{a:.3}"/>"#, pts.join(" "), hex(Some(*c)));
        }
        let _ = writeln!(s, "</g>");
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
    for (ti, t) in g.texts.iter().enumerate().filter(|(ti, t)| stroked(g, *ti, t)) {
        for st in font::text_strokes_font(t, g.text_font(ti)) {
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
    // (An empty title block cell is no TEXT.)
    for (ti, t) in g.texts.iter().enumerate().filter(|(ti, t)| !stroked(g, *ti, t) && !t.text.is_empty()) {
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
            Anchor::TopCenter => (1, 3),
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

/// PDF content stream of one sheet. Each element is drawn inside its
/// export layer's optional content group (live's PDFs list the DXF layers
/// in a viewer's Layers panel: Visible, BORDER_FRAME, TITLE_BLOCK…);
/// `ocgs` collects the layers in first-use order (`/OC<k>` = `ocgs[k]`).
/// The fonts a PDF's texts embed (Text: Selectable): each drawing font
/// face used, with the glyphs drawn in it (id → character, advance).
#[derive(Default)]
struct PdfFonts {
    faces: Vec<(noshape_sketch::text::EmbedFace, GlyphsUsed)>,
}

/// A character's code in an embedded font (live: its Unicode value; the
/// Basic Multilingual Plane's).
fn cid_of(c: char) -> u16 {
    u16::try_from(u32::from(c)).unwrap_or(0xFFFD)
}

/// Glyph id → (the character it draws, its advance in font units).
type GlyphsUsed = std::collections::BTreeMap<u16, (char, u16)>;

impl PdfFonts {
    fn index(&mut self, face: noshape_sketch::text::EmbedFace) -> usize {
        match self.faces.iter().position(|(f, _)| std::ptr::eq(f.data, face.data)) {
            Some(k) => k,
            None => {
                self.faces.push((face, Default::default()));
                self.faces.len() - 1
            }
        }
    }
}

/// A path's point in PDF points ("x y m" first, then "x y l"), 3 decimals:
/// formatted by hand (`{:.3}` took most of a second for the 750,000 points
/// of a big assembly drawing's text outlines).
fn path_point(out: &mut String, p: &[f64; 2], pt: f64, first: bool) {
    fn fixed3(out: &mut String, v: f64) {
        let n = (v * 1000.0).round();
        if !n.is_finite() || n.abs() >= 9e15 {
            let _ = write!(out, "{v:.3}");
            return;
        }
        if n < 0.0 || (n == 0.0 && v < 0.0) {
            out.push('-');
        }
        let a = n.abs() as u64;
        let _ = write!(out, "{}.{:03}", a / 1000, a % 1000);
    }
    fixed3(out, p[0] * pt);
    out.push(' ');
    fixed3(out, p[1] * pt);
    out.push_str(if first { " m\n" } else { " l\n" });
}

/// The extents [min, max] (sheet mm) of a sheet's lines, as live's DXF `$EXTMIN` / `$EXTMAX`; `None` on an empty
/// sheet.
pub(crate) fn line_extents(g: &SheetGeometry) -> Option<[[f64; 2]; 2]> {
    let mut points = g.lines.iter().flat_map(|l| l.points.iter());
    let first = *points.next()?;
    Some(points.fold([first, first], |[lo, hi], p| [[lo[0].min(p[0]), lo[1].min(p[1])], [hi[0].max(p[0]), hi[1].max(p[1])]]))
}

/// Where live's PDF puts a sheet (pt): its extents centred on the page at 1:1, as a plot of the extents (live
/// tpl-ANSI_A_INCH / ISO_A3: the frame 0.019 in / 0.5 mm left of the DXF's, the margins equal).
fn pdf_page_shift(g: &SheetGeometry) -> [f64; 2] {
    let Some([lo, hi]) = line_extents(g) else { return [0.0; 2] };
    [(g.width - lo[0] - hi[0]) / 2.0, (g.height - lo[1] - hi[1]) / 2.0]
}

fn polyline_length(l: &Polyline2) -> f64 {
    let close = l.closed.then(|| [*l.points.last().unwrap_or(&[0.0; 2]), *l.points.first().unwrap_or(&[0.0; 2])]);
    l.points.windows(2).map(|w| [w[0], w[1]]).chain(close).map(|[a, b]| (b[0] - a[0]).hypot(b[1] - a[1])).sum()
}

/// A line type laid along a line of length `len` as live's PDF does (AutoCAD's rule): whole periods, the rest split
/// between end dashes of at least half a dash (live 1857: a 25 mm centre mark arm ends in 6.21 mm dashes of a
/// 5.715 mm pattern; dx-linestyles: a 40 mm Dashed edge in 4.125 of 6.35). The dash array from the line's start;
/// `None` (solid) when one pattern doesn't fit.
fn centered_dashes(len: f64, pattern: &[f64]) -> Option<Vec<f64>> {
    let (&first, rest) = pattern.split_first()?;
    let period: f64 = pattern.iter().sum();
    let tail = period - first;
    if period.is_nan() || len.is_nan() || period <= 0.0 || first <= 0.0 {
        return None;
    }
    let k = ((len - tail - first) / period).floor();
    if k < 0.0 {
        return None;
    }
    let end = (len - tail - k * period) / 2.0;
    let mut d = vec![end];
    d.extend(rest);
    for _ in 0..k as usize {
        d.push(first);
        d.extend(rest);
    }
    d.extend([end, len]);
    Some(d)
}

/// A pen width (mm) as live's PDF writes it: in whole plot units of 1/600 in (live 1857: 0.40 mm 9 units, 0.15 mm
/// 4, 0.13 mm 3).
fn plot_width(mm: f64) -> f64 {
    (mm / 25.4 * 600.0).round().max(1.0) * 25.4 / 600.0
}

fn pdf_content(g: &SheetGeometry, ocgs: &mut Vec<super::cad_layers::CadLayer>, fonts: &mut PdfFonts) -> String {
    let pt = 72.0 / 25.4;
    let mut content = String::new();
    let mut cur: Option<super::cad_layers::CadLayer> = None;
    let mut layer = |content: &mut String, l: Option<super::cad_layers::CadLayer>| {
        if cur == l {
            return;
        }
        if cur.is_some() {
            content.push_str("EMC\n");
        }
        if let Some(l) = l {
            let k = ocgs.iter().position(|x| *x == l).unwrap_or_else(|| {
                ocgs.push(l);
                ocgs.len() - 1
            });
            let _ = writeln!(content, "/OC /OC{k} BDC");
        }
        cur = l;
    };
    content.push_str("1 J 1 j 0 0 0 RG 0 0 0 rg\n");
    let rgb = |c: Option<[u8; 3]>| {
        let c = c.unwrap_or([0, 0, 0]);
        format!("{:.3} {:.3} {:.3}", c[0] as f64 / 255.0, c[1] as f64 / 255.0, c[2] as f64 / 255.0)
    };
    for (pts, c) in &g.backdrops {
        let _ = writeln!(content, "{} rg", rgb(Some(*c)));
        for (i, p) in pts.iter().enumerate() {
            path_point(&mut content, p, pt, i == 0);
        }
        content.push_str("f\n");
    }
    for (li, l) in g.lines.iter().enumerate() {
        if l.points.len() < 2 {
            continue;
        }
        layer(&mut content, Some(g.cad_line_layer(li)));
        let _ = writeln!(content, "{} RG", rgb(g.line_color(li)));
        let (width_mm, dash) = g.line_pen(li);
        let (dash, phase) = match super::cad_export::pdf_dashes(g, li) {
            Some(p) => (centered_dashes(polyline_length(l), &p).unwrap_or_default(), 0.0),
            None => (dash, g.line_dash_phase(li)),
        };
        let _ = writeln!(content, "{:.3} w", plot_width(width_mm) * pt);
        if dash.is_empty() {
            for (i, p) in l.points.iter().enumerate() {
                path_point(&mut content, p, pt, i == 0);
            }
            content.push_str(if l.closed { "s\n" } else { "S\n" });
            continue;
        }
        // Live's PDF: a dashed line is its dashes, each a solid stroke.
        for piece in super::sheet::dash(&l.points, l.closed, &dash, phase) {
            for (i, p) in piece.iter().enumerate() {
                path_point(&mut content, p, pt, i == 0);
            }
            content.push_str("S\n");
        }
    }
    // Consecutive fills of one layer and colour (an exploded text's
    // triangles, arrowheads) are one path: the triangles don't overlap, so
    // the nonzero rule fills their union, and a renderer's antialiasing
    // sees each glyph whole (hayro darkened the slivers of an "N" drawn
    // one triangle at a time; the file is smaller too).
    let keys: Vec<_> = (0..g.fills.len()).map(|fi| (g.cad_fill_layer(fi), rgb(g.fill_color(fi)))).collect();
    for (fi, f) in g.fills.iter().enumerate() {
        let k = &keys[fi];
        if fi == 0 || keys[fi - 1] != *k {
            layer(&mut content, Some(k.0));
            let _ = writeln!(content, "{} rg", k.1);
        }
        for (i, p) in f.points.iter().enumerate() {
            path_point(&mut content, p, pt, i == 0);
        }
        if keys.get(fi + 1) != Some(k) {
            content.push_str("f\n");
        }
    }
    // Live outlines each filled arrowhead with a 1 plot unit stroke.
    if !g.outlined_fills.is_empty() {
        let _ = writeln!(content, "{:.3} w", plot_width(0.0) * pt);
        for &fi in &g.outlined_fills {
            let Some(f) = g.fills.get(fi).filter(|f| f.points.len() >= 3) else { continue };
            layer(&mut content, Some(g.cad_fill_layer(fi)));
            let _ = writeln!(content, "{} RG", rgb(g.fill_color(fi)));
            for (i, p) in f.points.iter().enumerate() {
                path_point(&mut content, p, pt, i == 0);
            }
            content.push_str("s\n");
        }
    }
    for (ti, t) in g.texts.iter().enumerate().filter(|(ti, t)| stroked(g, *ti, t)) {
        layer(&mut content, Some(g.cad_text_layer(ti)));
        let shx = g.text_font(ti).is_some_and(|f| super::sheet::is_shx(&f.family));
        if shx {
            let _ = writeln!(content, "{:.3} w {} RG", stroke_width(t) * pt, rgb(g.text_color(ti)));
            for st in font::text_strokes_font(t, g.text_font(ti)) {
                for (i, p) in st.iter().enumerate() {
                    path_point(&mut content, p, pt, i == 0);
                }
                content.push_str("S\n");
            }
            continue;
        }
        // A symbol our fonts have no outline for (GD&T ⌖ …): its strokes as
        // filled outlines, as live's PDF fills every glyph (Text Normal).
        let _ = writeln!(content, "{} rg", rgb(g.text_color(ti)));
        let r = stroke_width(t) / 2.0;
        for st in font::text_strokes_font(t, g.text_font(ti)) {
            for quad in stroke_quads(&st, r) {
                for (i, p) in quad.iter().enumerate() {
                    path_point(&mut content, p, pt, i == 0);
                }
                content.push_str("h\n");
            }
            content.push_str("f\n");
        }
    }
    // Empty title block cells draw nothing.
    for (ti, t) in g.texts.iter().enumerate().filter(|(ti, t)| !stroked(g, *ti, t) && !t.text.trim().is_empty()) {
        layer(&mut content, Some(g.cad_text_layer(ti)));
        let size = t.height * 1.4 * pt;
        let _ = writeln!(content, "{} rg", rgb(g.text_color(ti)));
        // In its drawing font, embedded: the glyphs where the outlines go.
        if let Some(run) = font::text_run(t, g.text_font(ti)) {
            use noshape_sketch::text::{SYNTH_BOLD, SYNTH_SLANT, drawing_glyphs};
            let lines = drawing_glyphs(&t.text, run.face.data);
            let (bold, slant) = (run.face.synth_bold, if run.face.synth_italic { SYNTH_SLANT } else { 0.0 });
            let k = fonts.index(run.face);
            let em = run.scale * fonts.faces[k].0.em * pt;
            let (c, sn) = (t.angle.cos(), t.angle.sin());
            let m = [em * run.sx * c, em * run.sx * sn, em * run.sx * slant * c - em * sn, em * run.sx * slant * sn + em * c];
            let tc = if bold { SYNTH_BOLD } else { 0.0 };
            for (glyphs, o) in lines.iter().zip(&run.origins) {
                if glyphs.is_empty() {
                    continue;
                }
                // Live's codes are the characters (CID = Unicode, its
                // CIDToGIDMap to the glyphs; samples/dx-pdf-selectable.pdf).
                let hex: String = glyphs.iter().map(|(_, ch, _)| format!("{:04X}", cid_of(*ch))).collect();
                for (gid, ch, adv) in glyphs {
                    fonts.faces[k].1.entry(*gid).or_insert((*ch, *adv));
                }
                // Synthesized bold: drawn again 0.03 em to the right.
                for shift in if bold { &[0.0, SYNTH_BOLD][..] } else { &[0.0][..] } {
                    let (e, f) = (o[0] * pt + shift * m[0], o[1] * pt + shift * m[1]);
                    let _ = writeln!(content, "BT /Type0TTF{k} 1 Tf {tc} Tc {:.5} {:.5} {:.5} {:.5} {e:.3} {f:.3} Tm <{hex}> Tj ET", m[0], m[1], m[2], m[3]);
                }
            }
            continue;
        }
        // Approximate Helvetica advance: 0.5 em per char.
        let width = t.text.chars().count() as f64 * size * 0.5;
        let (dx, dy) = match t.anchor {
            Anchor::BottomLeft => (0.0, 0.0),
            Anchor::BottomCenter => (-width / 2.0, 0.0),
            Anchor::Center => (-width / 2.0, -size * 0.35),
            Anchor::TopLeft => (0.0, -size * 0.75),
            Anchor::TopCenter => (-width / 2.0, -size * 0.75),
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
    layer(&mut content, None);
    // Translucent overlays over everything, each with its opacity's
    // graphics state (`/Ov<k>`, see [`pdf_overlay_states`]).
    for (k, (pts, c, _)) in g.overlays.iter().enumerate() {
        let _ = writeln!(content, "q /Ov{k} gs {} rg", rgb(Some(*c)));
        for (i, p) in pts.iter().enumerate() {
            path_point(&mut content, p, pt, i == 0);
        }
        content.push_str("f Q\n");
    }
    content
}

/// The page resource naming each overlay's opacity (`/ExtGState`), if any.
fn pdf_overlay_states(g: &SheetGeometry) -> String {
    if g.overlays.is_empty() {
        return String::new();
    }
    let states: String = g.overlays.iter().enumerate().map(|(k, (_, _, a))| format!(" /Ov{k} << /ca {a:.3} >>")).collect();
    format!(" /ExtGState <<{states} >>")
}

/// A PDF stream object, FlateDecode-compressed as live's exports (an A3
/// sheet's content was 680 KB plain): `dict` holds its other entries.
fn flate_stream(dict: &str, data: &[u8]) -> Vec<u8> {
    let z = zlib_parallel(data);
    let mut o = format!("<< {dict}/Length {} /Filter /FlateDecode >>\nstream\n", z.len()).into_bytes();
    o.extend_from_slice(&z);
    o.extend_from_slice(b"\nendstream");
    o
}

/// zlib data of `data` at level 6; a large one (a big assembly drawing's
/// 18 MB content: 0.8 s on one thread) as pigz does: 1 MB chunks deflated
/// in parallel, each ending on a byte boundary (sync flush) but the last,
/// joined under one header and the whole data's checksum.
fn zlib_parallel(data: &[u8]) -> Vec<u8> {
    use miniz_oxide::deflate::core::{CompressorOxide, TDEFLFlush, compress_to_output, create_comp_flags_from_zip_params};
    use rayon::prelude::*;
    const CHUNK: usize = 1 << 20;
    if data.len() <= 2 * CHUNK {
        return miniz_oxide::deflate::compress_to_vec_zlib(data, 6);
    }
    let chunks: Vec<&[u8]> = data.chunks(CHUNK).collect();
    let last = chunks.len() - 1;
    let parts: Vec<Vec<u8>> = chunks
        .par_iter()
        .enumerate()
        .map(|(i, c)| {
            // Raw deflate (negative window bits: no zlib header).
            let mut d = CompressorOxide::new(create_comp_flags_from_zip_params(6, -15, 0));
            let mut out = Vec::with_capacity(c.len() / 4);
            compress_to_output(&mut d, c, if i == last { TDEFLFlush::Finish } else { TDEFLFlush::Sync }, |b| {
                out.extend_from_slice(b);
                true
            });
            out
        })
        .collect();
    let (mut a, mut b) = (1u32, 0u32);
    for block in data.chunks(5552) {
        for &x in block {
            a += x as u32;
            b += a;
        }
        a %= 65521;
        b %= 65521;
    }
    let mut z = vec![0x78, 0x9c];
    for p in parts {
        z.extend_from_slice(&p);
    }
    z.extend_from_slice(&((b << 16) | a).to_be_bytes());
    z
}

/// The texts a PDF writes in its embedded fonts, read back (tests).
#[cfg(test)]
pub(crate) fn pdf_texts(pdf: &[u8]) -> Vec<String> {
    let d = pdf_decoded(pdf);
    // (The codes are the characters: /ToUnicode /Identity-H.)
    d.lines()
        .filter(|l| l.ends_with("> Tj ET"))
        .map(|l| {
            let hex = &l[l.rfind('<').unwrap_or(0) + 1..l.len() - 7];
            (0..hex.len() / 4).filter_map(|i| u32::from_str_radix(&hex[4 * i..4 * i + 4], 16).ok().and_then(char::from_u32)).collect()
        })
        .collect()
}

/// A PDF with its FlateDecode streams inflated (tests read the drawing
/// operators).
#[cfg(test)]
pub(crate) fn pdf_decoded(pdf: &[u8]) -> String {
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(k) = pdf[at..].windows(7).position(|w| w == b"stream\n") {
        let start = at + k + 7;
        out.extend_from_slice(&pdf[at..start]);
        let Some(len) = pdf[start..].windows(10).position(|w| w == b"\nendstream") else { break };
        let data = &pdf[start..start + len];
        let header = String::from_utf8_lossy(&pdf[at..start]);
        if header.rfind("/FlateDecode").is_some_and(|f| f > header.rfind("<<").unwrap_or(0)) {
            out.extend(miniz_oxide::inflate::decompress_to_vec_zlib(data).unwrap_or_default());
        } else {
            out.extend_from_slice(data);
        }
        out.extend_from_slice(b"\nendstream");
        at = start + len + 10;
    }
    out.extend_from_slice(&pdf[at..]);
    String::from_utf8_lossy(&out).into_owned()
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
    // The content first: it decides the layers (optional content groups),
    // shared by every page; their objects follow the images.
    let mut ocgs = Vec::new();
    let mut embedded = PdfFonts::default();
    let contents: Vec<String> = pages.iter().map(|g| pdf_content(g, &mut ocgs, &mut embedded)).collect();
    let n_images: usize = pages.iter().map(|g| g.images.iter().filter(|im| im.rgb.is_some()).count()).sum();
    let ocg0 = 4 + 2 * pages.len() + 11 + n_images;
    let ocg_ref = |k: usize| format!("{} 0 R", ocg0 + k);
    let oc_properties = if ocgs.is_empty() {
        String::new()
    } else {
        let all: Vec<String> = (0..ocgs.len()).map(ocg_ref).collect();
        // Live lists the layers by name in the viewer (ANNOTATION_TEXT, BORDER_FRAME, … Visible).
        let mut order: Vec<usize> = (0..ocgs.len()).collect();
        order.sort_by_key(|k| ocgs[*k].name(false));
        format!(" /OCProperties << /OCGs [{}] /D << /Order [{}] /ON [{}] >> >>", all.join(" "), order.iter().map(|k| ocg_ref(*k)).collect::<Vec<_>>().join(" "), all.join(" "))
    };
    let oc_resources = if ocgs.is_empty() { String::new() } else { format!(" /Properties <<{} >>", (0..ocgs.len()).map(|k| format!(" /OC{k} {}", ocg_ref(k))).collect::<String>()) };
    // The base-14 fonts only when a text uses one (a text in no drawing
    // font); else null objects keep the numbering (live's selectable file
    // has its one embedded font only).
    let base14 = contents.iter().any(|c| c.contains("BT /F"));
    let mut objects: Vec<Vec<u8>> = vec![
        format!("<< /Type /Catalog /Pages 2 0 R /PageLayout /SinglePage{oc_properties} >>").into_bytes(),
        format!("<< /Type /Pages /Kids [{}] /Count {} >>", (0..pages.len()).map(|i| format!("{} 0 R", 4 + 2 * i)).collect::<Vec<_>>().join(" "), pages.len()).into_bytes(),
        if base14 { b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>".to_vec() } else { b"null".to_vec() },
    ];
    // Fonts F2–F12 follow the pages, then the image XObjects (decoded RGB
    // supplied by the caller).
    let font0 = 4 + 2 * pages.len();
    // The embedded fonts' objects follow the layers (8 each, live's order:
    // Type0, CIDSystemInfo, DescendantFonts, CID font, W, descriptor,
    // CIDToGIDMap, file).
    let t0 = ocg0 + ocgs.len();
    let fonts: String = std::iter::once(" /F1 3 0 R".to_string())
        .chain((2..=12).map(|k| format!(" /F{k} {} 0 R", font0 + k - 2)))
        .filter(|_| base14)
        .chain((0..embedded.faces.len()).map(|k| format!(" /Type0TTF{k} {} 0 R", t0 + 8 * k)))
        .collect();
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
            images.push(flate_stream(&format!("/Type /XObject /Subtype /Image /Width {w} /Height {h} /ColorSpace /DeviceRGB /BitsPerComponent 8 "), rgb));
        }
        let shift = pdf_page_shift(g);
        let content = format!("1 0 0 1 {:.3} {:.3} cm\n", shift[0] * pt, shift[1] * pt) + &draw + contents[i].as_str();
        let (w, h) = (g.width * pt, g.height * pt);
        let xo = if xobjects.is_empty() { String::new() } else { format!(" /XObject <<{xobjects} >>") };
        let gs = pdf_overlay_states(g);
        objects.push(format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {w:.3} {h:.3}] /Resources << /Font <<{fonts} >>{xo}{gs}{oc_resources} >> /Contents {} 0 R >>", 5 + 2 * i).into_bytes());
        objects.push(flate_stream("", content.as_bytes()));
    }
    for name in &PDF_FONTS[1..] {
        objects.push(if base14 { format!("<< /Type /Font /Subtype /Type1 /BaseFont /{name} /Encoding /WinAnsiEncoding >>").into_bytes() } else { b"null".to_vec() });
    }
    objects.extend(images);
    for l in &ocgs {
        objects.push(format!("<< /Type /OCG /Name ({}) >>", l.name(false)).into_bytes());
    }
    for (k, (face, glyphs)) in embedded.faces.iter().enumerate() {
        let n = t0 + 8 * k;
        // A compact subset (live's: the drawn glyphs renumbered).
        let used: std::collections::BTreeMap<u16, char> = glyphs.iter().map(|(g, (c, _))| (*g, *c)).collect();
        let (file, gids) = super::pdf_font::subset(face.data, &used).unwrap_or_else(|| (face.data.to_vec(), glyphs.keys().map(|g| (*g, *g)).collect()));
        // Live's subsets are all tagged "AAAAAA".
        let name = format!("AAAAAA+{}", face.postscript_name);
        let u = |v: f64| (v * 1000.0 / face.units_per_em as f64).round() as i64;
        // By character code (CID): the subset's glyph, the width.
        let by_cid: std::collections::BTreeMap<u16, (u16, i64)> = glyphs.iter().map(|(g, (c, adv))| (cid_of(*c), (gids.get(g).copied().unwrap_or(*g), u(*adv as f64)))).collect();
        // The commonest width is the default (live's /DW 572: Noto Sans's
        // digits); /W lists the others in runs of codes.
        let mut counts: std::collections::BTreeMap<i64, usize> = std::collections::BTreeMap::new();
        for (_, w) in by_cid.values() {
            *counts.entry(*w).or_default() += 1;
        }
        let dw = counts.iter().max_by_key(|(w, n)| (**n, std::cmp::Reverse(**w))).map_or(1000, |(w, _)| *w);
        let mut widths = String::new();
        let mut last: Option<u16> = None;
        for (cid, (_, w)) in by_cid.iter().filter(|(_, (_, w))| *w != dw) {
            if last.is_some_and(|l| l + 1 == *cid) {
                widths.truncate(widths.len() - 2);
                let _ = write!(widths, "{w} ] ");
            } else {
                let _ = write!(widths, "{cid} [{w} ] ");
            }
            last = Some(*cid);
        }
        let b = face.bbox;
        objects.push(format!("<< /Type /Font /Subtype /Type0 /BaseFont /{name} /Encoding /Identity-H /ToUnicode /Identity-H /DescendantFonts {} 0 R >>", n + 2).into_bytes());
        objects.push(b"<< /Registry (Adobe) /Ordering (Identity) /Supplement 0 >>".to_vec());
        objects.push(format!("[{} 0 R ]", n + 3).into_bytes());
        objects.push(format!("<< /Type /Font /Subtype /CIDFontType2 /BaseFont /{name} /CIDSystemInfo {} 0 R /W {} 0 R /FontDescriptor {} 0 R /CIDToGIDMap {} 0 R /DW {dw} >>", n + 1, n + 4, n + 5, n + 6).into_bytes());
        objects.push(format!("[{widths}]").into_bytes());
        objects.push(
            format!(
                // Live's descriptor: the box's top and bottom as Ascent and Descent, no cap / x height or stem.
                "<< /Type /FontDescriptor /FontStretch /Normal /FontWeight {} /Flags 32 /FontBBox [{} {} {} {}] /ItalicAngle {:.6} /Ascent {}.000000 /Descent {}.000000 /CapHeight 0.000000 /XHeight 0.000000 /StemV 0.000000 /FontName /{name} /FontFamily ({}) /FontFile2 {} 0 R >>",
                if face.bold { 700 } else { 400 },
                u(b[0] as f64),
                u(b[1] as f64),
                u(b[2] as f64),
                u(b[3] as f64),
                face.italic_angle,
                u(b[3] as f64),
                u(b[1] as f64),
                face.family,
                n + 7
            )
            .into_bytes(),
        );
        let map: std::collections::BTreeMap<u16, u16> = by_cid.iter().map(|(c, (g, _))| (*c, *g)).collect();
        objects.push(flate_stream("", &super::pdf_font::cid_to_gid(&map)));
        objects.push(flate_stream(&format!("/Length1 {} ", file.len()), &file));
    }
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
    fn overlays_draw_over_the_ink_with_their_opacity() {
        let mut g = sample();
        g.overlays.push((vec![[10.0, 10.0], [60.0, 10.0], [60.0, 40.0]], [192, 192, 192], 0.25));
        // The colour mode leaves them as they are.
        let g = prepare(&g, &ExportOptions { color_mode: 2, ..Default::default() });
        assert_eq!(g.overlays[0].1, [192, 192, 192]);
        let svg = to_svg(&g);
        let at = svg.find(r#"fill-opacity="0.250""#).expect("overlay polygon");
        assert!(svg.rfind("<text").is_none_or(|t| t < at) && svg.rfind("<polyline").is_none_or(|l| l < at), "after the ink");
        let pdf = pdf_decoded(&to_pdf(&g));
        assert!(pdf.contains("/ExtGState << /Ov0 << /ca 0.250 >> >>"));
        assert!(pdf.find("/Ov0 gs").unwrap() > pdf.rfind(" Tj ET").unwrap(), "after the texts");
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

    /// PDF Text "Normal": a text's triangles are one filled path (not one
    /// fill each: hayro darkened an "N"'s slivers), per colour.
    #[test]
    fn a_pdf_fills_each_texts_triangles_as_one_path() {
        let mut g = sample();
        let m = g.mark();
        g.texts.push(Text2 { pos: [15.0, 15.0], height: 2.5, text: "NAME".into(), angle: 0.0, anchor: Anchor::Center, pick: 0 });
        g.color_since(m, [255, 0, 0]);
        let p = prepare(&g, &ExportOptions { explode_all: true, ..Default::default() });
        let red = (0..p.fills.len()).filter(|&i| p.fill_color(i) == Some([255, 0, 0])).count();
        assert!(red > 20, "{red} triangles");
        let content = pdf_decoded(&to_pdf(&p));
        let paths: Vec<&str> = content.split("1.000 0.000 0.000 rg\n").skip(1).collect();
        assert_eq!(paths.len(), 1, "one colour change for the red text");
        let path = paths[0].split("\nf\n").next().unwrap();
        assert_eq!(path.matches(" m\n").count(), red, "every triangle in that one path");
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
        assert!(pdf_decoded(&to_pdf(&g)).contains("1.000 0.000 0.000 RG"));
    }

    /// A table of many cells (a 1000-row BOM) exploded into fills keeps
    /// one span per run of equal colour, layer and font, not one per element
    /// (each lookup scans them: the PDF of live's 2000-instance assembly
    /// drawing took 31 s).
    #[test]
    fn prepare_keeps_spans_per_run() {
        let mut g = sample();
        let m = g.mark();
        for r in 0..300 {
            g.text([20.0, 20.0 + r as f64 * 0.1], 2.5, format!("B{r}-7"), Anchor::BottomLeft);
            g.line([10.0, r as f64], [12.0, r as f64], LineStyle::Thin);
        }
        g.color_since(m, [0, 0, 255]);
        for explode_all in [false, true] {
            let p = prepare(&g, &ExportOptions { explode_all, ..Default::default() });
            assert!(p.fills.len() + p.lines.len() + p.texts.len() > 600, "{explode_all}");
            for (what, n) in [("colors", p.colors.len()), ("pens", p.pens.len()), ("layers", p.layers.len()), ("cad", p.cad.len()), ("cad_members", p.cad_members.len()), ("fonts", p.fonts.len())] {
                assert!(n < 60, "explode_all {explode_all}: {n} {what} spans");
            }
            let last = if explode_all { p.fill_color(p.fills.len() - 1) } else { p.text_color(p.texts.len() - 1) };
            assert_eq!(last, Some([0, 0, 255]), "{explode_all}");
        }
    }

    /// Large streams are compressed in parallel chunks into one zlib
    /// stream that inflates to the data.
    #[test]
    fn large_streams_deflate_in_parallel_chunks() {
        let data: Vec<u8> = (0..5_000_000u32).flat_map(|i| format!("{}.{:03} {} l\n", i % 977, i % 1000, i % 13).into_bytes()).take(5_500_000).collect();
        let z = zlib_parallel(&data);
        assert!(miniz_oxide::inflate::decompress_to_vec_zlib(&z).unwrap() == data);
        let one = miniz_oxide::deflate::compress_to_vec_zlib(&data, 6).len();
        assert!(z.len() < one + one / 20, "{} vs {one}", z.len());
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

    /// PDF Text: Selectable (live: TrueType subsets of the drawing fonts):
    /// the texts are text in embedded Noto Sans, readable back through
    /// their ToUnicode map.
    #[test]
    fn selectable_pdf_text_embeds_the_drawing_font() {
        let p = to_pdf(&sample());
        let s = String::from_utf8_lossy(&p);
        let d = pdf_decoded(&p);
        // As live's (samples/tpl-ISO_A3.pdf): "AAAAAA+NotoSans", its descriptor's family and weight.
        assert!(s.contains("/Subtype /CIDFontType2") && s.contains("/BaseFont /AAAAAA+NotoSans /Encoding /Identity-H") && s.contains("/FontWeight 400 /Flags 32 /FontBBox [-556 -273 1321 1048]") && s.contains("/FontFamily (Noto Sans) /FontFile2"), "{s}");
        let base14: Vec<&str> = d.lines().filter(|l| l.starts_with("BT /F")).collect();
        assert!(base14.is_empty(), "no text left in a base-14 font: {base14:?}");
        // Live's encoding: the codes are the characters (/ToUnicode
        // /Identity-H), a CIDToGIDMap to the subset's glyphs, the commonest
        // width as /DW (Noto Sans's digits: 572), its objects apart.
        assert!(s.contains("/Encoding /Identity-H /ToUnicode /Identity-H /DescendantFonts") && s.contains("/CIDToGIDMap") && s.contains("/DW 572") && s.contains("<< /Registry (Adobe) /Ordering (Identity) /Supplement 0 >>"), "{s}");
        let texts = pdf_texts(&p);
        assert!(texts.iter().any(|t| t == "100"), "{texts:?}");
        assert!(p.len() < 120_000, "a subset, not the whole font: {} B", p.len());
    }

    #[test]
    fn line_types_are_laid_out_as_lives_pdf() {
        let center = [5.715, 1.143, 1.143, 1.143];
        let d = centered_dashes(25.0, &center).unwrap();
        assert_eq!(d.len(), 10);
        assert!((d[0] - 6.2135).abs() < 1e-3 && (d[8] - d[0]).abs() < 1e-9 && (d[4] - 5.715).abs() < 1e-9, "{d:?}");
        let d = centered_dashes(40.0, &[6.35, 3.175]).unwrap();
        assert!((d[0] - 4.125).abs() < 1e-9 && d.len() == 10, "{d:?}");
        assert!(centered_dashes(8.0, &center).is_none());
        // Live 2066: a 10 mm hidden edge, HIDDEN 0.1 / 0.05 in x 0.5: 0.8725, then four 1.27 mm dashes.
        let d = centered_dashes(10.0, &[1.27, 0.635]).unwrap();
        assert!((d[0] - 0.8725).abs() < 1e-9 && (d[2] - 1.27).abs() < 1e-9 && d.len() == 12, "{d:?}");
        assert!((plot_width(0.40) * 72.0 / 25.4 - 1.08).abs() < 1e-9 && (plot_width(0.15) * 72.0 / 25.4 - 0.48).abs() < 1e-9);
    }

    /// Live's exports of an empty ANSI A / ISO A3 template drawing (samples/tpl-*.dxf, .pdf): the DXF's extents are
    /// the zone ticks' ends; the PDF centres them on the page (the frame 0.019 in / 0.5 mm left of the DXF's).
    #[test]
    fn exports_place_the_sheet_as_live() {
        for (size, lo, hi, shift) in [
            (crate::drawing::SheetSize::AnsiA, [0.9652, 0.4826], [279.4, 215.4174], "1 0 0 1 -1.368 0.000 cm\n"),
            (crate::drawing::SheetSize::A3, [1.0, 0.5], [420.0, 296.5], "1 0 0 1 -1.417 0.000 cm\n"),
        ] {
            let sheet = Sheet { size, ..Default::default() };
            let (w, h) = size.size_mm();
            let mut g = SheetGeometry::new(w, h);
            g.add_border_and_title(&sheet);
            let [elo, ehi] = line_extents(&g).unwrap();
            let near = |a: [f64; 2], b: [f64; 2]| (a[0] - b[0]).abs() < 1e-3 && (a[1] - b[1]).abs() < 1e-3;
            assert!(near(elo, lo) && near(ehi, hi), "{size:?}: {elo:?} {ehi:?}");
            assert!(pdf_decoded(&to_pdf(&g)).contains(shift), "{size:?}");
            let dxf = String::from_utf8(crate::drawing::cad_export::to_dxf_version(&g, 4, acadrust::DxfVersion::AC1032).unwrap()).unwrap();
            let at = dxf.find("$EXTMIN").unwrap();
            let v: Vec<f64> = dxf[at..].lines().skip(2).step_by(2).take(2).map(|l| l.trim().parse().unwrap()).collect();
            assert!(near([v[0], v[1]], lo), "{size:?}: $EXTMIN {v:?}");
        }
    }

    #[test]
    fn pdf_has_valid_structure() {
        let p = to_pdf(&sample());
        let s = String::from_utf8_lossy(&p);
        // Layers as live: the export layers as optional content, by name.
        let d = pdf_decoded(&p);
        assert!(s.contains("/PageLayout /SinglePage /OCProperties << /OCGs [") && s.contains("/Type /OCG /Name (Visible)"), "{s}");
        assert!(d.contains("/OC /OC0 BDC") && d.matches(" BDC").count() == d.matches("EMC").count(), "{d}");
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
        // Text: Normal (outlines), to compare with the Selectable text above.
        let normal = super::prepare(&g, &super::ExportOptions { explode_all: true, ..Default::default() });
        std::fs::write("/tmp/out/sheet-normal.pdf", super::to_pdf(&normal)).unwrap();
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
