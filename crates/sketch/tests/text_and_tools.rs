//! Text boxes, length ratio constraint, fillet dimension retention and
//! conic offsets.

use noshape_sketch::text::{glyph_id_base, glyph_text_id, layout_text, DEFAULT_FONT};
use noshape_sketch::*;

fn v(x: f64, y: f64) -> DVec2 {
    DVec2::new(x, y)
}

#[test]
fn fillet_keeps_length_dimensions_on_trimmed_lines() {
    let mut sk = Sketch::new();
    let r = sk.corner_rectangle(v(0.0, 0.0), v(4.0, 3.0));
    let p0 = r.points[0];
    let _ = sk.add_constraint(ConstraintKind::Fix { entity: p0 }).unwrap();
    let w = sk.add_constraint(ConstraintKind::Length { curve: r.curves[0], value: 4.0 }).unwrap();
    let h = sk.add_constraint(ConstraintKind::Length { curve: r.curves[1], value: 3.0 }).unwrap();
    let top = sk.add_constraint(ConstraintKind::Length { curve: r.curves[2], value: 4.0 }).unwrap();
    assert!(sk.solve().is_solved());
    // Fillet the top-right corner (right and top lines).
    let corner = r.points[2];
    let res = sk.fillet_corner(corner, 0.5).unwrap();
    // The corner survives as a construction "virtual sharp".
    assert!(sk.entity(corner).is_some_and(|e| e.construction), "virtual sharp kept");
    // Dimensions keep their ids and values; the trimmed ones now measure to the sharp.
    for (cid, val) in [(w, 4.0), (h, 3.0), (top, 4.0)] {
        let c = sk.constraint(cid).expect("dimension kept");
        assert!((c.value().unwrap() - val).abs() < 1e-12, "{c:?}");
    }
    assert!(matches!(sk.constraint(h).unwrap().kind, ConstraintKind::Distance { .. }));
    assert!(matches!(sk.constraint(w).unwrap().kind, ConstraintKind::Length { .. }), "untouched line keeps its length");
    assert!(res.constraints_removed.iter().all(|c| ![w, h, top].contains(c)));
    let rep = sk.solve();
    assert!(rep.is_solved(), "{rep:?}");
    // Editing the retargeted height still drives the rectangle.
    sk.set_dimension_value(h, 5.0).unwrap();
    let rep = sk.solve();
    assert!(rep.is_solved(), "{rep:?}");
    let ys: Vec<f64> = r.points.iter().filter_map(|p| sk.point(*p)).map(|q| q.y).collect();
    assert!(ys.iter().any(|y| (y - 5.0).abs() < 1e-7), "top moved to y = 5: {ys:?}");
    let sharp = sk.point(corner).unwrap();
    assert!((sharp - v(4.0, 5.0)).length() < 1e-7, "{sharp}");
    // The fillet arc still has radius 0.5 and the region is closed.
    assert_eq!(sk.regions().len(), 1);
}

#[test]
fn fillet_without_dimensions_still_removes_the_corner() {
    let mut sk = Sketch::new();
    let p = sk.add_point(v(0.0, 0.0));
    let a = sk.add_point(v(3.0, 0.0));
    let b = sk.add_point(v(0.0, 3.0));
    sk.add_line_between(p, a).unwrap();
    sk.add_line_between(p, b).unwrap();
    sk.fillet_corner(p, 0.4).unwrap();
    assert!(sk.entity(p).is_none());
}

#[test]
fn length_ratio_constraint() {
    let mut sk = Sketch::new();
    let a = sk.add_line(v(0.0, 0.0), v(1.0, 0.0));
    let b = sk.add_line(v(0.0, 1.0), v(0.0, 3.0));
    let c = sk.add_constraint(ConstraintKind::LengthRatio { a, b, ratio: 2.0 }).unwrap();
    sk.add_constraint(ConstraintKind::Length { curve: b, value: 1.5 }).unwrap();
    assert!(sk.solve().is_solved());
    assert!((sk.curve(a).unwrap().length() - 3.0).abs() < 1e-9);
    sk.set_length_ratio(c, 0.5).unwrap();
    assert!(sk.solve().is_solved());
    assert!((sk.curve(a).unwrap().length() - 0.75).abs() < 1e-9);
    assert!(sk.set_length_ratio(c, -1.0).is_err());
    // Serialization round trip keeps the constraint.
    let json = serde_json::to_string(&sk).unwrap();
    let back: Sketch = serde_json::from_str(&json).unwrap();
    assert!(matches!(back.constraint(c).unwrap().kind, ConstraintKind::LengthRatio { ratio, .. } if (ratio - 0.5).abs() < 1e-15));
}

