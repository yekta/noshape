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
            let flat = cx.update(|cx| s.view.read(cx).flat_view.as_ref().map(|f| f.read(cx).hovered_bend()));
            expect((lit > 0, flat), (true, Some(Some(0))), "(the bend lit in 3D, its zone on the flat)")
        } },
        Row { id: "table / a joint row hovered lights its rip", live: "hovering Joint A (a rip) lights it in the 3D view and its edges in the flat view (live 11-17 sm-feel-1117)", gap: false, check: |s, cx| {
            s.press(cx, "sm-cell-0-0-0", MouseButton::Right)?;
            let label = cx
                .update(|cx| s.view.read(cx).context_menu.as_ref().map(|(_, m, _)| m.read(cx).labels()).unwrap_or_else(|| crate::os_menu::open_labels(cx)))
                .into_iter()
                .find(|l| l.ends_with(" to rip"))
                .ok_or("no Convert row")?;
            s.press(cx, &format!("menu:{label}"), MouseButton::Left)?;
            wait_until(cx, s.window, |cx| !s.view.read(cx).ui_busy());
            s.frames(cx);
            let at = s.bounds(cx, "sm-cell-1-0-0").ok_or("no row in Other joints")?.center();
            gpui_kit::VisualTestContext::from_window(s.window, cx).simulate_event(gpui_kit::MouseMoveEvent { position: at, pressed_button: None, modifiers: NONE });
            s.frames(cx);
            let lit = cx.update(|cx| s.view.read(cx).sm_hover_refs());
            let flat = cx.update(|cx| s.view.read(cx).flat_view.as_ref().map(|f| f.read(cx).joint_edges().len())).ok_or("no flat view")?;
            expect((!lit.is_empty() && lit.iter().all(|r| matches!(r, Reference::Face(_))), flat > 0), (true, true), "(the rip's faces lit in 3D, its edges on the flat)")
        } },
        Row { id: "table / Enter in a Radius cell moves the box down", live: "double-click Bend A's Radius, select all, 4, Enter: the box (2 px #1651b0) moves to Bend B's Radius, not editing, and no bend stays lit (live workflows-1123 02 s19-23)", gap: false, check: |s, cx| {
            let at = s.bounds(cx, "sm-cell-0-0-2").ok_or("no Radius cell")?.center();
            let mut vcx = gpui_kit::VisualTestContext::from_window(s.window, cx);
            vcx.simulate_event(gpui_kit::MouseMoveEvent { position: at, pressed_button: None, modifiers: NONE });
            for n in [1, 2] {
                vcx.simulate_event(MouseDownEvent { button: MouseButton::Left, position: at, modifiers: NONE, click_count: n, first_mouse: false });
                vcx.simulate_event(MouseUpEvent { button: MouseButton::Left, position: at, modifiers: NONE, click_count: n });
            }
            s.frames(cx);
            if cx.update(|cx| s.view.read(cx).ui.sm_edit) != Some((0, 2)) {
                return Err("no editor after the double-click".into());
            }
            s.keys(cx, "secondary-a 4 enter");
            wait_until(cx, s.window, |cx| !s.view.read(cx).ui_busy());
            s.frames(cx);
            let got = cx.update(|cx| {
                let v = s.view.read(cx);
                (v.ui.sm_edit, v.ui.sm_cell.map(|c| (c.0, c.2)), v.ui.sm_cell.is_some_and(|c| c.1 != 0), v.sm_hover_refs().is_empty())
            });
            expect(got, (None, Some((0, 2)), true, true), "(editing, the box's table and column, on another row, nothing lit)")
        } },
        Row { id: "table / a joint's Style list open lights its rip", live: "clicking Joint C's Style cell lights the rip in the 3D and flat views while its list is open (live workflows-1123 03 s23)", gap: false, check: |s, cx| {
            s.press(cx, "sm-cell-0-0-0", MouseButton::Right)?;
            let label = cx
                .update(|cx| s.view.read(cx).context_menu.as_ref().map(|(_, m, _)| m.read(cx).labels()).unwrap_or_else(|| crate::os_menu::open_labels(cx)))
                .into_iter()
                .find(|l| l.ends_with(" to rip"))
                .ok_or("no Convert row")?;
            s.press(cx, &format!("menu:{label}"), MouseButton::Left)?;
            wait_until(cx, s.window, |cx| !s.view.read(cx).ui_busy());
            s.frames(cx);
            s.press(cx, "sm-joint-style-0", MouseButton::Left)?;
            s.frames(cx);
            let away = s.bounds(cx, "menu:Butt joint - Direction 1").ok_or("no Style list")?.center();
            gpui_kit::VisualTestContext::from_window(s.window, cx).simulate_event(gpui_kit::MouseMoveEvent { position: away, pressed_button: None, modifiers: NONE });
            s.frames(cx);
            let lit = cx.update(|cx| (s.view.read(cx).ui.sm_hover, s.view.read(cx).sm_hover_refs()));
            let flat = cx.update(|cx| s.view.read(cx).flat_view.as_ref().map(|f| f.read(cx).joint_edges().len())).ok_or("no flat view")?;
            let open = cx.update(|cx| (s.view.read(cx).ui.sm_joint_open, crate::os_ui::dropdown_open("sm-joint-style-0", cx)));
            if !(!lit.1.is_empty() && lit.1.iter().all(|r| matches!(r, Reference::Face(_))) && flat > 0) {
                return Err(format!("on the list: hover {:?}, lit {:?}, flat edges {flat}, open {open:?}", lit.0, lit.1.len()));
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
        Row { id: "flange / the flip button ends the Bend angle row", live: "[Bend angle ▾] with the opposite-direction arrow button at the right of that row; nothing in the graphics (live 11-17 sm-feel-1117)", gap: false, check: |s, cx| {
            cx.update_window(s.window, |_, w, cx| s.view.update(cx, |v, cx| v.start_feature("sheetMetalFlange", w, cx))).unwrap();
            s.frames(cx);
            let (row, flip, own) = (s.bounds(cx, "row-angleControlType"), s.bounds(cx, "flip-oppositeDirection"), s.bounds(cx, "row-oppositeDirection"));
            cx.update_window(s.window, |_, w, cx| s.view.update(cx, |v, cx| v.cancel_edit(w, cx))).unwrap();
            s.frames(cx);
            let (row, flip) = (row.ok_or("no Angle control type row")?, flip.ok_or("no flip button")?);
            let inside = flip.top() >= row.top() && flip.bottom() <= row.bottom() && f32::from(row.right() - flip.right()).abs() <= 12.0;
            expect((inside, own.is_some()), (true, false), "(the button at the right of the row, a row of its own)")
        } },
        Row { id: "flange / its edges box lights edges and side faces, never a wall's faces", live: "Edges or side faces to flange: rim and bend edges and a rip's thin side face light; the walls' broad faces never (live 11-17 sm-feel-1117)", gap: false, check: |s, cx| {
            cx.update_window(s.window, |_, w, cx| s.view.update(cx, |v, cx| v.start_feature("sheetMetalFlange", w, cx))).unwrap();
            s.frames(cx);
            let got = cx.update(|cx| {
                let v = s.view.read(cx);
                let (ctx, built) = (v.context.as_ref()?, v.built.as_ref()?);
                let prio = v.viewport.read(cx).pick_priority.clone();
                let lit = |r: Reference| built.picks.ids_for(&r).first().is_some_and(|id| prio(*id) != i32::MIN);
                let (b, sm) = ctx.bodies.iter().find_map(|b| Some((b, ctx.sheet_metal.get(&b.id)?)))?;
                let (walls, sides): (Vec<_>, Vec<_>) = b.faces.iter().map(|n| (sm.wall_of_face(n).is_some(), lit(Reference::Face((**n).clone())))).partition(|x| x.0);
                Some((walls.iter().any(|x| x.1), sides.iter().any(|x| x.1), b.edges.iter().any(|n| lit(Reference::Edge((**n).clone())))))
            });
            cx.update_window(s.window, |_, w, cx| s.view.update(cx, |v, cx| v.cancel_edit(w, cx))).unwrap();
            s.frames(cx);
            expect(got, Some((false, true, true)), "(a wall face, a side face, an edge) lights")
        } },
        Row { id: "bend relief / its box lights corners, edges and side faces, never a wall's faces", live: "Bend relief › Bend relief (and Corner › Corner): a rip's side face filled orange, rim / bend edges, a bend end's corner as an orange disc; the broad faces never (live 11-17 sm-feel-1117)", gap: false, check: |s, cx| {
            cx.update_window(s.window, |_, w, cx| s.view.update(cx, |v, cx| v.start_feature("sheetMetalBendRelief", w, cx))).unwrap();
            s.frames(cx);
            let got = cx.update(|cx| {
                let v = s.view.read(cx);
                let (ctx, built) = (v.context.as_ref()?, v.built.as_ref()?);
                let prio = v.viewport.read(cx).pick_priority.clone();
                let lit = |r: Reference| built.picks.ids_for(&r).first().is_some_and(|id| prio(*id) != i32::MIN);
                let (b, sm) = ctx.bodies.iter().find_map(|b| Some((b, ctx.sheet_metal.get(&b.id)?)))?;
                let (walls, sides): (Vec<_>, Vec<_>) = b.faces.iter().map(|n| (sm.wall_of_face(n).is_some(), lit(Reference::Face((**n).clone())))).partition(|x| x.0);
                Some((walls.iter().any(|x| x.1), sides.iter().any(|x| x.1), b.edges.iter().any(|n| lit(Reference::Edge((**n).clone()))), b.vertices.iter().any(|n| lit(Reference::Vertex((**n).clone())))))
            });
            cx.update_window(s.window, |_, w, cx| s.view.update(cx, |v, cx| v.cancel_edit(w, cx))).unwrap();
            s.frames(cx);
            expect(got, Some((false, true, true, true)), "(a wall face, a side face, an edge, a vertex) lights")
        } },
        Row { id: "flat view / a click on a bend zone", live: "selects that bend in both views and its row in the Bends table", gap: false, check: |s, cx| {
            s.escape_all(cx);
            let fv = cx.update(|cx| s.view.read(cx).flat_view.clone()).ok_or("no flat view")?;
            // Inside the first bend's zone, off its centre line (a click there selects the line): 40 % of the way to its edge.
            let at = cx.update(|cx| fv.read(cx).bend_zone_point(0, 0.4, cx)).ok_or("no bend zone")?;
            let vp = cx.update(|cx| fv.read(cx).viewport.clone());
            cx.update(|cx| vp.update(cx, |_, cx| cx.emit(ViewportEvent::Click { pick: None, position: at, modifiers: NONE })));
            s.frames(cx);
            let (faces, row) = cx.update(|cx| {
                let v = s.view.read(cx);
                (v.selection.iter().filter(|r| matches!(r, Reference::Face(_))).count(), v.ui.sm_cell.map(|(t, r, _)| (t, r)))
            });
            expect((faces, row), (1, Some((0, 0))), "faces selected in 3D, the Bends row selected")
        } },
        Row { id: "3D view / a click on a bend face", live: "selects the face, outlines its Bends row and lights its zone in the flat view; hovering it changes no row (sm-cfg-io-1119)", gap: false, check: |s, cx| {
            s.escape_all(cx);
            let (face, id, row) = cx.update(|cx| {
                let v = s.view.read(cx);
                let (ctx, built) = (v.context.as_ref()?, v.built.as_ref()?);
                let data = ctx.sheet_metal.values().next()?;
                let face = Reference::Face((*data.bends.get(1)?.outer_faces.first()?.clone()).clone());
                let id = *built.picks.ids_for(&face).first()?;
                Some((face, id, noshape_ops::sheetmetal::joint_of(data, 1)))
            }).ok_or("no bend face to click")?;
            let at = glam::DVec2::new(500., 300.);
            cx.update_window(s.window, |_, w, cx| s.view.update(cx, |v, cx| {
                let vp = v.viewport.clone();
                v.on_viewport_event(&vp, &ViewportEvent::PointerMove { position: at, modifiers: NONE }, w, cx);
            })).unwrap();
            s.frames(cx);
            let hovered = cx.update(|cx| s.view.read(cx).ui.sm_cell);
            cx.update_window(s.window, |_, w, cx| s.view.update(cx, |v, cx| {
                let vp = v.viewport.clone();
                v.on_viewport_event(&vp, &ViewportEvent::Click { pick: Some(id), position: at, modifiers: NONE }, w, cx);
            })).unwrap();
            s.frames(cx);
            let got = cx.update(|cx| {
                let v = s.view.read(cx);
                let line = v.flat_view.as_ref().and_then(|f| f.read(cx).selected_bend());
                let want_line = v.flat_view.as_ref().and_then(|f| f.read(cx).line_of_bend(1));
                (hovered, v.selection == vec![face.clone()], v.ui.sm_cell, line.is_some() && line == want_line)
            });
            expect(got, (None, true, Some((0, row, 0)), true), "(row before the click, the face selected, the row outlined, its zone lit)")
        } },
        Row { id: "flat view / a bend zone hovered", live: "lights that bend in both views, not its table row; on the dashed centre line nothing (sheetmetal-gaps2)", gap: false, check: |s, cx| {
            let fv = cx.update(|cx| s.view.read(cx).flat_view.clone()).ok_or("no flat view")?;
            let vp = cx.update(|cx| fv.read(cx).viewport.clone());
            let mut lit = Vec::new();
            for frac in [0.4, 0.0] {
                let at = cx.update(|cx| fv.read(cx).bend_zone_point(0, frac, cx)).ok_or("no bend zone")?;
                cx.update(|cx| vp.update(cx, |_, cx| cx.emit(ViewportEvent::PointerMove { position: at, modifiers: NONE })));
                s.frames(cx);
                lit.push(cx.update(|cx| (s.view.read(cx).sm_hover_refs().is_empty(), s.view.read(cx).ui.sm_hover)));
            }
            expect(lit, vec![(false, None), (true, None)], "(nothing lit, table row hovered) in the zone, on the centre line")
        } },
        Row { id: "flat view / starts from Top", live: "it starts looking straight down (Top), the view cube 'Top'", gap: false, check: |s, cx| {
            let flat = cx.update(|cx| s.view.read(cx).flat_view.as_ref().map(|f| f.read(cx).viewport.clone())).ok_or("no flat view")?;
            let rot = cx.update(|cx| flat.read(cx).camera.rotation);
            let top = noshape_render::Camera::rotation_for(noshape_render::StandardView::Top);
            expect(rot.angle_between(top).to_degrees() < 0.01, true, "the flat camera on Top")
        } },
        Row { id: "flat view / fitted", live: "the flat fills 95.3 % of the pane's limiting side, centred on the other (skasm flat view fit / orientation)", gap: false, check: |s, cx| {
            let flat = cx.update(|cx| s.view.read(cx).flat_view.as_ref().map(|f| f.read(cx).viewport.clone())).ok_or("no flat view")?;
            let (drawn, size) = cx.update(|cx| (flat.read(cx).fit_box_px(), flat.read(cx).size()));
            let Some((a, b)) = drawn else { return Err("no flat drawn".into()) };
            let (lo, hi, fill) = (a, size - b, (b - a) / size);
            let fits = |f: f64, l: f64, h: f64| (f - 0.953).abs() <= 0.005 && (l - h).abs() <= 1.0;
            let centred = |l: f64, h: f64| (l - h).abs() <= 1.0;
            expect((fits(fill.x, lo.x, hi.x) && centred(lo.y, hi.y) && fill.y <= 0.958) || (fits(fill.y, lo.y, hi.y) && centred(lo.x, hi.x) && fill.x <= 0.958), true, &format!("margins {lo:?} / {hi:?} in {size:?}"))
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

/// Live 11-17 sm-feel-1117: an open box (a block converted, its top excluded, its bottom edges bends: four corner rips)
/// whose Joint A goes Rip → Bend in the table can't unfold: "Modify joint 1" is added with the unfold failure (a warning:
/// std's hole.fs reads it so), its bend kept in the 3D, a yellow toast "Sheet metal model cannot be unfolded.", and the
/// flat view goes blank.
#[gpui::test]
fn a_corner_rip_made_a_bend_cannot_unfold_as_lives(cx: &mut TestAppContext) {
    let start = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [60, 40]] }] },
        { "type": "extrude", "name": "Extrude 1", "params": { "entities": ["Sketch 1 region 0"], "depth": "20 mm" } },
        { "type": "sheetMetalStart", "name": "Sheet metal model 1", "params": { "process": "CONVERT", "partToConvert": ["part 0"], "facesToExclude": ["face through [30, 20, 20]"],
            "bends": ["edge through [30, 0, 0]", "edge through [60, 20, 0]", "edge through [30, 40, 0]", "edge through [0, 20, 0]"] } },
    ] });
    let (window, view) = part_studio_from(cx, "sm-unfold", Some(start));
    cx.update_window(window, |_, w, _| w.activate_window()).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && !view.read(cx).display.bodies.is_empty());
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.toggle_panel(crate::part_studio::config_panel::RightPanel::SheetMetal, w, cx))).unwrap();
    let s = Sm { window, view: view.clone() };
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    s.frames(cx);
    let part = cx.update(|cx| view.read(cx).context.as_ref().and_then(|c| c.sheet_metal.iter().find(|(_, d)| d.rips.len() == 4).map(|(b, _)| b.clone()))).expect("one part with four corner rips");
    let flat = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).context.as_ref().map(|c| noshape_model::sheetmetal::flat_pattern_for(c, &part).is_ok()));
    assert_eq!(flat(cx), Some(true), "the box unfolds");
    s.press(cx, "sm-joint-type-0", MouseButton::Left).unwrap();
    s.press(cx, "menu:Bend", MouseButton::Left).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    s.frames(cx);
    let msg = noshape_model::regen::features::sheetmetal_ops::CANNOT_UNFOLD;
    let got = cx.update(|cx| {
        let v = view.read(cx);
        let joint = v.studio(cx).and_then(|st| st.features.iter().find(|f| f.type_id == "sheetMetalJoint").map(|f| (f.name.clone(), f.id)));
        let state = joint.as_ref().and_then(|(_, id)| v.display.feature_state(*id)).map(|st| (st.status, st.drawn_failed, st.message.clone()));
        let bends = v.context.as_ref().and_then(|c| c.sheet_metal.get(&part)).map(|d| d.bends.len());
        (joint.map(|j| j.0), state, bends, v.message.clone())
    });
    use noshape_model::display::FeatureStatus;
    assert_eq!(got, (Some("Modify joint 1".into()), Some((FeatureStatus::Warning, true, Some(msg.into()))), Some(5), Some((FeatureStatus::Warning, msg.into()))), "(the joint, its state (live: a WARNING listed as failed), the part's bends, the yellow toast)");
    let state = cx.update_window(window, |_, w, cx| view.read(cx).ui_state(w, cx)).unwrap();
    let row = state["features"]["rows"].as_array().and_then(|r| r.iter().find(|r| r["name"] == "Modify joint 1")).cloned();
    assert_eq!(row.map(|r| r["state"].clone()), Some(serde_json::json!("error")), "the row drawn failed (live: red name, the header's failures icon)");
    assert_eq!(flat(cx), Some(false), "no flat");
}

