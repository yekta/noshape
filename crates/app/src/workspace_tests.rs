//! The workspace with the mouse and keys (live2 manual-4): the Ctrl+Space quick tab switcher (Ctrl held, a hovered
//! thumbnail is selected, a mouse-up on a thumbnail switches to it, a mouse-up off the card closes it without
//! switching); shortcuts with the focus on the top bar.

use crate::doc_model::DocModel;
use crate::workspace::Workspace;
use gpui_kit::{AnyWindowHandle, AppContext as _, Bounds, Entity, Modifiers, MouseButton, Point, TestAppContext, VisualTestContext, WindowBounds, WindowOptions, px, size};
use noshape_model::document::TabId;
use noshape_ops::{Ctx, registry};
use serde_json::{Value, json};

fn call(ctx: &mut Ctx, tool: &str, args: Value) -> Value {
    let op = registry::by_tool_name(tool).unwrap_or_else(|| panic!("no tool {tool}"));
    registry::run(ctx, op, &args).unwrap_or_else(|e| panic!("{tool}: {e:?}")).value
}

/// A window of its own for a dialog (the app's windows carry the dialogs' root; the test workspace's doesn't).
fn dialog_window(cx: &mut TestAppContext) -> AnyWindowHandle {
    struct Blank;
    impl gpui_kit::Render for Blank {
        fn render(&mut self, _: &mut gpui_kit::Window, _: &mut gpui_kit::Context<Self>) -> impl gpui_kit::IntoElement {
            gpui_kit::div()
        }
    }
    let bounds = Bounds { origin: Point::default(), size: size(px(900.), px(700.)) };
    cx.update(|cx| gpui_kit::open_window(WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() }, cx, |_, cx| cx.new(|_| Blank)).unwrap().0)
}

/// The app's hooks, keys and a headless GPU, as main sets them up.
fn app(cx: &mut TestAppContext) -> crate::gpu::Gpu {
    cx.update(gpui_kit::init);
    cx.update(|cx| cx.bind_keys(crate::actions::default_key_bindings()));
    static HOOKS: std::sync::Once = std::sync::Once::new();
    HOOKS.call_once(|| {
        crate::register_part_studio_hooks();
        crate::register_assembly_hooks();
        crate::register_variable_hooks();
        crate::register_preferences_hooks();
        crate::register_drawing_hooks();
    });
    let gpu = crate::gpu::Gpu::headless().unwrap();
    cx.update(|cx| cx.set_global(crate::gpu::GpuGlobal(gpu.clone())));
    gpu
}

/// A new document (its Part Studio 1 with a block) and Variable Studios A, B and C in a workspace, opened in that order (C active,
/// B the previous one).
fn workspace(cx: &mut TestAppContext, name: &str) -> (AnyWindowHandle, Entity<Workspace>, [TabId; 4]) {
    let gpu = app(cx);
    let dir = std::env::temp_dir().join(format!("noshape-workspace-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("tabs.noshape");
    for ext in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{ext}", path.display()));
    }
    {
        let mut ctx = Ctx::new();
        call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
        // A block in Part Studio 1 (its tab card has a picture to render).
        call(&mut ctx, "sketch_create", json!({ "plane": "plane:top" }));
        call(&mut ctx, "sketch_draw", json!({ "sketch": "Sketch 1", "tool": "corner_rectangle", "points": [[0, 0], [20, 10]] }));
        let region = call(&mut ctx, "sketch_get", json!({ "sketch": "Sketch 1" }))["regions"][0]["id"].as_str().unwrap().to_string();
        call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [region], "depth": "5 mm" } }));
        for name in ["A", "B", "C"] {
            call(&mut ctx, "tab_create", json!({ "kind": "variablestudio", "name": name }));
        }
    }
    let session = crate::session::Session::open(&path).unwrap();
    let doc = cx.new(|_| DocModel::new(session));
    let ids = doc.read_with(cx, |d, _| {
        let tabs = d.session.doc.ordered_tabs();
        let named = |n: &str| tabs.iter().find(|t| t.name == n).map(|t| t.id).unwrap_or_else(|| panic!("no tab {n}"));
        [tabs[0].id, named("A"), named("B"), named("C")]
    });
    let bounds = Bounds { origin: Point::default(), size: size(px(1400.), px(900.)) };
    let mut ws = None;
    let w: AnyWindowHandle = cx
        .update(|cx| {
            cx.open_window(WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() }, |window, cx| {
                let e = cx.new(|cx| Workspace::new(doc, gpu, window, cx));
                ws = Some(e.clone());
                e
            })
        })
        .unwrap()
        .into();
    let ws = ws.unwrap();
    for t in ids {
        cx.update_window(w, |_, window, cx| ws.update(cx, |ws, cx| ws.activate(t, window, cx))).unwrap();
        draw(cx, w);
    }
    (w, ws, ids)
}

fn draw(cx: &mut TestAppContext, w: AnyWindowHandle) {
    cx.update_window(w, |_, window, cx| window.draw(cx).clear(cx)).unwrap();
    cx.run_until_parked();
}

