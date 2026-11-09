//! Setting noshape up on this computer (`noshape_ops::install`): the `noshape`
//! command on PATH and noshape's MCP server in the coding agents installed.
//! Help › Install command line tool… and Connect to coding agents…, the
//! macOS app menu's same two items, and a one-time prompt on first launch.
//! A click shows the dialog at once (its rows "checking…"), never silence
//! while background work runs.

use crate::os_menu::{OsMenu, OsMenuItem};
use crate::os_ui as os;
use crate::theme::p;
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_ops::install::{self, Agent, Env, State};
use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

actions!(noshape_setup, [InstallCommandLineTool, ConnectCodingAgents]);

/// What the dialog offers.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Scope {
    Cli,
    Agents,
    /// Both, on first launch.
    Everything,
}

/// The Help menu's two setup rows (after live's rows: help_menu.rs).
pub fn help_items(menu: OsMenu) -> OsMenu {
    menu.item(OsMenuItem::new("Install command line tool…").on_click(|_, window, cx| open(Scope::Cli, window, cx)))
        .item(OsMenuItem::new("Connect to coding agents…").on_click(|_, window, cx| open(Scope::Agents, window, cx)))
}

/// The two items as actions (the macOS app menu), in the active window.
pub fn init(cx: &mut App) {
    cx.on_action(|_: &InstallCommandLineTool, cx| in_some_window(Scope::Cli, cx));
    cx.on_action(|_: &ConnectCodingAgents, cx| in_some_window(Scope::Agents, cx));
    // macOS: the app menu (noshape ▸), where Mac apps put these.
    if cfg!(target_os = "macos") {
        cx.set_menus(vec![Menu::new("Noshape").items([
            MenuItem::action("About Noshape", crate::about::ShowAbout),
            MenuItem::separator(),
            MenuItem::action("Install Command Line Tool…", InstallCommandLineTool),
            MenuItem::action("Connect to Coding Agents…", ConnectCodingAgents),
            MenuItem::separator(),
            MenuItem::action("Quit Noshape", crate::actions::Quit),
        ])]);
    }
}

/// Menu actions arrive while the window they were dispatched to is busy: the dialog opens right after (an update of
/// that window from inside its own dispatch fails, and the click showed nothing).
fn in_some_window(scope: Scope, cx: &mut App) {
    cx.defer(move |cx| {
        let mut w = cx.active_window().or_else(|| cx.windows().first().copied());
        if w.is_none() {
            crate::files::show_start_window(cx);
            w = cx.active_window().or_else(|| cx.windows().first().copied());
        }
        let Some(w) = w else {
            log::error!("setup: no window to show the dialog in");
            return;
        };
        if let Err(e) = w.update(cx, |_, window, cx| open(scope, window, cx)) {
            log::error!("setup: could not open the dialog: {e:#}");
        }
    });
}

/// How long a check of what is set up may take before its row says it couldn't (a hung network home, a slow disk).
const CHECK_TIMEOUT: Duration = Duration::from_secs(5);
/// Installing or connecting: macOS's password prompt waits for the user, an agent's command for its login.
const APPLY_TIMEOUT: Duration = Duration::from_secs(300);

/// What a row sets up.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum What {
    Cli,
    Agent(Agent),
}

/// A row's state.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Check {
    /// Being checked, or being set up.
    Working,
    /// Can be set up: a checkbox.
    Offer,
    /// Set up already, or just now.
    Done,
    /// Nothing to do: not installed, or the check couldn't tell.
    Unavailable,
    /// Setting it up failed (the row says why).
    Failed,
}

struct Row {
    what: What,
    label: String,
    status: String,
    check: Check,
    on: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Phase {
    Choosing,
    Working,
    Finished,
}

/// The dialog's state, shared by its rows' checks and its renders.
struct Setup {
    scope: Scope,
    rows: Vec<Row>,
    phase: Phase,
}

impl Setup {
    /// Every row "checking…" (the dialog shows at once; the checks fill them in).
    fn new(scope: Scope) -> Setup {
        let mut whats = Vec::new();
        if scope != Scope::Agents {
            whats.push(What::Cli);
        }
        if scope != Scope::Cli {
            whats.extend(Agent::ALL.into_iter().filter(|a| *a != Agent::ClaudeDesktop || !cfg!(target_os = "linux")).map(What::Agent));
        }
        let rows = whats.into_iter().map(|what| Row { what, label: label(what), status: "checking…".into(), check: Check::Working, on: false }).collect();
        Setup { scope, rows, phase: Phase::Choosing }
    }

