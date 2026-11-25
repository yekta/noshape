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

/// Every entity of the sketch (points included).
fn entities(cx: &mut TestAppContext, view: &Entity<PartStudioView>) -> usize {
    cx.update(|cx| view.read(cx).sketch_data(cx).map_or(0, |d| d.sketch.entities().count()))
}

fn kinds(cx: &mut TestAppContext, view: &Entity<PartStudioView>, kind: noshape_sketch::EntityKind) -> usize {
    cx.update(|cx| view.read(cx).sketch_data(cx).map_or(0, |d| d.sketch.entities().filter(|e| e.kind() == kind).count()))
}

/// A line drawn and ended, then `t` on: the entities there are.
fn line_then(cx: &mut TestAppContext, window: AnyWindowHandle, view: &Entity<PartStudioView>, t: Tool) -> usize {
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Line, w, cx))).unwrap();
    click(cx, window, view, (300., 300.));
    click(cx, window, view, (400., 300.));
    keys(cx, window, &["escape"]);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(t, w, cx))).unwrap();
    click(cx, window, view, (400., 300.));
    entities(cx, view)
}

/// A double-click at `at`: a click, then a second press with click count 2.
fn dclick(cx: &mut TestAppContext, window: AnyWindowHandle, view: &Entity<PartStudioView>, at: (f64, f64)) {
    click(cx, window, view, at);
    let p = glam::DVec2::new(at.0, at.1);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
        let vp = v.viewport.clone();
        v.on_viewport_event(&vp, &ViewportEvent::ToolDown { position: p, modifiers: Modifiers::default(), click_count: 2 }, w, cx);
        v.on_viewport_event(&vp, &ViewportEvent::ToolUp { position: p, modifiers: Modifiers::default() }, w, cx);
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
        Row { id: "arc / fillet / chamfer / use / mirror: the menu's head", live: "'Escape arc', 'Escape tangent arc', 'Escape center point arc', 'Escape fillet', 'Escape chamfer', 'Escape use', 'Escape mirror' (sketch-tools-1123)", gap: false, check: |cx| {
            let heads: Vec<String> = [Tool::Arc3, Tool::TangentArc, Tool::CenterArc, Tool::Fillet, Tool::Chamfer, Tool::Use, Tool::Mirror].into_iter().map(|t| {
                let (window, view) = sketch_with(cx, "st-menu-head", t);
                cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.open_sketch_context_menu(gpui_kit::point(px(600.), px(400.)), None, w, cx))).unwrap();
                draw(cx, window);
                let labels = cx.update(|cx| view.read(cx).context_menu.as_ref().map(|(_, m, _)| m.read(cx).labels()).unwrap_or_default());
                labels.into_iter().find(|l| l != "———").unwrap_or_default()
            }).collect();
            expect(heads, ["Escape arc", "Escape tangent arc", "Escape center point arc", "Escape fillet", "Escape chamfer", "Escape use", "Escape mirror"].map(String::from).to_vec(), "heads")
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
        Row { id: "keys / Ctrl+A selects nothing", live: "keys-1118: Ctrl+A in a sketch, the pointer over the graphics, selects nothing (the browser takes it)", gap: false, check: |cx| {
            let (window, view) = three_lines(cx, "st-ctrl-a", [(300., 600.), (400., 400.), (500., 500.)]);
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Select, w, cx))).unwrap();
            pointer(cx, window, &view, (700., 450.));
            keys(cx, window, &["secondary-a"]);
            let picked = cx.update(|cx| {
                let v = view.read(cx);
                (v.sketch.as_ref().unwrap().selection.len(), v.selection.len())
            });
            expect(picked, (0, 0), "sketch entities, model references selected")
        } },
        Row { id: "trim / hover reach", live: "sktools-1118: the piece lit tan with the pointer 3–6 px off the line, nothing at 7", gap: false, check: |cx| {
            let (window, view) = three_lines(cx, "st-trim-reach", [(300., 600.), (400., 400.), (500., 500.)]);
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Trim, w, cx))).unwrap();
            let lit = [6., 7.].map(|d| {
                pointer(cx, window, &view, (350., 300. - d));
                cx.update(|cx| {
                    let v = view.read(cx);
                    let (ed, data) = (v.sketch.as_ref().unwrap(), v.sketch_data(cx).unwrap());
                    let vp = v.viewport.read(cx);
                    let (o, dir) = vp.ray(glam::DVec2::new(350., 300. - d));
                    let p = ed.plane_point(o, dir).unwrap();
                    let sk = &data.sketch;
                    let level = sk.entities().filter(|e| e.kind() == noshape_sketch::EntityKind::Line).max_by(|a, b| sk.curve(a.id).map(|c| c.length()).unwrap_or(0.).total_cmp(&sk.curve(b.id).map(|c| c.length()).unwrap_or(0.))).unwrap().id;
                    v.in_sketch_reach(level, p, cx)
                })
            });
            expect(lit, [true, false], "lit at 6 px, at 7 px")
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
        Row { id: "insert image / its dialog, then a box at the image's aspect", live: "sk-tool-insert-image, 1715: 'Insert an image' with Current document | Other documents | Libraries, Libraries open; after the pick two clicks set opposite corners of a box with the image's aspect", gap: false, check: |cx| {
            let (window, view) = sketch_with(cx, "st-image", Tool::InsertImage);
            let dialog = cx.update(|cx| view.read(cx).ui_ref().and_then(|u| u.insert_panel.as_ref()).map(|(p, _)| (p.read(cx).title, p.read(cx).in_libraries())));
            cx.update(|cx| view.update(cx, |v, _| v.sketch.as_mut().unwrap().pending_image = Some(("guide.png".into(), vec![1, 2, 3], 2.0))));
            draw(cx, window);
            click(cx, window, &view, (300., 300.));
            click(cx, window, &view, (500., 330.));
            let aspect = cx.update(|cx| {
                let d = view.read(cx).sketch_data(cx)?;
                let ps: Vec<glam::DVec2> = d.images.first()?.box_points.iter().filter_map(|p| d.sketch.point(noshape_sketch::EntityId(*p))).collect();
                let (lo, hi) = ps.iter().fold((ps[0], ps[0]), |(lo, hi), p| (lo.min(*p), hi.max(*p)));
                Some(((hi.x - lo.x) / (hi.y - lo.y) * 1000.).round() / 1000.)
            });
            expect((dialog, aspect), (Some((Some("Insert an image"), true)), Some(2.0)), "the dialog's title and Libraries tab, the box's width / height")
        } },
        Row { id: "insert image / the user's images library", live: "behaviours dx (11-21 15:31): Library lists '‹name› images' above Onshape images, which stays the default; choosing it sets Type to its first entry and Image to that one's first image; Done takes it to the image box", gap: false, check: |cx| {
            let dir = std::env::temp_dir().join(format!("noshape-st-imglib-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(dir.join("lib")).map_err(|e| e.to_string())?;
            let png = dir.join("img-A.png");
            std::fs::write(&png, noshape_scene::png::encode_png(vec![255; 2 * 4], 2, 1, 2, 1)?).map_err(|e| e.to_string())?;
            let mut ctx = noshape_ops::Ctx::new();
            for (tool, args) in [
                ("document_create", serde_json::json!({ "path": dir.join("lib").join("img doc two.noshape") })),
                ("import_files", serde_json::json!({ "files": [png] })),
                ("history_create_version", serde_json::json!({ "name": "V1" })),
            ] {
                noshape_ops::registry::run(&mut ctx, noshape_ops::registry::by_tool_name(tool).ok_or(tool)?, &args).map_err(|e| e.message)?;
            }
            let lib = noshape_ops::images_library::build(&dir.join("lib"));
            let (window, view) = sketch_with(cx, "st-image-lib", Tool::InsertImage);
            let panel = cx.update(|cx| view.read(cx).ui_ref().and_then(|u| u.insert_panel.as_ref()).map(|(p, _)| p.clone())).ok_or("no dialog")?;
            cx.update(|cx| panel.update(cx, |p, cx| p.set_images_library(lib, cx)));
            draw(cx, window);
            cx.update(|cx| panel.update(cx, |p, cx| p.pick_in_libraries("sel-lib-library", "user", cx)));
            draw(cx, window);
            let done = gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds("sel-lib-image-done").ok_or("no Done")?;
            gpui_kit::VisualTestContext::from_window(window, cx).simulate_click(done.center(), Modifiers::default());
            draw(cx, window);
            let pending = cx.update(|cx| view.read(cx).sketch.as_ref().and_then(|s| s.pending_image.as_ref()).map(|(n, _, a)| (n.clone(), *a)));
            let _ = std::fs::remove_dir_all(&dir);
            expect(pending, Some(("img-A.png".to_string(), 2.0)), "the library's first image (2:1) ready for its box")
        } },
        Row { id: "search / Alt+C opens the panel", live: "search-tools-1118: a 600 px panel 10 px under the toolbar at the window's right edge, the toolbar box 'Close search', the input focused, 'Start typing to search tools', Highlight tool in toolbar on; Esc closes it", gap: false, check: |cx| {
            let (window, view) = sketch_with(cx, "st-search-open", Tool::Select);
            keys(cx, window, &["alt-c"]);
            draw(cx, window);
            let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
            let panel = vcx.debug_bounds("tool-search").ok_or("no panel")?;
            let start = vcx.debug_bounds("tool-search-start").is_some();
            let (width, right) = cx.update_window(window, |_, w, _| (w.viewport_size().width, panel.right())).unwrap();
            let highlight = cx.update(|cx| !view.read(cx).ui.palette_no_highlight);
            gpui_kit::VisualTestContext::from_window(window, cx).simulate_input("f");
            draw(cx, window);
            let typed = cx.update(|cx| view.read(cx).palette_query.clone());
            keys(cx, window, &["escape"]);
            let closed = cx.update(|cx| view.read(cx).palette.is_none());
            expect((f32::from(panel.size.width).round(), (f32::from(right - width)).abs() < 1., start, highlight, typed.as_str(), closed), (600., true, true, true, "f", true), "the panel's width, at the right edge, 'Start typing', Highlight on, the input typed into, Esc closes")
        } },
        Row { id: "search / Recently used and the info box", live: "behaviours 4.9 (10-27): an empty search lists a bold 'Recently used' over the tools used last, newest first (Delete part, Extrude), the first row highlighted; the right pane shows its icon, bold name and description 'Delete one or more parts or surfaces. For composite parts, specify whether to delete, dissolve, or ignore.'", gap: false, check: |cx| {
            use super::super::super::view_tools::PaletteItem;
            let (window, view) = part_studio(cx, "st-search-recent");
            for t in ["extrude", "deleteBodies"] {
                cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature(t, w, cx))).unwrap();
                draw(cx, window);
                keys(cx, window, &["escape"]);
            }
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.toggle_palette(w, cx))).unwrap();
            draw(cx, window);
            let mut vcx = gpui_kit::VisualTestContext::from_window(window, cx);
            let (heading, info) = (vcx.debug_bounds("tool-search-recent").is_some(), vcx.debug_bounds("tool-search-info").is_some());
            let (names, sel) = cx.update(|cx| {
                let v = view.read(cx);
                (v.palette_items(cx).iter().map(|i| if let PaletteItem::Feature(id) = i { features::get(id).map_or("", |t| t.name) } else { "" }).collect::<Vec<_>>(), v.ui.palette_sel)
            });
            let desc = crate::tool_text::description("deleteBodies");
            expect(
                (heading, names, sel, info, desc),
                (true, vec!["Delete part", "Extrude"], 0, true, Some("Delete one or more parts or surfaces. For composite parts, specify whether to delete, dissolve, or ignore.")),
                "'Recently used', newest first, the first row highlighted, its info pane",
            )
        } },
        Row { id: "search / no Recently used in a sketch", live: "sketch-feel-1125: in a sketch Alt+C, 'fil', Enter (Fillet), Esc, Alt+C again: no 'Recently used', only 'Start typing to search tools' (the Part Studio's lists it)", gap: false, check: |cx| {
            let (window, view) = sketch_with(cx, "st-search-no-recent", Tool::Select);
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.toggle_palette(w, cx))).unwrap();
            draw(cx, window);
            gpui_kit::VisualTestContext::from_window(window, cx).simulate_input("fil");
            draw(cx, window);
            keys(cx, window, &["enter"]);
            let fillet = cx.update(|cx| view.read(cx).sketch.as_ref().map(|ed| ed.tool));
            keys(cx, window, &["escape"]);
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.toggle_palette(w, cx))).unwrap();
            draw(cx, window);
            let heading = gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds("tool-search-recent").is_some();
            let items = cx.update(|cx| view.read(cx).palette_items(cx).len());
            expect((fillet, heading, items), (Some(Tool::Fillet), false, 0), "Fillet run from the panel; then no 'Recently used', nothing listed")
        } },
        Row { id: "search / the found tool lit in the sketch toolbar", live: "search-tools-1118: Alt+C in a sketch; 'f' lights Fillet's button yellow (its group's face), a member under a ⌄ lights the ⌄ (Fix: the test window's collapsed constraints), Highlight tool in toolbar off lights nothing", gap: false, check: |cx| {
            let (window, view) = sketch_with(cx, "st-search-lit", Tool::Select);
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.toggle_palette(w, cx))).unwrap();
            draw(cx, window);
            let lit = |cx: &mut TestAppContext, typed: &str| {
                gpui_kit::VisualTestContext::from_window(window, cx).simulate_input(typed);
                draw(cx, window);
                let names = ["Fillet", "caret Fillet", "Constraint(Fix)", "caret Constraint(Coincident)", "extrude"];
                let lit: Vec<&str> = names.into_iter().filter(|n| gpui_kit::VisualTestContext::from_window(window, cx).debug_bounds(Box::leak(format!("search-lit:{n}").into_boxed_str())).is_some()).collect();
                cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
                    if let Some((st, _)) = &v.palette {
                        st.clone().update(cx, |s, cx| s.set_value("", w, cx));
                    }
                    v.palette_query.clear();
                })).unwrap();
                lit
            };
            let found = [lit(cx, "f"), lit(cx, "chamfer"), lit(cx, "fix"), lit(cx, "extrude")];
            cx.update(|cx| view.update(cx, |v, _| v.ui.palette_no_highlight = true));
            let off = lit(cx, "f");
            expect((found.map(|l| l.join(",")), off.len()), (["Fillet", "caret Fillet", "caret Constraint(Coincident)", "extrude"].map(String::from), 0), "lit for f / chamfer / fix / extrude, lit with the highlight off")
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
        Row { id: "fillet / a drag from the corner creates it", live: "pressing on the corner and dragging: the radius follows the pointer, on release the fillet is created at it (an arc, two tangents, a radius) and its radius box opens (live sketch-tools-1123)", gap: false, check: |cx| {
            let (window, view) = sketch_with(cx, "st-fillet-drag", Tool::Line);
            for at in [(400., 300.), (500., 300.), (500., 400.)] {
                click(cx, window, &view, at);
            }
            keys(cx, window, &["escape"]);
            keys(cx, window, &["shift-f"]);
            let st = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).sketch.as_ref().map(|ed| format!("tool {:?} pending {} sel {} msg {:?}", ed.tool, ed.corner_pending(), ed.selection.len(), ed.message)));
            let ev = |cx: &mut TestAppContext, ev: ViewportEvent| {
                cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| {
                    let vp = v.viewport.clone();
                    v.on_viewport_event(&vp, &ev, w, cx);
                }))
                .unwrap();
                draw(cx, window);
            };
            let at = |x: f64, y: f64| glam::DVec2::new(x, y);
            ev(cx, ViewportEvent::PointerMove { position: at(500., 300.), modifiers: Modifiers::default() });
            ev(cx, ViewportEvent::ToolDown { position: at(500., 300.), modifiers: Modifiers::default(), click_count: 1 });
            let pressed = st(cx);
            // (A short drag in: the test's view is oblique, 28 px there is a radius the corner can't take, red, as live.)
            for k in 1..=6 {
                let t = k as f64 / 6.0;
                ev(cx, ViewportEvent::ToolDrag { position: at(500. - 4. * t, 300. + 4. * t) });
            }
            let dragged = st(cx);
            ev(cx, ViewportEvent::ToolUp { position: at(496., 304.), modifiers: Modifiers::default() });
            let arcs = cx.update(|cx| view.read(cx).sketch_data(cx).map_or(0, |d| d.sketch.entities().filter(|e| e.kind() == noshape_sketch::EntityKind::Arc).count()));
            expect(arcs, 1, &format!("arcs after the drag (pressed: {pressed:?}; dragged: {dragged:?})"))?;
            let boxed = cx.update(|cx| view.read(cx).sketch.as_ref().is_some_and(|ed| ed.dim_edit.is_some()) && view.read(cx).dim_input.is_some());
            expect(boxed, true, "the radius box open after the release")
        } },
        Row { id: "fillet / Shift+F", live: "the sketch fillet's key", gap: false, check: |cx| {
            let (window, view) = sketch_with(cx, "st-fillet", Tool::Select);
            keys(cx, window, &["shift-f"]);
            expect(tool(cx, &view), Some(Tool::Fillet), "tool")
        } },
        // Live sketch-tools-1123 (arcs.md, ellipse.md, polygon.md, slot.md, spline.md, fillet.md, mirror-pattern.md,
        // text.md, INDEX.md "Endings").
        Row { id: "arcs / ellipse / polygon: one Esc discards the pending shape and exits", live: "nothing kept, the tool off", gap: false, check: |cx| {
            for (t, n) in [(Tool::Arc3, 2), (Tool::CenterArc, 2), (Tool::TangentArc, 0), (Tool::Ellipse, 2), (Tool::Polygon, 2), (Tool::Conic, 2)] {
                let (window, view) = sketch_with(cx, "st-esc-pending", t);
                let start = if t == Tool::TangentArc { line_then(cx, window, &view, t) } else { 0 };
                for k in 0..n {
                    click(cx, window, &view, (400. + 60. * k as f64, 300. - 30. * k as f64));
                }
                if t == Tool::TangentArc {
                    pointer(cx, window, &view, (560., 240.));
                }
                keys(cx, window, &["escape"]);
                expect((entities(cx, &view) - start, tool(cx, &view)), (0, Some(Tool::Select)), &format!("{t:?}: new entities, tool"))?;
            }
            Ok(())
        } },
        Row { id: "arcs / Enter does nothing", live: "the pending arc and the tool stay; the next click commits it", gap: false, check: |cx| {
            let (window, view) = sketch_with(cx, "st-arc-enter", Tool::Arc3);
            click(cx, window, &view, (400., 300.));
            click(cx, window, &view, (500., 300.));
            keys(cx, window, &["enter"]);
            expect((kinds(cx, &view, noshape_sketch::EntityKind::Arc), tool(cx, &view)), (0, Some(Tool::Arc3)), "after Enter: arcs, tool")?;
            click(cx, window, &view, (450., 260.));
            expect(kinds(cx, &view, noshape_sketch::EntityKind::Arc), 1, "arcs after the third click")
        } },
        Row { id: "3 point arc / a double-click is the last click", live: "the arc committed, the tool stays", gap: false, check: |cx| {
            let (window, view) = sketch_with(cx, "st-arc-dbl", Tool::Arc3);
            click(cx, window, &view, (400., 300.));
            click(cx, window, &view, (500., 300.));
            dclick(cx, window, &view, (450., 260.));
            expect((kinds(cx, &view, noshape_sketch::EntityKind::Arc), tool(cx, &view)), (1, Some(Tool::Arc3)), "arcs, tool")
        } },
        Row { id: "tangent arc / a click in free space does nothing", live: "it needs an existing end", gap: false, check: |cx| {
            let (window, view) = sketch_with(cx, "st-tarc-free", Tool::TangentArc);
            click(cx, window, &view, (400., 300.));
            click(cx, window, &view, (480., 260.));
            expect((entities(cx, &view), tool(cx, &view)), (0, Some(Tool::TangentArc)), "entities, tool")
        } },
        Row { id: "arcs / ellipse / polygon / conic: the tool stays after a commit", live: "no chaining (only Line chains), the tool stays on (the polygon commits on Enter)", gap: false, check: |cx| {
            for (t, n) in [(Tool::Arc3, 3), (Tool::CenterArc, 3), (Tool::Ellipse, 3), (Tool::Polygon, 2), (Tool::Conic, 3)] {
                let (window, view) = sketch_with(cx, "st-stays", t);
                for at in [(400., 300.), (500., 300.), (450., 250.)].into_iter().take(n) {
                    click(cx, window, &view, at);
                }
                if t == Tool::Polygon {
                    keys(cx, window, &["enter"]);
                }
                let made = entities(cx, &view);
                if made == 0 {
                    return Err(format!("{t:?}: nothing committed"));
                }
                pointer(cx, window, &view, (600., 400.));
                expect((tool(cx, &view), entities(cx, &view)), (Some(t), made), &format!("{t:?}: tool, nothing pending added"))?;
            }
            Ok(())
        } },
        Row { id: "spline / the tool button or another tool mid-draw", live: "nothing kept", gap: false, check: |cx| {
            for again in [true, false] {
                let (window, view) = sketch_with(cx, "st-spline-switch", Tool::Spline);
                click(cx, window, &view, (400., 300.));
                click(cx, window, &view, (460., 260.));
                click(cx, window, &view, (520., 300.));
                if again {
                    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Spline, w, cx))).unwrap();
                } else {
                    keys(cx, window, &["l"]);
                }
                draw(cx, window);
                expect(kinds(cx, &view, noshape_sketch::EntityKind::FitSpline) + kinds(cx, &view, noshape_sketch::EntityKind::BSpline), 0, if again { "splines after the tool button" } else { "splines after l" })?;
            }
            Ok(())
        } },
        Row { id: "fillet / a double-click on a corner", live: "commits nothing", gap: false, check: |cx| {
            let (window, view) = sketch_with(cx, "st-fillet-dbl", Tool::Line);
            click(cx, window, &view, (400., 300.));
            click(cx, window, &view, (500., 300.));
            click(cx, window, &view, (500., 200.));
            keys(cx, window, &["escape"]);
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Fillet, w, cx))).unwrap();
            dclick(cx, window, &view, (500., 300.));
            expect(kinds(cx, &view, noshape_sketch::EntityKind::Arc), 0, "arcs")
        } },
        Row { id: "fillet / the tool button or l with a corner pending", live: "nothing kept", gap: false, check: |cx| {
            for again in [true, false] {
                let (window, view) = sketch_with(cx, "st-fillet-switch", Tool::Line);
                click(cx, window, &view, (400., 300.));
                click(cx, window, &view, (500., 300.));
                click(cx, window, &view, (500., 200.));
                keys(cx, window, &["escape"]);
                cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Fillet, w, cx))).unwrap();
                click(cx, window, &view, (500., 300.));
                if again {
                    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Fillet, w, cx))).unwrap();
                } else {
                    keys(cx, window, &["l"]);
                }
                draw(cx, window);
                expect(kinds(cx, &view, noshape_sketch::EntityKind::Arc), 0, if again { "arcs after the tool button" } else { "arcs after l" })?;
            }
            Ok(())
        } },
        Row { id: "mirror / Enter does nothing", live: "the tool and its line stay; the next click still mirrors", gap: false, check: |cx| {
            let (window, view) = sketch_with(cx, "st-mirror-enter", Tool::Line);
            click(cx, window, &view, (400., 200.));
            click(cx, window, &view, (400., 400.));
            keys(cx, window, &["escape"]);
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Line, w, cx))).unwrap();
            click(cx, window, &view, (300., 250.));
            click(cx, window, &view, (350., 350.));
            keys(cx, window, &["escape"]);
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Mirror, w, cx))).unwrap();
            click(cx, window, &view, (400., 330.));
            keys(cx, window, &["enter"]);
            expect((tool(cx, &view), lines(cx, &view)), (Some(Tool::Mirror), 2), "after Enter: tool, lines")?;
            click(cx, window, &view, (325., 300.));
            expect(lines(cx, &view), 3, "lines after the next click")
        } },
        Row { id: "use / Enter does nothing, one Esc turns it off", live: "Enter leaves the tool on with the sketch as it was; one Esc turns it off and keeps everything (sketch-tools-1123)", gap: false, check: |cx| {
            let (window, view) = sketch_with(cx, "st-use-enter", Tool::Line);
            click(cx, window, &view, (400., 300.));
            click(cx, window, &view, (450., 300.));
            keys(cx, window, &["escape"]);
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Use, w, cx))).unwrap();
            keys(cx, window, &["enter"]);
            expect((tool(cx, &view), lines(cx, &view)), (Some(Tool::Use), 1), "after Enter: tool, lines")?;
            keys(cx, window, &["escape"]);
            expect((tool(cx, &view), lines(cx, &view)), (Some(Tool::Select), 1), "after one Esc: tool, lines")
        } },
        Row { id: "pattern / Enter with a seed", live: "commits nothing; the tool stays", gap: false, check: |cx| {
            let (window, view) = sketch_with(cx, "st-pattern-enter", Tool::Line);
            click(cx, window, &view, (400., 300.));
            click(cx, window, &view, (450., 300.));
            keys(cx, window, &["escape", "escape"]);
            click(cx, window, &view, (425., 300.));
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::LinearPattern, w, cx))).unwrap();
            draw(cx, window);
            keys(cx, window, &["enter"]);
            expect((lines(cx, &view), tool(cx, &view)), (1, Some(Tool::LinearPattern)), "lines, tool")
        } },
        Row { id: "text / ✓", live: "commits the text and turns the Text tool off", gap: false, check: |cx| {
            let (window, view) = sketch_with(cx, "st-text-ok", Tool::Text);
            click(cx, window, &view, (400., 300.));
            click(cx, window, &view, (500., 340.));
            keys(cx, window, &["a", "b"]);
            cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.accept_sketch_text(w, cx))).unwrap();
            draw(cx, window);
            let texts = cx.update(|cx| view.read(cx).sketch_data(cx).map_or(0, |d| d.texts.len()));
            expect((texts, tool(cx, &view)), (1, Some(Tool::Select)), "texts, tool")
        } },
        Row { id: "arcs / a digit mid-draw", live: "opens no value box", gap: false, check: |cx| {
            let (window, view) = sketch_with(cx, "st-arc-digit", Tool::Arc3);
            click(cx, window, &view, (400., 300.));
            pointer(cx, window, &view, (480., 300.));
            keys(cx, window, &["5"]);
            let boxed = cx.update(|cx| view.read(cx).dim_input.is_some() || view.read(cx).sketch.as_ref().is_some_and(|ed| ed.otf_typing));
            expect(boxed, false, "a box open")
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

/// Qa 11-22 (SKETCH-TRIM-SHORT-END, live workflows-1122 22 s12–13): Trim clicked on a 20 px stub beyond a crossing,
/// 7 px from the line's end (in a point's reach): live trims the stub; ours took the used end, which Trim leaves, and
/// did nothing.
#[gpui::test]
fn trim_takes_a_short_end_near_its_end_point(cx: &mut TestAppContext) {
    let (window, view) = sketch_with(cx, "st-trim-stub", Tool::Line);
    for (a, b) in [((400., 200.), (400., 400.)), ((380., 300.), (600., 300.))] {
        click(cx, window, &view, a);
        click(cx, window, &view, b);
        keys(cx, window, &["escape"]);
        cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Line, w, cx))).unwrap();
    }
    let lengths = |cx: &mut TestAppContext| {
        let mut l = cx.update(|cx| line_lengths(&view.read(cx).sketch_data(cx).unwrap()));
        l.sort_by(f64::total_cmp);
        l
    };
    let before = lengths(cx);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Trim, w, cx))).unwrap();
    click(cx, window, &view, (387., 300.));
    let after = lengths(cx);
    assert_eq!(after.len(), 2, "{after:?}");
    // (The view is isometric: the horizontal line is the shorter one in the plane.)
    assert!((after[1] - before[1]).abs() < 1e-9, "the vertical line kept: {before:?} → {after:?}");
    assert!((after[0] / before[0] * 220.0 - 200.0).abs() < 1.0, "the horizontal line 220 → 200 px: {before:?} → {after:?}");
}