/// Live 11-18 sm-rip-1118 (b): a rip's thin side face clicked in Corner (or Bend relief) goes in as the rip's definition
/// edge, a chip "Edge of Sheet metal model 1", and the feature fails: "Sheet metal corner not found." / "Selection is not
/// a bend relief.".
#[gpui::test]
fn a_rips_side_face_in_corner_or_bend_relief_is_its_edge_and_fails_as_lives(cx: &mut TestAppContext) {
    let start = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [60, 40]] }] },
        { "type": "extrude", "name": "Extrude 1", "params": { "entities": ["Sketch 1 region 0"], "depth": "20 mm" } },
        { "type": "sheetMetalStart", "name": "Sheet metal model 1", "params": { "process": "CONVERT", "partToConvert": ["part 0"], "facesToExclude": ["face through [30, 20, 20]"],
            "bends": ["edge through [30, 0, 0]", "edge through [60, 20, 0]", "edge through [30, 40, 0]", "edge through [0, 20, 0]"] } },
    ] });
    let (window, view) = part_studio_from(cx, "sm-rip-side", Some(start));
    cx.update_window(window, |_, w, _| w.activate_window()).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && !view.read(cx).display.bodies.is_empty());
    // The thin side face of the rip at the box's corner (60, 0, 10) mm.
    let face = cx.update(|cx| {
        let v = view.read(cx);
        let ctx = v.context.as_ref().unwrap();
        let (b, sm) = ctx.bodies.iter().find_map(|b| Some((b, ctx.sheet_metal.get(&b.id)?))).expect("the sheet metal part");
        let corner = glam::DVec3::new(0.06, 0.0, 0.01);
        b.faces
            .iter()
            .enumerate()
            .filter(|(_, n)| sm.wall_of_face(n).is_none())
            .filter_map(|(i, n)| {
                let f = b.shape.sub_shape(noshape_kernel::ShapeType::Face, i).ok()?;
                f.face_plane().ok().flatten()?;
                let bb = f.bounding_box(false).ok()?;
                Some((Reference::Face((**n).clone()), ((bb.min + bb.max) * 0.5 - corner).length()))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(r, _)| r)
            .expect("a rip side face")
    });
    let mut got = Vec::new();
    for ty in ["sheetMetalCorner", "sheetMetalBendRelief"] {
        cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature(ty, w, cx))).unwrap();
        draw(cx, window);
        cx.update(|cx| view.update(cx, |v, cx| v.pick(noshape_scene::builder::PickTarget { reference: face.clone(), kind: noshape_scene::builder::PickKind::Face, anchor: None }, cx)));
        wait_until(cx, window, |cx| !view.read(cx).ui_busy());
        draw(cx, window);
        got.push(cx.update(|cx| {
            let v = view.read(cx);
            let d = v.edit.as_ref().unwrap().dialog.read(cx);
            let picked: Vec<String> = d.feature.params.0.values().filter_map(|p| match p {
                noshape_model::params::ParamValue::Query(q) => Some(q.iter().map(|r| (v.labeler(cx))(r)).collect::<Vec<_>>()),
                _ => None,
            }).flatten().collect();
            (picked, v.display.feature_state(d.feature.id).and_then(|s| s.message.clone()))
        }));
        cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.cancel_edit(w, cx))).unwrap();
        wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    }
    assert_eq!(got, vec![
        (vec!["Edge of Sheet metal model 1".to_string()], Some("Sheet metal corner not found.".to_string())),
        (vec!["Edge of Sheet metal model 1".to_string()], Some("Selection is not a bend relief.".to_string())),
    ]);
}

