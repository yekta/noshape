//! Interference detection (Onshape: Analysis tools ▸ Interference
//! detection, Part Studio): the material shared by each pair of the chosen
//! parts, with its volume, bounding box and mesh for drawing in red.

use glam::DVec3;
use noshape_kernel::Shape;

/// One interference between two parts (indices into the input list).
#[derive(Clone, Debug)]
pub struct Interference {
    pub a: usize,
    pub b: usize,
    pub volume: f64,
    pub bounds: (DVec3, DVec3),
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
}

/// Every pair of `shapes` whose solids overlap by more than a sliver.
pub fn interferences(shapes: &[Shape]) -> Vec<Interference> {
    let mut out = Vec::new();
    for a in 0..shapes.len() {
        for b in a + 1..shapes.len() {
            let (Ok(ba), Ok(bb)) = (shapes[a].bounding_box(false), shapes[b].bounding_box(false)) else { continue };
            // Boxes apart: nothing to compute.
            if ba.min.cmpgt(bb.max).any() || bb.min.cmpgt(ba.max).any() {
                continue;
            }
            let Ok(common) = noshape_kernel::common(&shapes[a], &shapes[b]) else { continue };
            let shape = common.shape;
            let Ok(volume) = shape.volume() else { continue };
            let size = (ba.max - ba.min).length().min((bb.max - bb.min).length()).max(1e-12);
            if volume <= size.powi(3) * 1e-9 {
                continue;
            }
            // The tight box (qa5 AN4: the loose one read 35.485 x 20.233 x 10 for 30 x 20 x 10).
            let Ok(bx) = shape.bounding_box(true) else { continue };
            let Ok(mesh) = noshape_kernel::tessellate(&shape, size * 2e-3, 0.3, false) else { continue };
            out.push(Interference { a, b, volume, bounds: (bx.min, bx.max), positions: mesh.positions, normals: mesh.normals, indices: mesh.indices });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlapping_boxes_interfere_and_apart_ones_dont() {
        let b = |lo: [f64; 3], hi: [f64; 3]| noshape_kernel::make_box_corners(DVec3::from(lo), DVec3::from(hi)).unwrap();
        let shapes = [b([0.0; 3], [2.0, 2.0, 2.0]), b([1.0, 1.0, 1.0], [3.0, 3.0, 3.0]), b([10.0; 3], [11.0; 3])];
        let found = interferences(&shapes);
        assert_eq!(found.len(), 1);
        let i = &found[0];
        assert_eq!((i.a, i.b), (0, 1));
        assert!((i.volume - 1.0).abs() < 1e-9, "{}", i.volume);
        assert!(i.bounds.0.distance(DVec3::ONE) < 1e-6 && i.bounds.1.distance(DVec3::splat(2.0)) < 1e-6);
        assert!(!i.indices.is_empty());
    }

    /// qa5 AN4: the size shown is the overlap's tight box (a half cylinder
    /// here: 1 × 2 × 2), not OCCT's loose one.
    #[test]
    fn interference_bounds_are_tight() {
        let frame = noshape_kernel::Plane::new(DVec3::ZERO, DVec3::Z, DVec3::X);
        let cyl = noshape_kernel::make_cylinder(&frame, 1.0, 2.0, std::f64::consts::TAU).unwrap();
        let half = noshape_kernel::make_box_corners(DVec3::new(0.0, -2.0, 0.0), DVec3::new(2.0, 2.0, 2.0)).unwrap();
        let found = interferences(&[cyl, half]);
        assert_eq!(found.len(), 1);
        let (lo, hi) = found[0].bounds;
        assert!(lo.distance(DVec3::new(0.0, -1.0, 0.0)) < 1e-6 && hi.distance(DVec3::new(1.0, 1.0, 2.0)) < 1e-6, "{lo} {hi}");
    }
}
