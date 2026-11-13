//! The sketch tools matrix (docs/parity/interaction-feel.md "Sketch tools"): live2's feel/sketchtools.json rows
//! (Search tools' ranking, how the tools chain, stay and leave, the sketch fillet's key) acted out on a sketch on Top.
//! A row ours doesn't match yet is a gap; the test fails when a row's status moves either way.

use super::*;
use noshape_model::features;

fn ranked(query: &str) -> Vec<&'static str> {
    let types: Vec<&'static features::FeatureType> = features::all().filter(|t| t.in_toolbar).collect();
    let names: Vec<&str> = types.iter().map(|t| t.name).collect();
    noshape_ops::tool_search::studio_rank(&names, query).into_iter().map(|i| types[i].name).collect()
}

fn lines(cx: &mut TestAppContext, view: &Entity<PartStudioView>) -> usize {
    cx.update(|cx| view.read(cx).sketch_data(cx).map_or(0, |d| d.sketch.entities().filter(|e| e.kind() == noshape_sketch::EntityKind::Line && !e.construction).count()))
}

fn circles(cx: &mut TestAppContext, view: &Entity<PartStudioView>) -> usize {
    cx.update(|cx| view.read(cx).sketch_data(cx).map_or(0, |d| d.sketch.entities().filter(|e| e.kind() == noshape_sketch::EntityKind::Circle).count()))
}

fn tool(cx: &mut TestAppContext, view: &Entity<PartStudioView>) -> Option<Tool> {
    cx.update(|cx| view.read(cx).sketch.as_ref().map(|s| s.tool))
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
        Row { id: "search / fil", live: "Fillet, Fill, Finish flex PCB model, Finish sheet metal model, 3D fit spline, Modify fillet", gap: false, check: |_| {
            expect(ranked("fil"), vec!["Fillet", "Fill", "Finish flex PCB model", "Finish sheet metal model", "3D fit spline", "Modify fillet"], "fil")
        } },
        Row { id: "search / fillet, chmf, lin pat, ext, sk, xyz", live: "Fillet, Modify fillet; Chamfer; Linear pattern; Extrude, External thread; Sketch; nothing", gap: false, check: |_| {
            expect(ranked("fillet"), vec!["Fillet", "Modify fillet"], "fillet")?;
            expect(ranked("chmf").first().copied(), Some("Chamfer"), "chmf")?;
            expect(ranked("lin pat").first().copied(), Some("Linear pattern"), "lin pat")?;
            expect(ranked("ext"), vec!["Extrude", "External thread"], "ext")?;
            expect(ranked("sk").first().copied(), Some("Sketch"), "sk")?;
            expect(ranked("xyz").len(), 0, "xyz")
        } },
        Row { id: "line / chains", live: "each click after the second continues from the last end", gap: false, check: |cx| {
            let (window, view) = sketch_with(cx, "st-chain", Tool::Line);
            for at in [(400., 300.), (500., 300.), (500., 400.)] {
                click(cx, window, &view, at);
            }
            expect((lines(cx, &view), tool(cx, &view)), (2, Some(Tool::Line)), "lines, tool")
        } },
        Row { id: "circle / stays active", live: "two circles in a row without re-picking the tool", gap: false, check: |cx| {
            let (window, view) = sketch_with(cx, "st-circle", Tool::Circle);
            for at in [(400., 300.), (440., 300.), (600., 300.), (640., 300.)] {
                click(cx, window, &view, at);
            }
            expect((circles(cx, &view), tool(cx, &view)), (2, Some(Tool::Circle)), "circles, tool")
        } },
        Row { id: "line / one Esc mid-chain exits the tool", live: "no tool active, the sketch stays open", gap: false, check: |cx| {
            let (window, view) = sketch_with(cx, "st-esc", Tool::Line);
            click(cx, window, &view, (400., 300.));
            click(cx, window, &view, (500., 300.));
            keys(cx, window, &["escape"]);
            expect(tool(cx, &view), Some(Tool::Select), "tool after one Esc")
        } },
        Row { id: "line / Space does nothing mid-chain", live: "the chain continues", gap: false, check: |cx| {
            let (window, view) = sketch_with(cx, "st-space", Tool::Line);
            click(cx, window, &view, (400., 300.));
            click(cx, window, &view, (500., 300.));
            keys(cx, window, &["space"]);
            click(cx, window, &view, (500., 400.));
            expect((lines(cx, &view), tool(cx, &view)), (2, Some(Tool::Line)), "lines, tool")
        } },
        Row { id: "line / the menu mid-chain", live: "'Escape line', 'Confirm Sketch 1', then the view / selection items; the tool stays", gap: false, check: |cx| {
            let (window, view) = sketch_with(cx, "st-menu", Tool::Line);
            click(cx, window, &view, (400., 300.));
            click(cx, window, &view, (500., 300.));
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.open_sketch_context_menu(gpui_kit::point(px(600.), px(400.)), None, w, cx))).unwrap();
            draw(cx, window);
            let labels = cx.update(|cx| view.read(cx).context_menu.as_ref().map(|(_, m, _)| m.read(cx).labels()).unwrap_or_default());
            let first: Vec<&str> = labels.iter().filter(|l| *l != "———").take(2).map(|s| s.as_str()).collect();
            expect((first, tool(cx, &view)), (vec!["Escape line", "Confirm Sketch 1"], Some(Tool::Line)), "first rows, tool")
        } },
        Row { id: "line / Enter mid-chain dimensions the segment just drawn", live: "the on-canvas length box becomes a driving dimension on it; the chain continues", gap: false, check: |cx| {
            let (window, view) = sketch_with(cx, "st-enter", Tool::Line);
            click(cx, window, &view, (400., 300.));
            click(cx, window, &view, (500., 300.));
            keys(cx, window, &["enter"]);
            let dims = dimensions(cx, &view).len();
            click(cx, window, &view, (500., 400.));
            expect((dims, lines(cx, &view), tool(cx, &view)), (1, 2, Some(Tool::Line)), "dimensions, lines, tool")
        } },
        Row { id: "line / Esc with the mid-chain menu open", live: "behaviours sketch/405–408 (a real X key): the first Esc exits the tool, the menu stays; a second closes it (live2's 11-13 note reads as one Esc doing both: asked)", gap: false, check: |cx| {
            let (window, view) = sketch_with(cx, "st-menu-esc", Tool::Line);
            click(cx, window, &view, (400., 300.));
            click(cx, window, &view, (500., 300.));
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.open_sketch_context_menu(gpui_kit::point(px(600.), px(400.)), None, w, cx))).unwrap();
            draw(cx, window);
            keys(cx, window, &["escape"]);
            let open = cx.update(|cx| view.read(cx).context_menu.is_some());
            expect((open, tool(cx, &view)), (true, Some(Tool::Select)), "after one Esc: menu open, tool")
        } },
        Row { id: "fillet / Shift+F", live: "the sketch fillet's key", gap: false, check: |cx| {
            let (window, view) = sketch_with(cx, "st-fillet", Tool::Select);
            keys(cx, window, &["shift-f"]);
            expect(tool(cx, &view), Some(Tool::Fillet), "tool")
        } },
    ]
}

#[gpui::test]
fn the_sketch_tools_matrix_matches_live(cx: &mut TestAppContext) {
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
    eprintln!("sketch tools matrix: {matched} / {} rows match live\n{}", report.len(), report.join("\n"));
    assert!(moved.is_empty(), "rows whose status moved (update interaction-feel.md and the row's gap): {moved:?}");
}
