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
pub fn prepare() -> Option<PathBuf> {
    let path = scenario_path();
    let scenario = read_json(&path).unwrap_or_else(|e| fail(&e));
    let name = scenario["start"].as_str().unwrap_or("empty").to_string();
    let starts_path = std::env::var_os("NOSHAPE_UI_STARTS").map(PathBuf::from).unwrap_or_else(|| path.with_file_name("starts.json"));
    let starts = if name == "empty" { json!({}) } else { read_json(&starts_path).unwrap_or_else(|e| fail(&e)) };
    let mut start = if name == "empty" { json!({ "units": "mm", "features": [] }) } else { starts.get(&name).cloned().unwrap_or_else(|| fail(&format!("no start {name} in {}", starts_path.display()))) };
    // A start's files are beside starts.json, in files/ (README "### start").
    if start.get("files").is_some() {
        start["files_dir"] = json!(starts_path.with_file_name("files"));
    }
    // A page start (README "Documents page and Preferences"): its documents in the run's own documents folder.
    if start.get("page").is_some() {
        let Some(docs) = std::env::var_os("NOSHAPE_DOCUMENTS_DIR").map(PathBuf::from) else { fail("a page start needs NOSHAPE_DOCUMENTS_DIR (the run's own documents folder)") };
        build_documents(&docs, &start["documents"], &starts);
        noshape_start_ui::start_window::open_on_owned_by_me();
        return None;
    }
    let dir = std::env::temp_dir().join(format!("noshape-uip-{}", std::process::id()));
    let doc = dir.join(format!("{name}.noshape"));
    noshape_ops::ui_parity::build_start(&mut noshape_ops::Ctx::new(), &doc, &start).unwrap_or_else(|e| fail(&format!("start {name}: {e}")));
    Some(doc)
}

/// A page start's documents (`{"name", "start"}`, a start of starts.json; `"folder"` with its own `documents`) in `dir`.
fn build_documents(dir: &Path, docs: &Value, starts: &Value) {
    for d in docs.as_array().into_iter().flatten() {
        let name = d["name"].as_str().unwrap_or_else(|| fail(&format!("a start document without a name: {d}")));
        if d.get("documents").is_some() {
            let sub = dir.join(name);
            let _ = std::fs::create_dir_all(&sub);
            build_documents(&sub, &d["documents"], starts);
            continue;
        }
        let start = match d["start"].as_str() {
            Some(s) => starts.get(s).cloned().unwrap_or_else(|| fail(&format!("no start {s}"))),
            None => json!({ "units": "mm", "features": [], "tabs": [] }),
        };
        let path = dir.join(format!("{name}.noshape"));
        noshape_ops::ui_parity::build_start(&mut noshape_ops::Ctx::new(), &path, &start).unwrap_or_else(|e| fail(&format!("start document {name}: {e}")));
        // Live's account opened each: they are in Recently opened.
        noshape_app_core::doc_files::add_recent(&path);
    }
}

/// The page a page start opens on ("documents", "preferences"), if the scenario's start is one.
pub fn page_start() -> Option<String> {
    let path = scenario_path();
    let scenario = read_json(&path).ok()?;
    let name = scenario["start"].as_str()?;
    let starts = std::env::var_os("NOSHAPE_UI_STARTS").map(PathBuf::from).unwrap_or_else(|| path.with_file_name("starts.json"));
    read_json(&starts).ok()?[name]["page"].as_str().map(str::to_string)
}

