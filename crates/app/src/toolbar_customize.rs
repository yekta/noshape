//! Customize toolbar… (right-click the toolbar), the same in the Part Studio
//! and the assembly (live part-studio/340–347, assembly/340–343,
//! dom/customize-toolbar-*.txt, behaviours.md "Toolbar right-click"):
//! - every tool set gets a `#f5f5f5` background and, hanging under the
//!   toolbar, a 10 px `#cccccc` sled with "•••" (cursor `move`) that drags
//!   the set; Undo / Redo have none;
//! - a tool (a split tool with its whole dropdown) drags onto the 600 px
//!   "Tools" panel to leave the toolbar: it is gone from the toolbar while
//!   dragged and the panel turns into a red dashed drop zone; tools in the
//!   panel drag back onto the toolbar;
//! - footer: ☐ Show items already in toolbar · Save · Reset to default ·
//!   Cancel (Save and Reset at 25 % until there is something to do).

use crate::theme::p;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::cell::Cell;

/// A tool of the Tools panel.
#[derive(Clone)]
pub(crate) struct PanelTool {
    /// The tool, as a one-tool slice of the studio's static tool lists.
    pub ids: &'static [&'static str],
    pub label: SharedString,
    /// The icon's SVG path.
    pub icon: SharedString,
}

/// How a view reaches its customize state.
pub(crate) type Field<V> = fn(&mut V) -> Option<&mut Customize>;
/// The saved tool set order and removed tools.
pub(crate) type Saved = (Vec<usize>, Vec<String>);
/// Ends customize mode: `Some` on Save, `None` on Cancel.
pub(crate) type Done<V> = fn(&mut V, Option<Saved>, &mut Context<V>);

/// The edited toolbar while customizing.
pub(crate) struct Customize {
    /// Tool set order (indices into the studio's default sets).
    pub order: Vec<usize>,
    /// Tools out of the toolbar.
    pub removed: Vec<String>,
    saved: Saved,
    pub show_all: bool,
    pub search: Entity<InputState>,
    _sub: Subscription,
}

/// Every set once, in `order` first, the missing ones appended.
pub(crate) fn full_order(order: &[usize], n: usize) -> Vec<usize> {
    let mut out: Vec<usize> = Vec::new();
    for i in order.iter().copied().filter(|i| *i < n) {
        if !out.contains(&i) {
            out.push(i);
        }
    }
    out.extend((0..n).filter(|i| !order.contains(i)));
    out
}

impl Customize {
    pub fn new<V: 'static>(n_sets: usize, order: &[usize], removed: &[String], window: &mut Window, cx: &mut Context<V>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).placeholder("Search tools"));
        let sub = cx.subscribe(&search, |_, _, ev: &InputEvent, cx| {
            if matches!(ev, InputEvent::Change) {
                cx.notify();
            }
        });
        let order = full_order(order, n_sets);
        Self { saved: (order.clone(), removed.to_vec()), order, removed: removed.to_vec(), show_all: false, search, _sub: sub }
    }

    fn changed(&self) -> bool {
        let sorted = |v: &[String]| {
            let mut v = v.to_vec();
            v.sort();
            v
        };
        self.order != self.saved.0 || sorted(&self.removed) != sorted(&self.saved.1)
    }

    fn customized(&self) -> bool {
        !self.removed.is_empty() || self.order.iter().enumerate().any(|(i, s)| i != *s)
    }

    pub fn is_removed(&self, id: &str) -> bool {
        self.removed.iter().any(|r| r == id)
    }

    /// Move set `set` before set `before` (drop a sled on a set).
    fn move_set(&mut self, set: usize, before: usize) {
        self.order.retain(|i| *i != set);
        let at = self.order.iter().position(|i| *i == before).unwrap_or(self.order.len());
        self.order.insert(at, set);
    }
}

thread_local! {
    /// The tools being dragged and whether from the toolbar (the drag value
    /// is not readable from views).
    static DRAGGED: Cell<Option<(&'static [&'static str], bool)>> = const { Cell::new(None) };
}

