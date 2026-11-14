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

/// EDGE-HIGHLIGHT-LOOK (user, macOS Retina): a large thin plate in iso showed a solid black wedge past its outline,
/// a back face thrown across the screen by the image-based silhouette pass (its push divided by a finite step's
/// on-screen length, which can come out as nothing). Nothing but the plate and its ~1 px silhouette may cover the
/// background, at 1× and 2×.
#[test]
fn a_thin_plate_draws_nothing_past_its_outline() {
    let _gpu = GPU.lock().unwrap_or_else(|e| e.into_inner());
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let (lo, hi) = ([0.0f32, 0.0, 0.0], [0.6f32, 0.4, 0.004]);
    let mesh = r.upload_mesh(&primitives::box_mesh(lo, hi, 1));
    let mut scene = Scene::default();
    scene.instances.push(Instance::new(Drawable::Mesh(mesh.clone())));
    // As the scene builder draws a part: its faces, then the same mesh as image-based silhouettes in the edge colour.
    scene.instances.push(Instance::new(Drawable::Mesh(mesh)).with_color([0.02, 0.02, 0.03, 1.0]).back_silhouette());
    let mut cam = Camera::default();
    cam.set_view(StandardView::Isometric);
    for dpr in [1.0f32, 2.0] {
        let (w, h) = ((800.0 * dpr) as u32, (600.0 * dpr) as u32);
        // Zoomed in on the near corner, like the user's view.
        cam.fit_box(glam::DVec3::new(0.3, 0.0, 0.0), glam::DVec3::new(0.6, 0.15, 0.004), w as f64 / h as f64);
        let f = r.render(RenderParams { scene: &scene, camera: &cam, width: w, height: h, device_pixel_ratio: dpr, style: &Style::default(), pick: false }).unwrap();
        // The plate's outline on screen: the convex hull of its corners.
        let vp = glam::DVec2::new(w as f64, h as f64);
        let mut pts: Vec<glam::DVec2> = (0..8)
            .filter_map(|k| cam.project(glam::DVec3::new(if k & 1 == 0 { lo[0] } else { hi[0] } as f64, if k & 2 == 0 { lo[1] } else { hi[1] } as f64, if k & 4 == 0 { lo[2] } else { hi[2] } as f64), vp, 1.0))
            .collect();
        pts.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
        let cross = |o: glam::DVec2, a: glam::DVec2, b: glam::DVec2| (a - o).perp_dot(b - o);
        let mut hull: Vec<glam::DVec2> = Vec::new();
        for pass in 0..2 {
            let start = hull.len();
            let it: Box<dyn Iterator<Item = &glam::DVec2>> = if pass == 0 { Box::new(pts.iter()) } else { Box::new(pts.iter().rev()) };
            for &p in it {
                while hull.len() >= start + 2 && cross(hull[hull.len() - 2], hull[hull.len() - 1], p) <= 0.0 {
                    hull.pop();
                }
                hull.push(p);
            }
            hull.pop();
        }
        let outside = |q: glam::DVec2| -> f64 {
            // Distance outside the (counter-clockwise) hull, 0 inside.
            let mut d: f64 = 0.0;
            for i in 0..hull.len() {
                let (a, b) = (hull[i], hull[(i + 1) % hull.len()]);
                let e = b - a;
                let side = e.perp_dot(q - a) / e.length();
                d = d.min(side);
            }
            -d
        };
        let mut stray = 0;
        for y in 0..h {
            for x in 0..w {
                let o = ((y * w + x) * 4) as usize;
                let lum = f.bgra[o] as u32 + f.bgra[o + 1] as u32 + f.bgra[o + 2] as u32;
                if lum < 600 && outside(glam::DVec2::new(x as f64 + 0.5, y as f64 + 0.5)) > 3.0 * dpr as f64 {
                    stray += 1;
                }
            }
        }
        assert!(stray < 20, "{stray} pixels drawn past the plate's outline at {dpr}x");
    }
}

/// SIL-AXIS-VIEW-GONE: in an exact standard view a curved face's facet can be exactly edge-on at the outline; the
/// tessellation edges round it pair that facet (facing neither way) with one facing away, and are the silhouette.
/// `dot(n1, v) * dot(n2, v) >= 0` dropped them, so a cylinder lost its side lines in Front view.
#[test]
fn a_silhouette_beside_an_edge_on_facet_is_drawn() {
    let _gpu = GPU.lock().unwrap_or_else(|e| e.into_inner());
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let (w, h) = (200u32, 150u32);
    let mut cam = Camera::default();
    cam.set_view(StandardView::Front);
    cam.fit_sphere(glam::DVec3::ZERO, 0.02, w as f64 / h as f64);
    let mut dark = |n2: [f32; 3]| {
        let seg = SilhouetteSegment { a: [0.0, 0.0, -0.01], b: [0.0, 0.0, 0.01], n1: [1.0, 0.0, 0.0], n2, pick_id: 0, width: 1.25 };
        let lines = r.upload_silhouettes(&SilhouetteData { segments: vec![seg] });
        let scene = Scene { instances: vec![Instance::new(Drawable::Lines(lines)).with_color([0.0, 0.0, 0.0, 1.0])], ..Default::default() };
        let f = r.render(RenderParams { scene: &scene, camera: &cam, width: w, height: h, device_pixel_ratio: 1.0, style: &Style::default(), pick: false }).unwrap();
        (0..w).map(|x| f.bgra[((h / 2 * w + x) * 4) as usize]).min().unwrap()
    };
    // Front view looks along +Y: n1 = +X is edge-on; the other facet turned away (+Y) or toward the viewer (-Y).
    let s = std::f32::consts::FRAC_1_SQRT_2;
    assert!(dark([s, s, 0.0]) < 100, "beside a facet turned away");
    assert!(dark([s, -s, 0.0]) < 100, "beside a facet turned toward the viewer");
    // Both facets facing the viewer: no silhouette.
    let mut r2 = r;
    let seg = SilhouetteSegment { a: [0.0, 0.0, -0.01], b: [0.0, 0.0, 0.01], n1: [s, -s, 0.0], n2: [-s, -s, 0.0], pick_id: 0, width: 1.25 };
    let lines = r2.upload_silhouettes(&SilhouetteData { segments: vec![seg] });
    let scene = Scene { instances: vec![Instance::new(Drawable::Lines(lines)).with_color([0.0, 0.0, 0.0, 1.0])], ..Default::default() };
    let f = r2.render(RenderParams { scene: &scene, camera: &cam, width: w, height: h, device_pixel_ratio: 1.0, style: &Style::default(), pick: false }).unwrap();
    assert!((0..w).all(|x| f.bgra[((h / 2 * w + x) * 4) as usize] > 200), "both facets toward the viewer: nothing");
}

/// EDGE-ON-PLANE-PICK: a sketch line lying in a datum plane seen edge-on picks (live: "Edge of Sketch 1", the
/// plane's edge line never takes the hover). The plane's border is drawn first in the pick pass without depth
/// writes, as its fill: its near side lay in front of the line and hid it.
#[test]
fn a_line_in_an_edge_on_plane_picks_over_the_plane() {
    let _gpu = GPU.lock().unwrap_or_else(|e| e.into_inner());
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let (w, h) = (200u32, 150u32);
    let mut cam = Camera::default();
    cam.set_view(StandardView::Front);
    cam.fit_sphere(glam::DVec3::ZERO, 0.03, w as f64 / h as f64);
    // The Right plane (x = 0), edge-on in Front view: fill and border, pick id 1.
    let e = 0.02f32;
    let corners = [[0.0, -e, -e], [0.0, e, -e], [0.0, e, e], [0.0, -e, e]];
    let fill = MeshData { positions: corners.to_vec(), normals: vec![[1.0, 0.0, 0.0]; 4], pick_ids: vec![1; 4], indices: vec![0, 1, 2, 0, 2, 3] };
    let mut border = LineData::default();
    border.push_polyline(&[corners[0], corners[1], corners[2], corners[3], corners[0]], [0, 0, 0, 0], 1, 2.0, 0);
    // The sketch line in it, from z = 0 up, pick id 2.
    let mut line = LineData::default();
    line.push_polyline(&[[0.0, 0.0, 0.0], [0.0, 0.0, 0.015]], [0, 0, 0, 255], 2, 1.5, 0);
    let scene = Scene {
        instances: vec![
            Instance::new(Drawable::Mesh(r.upload_mesh(&fill))).with_color([0.8, 0.85, 0.9, 0.2]).unlit().layer(5),
            Instance::new(Drawable::Lines(r.upload_lines(&border))).with_color([0.3, 0.5, 0.8, 0.35]).pick_behind().layer(5),
            Instance::new(Drawable::Lines(r.upload_lines(&line))).with_color([0.0, 0.0, 0.0, 1.0]).layer(10),
        ],
        ..Default::default()
    };
    let f = r.render(RenderParams { scene: &scene, camera: &cam, width: w, height: h, device_pixel_ratio: 1.0, style: &Style::default(), pick: true }).unwrap();
    let pick = f.pick.unwrap();
    let vp = glam::DVec2::new(w as f64, h as f64);
    let at = |z: f64| {
        let s = cam.project(glam::DVec3::new(0.0, 0.0, z), vp, 0.03).unwrap();
        pick.id_at(s.x as i32, s.y as i32).unwrap_or(0)
    };
    assert_eq!(at(0.0075), 2, "the middle of the line picks the line");
    assert_eq!(at(-0.01), 1, "below the line the plane's edge picks the plane");
}

/// A point picks around its drawn centre, 1 px narrower in CSS px (at most 5 px), at a pixel ratio of 1 and 2 alike (the
/// user's Retina: a margin multiplied by the ratio twice made a 6 px sketch point pick 11 px round itself, a
/// neighbouring point won the hover and the click, and a snap shown on hover didn't commit).
#[test]
fn a_point_picks_round_its_drawn_centre_at_any_pixel_ratio() {
    let _gpu = GPU.lock().unwrap_or_else(|e| e.into_inner());
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let (w, h) = (200u32, 150u32);
    let mut cam = Camera::default();
    cam.set_view(StandardView::Top);
    cam.fit_sphere(glam::DVec3::ZERO, 0.03, w as f64 / h as f64);
    let p = [0.004f32, 0.003, 0.0];
    let points = PointData { points: vec![PointSprite { position: p, color: [0, 0, 255, 255], pick_id: 7, size: 6.0, style: LineSegment::ON_TOP, _pad: 0 }] };
    let scene = Scene { instances: vec![Instance::new(Drawable::Points(r.upload_points(&points))).per_primitive_color().layer(20)], ..Default::default() };
    for dpr in [1.0f32, 2.0] {
        let (dw, dh) = ((w as f32 * dpr) as u32, (h as f32 * dpr) as u32);
        let f = r.render(RenderParams { scene: &scene, camera: &cam, width: dw, height: dh, device_pixel_ratio: dpr, style: &Style::default(), pick: true }).unwrap();
        let pick = f.pick.unwrap();
        let c = cam.project(glam::DVec3::new(p[0] as f64, p[1] as f64, 0.0), glam::DVec2::new(dw as f64, dh as f64), 0.03).unwrap();
        // CSS px to the right of the drawn centre.
        let at = |css: f64| pick.id_at((c.x + css * dpr as f64) as i32, c.y as i32).unwrap_or(0);
        assert_eq!(at(0.0), 7, "dpr {dpr}: the drawn centre");
        assert_eq!(at(1.0), 7, "dpr {dpr}: within its pick radius (2.5 px)");
        assert_eq!(at(3.0), 0, "dpr {dpr}: past it");
    }
}

