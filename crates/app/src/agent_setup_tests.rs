//! Help › Install command line tool… / Connect to coding agents… answer a click at once (P1 11-02: on macOS the
//! click showed nothing): the dialog is open before any check has run, its rows "checking…", and a check that
//! never answers (a fake `claude` that sleeps 30 s on PATH, a hung check) becomes "couldn't check".

use super::{Agent, Arc, CHECK_TIMEOUT, Check, CheckFuture, Checker, ConnectCodingAgents, Duration, Env, Scope, What, file_checks, init, install, open_with};
use gpui_kit::component::WindowExt as _;
use gpui_kit::{AnyWindowHandle, AppContext as _, Bounds, IntoElement, Point, Render, Styled as _, TestAppContext, Window, WindowBounds, WindowOptions, div, px, size};

struct Blank;

impl Render for Blank {
    fn render(&mut self, _: &mut Window, _: &mut gpui_kit::Context<Self>) -> impl IntoElement {
        div().size_full()
    }
}

fn window(cx: &mut TestAppContext) -> AnyWindowHandle {
    cx.update(gpui_kit::init);
    let bounds = Bounds { origin: Point::default(), size: size(px(900.), px(700.)) };
    cx.update(|cx| gpui_kit::open_window(WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() }, cx, |_, cx| cx.new(|_| Blank)).unwrap().0)
}

fn draw(cx: &mut TestAppContext, w: AnyWindowHandle) {
    cx.update_window(w, |_, window, cx| window.draw(cx).clear(cx)).unwrap();
    cx.run_until_parked();
}

/// A home with nothing but a `claude` that hangs (as one waiting on a login or the network would).
fn slow_claude_env(name: &str) -> Env {
    let home = std::env::temp_dir().join(format!("noshape-setup-{name}-{}", std::process::id()));
    let bin = home.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let claude = bin.join(if cfg!(windows) { "claude.cmd" } else { "claude" });
    std::fs::write(&claude, if cfg!(windows) { "@timeout /t 30 >nul\r\n" } else { "#!/bin/sh\nsleep 30\n" }).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&claude, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let os = if cfg!(windows) { install::Os::Windows } else if cfg!(target_os = "macos") { install::Os::Mac } else { install::Os::Linux };
    Env { os, home: home.clone(), path: bin.into_os_string(), appdata: Some(home.join("AppData")), codex_home: Some(home.join(".codex")), system_dirs: Vec::new() }
}

#[gpui::test]
fn the_dialog_opens_at_once_and_its_rows_fill_in(cx: &mut TestAppContext) {
    let w = window(cx);
    let start = std::time::Instant::now();
    let setup = cx.update_window(w, |_, window, cx| open_with(Scope::Agents, slow_claude_env("open"), file_checks(), CHECK_TIMEOUT, window, cx)).unwrap();
    // Open within the click itself, before any frame or background work.
    assert!(cx.update_window(w, |_, window, cx| window.has_active_dialog(cx)).unwrap(), "no dialog after the click");
    assert!(setup.borrow().rows.iter().all(|r| r.status == "checking…"), "rows: {:?}", setup.borrow().rows.iter().map(|r| &r.status).collect::<Vec<_>>());
    assert!(start.elapsed() < Duration::from_secs(1));
    // The checks read files only: the hung `claude` never runs, so the rows fill in without the timeout.
    draw(cx, w);
    assert!(!setup.borrow().checking());
    let s = setup.borrow();
    let claude = s.rows.iter().find(|r| r.what == What::Agent(Agent::ClaudeCode)).unwrap();
    assert_eq!((claude.check, claude.status.as_str()), (Check::Offer, "not connected"));
    let codex = s.rows.iter().find(|r| r.what == What::Agent(Agent::Codex)).unwrap();
    assert_eq!((codex.check, codex.status.as_str()), (Check::Unavailable, "not installed"));
}

#[gpui::test]
fn a_check_that_never_answers_says_so(cx: &mut TestAppContext) {
    let w = window(cx);
    let hung: Checker = Arc::new(|_, _| -> CheckFuture { Box::pin(std::future::pending()) });
    let setup = cx.update_window(w, |_, window, cx| open_with(Scope::Cli, slow_claude_env("hung"), hung, CHECK_TIMEOUT, window, cx)).unwrap();
    assert!(cx.update_window(w, |_, window, cx| window.has_active_dialog(cx)).unwrap());
    draw(cx, w);
    assert!(setup.borrow().checking(), "still checking before the timeout");
    cx.executor().advance_clock(CHECK_TIMEOUT + Duration::from_millis(100));
    draw(cx, w);
    let s = setup.borrow();
    assert_eq!(s.rows[0].check, Check::Unavailable);
    assert_eq!(s.rows[0].status, "couldn't check (no answer in 5 s)");
}

/// The macOS app menu's items are actions: the dialog opens in the window they were dispatched to.
#[gpui::test]
fn the_menu_actions_open_the_dialog(cx: &mut TestAppContext) {
    let w = window(cx);
    cx.update(init);
    cx.update_window(w, |_, window, cx| window.dispatch_action(Box::new(ConnectCodingAgents), cx)).unwrap();
    cx.run_until_parked();
    assert!(cx.update_window(w, |_, window, cx| window.has_active_dialog(cx)).unwrap(), "no dialog after the menu action");
}