    /// A check's answer for row `i` (None: no answer in time).
    fn checked(&mut self, i: usize, r: Option<Result<(Check, String), String>>) {
        let Some(row) = self.rows.get_mut(i) else { return };
        let (check, status) = match r {
            None => (Check::Unavailable, format!("couldn't check (no answer in {} s)", CHECK_TIMEOUT.as_secs())),
            Some(Err(e)) => (Check::Unavailable, format!("couldn't check: {e}")),
            Some(Ok(v)) => v,
        };
        row.on = check == Check::Offer;
        row.check = check;
        row.status = status;
    }

    fn checking(&self) -> bool {
        self.rows.iter().any(|r| r.check == Check::Working)
    }
}

fn label(what: What) -> String {
    match what {
        What::Cli => "Install the noshape command".into(),
        What::Agent(a) => format!("Connect {}", a.name()),
    }
}

/// What is set up for `what` now (files and folders only: no agent's command runs).
fn check(what: What, env: &Env) -> Result<(Check, String), String> {
    let command = install::this_binary().ok_or("couldn't tell where this program is")?;
    Ok(match what {
        What::Cli => {
            let st = install::cli_status(env, &command);
            match (&st.installed, st.on_path) {
                (Some(p), true) => (Check::Done, format!("installed: {}", p.display())),
                (Some(p), false) => (Check::Offer, format!("installed: {} (not on PATH)", p.display())),
                (None, _) => (Check::Offer, match &st.other {
                    Some(o) => format!("not installed ({} starts another Noshape)", o.display()),
                    None => "not installed".into(),
                }),
            }
        }
        What::Agent(a) => {
            let st = install::status(env, a, &command);
            match st.state {
                State::Missing => (Check::Unavailable, "not installed".into()),
                State::Connected => (Check::Done, st.state.label()),
                _ => (Check::Offer, st.state.label()),
            }
        }
    })
}

/// Set `what` up; what happened, or why it failed.
fn set_up(what: What, env: &Env) -> Result<String, String> {
    let command = install::this_binary().ok_or("couldn't tell where this program is")?;
    match what {
        What::Cli => install::cli_install(env, &command, None).map(|v| {
            let path = v["path"].as_str().unwrap_or("").to_string();
            match v["note"].as_str() {
                Some(n) => format!("installed: {path} ({n})"),
                None => format!("installed: {path}"),
            }
        }),
        What::Agent(a) => install::connect(env, a, &command).map(|ch| format!("{}{}", ch.result, if a == Agent::ClaudeDesktop { " (restart Claude Desktop to load it)" } else { "" })),
    }
}

/// `work` on the background executor: its result, or None after `limit` (left to finish on its own).
async fn timed<T: Send + 'static>(ex: BackgroundExecutor, work: impl Future<Output = T> + Send + 'static, limit: Duration) -> Option<T> {
    ex.spawn(work).with_timeout(limit, &ex).await.ok()
}

type CheckFuture = Pin<Box<dyn Future<Output = Result<(Check, String), String>> + Send>>;
type Checker = Arc<dyn Fn(What, Env) -> CheckFuture + Send + Sync>;

fn file_checks() -> Checker {
    Arc::new(|what, env| Box::pin(async move { check(what, &env) }))
}