/// Our 3D pick reach outside sketch mode, as the viewport picks (in the square of PICK_RADIUS 5 CSS px round the
/// pointer, vertices before edges before faces, then the nearest), measured on the pick buffer at a pixel ratio of 1
/// and 2, against live/feel/3d.json: a part edge (drawn 1 px, picked 3 px wide) lights from 6–7 px off it on either
/// side; a vertex (a 6 px disc, picked 5 px wide) from 7 px across, 9 diagonally, and
/// beats the edge it ends. docs/parity/interaction-feel.md.
#[test]
fn our_3d_pick_reach_for_edges_and_vertices() {
    let _gpu = GPU.lock().unwrap_or_else(|e| e.into_inner());
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let (w, h) = (300u32, 200u32);
    let (lo, hi) = ([-0.02f32, -0.01, -0.005], [0.02f32, 0.01, 0.005]);
    let mut cam = Camera::default();
    cam.set_view(StandardView::Top);
    cam.fit_sphere(glam::DVec3::ZERO, 0.05, w as f64 / h as f64);
    // The top face's front edge (y = lo, z = hi) as the builder draws a part edge, and its left end's vertex.
    let mut edge = LineData::default();
    edge.push_polyline(&[[lo[0], lo[1], hi[2]], [hi[0], lo[1], hi[2]]], [0, 0, 0, 255], 50, 1.0, LineSegment::CORE);
    let vertex = PointData { points: vec![PointSprite { position: [lo[0], lo[1], hi[2]], color: [255; 4], pick_id: 60, size: 6.0, style: 0, _pad: 0 }] };
    let scene = Scene {
        instances: vec![
            Instance::new(Drawable::Mesh(r.upload_mesh(&primitives::box_mesh(lo, hi, 1)))).with_color([0.6, 0.7, 0.8, 1.0]),
            Instance::new(Drawable::Lines(r.upload_lines(&edge))).with_color([0.0, 0.0, 0.0, 1.0]).layer(10),
            Instance::new(Drawable::Points(r.upload_points(&vertex))).per_primitive_color().layer(11),
        ],
        ..Default::default()
    };
    // Priorities as `PickMap::priority_fn` gives them (×2): vertex 10, edge 8, face 2.
    let prio = |id: u32| match id {
        60 => 10,
        50 => 8,
        0 => -1,
        _ => 2,
    };
    for dpr in [1.0f32, 2.0] {
        let (dw, dh) = ((w as f32 * dpr) as u32, (h as f32 * dpr) as u32);
        let f = r.render(RenderParams { scene: &scene, camera: &cam, width: dw, height: dh, device_pixel_ratio: dpr, style: &Style::default(), pick: true }).unwrap();
        let pick = f.pick.unwrap();
        let size = glam::DVec2::new(dw as f64, dh as f64);
        let screen = |p: [f32; 3]| cam.project(glam::DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64), size, 0.05).unwrap();
        let radius = (5.0 * dpr) as i32;
        let at = |q: glam::DVec2| pick.pick(q.x.round() as i32, q.y.round() as i32, radius, prio);
        // CSS px off the edge's middle, straight across it (outward over the background, inward over the top face).
        let mid = screen([0.0, lo[1], hi[2]]);
        let inward = (screen([0.0, 0.0, hi[2]]) - mid).normalize();
        let reach = |dir: glam::DVec2, id: u32| (0..40).take_while(|k| at(mid + dir * (*k as f64 * dpr as f64)) == Some(id)).count() as i32 - 1;
        let (out, inn) = (reach(-inward, 50), reach(inward, 50));
        assert!((6..=7).contains(&out) && (6..=7).contains(&inn), "dpr {dpr}: an edge lights from {out} px off it over the background, {inn} over its face");
        // The vertex straight out (left of the part) and along the diagonal away from it; it beats the edge where both
        // are in reach.
        let corner = screen([lo[0], lo[1], hi[2]]);
        let vertex_reach = |dir: glam::DVec2| (0..40).take_while(|k| at(corner + dir * (*k as f64 * dpr as f64)) == Some(60)).count() as i32 - 1;
        let along = (screen([hi[0], lo[1], hi[2]]) - corner).normalize();
        let (v, diag) = (vertex_reach(-along), vertex_reach((-along - inward).normalize()));
        assert_eq!(v, 7, "dpr {dpr}: a vertex lights from {v} px off it");
        assert!((8..=9).contains(&diag), "dpr {dpr}: a vertex lights from {diag} px off it diagonally");
        assert_eq!(at(corner + along * (6.0 * dpr as f64)), Some(60), "dpr {dpr}: on its edge 6 px from the vertex, the vertex wins");
        eprintln!("dpr {dpr}: edge {out} px out / {inn} px in, vertex {v} px ({diag} diagonally)");
    }
    // Hovered itself, the edge holds the hover to 9–10 px off it (live: a lit edge stays lit to 9–10).
    let mut scene = scene;
    scene.add_flag(50, noshape_render::scene::state::HOVER);
    for dpr in [1.0f32, 2.0] {
        let (dw, dh) = ((w as f32 * dpr) as u32, (h as f32 * dpr) as u32);
        let f = r.render(RenderParams { scene: &scene, camera: &cam, width: dw, height: dh, device_pixel_ratio: dpr, style: &Style::default(), pick: true }).unwrap();
        let pick = f.pick.unwrap();
        let size = glam::DVec2::new(dw as f64, dh as f64);
        let screen = |p: [f32; 3]| cam.project(glam::DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64), size, 0.05).unwrap();
        let radius = (5.0 * dpr) as i32;
        let at = |q: glam::DVec2| pick.pick(q.x.round() as i32, q.y.round() as i32, radius, prio);
        let mid = screen([0.0, lo[1], hi[2]]);
        let inward = (screen([0.0, 0.0, hi[2]]) - mid).normalize();
        let reach = |dir: glam::DVec2| (0..40).take_while(|k| at(mid + dir * (*k as f64 * dpr as f64)) == Some(50)).count() as i32 - 1;
        let (out, inn) = (reach(-inward), reach(inward));
        assert!((9..=10).contains(&out) && (9..=10).contains(&inn), "dpr {dpr}: a hovered edge holds to {out} px over the background, {inn} over its face");
    }
}

/// Overlapping datum planes pick the nearest (live ui-parity: Top where it lies in front of Front; ours took the
/// plane drawn last), a part behind the planes still wins over them, and its faces and edges pick as before.
#[test]
fn the_nearest_plane_wins_the_pick_and_parts_still_win_over_planes() {
    let _gpu = GPU.lock().unwrap_or_else(|e| e.into_inner());
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let (w, h) = (200u32, 150u32);
    let mut cam = Camera::default();
    cam.set_view(StandardView::Front);
    cam.fit_sphere(glam::DVec3::ZERO, 0.03, w as f64 / h as f64);
    // Two planes facing the viewer: id 1 in front (y = 0), id 2 behind it (y = 0.01), drawn last.
    let plane = |y: f32, id: u32| {
        let e = 0.02f32;
        MeshData { positions: vec![[-e, y, -e], [e, y, -e], [e, y, e], [-e, y, e]], normals: vec![[0.0, -1.0, 0.0]; 4], pick_ids: vec![id; 4], indices: vec![0, 1, 2, 0, 2, 3] }
    };
    // A part behind both planes on the right (a pick id per face from 10) with an edge (id 4) along its left side.
    let part = primitives::box_mesh([0.005, 0.02, -0.005], [0.015, 0.03, 0.005], 10);
    let mut edge = LineData::default();
    edge.push_polyline(&[[0.005, 0.02, -0.005], [0.005, 0.02, 0.005]], [0, 0, 0, 255], 4, 2.0, 0);
    let scene = Scene {
        instances: vec![
            Instance::new(Drawable::Mesh(r.upload_mesh(&plane(0.0, 1)))).with_color([0.8, 0.85, 0.9, 0.2]).unlit().layer(5),
            Instance::new(Drawable::Mesh(r.upload_mesh(&plane(0.01, 2)))).with_color([0.8, 0.85, 0.9, 0.2]).unlit().layer(5),
            Instance::new(Drawable::Mesh(r.upload_mesh(&part))).with_color([0.6, 0.7, 0.8, 1.0]),
            Instance::new(Drawable::Lines(r.upload_lines(&edge))).with_color([0.0, 0.0, 0.0, 1.0]).layer(10),
        ],
        ..Default::default()
    };
    let f = r.render(RenderParams { scene: &scene, camera: &cam, width: w, height: h, device_pixel_ratio: 1.0, style: &Style::default(), pick: true }).unwrap();
    let pick = f.pick.unwrap();
    let vp = glam::DVec2::new(w as f64, h as f64);
    let at = |x: f64, z: f64| {
        let s = cam.project(glam::DVec3::new(x, 0.0, z), vp, 0.03).unwrap();
        pick.id_at(s.x as i32, s.y as i32).unwrap_or(0)
    };
    assert_eq!(at(-0.01, 0.0), 1, "where the planes overlap the nearer one picks");
    assert!((10..16).contains(&at(0.011, 0.0)), "the part behind the planes picks its face: {}", at(0.011, 0.0));
    assert_eq!(at(0.005, 0.0), 4, "and its edge");
}