/// Qa 11-22 (SKETCH-MIRROR-LINE-LOOK, live workflows-1122 22 s35): after an entity is mirrored, live draws it plain
/// under the still pointer; it lights again once the pointer moves. Ours kept it hovered.
#[gpui::test]
fn a_mirrored_entity_is_plain_until_the_pointer_moves(cx: &mut TestAppContext) {
    let (window, view) = sketch_with(cx, "st-mirror-hold", Tool::Line);
    for (a, b) in [((400., 200.), (400., 400.)), ((450., 250.), (550., 350.))] {
        click(cx, window, &view, a);
        click(cx, window, &view, b);
        keys(cx, window, &["escape"]);
        cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Line, w, cx))).unwrap();
    }
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.activate_tool(Tool::Mirror, w, cx))).unwrap();
    let send = |cx: &mut TestAppContext, at: (f64, f64), press: bool| {
        let at = DVec2::new(at.0, at.1);
        cx.update_window(window, |_, w, cx| {
            view.update(cx, |v, cx| {
                let vp = v.viewport.clone();
                vp.update(cx, |vp, _| vp.place_pointer(at));
                let m = Modifiers::default();
                let mut evs = vec![ViewportEvent::PointerMove { position: at, modifiers: m }, ViewportEvent::Hover(None)];
                if press {
                    evs.extend([ViewportEvent::ToolDown { position: at, modifiers: m, click_count: 1 }, ViewportEvent::ToolUp { position: at, modifiers: m }, ViewportEvent::Hover(None)]);
                }
                for ev in &evs {
                    v.on_viewport_event(&vp, ev, w, cx);
                }
            })
        })
        .unwrap();
        cx.run_until_parked();
    };
    let hovered = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).sketch.as_ref().and_then(|ed| ed.hover));
    send(cx, (400., 300.), true);
    send(cx, (500., 300.), false);
    assert!(hovered(cx).is_some(), "the line to mirror lights under the pointer");
    let before = lines(cx, &view);
    send(cx, (500., 300.), true);
    assert_eq!(lines(cx, &view), before + 1, "mirrored");
    assert_eq!(hovered(cx), None, "plain under the still pointer");
    assert!(cx.update(|cx| view.read(cx).hover.is_none()));
    send(cx, (502., 302.), false);
    assert!(hovered(cx).is_some(), "lit again once the pointer moves");
}