/// Live2 sm-form-1118: Form's Location(s) takes points (sketch points, mate connectors); a click on a face leaves it
/// empty, the face neither picked nor its part.
#[gpui::test]
fn a_face_click_leaves_forms_locations_empty(cx: &mut TestAppContext) {
    let start = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [40, 30]] }] },
        { "type": "extrude", "name": "Extrude 1", "params": { "entities": ["Sketch 1 region 0"], "depth": "10 mm" } }
    ] });
    let (window, view) = part_studio_from(cx, "sm-form-face", Some(start));
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && !view.read(cx).display.bodies.is_empty());
    let face = cx.update(|cx| view.read(cx).display.bodies.iter().find_map(|b| b.face_refs.first().cloned()).expect("a face"));
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("sheetMetalFormed", w, cx))).unwrap();
    draw(cx, window);
    let dialog = cx.update(|cx| view.read(cx).edit.as_ref().expect("the Form dialog").dialog.clone());
    cx.update(|cx| dialog.update(cx, |d, _| d.user_focus_query("locations")));
    cx.update(|cx| view.update(cx, |v, cx| v.pick(noshape_scene::builder::PickTarget { reference: face.clone(), kind: noshape_scene::builder::PickKind::Face, anchor: None }, cx)));
    draw(cx, window);
    let state = cx.update(|cx| {
        let d = dialog.read(cx);
        (d.active_query.clone(), d.feature.params.query("locations").to_vec())
    });
    assert_eq!(state, (Some("locations".to_string()), Vec::new()), "(the active field, Location(s)) after a face click");
}