/// GPU-TEST-SEGV: renderers made and dropped on many threads at once (tests in one binary run in parallel) share
/// the process's one device; each renders its frame. With a Vulkan instance and device per renderer, lavapipe
/// crashed the process now and then inside libvulkan.
#[test]
fn renderers_on_many_threads_share_the_device() {
    if Renderer::new().is_err() {
        eprintln!("skipping: no GPU");
        return;
    }
    std::thread::scope(|s| {
        for _ in 0..8 {
            s.spawn(|| {
                for _ in 0..4 {
                    let mut r = Renderer::new().expect("the shared device");
                    let mut scene = Scene::default();
                    scene.instances.push(Instance::new(Drawable::Mesh(r.upload_mesh(&primitives::box_mesh([-0.01, -0.01, -0.01], [0.01, 0.01, 0.01], 1)))));
                    let f = r.render(RenderParams { scene: &scene, camera: &iso_camera(), width: 64, height: 48, device_pixel_ratio: 1.0, style: &Style::default(), pick: true }).unwrap();
                    assert_eq!(f.bgra.len(), 64 * 48 * 4);
                }
            });
        }
    });
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

/// Several section planes remove what any one of them removes (live 11-02: Top with Front, which alone cuts
/// nothing, still takes everything above Top away). A red block stands on a box, under a plane through the box.
#[test]
fn section_planes_each_remove_their_side() {
    let _gpu = GPU.lock().unwrap_or_else(|e| e.into_inner());
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let mut scene = Scene::default();
    scene.instances.push(Instance::new(Drawable::Mesh(r.upload_mesh(&primitives::box_mesh([-0.05, -0.03, -0.03], [0.05, 0.03, 0.03], 1)))).with_color([0.45, 0.6, 0.75, 1.0]));
    scene.instances.push(Instance::new(Drawable::Mesh(r.upload_mesh(&primitives::box_mesh([-0.01, -0.01, 0.03], [0.01, 0.01, 0.05], 100)))).with_color([1.0, 0.0, 0.0, 1.0]));
    let mut cam = Camera { rotation: Camera::look_rotation(glam::DVec3::NEG_Z, glam::DVec3::Y), ..Default::default() };
    let (w, h) = (400u32, 300u32);
    cam.fit_box(glam::DVec3::new(-0.05, -0.03, -0.03), glam::DVec3::new(0.05, 0.03, 0.05), w as f64 / h as f64);
    let mut red = |scene: &Scene| {
        let f = r.render(RenderParams { scene, camera: &cam, width: w, height: h, device_pixel_ratio: 1.0, style: &Style::default(), pick: false }).unwrap();
        let mut n = 0;
        for y in (h / 2 - 5)..(h / 2 + 5) {
            for x in (w / 2 - 5)..(w / 2 + 5) {
                let p = &f.bgra[((y * w + x) * 4) as usize..][..3];
                if p[2] as i32 > p[1] as i32 + 60 && p[2] as i32 > p[0] as i32 + 60 {
                    n += 1;
                }
            }
        }
        n
    };
    // Removes y < -0.1: nothing of either.
    scene.clip = Some(ClipPlane::new(glam::Vec3::NEG_Y, 0.1));
    assert!(red(&scene) > 80, "a plane past the model removed the block");
    // With z > 0.01 removed too, the block is gone.
    scene.clip_extra = vec![ClipPlane::new(glam::Vec3::Z, 0.01)];
    assert_eq!(red(&scene), 0, "the second plane's side is kept");
}

/// Where two parts' material meets on the cut the cap is red (Onshape's
/// interference colour) while `section_interference` is on (assemblies and
/// Part Studios); off, it stays hatched in the part colours.
#[test]
fn overlapping_caps_are_red_only_with_interference_on() {
    let _gpu = GPU.lock().unwrap_or_else(|e| e.into_inner());
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let mut scene = Scene::default();
    scene.instances.push(Instance::new(Drawable::Mesh(r.upload_mesh(&primitives::box_mesh([-0.05, -0.03, -0.03], [0.05, 0.03, 0.03], 1)))).with_color([0.45, 0.6, 0.75, 1.0]));
    // A pin through the block and the cut plane x = 0.
    scene.instances.push(Instance::new(Drawable::Mesh(r.upload_mesh(&primitives::box_mesh([-0.02, -0.01, -0.01], [0.02, 0.01, 0.01], 100)))).with_color([0.3, 0.3, 0.3, 1.0]));
    scene.clip = Some(ClipPlane::new(glam::Vec3::X, 0.0));
    let mut cam = Camera { rotation: Camera::look_rotation(glam::DVec3::new(-1.0, 0.0, 0.0), glam::DVec3::Z), ..Default::default() };
    let (w, h) = (400u32, 300u32);
    cam.fit_box(glam::DVec3::new(-0.05, -0.03, -0.03), glam::DVec3::new(0.05, 0.03, 0.03), w as f64 / h as f64);
    let mut reds = Vec::new();
    for interference in [false, true] {
        scene.section_interference = interference;
        let f = r.render(RenderParams { scene: &scene, camera: &cam, width: w, height: h, device_pixel_ratio: 1.0, style: &Style::default(), pick: false }).unwrap();
        let mut red = 0;
        for y in (h / 2 - 5)..(h / 2 + 5) {
            for x in (w / 2 - 5)..(w / 2 + 5) {
                let p = &f.bgra[((y * w + x) * 4) as usize..][..3];
                if p[2] as i32 > p[1] as i32 + 60 && p[2] as i32 > p[0] as i32 + 60 {
                    red += 1;
                }
            }
        }
        reds.push(red);
    }
    assert_eq!(reds[0], 0, "a Part Studio's overlap is red");
    assert!(reds[1] > 80, "an assembly's overlap isn't red ({} px)", reds[1]);
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

/// A section cap picks as its plane (Onshape: the caps stand for the section plane), where the plane cuts
/// material only: the face behind the cut beside the box picks as itself, empty space as nothing. A hovered
/// plane's caps mix 33 % toward SECTION_PLANE_PRE_HIGHLIGHT_COLOR #FFC685 (live 2255: #91bede → #b5c1c0).
#[test]
fn section_caps_pick_as_their_plane_and_hover_orange() {
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
    scene.clip = Some(ClipPlane::new(glam::Vec3::Z, 0.0));
    let mut cam = Camera { rotation: Camera::look_rotation(glam::DVec3::new(0.3, 1.0, -0.8).normalize(), glam::DVec3::Z), ..Default::default() };
    let (w, h) = (400u32, 300u32);
    cam.fit_box(glam::DVec3::splat(-0.05), glam::DVec3::splat(0.05), w as f64 / h as f64);
    let at = |p: glam::DVec3| cam.project(p, glam::DVec2::new(w as f64, h as f64), 0.2).expect("on screen");
    let cap = at(glam::DVec3::ZERO);
    let side = at(glam::DVec3::new(0.0, -0.05, -0.025));
    let render = |r: &mut Renderer, scene: &Scene| r.render(RenderParams { scene, camera: &cam, width: w, height: h, device_pixel_ratio: 1.0, style: &Style::default(), pick: true }).unwrap();
    let f = render(&mut r, &scene);
    let pick = f.pick.as_ref().unwrap();
    let id = |p: glam::DVec2| pick.ids[(p.y as u32 * w + p.x as u32) as usize];
    assert_eq!(section_cap_of_pick(id(cap)), Some(0), "the cap picks as plane 0's cap: {:#x}", id(cap));
    assert!(section_cap_of_pick(id(side)).is_none() && id(side) != 0, "the uncut side picks as the box: {:#x}", id(side));
    assert_eq!(id(glam::DVec2::new(3.0, 3.0)), 0, "empty space picks nothing");
    // The cap's light colour between hatch lines, before and after hovering.
    let light = |f: &noshape_render::Frame| {
        let mut best = [0u8; 3];
        for dy in -10..=10 {
            for dx in -10..=10 {
                let p = &f.bgra[(((cap.y as i32 + dy) as u32 * w + (cap.x as i32 + dx) as u32) * 4) as usize..][..3];
                if p.iter().map(|&c| c as u32).sum::<u32>() > best.iter().map(|&c| c as u32).sum::<u32>() {
                    best = [p[2], p[1], p[0]];
                }
            }
        }
        best
    };
    let plain = light(&f);
    scene.clip_hovered[0] = true;
    let hovered = light(&render(&mut r, &scene));
    for k in 0..3 {
        let want = plain[k] as f32 + 0.33 * ([255.0, 198.0, 133.0][k] - plain[k] as f32);
        assert!((hovered[k] as f32 - want).abs() <= 3.0, "hovered cap {hovered:?}, want {plain:?} 33 % toward #FFC685");
    }
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
/// black ground), lit as Onshape lights it: its top face reflects the sky, its
/// sides the ground. Locked by a camera below the box, the top reflects the
/// ground too.
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
    let below = Camera { rotation: Camera::look_rotation(glam::DVec3::new(-1.0, -1.0, 1.0).normalize(), glam::DVec3::Z), ..cam };
    let mut shot = |reflection: Option<Reflection>| {
        let mut scene = Scene::default();
        scene.instances.push(Instance::new(Drawable::Mesh(mesh.clone())).with_color([0.45, 0.6, 0.75, 1.0]).reflection(reflection));
        let f = r.render(RenderParams { scene: &scene, camera: &cam, width: w, height: h, device_pixel_ratio: 1.0, style: &Style::default(), pick: false }).unwrap();
        // The middle of the view, where the three visible faces meet.
        let (mut white, mut black) = (0u32, 0u32);
        for (y, x) in (h / 2 - 50..h / 2 + 50).flat_map(|y| (w / 2 - 50..w / 2 + 50).map(move |x| (y, x))) {
            let p = &f.bgra[((y * w + x) * 4) as usize..][..3];
            match (p[2], p[1], p[0]) {
                (120.., 120.., 120..) => white += 1,
                (0..=5, 0..=5, 0..=5) => black += 1,
                _ => {}
            }
        }
        // Above the middle: the top face.
        let top = f.bgra[(((h / 2 - 40) * w + w / 2) * 4) as usize + 2];
        (white, black, top)
    };
    let (white, black, top) = shot(Some(Reflection { environment: env.clone(), rotation: 0.0, blur: false, lock: None }));
    assert!(white > 1500 && black > 1500 && top >= 120, "not mirrored: {white} white, {black} black pixels, the top {top}");
    let (locked_white, _, _) = shot(Some(Reflection { environment: env.clone(), rotation: 0.0, blur: false, lock: Some(cam) }));
    assert!(locked_white.abs_diff(white) < 50, "locked where the camera is: {locked_white} white pixels, {white} unlocked");
    let (_, _, top) = shot(Some(Reflection { environment: env.clone(), rotation: 0.0, blur: false, lock: Some(below) }));
    assert!(top <= 5, "locked below, the top still reflects the sky: red {top}");
    let (_, black, _) = shot(None);
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

/// [`Instance::see_through_self`]: a mesh's `hidden_only` lines show behind other geometry, not behind the mesh itself
/// (the assembly Insert preview's edges through other parts, live mates.json insert_dialog).
#[test]
fn see_through_self_lines_skip_their_own_mesh() {
    let _gpu = GPU.lock().unwrap_or_else(|e| e.into_inner());
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let mut scene = Scene::default();
    scene.instances.push(Instance::new(Drawable::Mesh(r.upload_mesh(&primitives::box_mesh([-0.01, -0.01, -0.01], [0.01, 0.01, 0.01], 1)))).see_through_self());
    // Another part in front (the front view looks along +Y).
    scene.instances.push(Instance::new(Drawable::Mesh(r.upload_mesh(&primitives::box_mesh([0.03, -0.04, -0.01], [0.05, -0.02, 0.01], 1)))));
    let mut line = LineData::default();
    line.push_polyline(&[[-0.03, 0.02, 0.0], [0.06, 0.02, 0.0]], [255, 0, 0, 255], 0, 4.0, 0);
    scene.instances.push(Instance::new(Drawable::Lines(r.upload_lines(&line))).per_primitive_color().unpickable().hidden_only().see_through_self());
    let mut cam = Camera { rotation: Camera::rotation_for(StandardView::Front), ..Default::default() };
    cam.fit_sphere(glam::DVec3::new(0.015, 0.0, 0.0), 0.05, 4.0 / 3.0);
    let (w, h) = (400u32, 300u32);
    let f = r.render(RenderParams { scene: &scene, camera: &cam, width: w, height: h, device_pixel_ratio: 1.0, style: &Style::default(), pick: false }).unwrap();
    let vp = glam::DVec2::new(w as f64, h as f64);
    let px = |p: glam::DVec3| {
        let s = cam.project(p, vp, 0.05).unwrap();
        let o = ((s.y as u32 * w + s.x as u32) * 4) as usize;
        [f.bgra[o + 2], f.bgra[o + 1], f.bgra[o]]
    };
    let red = |c: [u8; 3]| c[0] > 200 && c[1] < 80;
    let behind_other = px(glam::DVec3::new(0.04, 0.02, 0.0));
    let behind_self = px(glam::DVec3::new(0.0, 0.02, 0.0));
    let visible = px(glam::DVec3::new(0.02, 0.02, 0.0));
    assert!(red(behind_other), "drawn behind the other part: {behind_other:?}");
    assert!(!red(behind_self), "not behind its own mesh: {behind_self:?}");
    assert!(!red(visible), "not where nothing hides it: {visible:?}");
}

/// [`Instance::outline_through`]: a selected part's outline shows through the part in front of it (an assembly's
/// selected instance, live2 2026-11-14), and nothing of it is drawn over its own image.
#[test]
fn an_outline_through_shows_past_the_part_in_front() {
    let _gpu = GPU.lock().unwrap_or_else(|e| e.into_inner());
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let back = r.upload_mesh(&primitives::box_mesh([-0.01, -0.005, -0.01], [0.01, 0.005, 0.01], 7));
    // A wide slab in front of its left half (the view looks roughly along +Y).
    let front = r.upload_mesh(&primitives::box_mesh([-0.06, -0.04, -0.04], [0.0, -0.03, 0.04], 1));
    let mut cam = Camera { rotation: Camera::look_rotation(glam::DVec3::new(0.25, 1.0, -0.2).normalize(), glam::DVec3::Z), ..Default::default() };
    cam.fit_sphere(glam::DVec3::ZERO, 0.05, 4.0 / 3.0);
    let (w, h) = (400u32, 300u32);
    let frame = |r: &mut Renderer, through: bool| {
        let mut scene = Scene::default();
        let mut b = Instance::new(Drawable::Mesh(back.clone())).with_color(srgb(0x9a, 0xb4, 0xcc));
        if through {
            b = b.outline_through();
        }
        scene.instances.push(b);
        scene.instances.push(Instance::new(Drawable::Mesh(front.clone())).with_color(srgb(0x40, 0x40, 0x40)));
        for id in 7..=12 {
            scene.set_state(id, state::SELECTED | state::BODY | state::OUTLINE);
        }
        r.render(RenderParams { scene: &scene, camera: &cam, width: w, height: h, device_pixel_ratio: 1.0, style: &Style::default(), pick: false }).unwrap().bgra
    };
    let (plain, through) = (frame(&mut r, false), frame(&mut r, true));
    let px = |b: &[u8], i: usize| [b[i * 4 + 2], b[i * 4 + 1], b[i * 4]];
    let orange = |c: [u8; 3]| c[0] > 200 && c[1] > 140 && c[2] < 140;
    let slab = |c: [u8; 3]| c[0] < 120 && c[0].abs_diff(c[1]) < 12 && c[1].abs_diff(c[2]) < 12;
    // The part's own image: its blue-grey faces.
    let own = |c: [u8; 3]| c[2] > c[0] + 20 && c[2] > 120;
    let (mut over_slab, mut over_own) = (0, Vec::new());
    for i in 0..(w * h) as usize {
        let (a, b) = (px(&plain, i), px(&through, i));
        if a == b {
            continue;
        }
        if slab(a) && orange(b) {
            over_slab += 1;
        } else if own(a) {
            over_own.push((i as u32 % w, i as u32 / w, a, b));
        }
    }
    assert!(over_slab > 20, "the outline drawn over the slab in front: {over_slab} px");
    assert!(over_own.is_empty(), "nothing over its own image ({} px): {:?}", over_own.len(), &over_own[..over_own.len().min(12)]);
}

/// Feature previews shade like Onshape's (live part-studio/230, default
/// view, white background): the top and front faces alike, the right side
/// lighter, flat (no highlight); the box is opaque in the preview blend's
/// model with the feature, mixed at the slider's 0.7 over the model before it
/// (here nothing: the white background).
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
    // The app's preview color (palette FEATURE_PREVIEW), opaque, and the
    // preview slider's rest value.
    let color = [srgb_lin(0x67), srgb_lin(0x84), srgb_lin(0x95), 1.0];
    let mut scene = Scene { preview_mix: Some(0.7), ..Default::default() };
    scene.instances.push(Instance::new(Drawable::Mesh(r.upload_mesh(&primitives::box_mesh([-0.05, -0.05, -0.05], [0.05, 0.05, 0.05], 1)))).with_color(color).preview().compare_layer(compare::TARGET));
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
        // Onshape's orthographic eye: 0.6 × the diameter back from the centre.
        let eye = if cam.projection == Projection::Perspective { cam.eye() } else { -fwd * 0.6 * 0.1 * 3f64.sqrt() };
        for n in [glam::DVec3::X, -glam::DVec3::X, glam::DVec3::Y, -glam::DVec3::Y, glam::DVec3::Z, -glam::DVec3::Z] {
            let p = n * 0.05;
            if n.dot(eye - p) <= 0.0 {
                continue;
            }
            let q = cam.project(p, glam::DVec2::new(w as f64, h as f64), 0.2).expect("on screen");
            let px = &f.bgra[((q.y as u32 * w + q.x as u32) * 4) as usize..][..3];
            let got = [px[2] as f64, px[1] as f64, px[0] as f64];
            let half = (light + (eye - p).normalize()).normalize();
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

/// Live 10-27 (behaviours "spec: psui part-studio.md 2.18 / 2.19"): three
/// 30 × 20 × 10 mm blocks in a row at x 0 / 40 / 80, iso view. The
/// highlight comes from Onshape's eye-space position, so only the far
/// block's top carries it: Part 1's top (142,187,214), Part 3's
/// (62,96,171) = 0.905 × its colour + about 9.
#[test]
fn the_highlight_moves_across_an_orthographic_view() {
    let _gpu = GPU.lock().unwrap_or_else(|e| e.into_inner());
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let (w, h) = (600u32, 400u32);
    let lin = |c: [u8; 3]| [srgb_lin(c[0]), srgb_lin(c[1]), srgb_lin(c[2]), 1.0];
    let mut scene = Scene::default();
    for (x, color) in [(0.0f32, [157, 207, 237]), (0.04, [165, 165, 165]), (0.08, [59, 97, 180])] {
        let mesh = r.upload_mesh(&primitives::box_mesh([x, 0.0, 0.0], [x + 0.03, 0.02, 0.01], 1));
        scene.instances.push(Instance::new(Drawable::Mesh(mesh)).with_color(lin(color)));
    }
    let mut cam = Camera::default();
    cam.set_view(StandardView::Isometric);
    cam.fit_box(glam::DVec3::ZERO, glam::DVec3::new(0.11, 0.02, 0.01), w as f64 / h as f64);
    let f = r.render(RenderParams { scene: &scene, camera: &cam, width: w, height: h, device_pixel_ratio: 1.0, style: &Style::default(), pick: false }).unwrap();
    let top = |x: f64| {
        let q = cam.project(glam::DVec3::new(x, 0.01, 0.01), glam::DVec2::new(w as f64, h as f64), 0.2).expect("on screen");
        let px = &f.bgra[((q.y as u32 * w + q.x as u32) * 4) as usize..][..3];
        [px[2] as f64, px[1] as f64, px[0] as f64]
    };
    for (x, want) in [(0.015, [142.0, 187.0, 214.0]), (0.095, [62.0, 96.0, 171.0])] {
        let got = top(x);
        assert!(got.iter().zip(want).all(|(g, w)| (g - w).abs() <= 4.0), "top at x {x}: {got:?} vs live {want:?}");
    }
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
/// on, orthographic, at 1x and 2x. The pixels are Onshape's canvas's: CSS
/// pixels, but device pixels on a Retina Mac (live 2255: 16 and 20 px
/// apart at dpr 1 whatever the zoom).
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
        let want = 15.0 * pace * if cfg!(target_os = "macos") { 1.0 } else { dpr };
        let best = ((want * 0.6) as usize..(want * 1.5) as usize).max_by(|a, b| ac(*a).total_cmp(&ac(*b))).unwrap() as f32;
        assert!((best - want).abs() <= 0.08 * want + 1.0, "{dpr}x: hatch period {best} px, Onshape's {want} px");
        // Not half of it (its second peak would fall on `want` too).
        let half = ac((want / 2.0).round() as usize);
        assert!(half < 0.3 * ac(best as usize), "{dpr}x: hatch lines every {} px, Onshape's {want} px", want / 2.0);
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

/// A feature dialog's preview (live previewBlend, `Scene::preview_mix`):
/// the model before the feature (BASE: a box) under the model with it
/// (TARGET: the box moved and a second box), mixed by the slider's alpha.
/// Untagged geometry (a datum line) is in both and doesn't change; each
/// pixel is (1 − alpha) × the base frame + alpha × the preview frame.
#[test]
fn preview_mix_blends_the_model_before_and_after() {
    let _gpu = GPU.lock().unwrap_or_else(|e| e.into_inner());
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let blue = [0.25f32, 0.4, 0.6, 1.0];
    let (w, h) = (400u32, 300u32);
    let boxm = |r: &Renderer, x: f32| r.upload_mesh(&primitives::box_mesh([x - 0.01, -0.01, 0.0], [x + 0.01, 0.01, 0.01], 1));
    let base = Instance::new(Drawable::Mesh(boxm(&r, -0.012))).with_color(blue).compare_layer(compare::BASE);
    let moved = Instance::new(Drawable::Mesh(boxm(&r, 0.012))).with_color(blue).compare_layer(compare::TARGET);
    let mut line = LineData::default();
    line.push_polyline(&[[-0.03, -0.012, 0.0], [0.03, -0.012, 0.0]], [255, 0, 0, 255], 0, 3.0, 0);
    let datum = Instance::new(Drawable::Lines(r.upload_lines(&line)));
    let mut cam = Camera::default();
    cam.fit_box(glam::DVec3::new(-0.03, -0.012, 0.0), glam::DVec3::new(0.03, 0.012, 0.01), w as f64 / h as f64);
    let render = |r: &mut Renderer, instances: Vec<Instance>, mix: Option<f32>| {
        let scene = Scene { instances, preview_mix: mix, ..Default::default() };
        r.render(RenderParams { scene: &scene, camera: &cam, width: w, height: h, device_pixel_ratio: 1.0, style: &Style::default(), pick: false }).unwrap().bgra
    };
    let untag = |i: &Instance| i.clone().compare_layer(0);
    let before = render(&mut r, vec![untag(&base), datum.clone()], None);
    let after = render(&mut r, vec![untag(&moved), datum.clone()], None);
    let mixed = render(&mut r, vec![base.clone(), moved.clone(), datum.clone()], Some(0.7));
    let at = |p: [f64; 3]| {
        let q = cam.project(glam::DVec3::from_array(p), glam::DVec2::new(w as f64, h as f64), 0.05).unwrap();
        ((q.y as u32 * w + q.x as u32) * 4) as usize
    };
    for (what, p) in [("box before", [-0.012, 0.0, 0.01]), ("box after", [0.012, 0.0, 0.01]), ("background", [0.0, 0.03, 0.0]), ("datum", [0.0, -0.012, 0.0])] {
        let o = at(p);
        for c in 0..3 {
            let want = 0.3 * before[o + c] as f32 + 0.7 * after[o + c] as f32;
            let got = mixed[o + c] as f32;
            assert!((got - want).abs() <= 3.0, "{what}: channel {c} {got} vs {want} (before {}, after {})", before[o + c], after[o + c]);
        }
    }
    let o = at([-0.012, 0.0, 0.01]);
    assert!(before[o..o + 3] != after[o..o + 3], "the boxes should differ there");
}

/// A feature preview picks the model with the feature: the frame draws the
/// model before it (an unpickable copy over the same faces), which must not
/// hide the picks (regression: no face could be picked in a feature dialog).
#[test]
fn a_preview_picks_the_model_with_the_feature() {
    let _gpu = GPU.lock().unwrap_or_else(|e| e.into_inner());
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let mesh = r.upload_mesh(&primitives::box_mesh([-0.01, -0.01, -0.01], [0.01, 0.01, 0.01], 1));
    let mut scene = Scene { preview_mix: Some(0.7), ..Default::default() };
    scene.instances.push(Instance::new(Drawable::Mesh(mesh.clone())).compare_layer(compare::TARGET));
    let mut base = Instance::new(Drawable::Mesh(mesh)).compare_layer(compare::BASE);
    base.pickable = false;
    base.pick_base = 1 << 30;
    scene.instances.push(base);
    let mut cam = Camera::default();
    cam.fit_sphere(glam::DVec3::ZERO, 0.0175, 4.0 / 3.0);
    let f = r.render(RenderParams { scene: &scene, camera: &cam, width: 400, height: 300, device_pixel_ratio: 1.0, style: &Style::default(), pick: true }).unwrap();
    let id = f.pick.unwrap().ids[(150 * 400 + 200) as usize];
    assert!((1..=6).contains(&id), "the centre picks {id}, not a face of the model with the feature");
}

/// While a feature dialog previews, a part's edges hover and pick like its faces (LP-DIRECTION-PICK: v0.4.9's
/// pick pass drew only the unpickable base, so a Linear pattern's Direction took no edge): the visible edges of
/// the model with the feature pick their ids through the mix, as without a preview.
#[test]
fn a_preview_picks_the_edges_of_the_model_with_the_feature() {
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
    let mid = |k: usize| {
        let (a, b) = (c(pairs[k].0), c(pairs[k].1));
        glam::DVec3::new((a[0] + b[0]) as f64 * 0.5, (a[1] + b[1]) as f64 * 0.5, (a[2] + b[2]) as f64 * 0.5)
    };
    let cam = iso_camera();
    let (w, h) = (400u32, 300u32);
    let vp = glam::DVec2::new(w as f64, h as f64);
    let (mesh, edges) = (r.upload_mesh(&primitives::box_mesh(lo, hi, 1)), r.upload_lines(&primitives::box_edges(lo, hi, 10, 1.5)));
    let picks = |r: &mut Renderer, preview: bool| {
        let mut scene = Scene::default();
        if preview {
            scene.preview_mix = Some(0.7);
            scene.instances.push(Instance::new(Drawable::Mesh(mesh.clone())).compare_layer(compare::TARGET));
            scene.instances.push(Instance::new(Drawable::Lines(edges.clone())).compare_layer(compare::TARGET));
            for d in [Drawable::Mesh(mesh.clone()), Drawable::Lines(edges.clone())] {
                let mut base = Instance::new(d).compare_layer(compare::BASE);
                base.pickable = false;
                base.pick_base = 1 << 30;
                scene.instances.push(base);
            }
        } else {
            scene.instances.push(Instance::new(Drawable::Mesh(mesh.clone())));
            scene.instances.push(Instance::new(Drawable::Lines(edges.clone())));
        }
        let f = r.render(RenderParams { scene: &scene, camera: &cam, width: w, height: h, device_pixel_ratio: 1.0, style: &Style::default(), pick: true }).unwrap();
        let pick = f.pick.unwrap();
        (0..12)
            .map(|k| {
                let s = cam.project(mid(k), vp, 0.02).unwrap();
                pick.id_at(s.x as i32, s.y as i32).unwrap_or(0)
            })
            .collect::<Vec<u32>>()
    };
    let plain = picks(&mut r, false);
    let visible: Vec<usize> = (0..12).filter(|&k| plain[k] == 10 + k as u32).collect();
    assert!(visible.len() >= 6, "an iso box shows at least 6 edges: {plain:?}");
    let previewed = picks(&mut r, true);
    for k in visible {
        assert_eq!(previewed[k], 10 + k as u32, "edge {k} under the preview picks {} (plain {:?}, preview {previewed:?})", previewed[k], plain);
    }
}

/// A preview's highlight lines (compare::POST) stay whole over the mix:
/// a line the new body covers keeps its colour, where an untagged one
/// shows at the mix's 30 % (live: a picked region under the new boss stays
/// fully outlined).
#[test]
fn preview_highlights_stay_whole_over_the_mix() {
    let _gpu = GPU.lock().unwrap_or_else(|e| e.into_inner());
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let (w, h) = (400u32, 300u32);
    let floor = Instance::new(Drawable::Mesh(r.upload_mesh(&primitives::box_mesh([-0.03, -0.03, -0.002], [0.03, 0.03, 0.0], 1)))).with_color([0.6, 0.6, 0.6, 1.0]).compare_layer(compare::BASE);
    let floor_too = floor.clone().compare_layer(compare::TARGET);
    // The new body, over the line.
    let boss = Instance::new(Drawable::Mesh(r.upload_mesh(&primitives::box_mesh([-0.01, -0.01, 0.0], [0.01, 0.01, 0.01], 10)))).with_color([0.2, 0.35, 0.5, 1.0]).compare_layer(compare::TARGET);
    let mut line = LineData::default();
    line.push_polyline(&[[-0.008, 0.0, 0.0], [0.008, 0.0, 0.0]], [255, 0, 0, 255], 0, 3.0, 0);
    let outline = Instance::new(Drawable::Lines(r.upload_lines(&line))).with_color([1.0, 0.0, 0.0, 1.0]);
    let mut cam = Camera::default();
    cam.set_view(StandardView::Top);
    cam.fit_box(glam::DVec3::new(-0.03, -0.03, 0.0), glam::DVec3::new(0.03, 0.03, 0.01), w as f64 / h as f64);
    let red_at_centre = |r: &mut Renderer, tag: u8| {
        let scene = Scene { instances: vec![floor.clone(), floor_too.clone(), boss.clone(), outline.clone().compare_layer(tag)], preview_mix: Some(0.7), ..Default::default() };
        let f = r.render(RenderParams { scene: &scene, camera: &cam, width: w, height: h, device_pixel_ratio: 1.0, style: &Style::default(), pick: false }).unwrap();
        // The reddest pixel across the line at the centre (antialiased edges).
        (h / 2 - 3..h / 2 + 4).map(|y| {
            let o = ((y * w + w / 2) * 4) as usize;
            (f.bgra[o + 2], f.bgra[o + 1])
        }).max_by_key(|(r, g)| *r as i32 - *g as i32).unwrap()
    };
    let (plain, post) = (red_at_centre(&mut r, 0), red_at_centre(&mut r, compare::POST));
    assert!(post.0 >= 245 && post.1 <= 20, "the post line lost its colour under the new body: {post:?}");
    assert!(plain.0 < 160, "an untagged line under the new body shows at the mix: {plain:?}");
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

/// A hovered part behind another (live2 11-14, feel/hidden-part-selection.png: a list row's or a face's hover): its
/// outline follows its whole silhouette, drawn over the part in front where that hides it.
#[test]
fn a_hovered_part_is_outlined_through_the_part_in_front() {
    let _gpu = GPU.lock().unwrap_or_else(|e| e.into_inner());
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let boxes = [([-0.02f32, -0.01, -0.005], [0.02f32, 0.01, 0.005]), ([-0.005f32, -0.01, 0.007], [0.035f32, 0.01, 0.017])];
    let meshes: Vec<_> = boxes.iter().enumerate().map(|(k, (lo, hi))| r.upload_mesh(&primitives::box_mesh(*lo, *hi, 1 + 6 * k as u32))).collect();
    let (w, h) = (400u32, 300u32);
    let mut cam = Camera { rotation: Camera::look_rotation(glam::DVec3::new(-1.0, 1.0, -1.0), glam::DVec3::Z), ..Default::default() };
    cam.fit_box(glam::DVec3::new(-0.02, -0.01, -0.005), glam::DVec3::new(0.035, 0.01, 0.017), w as f64 / h as f64);
    let mut frame = |only: Option<usize>, hover: Option<usize>| {
        let mut scene = Scene::default();
        for (k, m) in meshes.iter().enumerate() {
            if only.is_none_or(|o| o == k) {
                // Two colours: where one part covers the other, its pixels tell which.
                let c = if k == 0 { srgb(0x9a, 0xb4, 0xcc) } else { srgb(0xcc, 0xa8, 0x9a) };
                scene.instances.push(Instance::new(Drawable::Mesh(m.clone())).with_color(c));
            }
        }
        if let Some(k) = hover {
            for id in 1..=6 {
                scene.set_state(id + 6 * k as u32, state::HOVER | state::OUTLINE);
            }
            scene.hover_outline = true;
        }
        r.render(RenderParams { scene: &scene, camera: &cam, width: w, height: h, device_pixel_ratio: 1.0, style: &Style::default(), pick: false }).unwrap().bgra
    };
    let rest = frame(None, None);
    let px = |b: &[u8], i: usize| -> [u8; 3] { [b[i * 4 + 2], b[i * 4 + 1], b[i * 4]] };
    let background = px(&rest, 0);
    let orange = |c: [u8; 3]| (c[0] as i32 - 255).abs() <= 3 && (c[1] as i32 - 198).abs() <= 3 && (c[2] as i32 - 133).abs() <= 3;
    // The part whose own image loses the most pixels to the other is the one behind.
    let alone: Vec<Vec<u8>> = (0..2).map(|k| frame(Some(k), None)).collect();
    // A part's pixel is hidden where both parts cover it and the frame is nearer the other part's colour (ambient
    // occlusion shades the visible one near the other, so not exact equality).
    let dist = |a: [u8; 3], b: [u8; 3]| (0..3).map(|c| (a[c] as i32 - b[c] as i32).abs()).sum::<i32>();
    let hidden = |k: usize, i: usize| {
        let (mine, other) = (px(&alone[k], i), px(&alone[1 - k], i));
        mine != background && other != background && dist(px(&rest, i), other) < dist(px(&rest, i), mine)
    };
    let count = |k: usize| (0..(w * h) as usize).filter(|&i| hidden(k, i)).count();
    let back = if count(0) >= count(1) { 0 } else { 1 };
    assert!(count(back) > 500, "the parts overlap on screen");
    // Hidden pixels well away (over 3 px) from the part's visible ones: a depth-tested band never reaches them.
    let visible = |x: i32, y: i32| (0..w as i32).contains(&x) && (0..h as i32).contains(&y) && {
        let i = (y as u32 * w + x as u32) as usize;
        px(&alone[back], i) != background && !hidden(back, i)
    };
    let hovered = frame(None, Some(back));
    let through = (0..(w * h) as usize)
        .filter(|&i| {
            let (x, y) = ((i as u32 % w) as i32, (i as u32 / w) as i32);
            hidden(back, i) && orange(px(&hovered, i)) && !(-3..=3).any(|dy| (-3..=3).any(|dx| visible(x + dx, y + dy)))
        })
        .count();
    if let Ok(dir) = std::env::var("NOSHAPE_TEST_OUT") {
        for (name, b) in [("rest", &rest), ("hover", &hovered), ("a", &alone[0]), ("b", &alone[1])] {
            let mut rgba = b.clone();
            for p in rgba.as_chunks_mut::<4>().0 {
                p.swap(0, 2);
            }
            image::save_buffer(format!("{dir}/hover_through_{name}.png"), &rgba, w, h, image::ExtendedColorType::Rgba8).unwrap();
        }
    }
    assert!(through > 100, "only {through} outline pixels over the part in front, away from its visible image");
}

/// PART-HOVER-RED look (live behaviours "psui (2026-10-27, P2) PART-HOVER-RED look", psui/parthover crops): a hovered
/// whole part's outline is its silhouette line redrawn in solid hover orange, the part's boundary pixel and the one
/// outside it (2 px at 1×); the faces keep their colours and nothing is drawn farther out.
#[test]
fn a_hovered_part_is_outlined_on_its_silhouette() {
    let _gpu = GPU.lock().unwrap_or_else(|e| e.into_inner());
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let (lo, hi) = ([-0.02f32, -0.01, -0.005], [0.02f32, 0.01, 0.005]);
    let mesh = r.upload_mesh(&primitives::box_mesh(lo, hi, 1));
    let edges = r.upload_lines(&primitives::box_edges(lo, hi, 10, 1.0));
    for dpr in [1.0f32, 2.0] {
        let (w, h) = ((400.0 * dpr) as u32, (300.0 * dpr) as u32);
        let mut cam = Camera { rotation: Camera::look_rotation(glam::DVec3::new(-1.0, 1.0, -1.0), glam::DVec3::Z), ..Default::default() };
        cam.fit_box(glam::Vec3::from(lo).as_dvec3(), glam::Vec3::from(hi).as_dvec3(), w as f64 / h as f64);
        let mut frame = |hover: bool| {
            let mut scene = Scene::default();
            scene.instances.push(Instance::new(Drawable::Mesh(mesh.clone())).with_color(srgb(0x9a, 0xb4, 0xcc)));
            scene.instances.push(Instance::new(Drawable::Lines(edges.clone())).with_color([0.02, 0.02, 0.03, 1.0]).layer(1));
            if hover {
                for id in 1..=6 {
                    scene.set_state(id, state::HOVER | state::OUTLINE);
                }
                scene.hover_outline = true;
            }
            r.render(RenderParams { scene: &scene, camera: &cam, width: w, height: h, device_pixel_ratio: dpr, style: &Style::default(), pick: false }).unwrap().bgra
        };
        let (rest, hovered) = (frame(false), frame(true));
        let px = |b: &[u8], x: i32, y: i32| -> [u8; 3] {
            let i = ((y as u32 * w + x as u32) * 4) as usize;
            [b[i + 2], b[i + 1], b[i]]
        };
        let background = px(&rest, 2, 2);
        let orange = |c: [u8; 3]| (c[0] as i32 - 255).abs() <= 3 && (c[1] as i32 - 198).abs() <= 3 && (c[2] as i32 - 133).abs() <= 3;
        // Pixels within `d` device px of the part's image at rest.
        let near_part = |x: i32, y: i32, d: i32| (-d..=d).any(|dy| (-d..=d).any(|dx| (0..w as i32).contains(&(x + dx)) && (0..h as i32).contains(&(y + dy)) && px(&rest, x + dx, y + dy) != background));
        let (mut far, mut band, mut other) = (0, 0, 0);
        for y in 0..h as i32 {
            for x in 0..w as i32 {
                let (a, b) = (px(&rest, x, y), px(&hovered, x, y));
                if a == b {
                    continue;
                }
                if orange(b) {
                    band += 1;
                } else {
                    other += 1;
                }
                if !near_part(x, y, dpr.ceil() as i32) {
                    far += 1;
                }
            }
        }
        if let Ok(dir) = std::env::var("NOSHAPE_TEST_OUT") {
            for (name, b) in [("rest", &rest), ("hover", &hovered)] {
                let mut rgba = b.clone();
                for p in rgba.as_chunks_mut::<4>().0 {
                    p.swap(0, 2);
                }
                image::save_buffer(format!("{dir}/part_hover_{name}_{dpr}x.png"), &rgba, w, h, image::ExtendedColorType::Rgba8).unwrap();
            }
        }
        assert_eq!(far, 0, "{dpr}x: {far} pixels drawn away from the part");
        assert_eq!(other, 0, "{dpr}x: {other} pixels changed besides the orange band");
        assert!(band > (300.0 * dpr) as usize, "{dpr}x: only {band} band pixels");
        // Along the middle row: the band at the left silhouette, about 2 px (1×) wide, and the face's colour unchanged
        // just inside it.
        let y = h as i32 / 2;
        let start = (0..w as i32).find(|&x| orange(px(&hovered, x, y))).expect("the band on the middle row");
        let run = (start..w as i32).take_while(|&x| orange(px(&hovered, x, y))).count() as i32;
        let d = dpr as i32;
        assert!((2 * d - 1..=3 * d).contains(&run), "{dpr}x: band {run} px wide at the left silhouette");
        let inner = start + run + 1;
        assert_eq!(px(&hovered, inner, y), px(&rest, inner, y), "{dpr}x: the face inside the band changed");
    }
}


/// Live ("psui (2026-10-28, P1) part-studio 2.23"): a part made see-through by its appearance is picked like any part,
/// nearest first (`Instance::pick_solid`); other see-through meshes (datum planes, Isolate's parts) let what is behind
/// them win.
#[test]
fn a_see_through_part_in_front_wins_the_pick_when_solid() {
    let _gpu = GPU.lock().unwrap_or_else(|e| e.into_inner());
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let front = r.upload_mesh(&primitives::box_mesh([-0.01, -0.01, 0.0], [0.01, 0.01, 0.01], 1));
    let back = r.upload_mesh(&primitives::box_mesh([-0.1, -0.1, -0.04], [0.1, 0.1, -0.03], 1));
    let cam = iso_camera();
    let (w, h) = (400u32, 300u32);
    let picked = |r: &mut Renderer, solid: bool| {
        let mut scene = Scene::default();
        scene.instances.push(Instance::new(Drawable::Mesh(back.clone())).pick_base(1000));
        scene.instances.push(Instance::new(Drawable::Mesh(front.clone())).with_color([0.6, 0.8, 0.9, 0.5]).pick_solid(solid));
        let f = r.render(RenderParams { scene: &scene, camera: &cam, width: w, height: h, device_pixel_ratio: 1.0, style: &Style::default(), pick: true }).unwrap();
        let s = cam.project(glam::DVec3::new(0.0, 0.0, 0.01), glam::DVec2::new(w as f64, h as f64), 0.02).unwrap();
        f.pick.unwrap().id_at(s.x as i32, s.y as i32).unwrap()
    };
    let behind = picked(&mut r, false);
    assert!(behind > 1000, "a see-through mesh lets the part behind it win: {behind}");
    assert_eq!(picked(&mut r, true), 2, "the see-through part's own top face (+Z, id 2) wins");
}

/// The silhouette cull (`Renderer::set_silhouette_cull`, on by default) draws what the plain silhouette pass draws:
/// a ring of candidates round a cylinder, in two copies of one buffer (two slots), from an oblique view.
#[test]
fn culled_silhouettes_draw_as_the_plain_ones() {
    let _gpu = GPU.lock().unwrap_or_else(|e| e.into_inner());
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let (w, h) = (240u32, 180u32);
    let mut cam = Camera::default();
    cam.set_view(StandardView::Isometric);
    cam.fit_sphere(glam::DVec3::ZERO, 0.05, w as f64 / h as f64);
    // 48 rulings of a cylinder of radius 1 cm, each between the facets either side of it.
    let n = 48;
    let at = |k: f32| {
        let t = std::f32::consts::TAU * k / n as f32;
        [t.cos(), t.sin()]
    };
    let segments: Vec<SilhouetteSegment> = (0..n)
        .map(|k| {
            let [x, y] = at(k as f32);
            let ([a, b], [c, d]) = (at(k as f32 - 0.5), at(k as f32 + 0.5));
            SilhouetteSegment { a: [0.01 * x, 0.01 * y, -0.02], b: [0.01 * x, 0.01 * y, 0.02], n1: [a, b, 0.0], n2: [c, d, 0.0], pick_id: 0, width: 1.25 }
        })
        .collect();
    let lines = r.upload_silhouettes(&SilhouetteData { segments });
    let copy = |dx: f32| Instance::new(Drawable::Lines(lines.clone())).with_color([0.0, 0.0, 0.0, 1.0]).with_transform(glam::Mat4::from_translation(glam::Vec3::new(dx, 0.0, 0.0)));
    let scene = Scene { instances: vec![copy(-0.015), copy(0.015)], ..Default::default() };
    let frame = |r: &mut Renderer| r.render(RenderParams { scene: &scene, camera: &cam, width: w, height: h, device_pixel_ratio: 1.0, style: &Style::default(), pick: false }).unwrap().bgra;
    r.set_silhouette_cull(false);
    let plain = frame(&mut r);
    if !r.set_silhouette_cull(true) {
        eprintln!("skipping: no silhouette cull on this device");
        return;
    }
    let culled = frame(&mut r);
    assert!(plain.iter().filter(|&&v| v < 100).count() > 50, "the silhouettes drawn");
    assert_eq!(plain, culled);
}

/// User P1 11-12 ("hovered lines don't turn orange, though I can click them"): a curved face's silhouette picks as the
/// face, and hovering it lit only the face's boundary edges, so the line under the pointer stayed black. Live (behaviours
/// EDGE-HIGHLIGHT-LOOK (d)): a hovered face lights its silhouette too, as its boundary: opaque #FFC685; selected, #FFA90A.
#[test]
fn a_hovered_or_selected_faces_silhouette_is_lit() {
    let _gpu = GPU.lock().unwrap_or_else(|e| e.into_inner());
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let (w, h) = (240u32, 180u32);
    let mut cam = Camera::default();
    cam.set_view(StandardView::Isometric);
    cam.fit_sphere(glam::DVec3::ZERO, 0.03, w as f64 / h as f64);
    // The rulings of a cylinder's face (pick id 1), each between the facets either side of it.
    let n = 48;
    let at = |k: f32| {
        let t = std::f32::consts::TAU * k / n as f32;
        [t.cos(), t.sin()]
    };
    let segments: Vec<SilhouetteSegment> = (0..n)
        .map(|k| {
            let [x, y] = at(k as f32);
            let ([a, b], [c, d]) = (at(k as f32 - 0.5), at(k as f32 + 0.5));
            SilhouetteSegment { a: [0.01 * x, 0.01 * y, -0.02], b: [0.01 * x, 0.01 * y, 0.02], n1: [a, b, 0.0], n2: [c, d, 0.0], pick_id: 1, width: 1.25 }
        })
        .collect();
    let lines = r.upload_silhouettes(&SilhouetteData { segments });
    let frame = |r: &mut Renderer, st: u32| {
        let mut scene = Scene { instances: vec![Instance::new(Drawable::Lines(lines.clone())).with_color([0.0, 0.0, 0.0, 1.0])], ..Default::default() };
        if st != 0 {
            scene.set_state(1, st);
        }
        r.render(RenderParams { scene: &scene, camera: &cam, width: w, height: h, device_pixel_ratio: 1.0, style: &Style { msaa: 1, ..Style::default() }, pick: false }).unwrap().bgra
    };
    let count = |b: &[u8], want: [u8; 3]| b.as_chunks::<4>().0.iter().filter(|p| (p[2] as i32 - want[0] as i32).abs() <= 4 && (p[1] as i32 - want[1] as i32).abs() <= 4 && (p[0] as i32 - want[2] as i32).abs() <= 4).count();
    let rest = frame(&mut r, 0);
    assert!(count(&rest, [0, 0, 0]) > 50, "the silhouettes drawn black at rest");
    for (st, colour) in [(state::HOVER, [255, 198, 133]), (state::SELECTED, [255, 169, 10])] {
        let lit = frame(&mut r, st);
        assert!(count(&lit, colour) > 50, "state {st}: the silhouettes in {colour:?}: {} px", count(&lit, colour));
    }
}

/// Translucent mode as live 11-02: only a part's front faces blend, each at 25 % (a box seen from above: its top
/// over the white background moves each channel a quarter of the way from white to the top's shaded color; the
/// bottom face behind adds nothing).
#[test]
fn translucent_parts_blend_their_front_faces_once() {
    let _gpu = GPU.lock().unwrap_or_else(|e| e.into_inner());
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let mesh = r.upload_mesh(&primitives::box_mesh([-0.05, -0.03, -0.01], [0.05, 0.03, 0.01], 1));
    let mut cam = Camera { rotation: Camera::look_rotation(glam::DVec3::NEG_Z, glam::DVec3::Y), ..Default::default() };
    let (w, h) = (200u32, 150u32);
    cam.fit_box(glam::DVec3::new(-0.05, -0.03, -0.01), glam::DVec3::new(0.05, 0.03, 0.01), w as f64 / h as f64);
    let mut centre = |inst: Instance| {
        let scene = Scene { instances: vec![inst], ..Default::default() };
        let f = r.render(RenderParams { scene: &scene, camera: &cam, width: w, height: h, device_pixel_ratio: 1.0, style: &Style::default(), pick: false }).unwrap();
        let o = (((h / 2) * w + w / 2) * 4) as usize;
        [f.bgra[o + 2] as f32, f.bgra[o + 1] as f32, f.bgra[o] as f32]
    };
    let color = [0.45f32, 0.6, 0.75, 1.0];
    let opaque = centre(Instance::new(Drawable::Mesh(mesh.clone())).with_color(color));
    let see_through = centre(Instance::new(Drawable::Mesh(mesh)).with_color([color[0], color[1], color[2], 0.25]).front_only());
    for k in 0..3 {
        let want = 255.0 - (255.0 - opaque[k]) * 0.25;
        assert!((see_through[k] - want).abs() <= 2.0, "channel {k}: {see_through:?}, one layer over white is {want} (opaque {opaque:?})");
    }
}

/// Ambient occlusion as Onshape's (its pass darkens shaded faces where geometry stands close): the floor beside a
/// block's foot is darker with the preference on, and not at all when the scene asks for none (Hidden line,
/// Translucent).
#[test]
fn ambient_occlusion_darkens_a_corner_unless_the_scene_has_none() {
    let _gpu = GPU.lock().unwrap_or_else(|e| e.into_inner());
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let floor = r.upload_mesh(&primitives::box_mesh([-0.05, -0.05, -0.01], [0.05, 0.05, 0.0], 1));
    let block = r.upload_mesh(&primitives::box_mesh([-0.01, -0.01, 0.0], [0.01, 0.01, 0.02], 100));
    let grey = [0.6f32, 0.6, 0.6, 1.0];
    let mut scene = Scene { instances: vec![Instance::new(Drawable::Mesh(floor)).with_color(grey), Instance::new(Drawable::Mesh(block)).with_color(grey)], ..Default::default() };
    let mut cam = Camera { rotation: Camera::rotation_for(StandardView::Isometric), ..Default::default() };
    let (w, h) = (400u32, 300u32);
    cam.fit_box(glam::DVec3::new(-0.05, -0.05, -0.01), glam::DVec3::new(0.05, 0.05, 0.02), w as f64 / h as f64);
    // On the floor just in front of the block's foot (toward the viewer: +x, -y).
    let p = cam.project(glam::DVec3::new(0.0105, -0.0105, 0.0), glam::DVec2::new(w as f64, h as f64), 0.02).unwrap();
    let mut lum = |scene: &Scene, ao: f32| {
        let style = Style { ambient_occlusion: ao, ..Style::default() };
        let f = r.render(RenderParams { scene, camera: &cam, width: w, height: h, device_pixel_ratio: 1.0, style: &style, pick: false }).unwrap();
        let o = ((p.y as u32 * w + p.x as u32 + 2) * 4) as usize;
        f.bgra[o] as i32 + f.bgra[o + 1] as i32 + f.bgra[o + 2] as i32
    };
    let (off, on) = (lum(&scene, 0.0), lum(&scene, 0.375));
    assert!(on + 15 < off, "the corner is darker with occlusion: {on} vs {off}");
    scene.no_occlusion = true;
    assert_eq!(lum(&scene, 0.375), off, "no occlusion when the scene has none");
}

/// One sketch point drawn alone in each state, the middle row of pixels (RGB) and its pixel ratio.
fn sketch_point_rows(r: &mut Renderer, sprite: PointSprite, dpr: f32, st: u32) -> Vec<[u8; 3]> {
    let pts = r.upload_points(&PointData { points: vec![sprite] });
    let mut cam = Camera::default();
    cam.fit_sphere(glam::DVec3::ZERO, 0.01, 1.0);
    let style = Style { msaa: 1, ..Style::default() };
    let n = (64.0 * dpr) as u32;
    let mut scene = Scene::default();
    scene.instances.push(Instance::new(Drawable::Points(pts)).per_primitive_color());
    if st != 0 {
        scene.set_state(1, st);
    }
    let f = r.render(RenderParams { scene: &scene, camera: &cam, width: n, height: n, device_pixel_ratio: dpr, style: &style, pick: false }).unwrap();
    let y = n / 2;
    (0..n).map(|x| { let i = ((y * n + x) * 4) as usize; [f.bgra[i + 2], f.bgra[i + 1], f.bgra[i]] }).collect()
}

/// Drawn width in CSS px (first to last pixel that isn't white) and the colour `css` px from the centre.
fn row_width(row: &[[u8; 3]], dpr: f32) -> f32 {
    let ink = |p: &&[u8; 3]| p.iter().any(|c| *c < 250);
    let first = row.iter().position(|p| ink(&p)).unwrap_or(0);
    let last = row.iter().rposition(|p| ink(&p)).unwrap_or(0);
    (last + 1 - first) as f32 / dpr
}

fn row_at(row: &[[u8; 3]], dpr: f32, css: f32) -> [u8; 3] {
    row[(row.len() as f32 / 2.0 + css * dpr) as usize]
}

fn rgb_near(p: [u8; 3], want: [u8; 3], tol: i32) -> bool {
    p.iter().zip(want).all(|(a, b)| (*a as i32 - b as i32).abs() <= tol)
}

const UNDER_DEFINED: [u8; 4] = [0, 0, 208, 255];

/// A spline handle tip as live draws it (live 2221, at 1x): at rest a 9 px hollow handle, a ring in the point's
/// colour round a white middle; hovered a 14 px light orange disc, selected a 15 px orange one, each a little lighter
/// in the middle where the handle lies under it; hovered while selected, as selected. Sizes in CSS px at any ratio.
#[test]
fn a_handle_tip_is_drawn_as_lives_in_each_state() {
    let _gpu = GPU.lock().unwrap_or_else(|e| e.into_inner());
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let tip = PointSprite { position: [0.0; 3], color: UNDER_DEFINED, pick_id: 1, size: 9.0, style: PointSprite::HANDLE_TIP | PointSprite::SKETCH_POINT | 2, _pad: 0 };
    for dpr in [1.0f32, 2.0] {
        let rest = sketch_point_rows(&mut r, tip, dpr, 0);
        assert!((row_width(&rest, dpr) - 9.0).abs() <= 1.0, "dpr {dpr}: at rest {} px", row_width(&rest, dpr));
        assert!(rgb_near(row_at(&rest, dpr, 0.5), [255, 255, 255], 8), "dpr {dpr}: white inside {:?}", row_at(&rest, dpr, 0.5));
        let ring = row_at(&rest, dpr, 4.0);
        assert!(ring[2] > ring[0] + 60, "dpr {dpr}: a blue ring {ring:?}");
        for (st, width, colour) in [(state::HOVER, 14.0, [255, 198, 133]), (state::SELECTED, 15.0, [255, 169, 10]), (state::HOVER | state::SELECTED, 15.0, [255, 169, 10])] {
            let lit = sketch_point_rows(&mut r, tip, dpr, st);
            assert!((row_width(&lit, dpr) - width).abs() <= 1.0, "dpr {dpr} state {st}: {} px", row_width(&lit, dpr));
            assert!(rgb_near(row_at(&lit, dpr, 6.0), colour, 6), "dpr {dpr} state {st}: disc {:?}", row_at(&lit, dpr, 6.0));
            // (The middle a little lighter: the handle's white under 90 % of the colour.)
            let c = row_at(&lit, dpr, 0.5);
            assert!(c[1] > colour[1] && c[2] > colour[2] && rgb_near(c, colour, 40), "dpr {dpr} state {st}: centre {c:?}");
        }
    }
}

/// A sketch's end point and a centre as live draws them (live 2221, at 1x): at rest a 7 px dot; hovered or selected
/// an 11 px disc in the state colour with the point 30 % through its middle (a centre 10 %): hovered #b38b9b over a
/// blue point, selected #b37745; hovered while selected, as selected.
#[test]
fn a_sketch_point_is_drawn_as_lives_in_each_state() {
    let _gpu = GPU.lock().unwrap_or_else(|e| e.into_inner());
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let point = PointSprite { position: [0.0; 3], color: UNDER_DEFINED, pick_id: 1, size: 6.7, style: PointSprite::SKETCH_POINT | 2, _pad: 0 };
    let centre = PointSprite { style: point.style | PointSprite::CENTER, ..point };
    for dpr in [1.0f32, 2.0] {
        let rest = sketch_point_rows(&mut r, point, dpr, 0);
        assert!((row_width(&rest, dpr) - 7.0).abs() <= 1.0, "dpr {dpr}: at rest {} px", row_width(&rest, dpr));
        assert!(rgb_near(row_at(&rest, dpr, 0.0), [0, 0, 208], 6), "dpr {dpr}: {:?}", row_at(&rest, dpr, 0.0));
        for (sprite, st, middle) in [
            (point, state::HOVER, [179, 139, 155]),
            (point, state::SELECTED, [179, 119, 69]),
            (point, state::HOVER | state::SELECTED, [179, 119, 69]),
            (centre, state::HOVER, [229, 178, 140]),
            (centre, state::SELECTED, [229, 152, 30]),
        ] {
            let lit = sketch_point_rows(&mut r, sprite, dpr, st);
            assert!((row_width(&lit, dpr) - 11.0).abs() <= 1.0, "dpr {dpr} state {st}: {} px", row_width(&lit, dpr));
            let disc = if st & state::SELECTED != 0 { [255, 169, 10] } else { [255, 198, 133] };
            assert!(rgb_near(row_at(&lit, dpr, 4.0), disc, 8), "dpr {dpr} state {st}: disc {:?}", row_at(&lit, dpr, 4.0));
            assert!(rgb_near(row_at(&lit, dpr, 0.0), middle, 8), "dpr {dpr} state {st}: middle {:?}, live {middle:?}", row_at(&lit, dpr, 0.0));
        }
    }
}

/// User 11-12 (hat15b): a hovered sketch point on a plane seen at a slant drew half its disc (the far half hidden by
/// the plane under it, which read as an orange disc beside the point). Lit on a face tilted 60° from the view, the
/// disc is whole: the hover colour 4.5 px above and below the point as at its sides.
#[test]
fn a_hovered_sketch_point_on_a_slanted_face_draws_its_whole_disc() {
    let _gpu = GPU.lock().unwrap_or_else(|e| e.into_inner());
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let n = 64u32;
    let mut cam = Camera::default();
    cam.fit_sphere(glam::DVec3::ZERO, 0.01, 1.0);
    // The face through the point, its normal 60° off the view direction (about the camera's right).
    let (fwd, up) = (cam.forward(), cam.up());
    let right = fwd.cross(up).normalize();
    let normal = -fwd * 0.5 + up * (3f64.sqrt() / 2.0);
    let along = normal.cross(right).normalize();
    let corner = |a: f64, b: f64| { let q = right * a + along * b; [q.x as f32, q.y as f32, q.z as f32] };
    let e = 0.02;
    let face = MeshData { positions: vec![corner(-e, -e), corner(e, -e), corner(e, e), corner(-e, e)], normals: vec![[normal.x as f32, normal.y as f32, normal.z as f32]; 4], pick_ids: vec![2; 4], indices: vec![0, 1, 2, 0, 2, 3] };
    let point = PointSprite { position: [0.0; 3], color: UNDER_DEFINED, pick_id: 1, size: 6.7, style: PointSprite::SKETCH_POINT, _pad: 0 };
    let mut scene = Scene::default();
    scene.instances.push(Instance::new(Drawable::Mesh(r.upload_mesh(&face))).with_color([0.9, 0.9, 0.9, 1.0]).unlit());
    scene.instances.push(Instance::new(Drawable::Points(r.upload_points(&PointData { points: vec![point] }))).per_primitive_color());
    scene.set_state(1, state::HOVER);
    let f = r.render(RenderParams { scene: &scene, camera: &cam, width: n, height: n, device_pixel_ratio: 1.0, style: &Style { msaa: 1, ..Style::default() }, pick: false }).unwrap();
    let at = |dx: i32, dy: i32| {
        let (x, y) = ((n / 2) as i32 + dx, (n / 2) as i32 + dy);
        let i = ((y as u32 * n + x as u32) * 4) as usize;
        [f.bgra[i + 2], f.bgra[i + 1], f.bgra[i]]
    };
    // (Pixel centres 4.5 px from the point either way: it sits on a pixel corner.)
    for (dx, dy) in [(4, 0), (-5, 0), (0, 4), (0, -5)] {
        assert!(rgb_near(at(dx, dy), [255, 198, 133], 8), "the disc at ({dx}, {dy}): {:?}", at(dx, dy));
    }
}

/// DEPTH24-BLANK: on a device without float depth with stencil (24-bit depth + stencil) the box still draws. The
/// format is chosen once a process, so the render runs in a child of this test binary under `NOSHAPE_DEPTH24`.
#[test]
fn the_24_bit_depth_fallback_draws() {
    let _gpu = GPU.lock().unwrap_or_else(|e| e.into_inner());
    let out = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "offscreen::depth24_child", "--ignored", "--nocapture", "--test-threads=1"])
        .env("NOSHAPE_DEPTH24", "1")
        .output()
        .unwrap();
    let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(out.status.success() && text.contains("1 passed"), "{text}");
}

#[test]
#[ignore = "run by the_24_bit_depth_fallback_draws, in its own process"]
fn depth24_child() {
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let mesh = r.upload_mesh(&primitives::box_mesh([-0.01, -0.01, -0.01], [0.01, 0.01, 0.01], 1));
    let mut scene = Scene::default();
    let edges = r.upload_lines(&primitives::box_edges([-0.01, -0.01, -0.01], [0.01, 0.01, 0.01], 10, 1.5));
    scene.instances.push(Instance::new(Drawable::Mesh(mesh)).with_color([0.2, 0.4, 0.8, 1.0]));
    scene.instances.push(Instance::new(Drawable::Lines(edges)).with_color([0.02, 0.02, 0.03, 1.0]));
    scene.set_state(2, state::SELECTED);
    let mut cam = Camera::default();
    cam.fit_sphere(glam::DVec3::ZERO, 0.0175, 4.0 / 3.0);
    let style = Style::default();
    let f = r.render(RenderParams { scene: &scene, camera: &cam, width: 400, height: 300, device_pixel_ratio: 1.0, style: &style, pick: true }).unwrap();
    assert!(f.pick.is_some_and(|p| p.ids[150 * 400 + 200] != 0), "the box picks");
    let px = |x: usize, y: usize| { let i = (y * 400 + x) * 4; [f.bgra[i + 2], f.bgra[i + 1], f.bgra[i]] };
    let (centre, corner) = (px(200, 150), px(5, 5));
    assert_ne!(centre, corner, "the box over the background: centre {centre:?}, corner {corner:?}");
}

/// The 3D pick matrix (docs/parity/interaction-feel.md "3D"): live's reach outside sketch mode (live/feel/3d.json
/// "hover_reach_px", "priority_results"), one row per target and direction, at a device pixel ratio of 1 and 2, on
/// the pick buffer as the viewport picks it (the 5 px square, the kinds' priorities as `PickKind::priority` × 2).
/// From above: a 40 × 20 × 10 mm block with its front top edge (1 px, as the builder draws a part edge) and that
/// edge's left vertex (6 px); a shown sketch's line (2 px) and standalone point (7 px) off the part; a plane's border
/// (2 px); an assembly instance's edge; a sketch line over another part edge and a sketch point on another vertex.
/// Prints matched / total; fails when a matched row stops matching, or a gap starts to.
#[test]
fn the_3d_pick_matrix_matches_live() {
    let _gpu = GPU.lock().unwrap_or_else(|e| e.into_inner());
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let (w, h) = (360u32, 240u32);
    let (lo, hi) = ([-0.02f32, -0.01, -0.005], [0.02f32, 0.01, 0.005]);
    let mut cam = Camera::default();
    cam.set_view(StandardView::Top);
    cam.fit_sphere(glam::DVec3::new(0.0, 0.01, 0.0), 0.05, w as f64 / h as f64);
    let (edge, vertex, line, point, border, line_on_edge, point_on_vertex) = (50u32, 60u32, 70u32, 80u32, 90u32, 71u32, 81u32);
    let mut lines = LineData::default();
    // The front top edge's left half, and a sketch line drawn over its right half (a part edge under it).
    lines.push_polyline(&[[lo[0], lo[1], hi[2]], [0.0, lo[1], hi[2]]], [0, 0, 0, 255], edge, 1.0, LineSegment::CORE);
    lines.push_polyline(&[[0.0, lo[1], hi[2]], [hi[0], lo[1], hi[2]]], [0, 0, 0, 255], 51, 1.0, LineSegment::CORE);
    lines.push_polyline(&[[0.0, lo[1], hi[2]], [hi[0], lo[1], hi[2]]], [0, 0, 255, 255], line_on_edge, 2.0, 0);
    // Off the part: a shown sketch's line, a plane's border.
    lines.push_polyline(&[[-0.03, 0.03, 0.0], [0.0, 0.03, 0.0]], [0, 0, 255, 255], line, 2.0, 0);
    lines.push_polyline(&[[-0.03, 0.045, 0.0], [0.03, 0.045, 0.0]], [0, 0, 0, 255], border, 2.0, LineSegment::NO_HOLD | LineSegment::WIDE_PICK);
    // An assembly instance's edge (1.25 px, NO_HOLD as the assembly draws it), off the part.
    let asm_edge = 92u32;
    lines.push_polyline(&[[0.05, -0.02, 0.0], [0.05, 0.04, 0.0]], [0, 0, 0, 255], asm_edge, 1.25, LineSegment::NO_HOLD);
    let sprite = |p: [f32; 3], id: u32, size: f32, style: u32| PointSprite { position: p, color: [0, 0, 0, 255], pick_id: id, size, style, _pad: 0 };
    let points = PointData {
        points: vec![
            sprite([lo[0], lo[1], hi[2]], vertex, 6.0, 0),
            sprite([hi[0], lo[1], hi[2]], 61, 6.0, 0),
            sprite([hi[0], lo[1], hi[2]], point_on_vertex, 7.0, 128),
            sprite([0.015, 0.03, 0.0], point, 7.0, 128),
        ],
    };
    let base = Scene {
        instances: vec![
            Instance::new(Drawable::Mesh(r.upload_mesh(&primitives::box_mesh(lo, hi, 1)))).with_color([0.6, 0.7, 0.8, 1.0]),
            Instance::new(Drawable::Lines(r.upload_lines(&lines))).per_primitive_color().layer(10),
            Instance::new(Drawable::Points(r.upload_points(&points))).per_primitive_color().layer(11),
        ],
        ..Default::default()
    };
    let prio = move |id: u32| match id {
        0 => -1,
        x if x == point || x == point_on_vertex => 16,
        x if x == line || x == line_on_edge => 14,
        60 | 61 => 10,
        50 | 51 | 92 => 8,
        x if x == border => 0,
        _ => 2,
    };
    // (Row, the target's id, its anchor, the direction off it on screen (x right, y down; measured both ways and
    // averaged: where a target sits in its pixel moves one side's reach a pixel against the other's, live's too),
    // hovered, live's "lights at" range in px, ours not there yet.)
    type Row = (&'static str, u32, [f32; 3], (f64, f64), bool, (i32, i32), bool);
    let d = std::f64::consts::FRAC_1_SQRT_2;
    let rows: Vec<Row> = vec![
        ("edge / across (over the background and its face)", edge, [-0.01, lo[1], hi[2]], (0.0, 1.0), false, (6, 7), false),
        ("edge, lit / holds", edge, [-0.01, lo[1], hi[2]], (0.0, 1.0), true, (9, 10), false),
        ("vertex / straight out", vertex, [lo[0], lo[1], hi[2]], (1.0, 0.0), false, (7, 7), false),
        ("vertex / diagonally", vertex, [lo[0], lo[1], hi[2]], (d, d), false, (8, 10), false),
        ("vertex, lit / stays lit", vertex, [lo[0], lo[1], hi[2]], (1.0, 0.0), true, (7, 8), false),
        ("sketch line (not editing)", line, [-0.015, 0.03, 0.0], (0.0, 1.0), false, (5, 6), false),
        ("sketch line, lit / stays lit", line, [-0.015, 0.03, 0.0], (0.0, 1.0), true, (8, 10), false),
        ("sketch point (not editing)", point, [0.015, 0.03, 0.0], (1.0, 0.0), false, (7, 7), false),
        ("sketch point, lit / stays lit", point, [0.015, 0.03, 0.0], (1.0, 0.0), true, (7, 8), false),
        ("plane border", border, [0.0, 0.045, 0.0], (0.0, 1.0), false, (7, 8), false),
        ("plane border, lit / no hold", border, [0.0, 0.045, 0.0], (0.0, 1.0), true, (7, 8), false),
        ("assembly instance edge", asm_edge, [0.05, 0.0, 0.0], (1.0, 0.0), false, (6, 6), false),
        ("assembly instance edge, lit / no hold", asm_edge, [0.05, 0.0, 0.0], (1.0, 0.0), true, (6, 6), false),
        // Priority: on the spot, the sketch line over the part edge under it, the sketch point over the vertex.
        ("priority / sketch line over a part edge", line_on_edge, [0.01, lo[1], hi[2]], (0.0, 1.0), false, (0, 99), false),
        ("priority / sketch point over a vertex", point_on_vertex, [hi[0], lo[1], hi[2]], (1.0, 0.0), false, (0, 99), false),
    ];
    let mut report = Vec::new();
    let mut moved = Vec::new();
    for (name, id, anchor, dir, lit, (a, b), gap) in &rows {
        let mut scene = base.clone();
        if *lit {
            scene.add_flag(*id, noshape_render::scene::state::HOVER);
        }
        let mut ok = true;
        let mut seen = Vec::new();
        for dpr in [1.0f32, 2.0] {
            let (dw, dh) = ((w as f32 * dpr) as u32, (h as f32 * dpr) as u32);
            let f = r.render(RenderParams { scene: &scene, camera: &cam, width: dw, height: dh, device_pixel_ratio: dpr, style: &Style::default(), pick: true }).unwrap();
            let pick = f.pick.unwrap();
            let at0 = cam.project(glam::DVec3::new(anchor[0] as f64, anchor[1] as f64, anchor[2] as f64), glam::DVec2::new(dw as f64, dh as f64), 0.05).unwrap();
            let dir = glam::DVec2::new(dir.0, dir.1);
            let at = |k: f64| {
                let q = at0 + dir * (k * dpr as f64);
                pick.pick(q.x.round() as i32, q.y.round() as i32, (5.0 * dpr) as i32, prio)
            };
            let at_dir = |at: &dyn Fn(f64) -> Option<u32>, k: i32, s: f64| at(k as f64 * s);
            let side = |s: f64| (0..40).take_while(|k| at_dir(&at, *k, s) == Some(*id)).count() as i32 - 1;
            // (Both ways, averaged: half a pixel rounds up.)
            let reach = (side(1.0) + side(-1.0) + 1) / 2;
            seen.push(reach);
            ok &= (*a..=*b).contains(&reach);
        }
        report.push(format!("{} {name}: ours {seen:?}, live {a}–{b}", if ok { "=" } else { "≠" }));
        if ok == *gap {
            moved.push(*name);
        }
    }
    let matched = report.iter().filter(|l| l.starts_with('=')).count();
    eprintln!("3D pick matrix: {matched} / {} rows match live\n{}", rows.len(), report.join("\n"));
    assert!(moved.is_empty(), "rows whose status moved (update interaction-feel.md \"3D\" and the row's gap): {moved:?}");
}

/// The ink across a part's edge at a device pixel ratio of 1, in px (darkness summed down a pixel column through the
/// edge, against the darker surface beside it, times the edge's slope): live/feel/look.json's 1 px edges carry ~0.84
/// (live/feel/look/part-studio-iso.png, never reaching black). Median over the block's visible edges.
fn edge_ink(r: &mut Renderer, width: f32) -> f64 {
    let (lo, hi) = ([0.0f32, 0.0, 0.0], [0.04f32, 0.03, 0.01]);
    let mut scene = Scene::default();
    scene.instances.push(Instance::new(Drawable::Mesh(r.upload_mesh(&primitives::box_mesh(lo, hi, 1)))).with_color(srgb(0x9a, 0xb4, 0xcc)));
    let mut lines = LineData::default();
    let corner = |i: u32| [if i & 1 == 0 { lo[0] } else { hi[0] }, if i & 2 == 0 { lo[1] } else { hi[1] }, if i & 4 == 0 { lo[2] } else { hi[2] }];
    let pairs = [(0, 1), (2, 3), (4, 5), (6, 7), (0, 2), (1, 3), (4, 6), (5, 7), (0, 4), (1, 5), (2, 6), (3, 7)];
    for (k, (a, b)) in pairs.iter().enumerate() {
        lines.push_polyline(&[corner(*a), corner(*b)], [0, 0, 0, 255], 10 + k as u32, width, LineSegment::CORE);
    }
    scene.instances.push(Instance::new(Drawable::Lines(r.upload_lines(&lines))).per_primitive_color().layer(1));
    let mut cam = Camera::default();
    cam.set_view(noshape_render::StandardView::Isometric);
    let (w, h) = (900u32, 600u32);
    cam.fit_box(glam::Vec3::from(lo).as_dvec3(), glam::Vec3::from(hi).as_dvec3(), w as f64 / h as f64);
    cam.distance *= 1.6;
    let f = r.render(RenderParams { scene: &scene, camera: &cam, width: w, height: h, device_pixel_ratio: 1.0, style: &Style::default(), pick: false }).unwrap();
    let lum = |x: i64, y: i64| {
        let px = &f.bgra[((y as u32 * w + x as u32) * 4) as usize..][..3];
        0.0722 * px[0] as f64 + 0.7152 * px[1] as f64 + 0.2126 * px[2] as f64
    };
    let vp = glam::DVec2::new(w as f64, h as f64);
    let radius = (scene.bounds().map(|(a, b)| (b - a).length() * 0.5).unwrap_or(1.0)) as f64;
    let c3 = |i: u32| glam::Vec3::from(corner(i)).as_dvec3();
    let mut inks = Vec::new();
    // The iso view's visible edges: the top face's four and the three verticals in front.
    for (a, b) in [(4, 5), (6, 7), (4, 6), (5, 7), (1, 5), (2, 6), (3, 7)] {
        let (sa, sb) = (cam.project(c3(a), vp, radius).unwrap(), cam.project(c3(b), vp, radius).unwrap());
        let d = (sb - sa).normalize();
        if d.x.abs() < 0.3 {
            continue;
        }
        for k in 8..32 {
            let s = sa.lerp(sb, k as f64 / 40.0);
            let (x, y0) = (s.x.floor() as i64, s.y.round() as i64);
            let col: Vec<f64> = (y0 - 8..=y0 + 8).map(|y| lum(x, y)).collect();
            let m = (5..12).min_by(|i, j| col[*i].total_cmp(&col[*j])).unwrap();
            let refl = ((col[m - 5] + col[m - 4]) * 0.5).min((col[m + 4] + col[m + 5]) * 0.5);
            let ink: f64 = (m - 2..=m + 2).map(|i| ((refl - col[i]) / refl).clamp(0.0, 1.0)).sum();
            inks.push(ink * d.x.abs());
        }
    }
    inks.sort_by(f64::total_cmp);
    inks[inks.len() / 2]
}

/// look.json: a part's edges carry live's ink (~0.84 px at a pixel ratio of 1; 1.25 px edges drew ~1.15).
#[test]
fn part_edges_carry_lives_ink() {
    let _gpu = GPU.lock().unwrap_or_else(|e| e.into_inner());
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    let ink = edge_ink(&mut r, PART_EDGE_WIDTH);
    assert!((ink - 0.84).abs() <= 0.1, "a part edge's ink: ours {ink:.2} px, live 0.84");
}
