//! Release smoke test (`NOSHAPE_SMOKE=1`, run by .github/workflows/perf.yml
//! on every release): the app walks through the paths that touch platform
//! code most, then quits with status 0. A crash, an abort or a hang (the
//! watchdog) fails the job. v0.4.3 aborted on macOS 27 the first time a
//! Move cursor appeared (a private AppKit selector); this shows every cursor.
//!
//! Steps: every tab of the document and a new one of every kind; a sketch
//! of the first Part Studio with the Dimension tool started; every GPUI
//! cursor style and every app cursor through the platform; the main dialogs
//! and panels opened and closed; the right-click menus; an orbit; quit. Each step logs a
//! `SMOKE <step>` line to stderr, so a failure shows where it happened.

use crate::actions;
use crate::cursor::AppCursor;
use crate::workspace::Workspace;
use gpui_kit::*;
use noshape_model::document::TabContent;
use std::time::Duration;

/// Whole run limit: past it the app exits with status 3.
const WATCHDOG: Duration = Duration::from_secs(300);

pub fn enabled() -> bool {
    std::env::var_os("NOSHAPE_SMOKE").is_some()
}

/// Every GPUI cursor style (the match keeps the list complete).
fn cursor_styles() -> Vec<CursorStyle> {
    use CursorStyle as C;
    let all = vec![
        C::Arrow,
        C::IBeam,
        C::Crosshair,
        C::ClosedHand,
        C::OpenHand,
        C::PointingHand,
        C::ResizeLeft,
        C::ResizeRight,
        C::ResizeLeftRight,
        C::ResizeUp,
        C::ResizeDown,
        C::ResizeUpDown,
        C::ResizeUpLeftDownRight,
        C::ResizeUpRightDownLeft,
        C::ResizeColumn,
        C::ResizeRow,
        C::IBeamCursorForVerticalLayout,
        C::OperationNotAllowed,
        C::DragLink,
        C::DragCopy,
        C::ContextualMenu,
        C::Move,
        C::Wait,
    ];
    for c in &all {
        match c {
            C::Arrow | C::IBeam | C::Crosshair | C::ClosedHand | C::OpenHand | C::PointingHand | C::ResizeLeft | C::ResizeRight | C::ResizeLeftRight | C::ResizeUp | C::ResizeDown | C::ResizeUpDown => {}
            C::ResizeUpLeftDownRight | C::ResizeUpRightDownLeft | C::ResizeColumn | C::ResizeRow | C::IBeamCursorForVerticalLayout | C::OperationNotAllowed | C::DragLink | C::DragCopy | C::ContextualMenu | C::Move | C::Wait => {}
        }
    }
    all
}

/// Every app cursor (crate::cursor; the match keeps the list complete).
fn app_cursors() -> Vec<AppCursor> {
    let mut all: Vec<AppCursor> = (1..=6).map(AppCursor::Count).collect();
    all.extend([AppCursor::Crosshair, AppCursor::CrosshairBox, AppCursor::Confirmation, AppCursor::RemoveSelection, AppCursor::SnapMate, AppCursor::Move, AppCursor::ZoomIn, AppCursor::NoDrop]);
    for c in &all {
        match c {
            AppCursor::Count(_) | AppCursor::Crosshair | AppCursor::CrosshairBox | AppCursor::Confirmation | AppCursor::RemoveSelection | AppCursor::SnapMate | AppCursor::Move | AppCursor::ZoomIn | AppCursor::NoDrop => {}
        }
    }
    all
}

fn fail(msg: &str) -> ! {
    eprintln!("SMOKE FAIL: {msg}");
    std::process::exit(4);
}

