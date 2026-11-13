//! The dialogs matrix (docs/parity/interaction-feel.md "Dialogs"): live2's feel/dialogs.json rows (which features have
//! a manipulator, an arrow's grab reach, the drag steps, the Properties modal's keyboard) on the block start. A row ours
//! doesn't match yet is a gap; the test fails when a row's status moves either way.

use super::*;
use crate::part_studio::manipulators::{linear_step, snap_degrees};

fn block(cx: &mut TestAppContext, name: &str) -> (AnyWindowHandle, Entity<PartStudioView>) {
    let starts: serde_json::Value = serde_json::from_str(include_str!("../../../../tests/ui-parity/starts.json")).unwrap();
    let (window, view) = part_studio_from(cx, name, Some(starts["block"].clone()));
    cx.update_window(window, |_, w, _| w.activate_window()).unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy() && !view.read(cx).display.bodies.is_empty());
    (window, view)
}

/// The block's top face picked and `feature` opened on it; whether a manipulator shows.
fn manipulator_for(cx: &mut TestAppContext, feature: &str) -> bool {
    let (window, view) = block(cx, &format!("dlg-{feature}"));
    cx.update_window(window, |_, w, cx| {
        view.update(cx, |v, cx| {
            let b = v.display.bodies.first().cloned();
            v.selection = b.map(|b| b.face_refs.into_iter().take(1).collect()).unwrap_or_default();
            v.start_feature(feature, w, cx);
        })
    })
    .unwrap();
    wait_until(cx, window, |cx| !view.read(cx).ui_busy());
    draw(cx, window);
    cx.update(|cx| view.read(cx).current_manip(cx).is_some())
}

fn expect<T: PartialEq + std::fmt::Debug>(got: T, want: T, what: &str) -> Result<(), String> {
    if got == want { Ok(()) } else { Err(format!("{what}: ours {got:?}, live {want:?}")) }
}

type Check = fn(&mut TestAppContext) -> Result<(), String>;

struct Row {
    id: &'static str,
    live: &'static str,
    check: Check,
    gap: bool,
}

fn rows() -> Vec<Row> {
    vec![
        Row { id: "manipulators / which features", live: "Extrude: one arrow; Chamfer, Shell: none", gap: false, check: |cx| {
            expect((manipulator_for(cx, "extrude"), manipulator_for(cx, "chamfer"), manipulator_for(cx, "shell")), (true, false, false), "extrude, chamfer, shell")
        } },
        Row { id: "manipulators / an arrow's grab reach", live: "within 14 px across (16 no), from 4 px behind the base to the tip; nothing past the tip", gap: false, check: |cx| {
            let (window, view) = block(cx, "dlg-reach");
            cx.update_window(window, |_, w, cx| {
                view.update(cx, |v, cx| {
                    let b = v.display.bodies.first().cloned();
                    v.selection = b.map(|b| b.face_refs.into_iter().take(1).collect()).unwrap_or_default();
                    v.start_feature("extrude", w, cx);
                })
            })
            .unwrap();
            wait_until(cx, window, |cx| !view.read(cx).ui_busy());
            draw(cx, window);
            let vp = cx.update(|cx| view.read(cx).viewport.clone());
            let (a, b) = cx
                .update(|cx| {
                    let vp = vp.read(cx);
                    let &(a, b, glyph) = vp.manipulators.first()?;
                    let (a, b) = (vp.project(a)?, vp.project(b)?);
                    Some((a, if glyph > 0.0 { a + (b - a).normalize_or_zero() * glyph } else { b }))
                })
                .ok_or("no arrow")?;
            let (dir, len) = ((b - a).normalize(), (b - a).length());
            let n = dir.perp();
            let hit = |cx: &mut TestAppContext, p: glam::DVec2| cx.update(|cx| vp.read(cx).manipulator_index_at(p).is_some());
            let got = [
                hit(cx, a + dir * (len * 0.5) + n * 13.5),
                hit(cx, a + dir * (len * 0.5) + n * 16.0),
                hit(cx, a - dir * 3.5),
                hit(cx, a - dir * 6.0),
                hit(cx, b + dir * 3.0),
            ];
            expect(got, [true, false, true, false, false], "13.5 across, 16 across, 3.5 behind, 6 behind, 3 past the tip")
        } },
        Row { id: "drag / the length step follows the zoom", live: "10^(round(log10(100 px in the unit)) − 2): 0.1 mm at 0.25 mm/px, 1 mm at 0.414", gap: false, check: |_| {
            let mm = 0.001;
            expect(((linear_step(0.25 * mm, mm) / mm * 1e6).round() / 1e6, (linear_step(0.414 * mm, mm) / mm * 1e6).round() / 1e6), (0.1, 1.0), "steps (mm)")
        } },
        Row { id: "drag / the angle step", live: "5° within 2° (radius ≤ 573 px), free in between: 42.4154 stays, 43.5 → 45", gap: false, check: |_| {
            expect((snap_degrees(42.4154, 300.), snap_degrees(43.5, 300.)), (42.4154, 45.0), "angles")
        } },
        Row { id: "Properties / no field has the keyboard on opening", live: "no field focused; typing does nothing until a field is clicked", gap: false, check: |cx| {
            let (window, view) = block(cx, "dlg-props");
            cx.update_window(window, |_, w, cx| {
                view.update(cx, |v, cx| {
                    let b = v.display.bodies.first().map(|b| b.id.clone()).unwrap();
                    v.open_part_meta(b, w, cx);
                })
            })
            .unwrap();
            draw(cx, window);
            let form = crate::properties_dialog::open_form().ok_or("no modal")?;
            let (field, own) = cx.update_window(window, |_, w, cx| (form.read(cx).a_field_has_focus(w, cx), form.read(cx).has_focus(w))).unwrap();
            expect((field, own), (false, true), "a field focused, the modal focused")
        } },
    ]
}

#[gpui::test]
fn the_dialogs_matrix_matches_live(cx: &mut TestAppContext) {
    let rows = rows();
    let mut report = Vec::new();
    let mut moved = Vec::new();
    for r in &rows {
        let res = (r.check)(cx);
        let ok = res.is_ok();
        report.push(format!("{} {} (live: {}){}", if ok { "=" } else { "≠" }, r.id, r.live, res.err().map(|e| format!("\n    {e}")).unwrap_or_default()));
        if ok == r.gap {
            moved.push(r.id);
        }
    }
    let matched = report.iter().filter(|l| l.starts_with('=')).count();
    eprintln!("dialogs matrix: {matched} / {} rows match live\n{}", report.len(), report.join("\n"));
    assert!(moved.is_empty(), "rows whose status moved (update interaction-feel.md and the row's gap): {moved:?}");
}
