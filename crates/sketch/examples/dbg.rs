use noshape_sketch::*;
fn main() {
    let mut sk = Sketch::new();
    let s = sk.straight_slot(DVec2::new(0.0, 0.0), DVec2::new(4.0, 0.0), 2.0).unwrap();
    println!("{:?}", s.constraints);
    let r = sk.add_constraint(ConstraintKind::Tangent { a: s.curves[0], b: s.curves[1] });
    println!("{r:?}");
    for c in sk.constraints() { println!("{:?}", c.kind); }
    let rep = sk.solve();
    println!("{rep:?}");
}