/// The tab the scenario start opens on: its assembly ("start <name> asm") or its drawing ("<part> Drawing 1").
fn opens_on_tab() -> Option<String> {
    let path = scenario_path();
    let scenario = read_json(&path).ok()?;
    let name = scenario["start"].as_str().unwrap_or("empty");
    let starts = std::env::var_os("NOSHAPE_UI_STARTS").map(PathBuf::from).unwrap_or_else(|| path.with_file_name("starts.json"));
    let start = read_json(&starts).ok()?[name].clone();
    // A scenario that first clicks a document tab opens on it (live's runner: appearance-part-row-configure's start is
    // config-blocks' Part Studio); studios' whole-document starts open on the first tab and click it (live2's runner).
    if let Some(tab) = scenario["steps"].get(0).and_then(|s| s["on"]["doc_tab"].as_str()).filter(|_| scenario["area"] != "studios") {
        return Some(tab.to_string());
    }
    if let Some(d) = start.get("drawing") {
        // (A drawing of the start's assembly is named after its tab.)
        // (`"part": null`: of the whole Part Studio.)
        let of = if d["assembly"] == true {
            format!("start {name} asm")
        } else if d.get("part").is_some_and(Value::is_null) {
            "Part Studio 1".to_string()
        } else {
            d["part"].as_str().unwrap_or("Part 1").to_string()
        };
        return Some(format!("{of} Drawing 1"));
    }
    start.get("assembly").map(|_| format!("start {name} asm"))
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
        "PageUp" => "pageup",
        "PageDown" => "pagedown",
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
    // A pick's chip in a selection field (`"chip"`: its row, from 0).
    if let (Some(row), Some(field)) = (t.get("chip").and_then(Value::as_u64), t.get("field").and_then(Value::as_str)) {
        return Some(format!("chip:{field}:{row}"));
    }
    if t.get("rollback_bar").is_some() {
        return Some("button:Rollback bar".into());
    }
    // A tool in a toolbar dropdown: its caret first (the step then picks the entry).
    if let (Some(tool), Some(_)) = (t.get("tool").and_then(Value::as_str), t.get("menu")) {
        return Some(format!("caret:{tool}"));
    }
    // A face / edge / vertex point with an `instance` is geometry (in that part's coordinates), not the list row.
    let geometry = ["face", "edge", "vertex"].iter().any(|k| t.get(*k).is_some_and(Value::is_array));
    ["tool", "field", "feature", "part", "menu", "tab", "checkbox", "list_row", "dropdown", "instance", "mate", "doc_tab", "config_more", "config_row", "sheet_tab", "sheet_ref", "sheet_table", "panel_row", "panel_more", "caret", "context_more", "navbar", "nav", "doc_row", "history_row", "var_cell", "editor"]
        .iter()
        .filter(|k| !(geometry && **k == "instance"))
        .find_map(|k| t.get(*k).and_then(Value::as_str).map(|v| format!("{k}:{v}")))
}

type Studio = Option<Entity<PartStudioView>>;