fn dragged(cx: &App) -> Option<(&'static [&'static str], bool)> {
    if cx.has_active_drag() { DRAGGED.with(|d| d.get()) } else { None }
}

/// Tools dragged in customize mode (a split tool brings its dropdown).
#[derive(Clone)]
pub(crate) struct ToolDrag {
    ids: &'static [&'static str],
    icon: SharedString,
}

impl Render for ToolDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        // Live 345: the tool's own look (icon, and ▾ for a split tool).
        h_flex()
            .h(px(32.))
            .px(px(6.))
            .gap(px(2.))
            .child(svg().path(self.icon.clone()).size(px(20.)).text_color(gpui_kit::rgb(0x333333)))
            .when(self.ids.len() > 1, |d| d.child(crate::ui::glyph("ui-caret-down", 8., gpui_kit::rgb(0x333333))))
    }
}

/// A tool set dragged by its sled.
#[derive(Clone)]
pub(crate) struct ToolsetDrag {
    set: usize,
}

impl Render for ToolsetDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        sled_look(div().w(px(40.)))
    }
}

fn sled_look(d: Div) -> Div {
    d.h(px(10.))
        .bg(gpui_kit::rgb(0xcccccc))
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(10.))
        .font_weight(FontWeight::BOLD)
        .line_height(px(10.))
        .text_color(gpui_kit::rgb(0x333333))
        .child("•••")
}

/// A toolbar tool in customize mode: a cover drags it (cursor `move`) and
/// keeps clicks off the tool; it is gone from the toolbar while dragged.
pub(crate) fn tool(el: impl IntoElement, ids: &'static [&'static str], icon: SharedString, cx: &App) -> AnyElement {
    if dragged(cx).is_some_and(|(d, bar)| bar && d == ids) {
        return div().into_any_element();
    }
    div()
        .relative()
        .child(el)
        .child(
            div()
                .id(SharedString::from(format!("tbc-{}", ids[0])))
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .occlude()
                .cursor(CursorStyle::Move)
                .on_drag(ToolDrag { ids, icon }, |d, _, _, cx| {
                    DRAGGED.with(|g| g.set(Some((d.ids, true))));
                    cx.new(|_| d.clone())
                }),
        )
        .into_any_element()
}

/// One tool set in customize mode (live `div.toolset.allowDrag.customizing`,
/// 34 px in the 36 px bar): `#f5f5f5`, and with `sled` the "•••" sled
/// hanging under the bar (drawn over what is below). Drop a sled on a set
/// to put that set before it; drop a tool from the Tools panel to put it back.
pub(crate) fn toolset<V: 'static>(set: usize, content: impl IntoElement, sled: bool, field: Field<V>, cx: &mut Context<V>) -> AnyElement {
    h_flex()
        .id(SharedString::from(format!("tbset-{set}")))
        .relative()
        .h(px(34.))
        .bg(gpui_kit::rgb(0xf5f5f5))
        .child(content)
        .when(sled, |d| {
            d.child(deferred(
                sled_look(div())
                    .id(SharedString::from(format!("tbsled-{set}")))
                    .absolute()
                    .left_0()
                    .right_0()
                    .top(px(34.))
                    .cursor(CursorStyle::Move)
                    .on_drag(ToolsetDrag { set }, |d, _, _, cx| cx.new(|_| d.clone()))
                    // A sled dragged along the sleds drops on this set too.
                    .on_drop(cx.listener(move |this, d: &ToolsetDrag, _, cx| {
                        if let Some(c) = field(this)
                            && d.set != set
                        {
                            c.move_set(d.set, set);
                            cx.notify();
                        }
                    })),
            ))
        })
        .drag_over::<ToolsetDrag>(|st, _, _, _| st.border_l_2().border_color(gpui_kit::rgb(0x1651b0)))
        .on_drop(cx.listener(move |this, d: &ToolsetDrag, _, cx| {
            if let Some(c) = field(this)
                && d.set != set
            {
                c.move_set(d.set, set);
                cx.notify();
            }
        }))
        .on_drop(cx.listener(move |this, d: &ToolDrag, _, cx| {
            if let Some(c) = field(this) {
                c.removed.retain(|r| !d.ids.contains(&r.as_str()));
                cx.notify();
            }
        }))
        .into_any_element()
}