/// Qa 11-21 (FEEL-FLANGE-EDGE-BEHIND-WALL, live flange-handle-1120): editing a partial flange, its picked edge is drawn
/// whole in the selection's bright orange through the preview, also where the new wall covers it (ours there was tan,
/// (131,123,85): the orange blended away under the wall). The frame's pixels along the edge, every 5 % of it.
#[gpui::test]
fn a_flanges_picked_edge_is_bright_orange_under_its_wall(cx: &mut TestAppContext) {
    let Some(_gpu) = super::feel::gpu_pointer() else { return };
    RENDERING.set(true);
    const SELECT: [i32; 3] = [0xFF, 0xA9, 0x0A];
    let start = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [60, 40]] }] },
        { "type": "sheetMetalStart", "name": "Sheet metal model 1", "params": { "process": "THICKEN", "regions": ["Sketch 1 region 0"], "thickness": "2 mm", "radius": "2 mm" } },
        { "type": "sheetMetalFlange", "name": "Flange 1", "params": { "edges": ["edge through [20, 0, 2]"] } },
        { "type": "sheetMetalFlange", "name": "Flange 2", "params": { "edges": ["edge through [60, 25, 2]"] } },
    ] });
    let (window, view) = part_studio_from(cx, "sm-flange-edge-px", Some(start));
    cx.update_window(window, |_, w, _| w.activate_window()).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && !view.read(cx).display.bodies.is_empty());
    let flange = cx.update(|cx| view.read(cx).studio(cx).and_then(|st| st.features.iter().find(|f| f.name == "Flange 1").map(|f| f.id))).expect("Flange 1");
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.edit_feature(flange, w, cx))).unwrap();
    wait_until(cx, window, |cx| {
        let v = view.read(cx);
        !v.ui_busy() && v.ui.preview_before.is_some() && v.display.feature_state(flange).is_some_and(|s| s.status == noshape_model::display::FeatureStatus::Ok)
    });
    let check = |cx: &mut TestAppContext| -> Vec<String> {
        let (px, w) = super::sketch_hover::settled_frame(cx, window, &view);
        let scale = w as f64 / cx.update(|cx| view.read(cx).viewport.read(cx).size().x).round();
        let rgb = |x: i32, y: i32| -> Option<[i32; 3]> {
            let o = (y.max(0) as usize * w + x.max(0) as usize) * 4;
            px.get(o..o + 3).map(|b| [b[2] as i32, b[1] as i32, b[0] as i32])
        };
        let mut off = Vec::new();
        for k in 1..20 {
            let t = k as f64 / 20.0;
            let p = glam::DVec3::new(0.060 * t, 0.0, 0.002);
            let Some(s) = cx.update(|cx| view.read(cx).viewport.read(cx).project(p)) else { continue };
            let (x, y) = ((s.x * scale).round() as i32, (s.y * scale).round() as i32);
            let r = (3.0 * scale).ceil() as i32;
            let near = (-r..=r).flat_map(|dy| (-r..=r).map(move |dx| (dx, dy))).filter_map(|(dx, dy)| rgb(x + dx, y + dy)).any(|c| (0..3).all(|i| (c[i] - SELECT[i]).abs() <= 24));
            if !near {
                off.push(format!("x {:.0} mm: {:?}", p.x * 1e3, rgb(x, y)));
            }
        }
        off
    };
    let mut off = vec![("opened", check(cx))];
    // As qa walked it: Partial flange unticked (the wall along the whole edge), then ticked again, in the dialog.
    let dialog = cx.update(|cx| view.read(cx).edit.as_ref().expect("the Flange dialog").dialog.clone());
    for (on, what) in [(true, "Partial flange on"), (false, "Partial flange off again")] {
        cx.update(|cx| dialog.update(cx, |d, cx| d.set_value("isPartialFlange", noshape_model::params::ParamValue::Bool(on), cx)));
        wait_until(cx, window, |cx| {
            let v = view.read(cx);
            !v.ui_busy() && v.ui.preview_before.is_some() && v.display.feature_state(flange).is_some_and(|s| s.status == noshape_model::display::FeatureStatus::Ok)
        });
        off.push((what, check(cx)));
    }
    let off: Vec<String> = off.into_iter().filter(|(_, o)| !o.is_empty()).map(|(what, o)| format!("{what}: {}", o.join(", "))).collect();
    assert!(off.is_empty(), "the picked edge not bright orange at:\n{}", off.join("\n"));
}