#[gpui::test]
fn the_tab_switcher_takes_a_hovered_thumbnail_and_closes_on_a_mouse_up_off_it(cx: &mut TestAppContext) {
    let (w, ws, [_, a, b, c]) = workspace(cx, "switcher");
    let ctrl = Modifiers { control: true, ..Default::default() };
    let open = |cx: &mut TestAppContext| {
        ws.update(cx, |ws, cx| ws.switcher_step(cx));
        draw(cx, w);
    };
    open(cx);
    let list = ws.read_with(cx, |ws, _| ws.switcher.clone()).expect("open");
    assert_eq!((list.0[list.1], ws.read_with(cx, |ws, _| ws.active)), (b, Some(c)), "the previous tab selected");
    let far = list.0.iter().position(|t| *t == a).expect("A listed");
    let mut vcx = VisualTestContext::from_window(w, cx);
    let thumb = vcx.debug_bounds(Box::leak(format!("tab-switch-{far}").into_boxed_str())).expect("A's thumbnail").center();
    vcx.simulate_mouse_move(thumb, None, ctrl);
    draw(cx, w);
    assert_eq!(ws.read_with(cx, |ws, _| ws.switcher.as_ref().map(|s| s.0[s.1])), Some(a), "hovered with Ctrl held: selected");
    // A mouse-up off the card: closed, no switch.
    let mut vcx = VisualTestContext::from_window(w, cx);
    vcx.simulate_mouse_down(Point::new(px(700.), px(200.)), MouseButton::Left, ctrl);
    vcx.simulate_mouse_up(Point::new(px(700.), px(200.)), MouseButton::Left, ctrl);
    draw(cx, w);
    assert_eq!(ws.read_with(cx, |ws, _| (ws.switcher.is_some(), ws.active)), (false, Some(c)), "closed, still on C");
    // A mouse-up on a thumbnail: that tab, at once.
    open(cx);
    let mut vcx = VisualTestContext::from_window(w, cx);
    let thumb = vcx.debug_bounds(Box::leak(format!("tab-switch-{far}").into_boxed_str())).expect("A's thumbnail").center();
    vcx.simulate_mouse_move(thumb, None, ctrl);
    vcx.simulate_mouse_down(thumb, MouseButton::Left, ctrl);
    vcx.simulate_mouse_up(thumb, MouseButton::Left, ctrl);
    draw(cx, w);
    assert_eq!(ws.read_with(cx, |ws, _| (ws.switcher.is_some(), ws.active)), (false, Some(a)), "switched to A");
}

/// Live2 manual-4: with the focus on a top bar control (Tab past a dialog), a shortcut still works: Shift+E opens
/// Extrude.
#[gpui::test]
fn a_shortcut_works_with_the_focus_on_the_top_bar(cx: &mut TestAppContext) {
    let (w, ws, [studio, ..]) = workspace(cx, "top-bar-keys");
    cx.update_window(w, |_, window, cx| ws.update(cx, |ws, cx| ws.activate(studio, window, cx))).unwrap();
    let ps = cx.update(|cx| ws.read(cx).active_studio()).expect("the Part Studio");
    let settle = |cx: &mut TestAppContext, done: &dyn Fn(&mut TestAppContext) -> bool| {
        for _ in 0..1500 {
            if done(cx) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
            cx.executor().advance_clock(std::time::Duration::from_millis(50));
            draw(cx, w);
        }
    };
    settle(cx, &|cx| !cx.update(|cx| ps.read(cx).ui_busy()));
    cx.update_window(w, |_, window, cx| {
        let logo = ws.read(cx).logo_focus.clone();
        window.focus(&logo, cx);
    })
    .unwrap();
    draw(cx, w);
    let focused = cx.update_window(w, |_, window, cx| ws.read(cx).logo_focus.is_focused(window)).unwrap();
    assert!(focused, "the logo has the keyboard");
    cx.update_window(w, |_, window, cx| window.dispatch_keystroke(gpui_kit::Keystroke::parse("shift-e").unwrap(), cx)).unwrap();
    let dialog = |cx: &mut TestAppContext| cx.update_window(w, |_, window, cx| ps.read(cx).ui_state(window, cx)["dialog"].clone()).unwrap();
    settle(cx, &|cx| !dialog(cx).is_null());
    let d = dialog(cx);
    assert!(d["title"].as_str().is_some_and(|t| t.starts_with("Extrude")), "Extrude's dialog: {d}");
}

