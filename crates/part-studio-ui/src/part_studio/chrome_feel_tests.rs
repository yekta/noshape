//! The chrome matrix (docs/parity/interaction-feel.md "Chrome"): live2's feel/chrome.json rows (tooltips, the feature
//! dialog's place, drag and snap, Select other's place) acted out or computed, at a device pixel ratio of 1 and 2
//! where it draws. The shortcut map has its own matrix (app-core `the_shortcut_map_matches_lives`). A row ours
//! doesn't match yet is a gap; the test fails when a row's status moves either way.

use super::*;
use crate::part_studio::behaviors;
use gpui_kit::{Context, MouseMoveEvent, Pixels, Point, StatefulInteractiveElement as _, point, size};

const NONE: Modifiers = Modifiers { shift: false, control: false, alt: false, platform: false, function: false };

/// Two buttons with tooltips side by side, 100 px wide, at (20, 20) and (120, 20).
struct Tips;

impl Render for Tips {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let button = |id: &'static str, x: f32| div().id(id).absolute().left(px(x)).top(px(20.)).w(px(100.)).h(px(30.)).tooltip(crate::ui::tooltip(id));
        div().size_full().relative().child(button("First", 20.)).child(button("Second", 120.))
    }
}

struct Chrome {
    tips: AnyWindowHandle,
}

impl Chrome {
    fn open(cx: &mut TestAppContext, scale: f32) -> Self {
        let _ = part_studio_from(cx, &format!("chrome-{scale}"), None);
        // (An app window: it draws the open menu over its content.)
        let bounds = gpui_kit::Bounds { origin: point(px(0.), px(0.)), size: size(px(1920.), px(1080.)) };
        let options = gpui_kit::WindowOptions { window_bounds: Some(gpui_kit::WindowBounds::Windowed(bounds)), ..Default::default() };
        let (tips, _) = cx.update(|cx| crate::os_menu::open_window(options, cx, |_, cx| cx.new(|_| Tips))).unwrap();
        cx.update_window(tips, |_, w, _| w.set_scale_factor(scale)).unwrap();
        Chrome { tips }
    }

    fn move_to(&self, cx: &mut TestAppContext, x: f32, y: f32) {
        gpui_kit::VisualTestContext::from_window(self.tips, cx).simulate_event(MouseMoveEvent { position: point(px(x), px(y)), pressed_button: None, modifiers: NONE });
        draw(cx, self.tips);
    }

    /// `ms` of the clock, drawn every 10 ms; the tooltip shown then.
    fn wait(&self, cx: &mut TestAppContext, ms: u64) -> Option<String> {
        for _ in 0..ms / 10 {
            cx.executor().advance_clock(std::time::Duration::from_millis(10));
            draw(cx, self.tips);
        }
        crate::ui_script::shown_tooltip()
    }

    /// A three-row context menu opened for a right press at `at` (its rows' place `offset` from it); its bounds.
    fn menu_at(&self, cx: &mut TestAppContext, at: Point<Pixels>, offset: Point<Pixels>) -> Option<gpui_kit::Bounds<Pixels>> {
        self.move_to(cx, f32::from(at.x), f32::from(at.y));
        let menu = cx
            .update_window(self.tips, |_, w, cx| {
                // (Labels made at run time: ops' menu coverage reads the literal ones.)
                let menu = crate::os_menu::OsMenu::build(w, cx, |m, _, _| (1..=3).fold(m, |m, k| m.item(crate::os_menu::OsMenuItem::new(format!("Row {k}")))));
                crate::os_menu::open(at + offset, menu.clone(), w, cx);
                menu
            })
            .unwrap();
        draw(cx, self.tips);
        draw(cx, self.tips);
        // (The menu's box: its drawn bounds are inside its 1 px border.)
        let b = cx.update(|cx| crate::os_menu::open_bounds(cx)).map(|b| b.dilate(px(1.)));
        cx.update(|cx| menu.update(cx, |_, cx| cx.emit(gpui_kit::DismissEvent)));
        draw(cx, self.tips);
        b
    }