/// Live feel S1 (qa FEEL-SM-PARTIAL-FLANGE-MANIP): editing a partial flange, its picked edge is lit whole, as it was
/// before the flange (ours lit only the piece the flange leaves).
#[gpui::test]
fn a_partial_flanges_edge_is_lit_whole_while_edited(cx: &mut TestAppContext) {
    let start = serde_json::json!({ "units": "mm", "features": [
        { "type": "newSketch", "name": "Sketch 1", "plane": "Top", "shapes": [{ "corner_rectangle": [[0, 0], [60, 40]] }] },
        { "type": "sheetMetalStart", "name": "Sheet metal model 1", "params": { "process": "THICKEN", "regions": ["Sketch 1 region 0"], "thickness": "2 mm", "radius": "2 mm" } },
        { "type": "sheetMetalFlange", "name": "Flange 1", "params": { "edges": ["edge through [30, 0, 2]"], "isPartialFlange": true, "firstBoundOffset": "25 mm" } },
    ] });
    let (window, view) = part_studio_from(cx, "sm-partial-edge", Some(start));
    cx.update_window(window, |_, w, _| w.activate_window()).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && !view.read(cx).display.bodies.is_empty());
    let flange = cx.update(|cx| view.read(cx).studio(cx).and_then(|st| st.features.iter().find(|f| f.name == "Flange 1").map(|f| f.id))).expect("Flange 1");
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.edit_feature(flange, w, cx))).unwrap();
    wait_until(cx, window, |cx| {
        let v = view.read(cx);
        !v.ui_busy() && v.ui.preview_before.is_some() && v.display.feature_state(flange).is_some_and(|s| s.status == noshape_model::display::FeatureStatus::Ok)
    });
    let edges = cx.update(|cx| view.read(cx).edit.as_ref().map(|e| e.dialog.read(cx).feature.params.query("edges").to_vec())).unwrap_or_default();
    assert_eq!(edges.len(), 1, "the flange's edge: {edges:?}");
    let changed = cx.update(|cx| view.read(cx).changed_picks(&edges));
    assert_eq!(changed, edges, "the shortened edge drawn as before the flange");
    // Qa 11-21 (FEEL-FLANGE-EDGE-BEHIND-WALL, live flange-handle-1120): drawn whole over the preview's blend, so the
    // stretch under the new wall stays bright orange (it was drawn in the blend, (131,123,85) under the wall).
    draw(cx, window);
    let mix = cx.update(|cx| view.read(cx).viewport.read(cx).scene().preview_mix);
    let overlay: Vec<u8> = view.update(cx, |v, _| v.consumed_picks_overlay(&edges, &[], &edges, false).iter().map(|i| i.compare).collect());
    assert!(mix.is_some(), "the preview blended over the model before it");
    assert!(!overlay.is_empty() && overlay.iter().all(|&c| c == noshape_render::scene::compare::POST), "the whole edge drawn over the blend: {overlay:?}");
}