/// qa DOCK-TAB-SWITCH-TRAILING-FRAMES (since 0.4.54): the pointer rests on an inactive Part Studio tab until its hover
/// card shows (which starts the tab's picture), then clicks it. The card goes with the click, but the picture finishing
/// ~0.5 s later redrew the window: frames until ~700 ms after every return to an open tab; and its render, already on
/// the GPU, held the clicked tab's first frame back (~0.6 s on a software GPU). The render now waits `PICTURE_WAIT`
/// and a click on the tab in that time drops it; a card left showing still gets its picture drawn.
#[gpui::test]
fn a_clicked_tabs_card_picture_redraws_nothing(cx: &mut TestAppContext) {
    let (w, ws, [studio, ..]) = workspace(cx, "tab-card");
    // The picture's frame comes back (handed over on this thread).
    let _lock = noshape_ops::doc::gpu_lock();
    let gpu = match crate::gpu::Gpu::headless_rendering() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("tab card picture test skipped (no GPU): {e}"),
    };
    cx.update(|cx| cx.set_global(crate::gpu::GpuGlobal(gpu.clone())));
    let thumbs = ws.read_with(cx, |ws, _| ws.tab_thumbs.clone());
    let notified = std::rc::Rc::new(std::cell::Cell::new(0));
    let n = notified.clone();
    let _watch = cx.update(|cx| cx.observe(&thumbs, move |_, _| n.set(n.get() + 1)));
    let state = |cx: &mut TestAppContext| thumbs.read_with(cx, |t, _| t.state());
    // What the card asks for when it shows; made on the GPU's own thread.
    let ask = |cx: &mut TestAppContext| {
        thumbs.update(cx, |t, cx| {
            t.forget();
            t.request(studio, cx);
        });
        state(cx).0
    };
    let made = |cx: &mut TestAppContext| {
        let t = std::time::Instant::now();
        while state(cx).0 && t.elapsed() < std::time::Duration::from_secs(60) {
            std::thread::sleep(std::time::Duration::from_millis(10));
            gpu.deliver_frames();
            cx.executor().advance_clock(std::time::Duration::from_millis(50));
            draw(cx, w);
        }
        draw(cx, w);
    };
    // A card left showing: its picture goes to the GPU and redraws (asked again until the studio's part is there).
    let t = std::time::Instant::now();
    let mut asked = false;
    while state(cx).2 == 0 && t.elapsed() < std::time::Duration::from_secs(60) {
        asked = ask(cx);
        made(cx);
    }
    let shown = (asked, notified.replace(0) > 0, state(cx).1, state(cx).2);
    // The tab clicked (activated) while its picture is made: no redraw, and its render never goes to the GPU (the
    // clicked tab's own frame waited ~0.6 s behind it on a software GPU, qa's dock).
    let asked = ask(cx);
    cx.update_window(w, |_, window, cx| ws.update(cx, |ws, cx| ws.activate(studio, window, cx))).unwrap();
    made(cx);
    let clicked = (asked, notified.get() > 0, state(cx).1, state(cx).2);
    assert_eq!((shown, clicked), ((true, true, 1, 1), (true, false, 0, 1)), "((asked, redrawn, kept, GPU renders) for a card left showing, the same for one whose tab was clicked)");
}

/// Live 11-18 (behaviours "Preferences › Units effects"): the Versions and history flyout opened in one document is open
/// on entering another; closed, the next one opens without it.
#[gpui::test]
fn the_versions_flyouts_open_state_carries_over_to_the_next_document(cx: &mut TestAppContext) {
    let (w1, ws1, _) = workspace(cx, "hist-carry-1");
    assert!(!cx.update(|cx| ws1.read(cx).history_open()), "closed at first");
    cx.update_window(w1, |_, window, cx| ws1.update(cx, |ws, cx| ws.toggle_history(window, cx))).unwrap();
    draw(cx, w1);
    let (w2, ws2, _) = workspace(cx, "hist-carry-2");
    assert!(cx.update(|cx| ws2.read(cx).history_open()), "open in the next document");
    cx.update_window(w2, |_, window, cx| ws2.update(cx, |ws, cx| ws.toggle_history(window, cx))).unwrap();
    draw(cx, w2);
    let (_, ws3, _) = workspace(cx, "hist-carry-3");
    assert!(!cx.update(|cx| ws3.read(cx).history_open()), "closed there, closed in the one after");
}

