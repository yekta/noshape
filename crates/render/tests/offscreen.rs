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
    r.submit_picks(7, &[(11, PickRect::around(200, 150, 5)), (12, PickRect { x: -10, y: -10, width: 20, height: 20, depth_samples: 0 })]);
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

#[test]
fn visible_depth_samples_land_on_the_geometry() {
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    // A box off the view centre, so the average is not trivially the target.
    let mesh = r.upload_mesh(&primitives::box_mesh([0.004, -0.006, -0.006], [0.016, 0.006, 0.006], 1));
    let mut scene = Scene::default();
    scene.instances.push(Instance::new(Drawable::Mesh(mesh)));
    let scene = std::sync::Arc::new(scene);
    let mut cam = Camera::default();
    cam.fit_sphere(glam::DVec3::ZERO, 0.03, 4.0 / 3.0);
    let req = FrameRequest { scene, camera: cam, width: 400, height: 300, device_pixel_ratio: 1.0, style: Style::default(), invalidate_pick: true };
    r.submit_frame(7, 1, req).unwrap();
    r.submit_picks(7, &[(21, PickRect::depth_samples(2000))]);
    let mut got = None;
    while r.has_pending() {
        r.wait_oldest();
        for c in r.collect() {
            if let Completed::Pick { token: 21, result, .. } = c {
                got = Some(result.unwrap());
            }
        }
    }
    let buf = got.expect("sample query answered");
    assert!(buf.ids.is_empty());
    // About 2000 samples (a 52x39 viewport for 400x300); the box covers a fraction of them.
    assert_eq!((buf.frame_width, buf.frame_height), noshape_render::renderer::depth_sample_size(400, 300, 2000));
    let total = (buf.frame_width * buf.frame_height) as usize;
    assert!(total >= 2000 && total < 2200, "{total}");
    assert!(buf.samples.len() >= 4 && buf.samples.len() < total, "{} samples", buf.samples.len());
    assert!(buf.samples.iter().all(|s| s.id != 0 && s.depth > 0.0 && s.depth <= 1.0));
    // The average sample, unprojected with the pick pass's matrix, lies in the box.
    let n = buf.samples.len() as f32;
    let (x, y, z) = buf.samples.iter().fold((0.0, 0.0, 0.0), |a, s| (a.0 + s.x / n, a.1 + s.y / n, a.2 + s.depth / n));
    let (fw, fh) = (buf.frame_width as f32, buf.frame_height as f32);
    let ndc = glam::Vec4::new(x / fw * 2.0 - 1.0, 1.0 - y / fh * 2.0, z, 1.0);
    let w = buf.view_proj.unwrap().inverse() * ndc;
    let p = w.truncate() / w.w;
    assert!(p.x > 0.003 && p.x < 0.017 && p.y.abs() < 0.007 && p.z.abs() < 0.007, "{p:?}");
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

/// A UV sphere around the origin (counter-clockwise from outside).
fn sphere_mesh(radius: f32, n: u32) -> MeshData {
    let mut m = MeshData::default();
    for i in 0..=n {
        let t = i as f32 / n as f32 * std::f32::consts::PI;
        for j in 0..=n {
            let ph = j as f32 / n as f32 * std::f32::consts::TAU;
            let d = [t.sin() * ph.cos(), t.sin() * ph.sin(), t.cos()];
            m.positions.push([d[0] * radius, d[1] * radius, d[2] * radius]);
            m.normals.push(d);
        }
    }
    m.pick_ids = vec![1; m.positions.len()];
    let at = |i: u32, j: u32| i * (n + 1) + j;
    for i in 0..n {
        for j in 0..n {
            m.indices.extend_from_slice(&[at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j), at(i + 1, j + 1), at(i, j + 1)]);
        }
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
            // A sample the faces won reads like the surface beside the edge;
            // a line straddling two pixel rows (no pixel fully covered) reads
            // about half of it. Compare with the darker of the two surfaces
            // 5 px away (never below the old absolute limit of 100), so
            // brighter face shading doesn't turn anti-aliasing into failures.
            let beside = [-5.0, 5.0]
                .iter()
                .map(|o| {
                    let q = s + n * *o;
                    let px = &f.bgra[((q.y.floor() as u32 * w + q.x.floor() as u32) * 4) as usize..][..3];
                    px.iter().map(|&c| c as u32).sum::<u32>() / 3
                })
                .min()
                .unwrap();
            let limit = (beside * 7 / 10).max(100);
            total += 1;
            if lum > limit {
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

/// Section view caps: looking through a cut, the solid's inside is covered
/// by a hatched cap on the cut plane. A red block buried in the material
/// behind the cut must stay hidden (without caps the view went into the
/// solid and showed it), and the cap shows dark hatch lines.
#[test]
fn section_caps_cover_the_cut() {
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let mut scene = Scene::default();
    let blue = [0.45f32, 0.6, 0.75, 1.0];
    scene.instances.push(Instance::new(Drawable::Mesh(r.upload_mesh(&primitives::box_mesh([-0.05, -0.03, -0.03], [0.05, 0.03, 0.03], 1)))).with_color(blue));
    // Inside the material at x < 0, behind the cut plane x = 0.
    scene.instances.push(Instance::new(Drawable::Mesh(r.upload_mesh(&primitives::box_mesh([-0.03, -0.01, -0.01], [-0.01, 0.01, 0.01], 100)))).with_color([1.0, 0.0, 0.0, 1.0]));
    // Remove x > 0.
    scene.clip = Some(ClipPlane { normal: glam::Vec3::X, d: 0.0 });
    let mut cam = Camera { rotation: Camera::look_rotation(glam::DVec3::new(-1.0, 0.15, -0.1).normalize(), glam::DVec3::Z), ..Default::default() };
    let (w, h) = (400u32, 300u32);
    cam.fit_box(glam::DVec3::new(-0.05, -0.03, -0.03), glam::DVec3::new(0.05, 0.03, 0.03), w as f64 / h as f64);
    let f = r.render(RenderParams { scene: &scene, camera: &cam, width: w, height: h, device_pixel_ratio: 1.0, style: &Style::default(), pick: false }).unwrap();
    let px = |x: u32, y: u32| {
        let p = &f.bgra[((y * w + x) * 4) as usize..][..3];
        (p[2] as i32, p[1] as i32, p[0] as i32)
    };
    let (mut red, mut lums) = (0, Vec::new());
    for y in (h / 2 - 15)..(h / 2 + 15) {
        for x in (w / 2 - 15)..(w / 2 + 15) {
            let (r8, g8, b8) = px(x, y);
            // Red in any shade (a cap of the buried block would be red too).
            if r8 > g8 + 60 && r8 > b8 + 60 {
                red += 1;
            }
            lums.push(r8 + g8 + b8);
        }
    }
    assert_eq!(red, 0, "the buried block shows through the cut: no cap");
    lums.sort();
    let (dark, light) = (lums[lums.len() / 50], lums[lums.len() / 2]);
    assert!(dark * 4 < light * 3, "no hatch lines on the cap (darkest {dark}, median {light})");
}

/// Regression: a box cut at mid-height seen from above-front. The front face
/// below the plane is not cut and must keep its plain shading (the cap only
/// covers the cut, not faces the view ray reaches before the material).
#[test]
fn section_leaves_uncut_faces_unhatched() {
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let mut scene = Scene::default();
    scene.instances.push(Instance::new(Drawable::Mesh(r.upload_mesh(&primitives::box_mesh([-0.05, -0.05, -0.05], [0.05, 0.05, 0.05], 1)))).with_color([0.45, 0.6, 0.75, 1.0]));
    // Remove z > 0.
    scene.clip = Some(ClipPlane { normal: glam::Vec3::Z, d: 0.0 });
    let mut cam = Camera { rotation: Camera::look_rotation(glam::DVec3::new(0.3, 1.0, -0.8).normalize(), glam::DVec3::Z), ..Default::default() };
    let (w, h) = (400u32, 300u32);
    cam.fit_box(glam::DVec3::splat(-0.05), glam::DVec3::splat(0.05), w as f64 / h as f64);
    let f = r.render(RenderParams { scene: &scene, camera: &cam, width: w, height: h, device_pixel_ratio: 1.0, style: &Style::default(), pick: false }).unwrap();
    let lum = |x: u32, y: u32| {
        let p = &f.bgra[((y * w + x) * 4) as usize..][..3];
        p[0] as i32 + p[1] as i32 + p[2] as i32
    };
    let at = |p: glam::DVec3| cam.project(p, glam::DVec2::new(w as f64, h as f64), 0.2).expect("on screen");
    // The middle of the front face's uncut lower half, and of the cap.
    let side = at(glam::DVec3::new(0.0, -0.05, -0.025));
    let cap = at(glam::DVec3::new(0.0, 0.0, 0.0));
    let spread = |c: glam::DVec2| {
        let mut v: Vec<i32> = Vec::new();
        for dy in -8..=8 {
            for dx in -8..=8 {
                v.push(lum((c.x as i32 + dx) as u32, (c.y as i32 + dy) as u32));
            }
        }
        v.sort();
        (v[2], v[v.len() / 2])
    };
    let (side_dark, side_mid) = spread(side);
    assert!(side_dark * 10 > side_mid * 9, "hatch lines on the uncut front face (darkest {side_dark}, median {side_mid})");
    let (cap_dark, cap_mid) = spread(cap);
    assert!(cap_dark * 4 < cap_mid * 3, "no hatched cap on the cut (darkest {cap_dark}, median {cap_mid})");
}

/// Zebra stripes: a shaded box face turns into black and white stripes
/// whose count follows the instance setting; turning them off restores
/// the normal shading.
#[test]
fn zebra_stripes_replace_shading() {
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let (w, h) = (300u32, 300u32);
    let mesh = r.upload_mesh(&sphere_mesh(0.05, 48));
    let mut cam = iso_camera();
    cam.fit_box(glam::DVec3::splat(-0.05), glam::DVec3::splat(0.05), 1.0);
    let mut shot = |zebra: Option<Zebra>| {
        let mut scene = Scene::default();
        scene.instances.push(Instance::new(Drawable::Mesh(mesh.clone())).with_color([0.45, 0.6, 0.75, 1.0]).zebra(zebra));
        let f = r.render(RenderParams { scene: &scene, camera: &cam, width: w, height: h, device_pixel_ratio: 1.0, style: &Style::default(), pick: false }).unwrap();
        // Black / white / other pixels in the middle of the sphere, and
        // black-white changes along its vertical center line.
        let px = |x: u32, y: u32| {
            let p = &f.bgra[((y * w + x) * 4) as usize..][..3];
            (p[2] as i32, p[1] as i32, p[0] as i32)
        };
        let (mut black, mut white, mut other) = (0, 0, 0);
        for y in (h / 2 - 40)..(h / 2 + 40) {
            for x in (w / 2 - 40)..(w / 2 + 40) {
                match px(x, y) {
                    (0..=10, 0..=10, 0..=10) => black += 1,
                    (245.., 245.., 245..) => white += 1,
                    _ => other += 1,
                }
            }
        }
        let column: Vec<bool> = ((h / 2 - 40)..(h / 2 + 40)).map(|y| px(w / 2, y).0 > 128).collect();
        let changes = column.windows(2).filter(|c| c[0] != c[1]).count();
        (black, white, other, changes)
    };
    let (b, wh, o, few) = shot(Some(Zebra { stripes: 10, flip: false }));
    assert!(b > 500 && wh > 500 && o < 200, "not striped: {b} black, {wh} white, {o} other");
    let (_, _, _, many) = shot(Some(Zebra { stripes: 40, flip: false }));
    assert!(many > few && few > 0, "stripe count ignored: {few} vs {many} changes");
    let (b, wh, _, _) = shot(None);
    assert!(b < 50 && wh < 50, "shading not restored: {b} black, {wh} white");
}

/// Reflection analysis: a box mirrors a two-tone environment (white sky,
/// black ground): its top face reflects the sky, its sides the ground.
#[test]
fn reflection_mirrors_the_environment() {
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let (ew, eh) = (64u32, 32u32);
    let rgba: Vec<u8> = (0..eh).flat_map(|y| std::iter::repeat_n(if y < eh / 2 { [255u8, 255, 255, 255] } else { [0, 0, 0, 255] }, ew as usize).flatten()).collect();
    let env = r.upload_image(&ImageData { corners: [[0.0; 3]; 4], width: ew, height: eh, rgba });
    let mesh = r.upload_mesh(&primitives::box_mesh([-0.05, -0.05, -0.05], [0.05, 0.05, 0.05], 1));
    let (w, h) = (300u32, 300u32);
    let mut cam = Camera { rotation: Camera::look_rotation(glam::DVec3::new(-1.0, -1.0, -1.0).normalize(), glam::DVec3::Z), ..Default::default() };
    cam.fit_box(glam::DVec3::splat(-0.05), glam::DVec3::splat(0.05), 1.0);
    let mut shot = |reflection: Option<Reflection>| {
        let mut scene = Scene::default();
        scene.instances.push(Instance::new(Drawable::Mesh(mesh.clone())).with_color([0.45, 0.6, 0.75, 1.0]).reflection(reflection));
        let f = r.render(RenderParams { scene: &scene, camera: &cam, width: w, height: h, device_pixel_ratio: 1.0, style: &Style::default(), pick: false }).unwrap();
        // The middle of the view, where the three visible faces meet.
        let (mut white, mut black) = (0, 0);
        for (y, x) in (h / 2 - 50..h / 2 + 50).flat_map(|y| (w / 2 - 50..w / 2 + 50).map(move |x| (y, x))) {
            let p = &f.bgra[((y * w + x) * 4) as usize..][..3];
            match (p[2], p[1], p[0]) {
                (250.., 250.., 250..) => white += 1,
                (0..=5, 0..=5, 0..=5) => black += 1,
                _ => {}
            }
        }
        (white, black)
    };
    let (white, black) = shot(Some(Reflection { environment: env.clone(), rotation: 0.0, blur: false, lock: false }));
    assert!(white > 1500 && black > 1500, "not mirrored: {white} white, {black} black pixels");
    let (_, black) = shot(None);
    assert!(black < 100, "shaded box has {black} black pixels");
}

/// Sketch images: a textured quad facing the camera shows its texels the
/// right way up, is depth tested against meshes in front of and behind it
/// (transparent texels show what is behind), fades with the instance alpha,
/// follows section clipping unless `ignore_clip`, and is never picked.
#[test]
fn image_quad_is_textured_and_depth_tested() {
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    // 4x4 texels, row 0 on top: top-left quadrant green, bottom-right
    // quadrant transparent, the rest red.
    let mut rgba = Vec::new();
    for row in 0..4 {
        for col in 0..4 {
            rgba.extend_from_slice(match (row < 2, col < 2) {
                (true, true) => &[0u8, 255, 0, 255],
                (false, false) => &[0u8, 0, 255, 0],
                _ => &[255u8, 0, 0, 255],
            });
        }
    }
    let s = 0.01f32;
    let image = r.upload_image(&ImageData { corners: [[-s, -s, 0.0], [s, -s, 0.0], [s, s, 0.0], [-s, s, 0.0]], width: 4, height: 4, rgba });
    assert_eq!(image.bounds(), Some((glam::Vec3::new(-s, -s, 0.0), glam::Vec3::new(s, s, 0.0))));
    let mut scene = Scene::default();
    // In front of the top-right (red) quadrant.
    scene.instances.push(Instance::new(Drawable::Mesh(r.upload_mesh(&primitives::box_mesh([0.002, 0.002, 0.001], [0.009, 0.009, 0.003], 1)))));
    // Behind the transparent bottom-right quadrant.
    scene.instances.push(Instance::new(Drawable::Mesh(r.upload_mesh(&primitives::box_mesh([0.006, -0.009, -0.003], [0.009, -0.006, -0.001], 11)))));
    scene.instances.push(Instance::new(Drawable::Image(image)).layer(3));
    let (w, h) = (400u32, 300u32);
    let vp = glam::DVec2::new(w as f64, h as f64);
    let render = |r: &mut Renderer, scene: &Scene, cam: &Camera, msaa: u32| {
        let style = Style { msaa, ..Style::default() };
        r.render(RenderParams { scene, camera: cam, width: w, height: h, device_pixel_ratio: 1.0, style: &style, pick: true }).unwrap()
    };
    let at = |f: &Frame, cam: &Camera, p: [f64; 3]| {
        let q = cam.project(glam::DVec3::from(p), vp, 0.02).unwrap();
        let (x, y) = (q.x as i32, q.y as i32);
        let px = &f.bgra[((y as u32 * w + x as u32) * 4) as usize..][..3];
        ((px[2] as i32, px[1] as i32, px[0] as i32), f.pick.as_ref().unwrap().id_at(x, y).unwrap())
    };
    let is_red = |(r8, g8, b8): (i32, i32, i32)| r8 > 220 && g8 < 40 && b8 < 40;
    let is_green = |(r8, g8, b8): (i32, i32, i32)| g8 > 220 && r8 < 40 && b8 < 40;
    let is_white = |(r8, g8, b8): (i32, i32, i32)| r8 > 245 && g8 > 245 && b8 > 245;
    let is_gray = |(r8, g8, b8): (i32, i32, i32)| r8.max(g8).max(b8) - r8.min(g8).min(b8) < 40 && r8 < 245;
    for projection in [Projection::Orthographic, Projection::Perspective] {
        let mut cam = Camera { rotation: Camera::look_rotation(glam::DVec3::NEG_Z, glam::DVec3::Y), projection, ..Default::default() };
        cam.fit_box(glam::DVec3::new(-0.01, -0.01, -0.003), glam::DVec3::new(0.01, 0.01, 0.003), w as f64 / h as f64);
        for msaa in [1, 4] {
            let f = render(&mut r, &scene, &cam, msaa);
            let ctx = format!("{projection:?}, msaa {msaa}");
            let (c, id) = at(&f, &cam, [-0.005, -0.005, 0.0]);
            assert!(is_red(c), "{ctx}: bottom-left should be red, got {c:?}");
            assert_eq!(id, 0, "{ctx}: images are never picked");
            let (c, _) = at(&f, &cam, [-0.005, 0.005, 0.0]);
            assert!(is_green(c), "{ctx}: top-left should be green (row 0 on top), got {c:?}");
            let (c, id) = at(&f, &cam, [0.0055, 0.0055, 0.003]);
            assert!(is_gray(c), "{ctx}: the box in front should hide the image, got {c:?}");
            assert_eq!(id, 2, "{ctx}: top face of the box in front");
            let (c, _) = at(&f, &cam, [0.004, -0.004, 0.0]);
            assert!(is_white(c), "{ctx}: transparent texels show the background, got {c:?}");
            let (c, id) = at(&f, &cam, [0.0075, -0.0075, 0.0]);
            assert!(is_gray(c), "{ctx}: transparent texels show the box behind, got {c:?}");
            assert_eq!(id, 12, "{ctx}: the box behind transparent texels is pickable");
            if let Ok(dir) = std::env::var("NOSHAPE_TEST_OUT") {
                let mut rgba = f.bgra.clone();
                for px in rgba.as_chunks_mut::<4>().0 {
                    px.swap(0, 2);
                }
                image::save_buffer(format!("{dir}/image_{projection:?}_{msaa}.png"), &rgba, w, h, image::ExtendedColorType::Rgba8).unwrap();
            }
        }
        // Faded with the instance alpha: red over white turns pink.
        let mut faded = scene.clone();
        faded.instances[2].color[3] = 0.5;
        let f = render(&mut r, &faded, &cam, 4);
        let ((r8, g8, b8), _) = at(&f, &cam, [-0.005, -0.005, 0.0]);
        assert!(r8 > 240 && (150..225).contains(&g8) && (150..225).contains(&b8), "{projection:?}: faded image should be pink, got {:?}", (r8, g8, b8));
        let (c, _) = at(&f, &cam, [0.0055, 0.0055, 0.003]);
        assert!(is_gray(c), "{projection:?}: a faded image stays behind the box, got {c:?}");
        // Section clipping removes y < 0 of the image unless it ignores clipping.
        let mut cut = scene.clone();
        cut.clip = Some(ClipPlane { normal: glam::Vec3::NEG_Y, d: 0.0 });
        let f = render(&mut r, &cut, &cam, 4);
        let (c, _) = at(&f, &cam, [-0.005, -0.005, 0.0]);
        assert!(is_white(c), "{projection:?}: the clipped half should be gone, got {c:?}");
        let (c, _) = at(&f, &cam, [-0.005, 0.005, 0.0]);
        assert!(is_green(c), "{projection:?}: the kept half stays, got {c:?}");
        cut.instances[2].ignore_clip = true;
        let f = render(&mut r, &cut, &cam, 4);
        let (c, _) = at(&f, &cam, [-0.005, -0.005, 0.0]);
        assert!(is_red(c), "{projection:?}: ignore_clip keeps the whole image, got {c:?}");
    }
}

/// A large image (mip chain on the CPU, odd sizes) uploads and renders.
#[test]
fn image_with_odd_size_uploads_mips() {
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let (iw, ih) = (301u32, 97u32);
    let rgba: Vec<u8> = (0..iw * ih).flat_map(|_| [255u8, 0, 0, 255]).collect();
    let s = 0.01f32;
    let img = r.upload_image(&ImageData { corners: [[-s, -s, 0.0], [s, -s, 0.0], [s, s, 0.0], [-s, s, 0.0]], width: iw, height: ih, rgba });
    // Invalid data must not panic.
    let bad = r.upload_image(&ImageData { corners: [[0.0; 3]; 4], width: 10, height: 10, rgba: vec![0; 3] });
    let mut scene = Scene::default();
    scene.instances.push(Instance::new(Drawable::Image(img)));
    scene.instances.push(Instance::new(Drawable::Image(bad)));
    // Seen obliquely and small (minified: sampled from the mips).
    let mut cam = Camera { rotation: Camera::look_rotation(glam::DVec3::new(0.3, 0.2, -1.0).normalize(), glam::DVec3::Y), projection: Projection::Perspective, ..Default::default() };
    let (w, h) = (64u32, 48u32);
    cam.fit_box(glam::DVec3::new(-0.01, -0.01, 0.0), glam::DVec3::new(0.01, 0.01, 0.0), w as f64 / h as f64);
    let f = r.render(RenderParams { scene: &scene, camera: &cam, width: w, height: h, device_pixel_ratio: 1.0, style: &Style::default(), pick: false }).unwrap();
    let px = &f.bgra[(((h / 2) * w + w / 2) * 4) as usize..][..3];
    assert!(px[2] > 220 && px[1] < 40 && px[0] < 40, "center should be red, got {px:?}");
}

/// `hidden_only` lines show only where geometry hides them (the active
/// sketch's "underconstrained occluded" color).
#[test]
fn hidden_only_lines_show_only_behind_geometry() {
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let mut scene = Scene::default();
    scene.instances.push(Instance::new(Drawable::Mesh(r.upload_mesh(&primitives::box_mesh([-0.01, -0.01, -0.01], [0.01, 0.01, 0.01], 1)))));
    let mut line = LineData::default();
    // Behind the box as seen from the front (the front view looks along +Y).
    line.push_polyline(&[[-0.03, 0.02, 0.0], [0.03, 0.02, 0.0]], [255, 0, 0, 255], 0, 4.0, 0);
    scene.instances.push(Instance::new(Drawable::Lines(r.upload_lines(&line))).per_primitive_color().unpickable().hidden_only());
    let mut cam = Camera { rotation: Camera::rotation_for(StandardView::Front), ..Default::default() };
    cam.fit_sphere(glam::DVec3::ZERO, 0.035, 4.0 / 3.0);
    let (w, h) = (400u32, 300u32);
    let f = r.render(RenderParams { scene: &scene, camera: &cam, width: w, height: h, device_pixel_ratio: 1.0, style: &Style::default(), pick: false }).unwrap();
    let vp = glam::DVec2::new(w as f64, h as f64);
    let px = |p: glam::DVec3| {
        let s = cam.project(p, vp, 0.05).unwrap();
        let o = ((s.y as u32 * w + s.x as u32) * 4) as usize;
        [f.bgra[o + 2], f.bgra[o + 1], f.bgra[o]]
    };
    let hidden = px(glam::DVec3::new(0.0, 0.02, 0.0));
    let visible = px(glam::DVec3::new(0.025, 0.02, 0.0));
    assert!(hidden[0] > 200 && hidden[1] < 80, "the hidden part is drawn: {hidden:?}");
    assert!(visible[0] > 200 && visible[1] > 200, "the visible part is not: {visible:?}");
}

/// Feature previews shade like Onshape's (live part-studio/230, default
/// view, white background): the top and front faces alike, the right side
/// lighter, flat (no highlight), and see-through.
#[test]
fn feature_preview_shading_matches_onshape() {
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let (w, h) = (400u32, 300u32);
    // The app's preview color and opacity (theme FEATURE_PREVIEW, part_studio FEATURE_PREVIEW_ALPHA).
    let color = [srgb_lin(0x92), srgb_lin(0xae), srgb_lin(0xc5), 0.7];
    let mut scene = Scene::default();
    scene.instances.push(Instance::new(Drawable::Mesh(r.upload_mesh(&primitives::box_mesh([-0.05, -0.05, -0.05], [0.05, 0.05, 0.05], 1)))).with_color(color).preview());
    let mut cam = Camera::default();
    cam.fit_box(glam::DVec3::splat(-0.05), glam::DVec3::splat(0.05), w as f64 / h as f64);
    let f = r.render(RenderParams { scene: &scene, camera: &cam, width: w, height: h, device_pixel_ratio: 1.0, style: &Style::default(), pick: false }).unwrap();
    let rgb = |p: glam::DVec3| {
        let q = cam.project(p, glam::DVec2::new(w as f64, h as f64), 0.2).expect("on screen");
        let px = &f.bgra[((q.y as u32 * w + q.x as u32) * 4) as usize..][..3];
        [px[2] as i32, px[1] as i32, px[0] as i32]
    };
    let top = rgb(glam::DVec3::new(0.0, 0.0, 0.05));
    let front = rgb(glam::DVec3::new(0.0, -0.05, 0.0));
    let right = rgb(glam::DVec3::new(0.05, 0.0, 0.0));
    eprintln!("preview top {top:?} front {front:?} right {right:?}");
    let near = |a: [i32; 3], b: [i32; 3], tol: i32| a.iter().zip(b).all(|(x, y)| (x - y).abs() <= tol);
    // Live: top and front (132, 148, 158), right (160, 186, 202).
    assert!(near(top, front, 6), "top {top:?} and front {front:?} should read alike");
    assert!(near(top, [132, 148, 158], 6), "top {top:?}");
    assert!(near(right, [160, 186, 202], 6), "right {right:?}");
}

fn srgb_lin(c: u8) -> f32 {
    let c = c as f32 / 255.0;
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}
