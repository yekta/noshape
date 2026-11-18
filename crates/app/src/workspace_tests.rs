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

/// A new document (its Part Studio 1) and Variable Studios A, B and C in a workspace, opened in that order (C active,
/// B the previous one).
fn workspace(cx: &mut TestAppContext, name: &str) -> (AnyWindowHandle, Entity<Workspace>, [TabId; 4]) {
    cx.update(gpui_kit::init);
    cx.update(|cx| cx.bind_keys(crate::actions::default_key_bindings()));
    static HOOKS: std::sync::Once = std::sync::Once::new();
    HOOKS.call_once(|| {
        crate::register_part_studio_hooks();
        crate::register_assembly_hooks();
        crate::register_variable_hooks();
        crate::register_preferences_hooks();
    });
    let gpu = crate::gpu::Gpu::headless().unwrap();
    cx.update(|cx| cx.set_global(crate::gpu::GpuGlobal(gpu.clone())));
    let dir = std::env::temp_dir().join(format!("noshape-workspace-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("tabs.noshape");
    for ext in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{ext}", path.display()));
    }
    {
        let mut ctx = Ctx::new();
        call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
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
/// ~0.5 s later redrew the window: frames until ~700 ms after every return to an open tab. Activating the tab now keeps
/// that picture quietly; a card left showing still gets its picture drawn.
#[gpui::test]
fn a_clicked_tabs_card_picture_redraws_nothing(cx: &mut TestAppContext) {
    let (w, ws, [studio, ..]) = workspace(cx, "tab-card");
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
            draw(cx, w);
        }
        draw(cx, w);
    };
    // A card left showing: its picture redraws.
    let asked = ask(cx);
    made(cx);
    let shown = (asked, notified.replace(0), state(cx).1);
    // The tab clicked (activated) while its picture is made.
    let asked = ask(cx);
    cx.update_window(w, |_, window, cx| ws.update(cx, |ws, cx| ws.activate(studio, window, cx))).unwrap();
    made(cx);
    let clicked = (asked, notified.get(), state(cx).1);
    assert_eq!((shown, clicked), ((true, 1, 1), (true, 0, 1)), "((asked, redraws, kept) for a card left showing, the same for one whose tab was clicked)");
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