/// Import (live import-export-1118): the files are translated off the UI thread (the document unchanged until then);
/// the Part Studio goes right after the active tab, the view stays, the file is kept in CAD Imports, and the bubble
/// reads "<file> has been imported." with "Open document"; a file that doesn't translate is kept too, its bubble
/// "<file> uploaded but failed to translate." with the same link, red (live rr-import-1118: alert-danger).
#[gpui::test]
fn an_import_translates_off_the_ui_thread_and_bubbles_as_live(cx: &mut TestAppContext) {
    let (w, ws, [_, _, _, c]) = workspace(cx, "import");
    let dir = std::env::temp_dir().join(format!("noshape-workspace-import-files-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let good = dir.join("block.step");
    {
        let mut ctx = Ctx::new();
        call(&mut ctx, "document_create", json!({ "path": dir.join("src.noshape").to_str().unwrap() }));
        call(&mut ctx, "sketch_create", json!({ "plane": "plane:top" }));
        call(&mut ctx, "sketch_draw", json!({ "sketch": "Sketch 1", "tool": "corner_rectangle", "points": [[0, 0], [20, 10]] }));
        let region = call(&mut ctx, "sketch_get", json!({ "sketch": "Sketch 1" }))["regions"][0]["id"].clone();
        call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [region], "depth": "5 mm" } }));
        call(&mut ctx, "export_parts", json!({ "format": "STEP", "file": good.to_str().unwrap() }));
    }
    let broken = dir.join("broken.step");
    std::fs::write(&broken, "not a STEP file").unwrap();
    let doc = cx.update(|cx| ws.read(cx).doc.clone());
    let tabs = |cx: &mut TestAppContext| doc.read_with(cx, |d, _| d.session.doc.ordered_tabs().iter().map(|t| t.name.clone()).collect::<Vec<_>>());
    let before = tabs(cx);
    cx.update(|cx| crate::import_dialog::import(&doc, std::slice::from_ref(&good), Default::default(), Some(c), true, cx));
    assert_eq!(tabs(cx), before, "nothing added before the file is read");
    cx.run_until_parked();
    draw(cx, w);
    // Live import-undo-1123: the kept file in its own history entry first, the translated tab in a second.
    let newest = doc.read_with(cx, |d, _| d.session.store.history(d.session.branch).unwrap().iter().take(2).map(|m| m.description.clone()).collect::<Vec<_>>());
    assert_eq!(newest, ["Import document", "Insert tab(s) : block.step"]);
    let after = tabs(cx);
    let at = after.iter().position(|n| n == "C").unwrap();
    assert_eq!(after.get(at + 1).map(String::as_str), Some("block"), "the Part Studio right after the active tab: {after:?}");
    assert_eq!(cx.update(|cx| ws.read(cx).active), Some(c), "the view stays");
    let kept = |cx: &mut TestAppContext, name: &str| {
        doc.read_with(cx, |d, _| {
            let folder = d.session.doc.settings.folders.iter().find(|f| f.name == "CAD Imports").map(|f| f.id);
            d.session.doc.tabs.iter().any(|t| t.name == name && folder.is_some() && d.session.doc.parent_of(t.id) == folder)
        })
    };
    assert!(kept(cx, "block.step"), "the file kept in CAD Imports");
    assert!(doc.read_with(cx, |d, _| d.notice_link("block.step has been imported.").is_some_and(|l| l.label == "Open document")));
    // Live import-toast-1122: the green ✓ bubble, gone after 5 s.
    let look = doc.read_with(cx, |d, _| d.notice_link("block.step has been imported.").map(|l| (l.kind, l.stays)));
    assert_eq!(look, Some((Some(crate::os_ui::BubbleKind::Success), std::time::Duration::from_secs(5))));
    // Two files in one pick (live import-into-doc-1123): each translated and added on its own, right after the active
    // tab (the later one nearest it), each with its bubble.
    let plate = dir.join("plate.step");
    std::fs::copy(&good, &plate).unwrap();
    cx.update(|cx| crate::import_dialog::import(&doc, &[good.clone(), plate.clone()], Default::default(), Some(c), true, cx));
    cx.run_until_parked();
    let newest = doc.read_with(cx, |d, _| d.session.store.history(d.session.branch).unwrap().iter().take(3).map(|m| m.description.clone()).collect::<Vec<_>>());
    assert_eq!(newest, ["Import document", "Import document", "Insert tab(s) : block.step, plate.step"]);
    let after = tabs(cx);
    let at = after.iter().position(|n| n == "C").unwrap();
    assert_eq!(after[at + 1..at + 4], ["plate", "block (1)", "block"], "{after:?}");
    assert!(doc.read_with(cx, |d, _| d.notice_link("plate.step has been imported.").is_some()));
    // Live dx/toast-stack-1125: one toast at a time; plate's waits for block's and takes its place when it ends (its ×).
    assert_eq!(cx.update(|cx| ws.read(cx).waiting_toasts().iter().map(|m| m.to_string()).collect::<Vec<_>>()), ["plate.step has been imported."]);
    assert!(doc.read_with(cx, |d, _| d.notice_link("block.step has been imported.").is_some_and(|l| l.kind == Some(crate::os_ui::BubbleKind::Success))));
    draw(cx, w);
    let mut vcx = VisualTestContext::from_window(w, cx);
    let top = vcx.debug_bounds("toast-top").expect("block's bubble");
    assert_eq!(cx.update(|cx| ws.read(cx).shown_toast().map(str::to_string)).as_deref(), Some("block.step has been imported."));
    let mut vcx = VisualTestContext::from_window(w, cx);
    vcx.simulate_click(Point { x: top.right() - px(11.), y: top.center().y }, Modifiers::default());
    draw(cx, w);
    let mut vcx = VisualTestContext::from_window(w, cx);
    let now = vcx.debug_bounds("toast-top").expect("plate's bubble in its place");
    assert_eq!((now.top(), cx.update(|cx| ws.read(cx).shown_toast().map(str::to_string)).as_deref()), (top.top(), Some("plate.step has been imported.")));
    assert!(cx.update(|cx| ws.read(cx).waiting_toasts().is_empty()));
    cx.update(|cx| crate::import_dialog::import(&doc, std::slice::from_ref(&broken), Default::default(), Some(c), true, cx));
    cx.run_until_parked();
    assert!(kept(cx, "broken.step"), "a file that didn't translate is kept too");
    assert!(doc.read_with(cx, |d, _| d.notice_link("broken.step uploaded but failed to translate.").is_some_and(|l| l.label == "Open document")));
    // It waits for plate's bubble to end (live dx/toast-stack-1125), then comes up in its place.
    assert_eq!(cx.update(|cx| ws.read(cx).waiting_toasts().iter().map(|m| m.to_string()).collect::<Vec<_>>()), ["broken.step uploaded but failed to translate."]);
    cx.executor().advance_clock(std::time::Duration::from_secs(60));
    cx.run_until_parked();
    draw(cx, w);
    let red = cx.update(|cx| ws.read(cx).toast().is_some_and(|(k, m)| *k == crate::ui::AlertKind::Error && m == "broken.step uploaded but failed to translate."));
    assert!(red, "live's red bubble (rr-import-1118), not the info one");
    let _ = std::fs::remove_dir_all(&dir);
}

