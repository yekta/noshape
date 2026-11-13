//! The navigation matrix (docs/parity/interaction-feel.md "Navigation"): live2's navigation.json rows acted out with
//! real wheel notches, drags and keys in a Part Studio's graphics (the block, orthographic), at a device pixel ratio
//! of 1 and 2, the camera compared with live's numbers. The viewport's press and animation clock is the test's. A row ours doesn't match yet is a gap; the test fails when a
//! row's status moves either way.

use super::*;
use crate::viewport::Viewport;
use glam::{DVec2, DVec3};
use gpui_kit::{MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, ScrollDelta, ScrollWheelEvent, point};
use noshape_render::Camera;

struct Nav {
    window: AnyWindowHandle,
    view: Entity<PartStudioView>,
    vp: Entity<Viewport>,
    /// The viewport's press and animation clock, moved only by the test (T1: no wall-clock time).
    clock: std::rc::Rc<std::cell::Cell<std::time::Instant>>,
}

const NONE: Modifiers = Modifiers { shift: false, control: false, alt: false, platform: false, function: false };

impl Nav {
    fn open(cx: &mut TestAppContext, name: &str, scale: f32) -> Self {
        let block = serde_json::json!({ "units": "mm", "features": [
            { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [40, 30]] }] },
            { "type": "extrude", "name": "Extrude 1", "params": { "entities": ["Sketch 1 region 0"], "endBound": "BLIND", "depth": "10 mm", "operationType": "NEW" } }
        ] });
        let (window, view) = part_studio_from(cx, name, Some(block));
        cx.update_window(window, |_, w, _| {
            w.set_scale_factor(scale);
            w.activate_window();
        })
        .unwrap();
        wait_until(cx, window, |cx| !view.read(cx).ui_busy() && !view.read(cx).display.bodies.is_empty());
        let vp = cx.update(|cx| view.read(cx).viewport.clone());
        let clock = std::rc::Rc::new(std::cell::Cell::new(std::time::Instant::now()));
        let c = clock.clone();
        cx.update(|cx| vp.update(cx, |vp, _| vp.set_clock(move || c.get())));
        let nav = Nav { window, view, vp, clock };
        nav.settle(cx);
        nav
    }

    fn cam(&self, cx: &mut TestAppContext) -> Camera {
        cx.update(|cx| self.vp.read(cx).camera)
    }

    fn size(&self, cx: &mut TestAppContext) -> DVec2 {
        cx.update(|cx| self.vp.read(cx).size())
    }

    /// A viewport point (from its top left) in window coordinates.
    fn at(&self, cx: &mut TestAppContext, p: DVec2) -> Point<Pixels> {
        let o = cx.update(|cx| self.vp.read(cx).origin());
        point(o.x + px(p.x as f32), o.y + px(p.y as f32))
    }

    fn centre(&self, cx: &mut TestAppContext) -> DVec2 {
        self.size(cx) * 0.5
    }

    fn event(&self, cx: &mut TestAppContext, e: impl gpui_kit::InputEvent) {
        gpui_kit::VisualTestContext::from_window(self.window, cx).simulate_event(e);
        draw(cx, self.window);
    }

    /// Lets the camera come to rest: an animation, arrow steps, held drag moves (real time: the viewport animates on
    /// the clock).
    fn settle(&self, cx: &mut TestAppContext) {
        let t = std::time::Instant::now();
        while cx.update(|cx| self.vp.read(cx).camera_moving()) && t.elapsed() < std::time::Duration::from_secs(10) {
            self.tick(20);
            std::thread::sleep(std::time::Duration::from_millis(5));
            draw(cx, self.window);
        }
        draw(cx, self.window);
    }

    fn tick(&self, ms: u64) {
        self.clock.set(self.clock.get() + std::time::Duration::from_millis(ms));
    }

    /// Starts from the isometric view, fitted.
    fn reset(&self, cx: &mut TestAppContext) {
        self.settle(cx);
        cx.update(|cx| self.vp.update(cx, |v, cx| v.animate_to_view(noshape_render::StandardView::Isometric, cx)));
        self.settle(cx);
    }

    fn wheel(&self, cx: &mut TestAppContext, at: DVec2, notches: f32, horizontal: bool) {
        let p = self.at(cx, at);
        self.event(cx, MouseMoveEvent { position: p, pressed_button: None, modifiers: NONE });
        let (x, y) = if horizontal { (notches, 0.) } else { (0., notches) };
        self.event(cx, ScrollWheelEvent { position: p, delta: ScrollDelta::Lines(point(x, y * 3.)), modifiers: NONE, wheel_ticks: Some(point(x, y)), ..Default::default() });
    }

    /// A drag with `button` from `from` by `by` in 5 px steps, `m` held.
    fn drag(&self, cx: &mut TestAppContext, button: MouseButton, from: DVec2, by: DVec2, m: Modifiers) {
        let p0 = self.at(cx, from);
        self.event(cx, MouseMoveEvent { position: p0, pressed_button: None, modifiers: m });
        self.event(cx, MouseDownEvent { button, position: p0, modifiers: m, click_count: 1, first_mouse: false });
        let steps = (by.length() / 5.).ceil().max(1.) as usize;
        for k in 1..=steps {
            let p = self.at(cx, from + by * (k as f64 / steps as f64));
            self.event(cx, MouseMoveEvent { position: p, pressed_button: Some(button), modifiers: m });
        }
        // Held still before the release: no inertia.
        std::thread::sleep(std::time::Duration::from_millis(120));
        let p1 = self.at(cx, from + by);
        self.event(cx, MouseUpEvent { button, position: p1, modifiers: m, click_count: 1 });
        self.settle(cx);
    }

    fn keys(&self, cx: &mut TestAppContext, keys: &str) {
        let focus = cx.update(|cx| self.view.read(cx).focus.clone());
        cx.update_window(self.window, |_, w, cx| w.focus(&focus, cx)).unwrap();
        gpui_kit::VisualTestContext::from_window(self.window, cx).simulate_keystrokes(keys);
        draw(cx, self.window);
    }

    /// The view scale: the visible half height (mm).
    fn half_height(&self, cx: &mut TestAppContext) -> f64 {
        self.cam(cx).half_height() * 1000.
    }

    /// Where world point `w` shows (viewport px).
    fn project(&self, cx: &mut TestAppContext, w: glam::DVec3) -> DVec2 {
        let (c, size) = (self.cam(cx), self.size(cx));
        c.project(w, size, 1.0).unwrap_or(DVec2::NAN)
    }

    /// The world point under viewport pixel `p` on the camera's target plane.
    fn under(&self, cx: &mut TestAppContext, p: DVec2) -> glam::DVec3 {
        let (c, size) = (self.cam(cx), self.size(cx));
        c.unproject_on_target_plane(p, size)
    }

    /// The part's surface point under viewport pixel `p` (the CPU ray the rotate centre comes from).
    fn on_part(&self, cx: &mut TestAppContext, p: DVec2) -> Option<glam::DVec3> {
        let (c, size) = (self.cam(cx), self.size(cx));
        let (o, d) = c.ray(p, size);
        cx.update(|cx| noshape_model::interaction::entities_along_ray(&self.view.read(cx).display, o, d, 0.0, &|_| false, 1).first().map(|h| o + d.normalize() * h.depth))
    }

    /// Where the part's centre shows (viewport px).
    fn part_centre(&self, cx: &mut TestAppContext) -> DVec2 {
        let c = cx.update(|cx| {
            let v = self.view.read(cx);
            let b = v.display.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Solid).expect("the part");
            b.mesh.positions.iter().fold(glam::DVec3::ZERO, |c, p| c + glam::DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64)) / b.mesh.positions.len() as f64
        });
        self.project(cx, c)
    }

    /// Selects the part's top face (its flat face highest up).
    fn select_top(&self, cx: &mut TestAppContext) {
        cx.update(|cx| {
            self.view.update(cx, |v, _| {
                let b = v.display.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Solid).expect("the part");
                let top = b
                    .face_refs
                    .iter()
                    .enumerate()
                    .filter_map(|(i, r)| {
                        let &(first, count) = b.mesh.face_ranges.get(i)?;
                        let z: Vec<f32> = b.mesh.indices[first as usize..(first + count) as usize].iter().map(|&k| b.mesh.positions[k as usize][2]).collect();
                        let (lo, hi) = z.iter().fold((f32::MAX, f32::MIN), |(a, c), x| (a.min(*x), c.max(*x)));
                        ((hi - lo).abs() < 1e-7).then(|| (r.clone(), hi))
                    })
                    .max_by(|a, b| a.1.total_cmp(&b.1))
                    .expect("a flat face")
                    .0;
                v.selection = vec![top];
            })
        });
        draw(cx, self.window);
    }
}