/// Where target `t` shows now, in window coordinates.
fn locate(t: &Value, studio: &Studio, assembly: &Option<Entity<crate::assembly_view::AssemblyView>>, drawing: &Option<Entity<crate::drawing_view::DrawingEditor>>, cx: &App) -> Result<Point<Pixels>, String> {
    if let Some(name) = control_name(t) {
        if let Some(b) = noshape_app_core::ui_script::bounds_of(&name) {
            // A row's top or bottom edge (where a dragged row drops above or below it).
            return Ok(match t.get("edge").and_then(Value::as_str) {
                Some("top") => point(b.center().x, b.top() + px(3.)),
                Some("bottom") => point(b.center().x, b.bottom() - px(3.)),
                Some("left") => point(b.left() + px(3.), b.center().y),
                Some("right") => point(b.right() - px(3.), b.center().y),
                _ => b.center(),
            });
        }
        // A list row by the start of its text, as live's runner falls back to ("Assembly" for "Assembly 2 <1>"), when only
        // one row starts with it and a space.
        for kind in ["instance:", "feature:"] {
            if let Some(rest) = name.strip_prefix(kind) {
                let prefix = format!("{kind}{rest} ");
                let near: Vec<_> = noshape_app_core::ui_script::named(kind).into_iter().filter(|(k, _)| k.starts_with(&prefix)).collect();
                if let [(_, b)] = near.as_slice() {
                    return Ok(b.center());
                }
            }
        }
        // A field named by part of its label ("Distance" for "Offset distance"), a button by the words of its tooltip
        // ("Mass properties" for "Display mass and section properties", as live's runner), when only one matches.
        for kind in ["field:", "button:"] {
            if let Some(f) = name.strip_prefix(kind).map(str::to_lowercase) {
                let has = |k: &str| {
                    let k = k[kind.len()..].to_lowercase();
                    if kind == "field:" { k.contains(&f) } else { f.split_whitespace().all(|w| k.split_whitespace().any(|x| x == w)) }
                };
                let near: Vec<_> = noshape_app_core::ui_script::named(kind).into_iter().filter(|(k, _)| has(k)).collect();
                if let [(_, b)] = near.as_slice() {
                    return Ok(b.center());
                }
            }
        }
        return Err(format!("{name} is not shown"));
    }
    // The rollback bar dropped just below a row.
    if let Some(f) = t.get("rollback_to").and_then(Value::as_str) {
        let b = noshape_app_core::ui_script::bounds_of(&format!("feature:{f}")).ok_or_else(|| format!("feature:{f} is not shown"))?;
        return Ok(point(b.center().x, b.bottom() + px(3.)));
    }
    // A pointer is at whole pixels (a mouse's, and Chrome's under live's runner).
    let whole = |p: Point<Pixels>| point(px(f32::from(p.x).floor()), px(f32::from(p.y).floor()));
    if let Some(ps) = studio.as_ref() {
        return ps.read(cx).ui_target_point(t, cx).map(whole);
    }
    if let Some(d) = drawing.as_ref() {
        return d.read(cx).ui_target_point(t, cx).map(whole);
    }
    let asm = assembly.as_ref().ok_or("not in a Part Studio, an Assembly or a Drawing")?;
    asm.read(cx).ui_target_point(t, cx).map(whole)
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
                let drawing = ws.update(cx, |w, _| w.active_drawing()).ok().flatten();
                let busy = match (studio(cx), drawing) {
                    (Some(ps), _) => cx.update(|_, cx| ps.read(cx).ui_busy()).unwrap_or(false),
                    (None, Some(d)) => cx.update(|_, cx| d.read(cx).computing).unwrap_or(false),
                    (None, None) => ws.update(cx, |w, cx| w.ui_view_pending(cx)).unwrap_or(false),
                };
                if !busy || t.elapsed() > Duration::from_secs(30) {
                    break;
                }
            }
            sleep(cx, 400).await;
        };
        // (An assembly tab's state when no Part Studio is shown: skasm's
        // assembly scenarios.)
        let state = |cx: &mut AsyncWindowContext| -> Value { ws.update_in(cx, window_state).unwrap_or(Value::Null) };
        // The state with the tooltip drawn now (a redraw notes it).
        let read = async |cx: &mut AsyncWindowContext| -> Value {
            noshape_app_core::ui_script::clear_tooltip();
            noshape_app_core::ui_script::clear_dialog();
            let _ = cx.update(|window, _| window.refresh());
            sleep(cx, 150).await;
            let mut s = state(cx);
            if s.is_object() {
                s["tooltip"] = json!(noshape_app_core::ui_script::shown_tooltip());
                // A modal of the window's (Create Drawing, Export Drawing) holds the focus (live 2063: "other").
                if s["dialog"]["kind"] == "modal" && s["focus"]["kind"] == "none" {
                    s["focus"] = json!({ "kind": "other" });
                }
            }
            s
        };
        // Every named control painted where it is now.
        let redraw = async |cx: &mut AsyncWindowContext| {
            noshape_app_core::ui_script::clear();
            // The chrome (tab bar, navbar, left panels) is cached: repainted too.
            let _ = ws.update(cx, |w, cx| w.ui_redraw(cx));
            if let Some(ps) = studio(cx) {
                let _ = cx.update(|window, cx| {
                    ps.update(cx, |p, cx| p.ui_redraw(cx));
                    window.refresh();
                });
            } else if let Some(a) = ws.update(cx, |w, _| w.active_assembly()).ok().flatten() {
                let _ = cx.update(|window, cx| {
                    a.update(cx, |a, cx| a.ui_redraw(cx));
                    window.refresh();
                });
            } else if let Some(d) = ws.update(cx, |w, _| w.active_drawing()).ok().flatten() {
                let _ = cx.update(|window, cx| {
                    d.update(cx, |d, cx| d.ui_redraw(cx));
                    window.refresh();
                });
            } else {
                let _ = cx.update(|window, _| window.refresh());
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
        // A start with an assembly opens on it (README "start").
        if let Some(tab) = opens_on_tab() {
            let _ = ws.update_in(cx, |w, window, cx| w.activate_named(&tab, window, cx));
        }
        settle(cx).await;
        let start = read(cx).await;
        let mut steps = Vec::new();
        // Where the pointer is: a click's pointer comes from there in three
        // moves, as live's runner moves it (what it passes over can wake a
        // sketch inference).
        let mut pointer: Option<Point<Pixels>> = None;
        for (i, step) in scenario["steps"].as_array().cloned().unwrap_or_default().into_iter().enumerate() {
            let what = step["do"].as_str().unwrap_or("");
            let mods = modifiers(&step);
            let target = step.get("on").or_else(|| step.get("from")).cloned().unwrap_or(Value::Null);
            redraw(cx).await;
            let ps = studio(cx);
            let asm = ws.update(cx, |w, _| w.active_assembly()).ok().flatten();
            let drw = ws.update(cx, |w, _| w.active_drawing()).ok().flatten();
            let result: Result<(), String> = match what {
                "click" | "dblclick" | "rclick" | "hover" => match cx.update(|_, cx| locate(&target, &ps, &asm, &drw, cx)).unwrap_or_else(|e| Err(e.to_string())) {
                    Ok(at) => {
                        eprintln!("UI-SCRIPT step {} {what} at ({:.0}, {:.0})", i + 1, f32::from(at.x), f32::from(at.y));
                        let from = pointer.unwrap_or(at);
                        for k in 1..=3 {
                            let p = from + (at - from) * (k as f32 / 3.0);
                            let _ = cx.update(|window, cx| window.dispatch_event(PlatformInput::MouseMove(MouseMoveEvent { position: p, pressed_button: None, modifiers: mods }), cx));
                            if k < 3 {
                                sleep(cx, 20).await;
                            }
                        }
                        pointer = Some(at);
                        // The graphics area picks what is under the pointer before the press.
                        sleep(cx, if what == "hover" { step["ms"].as_u64().unwrap_or(1500) } else { 500 }).await;
                        // Then until the hover under the resting pointer has settled (a loaded machine answers the
                        // hover query late; an edge's midpoint snaps after its dwell).
                        for _ in 0..40 {
                            let settling = cx.update(|_, cx| ps.as_ref().is_some_and(|ps| ps.read(cx).ui_hover_settling(cx))).unwrap_or(false);
                            if !settling {
                                break;
                            }
                            sleep(cx, 50).await;
                        }
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
                                // (A native-style select's list: its rows are a menu's.)
                                let found = cx.update(|_, cx| locate(&row, &ps, &asm, &drw, cx).or_else(|e| if row_key == "list_row" { locate(&json!({ "menu": pick }), &ps, &asm, &drw, cx) } else { Err(e) })).unwrap_or_else(|e| Err(e.to_string()));
                                match found {
                                    Ok(at) => {
                                        let _ = cx.update(|window, cx| {
                                            window.dispatch_event(PlatformInput::MouseMove(MouseMoveEvent { position: at, pressed_button: None, modifiers: Modifiers::default() }), cx);
                                            mouse(window, cx, at, MouseButton::Left, Modifiers::default(), 1)
                                        });
                                        pointer = Some(at);
                                        Ok(())
                                    }
                                    Err(e) => Err(e),
                                }
                            }
                            None => Ok(()),
                        }
                    }
                    // A tool not on the toolbar (inside a dropdown): Search
                    // tools, its name and Enter, as live's runner does (a drawing has no tool search).
                    Err(_) if what == "click" && drw.is_none() && target.get("menu").is_none() && target.get("tool").and_then(Value::as_str).is_some() => {
                        let name = target["tool"].as_str().unwrap_or("");
                        for key in std::iter::once("alt+c".to_string()).chain(name.chars().map(|c| if c == ' ' { "Space".into() } else { c.to_string() })).chain(std::iter::once("Enter".to_string())) {
                            if let Some(ks) = keystroke(&key) {
                                let _ = cx.update(|window, cx| window.dispatch_keystroke(ks, cx));
                            }
                            sleep(cx, 60).await;
                        }
                        Ok(())
                    }
                    // A button not on screen: the view cube's Camera and render options menu (live's runner: "Section
                    // view" is its "Section view…").
                    Err(e) if what == "click" && target.get("button").and_then(Value::as_str).is_some() => {
                        let name = target["button"].as_str().unwrap_or("").to_string();
                        match cx.update(|_, cx| locate(&json!({ "button": "Camera and render options" }), &ps, &asm, &drw, cx)).unwrap_or_else(|e| Err(e.to_string())) {
                            Ok(at) => {
                                let _ = cx.update(|window, cx| {
                                    window.dispatch_event(PlatformInput::MouseMove(MouseMoveEvent { position: at, pressed_button: None, modifiers: Modifiers::default() }), cx);
                                    mouse(window, cx, at, MouseButton::Left, Modifiers::default(), 1)
                                });
                                sleep(cx, 300).await;
                                redraw(cx).await;
                                let row = cx.update(|_, cx| locate(&json!({ "menu": name }), &ps, &asm, &drw, cx).or_else(|_| locate(&json!({ "menu": format!("{name}…") }), &ps, &asm, &drw, cx))).unwrap_or_else(|e| Err(e.to_string()));
                                match row {
                                    Ok(at) => {
                                        let _ = cx.update(|window, cx| {
                                            window.dispatch_event(PlatformInput::MouseMove(MouseMoveEvent { position: at, pressed_button: None, modifiers: Modifiers::default() }), cx);
                                            mouse(window, cx, at, MouseButton::Left, Modifiers::default(), 1)
                                        });
                                        pointer = Some(at);
                                        Ok(())
                                    }
                                    Err(_) => {
                                        // Not there either: close the menu again.
                                        if let Some(ks) = keystroke("Escape") {
                                            let _ = cx.update(|window, cx| window.dispatch_keystroke(ks, cx));
                                        }
                                        Err(e)
                                    }
                                }
                            }
                            Err(_) => Err(e),
                        }
                    }
                    Err(e) => Err(e),
                },
                "drag" => {
                    let to = step.get("to").cloned().unwrap_or(Value::Null);
                    let from = cx.update(|_, cx| locate(&target, &ps, &asm, &drw, cx)).unwrap_or_else(|e| Err(e.to_string()));
                    let to = cx.update(|_, cx| locate(&to, &ps, &asm, &drw, cx)).unwrap_or_else(|e| Err(e.to_string()));
                    match (from, to) {
                        (Ok(a), Ok(b)) => {
                            // (As live's runner: three moves to the start, fifteen to the end.)
                            let start = pointer.unwrap_or(a);
                            for k in 1..=3 {
                                let p = start + (a - start) * (k as f32 / 3.0);
                                let _ = cx.update(|window, cx| window.dispatch_event(PlatformInput::MouseMove(MouseMoveEvent { position: p, pressed_button: None, modifiers: mods }), cx));
                                sleep(cx, 20).await;
                            }
                            let _ = cx.update(|window, cx| window.dispatch_event(PlatformInput::MouseDown(MouseDownEvent { button: MouseButton::Left, position: a, modifiers: mods, click_count: 1, first_mouse: false }), cx));
                            pointer = Some(b);
                            for k in 1..=15 {
                                let p = a + (b - a) * (k as f32 / 15.0);
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
        // The drawing's exports after the last step (`"export": ["PDF", "DXF", "DWG"]`), beside the recording as
        // <recording>.<ext>, for tools/drawing-export to compare with live's.
        let mut exported = Vec::new();
        // (An entry is the format's name, or {"format", "version"}: live's dialog shows its account's last version.)
        for entry in scenario["export"].as_array().cloned().unwrap_or_default() {
            let format = entry.as_str().or_else(|| entry["format"].as_str()).unwrap_or("").to_string();
            let version = entry["version"].as_str().map(str::to_string);
            let Some(d) = ws.update(cx, |w, _| w.active_drawing()).ok().flatten() else { break };
            let file = out_path().with_extension(format.to_lowercase());
            match cx.update(|_, cx| d.read(cx).ui_export(&format, version.as_deref(), cx)).map_err(|e| e.to_string()).and_then(|r| r) {
                Ok(bytes) => match std::fs::write(&file, bytes) {
                    Ok(()) => exported.push(json!(file.display().to_string())),
                    Err(e) => eprintln!("UI-SCRIPT export {format}: {e}"),
                },
                Err(e) => eprintln!("UI-SCRIPT export {format}: {e}"),
            }
        }
        let recording = json!({
            "scenario": scenario["name"],
            "exported": exported,
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

/// Runs a page scenario (README "Documents page and Preferences"): the steps go to the window in front (the page it
/// starts on, then any window a step opened: a document, Preferences), whose registered reader gives the state.
pub fn start_page(cx: &mut App) {
    let scenario = read_json(&scenario_path()).unwrap_or_else(|e| fail(&e));
    std::thread::spawn(|| {
        std::thread::sleep(WATCHDOG);
        eprintln!("UI-SCRIPT FAIL: not done after {}s", WATCHDOG.as_secs());
        std::process::exit(3);
    });
    cx.spawn(async move |cx| {
        let sleep = async |cx: &mut AsyncApp, ms: u64| cx.background_executor().timer(Duration::from_millis(ms)).await;
        // The window in front: the newest one a step opened, else the active one.
        let mut known: Vec<AnyWindowHandle> = Vec::new();
        let mut front: Option<AnyWindowHandle> = None;
        let mut follow = |cx: &mut AsyncApp| -> Option<AnyWindowHandle> {
            let now = cx.update(|cx| cx.windows());
            if let Some(new) = now.iter().rev().find(|w| !known.contains(w)) {
                front = Some(*new);
            }
            if front.is_none_or(|f| !now.contains(&f)) {
                front = cx.update(|cx| cx.active_window()).or_else(|| now.last().copied());
            }
            known = now;
            front
        };
        // The page's window up and drawn.
        let mut win = None;
        for _ in 0..100 {
            win = follow(cx);
            if win.is_some() {
                break;
            }
            sleep(cx, 100).await;
        }
        let Some(first) = win else { fail("no window opened") };
        let _ = first.update(cx, |_, window, _| {
            window.resize(size(px(1919.), px(992.)));
            window.activate_window();
        });
        sleep(cx, 1500).await;
        let redraw = async |w: AnyWindowHandle, cx: &mut AsyncApp| {
            noshape_app_core::ui_script::clear();
            let _ = w.update(cx, |_, window, _| window.refresh());
            sleep(cx, 150).await;
        };
        let read = async |w: AnyWindowHandle, cx: &mut AsyncApp| -> Value {
            noshape_app_core::ui_script::clear_tooltip();
            noshape_app_core::ui_script::clear_dialog();
            let _ = w.update(cx, |_, window, _| window.refresh());
            sleep(cx, 150).await;
            let mut s = w.update(cx, |_, window, cx| noshape_app_core::ui_script::page_state(window, cx)).ok().flatten().unwrap_or_else(|| json!({ "error": "a window without a ui-parity state" }));
            if s.is_object() {
                s["tooltip"] = json!(noshape_app_core::ui_script::shown_tooltip());
                if s["dialog"]["kind"] == "modal" && s["focus"]["kind"] == "none" {
                    s["focus"] = json!({ "kind": "other" });
                }
            }
            s
        };
        let start = read(first, cx).await;
        let mut steps = Vec::new();
        let mut pointer: Option<Point<Pixels>> = None;
        for (i, step) in scenario["steps"].as_array().cloned().unwrap_or_default().into_iter().enumerate() {
            let Some(w) = follow(cx) else { break };
            let what = step["do"].as_str().unwrap_or("");
            let mods = modifiers(&step);
            let target = step.get("on").or_else(|| step.get("from")).cloned().unwrap_or(Value::Null);
            redraw(w, cx).await;
            let find = |t: &Value, cx: &mut AsyncApp| cx.update(|cx| locate(t, &None, &None, &None, cx));
            // A control scrolled out of view is wheeled into it first, as a browser's click scrolls to it.
            let mut found = Err(String::new());
            if matches!(what, "click" | "dblclick" | "rclick" | "hover") {
                found = find(&target, cx);
                let (vw, vh) = w.update(cx, |_, window, _| (f32::from(window.viewport_size().width), f32::from(window.viewport_size().height))).unwrap_or((1919., 992.));
                let name = control_name(&target).unwrap_or_default();
                for _ in 0..16 {
                    let Ok(at) = found else { break };
                    let y = f32::from(at.y);
                    let clip = noshape_app_core::ui_script::clip_of(&name).filter(|c| c.size.height > px(30.));
                    let inside = clip.is_none_or(|c| c.top() + px(10.) < at.y && at.y < c.bottom() - px(10.));
                    if inside && (60.0..vh - 20.0).contains(&y) {
                        break;
                    }
                    // At the clip's right edge: the page's clip wheels the page, an inner list's that list.
                    let (wheel_at, mid) = match clip {
                        Some(c) if !inside => (point(c.right() - px(40.), c.center().y), f32::from(c.center().y)),
                        _ => (point(px(vw - 40.), px(vh / 2.)), vh / 2.),
                    };
                    let wheel = ScrollWheelEvent { position: wheel_at, delta: ScrollDelta::Pixels(point(px(0.), px((mid - y).clamp(-600., 600.)))), ..Default::default() };
                    let _ = w.update(cx, |_, window, cx| window.dispatch_event(PlatformInput::ScrollWheel(wheel), cx));
                    sleep(cx, 120).await;
                    redraw(w, cx).await;
                    found = find(&target, cx);
                }
                if let Ok(at) = found
                    && !(0.0..vh).contains(&f32::from(at.y))
                {
                    found = Err(format!("{} is out of view", control_name(&target).unwrap_or_default()));
                }
            }
            let result: Result<(), String> = match what {
                "click" | "dblclick" | "rclick" | "hover" => match found {
                    Ok(at) => {
                        let from = pointer.unwrap_or(at);
                        for k in 1..=3 {
                            let p = from + (at - from) * (k as f32 / 3.0);
                            let _ = w.update(cx, |_, window, cx| window.dispatch_event(PlatformInput::MouseMove(MouseMoveEvent { position: p, pressed_button: None, modifiers: mods }), cx));
                            sleep(cx, 20).await;
                        }
                        pointer = Some(at);
                        sleep(cx, if what == "hover" { step["ms"].as_u64().unwrap_or(1500) } else { 300 }).await;
                        let (button, clicks) = match what {
                            "rclick" => (MouseButton::Right, 1),
                            "dblclick" => (MouseButton::Left, 2),
                            "hover" => (MouseButton::Left, 0),
                            _ => (MouseButton::Left, 1),
                        };
                        let _ = w.update(cx, |_, window, cx| mouse(window, cx, at, button, mods, clicks));
                        // A select's pick: its entry from the list it opened.
                        match step["on"].get("pick").and_then(Value::as_str) {
                            Some(pick) => {
                                sleep(cx, 300).await;
                                redraw(w, cx).await;
                                match find(&json!({ "list_row": pick }), cx).or_else(|_| find(&json!({ "menu": pick }), cx)) {
                                    Ok(at) => {
                                        let _ = w.update(cx, |_, window, cx| {
                                            window.dispatch_event(PlatformInput::MouseMove(MouseMoveEvent { position: at, pressed_button: None, modifiers: Modifiers::default() }), cx);
                                            mouse(window, cx, at, MouseButton::Left, Modifiers::default(), 1)
                                        });
                                        pointer = Some(at);
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
                "type" => {
                    for ch in step["text"].as_str().unwrap_or("").chars() {
                        let key = if ch == ' ' { "Space".to_string() } else { ch.to_string() };
                        if let Some(ks) = keystroke(&key) {
                            let _ = w.update(cx, |_, window, cx| window.dispatch_keystroke(ks, cx));
                        }
                        sleep(cx, 40).await;
                    }
                    Ok(())
                }
                "key" => match keystroke(step["key"].as_str().unwrap_or("")) {
                    Some(ks) => {
                        let _ = w.update(cx, |_, window, cx| window.dispatch_keystroke(ks, cx));
                        Ok(())
                    }
                    None => Err(format!("unknown key {}", step["key"])),
                },
                "wait" => {
                    sleep(cx, step["ms"].as_u64().unwrap_or(500)).await;
                    Ok(())
                }
                other => Err(format!("unknown step {other} on a page")),
            };
            // A document a step opened is built and drawn before its state is read.
            sleep(cx, 900).await;
            let Some(w) = follow(cx) else { break };
            let state_now = read(w, cx).await;
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
            "app": { "name": "noshape", "version": noshape_ops::VERSION, "platform": std::env::consts::OS, "active": true },
            "start": start,
            "steps": steps,
        });
        write(&recording);
        eprintln!("UI-SCRIPT done: {}", out_path().display());
        cx.update(|cx| cx.quit());
    })
    .detach();
}

/// A document window's state after a step (README "state"): its active tab's, with the document's tab bar.
pub(crate) fn window_state(w: &mut Workspace, window: &mut Window, cx: &mut Context<Workspace>) -> Value {
    let mut s = if let Some(ps) = w.active_part_studio() {
        // A menu takes our keyboard but not a browser's: read the state with the focus the menu returns to.
        let ret = noshape_app_core::os_menu::open_return_focus(cx)
            .or_else(|| ps.read(cx).context_menu_return_focus(cx))
            .or_else(|| gpui_kit::component::native_menu::NativeMenu::open_fallback_context(window, cx));
        let mut s = match ret {
            None => ps.read(cx).ui_state(window, cx),
            Some(ret) => {
                let menu = window.focused(cx);
                window.focus(&ret, cx);
                let s = ps.read(cx).ui_state(window, cx);
                if let Some(m) = menu {
                    window.focus(&m, cx);
                }
                s
            }
        };
        // Edit in context: the assembly's instances as live reads them (asm-menu-edit-in-context).
        if s.is_object()
            && let Some(a) = w.any_assembly()
        {
            s["assembly"] = a.read(cx).ui_state(cx)["assembly"].clone();
        }
        s
    } else if let Some(d) = w.active_drawing() {
        d.read(cx).ui_state(cx)
    } else if let Some(a) = w.active_assembly() {
        a.read(cx).ui_state(cx)
    } else {
        other_tab_state(w, window, cx)
    };
    // Every state also has the document's tab bar (README "Documents, tabs and preferences").
    if s.is_object() {
        s["document"] = w.ui_document_state(cx);
        if let Some(f) = w.ui_rename_focus(window, cx) {
            s["focus"] = f;
        }
    }
    s
}

/// The state of a tab that is no Part Studio, Assembly or Drawing (a Variable or Feature Studio, a file): the
/// window's open menu, modal and dropdown, the tab rename's field.
fn other_tab_state(ws: &mut Workspace, window: &mut Window, cx: &mut Context<Workspace>) -> Value {
    let focus = match ws.ui_rename_focus(window, cx) {
        Some(f) => f,
        None if window.focused(cx).is_some() && !ws.ui_variables_hold_focus(window, cx) => json!({ "kind": "other" }),
        None => json!({ "kind": "none" }),
    };
    let list = noshape_app_core::os_menu::ui_state(cx).or_else(noshape_app_core::ui_script::open_list);
    let mut s = json!({
        "focus": focus,
        "dialog": noshape_app_core::ui_script::open_dialog(),
        "list": list,
        "selection": { "count": 0, "items": [] },
        "features": null,
        "parts": [],
        "message": null,
        "tooltip": null,
        "undo": { "undo": ws.doc.read(cx).can_undo(), "redo": ws.doc.read(cx).can_redo() },
        "sketch": null,
        "drawing": null,
        "variables": ws.ui_variables_state(cx),
    });
    // A Feature Studio: its editor, and its autocomplete as the list.
    if let Some(fs) = ws.ui_feature_studio_state(cx) {
        s["featurestudio"] = fs["featurestudio"].clone();
        if fs["unsaved"] == true {
            s["undo"]["undo"] = json!(true);
        }
        if !fs["list"].is_null() {
            s["list"] = fs["list"].clone();
        }
    }
    s
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
