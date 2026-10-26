use noshape_render::*;

/// One test on the GPU at a time: the Vulkan loader crashes (SIGSEGV in
/// wgpu's `set_object_name`) when one test builds pipelines while another
/// creates its instance.
static GPU: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn renders_box_and_picks_faces() {
    let _gpu = GPU.lock().unwrap_or_else(|e| e.into_inner());
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
    let _gpu = GPU.lock().unwrap_or_else(|e| e.into_inner());
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
    let _gpu = GPU.lock().unwrap_or_else(|e| e.into_inner());
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
    assert!((2000..2200).contains(&total), "{total}");
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

#[test]
fn plane_depth_counts_only_in_the_with_planes_samples() {
    let _gpu = GPU.lock().unwrap_or_else(|e| e.into_inner());
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    // Only a translucent, unlit datum plane (the Top plane, z = 0.004).
    let plane = r.upload_mesh(&primitives::quad_mesh([0.0, 0.0, 0.004], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], 0.01, 1));
    let mut scene = Scene::default();
    scene.instances.push(Instance::new(Drawable::Mesh(plane)).with_color([0.6, 0.7, 0.9, 0.3]).unlit());
    let scene = std::sync::Arc::new(scene);
    let mut cam = Camera::default();
    cam.fit_sphere(glam::DVec3::ZERO, 0.02, 4.0 / 3.0);
    let req = FrameRequest { scene, camera: cam, width: 400, height: 300, device_pixel_ratio: 1.0, style: Style::default(), invalidate_pick: true };
    r.submit_frame(7, 1, req).unwrap();
    r.submit_picks(7, &[(31, PickRect::depth_samples(2000))]);
    let mut got = None;
    while r.has_pending() {
        r.wait_oldest();
        for c in r.collect() {
            if let Completed::Pick { token: 31, result, .. } = c {
                got = Some(result.unwrap());
            }
        }
    }
    let buf = got.expect("sample query answered");
    assert!(buf.samples.is_empty(), "planes write no depth in the first pass");
    assert!(buf.samples_with_planes.len() >= 4);
    let n = buf.samples_with_planes.len() as f32;
    let (x, y, z) = buf.samples_with_planes.iter().fold((0.0, 0.0, 0.0), |a, s| (a.0 + s.x / n, a.1 + s.y / n, a.2 + s.depth / n));
    let (fw, fh) = (buf.frame_width as f32, buf.frame_height as f32);
    let w = buf.view_proj.unwrap().inverse() * glam::Vec4::new(x / fw * 2.0 - 1.0, 1.0 - y / fh * 2.0, z, 1.0);
    let p = w.truncate() / w.w;
    // On the plane, near its centre.
    assert!((p.z - 0.004).abs() < 1e-4 && p.x.abs() < 0.002 && p.y.abs() < 0.002, "{p:?}");
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
    let _gpu = GPU.lock().unwrap_or_else(|e| e.into_inner());
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

/// `state::NO_PICK`: a line drawn over a part edge (a shown sketch line)
/// stays visible but leaves the pick pass, so the edge under it is picked.
#[test]
fn no_pick_overlay_lets_the_edge_under_it_be_picked() {
    let _gpu = GPU.lock().unwrap_or_else(|e| e.into_inner());
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let (lo, hi) = ([-0.01f32, -0.01, -0.01], [0.01f32, 0.01, 0.01]);
    let c = |i: u32| [if i & 1 == 0 { lo[0] } else { hi[0] }, if i & 2 == 0 { lo[1] } else { hi[1] }, if i & 4 == 0 { lo[2] } else { hi[2] }];
    let pairs = [(0, 1), (2, 3), (4, 5), (6, 7), (0, 2), (1, 3), (4, 6), (5, 7), (0, 4), (1, 5), (2, 6), (3, 7)];
    let cam = iso_camera();
    let cam = &cam;
    let (w, h) = (400u32, 300u32);
    let vp = glam::DVec2::new(w as f64, h as f64);
    let mid = |k: usize| {
        let (a, b) = (c(pairs[k].0), c(pairs[k].1));
        glam::DVec3::new((a[0] + b[0]) as f64 * 0.5, (a[1] + b[1]) as f64 * 0.5, (a[2] + b[2]) as f64 * 0.5)
    };
    let mut render = |overlay: Option<(usize, bool)>| {
        let mut scene = Scene::default();
        scene.instances.push(Instance::new(Drawable::Mesh(r.upload_mesh(&primitives::box_mesh(lo, hi, 1)))));
        scene.instances.push(Instance::new(Drawable::Lines(r.upload_lines(&primitives::box_edges(lo, hi, 10, 1.5)))));
        if let Some((k, no_pick)) = overlay {
            let mut l = LineData::default();
            l.push_polyline(&[c(pairs[k].0), c(pairs[k].1)], [0, 0, 255, 255], 500, 3.0, LineSegment::ON_TOP);
            scene.instances.push(Instance::new(Drawable::Lines(r.upload_lines(&l))).layer(9));
            if no_pick {
                scene.set_state(500, state::NO_PICK);
            }
        }
        let f = r.render(RenderParams { scene: &scene, camera: cam, width: w, height: h, device_pixel_ratio: 1.0, style: &Style::default(), pick: true }).unwrap();
        let pick = f.pick.unwrap();
        move |p: glam::DVec3| {
            let s = cam.project(p, vp, 0.02).unwrap();
            pick.id_at(s.x as i32, s.y as i32).unwrap()
        }
    };
    // A visible edge: its midpoint picks it.
    let at = render(None);
    let k = (0..12).find(|&k| at(mid(k)) == 10 + k as u32).expect("a visible edge");
    assert_eq!(render(Some((k, false)))(mid(k)), 500, "the overlay covers the edge");
    assert_eq!(render(Some((k, true)))(mid(k)), 10 + k as u32, "a NO_PICK overlay lets the edge through");
}

/// Regression: edges fought faces at the same depth and looked dotted on
/// silhouettes, bottom faces and hole rims. Every visible edge of a shaded
/// plate in iso view (app-like scene: large default planes, wide viewport)
/// must be drawn solid.
#[test]
fn plate_edges_are_drawn_solid_over_faces() {
    let _gpu = GPU.lock().unwrap_or_else(|e| e.into_inner());
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
    let _gpu = GPU.lock().unwrap_or_else(|e| e.into_inner());
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
    let _gpu = GPU.lock().unwrap_or_else(|e| e.into_inner());
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
    scene.clip = Some(ClipPlane::new(glam::Vec3::X, 0.0));
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
    let _gpu = GPU.lock().unwrap_or_else(|e| e.into_inner());
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
    scene.clip = Some(ClipPlane::new(glam::Vec3::Z, 0.0));
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
    let _gpu = GPU.lock().unwrap_or_else(|e| e.into_inner());
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
    // Shaded again: no black stripes; Onshape's specular highlight may clip
    // to white (#999 on top of the lit color) but the shaded rest dominates.
    let (b, wh, o, _) = shot(None);
    assert!(b < 50 && o > 4 * wh, "shading not restored: {b} black, {wh} white, {o} other");
}

/// Reflection analysis: a box mirrors a two-tone environment (white sky,
/// black ground): its top face reflects the sky, its sides the ground.
#[test]
fn reflection_mirrors_the_environment() {
    let _gpu = GPU.lock().unwrap_or_else(|e| e.into_inner());
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
    let _gpu = GPU.lock().unwrap_or_else(|e| e.into_inner());
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
        // Faded with the instance alpha: red over white turns pink, mixed in
        // sRGB values as a WebGL canvas blends: (255, 128, 128).
        let mut faded = scene.clone();
        faded.instances[2].color[3] = 0.5;
        let f = render(&mut r, &faded, &cam, 4);
        let ((r8, g8, b8), _) = at(&f, &cam, [-0.005, -0.005, 0.0]);
        assert!(r8 > 240 && (120..136).contains(&g8) && (120..136).contains(&b8), "{projection:?}: faded image should be pink, got {:?}", (r8, g8, b8));
        let (c, _) = at(&f, &cam, [0.0055, 0.0055, 0.003]);
        assert!(is_gray(c), "{projection:?}: a faded image stays behind the box, got {c:?}");
        // Section clipping removes y < 0 of the image unless it ignores clipping.
        let mut cut = scene.clone();
        cut.clip = Some(ClipPlane::new(glam::Vec3::NEG_Y, 0.0));
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

/// Decals (live behaviours "icons2 decal rendering"): textured triangles on
/// a face show their texels exactly (unlit, over the face they lie on),
/// nothing outside the image's UV square, and a selected decal mixes 0.4
/// toward (255,170,10).
#[test]
fn decal_texels_are_exact_and_tint_when_selected() {
    let _gpu = GPU.lock().unwrap_or_else(|e| e.into_inner());
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    // 2x1 texels: blue-ish left, orange right (image A's gradient ends).
    let rgba = vec![15u8, 80, 239, 255, 240, 80, 14, 255];
    let s = 0.01f32;
    // The face's square, the image on its middle half (UV -0.5..1.5).
    let positions = vec![[-s, -s, 0.0], [s, -s, 0.0], [s, s, 0.0], [-s, s, 0.0]];
    let uvs = vec![[-0.5, 1.5], [1.5, 1.5], [1.5, -0.5], [-0.5, -0.5]];
    let decal = r.upload_textured(&TexturedData { positions, uvs, indices: vec![0, 1, 2, 0, 2, 3], width: 2, height: 1, rgba });
    let mut scene = Scene::default();
    scene.instances.push(Instance::new(Drawable::Mesh(r.upload_mesh(&primitives::box_mesh([-0.01, -0.01, -0.004], [0.01, 0.01, 0.0], 1)))));
    scene.instances.push(Instance::new(Drawable::Image(decal)).unpickable().layer(3));
    let (w, h) = (400u32, 300u32);
    let vp = glam::DVec2::new(w as f64, h as f64);
    let mut cam = Camera { rotation: Camera::look_rotation(glam::DVec3::NEG_Z, glam::DVec3::Y), ..Default::default() };
    cam.fit_box(glam::DVec3::new(-0.01, -0.01, -0.004), glam::DVec3::new(0.01, 0.01, 0.0), w as f64 / h as f64);
    let render = |r: &mut Renderer, scene: &Scene| {
        let style = Style { msaa: 4, ..Style::default() };
        r.render(RenderParams { scene, camera: &cam, width: w, height: h, device_pixel_ratio: 1.0, style: &style, pick: false }).unwrap()
    };
    let at = |f: &Frame, p: [f64; 3]| {
        let q = cam.project(glam::DVec3::from(p), vp, 0.02).unwrap();
        let px = &f.bgra[((q.y as u32 * w + q.x as u32) * 4) as usize..][..3];
        (px[2] as i32, px[1] as i32, px[0] as i32)
    };
    let near = |a: (i32, i32, i32), b: (i32, i32, i32)| (a.0 - b.0).abs() <= 1 && (a.1 - b.1).abs() <= 1 && (a.2 - b.2).abs() <= 1;
    let f = render(&mut r, &scene);
    let (left, right, outside) = (at(&f, [-0.0025, 0.0, 0.0]), at(&f, [0.0025, 0.0, 0.0]), at(&f, [-0.0075, 0.0, 0.0]));
    assert!(near(left, (15, 80, 239)), "the left texel exactly, unlit: {left:?}");
    assert!(near(right, (240, 80, 14)), "the right texel exactly: {right:?}");
    assert!(!near(outside, (15, 80, 239)) && !near(outside, (240, 80, 14)), "outside the image the face shows: {outside:?}");
    let mut sel = scene.clone();
    sel.instances[1].decal_selected = true;
    let f = render(&mut r, &sel);
    let tinted = at(&f, [-0.0025, 0.0, 0.0]);
    // 0.6 * (15,80,239) + 0.4 * (255,170,10) = (111,116,147).
    assert!(near(tinted, (111, 116, 147)), "selected: mixed 0.4 toward the amber: {tinted:?}");
}

/// A large image (mip chain on the CPU, odd sizes) uploads and renders.
#[test]
fn image_with_odd_size_uploads_mips() {
    let _gpu = GPU.lock().unwrap_or_else(|e| e.into_inner());
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
    let _gpu = GPU.lock().unwrap_or_else(|e| e.into_inner());
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
    let _gpu = GPU.lock().unwrap_or_else(|e| e.into_inner());
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

/// Onshape's part lighting (woolsthorpe `lightFragment`), in sRGB: 0.5 ×
/// the color, plus 0.5 × the color × max(N·L, 0) for one light along
/// eye-space (1, 1, 1), plus Blinn-Phong specular #999999 with exponent 30.
/// Every face normal of the default-blue cube, from Isometric and from
/// below; live (psb, part-studio/2258): its top #8cb9d4, its front #4e6876.
#[test]
fn part_faces_are_lit_like_onshape() {
    let _gpu = GPU.lock().unwrap_or_else(|e| e.into_inner());
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let (w, h) = (400u32, 300u32);
    let base = [0x9d, 0xcf, 0xed];
    let color = [srgb_lin(base[0]), srgb_lin(base[1]), srgb_lin(base[2]), 1.0];
    let mut scene = Scene::default();
    scene.instances.push(Instance::new(Drawable::Mesh(r.upload_mesh(&primitives::box_mesh([-0.05, -0.05, -0.05], [0.05, 0.05, 0.05], 1)))).with_color(color));
    let mut iso = Camera::default();
    iso.set_view(StandardView::Isometric);
    let below = Camera { rotation: Camera::look_rotation(glam::DVec3::new(1.0, -1.0, 1.0).normalize(), glam::DVec3::Z), ..Camera::default() };
    let mut samples = Vec::new();
    for mut cam in [iso, below] {
        cam.fit_box(glam::DVec3::splat(-0.05), glam::DVec3::splat(0.05), w as f64 / h as f64);
        let f = r.render(RenderParams { scene: &scene, camera: &cam, width: w, height: h, device_pixel_ratio: 1.0, style: &Style::default(), pick: false }).unwrap();
        let (fwd, up) = (cam.forward(), cam.up());
        let light = (fwd.cross(up) + up - fwd).normalize();
        for n in [glam::DVec3::X, -glam::DVec3::X, glam::DVec3::Y, -glam::DVec3::Y, glam::DVec3::Z, -glam::DVec3::Z] {
            let p = n * 0.05;
            if n.dot(cam.eye() - p) <= 0.0 {
                continue;
            }
            let q = cam.project(p, glam::DVec2::new(w as f64, h as f64), 0.2).expect("on screen");
            let px = &f.bgra[((q.y as u32 * w + q.x as u32) * 4) as usize..][..3];
            let got = [px[2] as f64, px[1] as f64, px[0] as f64];
            let half = (light + (cam.eye() - p).normalize()).normalize();
            let spec = n.dot(half).max(0.0).powf(30.0) * 0.6;
            let want: Vec<f64> = base.iter().map(|&c| ((c as f64 / 255.0) * (0.5 + 0.5 * n.dot(light).max(0.0)) + spec).min(1.0) * 255.0).collect();
            eprintln!("normal {n:?}: got {got:?}, Onshape's model {want:?}");
            assert!(got.iter().zip(&want).all(|(g, w)| (g - w).abs() <= 2.0), "normal {n:?}: {got:?} vs {want:?}");
            samples.push((n, got));
        }
    }
    assert_eq!(samples.len(), 6, "every face normal sampled");
    let live = |n: glam::DVec3, want: [f64; 3]| {
        let got = samples.iter().find(|(m, _)| *m == n).expect("sampled").1;
        assert!(got.iter().zip(want).all(|(g, w)| (g - w).abs() <= 4.0), "{n:?}: {got:?} vs live {want:?}");
    };
    live(glam::DVec3::Z, [140.0, 185.0, 212.0]);
    live(-glam::DVec3::Y, [78.0, 104.0, 118.0]);
}

fn srgb_lin(c: u8) -> f32 {
    let c = c as f32 / 255.0;
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

#[test]
fn texture_frames_stay_on_the_gpu_and_textures_are_reused_once_released() {
    let _gpu = GPU.lock().unwrap_or_else(|e| e.into_inner());
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
    let req = || FrameRequest { scene: scene.clone(), camera: cam, width: 320, height: 240, device_pixel_ratio: 1.0, style: Style::default(), invalidate_pick: false };
    r.set_present(9, Present::Texture);
    let frame_of = |r: &mut Renderer, seq: u64| {
        assert!(r.can_submit(9), "frame {seq} could not be submitted");
        r.submit_frame(9, seq, req()).unwrap();
        loop {
            r.wait_oldest();
            if let Some(Completed::Frame { result, .. }) = r.collect().into_iter().next() {
                return result.unwrap();
            }
        }
    };
    // Every frame the UI still holds keeps its texture; the renderer stops
    // accepting frames once all of a target's textures are held.
    let mut held = Vec::new();
    let mut seq = 0;
    while r.can_submit(9) {
        seq += 1;
        let f = frame_of(&mut r, seq);
        assert!(f.bgra.is_empty(), "a texture frame is not read back");
        let t = f.texture.clone().expect("the frame's texture");
        assert_eq!((t.width, t.height), (320, 240));
        assert!(held.iter().all(|h: &Frame| !std::sync::Arc::ptr_eq(h.texture.as_ref().unwrap(), &t)), "a held texture was rendered into again");
        held.push(f);
        assert!(seq < 32, "texture frames never stop");
    }
    assert!(seq >= 3, "only {seq} textures");
    // Releasing one frame frees its texture for the next frame.
    let released = held.remove(0).texture.unwrap();
    let released_ptr = std::sync::Arc::as_ptr(&released);
    drop(released);
    let f = frame_of(&mut r, seq + 1);
    assert_eq!(std::sync::Arc::as_ptr(f.texture.as_ref().unwrap()), released_ptr);
    // Back to read-back frames.
    drop(held);
    drop(f);
    r.set_present(9, Present::Readback);
    let f = frame_of(&mut r, seq + 2);
    assert!(f.texture.is_none());
    assert_eq!(f.bgra.len(), 320 * 240 * 4);
}

#[test]
fn view_cube_overlay_is_drawn_in_the_top_right_corner_only() {
    use noshape_render::overlay::{CubeColors, Overlay, ViewCube};
    let _gpu = GPU.lock().unwrap_or_else(|e| e.into_inner());
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let scene = std::sync::Arc::new(Scene::default());
    let cam = Camera::default();
    let (w, h) = (400u32, 300u32);
    let req = || FrameRequest { scene: scene.clone(), camera: cam, width: w, height: h, device_pixel_ratio: 1.0, style: Style::default(), invalidate_pick: false };
    let frame = |r: &mut Renderer, seq: u64| {
        r.submit_frame(4, seq, req()).unwrap();
        loop {
            r.wait_oldest();
            if let Some(Completed::Frame { result, .. }) = r.collect().into_iter().next() {
                return result.unwrap();
            }
        }
    };
    let plain = frame(&mut r, 1);
    // A red face (Onshape's is white, like the background here).
    let colors = CubeColors { face: [1.0, 0.0, 0.0, 1.0], frame: [0.808, 0.859, 0.898, 0.6], highlight: [0.565, 0.808, 0.945, 0.6], label: [0.0, 0.0, 0.0, 0.85], axes: [[0.804, 0.0, 0.0, 1.0], [0.0, 0.58, 0.0, 1.0], [0.0, 0.0, 0.816, 1.0]] };
    let cube = ViewCube { origin: [(w - 15 - 150) as f32, 15.0], hover: None, face_labels: Vec::new(), axis_labels: None, colors };
    r.set_overlay(4, Some(std::sync::Arc::new(Overlay { world_fills: Vec::new(), labels: Vec::new(), window_origin: [0.0; 2], view_cube: Some(cube) })));
    let with = frame(&mut r, 2);
    let px = |f: &Frame, x: u32, y: u32| -> [u8; 3] {
        let o = ((y * w + x) * 4) as usize;
        [f.bgra[o], f.bgra[o + 1], f.bgra[o + 2]]
    };
    if let Ok(dir) = std::env::var("NOSHAPE_TEST_OUT") {
        let mut rgba = with.bgra.clone();
        for p in rgba.as_chunks_mut::<4>().0 {
            p.swap(0, 2);
        }
        image::save_buffer(format!("{dir}/view_cube_overlay.png"), &rgba, w, h, image::ExtendedColorType::Rgba8).unwrap();
    }
    // The widget (150 px, 15 px in from the top-right corner) shows red faces.
    let red = |f: &Frame| (15..165).flat_map(|y| (w - 165..w - 15).map(move |x| (x, y))).filter(|&(x, y)| px(f, x, y) == [0, 0, 255]).count();
    assert_eq!(red(&plain), 0);
    assert!(red(&with) > 3000, "{} face pixels", red(&with));
    // Everything outside the widget is untouched (one row more at the
    // bottom: live draws the cube 1 px under the widget's middle, so the
    // down arrow's tip ends on the widget's last row, its anti-aliasing 1 px
    // under it; `view_cube::CENTER`).
    let changed_outside = (0..h).flat_map(|y| (0..w).map(move |x| (x, y))).filter(|&(x, y)| !(x >= w - 165 && y < 166)).filter(|&(x, y)| px(&plain, x, y) != px(&with, x, y)).count();
    assert_eq!(changed_outside, 0);
}

#[test]
fn overlay_path_pixel_clip_drops_pixels_outside_path_bounds() {
    use noshape_render::overlay::{CubeColors, Overlay, TextShading, ViewCube};
    let _gpu = GPU.lock().unwrap_or_else(|e| e.into_inner());
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let scene = std::sync::Arc::new(Scene::default());
    let (w, h) = (400u32, 300u32);
    let req = || FrameRequest { scene: scene.clone(), camera: Camera::default(), width: w, height: h, device_pixel_ratio: 2.0, style: Style::default(), invalidate_pick: false };
    let frame = |r: &mut Renderer, seq: u64| {
        r.submit_frame(6, seq, req()).unwrap();
        loop {
            r.wait_oldest();
            if let Some(Completed::Frame { result, .. }) = r.collect().into_iter().next() {
                return result.unwrap();
            }
        }
    };
    let colors = CubeColors { face: [1.0; 4], frame: [0.808, 0.859, 0.898, 0.6], highlight: [0.565, 0.808, 0.945, 0.6], label: [0.0, 0.0, 0.0, 0.85], axes: [[0.804, 0.0, 0.0, 1.0], [0.0, 0.58, 0.0, 1.0], [0.0, 0.0, 0.816, 1.0]] };
    // Half-pixel origin: the up arrow's base lies on a pixel boundary.
    let cube = ViewCube { origin: [61.0, 0.5], hover: None, face_labels: Vec::new(), axis_labels: None, colors };
    r.set_overlay(6, Some(std::sync::Arc::new(Overlay { world_fills: Vec::new(), labels: Vec::new(), window_origin: [0.0; 2], view_cube: Some(cube) })));
    let soft = frame(&mut r, 1);
    r.set_text_shading(TextShading { clip_path_pixels: true, ..Default::default() });
    let clipped = frame(&mut r, 2);
    for y in 17..23u32 {
        let row = |f: &Frame| (205..218u32).map(|x| f.bgra[((y * w + x) * 4) as usize]).collect::<Vec<_>>();
        eprintln!("{y}: soft {:?} clipped {:?}", row(&soft), row(&clipped));
    }
    let changed = soft.bgra.chunks(4).zip(clipped.bgra.chunks(4)).filter(|(a, b)| a != b).count();
    assert!(changed > 0, "clipping path pixels changed nothing");
}

/// Hidden edges stay hidden at close zoom (user report: thin ribs on a block
/// showed their far edges through the faces in front). A block with 1 x 4 mm
/// ribs; the edges at each rib's far corner (both faces back-facing, inset
/// from the corners where they meet visible ones) are red.
/// No red may show, orthographic or perspective, at 1x and 2x.
#[test]
fn edges_behind_faces_stay_hidden_at_close_zoom() {
    let _gpu = GPU.lock().unwrap_or_else(|e| e.into_inner());
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let mm = 1e-3f32;
    let blue = [0.45f32, 0.6, 0.75, 1.0];
    let mut scene = Scene::default();
    scene.instances.push(Instance::new(Drawable::Mesh(r.upload_mesh(&primitives::box_mesh([0.0; 3], [100.0 * mm, 100.0 * mm, 20.0 * mm], 1)))).with_color(blue));
    let ribs: Vec<([f32; 3], [f32; 3])> = (0..6).map(|i| {
        let x = (60.0 + i as f32 * 3.0) * mm;
        ([x, 0.0, 20.0 * mm], [x + mm, 4.0 * mm, 21.0 * mm])
    }).collect();
    for (i, (lo, hi)) in ribs.iter().enumerate() {
        scene.instances.push(Instance::new(Drawable::Mesh(r.upload_mesh(&primitives::box_mesh(*lo, *hi, 10 + i as u32 * 10)))).with_color(blue));
    }
    // Looking toward -x, +y, -z: the far corner of a rib is at its low x, high y, low z.
    let dir = glam::DVec3::new(-0.55, 0.75, -0.45).normalize();
    let mut lines = LineData::default();
    for (lo, hi) in &ribs {
        let far = glam::Vec3::new(lo[0], hi[1], lo[2]);
        for to in [glam::Vec3::new(hi[0], hi[1], lo[2]), glam::Vec3::new(lo[0], lo[1], lo[2]), glam::Vec3::new(lo[0], hi[1], hi[2])] {
            let t = (mm / (to - far).length()).min(0.25);
            lines.segments.push(LineSegment { a: far.lerp(to, t).into(), b: to.lerp(far, t).into(), color: [255, 0, 0, 255], pick_id: 0, width: 1.0, style: 0 });
        }
    }
    let probes = scene.instances.len();
    scene.instances.push(Instance::new(Drawable::Lines(r.upload_lines(&lines))).per_primitive_color());
    // NOSHAPE_TEST_OUT: also every box's edges in black, as the app draws
    // them (before/after crops; not checked).
    let out = std::env::var("NOSHAPE_TEST_OUT").ok();
    let mut look = scene.clone();
    look.instances.truncate(probes);
    let boxes = std::iter::once(([0.0f32; 3], [100.0 * mm, 100.0 * mm, 20.0 * mm])).chain(ribs.iter().copied());
    for (lo, hi) in boxes {
        look.instances.push(Instance::new(Drawable::Lines(r.upload_lines(&primitives::box_edges(lo, hi, 0, 1.25)))).with_color([0.0, 0.0, 0.0, 1.0]));
    }
    let mut failures = Vec::new();
    for projection in [Projection::Orthographic, Projection::Perspective] {
        for zoom_mm in [6.0, 3.0, 1.5] {
            for dpr in [1.0f32, 2.0] {
                let (w, h) = ((400.0 * dpr) as u32, (300.0 * dpr) as u32);
                let mut cam = Camera { rotation: Camera::look_rotation(dir, glam::DVec3::Z), projection, ..Default::default() };
                let end = glam::DVec3::new(67.5e-3, 0.0, 20.5e-3);
                cam.fit_box(end - glam::DVec3::splat(zoom_mm * 1e-3), end + glam::DVec3::splat(zoom_mm * 1e-3), w as f64 / h as f64);
                let f = r.render(RenderParams { scene: &scene, camera: &cam, width: w, height: h, device_pixel_ratio: dpr, style: &Style::default(), pick: false }).unwrap();
                let red = f.bgra.chunks(4).filter(|p| p[2] > 150 && p[1] < 110 && p[0] < 110).count();
                if red > 0 {
                    failures.push(format!("{projection:?}, {zoom_mm} mm, {dpr}x: {red} pixels of hidden edges show"));
                }
                if let Some(dir) = &out {
                    let f2 = r.render(RenderParams { scene: &look, camera: &cam, width: w, height: h, device_pixel_ratio: dpr, style: &Style::default(), pick: false }).unwrap();
                    for (name, f) in [("probes", &f), ("edges", &f2)] {
                        let mut rgba = f.bgra.clone();
                        for px in rgba.as_chunks_mut::<4>().0 {
                            px.swap(0, 2);
                        }
                        image::save_buffer(format!("{dir}/hidden_{name}_{projection:?}_{zoom_mm}mm_{dpr}x.png"), &rgba, w, h, image::ExtendedColorType::Rgba8).unwrap();
                    }
                }
            }
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

/// Copies sharing one mesh draw in one instanced call yet keep their own
/// transform, pick ids and highlight state: three boxes in a row, the middle
/// one selected and the right one hidden.
#[test]
fn shared_mesh_copies_keep_their_own_picks_and_states() {
    let _gpu = GPU.lock().unwrap_or_else(|e| e.into_inner());
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
    let xs = [-0.03f32, 0.0, 0.03];
    for (k, x) in xs.iter().enumerate() {
        let t = glam::Mat4::from_translation(glam::Vec3::new(*x, 0.0, 0.0));
        scene.instances.push(Instance::new(Drawable::Mesh(mesh.clone())).with_transform(t).pick_base(k as u32 * 100));
        scene.instances.push(Instance::new(Drawable::Lines(edges.clone())).with_color([0.02, 0.02, 0.03, 1.0]).with_transform(t).pick_base(k as u32 * 100));
    }
    for face in 1..=6 {
        scene.set_state(100 + face, state::SELECTED);
        scene.set_state(200 + face, state::HIDDEN);
    }
    let mut cam = Camera { rotation: Camera::look_rotation(glam::DVec3::new(0.0, 1.0, -0.3).normalize(), glam::DVec3::Z), ..Default::default() };
    let (w, h) = (600u32, 300u32);
    cam.fit_box(glam::DVec3::new(-0.04, -0.01, -0.01), glam::DVec3::new(0.04, 0.01, 0.01), w as f64 / h as f64);
    let f = r.render(RenderParams { scene: &scene, camera: &cam, width: w, height: h, device_pixel_ratio: 1.0, style: &Style::default(), pick: true }).unwrap();
    let pick = f.pick.unwrap();
    let at = |x: f32| cam.project(glam::DVec3::new(x as f64, -0.01, 0.0), glam::DVec2::new(w as f64, h as f64), 0.1).expect("on screen");
    let px = |p: glam::DVec2| {
        let o = ((p.y as u32 * w + p.x as u32) * 4) as usize;
        (pick.ids[(p.y as u32 * w + p.x as u32) as usize], [f.bgra[o + 2], f.bgra[o + 1], f.bgra[o]])
    };
    let (left, mid, right) = (px(at(xs[0])), px(at(xs[1])), px(at(xs[2])));
    assert!((1..=6).contains(&left.0), "left copy picks its own faces: {}", left.0);
    assert!((101..=106).contains(&mid.0), "middle copy picks its own faces: {}", mid.0);
    assert_eq!(right.0, 0, "the hidden copy is not picked");
    assert_ne!(left.1, mid.1, "the selected copy is highlighted, the other not");
    assert!(right.1.iter().all(|&c| c > 240), "the hidden copy is not drawn: {:?}", right.1);
}

/// Section caps hatch as Onshape's endcap shader does (webgl bundle:
/// getColorHash, getHatchingSmoothstep, uSectionPlanePatternScale =
/// SECTION_PLANE_ORTHOGRAPHIC_PATTERN_SCALE 30 / (P[5] · viewport height)):
/// lines across the direction (cos h, sin h) of the plane's frame, h the
/// hash of the part's sRGB color (offset by its id, 0 here: Onshape's
/// "predictable hatching"), every 15 · pace device pixels, pace = mix(1,
/// 1.75², fract(h)) / 1.75; between them the color at 90% (10% of plane 0's
/// black selector color); a black outline along the cap's border. A box cut
/// at x = 0 (tangent -Y, Onshape's for a cut parallel to Right), seen head
/// on, orthographic, at 1x and 2x.
#[test]
fn section_cap_hatch_follows_onshape() {
    let _gpu = GPU.lock().unwrap_or_else(|e| e.into_inner());
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    // h = 700 + 550.8 + 0.4: the lines ~49° off the frame's axes (so a
    // mirrored pattern fails), and a 16-bit mask's rounding (±0.00025 at
    // 0.5..1, ×1000 in the hash) would turn them visibly: Onshape's mask is
    // 32-bit float.
    let srgb = [0.70f32, 0.5508, 0.40];
    let lin = srgb.map(|c| if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) });
    let mut scene = Scene::default();
    scene.instances.push(Instance::new(Drawable::Mesh(r.upload_mesh(&primitives::box_mesh([-0.02, -0.01, -0.005], [0.02, 0.01, 0.005], 1)))).with_color([lin[0], lin[1], lin[2], 1.0]));
    scene.clip = Some(ClipPlane::new(glam::Vec3::X, 0.0));
    let plane = scene.clip.unwrap();
    assert!((plane.tangent - glam::Vec3::NEG_Y).length() < 1e-6, "Onshape's frame for a cut parallel to Right: {:?}", plane.tangent);
    // Onshape's hash and pace (GLSL mod: x - y floor(x / y)).
    let m = |x: f32, y: f32| x - y * (x / y).floor();
    let h = m(srgb[0] * 1000.0, 1000.0) + m(srgb[1] * 1000.0, 1000.0) + m(srgb[2] + 1000.0, 1000.0);
    let pace = (1.0 + (1.75f32 * 1.75 - 1.0) * h.fract()) / 1.75;
    for dpr in [1.0f32, 2.0] {
        let (w, h_px) = ((400.0 * dpr) as u32, (300.0 * dpr) as u32);
        let mut cam = Camera { rotation: Camera::look_rotation(glam::DVec3::NEG_X, glam::DVec3::Z), projection: Projection::Orthographic, ..Default::default() };
        cam.fit_box(glam::DVec3::new(-0.001, -0.012, -0.007), glam::DVec3::new(0.001, 0.012, 0.007), w as f64 / h_px as f64);
        let f = r.render(RenderParams { scene: &scene, camera: &cam, width: w, height: h_px, device_pixel_ratio: dpr, style: &Style::default(), pick: false }).unwrap();
        if let Ok(dir) = std::env::var("NOSHAPE_TEST_OUT") {
            let mut rgba = f.bgra.clone();
            for px in rgba.as_chunks_mut::<4>().0 {
                px.swap(0, 2);
            }
            image::save_buffer(format!("{dir}/cap_hatch_{dpr}x.png"), &rgba, w, h_px, image::ExtendedColorType::Rgba8).unwrap();
        }
        let lum = |x: f32, y: f32| -> f32 {
            let (x, y) = (x.round().clamp(0.0, w as f32 - 1.0) as u32, y.round().clamp(0.0, h_px as f32 - 1.0) as u32);
            let p = &f.bgra[((y * w + x) * 4) as usize..][..3];
            p[0] as f32 + p[1] as f32 + p[2] as f32
        };
        // Screen axes: right = +Y, down = -Z; the frame's u = -Y, v = n × u = -Z.
        // The pattern varies along (cos h, sin h) in (u, v): in screen
        // pixels along g = (-cos h, sin h).
        let g = glam::Vec2::new(-h.cos(), h.sin());
        let along = glam::Vec2::new(-g.y, g.x);
        let c = glam::Vec2::new(w as f32 / 2.0, h_px as f32 / 2.0);
        let span = 60.0 * dpr;
        let profile: Vec<f32> = (0..(2.0 * span) as i32).map(|k| {
            let p = c + g * (k as f32 - span);
            lum(p.x, p.y)
        }).collect();
        // Along the lines little changes, across them a lot (the direction
        // is Onshape's; a mirrored or rotated pattern changes along it too).
        let energy = |dir: glam::Vec2| -> f32 {
            let mut e = 0.0;
            for j in -20..=20 {
                for i in -20..=20 {
                    let p = c + glam::Vec2::new(i as f32, j as f32) * 2.0 * dpr;
                    let q = p + dir * 2.0;
                    e += (lum(q.x, q.y) - lum(p.x, p.y)).powi(2);
                }
            }
            e
        };
        let (e_along, e_across) = (energy(along), energy(g));
        assert!(e_along < 0.2 * e_across, "{dpr}x: the hatch lines don't run along Onshape's direction (change along {e_along}, across {e_across})");
        // Across them, the period (autocorrelation peak) is 15 · pace px.
        let mean = profile.iter().sum::<f32>() / profile.len() as f32;
        let d: Vec<f32> = profile.iter().map(|v| v - mean).collect();
        let ac = |lag: usize| d.iter().zip(&d[lag..]).map(|(a, b)| a * b).sum::<f32>();
        let want = 15.0 * pace * dpr;
        let best = ((want * 0.6) as usize..(want * 1.5) as usize).max_by(|a, b| ac(*a).total_cmp(&ac(*b))).unwrap() as f32;
        assert!((best - want).abs() <= 0.08 * want + 1.0, "{dpr}x: hatch period {best} px, Onshape's {want} px");
        // Between lines: the part color at 90%.
        let light = profile.iter().cloned().fold(f32::MIN, f32::max);
        let want_light = srgb.iter().map(|c| c * 0.9 * 255.0).sum::<f32>();
        assert!((light - want_light).abs() < 9.0, "{dpr}x: cap color {light}, want {want_light}");
        // A black outline along the border (the cap's edge at the box's top, z = 5 mm).
        let top = cam.project(glam::DVec3::new(0.0, 0.0, 0.005), glam::DVec2::new(w as f64, h_px as f64), 0.02).unwrap();
        let darkest = (-3..=3).map(|s| lum(top.x as f32, top.y as f32 + s as f32)).fold(f32::MAX, f32::min);
        assert!(darkest < 200.0, "{dpr}x: no outline at the cap's border (darkest {darkest})");
    }
}

/// Versions and history › Compare (live "Compare mode graphics", Onshape's
/// compare pass): a box in both versions, a boss only in the base, another
/// only in the target. Common surfaces are lit in the "same" white and stay
/// put at any blend; base-only ones are blue, target-only ones red-orange;
/// base-only geometry shows fully up to blend 0.5 and fades out to 1,
/// target-only geometry fades in to 0.5.
#[test]
fn compare_blend_follows_onshape() {
    let _gpu = GPU.lock().unwrap_or_else(|e| e.into_inner());
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let mut scene = Scene::default();
    let boxm = r.upload_mesh(&primitives::box_mesh([-0.02, -0.01, 0.0], [0.02, 0.01, 0.01], 1));
    let blue = [0.45f32, 0.6, 0.75, 1.0];
    for layer in [compare::BASE, compare::TARGET] {
        scene.instances.push(Instance::new(Drawable::Mesh(boxm.clone())).with_color(blue).compare_layer(layer));
    }
    // Bosses on the top: base-only at x < 0, target-only at x > 0.
    scene.instances.push(Instance::new(Drawable::Mesh(r.upload_mesh(&primitives::box_mesh([-0.015, -0.004, 0.01], [-0.007, 0.004, 0.02], 20)))).with_color(blue).compare_layer(compare::BASE));
    scene.instances.push(Instance::new(Drawable::Mesh(r.upload_mesh(&primitives::box_mesh([0.007, -0.004, 0.01], [0.015, 0.004, 0.02], 30)))).with_color(blue).compare_layer(compare::TARGET));
    let mut cam = Camera { rotation: Camera::look_rotation(glam::DVec3::new(0.0, 0.6, -1.0).normalize(), glam::DVec3::Z), projection: Projection::Orthographic, ..Default::default() };
    let (w, h) = (400u32, 300u32);
    cam.fit_box(glam::DVec3::new(-0.025, -0.012, 0.0), glam::DVec3::new(0.025, 0.012, 0.02), w as f64 / h as f64);
    // Pixels on the bosses' tops and on the box's top between them.
    let at = |p: [f64; 3]| cam.project(glam::DVec3::from_array(p), glam::DVec2::new(w as f64, h as f64), 0.05).unwrap();
    let (base_boss, target_boss, common) = (at([-0.011, 0.0, 0.02]), at([0.011, 0.0, 0.02]), at([0.0, 0.006, 0.01]));
    let render = |r: &mut Renderer, blend: f32| {
        let mut s = scene.clone();
        s.compare = Some(Compare { blend });
        let f = r.render(RenderParams { scene: &s, camera: &cam, width: w, height: h, device_pixel_ratio: 1.0, style: &Style::default(), pick: false }).unwrap();
        let px = |p: glam::DVec2| {
            let o = ((p.y as u32 * w + p.x as u32) * 4) as usize;
            [f.bgra[o + 2] as f32, f.bgra[o + 1] as f32, f.bgra[o] as f32]
        };
        (px(base_boss), px(target_boss), px(common))
    };
    let blends = [0.0f32, 0.25, 0.5, 0.75, 1.0];
    let shots: Vec<_> = blends.iter().map(|b| render(&mut r, *b)).collect();
    let near = |a: [f32; 3], b: [f32; 3], tol: f32| a.iter().zip(&b).all(|(x, y)| (x - y).abs() <= tol);
    // Common geometry: white lit (grey), the same at every blend.
    for (b, s) in blends.iter().zip(&shots) {
        assert!(near(s.2, shots[0].2, 3.0), "blend {b}: common top {:?} vs {:?}", s.2, shots[0].2);
        assert!((s.2[0] - s.2[2]).abs() < 4.0, "blend {b}: common top not grey: {:?}", s.2);
    }
    let (b0, b_mid, t1) = (shots[0].0, shots[2].0, shots[4].1);
    assert!(b0[2] > b0[0] + 40.0, "base-only boss is blue: {b0:?}");
    assert!(t1[0] > t1[2] + 60.0, "target-only boss is red-orange: {t1:?}");
    // Base-only: full up to 0.5, gone at 1. Behind it then: the target's
    // box top, which the base doesn't have there (the boss covers it), so
    // red-orange (live: "the box top under the boss exists only in V3, so it
    // is red when V3 is the target and blue when V3 is the base").
    assert!(near(shots[1].0, b0, 3.0) && near(b_mid, b0, 3.0), "base-only fades before 0.5: {:?} {:?}", shots[1].0, b_mid);
    assert!(shots[4].0[0] > shots[4].0[2] + 40.0, "at 1 the target's box top under the base boss: {:?}", shots[4].0);
    // At 0.75 halfway between full and gone (plain alpha over).
    let half: Vec<f32> = (0..3).map(|i| 0.5 * (b0[i] + shots[4].0[i])).collect();
    assert!(near(shots[3].0, [half[0], half[1], half[2]], 6.0), "base-only at 0.75: {:?}, want {half:?}", shots[3].0);
    // Target-only: gone at 0 (the base's box top under it is base-only:
    // blue), half at 0.25, full from 0.5.
    assert!(shots[0].1[2] > shots[0].1[0] + 40.0, "at 0 the base's box top under the target boss: {:?}", shots[0].1);
    assert!(near(shots[2].1, t1, 3.0) && near(shots[3].1, t1, 3.0), "target-only not full from 0.5: {:?} {:?}", shots[2].1, shots[3].1);
    let half: Vec<f32> = (0..3).map(|i| 0.5 * (t1[i] + shots[0].1[i])).collect();
    assert!(near(shots[1].1, [half[0], half[1], half[2]], 6.0), "target-only at 0.25: {:?}, want {half:?}", shots[1].1);
}

/// Label quads and shadows are drawn like GPUI draws a div with
/// `.rounded(4).border_1().shadow_xs()` (overlay.wgsl `fs_quad` /
/// `fs_shadow`): solid background inside, the full border color on the
/// straight border, the rounded corner left out, a faint shadow below;
/// and the pointer finds the label where it is drawn (`label_at`).
#[test]
fn label_quads_and_shadows_follow_gpui() {
    use noshape_render::overlay::{Label, Overlay, Quad, Shadow};
    let _gpu = GPU.lock().unwrap_or_else(|e| e.into_inner());
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let scene = std::sync::Arc::new(Scene::default());
    let cam = Camera::default();
    let (w, h) = (400u32, 300u32);
    let req = || FrameRequest { scene: scene.clone(), camera: cam, width: w, height: h, device_pixel_ratio: 1.0, style: Style::default(), invalidate_pick: false };
    let frame = |r: &mut Renderer, seq: u64| {
        r.submit_frame(8, seq, req()).unwrap();
        loop {
            r.wait_oldest();
            if let Some(Completed::Frame { result, .. }) = r.collect().into_iter().next() {
                return result.unwrap();
            }
        }
    };
    let plain = frame(&mut r, 1);
    let (bg, border) = ([0xd9 as f32 / 255.0, 0xa5 as f32 / 255.0, 0x66 as f32 / 255.0, 1.0], [1.0, 0xc2 as f32 / 255.0, 0x0e as f32 / 255.0, 1.0]);
    let label = Label {
        anchor: cam.target,
        offset: [-40.0, -11.0],
        quads: vec![Quad { bounds: [0.0, 0.0, 80.0, 22.0], corner_radius: 4.0, background: bg, border_width: 1.0, border_color: border }],
        shadows: vec![Shadow { bounds: [0.0, 1.0, 80.0, 23.0], corner_radius: 4.0, blur_radius: 2.0, color: [0.0, 0.0, 0.0, 0.05] }],
        hit: Some(7),
        ..Default::default()
    };
    let overlay = Overlay { labels: vec![label], ..Default::default() };
    r.set_overlay(8, Some(std::sync::Arc::new(overlay.clone())));
    let with = frame(&mut r, 2);
    let px = |f: &Frame, x: u32, y: u32| -> [i32; 3] {
        let o = ((y * w + x) * 4) as usize;
        [f.bgra[o + 2] as i32, f.bgra[o + 1] as i32, f.bgra[o] as i32]
    };
    let (x0, y0) = (w / 2 - 40, h / 2 - 11);
    let near = |a: [i32; 3], b: [i32; 3]| a.iter().zip(&b).all(|(p, q)| (p - q).abs() <= 1);
    assert!(near(px(&with, x0 + 40, y0 + 11), [0xd9, 0xa5, 0x66]), "inside {:?}", px(&with, x0 + 40, y0 + 11));
    assert!(near(px(&with, x0 + 40, y0), [0xff, 0xc2, 0x0e]), "top border {:?}", px(&with, x0 + 40, y0));
    assert!(near(px(&with, x0, y0 + 11), [0xff, 0xc2, 0x0e]), "left border {:?}", px(&with, x0, y0 + 11));
    // The corner pixel is outside the 4 px rounding: (almost) the frame.
    let (c, p) = (px(&with, x0, y0), px(&plain, x0, y0));
    assert!(c.iter().zip(&p).all(|(a, b)| (a - b).abs() <= 12), "corner {c:?} vs frame {p:?}");
    // The shadow darkens the row under the box a little, nothing far away.
    let (s, p) = (px(&with, x0 + 40, y0 + 22), px(&plain, x0 + 40, y0 + 22));
    assert!(s.iter().zip(&p).all(|(a, b)| b - a >= 1 && b - a <= 12), "shadow {s:?} vs frame {p:?}");
    assert_eq!(px(&with, x0 + 40, y0 + 40), px(&plain, x0 + 40, y0 + 40));
    // Hit testing with the frame's camera.
    let view_proj = cam.projection_matrix(w as f64 / h as f64, 1.0) * cam.view_matrix();
    let logical = glam::DVec2::new(w as f64, h as f64);
    assert_eq!(overlay.label_at(&view_proj, logical, 1.0, glam::DVec2::new(w as f64 / 2.0, h as f64 / 2.0)), Some(7));
    assert_eq!(overlay.label_at(&view_proj, logical, 1.0, glam::DVec2::new(w as f64 / 2.0 + 41.0, h as f64 / 2.0)), None);
}

/// Faces nothing hides or clips are drawn by the discard-free pipeline
/// (`mesh-solid`); the same parts drawn by the discarding one (a hidden
/// pick id the mesh doesn't use moves it there) give the same pixels.
#[test]
fn faces_without_discard_match_the_discarding_pipeline() {
    let _gpu = GPU.lock().unwrap_or_else(|e| e.into_inner());
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let (w, h) = (320u32, 240u32);
    // Box A's faces use ids 1, 3..6 (2 unused, inside its range); box B
    // behind it, overlapping on screen.
    let mut a = primitives::box_mesh([-0.05, -0.05, -0.05], [0.05, 0.05, 0.05], 1);
    for id in &mut a.pick_ids {
        if *id == 2 {
            *id = 3;
        }
    }
    let b = primitives::box_mesh([-0.02, -0.12, -0.12], [0.08, -0.02, -0.02], 1);
    let mut scene = Scene::default();
    scene.instances.push(Instance::new(Drawable::Mesh(r.upload_mesh(&a))).with_color([0.3, 0.5, 0.8, 1.0]));
    scene.instances.push(Instance::new(Drawable::Mesh(r.upload_mesh(&b))).with_color([0.8, 0.4, 0.2, 1.0]).pick_base(10));
    scene.states = vec![0; 20];
    scene.states[1] = state::SELECTED;
    scene.states[13] = state::SELECTED;
    let mut cam = Camera::default();
    cam.set_view(StandardView::Isometric);
    cam.fit_box(glam::DVec3::splat(-0.12), glam::DVec3::splat(0.08), w as f64 / h as f64);
    let mut shot = |scene: &Scene| r.render(RenderParams { scene, camera: &cam, width: w, height: h, device_pixel_ratio: 1.0, style: &Style::default(), pick: false }).unwrap().bgra;
    let solid = shot(&scene);
    scene.states[2] = state::HIDDEN;
    let discarding = shot(&scene);
    let drawn = solid.chunks(4).filter(|p| p[..3] != discarding.chunks(4).next().unwrap()[..3]).count();
    let differ = solid.chunks(4).zip(discarding.chunks(4)).filter(|(x, y)| x != y).count();
    eprintln!("pixels differing {differ} of {}, {drawn} unlike the corner", solid.len() / 4);
    assert!(drawn > 1000, "both boxes drawn");
    assert_eq!(differ, 0);
}

/// `Instance::on_top`: a translucent mesh inside an opaque box shows through
/// it (FeatureScript debug faces behind the model), while an ordinary
/// translucent one stays hidden.
#[test]
fn on_top_meshes_show_through_the_model() {
    let _gpu = GPU.lock().unwrap_or_else(|e| e.into_inner());
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let outer = r.upload_mesh(&primitives::box_mesh([-0.01, -0.01, -0.01], [0.01, 0.01, 0.01], 1));
    let inner = r.upload_mesh(&primitives::box_mesh([-0.003, -0.003, -0.003], [0.003, 0.003, 0.003], 2));
    let cam = iso_camera();
    let style = Style::default();
    // (red, green) at the centre, the inner box drawn on top or not.
    let mut centre = |on_top: bool| {
        let mut scene = Scene::default();
        scene.instances.push(Instance::new(Drawable::Mesh(outer.clone())).with_color([0.5, 0.5, 0.5, 1.0]));
        let red = Instance::new(Drawable::Mesh(inner.clone())).with_color([1.0, 0.0, 0.0, 0.5]).unlit();
        scene.instances.push(if on_top { red.on_top() } else { red });
        let f = r.render(RenderParams { scene: &scene, camera: &cam, width: 400, height: 300, device_pixel_ratio: 1.0, style: &style, pick: false }).unwrap();
        let px = &f.bgra[(150 * 400 + 200) * 4..][..4];
        (px[2] as i32, px[1] as i32)
    };
    let (hidden_r, hidden_g) = centre(false);
    let (shown_r, shown_g) = centre(true);
    assert!((hidden_r - hidden_g).abs() < 30, "hidden: ({hidden_r}, {hidden_g})");
    assert!(shown_r > shown_g + 60, "on top: ({shown_r}, {shown_g})");
}
