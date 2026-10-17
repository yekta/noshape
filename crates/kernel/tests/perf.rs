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
