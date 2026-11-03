//! The interaction parity suite's driver (tests/ui-parity/README.md):
//! `NOSHAPE_UI_SCRIPT=<scenario.json>` builds the scenario's start document,
//! opens it, runs the steps through the window's own input dispatch (mouse
//! events at a named control or a model point, keystrokes as typed), writes
//! the state after each step to `NOSHAPE_UI_STATE_OUT` and quits.

use crate::workspace::Workspace;
use gpui_kit::*;
use noshape_part_studio_ui::part_studio::PartStudioView;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Whole run limit: past it the recording so far is written and the app exits with status 3.
const WATCHDOG: Duration = Duration::from_secs(180);

pub fn enabled() -> bool {
    noshape_app_core::ui_script::active()
}

fn scenario_path() -> PathBuf {
    PathBuf::from(std::env::var_os("NOSHAPE_UI_SCRIPT").unwrap_or_default())
}

fn read_json(path: &Path) -> Result<Value, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

fn out_path() -> PathBuf {
    std::env::var_os("NOSHAPE_UI_STATE_OUT").map(PathBuf::from).unwrap_or_else(|| std::env::temp_dir().join("noshape-ui-state.json"))
}

fn write(recording: &Value) {
    let text = serde_json::to_string_pretty(recording).unwrap_or_default();
    if let Err(e) = std::fs::write(out_path(), text) {
        eprintln!("UI-SCRIPT could not write {}: {e}", out_path().display());
    }
}

fn fail(msg: &str) -> ! {
    eprintln!("UI-SCRIPT FAIL: {msg}");
    write(&json!({ "scenario": scenario_path().file_stem().map(|s| s.to_string_lossy().to_string()), "error": msg }));
    std::process::exit(2);
}

/// The start document of the scenario, built in a scratch file (before the window opens).
pub fn prepare() -> PathBuf {
    let path = scenario_path();
    let scenario = read_json(&path).unwrap_or_else(|e| fail(&e));
    let name = scenario["start"].as_str().unwrap_or("empty").to_string();
    let starts = std::env::var_os("NOSHAPE_UI_STARTS").map(PathBuf::from).unwrap_or_else(|| path.with_file_name("starts.json"));
    let start = if name == "empty" { json!({ "units": "mm", "features": [] }) } else { read_json(&starts).unwrap_or_else(|e| fail(&e)).get(&name).cloned().unwrap_or_else(|| fail(&format!("no start {name} in {}", starts.display()))) };
    let dir = std::env::temp_dir().join(format!("noshape-uip-{}", std::process::id()));
    let doc = dir.join(format!("{name}.noshape"));
    noshape_ops::ui_parity::build_start(&mut noshape_ops::Ctx::new(), &doc, &start).unwrap_or_else(|e| fail(&format!("start {name}: {e}")));
    doc
}