/// qa SPEED-FILE-TAB-IMPORT-MENU (live's menu closes at 58 ms): a STEP file tab's Import… opens the dialog at once and
/// writes the import's temporary copy off the UI thread; Import waits for it and adds the Part Studio.
#[gpui::test]
fn a_file_tabs_import_opens_its_dialog_before_the_file_is_written(cx: &mut TestAppContext) {
    let (w, ws, _) = workspace(cx, "file-tab-import");
    let dir = std::env::temp_dir().join(format!("noshape-workspace-file-tab-import-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let step = dir.join("block.step");
    {
        let mut ctx = Ctx::new();
        call(&mut ctx, "document_create", json!({ "path": dir.join("src.noshape").to_str().unwrap() }));
        call(&mut ctx, "sketch_create", json!({ "plane": "plane:top" }));
        call(&mut ctx, "sketch_draw", json!({ "sketch": "Sketch 1", "tool": "corner_rectangle", "points": [[0, 0], [20, 10]] }));
        let region = call(&mut ctx, "sketch_get", json!({ "sketch": "Sketch 1" }))["regions"][0]["id"].clone();
        call(&mut ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [region], "depth": "5 mm" } }));
        call(&mut ctx, "export_parts", json!({ "format": "STEP", "file": step.to_str().unwrap() }));
    }
    let doc = cx.update(|cx| ws.read(cx).doc.clone());
    let bytes = std::fs::read(&step).unwrap();
    let file = doc.update(cx, |d, _| d.session.doc.add_tab("block.step", noshape_model::TabContent::File(noshape_model::file_tab::FileTab::new("block.step", &bytes, 0))));
    let copy = std::env::temp_dir().join(format!("noshape-translate-{}-{file}", std::process::id())).join("block.step");
    let _ = std::fs::remove_file(&copy);
    let d = dialog_window(cx);
    cx.update_window(d, |_, window, cx| ws.update(cx, |ws, cx| ws.translate_file(file, window, cx))).unwrap();
    let open = |cx: &mut TestAppContext| cx.update_window(d, |_, window, cx| gpui_kit::component::WindowExt::has_active_dialog(window, cx)).unwrap();
    assert!(open(cx), "the dialog at once");
    assert!(!copy.exists(), "the copy is written off the UI thread");
    draw(cx, d);
    VisualTestContext::from_window(d, cx).simulate_keystrokes("enter");
    cx.run_until_parked();
    draw(cx, w);
    assert!(!open(cx), "Import closed the dialog");
    let tabs = doc.read_with(cx, |d, _| d.session.doc.ordered_tabs().iter().map(|t| t.name.clone()).collect::<Vec<_>>());
    assert!(tabs.iter().any(|n| n == "block"), "the Part Studio: {tabs:?}");
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(copy.parent().unwrap());
}

