//! DWG import ("Insert DXF or DWG") against real files from several DWG
//! versions. Fixtures come from the LibreDWG test-data folder
//! (https://github.com/LibreDWG/libredwg/tree/master/test/test-data); see
//! `tests/fixtures/dwg/SOURCES` for the original names.

use glam::DVec2;
use noshape_sketch::dwg::{parse_cad, parse_dwg};
use noshape_sketch::dxf::{parse_dxf, DxfDrawing, DxfEntity};
use noshape_sketch::sketch::Sketch;
use noshape_sketch::Curve2;
use std::f64::consts::{PI, TAU};

fn fixture(name: &str) -> Vec<u8> {
    let path = format!("{}/tests/fixtures/dwg/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn dwg(name: &str) -> DxfDrawing {
    let bytes = fixture(name);
    let d = parse_dwg(&bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
    // the sniffing entry point picks the DWG reader too
    assert_eq!(parse_cad(&bytes).unwrap(), d);
    d
}

fn version(name: &str) -> String {
    String::from_utf8_lossy(&fixture(name)[..6]).into_owned()
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * (1.0 + a.abs().max(b.abs()))
}

fn close2(a: DVec2, b: DVec2) -> bool {
    close(a.x, b.x) && close(a.y, b.y)
}

/// Same entities (in order) up to floating point noise from DXF's decimal text.
fn assert_same(dwg: &DxfDrawing, dxf: &DxfDrawing) {
    assert_eq!(dwg.unit_scale, dxf.unit_scale);
    assert_eq!(dwg.entities.len(), dxf.entities.len(), "{dwg:?}\n{dxf:?}");
    for (a, b) in dwg.entities.iter().zip(&dxf.entities) {
        let ok = match (a, b) {
            (DxfEntity::Point(p), DxfEntity::Point(q)) => close2(*p, *q),
            (DxfEntity::Curve(Curve2::Line { a: a0, b: b0 }), DxfEntity::Curve(Curve2::Line { a: a1, b: b1 })) => close2(*a0, *a1) && close2(*b0, *b1),
            (DxfEntity::Curve(Curve2::Circle { center: c0, radius: r0 }), DxfEntity::Curve(Curve2::Circle { center: c1, radius: r1 })) => close2(*c0, *c1) && close(*r0, *r1),
            (
                DxfEntity::Curve(Curve2::Arc { center: c0, radius: r0, start_angle: s0, end_angle: e0 }),
                DxfEntity::Curve(Curve2::Arc { center: c1, radius: r1, start_angle: s1, end_angle: e1 }),
            ) => close2(*c0, *c1) && close(*r0, *r1) && close(*s0, *s1) && close(*e0, *e1),
            _ => false,
        };
        assert!(ok, "DWG {a:?} != DXF {b:?}");
    }
}

#[test]
fn r2000_entities_match_dxf_export() {
    assert_eq!(version("r2000_entities-2d.dwg"), "AC1015");
    let d = dwg("r2000_entities-2d.dwg");
    let x = parse_dxf(&String::from_utf8_lossy(&fixture("r2000_entities-2d.dxf"))).unwrap();
    assert_same(&d, &x);
    assert_eq!(d.unit_scale, Some(0.0254));
    assert_eq!(d.entities[0], DxfEntity::Point(DVec2::new(1.0, 2.0)));
    assert_eq!(d.entities[1], DxfEntity::Curve(Curve2::Line { a: DVec2::new(2.0, 3.0), b: DVec2::new(3.0, 4.0) }));
    assert_eq!(d.entities[2], DxfEntity::Curve(Curve2::Arc { center: DVec2::new(5.0, 5.0), radius: 1.0, start_angle: 1.5 * PI, end_angle: TAU }));
    assert_eq!(d.entities[3], DxfEntity::Curve(Curve2::Circle { center: DVec2::new(3.0, 1.0), radius: 1.0 }));
    // BLOCK1 (line (0,0)-(1,1)) inserted at (6,1), scale 0.5, rotated 30°;
    // the dimension's block is annotation and not imported.
    let (s30, c30) = (PI / 6.0).sin_cos();
    let DxfEntity::Curve(Curve2::Line { a, b }) = d.entities[4] else { panic!("{:?}", d.entities[4]) };
    assert!((a - DVec2::new(6.0, 1.0)).length() < 1e-9);
    assert!((b - DVec2::new(6.0 + 0.5 * (c30 - s30), 1.0 + 0.5 * (s30 + c30))).length() < 1e-9, "{b}");
    assert_eq!(d.entities.len(), 7);
}

#[test]
fn r2004_ellipse_and_spline() {
    assert_eq!(version("r2004_ellipse.dwg"), "AC1018");
    let d = dwg("r2004_ellipse.dwg");
    assert_eq!(d.unit_scale, Some(0.0254));
    let [DxfEntity::Curve(Curve2::Ellipse { center, major, minor })] = d.entities.as_slice() else { panic!("{d:?}") };
    assert!(close2(*center, DVec2::new(11.14325882289372, 9.537395074421863)));
    assert!(close2(*major, DVec2::new(-8.75080174866065, -8.034650640524557)));
    assert!(*minor > 0.0 && *minor < major.length() && close(*minor, 3.99294258186985));

    assert_eq!(version("r2004_spline.dwg"), "AC1018");
    let d = dwg("r2004_spline.dwg");
    let [DxfEntity::Curve(Curve2::Nurbs(n))] = d.entities.as_slice() else { panic!("{d:?}") };
    assert_eq!(n.degree, 2);
    assert_eq!(n.knots, vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0]);
    assert_eq!(n.ctrl.len(), 3);
    assert!(n.weights.is_none());
    assert!(n.is_valid());
    assert!(close2(n.ctrl[0], DVec2::new(28.059004438447758, 8.806772793885493)));
}

#[test]
fn r2007_bulged_polyline_matches_dxf_export() {
    // DONUT: a closed LWPOLYLINE of two half-circle bulge segments.
    assert_eq!(version("r2007_donut.dwg"), "AC1021");
    let d = dwg("r2007_donut.dwg");
    let x = parse_dxf(&String::from_utf8_lossy(&fixture("r2007_donut.dxf"))).unwrap();
    assert_same(&d, &x);
    assert_eq!(d.entities.len(), 2);
    let mut sweep = 0.0;
    for e in &d.entities {
        let DxfEntity::Curve(Curve2::Arc { center, radius, start_angle, end_angle }) = e else { panic!("{e:?}") };
        assert!(close2(*center, DVec2::new(25.846079911735316, 3.111665977501204)));
        assert!(close(*radius, 2.5));
        assert!(close(end_angle - start_angle, PI));
        sweep += end_angle - start_angle;
    }
    assert!(close(sweep, TAU));
    // the two halves join into one closed loop in the sketch
    let mut sk = Sketch::new();
    let ids = sk.insert_drawing(&d, d.unit_scale.unwrap(), DVec2::ZERO);
    assert_eq!(ids.len(), 2);
    let (s0, e0) = sk.endpoints(ids[0]).unwrap();
    let (s1, e1) = sk.endpoints(ids[1]).unwrap();
    assert!((s0 == e1 || s0 == s1) && (e0 == s1 || e0 == e1));
}

#[test]
fn r2010_open_polyline() {
    assert_eq!(version("r2010_polyline.dwg"), "AC1024");
    let d = dwg("r2010_polyline.dwg");
    assert_eq!(d.entities.len(), 4);
    let lines: Vec<(DVec2, DVec2)> = d
        .entities
        .iter()
        .map(|e| match e {
            DxfEntity::Curve(Curve2::Line { a, b }) => (*a, *b),
            other => panic!("{other:?}"),
        })
        .collect();
    // consecutive segments share vertices; the polyline is open
    for w in lines.windows(2) {
        assert_eq!(w[0].1, w[1].0);
    }
    assert_ne!(lines[3].1, lines[0].0);
    assert!(close2(lines[0].0, DVec2::new(9.005348630852552, 12.291278959468322)));
    assert!(close2(lines[3].1, DVec2::new(10.993230097486418, 2.2124385553025974)));
}

#[test]
fn r2013_arc() {
    assert_eq!(version("r2013_arc.dwg"), "AC1027");
    let d = dwg("r2013_arc.dwg");
    let [DxfEntity::Curve(Curve2::Arc { center, radius, start_angle, end_angle })] = d.entities.as_slice() else { panic!("{d:?}") };
    assert!(close2(*center, DVec2::new(-18.19331722931173, 6.686516981032791)));
    assert!(close(*radius, 8.293098252883262));
    assert!(close(*start_angle, 2.002420826591628));
    assert!(*end_angle > *start_angle && *end_angle - *start_angle < TAU);
}

#[test]
fn r2018_files() {
    assert_eq!(version("r2018_sample.dwg"), "AC1032");
    let d = dwg("r2018_sample.dwg");
    let x = parse_dxf(&String::from_utf8_lossy(&fixture("r2018_sample.dxf"))).unwrap();
    assert_same(&d, &x);
    assert_eq!(d.unit_scale, Some(0.001));
    let circles = d.entities.iter().filter(|e| matches!(e, DxfEntity::Curve(Curve2::Circle { radius, .. }) if close(*radius, 20.0))).count();
    let lines = d.entities.iter().filter(|e| matches!(e, DxfEntity::Curve(Curve2::Line { .. }))).count();
    assert_eq!((circles, lines, d.entities.len()), (1, 7, 8));
    // a closed 100 x 140 rectangle
    assert_eq!(d.entities[1], DxfEntity::Curve(Curve2::Line { a: DVec2::new(-50.3187372547813, -85.30424087608696), b: DVec2::new(49.6812627452187, -85.30424087608696) }));

    let d = dwg("r2018_point.dwg");
    assert!(matches!(d.entities.as_slice(), [DxfEntity::Point(p)] if close2(*p, DVec2::new(27.928010655312818, 8.653076880431954))));
    let d = dwg("r2018_circle.dwg");
    assert!(matches!(d.entities.as_slice(), [DxfEntity::Curve(Curve2::Circle { radius, .. })] if close(*radius, 13.530413845909695)));
}

#[test]
fn every_fixture_inserts_into_a_sketch() {
    for name in ["r2000_entities-2d.dwg", "r2004_ellipse.dwg", "r2004_spline.dwg", "r2007_donut.dwg", "r2010_polyline.dwg", "r2013_arc.dwg", "r2018_sample.dwg", "r2018_point.dwg", "r2018_circle.dwg"] {
        let d = dwg(name);
        let mut sk = Sketch::new();
        let ids = sk.insert_drawing(&d, d.unit_scale.unwrap_or(1.0), DVec2::ZERO);
        assert_eq!(ids.len(), d.entities.len(), "{name}");
    }
}

#[test]
fn rejects_bad_input() {
    // truncated / unsupported DWG
    let bytes = fixture("r2018_sample.dwg");
    assert!(parse_dwg(&bytes[..200]).is_err());
    assert!(parse_cad(&bytes[..200]).is_err());
    assert!(parse_dwg(b"AC1009\0\0\0\0").is_err());
    assert!(parse_dwg(b"0\nSECTION\n").is_err());
    // flipped bytes must not panic
    let mut bad = bytes.clone();
    for i in (300..bad.len()).step_by(97) {
        bad[i] ^= 0x5a;
    }
    let _ = parse_dwg(&bad);
    // DXF text goes through the DXF reader
    let d = parse_cad(&fixture("r2000_entities-2d.dxf")).unwrap();
    assert_eq!(d.entities.len(), 7);
}