/// A key name of the scenario format as a GPUI keystroke ("Enter", "mod+z", "shift+e", "#").
fn keystroke(key: &str) -> Option<Keystroke> {
    let parts: Vec<&str> = key.split('+').collect();
    let (last, mods) = parts.split_last()?;
    let mut spec: Vec<String> = mods
        .iter()
        .map(|m| match m.to_lowercase().as_str() {
            "mod" if cfg!(target_os = "macos") => "cmd".to_string(),
            "mod" => "ctrl".to_string(),
            other => other.to_string(),
        })
        .collect();
    let name = match *last {
        "Enter" => "enter",
        "Escape" => "escape",
        "Tab" => "tab",
        "Backspace" => "backspace",
        "Delete" => "delete",
        "ArrowUp" => "up",
        "ArrowDown" => "down",
        "ArrowLeft" => "left",
        "ArrowRight" => "right",
        "Space" => "space",
        "Home" => "home",
        "End" => "end",
        k if k.len() > 1 && k.starts_with('F') => return Keystroke::parse(&[spec.join("-"), k.to_lowercase()].iter().filter(|s| !s.is_empty()).cloned().collect::<Vec<_>>().join("-")).ok(),
        k => k,
    };
    // Shift with a symbol key types its shifted symbol (US layout) and, as GPUI reads a keyboard (X11's
    // keystroke_from_xkb), no shift: shift+/ is "?".
    const SHIFTED: [(&str, &str); 21] = [
        ("/", "?"), ("1", "!"), ("2", "@"), ("3", "#"), ("4", "$"), ("5", "%"), ("6", "^"), ("7", "&"), ("8", "*"), ("9", "("), ("0", ")"),
        ("-", "_"), ("=", "+"), ("[", "{"), ("]", "}"), (";", ":"), ("'", "\""), (",", "<"), (".", ">"), ("`", "~"), ("\\", "|"),
    ];
    let name = match SHIFTED.iter().find(|(k, _)| *k == name) {
        Some((_, shifted)) if spec.iter().any(|m| m == "shift") => {
            spec.retain(|m| m != "shift");
            shifted
        }
        _ => name,
    };
    let ch = (name.chars().count() == 1).then(|| name.to_string());
    let upper = ch.as_ref().is_some_and(|c| c.chars().all(char::is_uppercase));
    if upper {
        spec.push("shift".into());
    }
    spec.push(if upper { name.to_lowercase() } else { name.to_string() });
    let mut ks = Keystroke::parse(&spec.join("-")).ok()?;
    let plain = !ks.modifiers.control && !ks.modifiers.platform && !ks.modifiers.alt;
    ks.key_char = match name {
        "space" if plain => Some(" ".into()),
        _ if plain => ch,
        _ => None,
    };
    Some(ks)
}

fn modifiers(step: &Value) -> Modifiers {
    let mut m = Modifiers::default();
    for k in step["mods"].as_str().unwrap_or("").split('+').filter(|k| !k.is_empty()) {
        match k {
            "shift" => m.shift = true,
            "alt" => m.alt = true,
            "mod" if cfg!(target_os = "macos") => m.platform = true,
            "mod" => m.control = true,
            _ => {}
        }
    }
    m
}

/// The name a control target is recorded under (`ui_script::mark`), if it is one.
fn control_name(t: &Value) -> Option<String> {
    // A chip's × (`"index"`: which chip, the first by default), a row's eye; other buttons go by their tooltip.
    match t.get("button").and_then(Value::as_str) {
        Some("x") => return t.get("field").and_then(Value::as_str).map(|f| format!("button:x:{f}:{}", t.get("index").and_then(Value::as_u64).unwrap_or(0))),
        Some("eye") => {
            return t.get("feature").and_then(Value::as_str).map(|f| format!("button:eye:{f}")).or_else(|| t.get("part").and_then(Value::as_str).map(|p| format!("button:eye-part:{p}")));
        }
        Some(b) => return Some(format!("button:{b}")),
        None => {}
    }
    if t.get("rollback_bar").is_some() {
        return Some("button:Rollback bar".into());
    }
    // A tool in a toolbar dropdown: its caret first (the step then picks the entry).
    if let (Some(tool), Some(_)) = (t.get("tool").and_then(Value::as_str), t.get("menu")) {
        return Some(format!("caret:{tool}"));
    }
    ["tool", "field", "feature", "part", "menu", "tab", "checkbox", "list_row", "dropdown"].iter().find_map(|k| t.get(*k).and_then(Value::as_str).map(|v| format!("{k}:{v}")))
}

type Studio = Option<Entity<PartStudioView>>;

