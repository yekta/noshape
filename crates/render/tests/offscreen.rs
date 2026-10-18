use noshape_render::*;

#[test]
fn renders_box_and_picks_faces() {
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let mesh = r.upload_mesh(&primitives::box_mesh([-0.01, -0.01, -0.01], [0.01, 0.01, 0.01], 1));
    let edges = r.upload_lines(&primitives::box_edges([-0.01, -0.01, -0.01], [0.01, 0.01, 0.01], 10, 1.5));
    let mut scene = Scene::default();
    scene.instances.push(Instance::new(Drawable::Mesh(mesh)));
    scene.instances.push(Instance::new(Drawable::Lines(edges)).with_color([0.02, 0.02, 0.03, 1.0]));
    scene.set_state(2, state::SELECTED);
    let mut cam = Camera::default();
    cam.fit_sphere(glam::DVec3::ZERO, 0.0175, 4.0 / 3.0);
    let style = Style::default();
    let f = r
        .render(RenderParams { scene: &scene, camera: &cam, width: 400, height: 300, device_pixel_ratio: 1.0, style: &style, pick: true })
        .unwrap();
    assert_eq!(f.bgra.len(), 400 * 300 * 4);
    let pick = f.pick.unwrap();
    let center = pick.ids[(150 * 400 + 200) as usize];
    assert!(center != 0, "center pixel should hit the box");
    if let Ok(dir) = std::env::var("NOSHAPE_TEST_OUT") {
        let mut rgba = f.bgra.clone();
        for px in rgba.as_chunks_mut::<4>().0 {
            px.swap(0, 2);
        }
        image::save_buffer(format!("{dir}/box.png"), &rgba, 400, 300, image::ExtendedColorType::Rgba8).unwrap();
    }
}

#[test]
fn pipelined_frames_and_lazy_pick_regions() {
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let mesh = r.upload_mesh(&primitives::box_mesh([-0.01, -0.01, -0.01], [0.01, 0.01, 0.01], 1));
    let mut scene = Scene::default();
    scene.instances.push(Instance::new(Drawable::Mesh(mesh)));
    let scene = std::sync::Arc::new(scene);
    let mut cam = Camera::default();
    cam.fit_sphere(glam::DVec3::ZERO, 0.0175, 4.0 / 3.0);
    let req = |seq: u64| FrameRequest { scene: scene.clone(), camera: cam, width: 400, height: 300, device_pixel_ratio: 1.0, style: Style::default(), invalidate_pick: seq == 1 };
    // As many frames in flight as the adapter allows, then a pick query.
    let n = r.frames_in_flight() as u64;
    for seq in 1..=n {
        assert!(r.can_submit(7));
        r.submit_frame(7, seq, req(seq)).unwrap();
    }
    assert!(!r.can_submit(7));
    r.submit_picks(7, &[(11, PickRect::around(200, 150, 5)), (12, PickRect { x: -10, y: -10, width: 20, height: 20 })]);
    let mut frames = Vec::new();
    let mut picks = Vec::new();
    while r.has_pending() {
        r.wait_oldest();
        for c in r.collect() {
            match c {
                Completed::Frame { target, seq, result } => frames.push((target, seq, result.unwrap())),
                Completed::Pick { token, result, .. } => picks.push((token, result.unwrap())),
            }
        }
    }
    assert_eq!(frames.iter().map(|f| f.1).collect::<Vec<_>>(), (1..=n).collect::<Vec<_>>());
    assert!(frames.iter().all(|f| f.2.bgra.len() == 400 * 300 * 4));
    assert!(r.can_submit(7));
    let center = picks.iter().find(|p| p.0 == 11).unwrap();
    assert_eq!((center.1.width, center.1.height), (11, 11));
    assert!(center.1.covers(200, 150, 5));
    assert!(center.1.pick(200, 150, 5, |_| 0).is_some(), "center should hit the box");
    // The corner query is clipped to the frame and sees only background.
    let corner = picks.iter().find(|p| p.0 == 12).unwrap();
    assert_eq!((corner.1.x0, corner.1.y0, corner.1.width, corner.1.height), (0, 0, 10, 10));
    assert!(corner.1.ids.iter().all(|&id| id == 0));
}

/// A fan-triangulated disk (triangulation differs from the box face's).
fn disk_mesh(center: [f32; 3], radius: f32, z_sign: f32, pick_id: u32) -> MeshData {
    let mut m = MeshData::default();
    let n = 48;
    m.positions.push(center);
    for k in 0..n {
        let a = k as f32 / n as f32 * std::f32::consts::TAU;
        m.positions.push([center[0] + radius * a.cos(), center[1] + radius * a.sin(), center[2]]);
    }
    m.normals = vec![[0.0, 0.0, z_sign]; m.positions.len()];
    m.pick_ids = vec![pick_id; m.positions.len()];
    for k in 0..n {
        m.indices.extend_from_slice(&[0, 1 + k, 1 + (k + 1) % n]);
    }
    m
}

