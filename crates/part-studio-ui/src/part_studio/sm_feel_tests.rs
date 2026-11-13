//! The sheet metal matrix (docs/parity/interaction-feel.md "Sheet metal"): live2's feel/sheetmetal.json rows acted out
//! with real clicks and keys on the sm-box start (a 60 × 40 plate with two flanges), the Sheet metal table and flat
//! view panel open. A row ours doesn't match yet is a gap; the test fails when a row's status moves either way.

use super::*;
use gpui_kit::{MouseButton, MouseDownEvent, MouseUpEvent, Pixels, ScrollDelta, ScrollWheelEvent, point};

const NONE: Modifiers = Modifiers { shift: false, control: false, alt: false, platform: false, function: false };

struct Sm {
    window: AnyWindowHandle,
    view: Entity<PartStudioView>,
}

impl Sm {
    fn open(cx: &mut TestAppContext) -> Self {
        let starts: serde_json::Value = serde_json::from_str(include_str!("../../../../tests/ui-parity/starts.json")).unwrap();
        let (window, view) = part_studio_from(cx, "sm-matrix", Some(starts["sm-box"].clone()));
        cx.update_window(window, |_, w, _| w.activate_window()).unwrap();
        wait_until(cx, window, |cx| !view.read(cx).ui_busy() && !view.read(cx).display.bodies.is_empty());
        cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.toggle_panel(crate::part_studio::config_panel::RightPanel::SheetMetal, w, cx))).unwrap();
        let sm = Sm { window, view };
        wait_until(cx, window, |cx| !sm.view.read(cx).ui_busy());
        sm.frames(cx);
        sm
    }

    fn frames(&self, cx: &mut TestAppContext) {
        for _ in 0..5 {
            cx.executor().advance_clock(std::time::Duration::from_millis(20));
            draw(cx, self.window);
        }
    }

    fn bounds(&self, cx: &mut TestAppContext, sel: &str) -> Option<gpui_kit::Bounds<Pixels>> {
        draw(cx, self.window);
        gpui_kit::VisualTestContext::from_window(self.window, cx).debug_bounds(Box::leak(sel.to_string().into_boxed_str()))
    }

    fn press(&self, cx: &mut TestAppContext, sel: &str, button: MouseButton) -> Result<(), String> {
        let at = self.bounds(cx, sel).ok_or(format!("no {sel}"))?.center();
        let mut vcx = gpui_kit::VisualTestContext::from_window(self.window, cx);
        vcx.simulate_event(gpui_kit::MouseMoveEvent { position: at, pressed_button: None, modifiers: NONE });
        vcx.simulate_event(MouseDownEvent { button, position: at, modifiers: NONE, click_count: 1, first_mouse: false });
        vcx.simulate_event(MouseUpEvent { button, position: at, modifiers: NONE, click_count: 1 });
        self.frames(cx);
        Ok(())
    }

    fn keys(&self, cx: &mut TestAppContext, keys: &str) {
        gpui_kit::VisualTestContext::from_window(self.window, cx).simulate_keystrokes(keys);
        self.frames(cx);
    }

    fn escape_all(&self, cx: &mut TestAppContext) {
        cx.update_window(self.window, |_, _, cx| self.view.update(cx, |v, cx| {
            v.ui.sm_edit = None;
            v.ui.sm_cell = None;
            v.selection.clear();
            cx.notify();
        }))
        .unwrap();
        cx.update(|cx| {
            if let Some(m) = crate::os_menu::open_menu(cx) {
                m.update(cx, |_, cx| cx.emit(gpui_kit::DismissEvent));
            }
        });
        cx.update_window(self.window, |_, _, cx| self.view.update(cx, |v, cx| v.close_context_menu(cx))).unwrap();
        self.frames(cx);
    }
}

fn expect<T: PartialEq + std::fmt::Debug>(got: T, want: T, what: &str) -> Result<(), String> {
    if got == want { Ok(()) } else { Err(format!("{what}: ours {got:?}, live {want:?}")) }
}

type Check = fn(&Sm, &mut TestAppContext) -> Result<(), String>;