/// Qa 11-22 (FEEL-BEND-FACE-PICKED-LOOK, live workflows-1122 11 s12): Bend's line picked from Sketch 2 fills Sheet
/// metal face to bend with the plate's face on the sketch plane (std onBendChange) and makes it the active field; live
/// outlines that face whole in the selection orange over the preview (ours lit only its front edge).
#[gpui::test]
fn a_bends_filled_face_is_outlined_whole(cx: &mut TestAppContext) {
    let Some(_gpu) = super::feel::gpu_pointer() else { return };
    RENDERING.set(true);
    const SELECT: [i32; 3] = [0xFF, 0xA9, 0x0A];
    let starts: serde_json::Value = serde_json::from_str(include_str!("../../../../tests/ui-parity/starts.json")).unwrap();
    let (window, view) = part_studio_from(cx, "sm-bend-face-px", Some(starts["plate-bend"].clone()));
    cx.update_window(window, |_, w, _| w.activate_window()).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && !view.read(cx).display.bodies.is_empty());
    let line = cx.update(|cx| {
        let v = view.read(cx);
        let sk = v.display.sketches.iter().find(|s| v.studio(cx).and_then(|st| st.features.iter().find(|f| f.id == s.feature)).is_some_and(|f| f.name == "Sketch 2")).expect("Sketch 2");
        Reference::SketchEntity { sketch: sk.feature, entity: sk.curves.first().expect("the line").entity }
    });
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("sheetMetalBend", w, cx))).unwrap();
    draw(cx, window);
    let dialog = cx.update(|cx| view.read(cx).edit.as_ref().expect("the Bend dialog").dialog.clone());
    cx.update(|cx| dialog.update(cx, |d, _| d.user_focus_query("bendReference")));
    cx.update(|cx| view.update(cx, |v, cx| v.pick(noshape_scene::builder::PickTarget { reference: line, kind: noshape_scene::builder::PickKind::SketchCurve, anchor: None }, cx)));
    wait_until(cx, window, |cx| {
        let v = view.read(cx);
        !v.ui_busy() && v.ui.preview_before.is_some() && v.edit.as_ref().is_some_and(|e| !e.dialog.read(cx).feature.params.query("face").is_empty())
    });
    let state = cx.update(|cx| dialog.read(cx).active_query.clone());
    assert_eq!(state.as_deref(), Some("face"), "Sheet metal face to bend active once filled");
    let (px, w) = super::sketch_hover::settled_frame(cx, window, &view);
    let scale = w as f64 / cx.update(|cx| view.read(cx).viewport.read(cx).size().x).round();
    let rgb = |x: i32, y: i32| -> Option<[i32; 3]> {
        let o = (y.max(0) as usize * w + x.max(0) as usize) * 4;
        px.get(o..o + 3).map(|b| [b[2] as i32, b[1] as i32, b[0] as i32])
    };
    let mut off = Vec::new();
    let sides = [(glam::DVec3::new(0.0, 0.0, 0.0), glam::DVec3::new(0.060, 0.0, 0.0)), (glam::DVec3::new(0.0, 0.0, 0.0), glam::DVec3::new(0.0, 0.040, 0.0)), (glam::DVec3::new(0.0, 0.040, 0.0), glam::DVec3::new(0.060, 0.040, 0.0)), (glam::DVec3::new(0.060, 0.0, 0.0), glam::DVec3::new(0.060, 0.040, 0.0))];
    for (a, b) in sides {
        for k in 1..10 {
            let p = a + (b - a) * (k as f64 / 10.0);
            let Some(s) = cx.update(|cx| view.read(cx).viewport.read(cx).project(p)) else { continue };
            let (x, y) = ((s.x * scale).round() as i32, (s.y * scale).round() as i32);
            let r = (3.0 * scale).ceil() as i32;
            let near = (-r..=r).flat_map(|dy| (-r..=r).map(move |dx| (dx, dy))).filter_map(|(dx, dy)| rgb(x + dx, y + dy)).any(|c| (0..3).all(|i| (c[i] - SELECT[i]).abs() <= 24));
            if !near {
                off.push(format!("({:.0}, {:.0}) mm: {:?}", p.x * 1e3, p.y * 1e3, rgb(x, y)));
            }
        }
    }
    assert!(off.is_empty(), "the face to bend's outline not orange at:\n{}", off.join("\n"));
}