fn iso_camera() -> Camera {
    let mut cam = Camera::default();
    cam.fit_sphere(glam::DVec3::ZERO, 0.0175, 4.0 / 3.0);
    cam
}

/// Regression: a sketch region drawn on a part face (pick-only, same depth,
/// different triangulation) must win the pick against the face; a region
/// hidden behind the part must not.
#[test]
fn pick_only_region_on_face_wins_the_pick() {
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let (lo, hi) = ([-0.01, -0.01, -0.01], [0.01, 0.01, 0.01]);
    let mut scene = Scene::default();
    scene.instances.push(Instance::new(Drawable::Mesh(r.upload_mesh(&primitives::box_mesh(lo, hi, 1)))));
    // On the visible top face (+Z) and on the hidden bottom face (-Z).
    let top = r.upload_mesh(&disk_mesh([0.002, -0.001, 0.01], 0.006, 1.0, 100));
    let bottom = r.upload_mesh(&disk_mesh([0.0, 0.0, -0.01], 0.006, -1.0, 200));
    scene.instances.push(Instance::new(Drawable::Mesh(top)).with_color([0.0; 4]).pick_only().layer(9));
    scene.instances.push(Instance::new(Drawable::Mesh(bottom)).with_color([0.0; 4]).pick_only().layer(9));
    let cam = iso_camera();
    let (w, h) = (400u32, 300u32);
    let f = r
        .render(RenderParams { scene: &scene, camera: &cam, width: w, height: h, device_pixel_ratio: 1.0, style: &Style::default(), pick: true })
        .unwrap();
    let pick = f.pick.unwrap();
    let vp = glam::DVec2::new(w as f64, h as f64);
    let at = |p: glam::DVec3| {
        let s = cam.project(p, vp, 0.02).unwrap();
        pick.id_at(s.x as i32, s.y as i32).unwrap()
    };
    let (mut hits, mut total) = (0, 0);
    for i in 0..10 {
        for j in 0..10 {
            let (u, v) = (i as f64 / 9.0 * 2.0 - 1.0, j as f64 / 9.0 * 2.0 - 1.0);
            if u * u + v * v > 0.8 {
                continue;
            }
            total += 1;
            hits += (at(glam::DVec3::new(0.002 + u * 0.006, -0.001 + v * 0.006, 0.01)) == 100) as usize;
        }
    }
    assert_eq!(hits, total, "region on the top face must win every pixel");
    // The bottom region is behind the part: the faces in front win.
    for p in [glam::DVec3::new(0.0, 0.0, -0.01), glam::DVec3::new(0.003, 0.003, -0.01)] {
        assert_ne!(at(p), 200, "region behind the part must not be picked");
    }
    // Outside the region the face itself is picked (+Z face has id 2).
    assert_eq!(at(glam::DVec3::new(-0.008, 0.008, 0.01)), 2);
}