    /// The pointer from empty space onto the first button.
    fn hover_first(&self, cx: &mut TestAppContext) {
        self.move_to(cx, 300., 200.);
        let _ = self.wait(cx, 400);
        crate::ui_script::clear_tooltip();
        self.move_to(cx, 60., 35.);
    }
}

fn near(got: f32, want: f32, what: &str) -> Result<(), String> {
    if (got - want).abs() < 0.5 { Ok(()) } else { Err(format!("{what}: ours {got}, live {want}")) }
}

type Check = fn(&Chrome, &mut TestAppContext) -> Result<(), String>;

struct Row {
    id: &'static str,
    live: &'static str,
    check: Check,
    gap: bool,
}

/// Where a 220 × 300 dialog dropped at (x, y) lands in a 1000 × 700 graphics area.
fn dropped(x: f32, y: f32) -> (f32, f32) {
    let p = behaviors::place_feature_dialog(point(px(x), px(y)), size(px(220.), px(300.)), size(px(1000.), px(700.)));
    (f32::from(p.x), f32::from(p.y))
}

fn rows() -> Vec<Row> {
    vec![
        Row { id: "tooltip / shows after 400 ms", live: "Bootstrap delay show 400 ms (fully shown 470-545 ms with the fade)", gap: false, check: |c, cx| {
            c.hover_first(cx);
            let early = c.wait(cx, 350);
            let late = c.wait(cx, 80);
            if early.is_some() || late.as_deref() != Some("First") {
                return Err(format!("at 350 ms {early:?}, at 430 ms {late:?}"));
            }
            Ok(())
        } },
        Row { id: "tooltip / warm switch to the next button", live: "while one shows, the next button's appears at once (~60 ms), no 400 ms wait", gap: false, check: |c, cx| {
            c.hover_first(cx);
            let _ = c.wait(cx, 450);
            crate::ui_script::clear_tooltip();
            c.move_to(cx, 160., 35.);
            let next = c.wait(cx, 30);
            if next.as_deref() != Some("Second") {
                return Err(format!("30 ms after moving on: {next:?}"));
            }
            Ok(())
        } },
        Row { id: "tooltip / cold again after a pause", live: "gone ~200 ms after the pointer left; then the 400 ms wait again", gap: false, check: |c, cx| {
            c.hover_first(cx);
            let _ = c.wait(cx, 450);
            c.move_to(cx, 300., 200.);
            let _ = c.wait(cx, 400);
            crate::ui_script::clear_tooltip();
            c.move_to(cx, 160., 35.);
            let early = c.wait(cx, 100);
            if early.is_some() {
                return Err(format!("100 ms after coming back: {early:?}"));
            }
            Ok(())
        } },
        Row { id: "tooltip / longer help after 2 s", live: "data-expand-delay 2000 ms after it shows (measured 2.25-2.5 s from arrival)", gap: false, check: |_, _| {
            near(crate::ui::TOOLTIP_EXPAND_DELAY.as_millis() as f32, 2000., "ms")
        } },
        Row { id: "feature dialog / default place", live: "top left of the graphics area, 3 px below its top", gap: false, check: |_, _| {
            let (x, y) = dropped(0., 3.);
            near(x, 0., "x")?;
            near(y, 3., "y")
        } },
        Row { id: "feature dialog / snaps to the top", live: "within ~10 px of the top: 3 px", gap: false, check: |_, _| near(dropped(300., 11.).1, 3., "y") },
        Row { id: "feature dialog / snaps to the right", live: "within ~10 px of the right edge: flush", gap: false, check: |_, _| near(dropped(772., 200.).0, 780., "x") },
        Row { id: "feature dialog / snaps to the bottom", live: "within ~10 px of the bottom: 2 px off it", gap: false, check: |_, _| near(dropped(300., 390.).1, 398., "y") },
        Row { id: "feature dialog / the left edge doesn't snap", live: "dropped 8 px from the left it stays there", gap: false, check: |_, _| near(dropped(8., 200.).0, 8., "x") },
        Row { id: "feature dialog / can't leave the graphics area", live: "clamped inside it", gap: false, check: |_, _| {
            let (x, y) = dropped(2000., 2000.);
            near(x, 780., "x")?;
            near(y, 398., "y")?;
            near(dropped(-50., 200.).0, 0., "x left")
        } },
        Row { id: "context menu / at the pointer + (0, 2)", live: "its top left at the pointer + (0, 2) in the graphics", gap: false, check: |c, cx| {
            let b = c.menu_at(cx, point(px(300.), px(200.)), crate::os_menu::MENU_AT_POINTER).ok_or("no menu")?;
            near(f32::from(b.left()), 300., "left")?;
            near(f32::from(b.top()), 202., "top")
        } },
        Row { id: "context menu / flips at the window's right and bottom", live: "its right edge 1 px left of the pointer, its bottom 4 px below it (1860, 925: menu at 1738, 837, 121 x 92)", gap: false, check: |c, cx| {
            let size = cx.update_window(c.tips, |_, w, _| w.viewport_size()).unwrap();
            let at = point(size.width - px(20.), size.height - px(20.));
            let b = c.menu_at(cx, at, crate::os_menu::MENU_AT_POINTER).ok_or("no menu")?;
            near(f32::from(b.right()), f32::from(at.x) - 1., "right")?;
            near(f32::from(b.bottom()), f32::from(at.y) + 4., "bottom")
        } },
        Row { id: "context menu / a feature-list row's at + (10, 12)", live: "+ (10, 12) on feature-list rows", gap: false, check: |c, cx| {
            let b = c.menu_at(cx, point(px(300.), px(200.)), crate::os_menu::MENU_AT_ROW).ok_or("no menu")?;
            near(f32::from(b.left()), 310., "left")?;
            near(f32::from(b.top()), 212., "top")
        } },
        Row { id: "feature list / its width per tab", live: "remembered per tab (left-content-pane<elementId>PanelLength); a tab not resized has the default 205", gap: false, check: |_, _| {
            let (a, b) = (noshape_model::document::TabId::new_random(), noshape_model::document::TabId::new_random());
            crate::ui::remember_left_panel_w(a, 320.);
            near(crate::ui::left_panel_w(a), 320., "the widened tab")?;
            near(crate::ui::left_panel_w(b), crate::ui::LEFT_PANEL_W, "another tab")
        } },
        Row { id: "select other / its size", live: "220 px wide, 24 px rows in a 90 px list (~3.75 rows); with nothing found 83 px tall", gap: false, check: |_, _| {
            near(behaviors::SELECT_OTHER_SIZE.0, 220., "width")?;
            near(behaviors::SELECT_OTHER_EMPTY_H, 83., "height with nothing found")
        } },
        Row { id: "select other / its place", live: "the dialog's top left at the pointer + (100, 51)", gap: false, check: |_, _| {
            near(behaviors::SELECT_OTHER_OFFSET.0, 100., "x")?;
            near(behaviors::SELECT_OTHER_OFFSET.1, 51., "y")
        } },
    ]
}

/// Every row at a pixel ratio of 1 and 2. Prints matched / total.
#[gpui::test]
fn the_chrome_matrix_matches_live(cx: &mut TestAppContext) {
    let rows = rows();
    let chromes: Vec<Chrome> = [1.0f32, 2.0].iter().map(|s| Chrome::open(cx, *s)).collect();
    let mut report = Vec::new();
    let mut moved = Vec::new();
    for r in &rows {
        let mut why = Vec::new();
        for (c, scale) in chromes.iter().zip([1, 2]) {
            if let Err(e) = (r.check)(c, cx) {
                why.push(format!("scale {scale}: {e}"));
            }
        }
        let ok = why.is_empty();
        report.push(format!("{} {} (live: {}){}", if ok { "=" } else { "≠" }, r.id, r.live, if ok { String::new() } else { format!("\n    {}", why.join("\n    ")) }));
        if ok == r.gap {
            moved.push(r.id);
        }
    }
    let matched = report.iter().filter(|l| l.starts_with('=')).count();
    eprintln!("chrome matrix: {matched} / {} rows match live\n{}", report.len(), report.join("\n"));
    assert!(moved.is_empty(), "rows whose status moved (update interaction-feel.md and the row's gap): {moved:?}");
}