/// Where target `t` shows now, in window coordinates.
fn locate(t: &Value, studio: &Studio, cx: &App) -> Result<Point<Pixels>, String> {
    if let Some(name) = control_name(t) {
        if let Some(b) = noshape_app_core::ui_script::bounds_of(&name) {
            // A row's top or bottom edge (where a dragged row drops above or below it).
            return Ok(match t.get("edge").and_then(Value::as_str) {
                Some("top") => point(b.center().x, b.top() + px(3.)),
                Some("bottom") => point(b.center().x, b.bottom() - px(3.)),
                _ => b.center(),
            });
        }
        // A field named by part of its label ("Distance" for "Offset distance"), when only one matches.
        if let Some(f) = name.strip_prefix("field:").map(str::to_lowercase) {
            let near: Vec<_> = noshape_app_core::ui_script::named("field:").into_iter().filter(|(k, _)| k.to_lowercase().contains(&f)).collect();
            if let [(_, b)] = near.as_slice() {
                return Ok(b.center());
            }
        }
        return Err(format!("{name} is not shown"));
    }
    // The rollback bar dropped just below a row.
    if let Some(f) = t.get("rollback_to").and_then(Value::as_str) {
        let b = noshape_app_core::ui_script::bounds_of(&format!("feature:{f}")).ok_or_else(|| format!("feature:{f} is not shown"))?;
        return Ok(point(b.center().x, b.bottom() + px(3.)));
    }
    let ps = studio.as_ref().ok_or("not in a Part Studio")?;
    // A pointer is at whole pixels (a mouse's, and Chrome's under live's runner).
    ps.read(cx).ui_target_point(t, cx).map(|p| point(px(f32::from(p.x).floor()), px(f32::from(p.y).floor())))
}

/// Presses at `at`, where the pointer already is (a pointer that doesn't move sends no second move: a repeated
/// one would snap an edge's midpoint).
fn mouse(window: &mut Window, cx: &mut App, at: Point<Pixels>, button: MouseButton, modifiers: Modifiers, clicks: usize) {
    for n in 1..=clicks {
        window.dispatch_event(PlatformInput::MouseDown(MouseDownEvent { button, position: at, modifiers, click_count: n, first_mouse: false }), cx);
        window.dispatch_event(PlatformInput::MouseUp(MouseUpEvent { button, position: at, modifiers, click_count: n }), cx);
    }
}