/// Regression: edges fought faces at the same depth and looked dotted on
/// silhouettes, bottom faces and hole rims. Every visible edge of a shaded
/// plate in iso view (app-like scene: large default planes, wide viewport)
/// must be drawn solid.
#[test]
fn plate_edges_are_drawn_solid_over_faces() {
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let (lo, hi) = ([-0.05f32, -0.03, 0.0], [0.05f32, 0.03, 0.01]);
    let mut scene = Scene::default();
    scene.instances.push(Instance::new(Drawable::Mesh(r.upload_mesh(&primitives::box_mesh(lo, hi, 1)))).with_color(srgb(0x9a, 0xb4, 0xcc)));
    scene.instances.push(Instance::new(Drawable::Lines(r.upload_lines(&primitives::box_edges(lo, hi, 10, 1.5)))).per_primitive_color().layer(1));
    for (u, v) in [([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]), ([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]), ([0.0, 1.0, 0.0], [0.0, 0.0, 1.0])] {
        let plane = r.upload_mesh(&primitives::quad_mesh([0.0; 3], u, v, 0.11, 100));
        scene.instances.push(Instance::new(Drawable::Mesh(plane)).with_color([0.8, 0.85, 0.9, 0.2]).layer(5));
    }
    // A low, grazing view: the top face rises steeply toward the viewer from
    // its back edges, where faces used to win the edge's MSAA samples.
    let mut cam = Camera { rotation: Camera::look_rotation(glam::DVec3::new(-1.0, 1.0, -0.12), glam::DVec3::Z), ..Default::default() };
    let (w, h) = (1190u32, 790u32);
    cam.fit_box(glam::Vec3::from(lo).as_dvec3(), glam::Vec3::from(hi).as_dvec3(), w as f64 / h as f64);
    let f = r
        .render(RenderParams { scene: &scene, camera: &cam, width: w, height: h, device_pixel_ratio: 1.0, style: &Style::default(), pick: false })
        .unwrap();
    let corner = |i: u32| {
        glam::DVec3::new(
            if i & 1 == 0 { lo[0] } else { hi[0] } as f64,
            if i & 2 == 0 { lo[1] } else { hi[1] } as f64,
            if i & 4 == 0 { lo[2] } else { hi[2] } as f64,
        )
    };
    let vp = glam::DVec2::new(w as f64, h as f64);
    let radius = (scene.bounds().map(|(a, b)| (b - a).length() * 0.5).unwrap_or(1.0)) as f64;
    // The corner (x0, y1, z0) is hidden in this view; skip its three edges.
    let pairs = [(0, 1), (4, 5), (6, 7), (1, 3), (4, 6), (5, 7), (0, 4), (1, 5), (3, 7)];
    let (mut light, mut total) = (Vec::new(), 0);
    for (a, b) in pairs {
        let (sa, sb) = (cam.project(corner(a), vp, radius).unwrap(), cam.project(corner(b), vp, radius).unwrap());
        let n = (sb - sa).perp().normalize();
        // Away from the corners, where other edges meet.
        for k in 3..38 {
            let s = sa.lerp(sb, k as f64 / 40.0);
            // Darkest pixel across the (anti-aliased, 1.5 px) line at this point.
            let lum = [-1.0, -0.5, 0.0, 0.5, 1.0]
                .iter()
                .map(|o| {
                    let q = s + n * *o;
                    let px = &f.bgra[((q.y.floor() as u32 * w + q.x.floor() as u32) * 4) as usize..][..3];
                    px.iter().map(|&c| c as u32).sum::<u32>() / 3
                })
                .min()
                .unwrap();
            total += 1;
            if lum > 100 {
                light.push(((a, b), k, lum));
            }
        }
    }
    if let Ok(dir) = std::env::var("NOSHAPE_TEST_OUT") {
        let mut rgba = f.bgra.clone();
        for px in rgba.as_chunks_mut::<4>().0 {
            px.swap(0, 2);
        }
        image::save_buffer(format!("{dir}/plate_edges.png"), &rgba, w, h, image::ExtendedColorType::Rgba8).unwrap();
    }
    // A 1.5 px silhouette straddling two pixel rows over the light background
    // can read ~125 at an isolated sample; dotted edges fail runs of samples
    // (about 10 % of them before the fix).
    let consecutive = light.windows(2).any(|w| w[0].0 == w[1].0 && w[1].1 == w[0].1 + 1);
    assert!(light.len() * 50 <= total && !consecutive, "{} of {total} edge samples covered by faces: {light:?}", light.len());
}

/// Regression (stale frames): back-to-back scene changes for one target,
/// including changes of only an instance color or only a pick state, must
/// each render their own content (instance/state uploads are skipped only
/// when the data is really unchanged).
#[test]
fn every_scene_change_renders_fresh_content() {
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let mesh = r.upload_mesh(&primitives::box_mesh([-0.01, -0.01, -0.01], [0.01, 0.01, 0.01], 1));
    let scene = |color: [f32; 4], selected: bool| {
        let mut s = Scene::default();
        s.instances.push(Instance::new(Drawable::Mesh(mesh.clone())).with_color(color));
        if selected {
            s.set_state(2, state::SELECTED);
        }
        std::sync::Arc::new(s)
    };
    let red = [0.8, 0.05, 0.05, 1.0];
    let blue = [0.05, 0.05, 0.8, 1.0];
    let scenes = [scene(red, false), scene(blue, false), scene(blue, true), scene(red, false)];
    let cam = iso_camera();
    let (w, h) = (200u32, 150u32);
    let mut frames = Vec::new();
    for (i, s) in scenes.iter().enumerate() {
        let seq = i as u64 + 1;
        while !r.can_submit(3) {
            r.wait_oldest();
            frames.extend(r.collect());
        }
        r.submit_frame(3, seq, FrameRequest { scene: s.clone(), camera: cam, width: w, height: h, device_pixel_ratio: 1.0, style: Style::default(), invalidate_pick: true }).unwrap();
    }
    while r.has_pending() {
        r.wait_oldest();
        frames.extend(r.collect());
    }
    // Top face (+Z, id 2) center pixel of every frame.
    let px: Vec<[u8; 3]> = frames
        .into_iter()
        .filter_map(|c| match c {
            Completed::Frame { result, .. } => Some(result.unwrap()),
            _ => None,
        })
        .map(|f| {
            let p = cam.project(glam::DVec3::new(0.0, 0.0, 0.01), glam::DVec2::new(w as f64, h as f64), 0.02).unwrap();
            let o = ((p.y as u32 * w + p.x as u32) * 4) as usize;
            [f.bgra[o + 2], f.bgra[o + 1], f.bgra[o]]
        })
        .collect();
    assert_eq!(px.len(), 4);
    let reddish = |c: [u8; 3]| c[0] > c[2] + 40;
    let bluish = |c: [u8; 3]| c[2] > c[0] + 40;
    assert!(reddish(px[0]), "frame 1 {:?}", px[0]);
    assert!(bluish(px[1]), "frame 2 {:?}", px[1]);
    assert_ne!(px[2], px[1], "selection state change must show");
    assert!(reddish(px[3]), "frame 4 {:?}", px[3]);
}
