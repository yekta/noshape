//! Picking on imported parts with B-spline faces and edges (lead 11-27: the user's sketch selection failed on used
//! B-spline geometry; this is the Part Studio side): every face and edge an import shows lights under the pointer and
//! a click takes it, with nothing, itself or another entity selected; clicks add and Shift toggles; a box takes them;
//! Select ▸ grows across them as their geometry says.

use super::*;
use super::feel::{Pointer, gpu_pointer};
use crate::part_studio::face_depths;
use glam::DVec3;
use noshape_kernel::{Shape, ShapeType};
use std::collections::BTreeSet;

/// The files, and how much of the part the view shows (the thread's turns lie closer than the pick's reach from afar).
const FILES: [(&str, f64); 2] = [("step/g25-b.step", 0.35), ("booleans/fan-blade-thread.step", 0.12)];

fn import(cx: &mut TestAppContext, name: &str, file: &str) -> (AnyWindowHandle, Entity<PartStudioView>) {
    let file = format!("{}/../kernel/tests/data/{file}", env!("CARGO_MANIFEST_DIR"));
    let (window, view) = part_studio_with(cx, name, |path| {
        let mut ctx = noshape_ops::Ctx::new();
        for (tool, args) in [("document_create", serde_json::json!({ "path": path })), ("import_files", serde_json::json!({ "files": [file] }))] {
            noshape_ops::registry::run(&mut ctx, noshape_ops::registry::by_tool_name(tool).unwrap(), &args).unwrap();
        }
        drop(ctx);
        let mut s = crate::session::Session::open(path).unwrap();
        s.doc.tabs.retain(|t| t.name != "Part Studio 1");
        s
    });
    cx.update_window(window, |_, w, _| w.activate_window()).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && view.read(cx).display.bodies.iter().any(|b| !b.face_refs.is_empty()));
    (window, view)
}

fn v3(q: &[f32; 3]) -> DVec3 {
    DVec3::new(q[0] as f64, q[1] as f64, q[2] as f64)
}