/// Run the smoke test in `window` (the document window of `ws`).
pub fn start(window: &mut Window, cx: &mut Context<Workspace>) {
    std::thread::spawn(|| {
        std::thread::sleep(WATCHDOG);
        eprintln!("SMOKE FAIL: not done after {}s", WATCHDOG.as_secs());
        std::process::exit(3);
    });
    window.activate_window();
    cx.activate(true);
    cx.spawn_in(window, async move |ws, cx| {
        let wait = async |cx: &mut AsyncWindowContext, ms: u64| cx.background_executor().timer(Duration::from_millis(ms)).await;
        let step = |name: &str| eprintln!("SMOKE {name}");
        wait(cx, 3000).await;

        step("tabs");
        let tabs = ws.update(cx, |w, cx| w.doc.read(cx).session.doc.ordered_tabs().iter().map(|t| t.id).collect::<Vec<_>>()).unwrap_or_else(|_| fail("no workspace"));
        for t in tabs {
            ws.update_in(cx, |w, window, cx| w.activate(t, window, cx)).unwrap_or_else(|_| fail("activate"));
            wait(cx, 1500).await;
        }
        for kind in ["partstudio", "assembly", "drawing", "variablestudio", "featurestudio", "materiallibrary"] {
            step(&format!("new {kind}"));
            ws.update_in(cx, |w, window, cx| w.add_tab(kind, window, cx)).unwrap_or_else(|_| fail("add tab"));
            wait(cx, 2000).await;
        }

        step("sketch");
        let sketch = ws
            .update(cx, |w, cx| {
                w.doc.read(cx).session.doc.ordered_tabs().iter().find_map(|t| match &t.content {
                    TabContent::PartStudio(ps) => ps.features.iter().find(|f| f.type_id == "sketch").map(|f| (t.id, f.id)),
                    _ => None,
                })
            })
            .unwrap_or_else(|_| fail("no workspace"));
        if let Some((tab, feature)) = sketch {
            ws.update_in(cx, |w, window, cx| w.activate(tab, window, cx)).unwrap_or_else(|_| fail("activate"));
            wait(cx, 2000).await;
            ws.update_in(cx, |w, window, cx| {
                if let Some(ps) = w.active_part_studio() {
                    ps.update(cx, |ps, cx| ps.edit_feature(feature, window, cx));
                }
            })
            .unwrap_or_else(|_| fail("edit sketch"));
            wait(cx, 2000).await;
            step("dimension");
            let _ = cx.update(|window, cx| window.dispatch_action(Box::new(actions::SketchDimension), cx));
            wait(cx, 1000).await;
        } else {
            step("no sketch in the document");
        }

        step("cursors");
        for style in cursor_styles() {
            let _ = cx.update(|window, cx| {
                cx.set_platform_cursor_style(style);
                window.refresh();
            });
            wait(cx, 60).await;
        }
        for cursor in app_cursors() {
            let _ = cx.update(|window, cx| crate::cursor::set(window, Some(cursor), cx));
            wait(cx, 60).await;
        }
        let _ = cx.update(|window, cx| {
            crate::cursor::set(window, None, cx);
            cx.set_platform_cursor_style(CursorStyle::Arrow);
        });
        for _ in 0..3 {
            let _ = cx.update(|window, cx| window.dispatch_action(Box::new(actions::Cancel), cx));
            wait(cx, 500).await;
        }

        step("dialogs");
        let open_close: [(&str, Box<dyn Action>, Box<dyn Action>); 6] = [
            ("extrude", Box::new(actions::Extrude), Box::new(actions::Cancel)),
            ("section view", Box::new(actions::SectionView), Box::new(actions::Cancel)),
            ("shortcuts", Box::new(actions::ToggleShortcutHelp), Box::new(actions::ToggleShortcutHelp)),
            ("tab manager", Box::new(actions::TabManager), Box::new(actions::TabManager)),
            ("version graph", Box::new(actions::ToggleVersionGraph), Box::new(actions::ToggleVersionGraph)),
            ("tool search", Box::new(actions::SearchTools), Box::new(actions::Cancel)),
        ];
        for (name, open, close) in open_close {
            step(name);
            let _ = cx.update(|window, cx| window.dispatch_action(open, cx));
            wait(cx, 1000).await;
            let _ = cx.update(|window, cx| window.dispatch_action(close, cx));
            wait(cx, 700).await;
        }

        // Right-click menus (crate::os_menu) of the viewport and the
        // features list, through the window's own event dispatch.
        let size = cx.update(|window, _| window.viewport_size()).unwrap_or_else(|_| fail("no window"));
        for (name, at) in [("viewport menu", point(size.width * 0.6, size.height * 0.5)), ("features list menu", point(px(120.), px(200.)))] {
            step(name);
            let _ = cx.update(|window, cx| {
                let modifiers = Modifiers::default();
                window.dispatch_event(PlatformInput::MouseMove(MouseMoveEvent { position: at, pressed_button: None, modifiers }), cx);
                window.dispatch_event(PlatformInput::MouseDown(MouseDownEvent { button: MouseButton::Right, position: at, modifiers, click_count: 1, first_mouse: false }), cx);
                window.dispatch_event(PlatformInput::MouseUp(MouseUpEvent { button: MouseButton::Right, position: at, modifiers, click_count: 1 }), cx);
            });
            // The viewport's menu opens when its GPU pick is back.
            let mut open = false;
            for _ in 0..24 {
                wait(cx, 250).await;
                open = cx.update(|_, cx| crate::os_menu::is_open(cx)).unwrap_or(false);
                if open {
                    break;
                }
            }
            step(if open { "menu open" } else { "no menu here" });
            let _ = cx.update(|window, cx| window.dispatch_action(Box::new(crate::os_menu::MenuDismiss), cx));
            wait(cx, 500).await;
        }

        step("orbit");
        for k in 0..12 {
            let a: Box<dyn Action> = if k % 3 == 2 { Box::new(actions::RotateUp) } else { Box::new(actions::RotateLeft) };
            let _ = cx.update(|window, cx| window.dispatch_action(a, cx));
            wait(cx, 300).await;
        }
        for a in [Box::new(actions::ToggleProjection) as Box<dyn Action>, Box::new(actions::ZoomToFit), Box::new(actions::ToggleProjection)] {
            let _ = cx.update(|window, cx| window.dispatch_action(a, cx));
            wait(cx, 700).await;
        }

        step("done");
        let _ = cx.update(|_, cx| cx.quit());
    })
    .detach();
}
