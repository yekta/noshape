//! Performance smoke test (ignored by default):
//! `cargo test -p noshape-kernel --release --test perf -- --ignored --nocapture`
use noshape_kernel::*;
use std::f64::consts::PI;
use std::time::Instant;

#[test]
#[ignore]
fn plate_with_400_filleted_holes() {
    let t0 = Instant::now();
    let plate = make_box(&Plane::xy(), 200.0, 200.0, 10.0).unwrap();
    let mut tools = Vec::new();
    for i in 0..20 {
        for j in 0..20 {
            let c = DVec3::new(5.0 + 10.0 * i as f64, 5.0 + 10.0 * j as f64, -1.0);
            tools.push(make_cylinder(&Plane::new(c, DVec3::Z, DVec3::X), 3.0, 12.0, 2.0 * PI).unwrap());
        }
    }
    let holed = boolean(BooleanOp::Cut, &[plate], &tools, &BooleanOptions::default()).unwrap().shape;
    let t_bool = t0.elapsed();
    // Top circular edges of the holes.
    let top: Vec<Shape> = holed
        .edges()
        .into_iter()
        .filter(|e| {
            let i = e.edge_info().unwrap();
            matches!(i.curve, Curve::Circle { .. }) && (i.start.z - 10.0).abs() < 1e-9
        })
        .collect();
    assert_eq!(top.len(), 400);
    let t1 = Instant::now();
    let filleted = fillet(&holed, &top, 0.8).unwrap();
    let t_fillet = t1.elapsed();
    let s = filleted.shape;
    let nf = s.faces().len();
    let t2 = Instant::now();
    let mesh = tessellate(&s, 0.05, 0.3, true).unwrap();
    let t_mesh = t2.elapsed();
    let t3 = Instant::now();
    let mesh2 = tessellate(&s, 0.05, 0.3, true).unwrap();
    let t_mesh_cached = t3.elapsed();
    let t4 = Instant::now();
    let bytes = s.to_bin_bytes(false).unwrap();
    let back = Shape::from_bin_bytes(&bytes).unwrap();
    let t_ser = t4.elapsed();
    let t5 = Instant::now();
    let v = s.volume().unwrap();
    let t_props = t5.elapsed();
    let t6 = Instant::now();
    let hlr = project_hlr(&s, DVec3::new(-1.0, -1.0, -1.0), DVec3::Z, false).unwrap();
    let t_hlr = t6.elapsed();
    println!("faces: {nf}, triangles: {}, vertices: {}", mesh.triangle_count(), mesh.positions.len());
    println!("boolean cut 400 tools: {t_bool:?}");
    println!("fillet 400 edges:      {t_fillet:?}");
    println!("tessellate:            {t_mesh:?}");
    println!("tessellate (reuse):    {t_mesh_cached:?}");
    println!("bin ser+deser ({} KB): {t_ser:?}", bytes.len() / 1024);
    println!("volume ({v:.1}):        {t_props:?}");
    println!("poly HLR ({} edges):   {t_hlr:?}", hlr.len());
    assert_eq!(mesh2.triangle_count(), mesh.triangle_count());
    assert_eq!(back.faces().len(), nf);
}