/// The angle (degrees) between two cameras' rotations.
fn turned(a: &Camera, b: &Camera) -> f64 {
    a.rotation.angle_between(b.rotation).to_degrees()
}

fn near(got: f64, want: f64, tol: f64, what: &str) -> Result<(), String> {
    if (got - want).abs() <= tol { Ok(()) } else { Err(format!("{what}: ours {got:.4}, live {want:.4}")) }
}

type Check = fn(&Nav, &mut TestAppContext) -> Result<(), String>;

struct Row {
    id: &'static str,
    live: &'static str,
    check: Check,
    gap: bool,
}

/// Live's rotation per CSS px: 3 turns across the canvas diagonal (in device px; per CSS px the same).
/// Live's wheel notch on the running OS, in powers of two of the view scale: Onshape zooms 0.5 / 120 × wheelDelta,
/// ×3 where `navigator.platform` is Windows, and scales by 2^(−amount / 6).
fn live_notch() -> f64 {
    if cfg!(target_os = "windows") { 1. / 4. } else { 1. / 12. }
}

fn live_deg_per_px(size: DVec2) -> f64 {
    360. * 3. / size.length()
}

fn rows() -> Vec<Row> {
    vec![
        Row { id: "wheel / a notch in", live: "the view scale × 2^(-1/12) = 0.943874 on Linux and Mac, × 2^(-1/4) = 0.840896 on Windows", gap: false, check: |n, cx| {
            n.reset(cx);
            let (before, c) = (n.half_height(cx), n.centre(cx));
            n.wheel(cx, c, 1., false);
            near(n.half_height(cx) / before, 2f64.powf(-live_notch()), 1e-4, "scale")
        } },
        Row { id: "wheel / a notch out", live: "× 2^(1/12) = 1.059463 on Linux and Mac, × 2^(1/4) = 1.189207 on Windows", gap: false, check: |n, cx| {
            n.reset(cx);
            let (before, c) = (n.half_height(cx), n.centre(cx));
            n.wheel(cx, c, -1., false);
            near(n.half_height(cx) / before, 2f64.powf(live_notch()), 1e-4, "scale")
        } },
        Row { id: "wheel / in and out toward the pointer", live: "the point under the pointer stays (within ~1 px), zooming out too (no central zoom out)", gap: false, check: |n, cx| {
            n.reset(cx);
            let at = n.centre(cx) + DVec2::new(180., -120.);
            let w = n.under(cx, at);
            n.wheel(cx, at, 3., false);
            let a = n.project(cx, w).distance(at);
            n.wheel(cx, at, -5., false);
            let b = n.project(cx, w).distance(at);
            near(a.max(b), 0., 1., "px the point moved")
        } },
        Row { id: "wheel / sideways", live: "ignored when |dx| > |dy|", gap: false, check: |n, cx| {
            n.reset(cx);
            let (before, c) = (n.cam(cx), n.centre(cx));
            n.wheel(cx, c, 1., true);
            let after = n.cam(cx);
            near((after.distance - before.distance).abs() + turned(&before, &after) + after.target.distance(before.target), 0., 1e-9, "change")
        } },
        Row { id: "right drag / orbits 3 turns per canvas diagonal", live: "100 px: 57.049 deg on live's 1673 × 886 canvas (360 × 3 / diagonal per px)", gap: false, check: |n, cx| {
            n.reset(cx);
            let (before, c, size) = (n.cam(cx), n.centre(cx), n.size(cx));
            n.drag(cx, MouseButton::Right, c, DVec2::new(100., 0.), NONE);
            near(turned(&before, &n.cam(cx)), 100. * live_deg_per_px(size), 0.05, "deg")
        } },
        Row { id: "right drag / sideways turns about the camera's up", live: "up unchanged; vertical motion about the camera's right", gap: false, check: |n, cx| {
            n.reset(cx);
            let (before, c) = (n.cam(cx), n.centre(cx));
            n.drag(cx, MouseButton::Right, c, DVec2::new(100., 0.), NONE);
            let mid = n.cam(cx);
            n.drag(cx, MouseButton::Right, c, DVec2::new(0., 100.), NONE);
            let after = n.cam(cx);
            near(before.up().dot(mid.up()), 1., 1e-6, "up kept by a sideways drag")?;
            near(mid.right().dot(after.right()), 1., 1e-6, "right kept by a vertical drag")
        } },
        Row { id: "Shift + right drag", live: "rotates as a plain right drag", gap: false, check: |n, cx| {
            n.reset(cx);
            let (before, c, size) = (n.cam(cx), n.centre(cx), n.size(cx));
            n.drag(cx, MouseButton::Right, c, DVec2::new(100., 0.), Modifiers { shift: true, ..NONE });
            near(turned(&before, &n.cam(cx)), 100. * live_deg_per_px(size), 0.05, "deg")
        } },
        Row { id: "middle drag / pans 1:1", live: "a 100, 50 px drag moves the model 100, 50 px", gap: false, check: |n, cx| {
            n.reset(cx);
            let c = n.centre(cx);
            let w = n.under(cx, c);
            n.drag(cx, MouseButton::Middle, c, DVec2::new(100., 50.), NONE);
            let moved = n.project(cx, w) - c;
            near(moved.distance(DVec2::new(100., 50.)), 0., 0.5, "px off")
        } },
        Row { id: "Ctrl + right drag / pans", live: "100 px drag, 100 px pan, no rotation", gap: false, check: |n, cx| {
            n.reset(cx);
            let (before, c) = (n.cam(cx), n.centre(cx));
            let w = n.under(cx, c);
            n.drag(cx, MouseButton::Right, c, DVec2::new(100., 0.), Modifiers { control: true, ..NONE });
            near(turned(&before, &n.cam(cx)), 0., 1e-6, "deg turned")?;
            near((n.project(cx, w) - c).distance(DVec2::new(100., 0.)), 0., 0.5, "px off")
        } },
        Row { id: "right drag / about the point under the pointer", live: "starting on the part, that point stays within 0.4 px of where the press was", gap: false, check: |n, cx| {
            n.reset(cx);
            let at = n.part_centre(cx);
            let w = n.on_part(cx, at).ok_or(format!("no part under the press at {at:?}"))?;
            n.drag(cx, MouseButton::Right, at, DVec2::new(80., 40.), NONE);
            near(n.project(cx, w).distance(at), 0., 0.4, "px the point moved")
        } },
        Row { id: "right drag / from empty space, about the view centre", live: "starting on empty space it rotates about the view centre (the old centre moved 2 px)", gap: false, check: |n, cx| {
            n.reset(cx);
            let (size, centre) = (n.size(cx), n.centre(cx));
            let from = DVec2::new(40., size.y - 40.);
            if n.on_part(cx, from).is_some() {
                return Err("the press is on the part".into());
            }
            let w = n.under(cx, centre);
            n.drag(cx, MouseButton::Right, from, DVec2::new(80., -40.), NONE);
            near(n.project(cx, w).distance(centre), 0., 2., "px the view centre's point moved")
        } },
        Row { id: "Alt + right drag / turntable", live: "an upright view turns about the vertical axis: the view direction's z stays (-0.5774 from isometric)", gap: false, check: |n, cx| {
            n.reset(cx);
            let (before, c) = (n.cam(cx), n.centre(cx));
            n.drag(cx, MouseButton::Right, c, DVec2::new(100., 0.), Modifiers { alt: true, ..NONE });
            let after = n.cam(cx);
            near(after.forward().z, before.forward().z, 1e-6, "view direction z")?;
            if turned(&before, &after) < 10. {
                return Err(format!("turned {:.2} deg", turned(&before, &after)));
            }
            Ok(())
        } },
        Row { id: "right drag / Ctrl pressed mid-drag pans, released rotates again", live: "the mode is re-evaluated on every move (50 px with Ctrl: 50.2 px pan)", gap: false, check: |n, cx| {
            n.reset(cx);
            let c = n.centre(cx);
            let ctrl = Modifiers { control: true, ..NONE };
            let mv = |n: &Nav, cx: &mut TestAppContext, to: DVec2, m: Modifiers| {
                let p = n.at(cx, to);
                n.event(cx, MouseMoveEvent { position: p, pressed_button: Some(MouseButton::Right), modifiers: m });
            };
            let p0 = n.at(cx, c);
            n.event(cx, MouseMoveEvent { position: p0, pressed_button: None, modifiers: NONE });
            n.event(cx, MouseDownEvent { button: MouseButton::Right, position: p0, modifiers: NONE, click_count: 1, first_mouse: false });
            for k in 1..=10 {
                mv(n, cx, c + DVec2::new(5. * k as f64, 0.), NONE);
            }
            n.settle(cx);
            let rotated = n.cam(cx);
            let w = n.under(cx, c + DVec2::new(50., 0.));
            for k in 1..=10 {
                mv(n, cx, c + DVec2::new(50., 5. * k as f64), ctrl);
            }
            let panned = n.cam(cx);
            let moved = n.project(cx, w) - (c + DVec2::new(50., 0.));
            for k in 1..=10 {
                mv(n, cx, c + DVec2::new(50. + 5. * k as f64, 50.), NONE);
            }
            std::thread::sleep(std::time::Duration::from_millis(120));
            let p1 = n.at(cx, c + DVec2::new(100., 50.));
            n.event(cx, MouseUpEvent { button: MouseButton::Right, position: p1, modifiers: NONE, click_count: 1 });
            n.settle(cx);
            if turned(&n.cam(cx), &rotated) < 10. {
                return Err("Ctrl released: no rotation".into());
            }
            near(turned(&rotated, &panned), 0., 1e-6, "deg turned with Ctrl")?;
            near(moved.distance(DVec2::new(0., 50.)), 0., 0.5, "px off the 50 px pan")
        } },
        Row { id: "keys / Ctrl+Shift+arrows pan 150 px in 250 ms", live: "DEFAULT_PAN_AMOUNT_PX 150, animated 250 ms", gap: false, check: |n, cx| {
            for (keys, want) in [("ctrl-shift-left", DVec2::new(-150., 0.)), ("ctrl-shift-up", DVec2::new(0., -150.))] {
                n.reset(cx);
                let c = n.centre(cx);
                let w = n.under(cx, c);
                n.keys(cx, keys);
                let (_, d) = cx.update(|cx| n.vp.read(cx).animation()).ok_or(format!("{keys}: no animation"))?;
                near(d.as_millis() as f64, 250., 0.5, &format!("{keys} ms"))?;
                n.settle(cx);
                near((n.project(cx, w) - c).distance(want), 0., 0.5, &format!("{keys} px off"))?;
            }
            Ok(())
        } },
        Row { id: "keys / N: normal to the face in 500 ms, again flips", live: "N views the selected face head on (500 ms); pressed when already normal it turns 180 deg", gap: false, check: |n, cx| {
            n.reset(cx);
            n.select_top(cx);
            n.keys(cx, "n");
            let (_, d) = cx.update(|cx| n.vp.read(cx).animation()).ok_or("no animation")?;
            near(d.as_millis() as f64, 500., 0.5, "ms")?;
            n.settle(cx);
            near(n.cam(cx).forward().z, -1., 1e-6, "looking down at the top face: forward z")?;
            n.keys(cx, "n");
            n.settle(cx);
            near(n.cam(cx).forward().z, 1., 1e-6, "N again: forward z")
        } },
        Row { id: "keys / arrows: 15°, Shift 90°, Ctrl 5°, in 250 ms", live: "ROTATE_ANGLE 15, COARSE 90, PRECISION 5; 250 ms; every key repeat turns (behaviours: held arrow)", gap: false, check: |n, cx| {
            for (keys, deg) in [("left", 15.), ("shift-left", 90.), ("ctrl-left", 5.)] {
                n.reset(cx);
                let before = n.cam(cx);
                n.keys(cx, keys);
                let steps = cx.update(|cx| n.vp.read(cx).step_angles());
                let [step] = steps[..] else { return Err(format!("{keys}: {} steps turning", steps.len())) };
                near(step.abs().to_degrees(), deg, 0.01, &format!("{keys} step"))?;
                n.settle(cx);
                near(turned(&before, &n.cam(cx)), deg, 0.01, keys)?;
            }
            near(crate::navigation::CAMERA_SHORT_ANIMATION.as_millis() as f64, 250., 0.5, "a step's ms")
        } },
        Row { id: "keys / Shift+1, 5, 7 and F: 500 ms", live: "named views and zoom to fit: CAMERA_LONG_ANIMATION_TIME_MS 500", gap: false, check: |n, cx| {
            for keys in ["shift-1", "shift-5", "shift-7", "f"] {
                n.reset(cx);
                let off = n.centre(cx) + DVec2::new(60., 0.);
            n.wheel(cx, off, 4., false);
                n.keys(cx, keys);
                let (_, d) = cx.update(|cx| n.vp.read(cx).animation()).ok_or(format!("{keys}: no animation"))?;
                near(d.as_millis() as f64, 500., 0.5, &format!("{keys} ms"))?;
            }
            Ok(())
        } },
        Row { id: "keys / Shift+1..7: the named views", live: "direction / up: Front (0,1,0)/+Z, Back (0,-1,0)/+Z, Left (1,0,0)/+Z, Right (-1,0,0)/+Z, Top (0,0,-1)/+Y, Bottom (0,0,1)/-Y, Shift+7 isometric", gap: false, check: |n, cx| {
            let want = [
                ("shift-1", DVec3::Y, DVec3::Z),
                ("shift-2", -DVec3::Y, DVec3::Z),
                ("shift-3", DVec3::X, DVec3::Z),
                ("shift-4", -DVec3::X, DVec3::Z),
                ("shift-5", -DVec3::Z, DVec3::Y),
                ("shift-6", DVec3::Z, -DVec3::Y),
            ];
            for (keys, dir, up) in want {
                n.reset(cx);
                n.keys(cx, keys);
                n.settle(cx);
                let c = n.cam(cx);
                if c.forward().distance(dir) > 1e-6 || c.up().distance(up) > 1e-6 {
                    return Err(format!("{keys}: ours direction {:?} up {:?}, live {dir:?} / {up:?}", c.forward(), c.up()));
                }
            }
            Ok(())
        } },
        Row { id: "animation / smootherstep", live: "p = t³(6t² − 15t + 10)", gap: false, check: |_, _| {
            for t in [0.0, 0.25, 0.5, 0.8, 1.0] {
                near(crate::navigation::ease(t), t * t * t * (t * (6. * t - 15.) + 10.), 1e-12, &format!("t {t}"))?;
            }
            Ok(())
        } },
        Row { id: "animation / a new one while one runs", live: "ignored until the running one finishes", gap: false, check: |n, cx| {
            n.reset(cx);
            n.keys(cx, "shift-1");
            let first = cx.update(|cx| n.vp.read(cx).animation()).ok_or("no animation")?;
            n.keys(cx, "shift-5");
            let now = cx.update(|cx| n.vp.read(cx).animation()).ok_or("no animation")?;
            near(turned(&first.0, &now.0), 0., 1e-6, "deg between the targets")
        } },
        Row { id: "double middle click / zoom to fit", live: "two middle releases within 500 ms fit (150 ms apart did, 600 ms didn't)", gap: false, check: |n, cx| {
            let middle = |n: &Nav, cx: &mut TestAppContext| {
                let c = n.centre(cx) + DVec2::new(40., 30.);
                let p = n.at(cx, c);
                n.event(cx, MouseDownEvent { button: MouseButton::Middle, position: p, modifiers: NONE, click_count: 1, first_mouse: false });
                n.event(cx, MouseUpEvent { button: MouseButton::Middle, position: p, modifiers: NONE, click_count: 1 });
            };
            n.reset(cx);
            let off = n.centre(cx) + DVec2::new(60., 0.);
            n.wheel(cx, off, 4., false);
            middle(n, cx);
            n.tick(150);
            middle(n, cx);
            let fit = cx.update(|cx| n.vp.read(cx).animation()).map(|a| a.1.as_millis());
            n.reset(cx);
            let off = n.centre(cx) + DVec2::new(60., 0.);
            n.wheel(cx, off, 4., false);
            middle(n, cx);
            n.tick(600);
            middle(n, cx);
            let late = cx.update(|cx| n.vp.read(cx).animation()).is_some();
            if fit != Some(500) || late {
                return Err(format!("150 ms apart: animation {fit:?} ms (live 500); 600 ms apart: animating {late} (live no)"));
            }
            Ok(())
        } },
    ]
}