/// A footer button (live `os-button`: 30 px, radius 3, 14.67 px text 10 px
/// in; primary `#1651b0`, default white with a 1 px `#aaa` ring; 25 %
/// while disabled).
fn button(id: &'static str, label: &'static str, primary: bool, enabled: bool) -> Stateful<Div> {
    div()
        .id(id)
        .h(px(30.))
        .px(px(10.))
        .flex()
        .items_center()
        .rounded(px(3.))
        .text_size(px(14.67))
        .when(primary, |d| d.bg(gpui_kit::rgb(0x1651b0)).text_color(gpui_kit::rgb(0xffffff)))
        .when(!primary, |d| d.bg(gpui_kit::rgb(0xffffff)).border_1().border_color(gpui_kit::rgb(0xaaaaaa)).text_color(gpui_kit::rgb(0x333333)))
        .when(!enabled, |d| d.opacity(0.25))
        .when(enabled, |d| d.cursor_pointer())
        .child(label)
}

/// The "Tools" panel (live dom/customize-toolbar-part-studio.txt: 600 px,
/// `#f5f5f5`, a 43 px header and a 42 px footer on white). `tools` is every
/// tool in toolbar order; `done` ends customize mode, with the edited order
/// and removed tools on Save (None on Cancel).
pub(crate) fn panel<V: 'static>(
    view: &mut V,
    tools: &[PanelTool],
    field: Field<V>,
    done: Done<V>,
    window: &Window,
    cx: &mut Context<V>,
) -> Option<AnyElement> {
    let c = field(view)?;
    let pal = p();
    let shadow = |y: f32, blur: f32| vec![BoxShadow { color: hsla(0., 0., 0., 0.247), offset: point(px(0.), px(y)), blur_radius: px(blur), spread_radius: px(0.), inset: false }];
    let root = v_flex()
        .id("tb-custom-panel")
        .occlude()
        .w(px(600.))
        .bg(gpui_kit::rgb(0xf5f5f5))
        .text_color(pal.text)
        // 0 5px 10px; GPUI blurs twice as far as CSS.
        .shadow(shadow(5., 5.))
        // Dropping a toolbar tool here takes it out of the toolbar.
        .on_drop(cx.listener(move |this, d: &ToolDrag, _, cx| {
            if let Some(c) = field(this) {
                for id in d.ids {
                    if !c.is_removed(id) {
                        c.removed.push(id.to_string());
                    }
                }
                cx.notify();
            }
        }));
    // Live 345: while a toolbar tool is dragged, the panel is only the drop zone.
    if dragged(cx).is_some_and(|(_, bar)| bar) {
        return Some(
            root.pt(px(10.))
                .px(px(10.))
                .pb(px(11.))
                .child(div().h(px(21.)).text_center().text_size(px(14.)).line_height(px(21.)).child("Drop items here to remove them from the toolbar"))
                .child(div().h(px(99.)).border_1().border_dashed().border_color(gpui_kit::rgb(0xbd3039)))
                .into_any_element(),
        );
    }
    let query = c.search.read(cx).value().trim().to_lowercase();
    let (changed, customized, show_all) = (c.changed(), c.customized(), c.show_all);
    let listed: Vec<(PanelTool, bool)> =
        tools.iter().filter(|t| c.show_all || c.is_removed(t.ids[0])).filter(|t| query.is_empty() || t.label.to_lowercase().contains(&query)).map(|t| (t.clone(), !c.is_removed(t.ids[0]))).collect();
    // Live 343: three 186 px columns of 34 px rows (20 px icon, 14 px name
    // 4 px after it); tools still in the toolbar are greyed.
    let mut grid = div().flex().flex_wrap().pt(px(10.)).pl(px(17.)).pb(px(6.));
    for (t, in_bar) in listed.iter().cloned() {
        let ids = t.ids;
        grid = grid.child(
            h_flex()
                .id(SharedString::from(format!("tbtool-{}", t.ids[0])))
                .w(px(186.))
                .h(px(34.))
                .gap(px(4.))
                .when(in_bar, |d| d.opacity(0.4))
                .when(!in_bar, |d| {
                    d.cursor(CursorStyle::Move).on_drag(ToolDrag { ids, icon: t.icon.clone() }, |d, _, _, cx| {
                        DRAGGED.with(|g| g.set(Some((d.ids, false))));
                        cx.new(|_| d.clone())
                    })
                })
                .child(svg().path(t.icon.clone()).size(px(20.)).flex_none().text_color(gpui_kit::rgb(0x333333)))
                .child(div().truncate().text_size(px(14.)).child(t.label.clone())),
        );
    }
    let body: AnyElement = if listed.is_empty() {
        // Live (dom): the bold line and the hint, centred, 62 px in all.
        v_flex()
            .pt(px(11.))
            .pb(px(9.))
            .items_center()
            .text_size(px(14.))
            .line_height(px(21.))
            .child(div().font_weight(FontWeight::BOLD).child("All items are already in the toolbar"))
            .child("Drop items here to remove them from the toolbar")
            .into_any_element()
    } else {
        // Live 343: the list scrolls; the panel ends ~100 px above the window's bottom.
        let max = (f32::from(window.viewport_size().height) - 270.).max(120.);
        div().id("tb-custom-body").max_h(px(max)).overflow_y_scroll().child(grid).into_any_element()
    };
    Some(
        root.child(
            // Live (dom): "Tools" in a 31 px box at the top of the 37 px row,
            // the 198 x 30 search box 1 px lower.
            h_flex()
                .items_start()
                .h(px(43.))
                .pt(px(6.))
                .px(px(3.))
                .bg(gpui_kit::rgb(0xffffff))
                .shadow(shadow(0., 0.5))
                .child(div().flex_1().p(px(5.)).text_size(px(14.)).font_weight(FontWeight::BOLD).line_height(px(21.)).child("Tools"))
                .child(div().w(px(198.)).mt(px(1.)).mr(px(3.)).child(Input::new(&c.search).small().h(px(30.)).text_size(px(14.)).pl(px(5.)))),
        )
        .child(body)
        .child(
            h_flex()
                .h(px(42.))
                .pt(px(6.))
                .px(px(3.))
                .bg(gpui_kit::rgb(0xffffff))
                .shadow(shadow(0., 0.5))
                .child(div().pl(px(7.)).child(crate::os_ui::checkbox("tb-show-all", "Show items already in toolbar", show_all).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if let Some(c) = field(this) {
                        c.show_all = !c.show_all;
                    }
                    cx.notify();
                }))))
                .child(div().flex_1())
                .child(button("tb-save", "Save", true, changed).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if changed && let Some(c) = field(this) {
                        let (order, removed) = (c.order.clone(), c.removed.clone());
                        done(this, Some((order, removed)), cx);
                    }
                    cx.notify();
                })))
                .child(div().w(px(7.)))
                .child(button("tb-reset", "Reset to default", false, customized).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if let Some(c) = field(this)
                        && customized
                    {
                        let n = c.order.len();
                        c.order = (0..n).collect();
                        c.removed.clear();
                    }
                    cx.notify();
                })))
                .child(div().w(px(5.)))
                .child(button("tb-cancel", "Cancel", false, true).mr(px(3.)).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    done(this, None, cx);
                    cx.notify();
                }))),
        )
        .into_any_element(),
    )
}