/// Distance from `p` to segment `a b`.
fn seg_dist(p: DVec2, a: DVec2, b: DVec2) -> f64 {
    let ab = b - a;
    let t = if ab.length_squared() > 0.0 { ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
    p.distance(a + ab * t)
}

/// Each entity with a clear spot on screen, with that spot.
type Spots = Vec<(Reference, DVec2)>;

/// The part fitted in the iso view; then each face's and edge's clearest spot on screen: shown in front, away from
/// every other edge and vertex (10 px, past the 6-7 px reach), the farthest such. Entities without one are left out.
fn spots(cx: &mut TestAppContext, window: AnyWindowHandle, view: &Entity<PartStudioView>, fit: f64, from: noshape_render::StandardView) -> (Spots, Spots, usize, usize) {
    cx.update_window(window, |_, _, cx| {
        view.update(cx, |v, cx| {
            let ps: Vec<DVec3> = v.display.bodies.iter().flat_map(|b| b.mesh.positions.iter().map(v3)).collect();
            let lo = ps.iter().fold(DVec3::splat(f64::MAX), |a, q| a.min(*q));
            let hi = ps.iter().fold(DVec3::splat(f64::MIN), |a, q| a.max(*q));
            v.viewport.update(cx, |vp, cx| {
                let size = vp.size();
                vp.camera.set_view(from);
                vp.camera.fit_sphere((lo + hi) * 0.5, (hi - lo).length() * fit, size.x / size.y);
                vp.request_frame(cx);
            });
        })
    })
    .unwrap();
    draw(cx, window);
    cx.update(|cx| {
        let v = view.read(cx);
        let vp = v.viewport.read(cx);
        let size = vp.size();
        // (Clear of the panels and the view cube over the graphics' sides and corners.)
        let inside = |p: DVec2| p.x > size.x * 0.25 && p.y > size.y * 0.15 && p.x < size.x * 0.8 && p.y < size.y * 0.85;
        let scale = v.display.bodies.iter().flat_map(|b| b.mesh.positions.iter().map(v3)).fold(0.0f64, |a, q| a.max(q.length())).max(1e-3);
        // Projected edges and vertices of every shown body.
        let mut lines: Vec<(Reference, Vec<DVec2>)> = Vec::new();
        let mut points: Vec<DVec2> = Vec::new();
        for b in v.display.bodies.iter().filter(|b| !b.hidden) {
            for (r, pl) in b.edge_refs.iter().zip(&b.mesh.edges) {
                lines.push((r.clone(), pl.iter().filter_map(|q| vp.project(v3(q))).collect()));
            }
            points.extend(b.mesh.vertices.iter().filter_map(|q| vp.project(v3(q))));
        }
        let clear_of = |p: DVec2, own: Option<&Reference>| -> f64 {
            let e = lines.iter().filter(|(r, _)| Some(r) != own).flat_map(|(_, pl)| pl.windows(2).map(move |w| seg_dist(p, w[0], w[1]))).fold(f64::MAX, f64::min);
            points.iter().map(|q| q.distance(p)).fold(e, f64::min)
        };
        let shown = |p: DVec3, at: DVec2| -> bool {
            let (o, d) = vp.ray(at);
            let t = (p - o).dot(d) / d.length_squared();
            face_depths(&v.display, None, o, d).iter().all(|f| *f >= t - 2e-3 * scale)
        };
        let (mut faces, mut edges, mut nf, mut ne) = (Vec::new(), Vec::new(), 0, 0);
        for b in v.display.bodies.iter().filter(|b| !b.hidden) {
            for (r, (first, count)) in b.face_refs.iter().zip(&b.mesh.face_ranges) {
                nf += 1;
                let idx = &b.mesh.indices[*first as usize..(*first + *count) as usize];
                let best = idx
                    .chunks(3)
                    .filter_map(|t| {
                        let c = t.iter().map(|&k| v3(&b.mesh.positions[k as usize])).sum::<DVec3>() / 3.0;
                        let at = vp.project(c)?;
                        let (o, d) = vp.ray(at);
                        let own = face_depths(&v.display, Some(r), o, d).into_iter().fold(f64::MAX, f64::min);
                        let all = face_depths(&v.display, None, o, d).into_iter().fold(f64::MAX, f64::min);
                        (inside(at) && own <= all + 1e-6 * scale).then(|| (clear_of(at, None), at))
                    })
                    .max_by(|a, b| a.0.total_cmp(&b.0));
                if let Some((_, at)) = best.filter(|(gap, _)| *gap >= 10.0) {
                    faces.push((r.clone(), at));
                }
            }
            // (Drawn edges: a degenerated one has no line.)
            for (r, pl) in b.edge_refs.iter().zip(&b.mesh.edges).filter(|(_, pl)| pl.len() > 1) {
                ne += 1;
                // (Away from the edge's middle too, where its midpoint lights instead: live's implicit midpoint.)
                let length: f64 = pl.windows(2).map(|w| v3(&w[0]).distance(v3(&w[1]))).sum();
                let mut run = 0.0;
                let mid = pl.windows(2).find_map(|w| {
                    let (a, c) = (v3(&w[0]), v3(&w[1]));
                    let step = a.distance(c);
                    let found = (run + step >= length * 0.5 && step > 0.0).then(|| a + (c - a) * ((length * 0.5 - run) / step));
                    run += step;
                    found
                });
                let mid = mid.and_then(|m| vp.project(m));
                // (Points along each piece: a straight edge is one piece, its middle the midpoint's.)
                let best = pl
                    .windows(2)
                    .flat_map(|w| [0.2, 0.35, 0.65, 0.8].map(|t| v3(&w[0]).lerp(v3(&w[1]), t)))
                    .filter_map(|p| {
                        let at = vp.project(p)?;
                        let from_mid = mid.map_or(f64::MAX, |m| m.distance(at));
                        (inside(at) && shown(p, at) && from_mid >= 10.0).then(|| (clear_of(at, Some(r)), at))
                    })
                    .max_by(|a, b| a.0.total_cmp(&b.0));
                if let Some((_, at)) = best.filter(|(gap, _)| *gap >= 10.0) {
                    edges.push((r.clone(), at));
                }
            }
        }
        (faces, edges, nf, ne)
    })
}

fn click_with(cx: &mut TestAppContext, window: AnyWindowHandle, view: &Entity<PartStudioView>, pointer: &Pointer, at: DVec2, modifiers: Modifiers) {
    let position = pointer.window_point(cx, at);
    let button = gpui_kit::MouseButton::Left;
    cx.update_window(window, |_, w, cx| {
        w.dispatch_event(gpui_kit::PlatformInput::MouseDown(gpui_kit::MouseDownEvent { button, position, modifiers, click_count: 1, first_mouse: false }), cx);
        w.dispatch_event(gpui_kit::PlatformInput::MouseUp(gpui_kit::MouseUpEvent { button, position, modifiers, click_count: 1 }), cx);
    })
    .unwrap();
    cx.run_until_parked();
    pointer.settle(cx, view);
}

fn selected(cx: &mut TestAppContext, view: &Entity<PartStudioView>) -> BTreeSet<String> {
    cx.update(|cx| view.read(cx).selection.iter().map(|r| format!("{r:?}")).collect())
}

fn set_of(rs: &[&Reference]) -> BTreeSet<String> {
    rs.iter().map(|r| format!("{r:?}")).collect()
}

/// Every face and edge with a clear spot, one after another under the real pointer: it lights with nothing selected
/// (the first) or the one before selected, a click adds it, the one before lights while both are selected and a Shift
/// click takes it out, and the entity lights again while it is the one selected.
#[gpui::test]
#[ignore = "T2: slow (every entity of two imports from 7 views at 2 zooms under the GPU, ~4 min)"]
fn bspline_faces_and_edges_of_an_import_light_and_click_in_every_selection_state(cx: &mut TestAppContext) {
    let Some(_gpu) = gpu_pointer() else { return };
    RENDERING.set(true);
    let mut wrong = Vec::new();
    for (k, (file, fit)) in FILES.iter().enumerate() {
        let (window, view) = import(cx, &format!("bspline-pick-{k}"), file);
        let pointer = Pointer::new(cx, window, &view);
        let lit = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).hover.clone());
        // From every side: what one view hides or crowds another shows.
        let mut seen: BTreeSet<String> = BTreeSet::new();
        let mut counts = (0, 0);
        use noshape_render::StandardView as V;
        for (from, zoom) in [V::Isometric, V::Top, V::Bottom, V::Front, V::Back, V::Left, V::Right].into_iter().flat_map(|v| [(v, 1.0), (v, 2.0)]) {
            let (faces, edges, nf, ne) = spots(cx, window, &view, *fit * zoom, from);
            counts = (nf, ne);
            seen.extend(faces.iter().chain(&edges).map(|(r, _)| format!("{r:?}")));
            view.update(cx, |v, _| v.selection.clear());
            let all: Vec<&(Reference, DVec2)> = faces.iter().chain(&edges).collect();
            let mut before: Option<&(Reference, DVec2)> = None;
            for e in &all {
                let (r, at) = (&e.0, e.1);
                let state = if before.is_some() { "another selected" } else { "nothing selected" };
                pointer.move_to(cx, &view, at);
                if lit(cx).as_ref() != Some(r) {
                    wrong.push(format!("{file}: {r:?} at ({:.0}, {:.0}), {state}: lit {:?}", at.x, at.y, lit(cx)));
                }
                pointer.click(cx, &view, at);
                let want: Vec<&Reference> = before.map(|b| &b.0).into_iter().chain([r]).collect();
                if selected(cx, &view) != set_of(&want) {
                    wrong.push(format!("{file}: a click on {r:?}, {state}: selection {:?}", selected(cx, &view)));
                }
                if let Some(b) = before {
                    pointer.move_to(cx, &view, b.1);
                    if lit(cx).as_ref() != Some(&b.0) {
                        wrong.push(format!("{file}: {:?}, both selected: lit {:?}", b.0, lit(cx)));
                    }
                    click_with(cx, window, &view, &pointer, b.1, Modifiers { shift: true, ..Default::default() });
                    if selected(cx, &view) != set_of(&[r]) {
                        wrong.push(format!("{file}: Shift+click on the selected {:?}: selection {:?}", b.0, selected(cx, &view)));
                    }
                    pointer.move_to(cx, &view, at);
                    if lit(cx).as_ref() != Some(r) {
                        wrong.push(format!("{file}: {r:?}, itself selected: lit {:?}", lit(cx)));
                    }
                }
                // (Re-synced: one wrong step doesn't spoil the rest.)
                view.update(cx, |v, _| {
                    v.selection.clear();
                    v.selection.push(r.clone());
                });
                before = Some(e);
            }
        }
        let (nf, ne) = counts;
        let (f, e) = (seen.iter().filter(|r| r.starts_with("Face")).count(), seen.len() - seen.iter().filter(|r| r.starts_with("Face")).count());
        eprintln!("bspline-pick {file}: {f} of {nf} faces and {e} of {ne} edges had a clear spot");
        assert!(f * 2 >= nf && e * 3 >= ne, "{file}: too few clear spots ({f} / {nf} faces, {e} / {ne} edges)");
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// No GPU: a window round the import takes every face and edge (live box_select: the hidden ones too), a crossing box
/// over an edge's clear spot takes that edge, and Select ▸ Tangent connected faces / edges reach as far as the
/// geometry is smooth: faces whose normals agree along their shared edge, edges whose tangents agree at their shared
/// vertex (sampled along the whole edge, so a B-spline's middle alone doesn't decide).
#[gpui::test]
fn bspline_faces_and_edges_of_an_import_box_select_and_grow_as_their_geometry(cx: &mut TestAppContext) {
    let mut wrong = Vec::new();
    for (k, (file, fit)) in FILES.iter().enumerate() {
        let (window, view) = import(cx, &format!("bspline-grow-{k}"), file);
        // (The window round the whole part.)
        spots(cx, window, &view, 0.55, noshape_render::StandardView::Isometric);
        let (refs, a, b) = cx.update(|cx| {
            let v = view.read(cx);
            let vp = v.viewport.read(cx);
            let mut refs: BTreeSet<String> = BTreeSet::new();
            let (mut a, mut b) = (DVec2::splat(f64::MAX), DVec2::splat(f64::MIN));
            for body in v.display.bodies.iter().filter(|b| !b.hidden && !b.face_refs.is_empty()) {
                // (An edge without a line, a degenerated one, isn't drawn: nothing to take.)
                refs.extend(body.face_refs.iter().map(|r| format!("{r:?}")));
                refs.extend(body.edge_refs.iter().zip(&body.mesh.edges).filter(|(_, pl)| pl.len() > 1).map(|(r, _)| format!("{r:?}")));
                for p in body.mesh.positions.iter().filter_map(|q| vp.project(v3(q))) {
                    a = a.min(p);
                    b = b.max(p);
                }
            }
            (refs, a, b)
        });
        let boxed = |cx: &mut TestAppContext, rect: (DVec2, DVec2), crossing: bool| {
            view.update(cx, |v, _| v.selection.clear());
            cx.update_window(window, |_, w, cx| {
                view.update(cx, |v, cx| {
                    let vp = v.viewport.clone();
                    v.on_viewport_event(&vp, &ViewportEvent::BoxSelect { ids: Vec::new(), modifiers: Modifiers::default(), crossing, rect }, w, cx);
                })
            })
            .unwrap();
            draw(cx, window);
            selected(cx, &view)
        };
        let got = boxed(cx, (a - DVec2::splat(10.0), b + DVec2::splat(10.0)), false);
        let missed: Vec<&String> = refs.difference(&got).collect();
        if !missed.is_empty() {
            wrong.push(format!("{file}: a window round the part misses {} of {}: {missed:?}", missed.len(), refs.len()));
        }
        let (_, edges, _, _) = spots(cx, window, &view, *fit, noshape_render::StandardView::Isometric);
        for (r, at) in &edges {
            let got = boxed(cx, (*at - DVec2::splat(3.0), *at + DVec2::splat(3.0)), true);
            if !got.contains(&format!("{r:?}")) {
                wrong.push(format!("{file}: a crossing box on {r:?} at ({:.0}, {:.0}): {got:?}", at.x, at.y));
            }
        }
        // Select ▸ against the geometry, on the regenerated bodies.
        let (ctx, sel) = cx.update(|cx| {
            let v = view.read(cx);
            let ctx = v.context.clone().unwrap();
            let edges = v.display.bodies.iter().filter(|b| !b.hidden).flat_map(|b| b.edge_refs.iter().zip(&b.mesh.edges)).filter(|(_, pl)| pl.len() > 1).map(|(r, _)| r.clone());
            (ctx, v.display.bodies.iter().filter(|b| !b.hidden).flat_map(|b| b.face_refs.iter().cloned()).chain(edges).collect::<Vec<_>>())
        });
        for body in ctx.bodies.iter() {
            let Ok(edge_faces) = body.shape.edge_faces() else { continue };
            let vertex_edges = body.shape.vertex_edges().unwrap_or_default();
            let face = |i: usize| body.shape.sub_shape(ShapeType::Face, i).ok();
            let edge = |i: usize| body.shape.sub_shape(ShapeType::Edge, i).ok();
            // The largest angle between the faces' normals along the edge (9 samples), degrees.
            let crease = |e: usize, fa: usize, fb: usize| -> Option<f64> {
                let (s, a, c) = (edge(e)?, face(fa)?, face(fb)?);
                let info = s.edge_info().ok()?;
                let mut worst: f64 = 0.0;
                for k in 1..10 {
                    let p = s.point_at(info.first_param + (info.last_param - info.first_param) * k as f64 / 10.0).ok()?;
                    let (na, nc) = (a.normal_at_point(p).ok()?.normalize_or_zero(), c.normal_at_point(p).ok()?.normalize_or_zero());
                    worst = worst.max(na.dot(nc).abs().min(1.0).acos().to_degrees());
                }
                Some(worst)
            };
            let kind = |s: Option<Shape>, is_face: bool| -> String {
                let Some(s) = s else { return "?".into() };
                if is_face { format!("{:?}", s.face_surface().ok()).chars().take(24).collect() } else { format!("{:?}", s.edge_curve().ok()).chars().take(24).collect() }
            };
            let face_ref = |i: usize| Reference::Face((*body.faces[i]).clone());
            let edge_ref = |i: usize| Reference::Edge((*body.edges[i]).clone());
            // Tangent connected faces, from every face.
            let smooth: Vec<Option<bool>> = edge_faces.iter().enumerate().map(|(e, fs)| (fs.len() == 2).then(|| crease(e, fs[0] as usize, fs[1] as usize).is_some_and(|d| d < 1.0))).collect();
            for f in 0..body.faces.len() {
                let mut want = BTreeSet::from([f]);
                let mut queue = vec![f];
                while let Some(g) = queue.pop() {
                    for (e, fs) in edge_faces.iter().enumerate() {
                        if smooth[e] == Some(true) && fs.contains(&(g as u32)) {
                            for &o in fs {
                                if want.insert(o as usize) {
                                    queue.push(o as usize);
                                }
                            }
                        }
                    }
                }
                let want: BTreeSet<String> = want.into_iter().map(|i| format!("{:?}", face_ref(i))).collect();
                let got: BTreeSet<String> = cx.update(|cx| view.read(cx).related(&face_ref(f), noshape_model::select::Criterion::TangentFaces)).iter().map(|r| format!("{r:?}")).collect();
                if got != want {
                    let creases: Vec<String> = edge_faces.iter().enumerate().filter(|(_, fs)| fs.len() == 2 && fs.contains(&(f as u32))).map(|(e, fs)| format!("e{e} {} {:?}°", kind(edge(e), false), crease(e, fs[0] as usize, fs[1] as usize).map(|d| (d * 100.0).round() / 100.0))).collect();
                    wrong.push(format!("{file}: Tangent connected faces from face {f} ({}): {} faces, the geometry {}; its edges {creases:?}", kind(face(f), true), got.len(), want.len()));
                }
            }
            // Tangent connected edges, from every edge: tangents at each shared vertex.
            let ends = |e: usize| -> Vec<(usize, DVec3)> {
                let Some(s) = edge(e) else { return Vec::new() };
                let Ok(info) = s.edge_info() else { return Vec::new() };
                let mut out = Vec::new();
                for (vi, _) in vertex_edges.iter().enumerate().filter(|(_, es)| es.contains(&(e as u32))) {
                    let Ok(p) = body.shape.sub_shape(ShapeType::Vertex, vi).and_then(|x| x.vertex_point()) else { continue };
                    for (q, t) in [(info.start, info.first_param), (info.end, info.last_param)] {
                        if q.distance(p) < 1e-6 && let Ok(d) = s.tangent_at(t) {
                            out.push((vi, d.normalize_or_zero()));
                        }
                    }
                }
                out
            };
            for e in 0..body.edges.len() {
                if edge(e).and_then(|s| s.edge_info().ok()).is_some_and(|i| i.degenerated) {
                    continue;
                }
                let mut want = BTreeSet::from([e]);
                let mut queue = vec![e];
                while let Some(g) = queue.pop() {
                    for (vi, t) in ends(g) {
                        for &o in &vertex_edges[vi] {
                            let o = o as usize;
                            if !want.contains(&o) && ends(o).iter().any(|(v2, t2)| *v2 == vi && t2.dot(t).abs() > 0.9998) {
                                want.insert(o);
                                queue.push(o);
                            }
                        }
                    }
                }
                let want: BTreeSet<String> = want.into_iter().map(|i| format!("{:?}", edge_ref(i))).collect();
                let got: BTreeSet<String> = cx.update(|cx| view.read(cx).related(&edge_ref(e), noshape_model::select::Criterion::TangentEdges)).iter().map(|r| format!("{r:?}")).collect();
                if got != want {
                    wrong.push(format!("{file}: Tangent connected edges from edge {e} ({}): {} edges, the geometry {}", kind(edge(e), false), got.len(), want.len()));
                }
            }
        }
        // The other Select ▸ items run on every shown face and edge and keep their seed.
        for r in &sel {
            let criteria: &[noshape_model::select::Criterion] = if matches!(r, Reference::Face(_)) { &noshape_model::select::Criterion::FACE } else { &noshape_model::select::Criterion::EDGE };
            for c in criteria {
                let got = cx.update(|cx| view.read(cx).related(r, *c));
                let seeded = got.contains(r) || matches!(c, noshape_model::select::Criterion::FaceLoop | noshape_model::select::Criterion::Hole | noshape_model::select::Criterion::Fillets);
                if !seeded {
                    wrong.push(format!("{file}: Select ▸ {} on {r:?}: {} entities, without the seed", c.label(), got.len()));
                }
                let undrawn: Vec<&Reference> = got.iter().filter(|g| !sel.contains(g)).collect();
                if !undrawn.is_empty() {
                    wrong.push(format!("{file}: Select ▸ {} on {r:?} takes {} entities not drawn: {undrawn:?}", c.label(), undrawn.len()));
                }
            }
        }
    }
    for w in &wrong {
        eprintln!("bspline-grow: {w}");
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}