/// Once, on the first launch of an installed (release) app: offer what isn't
/// set up yet, if anything is (checked first: this isn't a click).
pub fn first_launch(cx: &mut App) {
    // Once per install, ever (not per update); Help › Connect to coding agents… shows the same status any time.
    let flag = noshape_app_core::paths::config_dir().join("setup-offered");
    let installed = option_env!("NOSHAPE_VERSION").is_some() && !cfg!(debug_assertions);
    let quiet = ["NOSHAPE_NO_SETUP_PROMPT", "NOSHAPE_NO_NATIVE_DIALOGS", "NOSHAPE_SMOKE"].iter().any(|k| std::env::var_os(k).is_some());
    if !installed || quiet || flag.exists() {
        return;
    }
    let _ = std::fs::write(&flag, "");
    cx.spawn(async move |cx| {
        let mut setup = Setup::new(Scope::Everything);
        let env = Env::current();
        for i in 0..setup.rows.len() {
            let (what, env) = (setup.rows[i].what, env.clone());
            let r = timed(cx.background_executor().clone(), async move { check(what, &env) }, CHECK_TIMEOUT).await;
            setup.checked(i, r);
        }
        // Shown either way, so the user sees what is set up: with nothing to offer it is only the status and Close.
        if !setup.rows.iter().any(|r| r.check == Check::Offer) {
            setup.phase = Phase::Finished;
        }
        cx.update(|cx| {
            let Some(w) = cx.active_window().or_else(|| cx.windows().first().copied()) else { return };
            let _ = w.update(cx, |_, window, cx| show(Rc::new(RefCell::new(setup)), Env::current(), window, cx));
        });
    })
    .detach();
}

/// The dialog, at once, with its rows "checking…"; each check fills its row in (or says it couldn't).
fn open(scope: Scope, window: &mut Window, cx: &mut App) {
    open_with(scope, Env::current(), file_checks(), CHECK_TIMEOUT, window, cx);
}

fn open_with(scope: Scope, env: Env, checker: Checker, limit: Duration, window: &mut Window, cx: &mut App) -> Rc<RefCell<Setup>> {
    let setup = Rc::new(RefCell::new(Setup::new(scope)));
    show(setup.clone(), env.clone(), window, cx);
    let whats: Vec<What> = setup.borrow().rows.iter().map(|r| r.what).collect();
    for (i, what) in whats.into_iter().enumerate() {
        let (env, checker, setup) = (env.clone(), checker.clone(), setup.clone());
        window
            .spawn(cx, async move |cx| {
                let r = timed(cx.background_executor().clone(), checker(what, env), limit).await;
                let _ = cx.update(|window, _| {
                    setup.borrow_mut().checked(i, r);
                    window.refresh();
                });
            })
            .detach();
    }
    setup
}