struct Row {
    id: &'static str,
    live: &'static str,
    check: Check,
    gap: bool,
}

fn rows() -> Vec<Row> {
    vec![
        Row { id: "table / the Bends columns", live: "#, Name, Radius(mm), Angle(deg), Bend direction, K Factor", gap: false, check: |s, cx| {
            for c in ["#", "Name", "Radius(mm)", "Angle(deg)", "Bend direction", "K Factor"] {
                s.bounds(cx, &format!("sm-cell-{c}")).ok_or(format!("no {c} column"))?;
            }
            Ok(())
        } },
        Row { id: "table / a row click selects the row only", live: "the row (os-selected-cell); nothing selected in 3D", gap: false, check: |s, cx| {
            s.press(cx, "sm-cell-0-0-0", MouseButton::Left)?;
            let (cell, sel) = cx.update(|cx| (s.view.read(cx).ui.sm_cell, s.view.read(cx).selection.len()));
            expect((cell, sel), (Some((0, 0, 0)), 0), "selected cell, 3D selection")
        } },
        Row { id: "table / Enter edits a selected Radius cell", live: "a click selects the cell, Enter edits it ('2 mm')", gap: false, check: |s, cx| {
            s.press(cx, "sm-cell-0-0-2", MouseButton::Left)?;
            s.keys(cx, "enter");
            expect(cx.update(|cx| s.view.read(cx).ui.sm_edit), Some((0, 2)), "editing")
        } },
        Row { id: "table / a Radius edit adds Modify joint 1 at the end", live: "typing 4 + Enter: 'Modify joint 1' added at the end of the list", gap: false, check: |s, cx| {
            s.press(cx, "sm-cell-0-0-2", MouseButton::Left)?;
            s.keys(cx, "enter");
            s.keys(cx, "4");
            s.keys(cx, "enter");
            wait_until(cx, s.window, |cx| !s.view.read(cx).ui_busy());
            let last = cx.update(|cx| s.view.read(cx).studio(cx).and_then(|ps| ps.features.last().map(|f| f.name.clone())));
            expect(last.as_deref(), Some("Modify joint 1"), "the last feature")
        } },
        Row { id: "table / a bend row's menu", live: "the first row: 'Move down', 'Convert Bend A to rip' (no Move up)", gap: false, check: |s, cx| {
            s.press(cx, "sm-cell-0-0-0", MouseButton::Right)?;
            let labels: Vec<String> = cx
                .update(|cx| s.view.read(cx).context_menu.as_ref().map(|(_, m, _)| m.read(cx).labels()).unwrap_or_else(|| crate::os_menu::open_labels(cx)))
                .into_iter()
                .filter(|l| l != "———")
                .collect();
            if labels.len() != 2 || labels[0] != "Move down" || !(labels[1].starts_with("Convert Bend ") && labels[1].ends_with(" to rip")) {
                return Err(format!("{labels:?}"));
            }
            Ok(())
        } },
        Row { id: "table / Convert to rip", live: "the row moves to Other joints as 'Joint A' (Rip, Edge joint) and the flange becomes its own part (Parts (2))", gap: false, check: |s, cx| {
            s.press(cx, "sm-cell-0-0-0", MouseButton::Right)?;
            let label = cx
                .update(|cx| s.view.read(cx).context_menu.as_ref().map(|(_, m, _)| m.read(cx).labels()).unwrap_or_else(|| crate::os_menu::open_labels(cx)))
                .into_iter()
                .find(|l| l.ends_with(" to rip"))
                .ok_or("no Convert row")?;
            s.press(cx, &format!("menu:{label}"), MouseButton::Left)?;
            wait_until(cx, s.window, |cx| !s.view.read(cx).ui_busy());
            s.frames(cx);
            s.bounds(cx, "sm-cell-1-0-0").ok_or("no row in Other joints")?;
            expect(cx.update(|cx| s.view.read(cx).display.bodies.len()), 2, "parts")
        } },
        Row { id: "table / a row hovered lights its bend", live: "hovering a bend row lights that bend rgb(255,198,133) in the 3D and flat views", gap: false, check: |s, cx| {
            let at = s.bounds(cx, "sm-cell-0-0-1").ok_or("no row")?.center();
            gpui_kit::VisualTestContext::from_window(s.window, cx).simulate_event(gpui_kit::MouseMoveEvent { position: at, pressed_button: None, modifiers: NONE });
            s.frames(cx);
            let lit = cx.update(|cx| s.view.read(cx).sm_hover_refs().len());
            if lit == 0 {
                return Err("nothing lit".into());
            }
            Ok(())
        } },
        Row { id: "flange / no manipulator", live: "the Flange dialog shows a preview wall and no length arrow", gap: false, check: |s, cx| {
            cx.update_window(s.window, |_, w, cx| s.view.update(cx, |v, cx| v.start_feature("sheetMetalFlange", w, cx))).unwrap();
            s.frames(cx);
            let manip = cx.update(|cx| s.view.read(cx).current_manip(cx).is_some());
            cx.update_window(s.window, |_, w, cx| s.view.update(cx, |v, cx| v.cancel_edit(w, cx))).unwrap();
            s.frames(cx);
            expect(manip, false, "a manipulator")
        } },
        Row { id: "flat view / starts from Top", live: "it starts looking straight down (Top), the view cube 'Top'", gap: false, check: |s, cx| {
            let flat = cx.update(|cx| s.view.read(cx).flat_view.as_ref().map(|f| f.read(cx).viewport.clone())).ok_or("no flat view")?;
            let rot = cx.update(|cx| flat.read(cx).camera.rotation);
            let top = noshape_render::Camera::rotation_for(noshape_render::StandardView::Top);
            expect(rot.angle_between(top).to_degrees() < 0.01, true, "the flat camera on Top")
        } },
        Row { id: "flat view / its own camera", live: "the wheel over the flat view zooms it alone; the main view doesn't move", gap: false, check: |s, cx| {
            let flat = cx.update(|cx| s.view.read(cx).flat_view.as_ref().map(|f| f.read(cx).viewport.clone())).ok_or("no flat view")?;
            let main = cx.update(|cx| s.view.read(cx).viewport.clone());
            let (f0, m0) = cx.update(|cx| (flat.read(cx).camera, main.read(cx).camera));
            let (o, size) = cx.update(|cx| (flat.read(cx).origin(), flat.read(cx).size()));
            let at = point(o.x + px(size.x as f32 * 0.5), o.y + px(size.y as f32 * 0.5));
            let mut vcx = gpui_kit::VisualTestContext::from_window(s.window, cx);
            vcx.simulate_event(gpui_kit::MouseMoveEvent { position: at, pressed_button: None, modifiers: NONE });
            vcx.simulate_event(ScrollWheelEvent { position: at, delta: ScrollDelta::Lines(point(0., 3.)), modifiers: NONE, wheel_ticks: Some(point(0., 1.)), ..Default::default() });
            s.frames(cx);
            let (f1, m1) = cx.update(|cx| (flat.read(cx).camera, main.read(cx).camera));
            expect((f1 != f0, m1 == m0), (true, true), "flat moved, main still")
        } },
    ]
}

#[gpui::test]
fn the_sheet_metal_matrix_matches_live(cx: &mut TestAppContext) {
    let rows = rows();
    let mut report = Vec::new();
    let mut moved = Vec::new();
    for r in &rows {
        // A fresh document each row (an edit changes the model).
        let s = Sm::open(cx);
        let res = (r.check)(&s, cx);
        s.escape_all(cx);
        let ok = res.is_ok();
        report.push(format!("{} {} (live: {}){}", if ok { "=" } else { "≠" }, r.id, r.live, res.err().map(|e| format!("\n    {e}")).unwrap_or_default()));
        if ok == r.gap {
            moved.push(r.id);
        }
    }
    let matched = report.iter().filter(|l| l.starts_with('=')).count();
    eprintln!("sheet metal matrix: {matched} / {} rows match live\n{}", report.len(), report.join("\n"));
    assert!(moved.is_empty(), "rows whose status moved (update interaction-feel.md and the row's gap): {moved:?}");
}