/// Every row at a pixel ratio of 1 and 2. Prints matched / total (`NAV_ROW=<part of an id>` runs only those rows).
#[gpui::test]
fn the_navigation_matrix_matches_live(cx: &mut TestAppContext) {
    let rows = rows();
    let only = std::env::var("NAV_ROW").ok();
    let navs: Vec<Nav> = [1.0f32, 2.0].iter().map(|s| Nav::open(cx, &format!("nav-{s}"), *s)).collect();
    let mut report = Vec::new();
    let mut moved = Vec::new();
    for r in &rows {
        if only.as_deref().is_some_and(|o| !r.id.contains(o)) {
            continue;
        }
        let mut why = Vec::new();
        for (n, scale) in navs.iter().zip([1, 2]) {
            if let Err(e) = (r.check)(n, cx) {
                why.push(format!("scale {scale}: {e}"));
            }
        }
        let ok = why.is_empty();
        report.push(format!("{} {} (live: {}){}", if ok { "=" } else { "≠" }, r.id, r.live, if ok { String::new() } else { format!("\n    {}", why.join("\n    ")) }));
        if ok == r.gap {
            moved.push(r.id);
        }
    }
    let matched = report.iter().filter(|l| l.starts_with('=')).count();
    eprintln!("navigation matrix: {matched} / {} rows match live\n{}", report.len(), report.join("\n"));
    assert!(moved.is_empty(), "rows whose status moved (update interaction-feel.md and the row's gap): {moved:?}");
}
