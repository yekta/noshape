//! Setting noshape up on this computer (`noshape_ops::install`): the `noshape`
//! command on PATH and noshape's MCP server in the coding agents installed.
//! Help › Install command line tool… and Connect to coding agents…, the
//! macOS app menu's same two items, and a one-time prompt on first launch.

use crate::os_menu::{OsMenu, OsMenuItem};
use crate::os_ui as os;
use crate::theme::p;
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_ops::install::{self, Agent, AgentStatus, CliStatus, Env, State};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

actions!(noshape_setup, [InstallCommandLineTool, ConnectCodingAgents]);

/// What the dialog offers.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Scope {
    Cli,
    Agents,
    /// Both, on first launch.
    Everything,
}

/// The Help menu's items.
pub fn help_items(menu: OsMenu) -> OsMenu {
    menu.item(OsMenuItem::new("Install command line tool…").on_click(|_, window, cx| open(Scope::Cli, window, cx)))
        .item(OsMenuItem::new("Connect to coding agents…").on_click(|_, window, cx| open(Scope::Agents, window, cx)))
}

/// The two items as actions (the macOS app menu), in the active window.
pub fn init(cx: &mut App) {
    cx.on_action(|_: &InstallCommandLineTool, cx| in_active_window(Scope::Cli, cx));
    cx.on_action(|_: &ConnectCodingAgents, cx| in_active_window(Scope::Agents, cx));
    // macOS: the app menu (noshape ▸), where Mac apps put these.
    if cfg!(target_os = "macos") {
        cx.set_menus(vec![Menu::new("noshape").items([
            MenuItem::action("Install Command Line Tool…", InstallCommandLineTool),
            MenuItem::action("Connect to Coding Agents…", ConnectCodingAgents),
            MenuItem::separator(),
            MenuItem::action("Quit noshape", crate::actions::Quit),
        ])]);
    }
}

fn in_active_window(scope: Scope, cx: &mut App) {
    if cx.active_window().is_none() {
        crate::files::show_start_window(cx);
    }
    let Some(w) = cx.active_window().or_else(|| cx.windows().first().copied()) else { return };
    let _ = w.update(cx, |_, window, cx| open(scope, window, cx));
}

/// Once, on the first launch of an installed (release) app: offer what isn't
/// set up yet, if anything is.
pub fn first_launch(cx: &mut App) {
    let flag = noshape_app_core::paths::config_dir().join("setup-offered");
    let installed = option_env!("NOSHAPE_VERSION").is_some() && !cfg!(debug_assertions);
    let quiet = ["NOSHAPE_NO_SETUP_PROMPT", "NOSHAPE_NO_NATIVE_DIALOGS", "NOSHAPE_SMOKE"].iter().any(|k| std::env::var_os(k).is_some());
    if !installed || quiet || flag.exists() {
        return;
    }
    let _ = std::fs::write(&flag, "");
    let task = cx.background_executor().spawn(async move { gather() });
    cx.spawn(async move |cx| {
        let Some(found) = task.await else { return };
        let worth = found.cli.installed.is_none() || found.agents.iter().any(|a| matches!(a.state, State::NotConnected | State::Elsewhere(_)));
        if !worth {
            return;
        }
        cx.update(|cx| {
            let Some(w) = cx.active_window().or_else(|| cx.windows().first().copied()) else { return };
            let _ = w.update(cx, |_, window, cx| show(Scope::Everything, found, window, cx));
        });
    })
    .detach();
}

/// What is set up now (files and folders only: no agent's command runs).
#[derive(Clone)]
struct Found {
    command: PathBuf,
    cli: CliStatus,
    agents: Vec<AgentStatus>,
}

fn gather() -> Option<Found> {
    let command = install::this_binary()?;
    let env = Env::current();
    Some(Found { cli: install::cli_status(&env, &command), agents: install::statuses(&env, &command).into_iter().filter(|a| a.found.is_some()).collect(), command })
}

fn open(scope: Scope, window: &mut Window, cx: &mut App) {
    let task = cx.background_executor().spawn(async move { gather() });
    window
        .spawn(cx, async move |cx| {
            let Some(found) = task.await else { return };
            let _ = cx.update(|window, cx| show(scope, found, window, cx));
        })
        .detach();
}

/// One row: a checkbox, what it does, its current state.
struct Row {
    what: Option<Agent>,
    label: String,
    state: String,
    on: bool,
    /// Already done: shown checked, not a choice.
    done: bool,
}

