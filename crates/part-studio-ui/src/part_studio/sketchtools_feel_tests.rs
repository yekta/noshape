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

/// A sketch with a horizontal line from x0 to x1 at y 300 (px) and two vertical ones at x, from 240 to 360.
fn three_lines(cx: &mut TestAppContext, name: &str, xs: [(f64, f64); 3]) -> (AnyWindowHandle, Entity<PartStudioView>) {
    let (window, view) = sketch_with(cx, name, Tool::Line);
    for (k, (a, b)) in xs.into_iter().enumerate() {
        let (from, to) = if k == 0 { ((a, 300.), (b, 300.)) } else { ((a, 240.), (b, 360.)) };
        click(cx, window, &view, from);
        click(cx, window, &view, to);
        keys(cx, window, &["escape"]);
        cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Line, w, cx))).unwrap();
    }
    (window, view)
}

/// The pointer moved to `at` (viewport px) with nothing pressed.
fn pointer(cx: &mut TestAppContext, window: AnyWindowHandle, view: &Entity<PartStudioView>, at: (f64, f64)) {
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
        let vp = v.viewport.clone();
        v.on_viewport_event(&vp, &ViewportEvent::PointerMove { position: glam::DVec2::new(at.0, at.1), modifiers: Modifiers::default() }, w, cx);
    }))
    .unwrap();
    draw(cx, window);
}