/// Live upload-only-1118: a bare assembly file (.sldasm) gets the Import dialog's info box with ☐ Upload only; Import
/// (Enter) does nothing until Upload only is ticked, then keeps the file as it is, as a file tab right after the active
/// tab (not in CAD Imports, nothing translated), the view staying, with "probe.sldasm has been imported." and its link.
#[gpui::test]
fn a_bare_assembly_is_uploaded_only_once_upload_only_is_ticked(cx: &mut TestAppContext) {
    let (w, ws, [_, _, _, c]) = workspace(cx, "upload-only");
    let dir = std::env::temp_dir().join(format!("noshape-workspace-upload-only-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let probe = dir.join("probe.sldasm");
    std::fs::write(&probe, b"not translated").unwrap();
    let doc = cx.update(|cx| ws.read(cx).doc.clone());
    let tabs = |cx: &mut TestAppContext| doc.read_with(cx, |d, _| d.session.doc.ordered_tabs().iter().map(|t| t.name.clone()).collect::<Vec<_>>());
    let before = tabs(cx);
    let d = dialog_window(cx);
    let open = |cx: &mut TestAppContext| cx.update_window(d, |_, window, cx| gpui_kit::component::WindowExt::has_active_dialog(window, cx)).unwrap();
    cx.update_window(d, |_, window, cx| crate::import_dialog::open(doc.clone(), vec![probe.clone()], Some(c), true, window, cx)).unwrap();
    draw(cx, d);
    VisualTestContext::from_window(d, cx).simulate_keystrokes("enter");
    draw(cx, d);
    assert!(open(cx), "Import is disabled until Upload only is ticked");
    assert_eq!(tabs(cx), before);
    let mut vcx = VisualTestContext::from_window(d, cx);
    let upload = vcx.debug_bounds("imp-upload").expect("the Upload only box").center();
    vcx.simulate_click(upload, Modifiers::default());
    draw(cx, d);
    VisualTestContext::from_window(d, cx).simulate_keystrokes("enter");
    cx.run_until_parked();
    draw(cx, d);
    draw(cx, w);
    assert!(!open(cx), "Enter imports once Upload only is ticked");
    let after = tabs(cx);
    let at = after.iter().position(|n| n == "C").unwrap();
    assert_eq!(after.get(at + 1).map(String::as_str), Some("probe.sldasm"), "the file right after the active tab: {after:?}");
    assert_eq!(after.len(), before.len() + 1, "nothing else: no Part Studio, no CAD Imports copy: {after:?}");
    assert_eq!(cx.update(|cx| ws.read(cx).active), Some(c), "the view stays");
    assert!(doc.read_with(cx, |d, _| d.notice_link("probe.sldasm has been imported.").is_some_and(|l| l.label == "Open document")));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Live2 doc-level-1117 / ours-1118: a tab switch takes the old element away at once and shows the new tab's list in
/// the same frame (no loading state between): Part Studio 1's feature list in the first frame after it is activated,
/// gone in the first frame after a Variable Studio is.
#[gpui::test]
fn a_tab_switch_shows_the_new_tab_in_its_first_frame(cx: &mut TestAppContext) {
    let (w, ws, [studio, a, ..]) = workspace(cx, "tab-switch-frame");
    let list = |cx: &mut TestAppContext| gpui_kit::VisualTestContext::from_window(w, cx).debug_bounds("feature-list").is_some();
    let one_frame = |cx: &mut TestAppContext, t: TabId| {
        cx.update_window(w, |_, window, cx| {
            ws.update(cx, |ws, cx| ws.activate(t, window, cx));
            window.draw(cx).clear(cx);
        })
        .unwrap();
    };
    one_frame(cx, studio);
    assert!(list(cx), "the Part Studio's feature list in the first frame");
    one_frame(cx, a);
    assert!(!list(cx), "gone in the first frame after the Variable Studio is activated");
    cx.run_until_parked();
}

/// qa session-1118: Create Drawing of a part with No views starts Insert view with the first view's scale fitted to the
/// sheet (live: a 200 mm Tray on A3 at 1:2); it was placed at the sheet's 1:1.
#[gpui::test]
fn create_drawing_with_no_views_fits_the_first_views_scale(cx: &mut TestAppContext) {
    use noshape_model::drawing::{Projection, SheetSize, TitleBlockStyle, templates::DrawingTemplate};
    let (w, ws, [studio, ..]) = workspace(cx, "no-views-scale");
    let part = ws.read_with(cx, |ws, cx| {
        let doc = &ws.doc.read(cx).session.doc;
        let Some(noshape_model::TabContent::PartStudio(ps)) = doc.tab(studio).map(|t| &t.content) else { panic!("the Part Studio") };
        let f = ps.features.iter().find(|f| f.type_id == "extrude").expect("the extrude");
        noshape_model::BodyId { feature: f.id, index: 0 }
    });
    let template = DrawingTemplate::standard("ISO A3", SheetSize::A3, TitleBlockStyle::Iso, false, Projection::FirstAngle);
    let source = noshape_model::drawing::create::DrawingSource { tab: studio, part: Some(part), config: Default::default(), sketch: None, flat: false };
    cx.update_window(w, |_, window, cx| ws.update(cx, |ws, cx| ws.create_drawing_tab("Part 1 Drawing 1".into(), (template, None), (Some(source), None), false, window, cx))).unwrap();
    let mut scale = None;
    for _ in 0..600 {
        draw(cx, w);
        scale = ws.read_with(cx, |ws, cx| {
            let tab = ws.doc.read(cx).session.doc.tabs.iter().find(|t| matches!(t.content, noshape_model::TabContent::Drawing(_)))?.id;
            ws.drawing_editor(tab)?.read(cx).insert.scale
        });
        if scale.is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        cx.executor().advance_clock(std::time::Duration::from_millis(50));
    }
    let want = noshape_model::drawing::create::first_part_view_scale(SheetSize::A3, [20.0, 5.0]);
    assert_eq!(scale, Some(want), "the first view's scale fitted, not the sheet's 1:1");
}

fn tab_bounds(cx: &mut TestAppContext, w: AnyWindowHandle, t: TabId) -> Bounds<gpui_kit::Pixels> {
    VisualTestContext::from_window(w, cx).debug_bounds(Box::leak(format!("doc-tab-{t}").into_boxed_str())).unwrap_or_else(|| panic!("tab {t} not drawn"))
}

/// Live2 feel/documents.json `tabBar.dragStartPx` and bm 6562: a tab pressed and moved 5 px is still a click; from
/// 6 px it drags, leaving a gap its own width in the strip and a 100 px ghost under the pointer.
#[gpui::test]
fn a_tab_drags_from_6_px_leaving_a_gap_its_width_and_a_100_px_ghost(cx: &mut TestAppContext) {
    let (w, ws, [_, a, ..]) = workspace(cx, "tab-drag");
    let tab = tab_bounds(cx, w, a);
    let at = tab.center();
    let none = Modifiers::none();
    let mut vcx = VisualTestContext::from_window(w, cx);
    vcx.simulate_mouse_move(at, None, none);
    vcx.simulate_mouse_down(at, MouseButton::Left, none);
    vcx.simulate_mouse_move(at + gpui_kit::point(px(5.), px(0.)), Some(MouseButton::Left), none);
    draw(cx, w);
    assert!(!cx.update(|cx| cx.has_active_drag()), "5 px: still a click");
    let mut vcx = VisualTestContext::from_window(w, cx);
    vcx.simulate_mouse_move(at + gpui_kit::point(px(6.), px(0.)), Some(MouseButton::Left), none);
    draw(cx, w);
    assert!(cx.update(|cx| cx.has_active_drag()), "6 px: a drag");
    assert_eq!(ws.read_with(cx, |ws, _| ws.tab_drag_and_rename().0), Some(a), "the strip shows A's gap");
    let mut vcx = VisualTestContext::from_window(w, cx);
    let (gap, ghost) = (vcx.debug_bounds("tab-gap").expect("the gap"), vcx.debug_bounds("tab-ghost").expect("the ghost"));
    assert!((f32::from(gap.size.width) - f32::from(tab.size.width)).abs() < 0.5, "the gap is the tab's width: {gap:?} {tab:?}");
    assert_eq!(ghost.size.width, px(100.), "a 100 px ghost");
    assert!(ghost.left() <= at.x + px(6.) && at.x + px(6.) <= ghost.right(), "under the pointer: {ghost:?}");
    vcx.simulate_mouse_up(at + gpui_kit::point(px(6.), px(0.)), MouseButton::Left, none);
    draw(cx, w);
}

/// Live document/1227 (behaviours 6586 qa5 tab management): a tab dragged over a folder tab's middle lights the
/// folder (its drop blue under a faded ghost); near the folder's end it reorders instead, nothing lit.
#[gpui::test]
fn a_tab_dragged_over_a_folders_middle_lights_it(cx: &mut TestAppContext) {
    let (w, ws, [_, a, ..]) = workspace(cx, "tab-over-folder");
    let folder = ws.update(cx, |ws, cx| {
        ws.doc.update(cx, |d, cx| {
            let mut id = None;
            d.edit_document("New folder", cx, |doc| id = Some(doc.add_folder("F", None)));
            id.expect("the folder")
        })
    });
    draw(cx, w);
    let f = tab_bounds(cx, w, folder);
    let at = tab_bounds(cx, w, a).center();
    let none = Modifiers::none();
    let mut vcx = VisualTestContext::from_window(w, cx);
    vcx.simulate_mouse_move(at, None, none);
    vcx.simulate_mouse_down(at, MouseButton::Left, none);
    vcx.simulate_mouse_move(at + gpui_kit::point(px(10.), px(0.)), Some(MouseButton::Left), none);
    draw(cx, w);
    assert!(cx.update(|cx| cx.has_active_drag()), "dragging A");
    let into = |cx: &mut TestAppContext| ws.read_with(cx, |ws, _| ws.tab_drop_folder());
    VisualTestContext::from_window(w, cx).simulate_mouse_move(f.center(), Some(MouseButton::Left), none);
    draw(cx, w);
    assert_eq!(into(cx), Some(folder), "over the folder's middle: it lights");
    let end = gpui_kit::point(f.right() - px(3.), f.center().y);
    VisualTestContext::from_window(w, cx).simulate_mouse_move(end, Some(MouseButton::Left), none);
    draw(cx, w);
    assert_eq!(into(cx), None, "near its end: a reorder, the folder not lit");
    VisualTestContext::from_window(w, cx).simulate_mouse_up(end, MouseButton::Left, none);
    draw(cx, w);
}

/// Live2 11-13 and 2279 (b): a double-click on a tab activates it and does nothing more (no rename: Rename is in its
/// menu).
#[gpui::test]
fn a_double_click_on_a_tab_only_activates_it(cx: &mut TestAppContext) {
    let (w, ws, [_, a, ..]) = workspace(cx, "tab-dblclick");
    let at = tab_bounds(cx, w, a).center();
    let mut vcx = VisualTestContext::from_window(w, cx);
    vcx.simulate_event(gpui_kit::MouseDownEvent { button: MouseButton::Left, position: at, modifiers: Modifiers::none(), click_count: 1, first_mouse: false });
    vcx.simulate_event(gpui_kit::MouseUpEvent { button: MouseButton::Left, position: at, modifiers: Modifiers::none(), click_count: 1 });
    vcx.simulate_event(gpui_kit::MouseDownEvent { button: MouseButton::Left, position: at, modifiers: Modifiers::none(), click_count: 2, first_mouse: false });
    vcx.simulate_event(gpui_kit::MouseUpEvent { button: MouseButton::Left, position: at, modifiers: Modifiers::none(), click_count: 2 });
    draw(cx, w);
    assert_eq!(ws.read_with(cx, |ws, _| (ws.active, ws.tab_drag_and_rename().1)), (Some(a), None), "A active, not renaming");
}

/// Live fv-1121 (3): Tab › Rename's field; Esc closes it and keeps the name, typed over or not.
#[gpui::test]
fn esc_in_a_tab_rename_keeps_the_name(cx: &mut TestAppContext) {
    let (w, ws, [_, a, ..]) = workspace(cx, "tab-rename-esc");
    let name = |cx: &mut TestAppContext| ws.read_with(cx, |ws, cx| ws.doc.read(cx).session.doc.tab(a).map(|t| t.name.clone()));
    let before = name(cx);
    for typed in ["", "x"] {
        cx.update_window(w, |_, window, cx| ws.update(cx, |ws, cx| ws.start_tab_rename(a, window, cx))).unwrap();
        draw(cx, w);
        let mut vcx = VisualTestContext::from_window(w, cx);
        if !typed.is_empty() {
            vcx.simulate_input(typed);
        }
        vcx.simulate_keystrokes("escape");
        draw(cx, w);
        assert_eq!(ws.read_with(cx, |ws, _| ws.tab_drag_and_rename().1), None, "{typed:?}: the field closed");
        assert_eq!(name(cx), before, "{typed:?}: the name kept");
    }
}

/// Live ws-1118 r0: ☰ › Restore deleted workspaces… lists a deleted workspace under Name | Deleted by | Deleted at, the
/// time it was deleted kept.
#[gpui::test]
fn restore_deleted_workspaces_lists_who_and_when(cx: &mut TestAppContext) {
    let (_, ws, _) = workspace(cx, "restore-ws");
    let w = dialog_window(cx);
    let doc = ws.read_with(cx, |ws, _| ws.doc.clone());
    let deleted = doc.update(cx, |d, _| {
        let store = &mut d.session.store;
        let main = store.list_branches().unwrap()[0].clone();
        let b = store.create_branch(main.head, "B2").unwrap();
        store.delete_branch(b).unwrap();
        store.branch(b).unwrap()
    });
    assert!(deleted.deleted_at.is_some());
    cx.update_window(w, |_, window, cx| crate::doc_dialogs::restore_deleted_workspaces(doc.clone(), window, cx)).unwrap();
    draw(cx, w);
    assert!(VisualTestContext::from_window(w, cx).debug_bounds("restore-ws-head").is_some(), "the table's header");
}

/// Live rq 2403: Ctrl+click or Shift+click on a tab opens it in a new window; this window keeps its tab.
#[gpui::test]
fn ctrl_or_shift_click_on_a_tab_opens_it_in_a_new_window(cx: &mut TestAppContext) {
    let (w, ws, [_, a, _, c]) = workspace(cx, "tab-new-window");
    let at = tab_bounds(cx, w, a).center();
    for (n, m) in [(2, Modifiers::secondary_key()), (3, Modifiers::shift())] {
        let mut vcx = VisualTestContext::from_window(w, cx);
        vcx.simulate_click(at, m);
        draw(cx, w);
        assert_eq!(cx.update(|cx| cx.windows().len()), n, "{m:?}: a new window");
        assert_eq!(ws.read_with(cx, |ws, _| ws.active), Some(c), "{m:?}: this window stays on C");
    }
}

/// Import… of files that aren't CAD reads them off the UI thread: nothing changes in the pick's own frame, then each
/// becomes a file tab right after the active one with the last one's toast (live doc-plus-import-png); a file that
/// can't be read says so instead of being dropped silently.
#[gpui::test]
fn imported_plain_files_are_read_in_the_background_and_a_missing_one_says_so(cx: &mut TestAppContext) {
    let (w, ws, [_, _, _, c]) = workspace(cx, "import-plain");
    let dir = std::env::temp_dir().join(format!("noshape-workspace-import-plain-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let png = dir.join("photo.png");
    std::fs::write(&png, b"\x89PNG\r\n\x1a\n").unwrap();
    let missing = dir.join("gone.pdf");
    let doc = cx.update(|cx| ws.read(cx).doc.clone());
    let tabs = |cx: &mut TestAppContext| doc.read_with(cx, |d, _| d.session.doc.ordered_tabs().iter().map(|t| t.name.clone()).collect::<Vec<_>>());
    let before = tabs(cx);
    let errors = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let seen = errors.clone();
    let _sub = cx.update(|cx| cx.subscribe(&doc, move |_, e: &crate::doc_model::DocEvent, _| {
        if let crate::doc_model::DocEvent::Error(m) = e {
            seen.borrow_mut().push(m.clone());
        }
    }));
    cx.update(|cx| crate::workspace::import_picked(doc.clone(), Some(c), w, vec![missing.clone(), png.clone()], cx));
    assert_eq!(tabs(cx), before, "the files are read off the UI thread");
    cx.run_until_parked();
    let after = tabs(cx);
    let at = after.iter().position(|n| n == "C").unwrap();
    assert_eq!(after.get(at + 1).map(String::as_str), Some("photo.png"), "{after:?}");
    assert_eq!(after.len(), before.len() + 1, "{after:?}");
    assert!(doc.read_with(cx, |d, _| d.notice_link("photo.png has been imported.").is_some_and(|l| l.label == "Open document")));
    let errors = errors.borrow();
    assert!(errors.len() == 1 && errors[0].starts_with("Import failed: gone.pdf could not be read"), "{errors:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// qa (flow 91, ui_script.rs "not in a Part Studio…"): a document in a window of its own, as the documents page
/// opens one (an app window whose content becomes the document's workspace), takes a page scenario's graphics targets:
/// its Part Studio's origin.
#[gpui::test]
fn a_page_step_finds_the_graphics_of_a_document_the_page_opened(cx: &mut TestAppContext) {
    let gpu = app(cx);
    let dir = std::env::temp_dir().join(format!("noshape-workspace-page-open-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("page-open.noshape");
    for ext in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{ext}", path.display()));
    }
    call(&mut Ctx::new(), "document_create", json!({ "path": path.to_str().unwrap() }));
    let doc = cx.new(|_| DocModel::new(crate::session::Session::open(&path).unwrap()));
    let bounds = Bounds { origin: Point::default(), size: size(px(1400.), px(900.)) };
    let options = WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() };
    let w = cx.update(|cx| crate::os_menu::open_window(options, cx, |window, cx| cx.new(|cx| Workspace::new(doc, gpu, window, cx)))).unwrap().0;
    let mut acx = cx.to_async();
    let ws = crate::ui_script::doc_workspace(w, &mut acx).expect("the window's document");
    let tab = cx.update(|cx| ws.read(cx).doc.read(cx).session.doc.ordered_tabs()[0].id);
    cx.update_window(w, |_, window, cx| ws.update(cx, |ws, cx| ws.activate(tab, window, cx))).unwrap();
    for _ in 0..12000 {
        draw(cx, w);
        if cx.update(|cx| ws.read(cx).active_part_studio().is_some_and(|ps| !ps.read(cx).ui_busy())) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let at = crate::ui_script::page_locate(&json!({ "origin": true }), w, &mut acx);
    assert!(at.as_ref().is_ok_and(|p| p.x > px(0.) && p.y > px(0.) && p.x < bounds.size.width && p.y < bounds.size.height), "the origin in the window: {at:?}");
    cx.update_window(w, |_, window, _| window.remove_window()).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}