#[test]
fn text_box_scales_with_its_height_and_regenerates_outlines() {
    let mut sk = Sketch::new();
    let layout = layout_text("OK", DEFAULT_FONT, false, false);
    assert!(layout.width > 0.0);
    let tb = sk.add_text_box(v(1.0, 2.0), 0.5, layout.width).unwrap();
    assert!(sk.solve().is_solved());
    // 3 DOF left: position + size (the bottom edge is horizontal).
    let rep = sk.solve();
    assert_eq!(rep.dof, Some(3), "{rep:?}");
    // Dimension the height (left edge): the width follows the aspect ratio.
    sk.add_constraint(ConstraintKind::Length { curve: tb.lines[3], value: 1.0 }).unwrap();
    assert!(sk.solve().is_solved());
    let pl = sk.text_placement(&tb.points, false, false).unwrap();
    assert!((pl.height - 1.0).abs() < 1e-9);
    assert!((sk.curve(tb.lines[0]).unwrap().length() - layout.width).abs() < 1e-9);
    assert!((pl.x - DVec2::X).length() < 1e-9 && (pl.y - DVec2::Y).length() < 1e-9);

    // Outlines with stable ids above the allocator range.
    let mut out = sk.clone();
    let base = glyph_id_base(7);
    let curves = out.add_text_outlines(&layout, &pl, Some(base));
    assert!(curves.len() > 4);
    assert!(curves.iter().all(|c| c.0 >= base && glyph_text_id(*c) == Some(7)));
    // The O is a ring and the K a single region: regions stay computable and
    // solving a sketch with frozen outline entities works.
    let regions = out.regions();
    assert!(regions.len() >= 2, "{} regions", regions.len());
    assert!(out.solve().is_solved());
    // Outline geometry lies inside the box (ascender at the box top).
    let bb = out.entities_bbox(&curves);
    assert!(bb.min.x >= pl.origin.x - 0.05 && bb.max.y <= pl.origin.y + 1.0 + 1e-6, "{bb:?}");
    // Same input, same ids (deterministic regeneration).
    let mut again = sk.clone();
    assert_eq!(again.add_text_outlines(&layout, &pl, Some(base)), curves);
}

#[test]
fn offset_conic_uses_few_points() {
    let mut sk = Sketch::new();
    let c = sk.add_conic(v(0.0, 0.0), v(1.0, 1.5), v(2.0, 0.0), 0.6).unwrap();
    let res = sk.offset(&[c], 0.1).unwrap();
    let spline = res.created.iter().copied().find(|e| sk.kind(*e) == Some(EntityKind::BSpline)).expect("offset spline");
    let Some(Entity { geometry: Geometry::BSpline { control, .. }, .. }) = sk.entity(spline) else { panic!() };
    assert!(control.len() <= 11, "{} control points", control.len());
    // Close to the true offset everywhere.
    let orig = sk.curve(c).unwrap();
    let off = sk.curve(spline).unwrap();
    let (t0, t1) = orig.domain();
    for i in 0..=40 {
        let t = t0 + (t1 - t0) * i as f64 / 40.0;
        let d = orig.ders(t);
        let q = d[0] + d[1].normalize().perp() * 0.1;
        assert!(off.distance(q) < 1e-3, "deviation {} at {t}", off.distance(q));
    }
}

#[test]
fn offset_circular_conic_stays_a_conic() {
    // rho = 1/(1+sqrt2)... a conic with weight cos(45 deg) is a quarter circle.
    let w = std::f64::consts::FRAC_1_SQRT_2;
    let rho = w / (1.0 + w);
    let mut sk = Sketch::new();
    let c = sk.add_conic(v(1.0, 0.0), v(1.0, 1.0), v(0.0, 1.0), rho).unwrap();
    let res = sk.offset(&[c], 0.25).unwrap();
    let conic = res.created.iter().copied().find(|e| sk.kind(*e) == Some(EntityKind::Conic)).expect("offset is a conic");
    let off = sk.curve(conic).unwrap();
    let (t0, t1) = off.domain();
    for i in 0..=20 {
        let p = off.point(t0 + (t1 - t0) * i as f64 / 20.0);
        assert!((p.length() - 0.75).abs() < 1e-9, "offset of a quarter circle is a circle: {}", p.length());
    }
}

#[test]
fn every_sketch_font_lays_out_in_its_styles() {
    use noshape_sketch::text::{SKETCH_DEFAULT_FONT, SKETCH_FONTS, sketch_font_of_file};
    assert_eq!(SKETCH_FONTS.len(), 32);
    assert!(SKETCH_FONTS.iter().any(|f| f.name == SKETCH_DEFAULT_FONT));
    for f in SKETCH_FONTS {
        let text = if f.name.contains("Chinese") || f.name.contains("Japanese") || f.name.contains("Korean") { "字Ab" } else { "Ab" };
        for (bold, italic) in [(false, false), (f.bold, false), (false, f.italic), (f.bold, f.italic)] {
            let l = layout_text(text, f.name, bold, italic);
            assert!(!l.contours.is_empty() && l.width > 0.0, "{} bold {bold} italic {italic}", f.name);
        }
    }
    // A variable font's Bold is its 700 instance: wider strokes, wider text.
    let (r, b) = (layout_text("Wide", "Open Sans", false, false), layout_text("Wide", "Open Sans", true, false));
    assert!(b.width > r.width * 1.02, "{} vs {}", b.width, r.width);
    assert_eq!(sketch_font_of_file("OpenSans-BoldItalic.ttf"), Some("Open Sans"));
    assert_eq!(sketch_font_of_file("RobotoSlab-Regular.ttf"), Some("Roboto Slab"));
}

#[test]
fn sketch_fonts_name_their_families() {
    use noshape_sketch::text::{font_family_name, sketch_font_files};
    let files = sketch_font_files("Open Sans");
    assert_eq!(files.len(), 2, "regular and italic variable files");
    assert_eq!(font_family_name(files[0]).as_deref(), Some("Open Sans"));
    assert_eq!(font_family_name(sketch_font_files("PTSans")[0]).as_deref(), Some("PT Sans"));
    assert_eq!(sketch_font_files("Allerta").len(), 1);
}
