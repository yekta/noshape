//! Small procedural meshes used for overlays (planes, triad, view cube) and tests.

use crate::scene::{LineData, MeshData};

/// Axis-aligned box mesh with one pick id per face (ids `first_id..first_id+6`).
pub fn box_mesh(min: [f32; 3], max: [f32; 3], first_id: u32) -> MeshData {
    let mut m = MeshData::default();
    let [x0, y0, z0] = min;
    let [x1, y1, z1] = max;
    let faces: [([f32; 3], [[f32; 3]; 4]); 6] = [
        ([0.0, 0.0, -1.0], [[x0, y0, z0], [x0, y1, z0], [x1, y1, z0], [x1, y0, z0]]),
        ([0.0, 0.0, 1.0], [[x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1]]),
        ([0.0, -1.0, 0.0], [[x0, y0, z0], [x1, y0, z0], [x1, y0, z1], [x0, y0, z1]]),
        ([0.0, 1.0, 0.0], [[x0, y1, z0], [x0, y1, z1], [x1, y1, z1], [x1, y1, z0]]),
        ([-1.0, 0.0, 0.0], [[x0, y0, z0], [x0, y0, z1], [x0, y1, z1], [x0, y1, z0]]),
        ([1.0, 0.0, 0.0], [[x1, y0, z0], [x1, y1, z0], [x1, y1, z1], [x1, y0, z1]]),
    ];
    for (fi, (n, quad)) in faces.iter().enumerate() {
        let base = m.positions.len() as u32;
        for p in quad {
            m.positions.push(*p);
            m.normals.push(*n);
            m.pick_ids.push(first_id + fi as u32);
        }
        m.indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    m
}

/// The 12 edges of a box as line segments with pick ids `first_id..first_id+12`.
pub fn box_edges(min: [f32; 3], max: [f32; 3], first_id: u32, width: f32) -> LineData {
    let c = |i: u32| {
        [
            if i & 1 == 0 { min[0] } else { max[0] },
            if i & 2 == 0 { min[1] } else { max[1] },
            if i & 4 == 0 { min[2] } else { max[2] },
        ]
    };
    let pairs = [(0, 1), (2, 3), (4, 5), (6, 7), (0, 2), (1, 3), (4, 6), (5, 7), (0, 4), (1, 5), (2, 6), (3, 7)];
    let mut l = LineData::default();
    for (k, (a, b)) in pairs.iter().enumerate() {
        l.push_polyline(&[c(*a), c(*b)], [20, 20, 24, 255], first_id + k as u32, width, 0);
    }
    l
}

/// A flat square (both sides) used to display datum planes.
pub fn quad_mesh(center: [f32; 3], u: [f32; 3], v: [f32; 3], half: f32, pick_id: u32) -> MeshData {
    let c = glam::Vec3::from(center);
    let u = glam::Vec3::from(u) * half;
    let v = glam::Vec3::from(v) * half;
    let n = u.cross(v).normalize();
    let corners = [c - u - v, c + u - v, c + u + v, c - u + v];
    let mut m = MeshData::default();
    for p in corners {
        m.positions.push(p.into());
        m.normals.push(n.into());
        m.pick_ids.push(pick_id);
    }
    m.indices = vec![0, 1, 2, 0, 2, 3];
    m
}