/// Qa SKT-POSTCOMMIT-HOVER (live sketch-tools-1123): the click that commits a 3 point arc or an ellipse leaves the new
/// curve under the still pointer plain, with no snap square (ours lit it orange with the square); it lights once the
/// pointer moves.
#[gpui::test]
fn a_committed_shape_is_plain_under_the_still_pointer(cx: &mut TestAppContext) {
    for (tool, pts) in [(Tool::Arc3, [(400., 300.), (600., 300.), (500., 230.)]), (Tool::Ellipse, [(500., 300.), (600., 300.), (500., 250.)])] {
        let (window, view) = sketch_with(cx, &format!("st-postcommit-{tool:?}"), tool);
        let send = |cx: &mut TestAppContext, at: (f64, f64), press: bool| {
            let at = DVec2::new(at.0, at.1);
            cx.update_window(window, |_, w, cx| {
                view.update(cx, |v, cx| {
                    let vp = v.viewport.clone();
                    vp.update(cx, |vp, _| vp.place_pointer(at));
                    let m = Modifiers::default();
                    let mut evs = vec![ViewportEvent::PointerMove { position: at, modifiers: m }, ViewportEvent::Hover(None)];
                    if press {
                        evs.extend([ViewportEvent::ToolDown { position: at, modifiers: m, click_count: 1 }, ViewportEvent::ToolUp { position: at, modifiers: m }, ViewportEvent::Hover(None)]);
                    }
                    for ev in &evs {
                        v.on_viewport_event(&vp, ev, w, cx);
                    }
                })
            })
            .unwrap();
            cx.run_until_parked();
        };
        let count = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).sketch_data(cx).map_or(0, |d| d.sketch.entities().count()));
        let before = count(cx);
        for p in pts {
            send(cx, p, true);
        }
        let lit = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).sketch.as_ref().map(|ed| (ed.hover.is_some(), ed.inference.is_some())));
        assert!(count(cx) > before, "{tool:?}: committed");
        send(cx, pts[2], false);
        assert_eq!(lit(cx), Some((false, false)), "{tool:?}: plain under the still pointer, no snap");
        assert!(cx.update(|cx| view.read(cx).hover.is_none()), "{tool:?}");
        send(cx, (pts[2].0 + 1.5, pts[2].1 + 1.5), false);
        assert_eq!(lit(cx).map(|l| l.0 || l.1), Some(true), "{tool:?}: lit again once the pointer moves");
    }
}

