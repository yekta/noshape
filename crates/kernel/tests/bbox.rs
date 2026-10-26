//! Optimal bounding boxes are exact, as Onshape's (qa5-routing BBOX-TOL):
//! OCCT's box of a free-form face reached Precision::Confusion past the
//! short edges at its corner patches (the imported filleted block g47-1part:
//! z up to 10.0001 mm for 10).

use noshape_kernel::*;

#[test]
fn an_imported_filleted_block_has_its_exact_box() {
    let p = std::path::Path::new("/root/onshape-research/live/samples/g47-1part.step");
    if !p.is_file() {
        return;
    }
    let mm = 1e-3;
    for imp in read_step(p, LengthUnit::Meter).unwrap() {
        let b = imp.shape.bounding_box(true).unwrap();
        for (got, want) in [(b.min, DVec3::ZERO), (b.max, DVec3::new(40.0, 20.0, 10.0) * mm)] {
            assert!((got - want).abs().max_element() < 1e-12, "{:?} for {:?}", got / mm, want / mm);
        }
    }
}
