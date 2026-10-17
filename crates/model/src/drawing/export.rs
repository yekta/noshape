//! Drawing export: SVG, DXF (AutoCAD R12 ASCII) and PDF (vector, built-in
//! Helvetica). All take a [`SheetGeometry`] in millimeters.

use super::sheet::{Anchor, LineStyle, SheetGeometry};
use std::fmt::Write as _;

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
    // Flip Y: sheet coordinates are Y-up.
    let _ = writeln!(s, r#"<g transform="translate(0 {}) scale(1 -1)" fill="none" stroke="black" stroke-linecap="round" stroke-linejoin="round">"#, g.height);
    for l in &g.lines {
        if l.points.len() < 2 {
            continue;
        }
        let pts: Vec<String> = l.points.iter().map(|p| format!("{:.4},{:.4}", p[0], p[1])).collect();
        let tag = if l.closed { "polygon" } else { "polyline" };
        let dash = l.style.dashes();
        let dash_attr = if dash.is_empty() {
            String::new()
        } else {
            format!(r#" stroke-dasharray="{}""#, dash.iter().map(|d| d.to_string()).collect::<Vec<_>>().join(" "))
        };
        let _ = writeln!(s, r#"<{tag} points="{}" stroke-width="{}"{dash_attr}/>"#, pts.join(" "), l.style.width_mm());
    }
    for f in &g.fills {
        let pts: Vec<String> = f.points.iter().map(|p| format!("{:.4},{:.4}", p[0], p[1])).collect();
        let _ = writeln!(s, r#"<polygon points="{}" fill="black" stroke="none"/>"#, pts.join(" "));
    }
    let _ = writeln!(s, "</g>");
    for t in &g.texts {
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
            r#"<text x="{x:.4}" y="{y:.4}" font-family="Helvetica, Arial, sans-serif" font-size="{:.3}" text-anchor="{anchor}"{baseline} transform="rotate({:.3} {x:.4} {y:.4})">{}</text>"#,
            t.height * 1.4,
            -t.angle.to_degrees(),
            esc_xml(&t.text)
        );
    }
    let _ = writeln!(s, "</svg>");
    s
}

/// DXF R12 (AC1009): LINE/POLYLINE/TEXT/SOLID entities on per-style layers, in mm.
pub fn to_dxf(g: &SheetGeometry) -> String {
    let mut s = String::new();
    let pair = |s: &mut String, code: i32, v: &str| {
        let _ = writeln!(s, "{code:>3}\n{v}");
    };
    pair(&mut s, 0, "SECTION");
    pair(&mut s, 2, "HEADER");
    pair(&mut s, 9, "$ACADVER");
    pair(&mut s, 1, "AC1009");
    pair(&mut s, 9, "$INSUNITS");
    pair(&mut s, 70, "4");
    pair(&mut s, 0, "ENDSEC");
    // Tables: line types and layers.
    pair(&mut s, 0, "SECTION");
    pair(&mut s, 2, "TABLES");
    pair(&mut s, 0, "TABLE");
    pair(&mut s, 2, "LTYPE");
    pair(&mut s, 70, "3");
    for (name, pattern) in [("CONTINUOUS", &[][..]), ("HIDDEN", &[3.0, -1.5][..]), ("CENTER", &[8.0, -1.5, 1.5, -1.5][..])] {
        pair(&mut s, 0, "LTYPE");
        pair(&mut s, 2, name);
        pair(&mut s, 70, "0");
        pair(&mut s, 3, name);
        pair(&mut s, 72, "65");
        pair(&mut s, 73, &pattern.len().to_string());
        let total: f64 = pattern.iter().map(|x: &f64| x.abs()).sum();
        pair(&mut s, 40, &format!("{total}"));
        for p in pattern {
            pair(&mut s, 49, &format!("{p}"));
        }
    }
    pair(&mut s, 0, "ENDTAB");
    let styles = [
        LineStyle::Visible,
        LineStyle::Hidden,
        LineStyle::Tangent,
        LineStyle::Centerline,
        LineStyle::Thin,
        LineStyle::Border,
        LineStyle::Section,
    ];
    pair(&mut s, 0, "TABLE");
    pair(&mut s, 2, "LAYER");
    pair(&mut s, 70, &styles.len().to_string());
    for st in styles {
        pair(&mut s, 0, "LAYER");
        pair(&mut s, 2, st.layer());
        pair(&mut s, 70, "0");
        pair(&mut s, 62, "7");
        let lt = match st {
            LineStyle::Hidden => "HIDDEN",
            LineStyle::Centerline | LineStyle::Section => "CENTER",
            _ => "CONTINUOUS",
        };
        pair(&mut s, 6, lt);
    }
    pair(&mut s, 0, "ENDTAB");
    pair(&mut s, 0, "ENDSEC");

    pair(&mut s, 0, "SECTION");
    pair(&mut s, 2, "ENTITIES");
    for l in &g.lines {
        if l.points.len() == 2 && !l.closed {
            pair(&mut s, 0, "LINE");
            pair(&mut s, 8, l.style.layer());
            pair(&mut s, 10, &format!("{:.6}", l.points[0][0]));
            pair(&mut s, 20, &format!("{:.6}", l.points[0][1]));
            pair(&mut s, 11, &format!("{:.6}", l.points[1][0]));
            pair(&mut s, 21, &format!("{:.6}", l.points[1][1]));
        } else if l.points.len() >= 2 {
            pair(&mut s, 0, "POLYLINE");
            pair(&mut s, 8, l.style.layer());
            pair(&mut s, 66, "1");
            pair(&mut s, 70, if l.closed { "1" } else { "0" });
            for p in &l.points {
                pair(&mut s, 0, "VERTEX");
                pair(&mut s, 8, l.style.layer());
                pair(&mut s, 10, &format!("{:.6}", p[0]));
                pair(&mut s, 20, &format!("{:.6}", p[1]));
            }
            pair(&mut s, 0, "SEQEND");
        }
    }
    for f in &g.fills {
        if f.points.len() >= 3 {
            pair(&mut s, 0, "SOLID");
            pair(&mut s, 8, "ANNOTATION");
            let p = &f.points;
            let pts = [p[0], p[1], p[2], p[2]];
            for (i, q) in pts.iter().enumerate() {
                pair(&mut s, 10 + i as i32, &format!("{:.6}", q[0]));
                pair(&mut s, 20 + i as i32, &format!("{:.6}", q[1]));
            }
        }
    }
    for t in &g.texts {
        pair(&mut s, 0, "TEXT");
        pair(&mut s, 8, "ANNOTATION");
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

/// Single-page vector PDF.
pub fn to_pdf(g: &SheetGeometry) -> Vec<u8> {
    let pt = 72.0 / 25.4;
    let mut content = String::new();
    content.push_str("1 J 1 j 0 0 0 RG 0 0 0 rg\n");
    for l in &g.lines {
        if l.points.len() < 2 {
            continue;
        }
        let dash = l.style.dashes();
        if dash.is_empty() {
            content.push_str("[] 0 d\n");
        } else {
            let d: Vec<String> = dash.iter().map(|x| format!("{:.3}", x * pt)).collect();
            let _ = writeln!(content, "[{}] 0 d", d.join(" "));
        }
        let _ = writeln!(content, "{:.3} w", l.style.width_mm() * pt);
        for (i, p) in l.points.iter().enumerate() {
            let _ = writeln!(content, "{:.3} {:.3} {}", p[0] * pt, p[1] * pt, if i == 0 { "m" } else { "l" });
        }
        content.push_str(if l.closed { "s\n" } else { "S\n" });
    }
    content.push_str("[] 0 d\n");
    for f in &g.fills {
        for (i, p) in f.points.iter().enumerate() {
            let _ = writeln!(content, "{:.3} {:.3} {}", p[0] * pt, p[1] * pt, if i == 0 { "m" } else { "l" });
        }
        content.push_str("f\n");
    }
    for t in &g.texts {
        let size = t.height * 1.4 * pt;
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
        let _ = writeln!(content, "BT /F1 {size:.3} Tf {c:.5} {s:.5} {:.5} {c:.5} {x:.3} {y:.3} Tm ({text}) Tj ET", -s);
    }

    let (w, h) = (g.width * pt, g.height * pt);
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {w:.3} {h:.3}] /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>"),
        format!("<< /Length {} >>\nstream\n{}endstream", content.len(), content),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>".to_string(),
    ];
    let mut out: Vec<u8> = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets = Vec::new();
    for (i, o) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{}\nendobj\n", i + 1, o).as_bytes());
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