/// Live workflows-1122 22 s11: Esc after a line's second click ends the chain; the end point under the still pointer
/// stays plain (ours lit it) until the pointer moves.
#[gpui::test]
fn esc_ending_a_chain_leaves_its_end_plain_until_the_pointer_moves(cx: &mut TestAppContext) {
    let (window, view) = sketch_with(cx, "st-esc-hold", Tool::Line);
    click(cx, window, &view, (400., 300.));
    click(cx, window, &view, (550., 300.));
    let hover_at = |cx: &mut TestAppContext, at: (f64, f64)| {
        let at = DVec2::new(at.0, at.1);
        cx.update_window(window, |_, w, cx| {
            view.update(cx, |v, cx| {
                let vp = v.viewport.clone();
                vp.update(cx, |vp, _| vp.place_pointer(at));
                for ev in [ViewportEvent::PointerMove { position: at, modifiers: Modifiers::default() }, ViewportEvent::Hover(None)] {
                    v.on_viewport_event(&vp, &ev, w, cx);
                }
            })
        })
        .unwrap();
        cx.run_until_parked();
    };
    let hovered = |cx: &mut TestAppContext| cx.update(|cx| view.read(cx).sketch.as_ref().and_then(|ed| ed.hover));
    hover_at(cx, (550., 300.));
    keys(cx, window, &["escape"]);
    hover_at(cx, (550., 300.));
    assert_eq!(hovered(cx), None, "plain under the still pointer");
    hover_at(cx, (551., 300.));
    assert!(hovered(cx).is_some(), "lit again once the pointer moves");
}
