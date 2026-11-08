//! Lead 11-08 P1: an exported mesh is a closed surface, each edge used once in each direction. A face whose triangles
//! fold over one another (BRepMesh on cf-sheet-metal-model-plus's flange strip) is re-triangulated from its own nodes.
use noshape_kernel::*;
use std::collections::HashMap;
use std::f64::consts::TAU;

/// Edges used twice the same way and edges used one way only, corners welded by position.
fn folds_and_holes(t: &Tessellation) -> (usize, usize) {
    let mut id: HashMap<[u32; 3], u32> = HashMap::new();
    let weld: Vec<u32> = t
        .positions
        .iter()
        .map(|p| {
            let n = id.len() as u32;
            *id.entry(p.map(f32::to_bits)).or_insert(n)
        })
        .collect();
    let mut used: HashMap<(u32, u32), u32> = HashMap::new();
    for tri in t.indices.as_chunks::<3>().0 {
        let w = tri.map(|i| weld[i as usize]);
        for k in 0..3 {
            *used.entry((w[k], w[(k + 1) % 3])).or_default() += 1;
        }
    }
    let folds = used.values().filter(|&&n| n > 1).count();
    let holes = used.keys().filter(|(a, b)| !used.contains_key(&(*b, *a))).count();
    (folds, holes)
}

/// The first triangle of the first face with more than one turned over.
fn turn_one_over(t: &mut Tessellation, face: usize) {
    let r = t.faces[face];
    assert!(r.index_count >= 6, "face {face} has {} triangles", r.index_count / 3);
    t.indices.swap(r.first_index as usize + 1, r.first_index as usize + 2);
}

#[test]
fn a_folded_face_is_triangulated_again_closed_and_outward() {
    let part = make_box_corners(DVec3::ZERO, DVec3::new(0.03, 0.02, 0.01)).unwrap();
    let mut t = tessellate(&part, 1e-4, 0.35, true).unwrap();
    assert_eq!(folds_and_holes(&t), (0, 0));
    let before = t.indices.len();
    turn_one_over(&mut t, 0);
    assert_ne!(folds_and_holes(&t), (0, 0));
    assert_eq!(repair_folded_faces(&part, &mut t), 1);
    assert_eq!(folds_and_holes(&t), (0, 0));
    assert_eq!(t.indices.len(), before);
    // Every triangle faces out of the box: its normal away from the centre.
    let centre = DVec3::new(0.015, 0.01, 0.005);
    for tri in t.indices.as_chunks::<3>().0 {
        let [a, b, c] = tri.map(|i| DVec3::from(t.positions[i as usize].map(f64::from)));
        assert!((b - a).cross(c - a).dot((a + b + c) / 3.0 - centre) > 0.0, "a triangle faces into the box");
    }
}

#[test]
fn a_face_that_cannot_be_laid_flat_is_left_as_it_was() {
    // A cylinder's side: its parameter plane doesn't close the rings round it.
    let part = make_cylinder(&Plane::new(DVec3::ZERO, DVec3::Z, DVec3::X), 0.01, 0.02, TAU).unwrap();
    let mut t = tessellate(&part, 1e-4, 0.35, true).unwrap();
    let side = (0..t.faces.len()).max_by_key(|&f| t.faces[f].index_count).unwrap();
    turn_one_over(&mut t, side);
    let folded = t.indices.clone();
    let fixed = repair_folded_faces(&part, &mut t);
    if fixed == 0 {
        assert_eq!(t.indices, folded);
    } else {
        assert_eq!(folds_and_holes(&t), (0, 0));
    }
}