/// The user's knurling feature (lead 10-25) as its operations build it, timed
/// against live (behaviours "kern (10-25) knurl timing"): a Ø50 × 40 mm
/// cylinder, ANSI/ASME B94.6 circular pitch 12 TPI (74 grooves), tooth angle
/// 80°, groove depth 100 %, tooth helix angle 30° (0 for Straight). Each
/// groove is the diamond profile swept along the helix from the bottom
/// circle; the grooves are a circular pattern of it, Diamond adds their mirror
/// images; one subtraction. Live: Straight 0.75 s, 224 faces, 76702.187 mm³;
/// Diagonal 3.80 s, 224 faces, 76419.516 mm³; Diamond 32.05 s, 8142 faces,
/// 75405.795 mm³. `KNURL=straight|diagonal|diamond` runs one (default all).
#[test]
#[ignore]
fn knurl_like_onshape() {
    let (r, h, n) = (25.0, 40.0, 74usize);
    let depth = 0.034 * 25.4;
    let tooth = 80f64.to_radians();
    let only = std::env::var("KNURL").ok();
    for (kind, live_s, live_faces, live_volume) in [("straight", 0.75, 224, 76702.187), ("diagonal", 3.80, 224, 76419.516), ("diamond", 32.05, 8142, 75405.795)] {
        if only.as_deref().is_some_and(|k| k != kind) {
            continue;
        }
        let t0 = Instant::now();
        let helix_angle = if kind == "straight" { 0.0 } else { 30f64.to_radians() };
        // knurling.fs: the profile in the plane square to the axis at the
        // path's start, x along the circle, y outward.
        let cos_h = helix_angle.cos();
        let groove_half_base = depth * (tooth / 2.0).tan() / cos_h;
        let extra = 2.0 * r * (1.0 - (groove_half_base / r).asin().cos());
        let tooth_height = depth + extra;
        let half_base = tooth_height * (tooth / 2.0).tan() / cos_h;
        let p = |x: f64, y: f64| DVec3::new(r + y, -x, 0.0);
        let profile = make_face_from_wire(&make_polyline(&[p(half_base, extra), p(0.0, -depth), p(-half_base, extra), p(0.0, extra + tooth_height)], true).unwrap(), true).unwrap();
        let frame = Plane::new(DVec3::ZERO, DVec3::Z, DVec3::X);
        let path = if kind == "straight" { make_line(DVec3::new(r, 0.0, 0.0), DVec3::new(r, 0.0, h)).unwrap() } else { make_helix(&frame, r, 2.0 * r * PI * (PI / 2.0 - helix_angle).tan(), h, true, 0.0).unwrap() };
        // KNURL_SWEEP=frenet|discrete: the sweep's frame (default corrected Frenet).
        let mode = match std::env::var("KNURL_SWEEP").as_deref() {
            Ok("frenet") => SweepMode::Frenet,
            Ok("discrete") => SweepMode::Discrete,
            _ => SweepMode::CorrectedFrenet,
        };
        let groove = sweep(&profile, &path, &SweepOptions { mode, ..SweepOptions::default() }).unwrap().shape;
        if let Some(cap) = groove.faces().into_iter().filter_map(|f| f.face_plane().ok().flatten()).find(|p| p.origin.z > h * 0.5 || p.normal.z.abs() > 0.5 && p.origin.z > h * 0.5) {
            eprintln!("knurl {kind}: end cap normal {:?}", cap.normal);
        }
        let mut tools: Vec<Shape> = (0..n).map(|i| transform(&groove, &Transform::Rotate { origin: DVec3::ZERO, axis: DVec3::Z, angle: 2.0 * PI * i as f64 / n as f64 }, true).unwrap().shape).collect();
        if kind == "diamond" {
            let mirror = Plane::new(DVec3::new(r, 0.0, 0.0), DVec3::Y, DVec3::X);
            let mirrored: Vec<Shape> = tools.iter().map(|t| transform(t, &Transform::Mirror(mirror), true).unwrap().shape).collect();
            tools.extend(mirrored);
        }
        let t_tools = t0.elapsed();
        let cylinder = make_cylinder(&frame, r, h, 2.0 * PI).unwrap();
        // As the model does after a boolean: faces left on one surface merged.
        let res = boolean(BooleanOp::Cut, &[cylinder], &tools, &BooleanOptions::default()).and_then(|r| unify_same_domain(&r.shape));
        let t = t0.elapsed();
        match res {
            Ok(res) => {
                let (faces, volume) = (res.shape.count(ShapeType::Face), res.shape.volume().unwrap());
                eprintln!("knurl {kind}: {:.2} s (tools {:.2} s; live {live_s} s), {faces} faces (live {live_faces}), volume {volume:.3} mm3 (live {live_volume})", t.as_secs_f64(), t_tools.as_secs_f64());
                assert!((volume - live_volume).abs() < 5e-4 * live_volume, "knurl {kind}: volume {volume} vs live {live_volume}");
                assert_eq!(faces, live_faces, "knurl {kind}: faces");
            }
            Err(e) => panic!("knurl {kind}: failed after {:.2} s (live {live_s} s): {e}", t.as_secs_f64()),
        }
    }
}