/// Run the scenario in `window` (the document window of `ws`).
pub fn start(window: &mut Window, cx: &mut Context<Workspace>) {
    let scenario = read_json(&scenario_path()).unwrap_or_else(|e| fail(&e));
    std::thread::spawn(|| {
        std::thread::sleep(WATCHDOG);
        eprintln!("UI-SCRIPT FAIL: not done after {}s", WATCHDOG.as_secs());
        std::process::exit(3);
    });
    // Live's page (Chrome's viewport on its runner): the same toolbar groups and dialog places.
    window.resize(size(px(1919.), px(992.)));
    window.activate_window();
    cx.activate(true);
    cx.spawn_in(window, async move |ws, cx| {
        let sleep = async |cx: &mut AsyncWindowContext, ms: u64| cx.background_executor().timer(Duration::from_millis(ms)).await;
        let studio = |cx: &mut AsyncWindowContext| -> Studio { ws.update(cx, |w, _| w.active_part_studio()).ok().flatten() };
        // Settled: the regeneration done, and a few frames for picks and menus.
        let settle = async |cx: &mut AsyncWindowContext| {
            let t = Instant::now();
            loop {
                sleep(cx, 120).await;
                let busy = match studio(cx) {
                    Some(ps) => cx.update(|_, cx| ps.read(cx).ui_busy()).unwrap_or(false),
                    None => true,
                };
                if !busy || t.elapsed() > Duration::from_secs(30) {
                    break;
                }
            }
            sleep(cx, 400).await;
        };
        let state = |cx: &mut AsyncWindowContext| -> Value {
            let Some(ps) = studio(cx) else { return json!({ "error": "no Part Studio shown" }) };
            cx.update(|window, cx| ps.read(cx).ui_state(window, cx)).unwrap_or(Value::Null)
        };
        // The state with the tooltip drawn now (a redraw notes it).
        let read = async |cx: &mut AsyncWindowContext| -> Value {
            noshape_app_core::ui_script::clear_tooltip();
            let _ = cx.update(|window, _| window.refresh());
            sleep(cx, 150).await;
            let mut s = state(cx);
            if s.is_object() {
                s["tooltip"] = json!(noshape_app_core::ui_script::shown_tooltip());
            }
            s
        };
        // Every named control painted where it is now.
        let redraw = async |cx: &mut AsyncWindowContext| {
            noshape_app_core::ui_script::clear();
            if let Some(ps) = studio(cx) {
                let _ = cx.update(|window, cx| {
                    ps.update(cx, |p, cx| p.ui_redraw(cx));
                    window.refresh();
                });
            }
            sleep(cx, 150).await;
        };
        // Focus events need an active window (X11 without a window manager:
        // the runner activates it with xdotool).
        let mut active = false;
        for _ in 0..50 {
            active = cx.update(|window, _| window.is_window_active()).unwrap_or(false);
            if active {
                break;
            }
            sleep(cx, 100).await;
        }
        if !active {
            eprintln!("UI-SCRIPT warning: the window is not active, fields get no focus events");
        }
        settle(cx).await;
        let start = read(cx).await;
        let mut steps = Vec::new();
        for (i, step) in scenario["steps"].as_array().cloned().unwrap_or_default().into_iter().enumerate() {
            let what = step["do"].as_str().unwrap_or("");
            let mods = modifiers(&step);
            let target = step.get("on").or_else(|| step.get("from")).cloned().unwrap_or(Value::Null);
            redraw(cx).await;
            let ps = studio(cx);
            let result: Result<(), String> = match what {
                "click" | "dblclick" | "rclick" | "hover" => match cx.update(|_, cx| locate(&target, &ps, cx)).unwrap_or_else(|e| Err(e.to_string())) {
                    Ok(at) => {
                        eprintln!("UI-SCRIPT step {} {what} at ({:.0}, {:.0})", i + 1, f32::from(at.x), f32::from(at.y));
                        let _ = cx.update(|window, cx| window.dispatch_event(PlatformInput::MouseMove(MouseMoveEvent { position: at, pressed_button: None, modifiers: mods }), cx));
                        // The graphics area picks what is under the pointer before the press.
                        sleep(cx, if what == "hover" { step["ms"].as_u64().unwrap_or(1500) } else { 500 }).await;
                        let (button, clicks) = match what {
                            "rclick" => (MouseButton::Right, 1),
                            "dblclick" => (MouseButton::Left, 2),
                            "hover" => (MouseButton::Left, 0),
                            _ => (MouseButton::Left, 1),
                        };
                        let _ = cx.update(|window, cx| mouse(window, cx, at, button, mods, clicks));
                        // A dropdown step picks its entry from the list it opened (a toolbar dropdown: its tool).
                        let pick = step["on"].get("pick").or_else(|| step["on"].get("tool").and(step["on"].get("menu"))).and_then(Value::as_str);
                        let row_key = if step["on"].get("pick").is_some() { "list_row" } else { "menu" };
                        match pick {
                            Some(pick) => {
                                sleep(cx, 300).await;
                                redraw(cx).await;
                                let row = json!({ row_key: pick });
                                match cx.update(|_, cx| locate(&row, &ps, cx)).unwrap_or_else(|e| Err(e.to_string())) {
                                    Ok(at) => {
                                        let _ = cx.update(|window, cx| {
                                            window.dispatch_event(PlatformInput::MouseMove(MouseMoveEvent { position: at, pressed_button: None, modifiers: Modifiers::default() }), cx);
                                            mouse(window, cx, at, MouseButton::Left, Modifiers::default(), 1)
                                        });
                                        Ok(())
                                    }
                                    Err(e) => Err(e),
                                }
                            }
                            None => Ok(()),
                        }
                    }
                    Err(e) => Err(e),
                },
                "drag" => {
                    let to = step.get("to").cloned().unwrap_or(Value::Null);
                    let from = cx.update(|_, cx| locate(&target, &ps, cx)).unwrap_or_else(|e| Err(e.to_string()));
                    let to = cx.update(|_, cx| locate(&to, &ps, cx)).unwrap_or_else(|e| Err(e.to_string()));
                    match (from, to) {
                        (Ok(a), Ok(b)) => {
                            let _ = cx.update(|window, cx| {
                                window.dispatch_event(PlatformInput::MouseMove(MouseMoveEvent { position: a, pressed_button: None, modifiers: mods }), cx);
                                window.dispatch_event(PlatformInput::MouseDown(MouseDownEvent { button: MouseButton::Left, position: a, modifiers: mods, click_count: 1, first_mouse: false }), cx);
                            });
                            for k in 1..=10 {
                                let p = a + (b - a) * (k as f32 / 10.0);
                                let _ = cx.update(|window, cx| window.dispatch_event(PlatformInput::MouseMove(MouseMoveEvent { position: p, pressed_button: Some(MouseButton::Left), modifiers: mods }), cx));
                                sleep(cx, 30).await;
                            }
                            let _ = cx.update(|window, cx| window.dispatch_event(PlatformInput::MouseUp(MouseUpEvent { button: MouseButton::Left, position: b, modifiers: mods, click_count: 1 }), cx));
                            Ok(())
                        }
                        (Err(e), _) | (_, Err(e)) => Err(e),
                    }
                }
                "type" => {
                    for ch in step["text"].as_str().unwrap_or("").chars() {
                        let key = if ch == ' ' { "Space".to_string() } else { ch.to_string() };
                        if let Some(ks) = keystroke(&key) {
                            let _ = cx.update(|window, cx| window.dispatch_keystroke(ks, cx));
                        }
                        sleep(cx, 40).await;
                    }
                    Ok(())
                }
                "key" => match keystroke(step["key"].as_str().unwrap_or("")) {
                    Some(ks) => {
                        let _ = cx.update(|window, cx| window.dispatch_keystroke(ks, cx));
                        Ok(())
                    }
                    None => Err(format!("unknown key {}", step["key"])),
                },
                "wait" => {
                    match step["until"].as_str() {
                        Some("ms") => sleep(cx, step["ms"].as_u64().unwrap_or(500)).await,
                        Some("dialog") => {
                            for _ in 0..50 {
                                let open = cx.update(|window, cx| studio_has_dialog(&ps, window, cx)).unwrap_or(false);
                                if open {
                                    break;
                                }
                                sleep(cx, 100).await;
                            }
                        }
                        _ => {}
                    }
                    Ok(())
                }
                other => Err(format!("unknown step {other}")),
            };
            settle(cx).await;
            let state_now = read(cx).await;
            let mut rec = json!({ "step": i + 1, "do": step, "ok": result.is_ok(), "state": state_now });
            if let Err(e) = result {
                eprintln!("UI-SCRIPT step {}: {e}", i + 1);
                rec["error"] = json!(e);
            }
            steps.push(rec);
        }
        let recording = json!({
            "scenario": scenario["name"],
            "recorded": utc_now(),
            "app": { "name": "noshape", "version": noshape_ops::VERSION, "platform": std::env::consts::OS, "active": active },
            "start": start,
            "steps": steps,
        });
        write(&recording);
        eprintln!("UI-SCRIPT done: {}", out_path().display());
        let _ = cx.update(|_, cx| cx.quit());
    })
    .detach();
}

fn studio_has_dialog(ps: &Studio, window: &Window, cx: &App) -> bool {
    ps.as_ref().is_some_and(|p| p.read(cx).ui_state(window, cx)["dialog"].is_object())
}

/// Now as `2026-11-03T01:00:00Z`.
fn utc_now() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()) as i64;
    let (y, m, d) = noshape_model::drawing::format::civil_from_days(secs.div_euclid(86_400));
    let rem = secs.rem_euclid(86_400);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", rem / 3600, rem % 3600 / 60, rem % 60)
}