fn rows(scope: Scope, found: &Found) -> Vec<Row> {
    let mut v = Vec::new();
    if scope != Scope::Agents {
        let (state, done) = match (&found.cli.installed, found.cli.on_path) {
            (Some(p), true) => (format!("installed: {}", p.display()), true),
            (Some(p), false) => (format!("installed: {} (not on PATH)", p.display()), false),
            (None, _) => (match &found.cli.other {
                Some(o) => format!("not installed ({} starts another noshape)", o.display()),
                None => "not installed".into(),
            }, false),
        };
        v.push(Row { what: None, label: "Install the noshape command".into(), state, on: !done, done });
    }
    if scope != Scope::Cli {
        for a in &found.agents {
            let done = a.state == State::Connected;
            v.push(Row { what: Some(a.agent), label: format!("Connect {}", a.agent.name()), state: a.state.label(), on: !done, done });
        }
    }
    v
}

fn show(scope: Scope, found: Found, window: &mut Window, cx: &mut App) {
    let rows = Rc::new(RefCell::new(rows(scope, &found)));
    let none_found = scope != Scope::Cli && found.agents.is_empty();
    let command = found.command.clone();
    window.open_dialog(cx, move |dialog, _, cx| {
        let pal = p();
        let title = match scope {
            Scope::Cli => "Install command line tool",
            Scope::Agents => "Connect to coding agents",
            Scope::Everything => "Set up noshape",
        };
        let intro = match scope {
            Scope::Cli => "The noshape command runs every operation of the app from a terminal, and its MCP server (noshape mcp) for coding agents.",
            Scope::Agents => "Connected agents can use noshape through its MCP server (noshape mcp): ask them to model a part, edit a drawing or export files.",
            Scope::Everything => "noshape can be used from a terminal and by coding agents through its MCP server (noshape mcp).",
        };
        let any = rows.borrow().iter().any(|r| r.on && !r.done);
        let list = v_flex().gap(px(6.)).children(rows.borrow().iter().enumerate().map(|(i, r)| {
            let rows = rows.clone();
            let done = r.done;
            h_flex()
                .id(("setup-row", i))
                .min_h(px(26.))
                .gap(px(8.))
                .when(!done, |d| d.cursor_pointer())
                .child(div().when(done, |d| d.opacity(0.5)).child(os::checkbox_box(r.on || done)))
                .child(div().font_weight(FontWeight::SEMIBOLD).child(r.label.clone()))
                .child(div().text_color(pal.text_muted).child(r.state.clone()))
                .on_click(move |_, window, _| {
                    if let Some(r) = rows.borrow_mut().get_mut(i).filter(|r| !r.done) {
                        r.on = !r.on;
                    }
                    window.refresh();
                })
        }));
        let (rows2, command) = (rows.clone(), command.clone());
        os::modal(dialog, title, 560.)
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
            .footer(os::modal_footer().child(os::modal_ok(div().when(!any, |d| d.opacity(0.4)).child(os::primary_button("ok", if scope == Scope::Everything { "Set up" } else { "Apply" })))).child(os::modal_cancel(os::cancel_button("cancel", if scope == Scope::Everything { "Not now" } else { "Cancel" }, cx))))
            .on_ok(move |_, window, cx| {
                let chosen: Vec<Option<Agent>> = rows2.borrow().iter().filter(|r| r.on && !r.done).map(|r| r.what).collect();
                if chosen.is_empty() {
                    return false;
                }
                apply(chosen, command.clone(), window, cx);
                true
            })
    });
}

/// Do what was chosen (in the background: the agents' commands, macOS's
/// password prompt), then say what happened.
fn apply(chosen: Vec<Option<Agent>>, command: PathBuf, window: &mut Window, cx: &mut App) {
    let task = cx.background_executor().spawn(async move {
        let env = Env::current();
        let mut lines = Vec::new();
        let mut failed = false;
        for c in chosen {
            match c {
                None => match install::cli_install(&env, &command, None) {
                    Ok(v) => {
                        lines.push(format!("The noshape command: {}", v["path"].as_str().unwrap_or("")));
                        if let Some(n) = v["note"].as_str() {
                            lines.push(format!("  {n}"));
                        }
                    }
                    Err(e) => {
                        failed = true;
                        lines.push(format!("The noshape command: {e}"));
                    }
                },
                Some(a) => match install::connect(&env, a, &command) {
                    Ok(ch) => lines.push(format!("{}: {}{}", a.name(), ch.result, if a == Agent::ClaudeDesktop { " (restart Claude Desktop to load it)" } else { "" })),
                    Err(e) => {
                        failed = true;
                        lines.push(format!("{}: {e}", a.name()));
                    }
                },
            }
        }
        (failed, lines.join("\n"))
    });
    window
        .spawn(cx, async move |cx| {
            let (failed, detail) = task.await;
            let _ = cx.update(|window, cx| {
                let (level, message) = if failed { (PromptLevel::Warning, "Some of the setup didn't work") } else { (PromptLevel::Info, "noshape is set up") };
                drop(window.prompt(level, message, Some(&detail), &["OK"], cx));
            });
        })
        .detach();
}