// Bend with its face filled from the line, its field clicked and that face clicked again where the preview still has it (qa
// workflow 11 steps 13-14, (20, 20, 2) on plate-bend with a Flange): live keeps "Face of Sheet metal model 1" in the field and Bend 1 builds (qa
// BEND-FACE-RECLICK-EMPTIES).
#[gpui::test]
fn a_bends_face_clicked_again_stays_in_its_field(cx: &mut TestAppContext) {
    let Some(_gpu) = super::feel::gpu_pointer() else { return };
    RENDERING.set(true);
    let starts: serde_json::Value = serde_json::from_str(include_str!("../../../../tests/ui-parity/starts.json")).unwrap();
    // With wf11's Flange 1 (15 mm on the x 60 edge) before it, the preview names the plate's face anew.
    let mut start = starts["plate-bend"].clone();
    start["features"].as_array_mut().unwrap().push(serde_json::json!({ "type": "sheetMetalFlange", "name": "Flange 1", "params": { "edges": ["edge through [60, 20, 2]"], "distance": "15 mm" } }));
    let (window, view) = part_studio_from(cx, "sm-bend-face-reclick", Some(start));
    cx.update_window(window, |_, w, _| w.activate_window()).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && !view.read(cx).display.bodies.is_empty());
    let line = cx.update(|cx| {
        let v = view.read(cx);
        let sk = v.display.sketches.iter().find(|s| v.studio(cx).and_then(|st| st.features.iter().find(|f| f.id == s.feature)).is_some_and(|f| f.name == "Sketch 2")).expect("Sketch 2");
        Reference::SketchEntity { sketch: sk.feature, entity: sk.curves.first().expect("the line").entity }
    });
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("sheetMetalBend", w, cx))).unwrap();
    let dialog = cx.update(|cx| view.read(cx).edit.as_ref().expect("the Bend dialog").dialog.clone());
    cx.update(|cx| dialog.update(cx, |d, _| d.user_focus_query("bendReference")));
    cx.update(|cx| view.update(cx, |v, cx| v.pick(noshape_scene::builder::PickTarget { reference: line, kind: noshape_scene::builder::PickKind::SketchCurve, anchor: None }, cx)));
    let faces = |cx: &mut TestAppContext| cx.update(|cx| dialog.read(cx).feature.params.query("face").len());
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && view.read(cx).ui.preview_before.is_some());
    assert_eq!(faces(cx), 1, "the face filled from the line");
    cx.update(|cx| dialog.update(cx, |d, _| d.user_focus_query("face")));
    // The click is on the preview's face (the Bend's name), not the face before it: wait for the preview's display.
    let bend = cx.update(|cx| dialog.read(cx).feature.id);
    wait_until(cx, window, |cx| view.read(cx).display.bodies.iter().any(|b| b.face_refs.iter().any(|r| crate::part_studio::behaviors::made_by(r, bend))));
    let pointer = super::feel::Pointer::new(cx, window, &view);
    let p = cx.update(|cx| view.read(cx).viewport.read(cx).project(glam::DVec3::new(0.020, 0.020, 0.002))).expect("on screen").round();
    pointer.move_to(cx, &view, p);
    pointer.frames(cx, &view);
    pointer.click(cx, &view, p);
    assert_eq!(faces(cx), 1, "the face kept after the click");
}