fn show(setup: Rc<RefCell<Setup>>, env: Env, window: &mut Window, cx: &mut App) {
    window.open_dialog(cx, move |dialog, _, cx| {
        let pal = p();
        let s = setup.borrow();
        let scope = s.scope;
        let title = match scope {
            Scope::Cli => "Install command line tool",
            Scope::Agents => "Connect to coding agents",
            Scope::Everything => "Set up Noshape",
        };
        let intro = match scope {
            Scope::Cli => "The noshape command runs every operation of the app from a terminal, and its MCP server (noshape mcp) for coding agents.",
            Scope::Agents => "Connected agents can use Noshape through its MCP server (noshape mcp): ask them to model a part, edit a drawing or export files.",
            Scope::Everything => "Noshape can be used from a terminal and by coding agents through its MCP server (noshape mcp).",
        };
        let choosing = s.phase == Phase::Choosing;
        let any = s.rows.iter().any(|r| r.on && r.check == Check::Offer);
        let none_found = scope != Scope::Cli && !s.checking() && s.rows.iter().filter(|r| matches!(r.what, What::Agent(_))).all(|r| r.check == Check::Unavailable);
        let list = v_flex().gap(px(6.)).children(s.rows.iter().enumerate().map(|(i, r)| {
            let toggles = choosing && r.check == Check::Offer;
            let shown_on = r.check == Check::Done || (r.on && r.check == Check::Offer);
            let setup = setup.clone();
            h_flex()
                .id(("setup-row", i))
                .min_h(px(26.))
                .gap(px(8.))
                .when(toggles, |d| d.cursor_pointer())
                .child(div().when(!toggles, |d| d.opacity(0.5)).child(os::checkbox_box(shown_on)))
                .child(div().flex_none().font_weight(FontWeight::SEMIBOLD).child(r.label.clone()))
                .child(div().min_w_0().text_color(if r.check == Check::Failed { pal.error } else { pal.text_muted }).child(r.status.clone()))
                .on_click(move |_, window, _| {
                    if let Some(r) = setup.borrow_mut().rows.get_mut(i).filter(|_| toggles) {
                        r.on = !r.on;
                    }
                    window.refresh();
                })
        }));
        let ok_label = match s.phase {
            Phase::Choosing if scope == Scope::Everything => "Set up",
            Phase::Choosing => "Apply",
            Phase::Working => "Working…",
            Phase::Finished => "Close",
        };
        let ok_pale = (choosing && !any) || s.phase == Phase::Working;
        let cancel = (s.phase != Phase::Finished).then(|| os::modal_cancel(os::cancel_button("cancel", if !choosing { "Close" } else if scope == Scope::Everything { "Not now" } else { "Cancel" }, cx)));
        drop(s);
        let (setup2, env) = (setup.clone(), env.clone());
        os::modal(dialog, title, 600.)
            .child(
                v_flex()
                    .px(px(11.))
                    .pt(px(4.))
                    .pb(px(16.))
                    .gap(px(12.))
                    .text_size(px(14.))
                    .text_color(pal.text)
                    .child(div().child(intro))
                    .child(list)
                    .when(none_found, |d| d.child(div().text_color(pal.text_muted).child("No coding agent found (Claude Code, Codex CLI, Claude Desktop). Install one, then use Help › Connect to coding agents…"))),
            )
            .footer(os::modal_footer().child(os::modal_ok(div().when(ok_pale, |d| d.opacity(0.4)).child(os::primary_button("ok", ok_label)))).children(cancel))
            .on_ok(move |_, window, cx| {
                let phase = setup2.borrow().phase;
                match phase {
                    Phase::Finished => true,
                    Phase::Working => false,
                    Phase::Choosing => {
                        apply(setup2.clone(), env.clone(), window, cx);
                        false
                    }
                }
            })
    });
}

/// Set up what was chosen, one after the other (in the background: the agents' commands, macOS's password prompt),
/// each row saying "working…" and then what happened: the path installed to, or the exact error.
fn apply(setup: Rc<RefCell<Setup>>, env: Env, window: &mut Window, cx: &mut App) {
    let chosen: Vec<(usize, What)> = {
        let mut s = setup.borrow_mut();
        let chosen: Vec<(usize, What)> = s.rows.iter().enumerate().filter(|(_, r)| r.on && r.check == Check::Offer).map(|(i, r)| (i, r.what)).collect();
        if chosen.is_empty() {
            return;
        }
        s.phase = Phase::Working;
        for (i, what) in &chosen {
            s.rows[*i].check = Check::Working;
            s.rows[*i].status = if *what == What::Cli { "installing…".into() } else { "connecting…".into() };
        }
        chosen
    };
    window.refresh();
    window
        .spawn(cx, async move |cx| {
            for (i, what) in chosen {
                let env = env.clone();
                let r = timed(cx.background_executor().clone(), async move { set_up(what, &env) }, APPLY_TIMEOUT).await;
                let (check, status) = match r {
                    Some(Ok(text)) => (Check::Done, text),
                    Some(Err(e)) => (Check::Failed, format!("failed: {e}")),
                    None => (Check::Failed, format!("no answer in {} min", APPLY_TIMEOUT.as_secs() / 60)),
                };
                if check == Check::Failed {
                    log::warn!("setup: {}: {status}", label(what));
                }
                let setup = setup.clone();
                let _ = cx.update(move |window, _| {
                    let mut s = setup.borrow_mut();
                    s.rows[i].check = check;
                    s.rows[i].status = status;
                    window.refresh();
                });
            }
            let _ = cx.update(move |window, _| {
                setup.borrow_mut().phase = Phase::Finished;
                window.refresh();
            });
        })
        .detach();
}

#[cfg(test)]
#[path = "agent_setup_tests.rs"]
mod tests;
