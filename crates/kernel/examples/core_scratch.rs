use glam::DVec3;
use noshape_kernel::*;
const MM: f64 = 1e-3;
fn vol(s: &Shape) -> f64 { volume_properties(s).unwrap().mass / (MM * MM * MM) }
fn edges_where(s: &Shape, f: impl Fn(&EdgeInfo) -> bool) -> Vec<Shape> {
    s.edges().into_iter().filter(|e| e.edge_info().map(|i| !i.degenerated && f(&i)).unwrap_or(false)).collect()
}
fn vert(s: &Shape) -> Vec<Shape> { edges_where(s, |i| (i.start.z - i.end.z).abs() > 1e-9 && (i.start.x - i.end.x).abs() < 1e-9 && (i.start.y - i.end.y).abs() < 1e-9) }
fn top(s: &Shape) -> Vec<Shape> { edges_where(s, |i| i.start.z > 9.99 * MM && i.end.z > 9.99 * MM) }
fn run(name: &str, want: f64, first_vert: bool, rv: f64, rt: f64, rust: bool) {
    set_rust_blend(Some(rust));
    let b = make_box_corners(DVec3::ZERO, DVec3::new(45.0, 20.0, 10.0) * MM).unwrap();
    let t = std::time::Instant::now();
    let r = if first_vert {
        let s = fillet_checked(&b, &vert(&b), &[rv * MM]).unwrap().shape;
        fillet_checked(&s, &top(&s), &[rt * MM]).map(|r| r.shape)
    } else {
        let s = fillet_checked(&b, &top(&b), &[rt * MM]).unwrap().shape;
        let v = vert(&s);
        fillet_checked(&s, &v, &[rv * MM]).map(|r| r.shape)
    };
    let ms = t.elapsed().as_secs_f64() * 1e3;
    match r {
        Ok(s) => {
            let mut kinds = std::collections::BTreeMap::new();
            for f in s.faces() { *kinds.entry(format!("{:?}", f.face_surface().ok().map(|s| format!("{s:?}").split(|c: char| !c.is_alphanumeric()).next().unwrap_or("").to_string()))).or_insert(0) += 1; }
            println!("{name} rust={rust}: vol {:.4} (live {want:.4}, d {:+.4}) valid {} faces {} {kinds:?} {ms:.0} ms", vol(&s), vol(&s) - want, s.is_valid(), s.faces().len());
        }
        Err(e) => println!("{name} rust={rust}: FAILED {e} {ms:.0} ms"),
    }
}
fn cyl(f: &Shape) -> bool { f.face_surface().map(|s| format!("{s:?}").starts_with("Cylinder")).unwrap_or(false) }
fn defeat() {
    let b = make_box_corners(DVec3::ZERO, DVec3::new(45.0, 20.0, 10.0) * MM).unwrap();
    let s = fillet_checked(&b, &top(&b), &[2.0 * MM]).unwrap().shape;
    let cyls: Vec<Shape> = s.faces().into_iter().filter(cyl).collect();
    println!("top2: {} faces, {} cylinders, vol {:.4}", s.faces().len(), cyls.len(), vol(&s));
    let t = std::time::Instant::now();
    match delete_faces(&s, &cyls) {
        Ok(r) => println!("all removed: vol {:.4} faces {} valid {} {:.0} ms", vol(&r.shape), r.shape.faces().len(), r.shape.is_valid(), t.elapsed().as_secs_f64() * 1e3),
        Err(e) => println!("all removed: {e}"),
    }
    // the two at the corner (45, 20)
    let two: Vec<Shape> = cyls.iter().filter(|f| { let bb = f.bounding_box(false).unwrap(); bb.max.x > 44.0 * MM && bb.max.y > 19.0 * MM && (bb.min.x > 40.0 * MM || bb.min.y > 15.0 * MM) }).cloned().collect();
    println!("{} at the corner", two.len());
    match delete_faces(&s, &two) {
        Ok(r) => println!("two removed: vol {:.4} faces {} valid {}", vol(&r.shape), r.shape.faces().len(), r.shape.is_valid()),
        Err(e) => println!("two removed: {e}"),
    }
}
fn main() {
    set_working_scale(1000.0).unwrap();
    defeat();
    for rust in [false, true] {
        run("a vert2 then top4", 8548.563923114056, true, 2.0, 4.0, rust);
        run("a top4 then vert2", 8548.563923114056, false, 2.0, 4.0, rust);
        run("b vert4 then top2", 8759.36625936351, true, 4.0, 2.0, rust);
        run("b top2 then vert4", 8759.36625936351, false, 4.0, 2.0, rust);
    }
}