/// The lengths of the sketch's solid lines (metres).
fn line_lengths(data: &noshape_model::sketch_feature::SketchData) -> Vec<f64> {
    let sk = &data.sketch;
    sk.entities().filter(|e| e.kind() == noshape_sketch::EntityKind::Line && !e.construction).filter_map(|e| sk.endpoints(e.id)).filter_map(|(a, b)| Some((sk.point(b)? - sk.point(a)?).length())).collect()
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
        Row { id: "trim / a click removes the piece between the crossings", live: "H -100..0 crossed at -70 and -40: the middle piece goes", gap: false, check: |cx| {
            let (window, view) = sketch_with(cx, "st-trim", Tool::Line);
            for (a, b) in [((300., 300.), (600., 300.)), ((400., 200.), (400., 400.)), ((500., 200.), (500., 400.))] {
                click(cx, window, &view, a);
                click(cx, window, &view, b);
                keys(cx, window, &["escape"]);
                cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Line, w, cx))).unwrap();
            }
            let before = lines(cx, &view);
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Trim, w, cx))).unwrap();
            click(cx, window, &view, (450., 300.));
            // The crossed line keeps its two outer pieces: 3 lines -> 4.
            expect((before, lines(cx, &view)), (3, 4), "lines before, after the trim")
        } },
        Row { id: "trim / hover lights the piece", live: "the piece up to the nearest crossings lit tan with orange dots at its ends", gap: false, check: |cx| {
            let (window, view) = three_lines(cx, "st-trim-hover", [(300., 600.), (400., 400.), (500., 500.)]);
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Trim, w, cx))).unwrap();
            pointer(cx, window, &view, (450., 300.));
            // What the pick buffer under the pointer gives (no GPU here): the crossed line.
            cx.update(|cx| view.update(cx, |v, cx| {
                let data = v.sketch_data(cx).unwrap();
                let sk = &data.sketch;
                let longest = sk.entities().filter(|e| e.kind() == noshape_sketch::EntityKind::Line).max_by(|a, b| sk.curve(a.id).map(|c| c.length()).unwrap_or(0.).total_cmp(&sk.curve(b.id).map(|c| c.length()).unwrap_or(0.))).map(|e| e.id);
                v.sketch.as_mut().unwrap().hover = longest;
            }));
            let (dots, piece, whole) = cx.update(|cx| {
                let v = view.read(cx);
                let (ed, data) = (v.sketch.as_ref().unwrap(), v.sketch_data(cx).unwrap());
                let (_, points) = ed.overlay(&data, &noshape_units::UnitPrefs::default());
                let lit = noshape_scene::palette::rgb8(noshape_scene::palette::viewport::PREHIGHLIGHT);
                let dots: Vec<glam::Vec3> = points.points.iter().filter(|p| p.color == lit).map(|p| glam::Vec3::from(p.position)).collect();
                let piece = if dots.len() == 2 { (dots[1] - dots[0]).length() as f64 } else { 0. };
                (dots.len(), piece, line_lengths(&data).into_iter().fold(0f64, f64::max))
            });
            // The middle piece is a third of the crossed line.
            expect((dots, (piece / whole * 3.).round()), (2, 1.), "end dots, the lit piece in thirds of the line")
        } },
        Row { id: "trim / a drag removes every piece its trail crosses", live: "tr0: after the middle piece's trim, one stroke across H's left piece and the line at -70 removes both (that line whole, nothing crosses it any more)", gap: false, check: |cx| {
            // live's -100..0 at y 40, verticals at -70 and -40 (20..60), at 3 px a millimetre.
            let (window, view) = three_lines(cx, "st-trim-drag", [(300., 600.), (390., 390.), (480., 480.)]);
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Trim, w, cx))).unwrap();
            click(cx, window, &view, (435., 300.));
            let after_click = lines(cx, &view);
            drag(cx, window, &view, (340., 320.), (420., 260.), 8);
            let mut left: Vec<i64> = cx.update(|cx| line_lengths(&view.read(cx).sketch_data(cx).unwrap())).iter().map(|l| (l * 1000.).round() as i64).collect();
            left.sort();
            expect((after_click, lines(cx, &view), left.len()), (4, 2, 2), "lines after the click, after the drag")
        } },
        // (live2's "extendHover" light blue was the sketch's X axis under its line: behaviours "psui (2992)".)
        Row { id: "extend / hover draws no extension", live: "behaviours 'psui (2992)': the line and its end lit orange, nothing past it; the clicks: the curve, then the boundary", gap: false, check: |cx| {
            let (window, view) = sketch_with(cx, "st-extend-hover", Tool::Line);
            for (a, b) in [((300., 300.), (400., 300.)), ((500., 240.), (500., 360.))] {
                click(cx, window, &view, a);
                click(cx, window, &view, b);
                keys(cx, window, &["escape"]);
                cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Line, w, cx))).unwrap();
            }
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Extend, w, cx))).unwrap();
            pointer(cx, window, &view, (395., 300.));
            let ahead = cx.update(|cx| view.update(cx, |v, cx| {
                let data = v.sketch_data(cx).unwrap();
                let sk = &data.sketch;
                // (The sketch view is tilted: the lines by length.) The short line, what the pick under the pointer
                // gives; its end by the pointer is the second one drawn.
                let line = |e: &&noshape_sketch::Entity| e.kind() == noshape_sketch::EntityKind::Line;
                let len = |id| sk.curve(id).map_or(0., |c| c.length());
                let mut ids: Vec<_> = sk.entities().filter(line).map(|e| e.id).collect();
                ids.sort_by(|a, b| len(*a).total_cmp(&len(*b)));
                let (Some(&short), Some(&long)) = (ids.first(), ids.last()) else { return false };
                let (Some(noshape_sketch::Curve2::Line { a, b }), Some(far)) = (sk.curve(short), sk.curve(long).map(|c| c.point(0.5))) else { return false };
                let ed = v.sketch.as_mut().unwrap();
                ed.hover = Some(short);
                let (we, wa, wf) = (ed.frame.to_world(b), ed.frame.to_world(a), ed.frame.to_world(far));
                let (lines, _) = ed.overlay(&data, &noshape_units::UnitPrefs::default());
                let dir = (we - wa).normalize();
                // A segment on the line's extension past its end, toward the other line.
                lines.segments.iter().any(|sg| {
                    let mid = (glam::DVec3::from(glam::Vec3::from(sg.a)) + glam::DVec3::from(glam::Vec3::from(sg.b))) * 0.5;
                    let along = (mid - we).dot(dir);
                    let off = ((mid - we) - dir * along).length();
                    along > 0. && along < (wf - we).length() * 1.5 && off < 1e-5
                })
            }));
            expect(ahead, false, "an extension drawn past the line's end")
        } },
        Row { id: "text / two clicks, then its dialog", live: "the first corner, then the opposite one; the Text dialog opens with 'Default text' selected", gap: false, check: |cx| {
            let (window, view) = sketch_with(cx, "st-text", Tool::Text);
            click(cx, window, &view, (300., 300.));
            let after_one = cx.update(|cx| view.read(cx).sketch.as_ref().unwrap().text_dialog_open());
            click(cx, window, &view, (500., 350.));
            let (open, text) = cx.update(|cx| view.read(cx).sketch.as_ref().map(|s| (s.text_dialog_open(), s.text_string.clone())).unwrap());
            expect((after_one, open, text.as_str()), (false, true, "Default text"), "dialog after one click, after two, its text")
        } },
        Row { id: "search / in a sketch", live: "sketch tools: 'Offset o', 'Linear pattern', 'Circular pattern', 'Mirror', 'Text', 'Insert image', 'Trim', 'Extend'", gap: false, check: |_| {
            let tools = noshape_ops::tool_search::searchable_sketch_tools();
            let names: Vec<&str> = tools.iter().map(|t| t.name()).collect();
            let missing: Vec<&str> = ["Offset", "Linear pattern", "Circular pattern", "Mirror", "Text", "Insert image", "Trim", "Extend"].into_iter().filter(|n| !names.contains(n)).collect();
            let offset_key = tools.iter().find(|t| t.name() == "Offset").and_then(|t| t.shortcut()).map(str::to_lowercase);
            expect((missing, offset_key.as_deref()), (vec![], Some("o")), "missing, Offset's key")
        } },
        Row { id: "fillet / a click on a corner previews, nothing committed", live: "a preview arc 'R5' with an arrow; nothing committed yet; Esc cancels", gap: false, check: |cx| {
            let (window, view) = sketch_with(cx, "st-fillet-click", Tool::Line);
            for at in [(400., 300.), (500., 300.), (500., 400.)] {
                click(cx, window, &view, at);
            }
            keys(cx, window, &["escape"]);
            keys(cx, window, &["shift-f"]);
            click(cx, window, &view, (500., 300.));
            let arcs = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).sketch_data(cx).map_or(0, |d| d.sketch.entities().filter(|e| e.kind() == noshape_sketch::EntityKind::Arc).count()));
            let after_click = arcs(cx);
            keys(cx, window, &["escape"]);
            expect((after_click, arcs(cx)), (0, 0), "arcs after the click, after Esc")
        } },
        Row { id: "fillet / a drag from the corner creates it", live: "pressing on the corner and dragging: on release the fillet is created (an arc, two tangents, a radius) and its radius box opens", gap: false, check: |cx| {
            let (window, view) = sketch_with(cx, "st-fillet-drag", Tool::Line);
            for at in [(400., 300.), (500., 300.), (500., 400.)] {
                click(cx, window, &view, at);
            }
            keys(cx, window, &["escape"]);
            keys(cx, window, &["shift-f"]);
            drag(cx, window, &view, (500., 300.), (480., 320.), 6);
            let arcs = cx.update(|cx| view.read(cx).sketch_data(cx).map_or(0, |d| d.sketch.entities().filter(|e| e.kind() == noshape_sketch::EntityKind::Arc).count()));
            expect(arcs, 1, "arcs after the drag")
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
