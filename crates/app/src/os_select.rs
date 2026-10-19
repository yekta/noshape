//! Onshape's select (feature dialog End type, Hole type, …): closed, the
//! current item in 14 px text over a `#d7d7d7` underline with a caret; open,
//! a search input with a 2 px `#1651b0` underline takes its place and the
//! list hangs under it (live dialogs/extrude-13..16, behaviours.md "End
//! type"):
//!
//! - list as wide as the select (194 px in feature dialogs), white, 1 px
//!   `#d7d7d7` border, no shadow, 5 px above the first row and 4 px below the
//!   last; rows 26 px, 14 px `#666` text 12 px in;
//! - the highlighted row (at first the current item) is `#1651b0` with white
//!   text; ↑/↓ move it, the pointer moves it, Enter or a click picks it;
//! - typing filters the items (the matched letters bold) and highlights the
//!   first match; a blue × clears the text; Esc or a click elsewhere closes.
//!
//! The host keeps one `OsSelect` per field, calls [`OsSelect::set_items`] when
//! rendering and listens for [`OsSelectEvent::Picked`].

use crate::theme::p;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::cell::Cell;
use std::rc::Rc;

/// Row height, list padding and text inset (live).
const ROW_H: f32 = 26.;
const LIST_PAD_TOP: f32 = 5.;
const LIST_PAD_BOTTOM: f32 = 4.;
const TEXT_INSET: f32 = 12.;
/// Height of the closed select with its 1 px underline (live: 26 px in a
/// 28 px row); the open search input is 1 px taller (2 px underline).
const FIELD_H: f32 = 26.;
/// Tallest open list (live: a 9-item list shows 7½ rows and scrolls).
const LIST_MAX_H: f32 = 200.;

actions!(os_select, [SelectUp, SelectDown, SelectDismiss]);

/// Keys of an open select, bound in its own key context so they win over the
/// dialog's (Esc = cancel the dialog) and the window's (arrows = rotate the view).
fn bind_keys(cx: &mut App) {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        // Also on the search input itself: bindings matched at a deeper context
        // run first, and "FeatureDialog > Input" binds Esc to Cancel there; ours
        // are added later, so they win at the same depth.
        for ctx in ["OsSelect", "OsSelect > Input"] {
            cx.bind_keys([KeyBinding::new("up", SelectUp, Some(ctx)), KeyBinding::new("down", SelectDown, Some(ctx)), KeyBinding::new("escape", SelectDismiss, Some(ctx))]);
        }
    });
}

pub enum OsSelectEvent {
    Picked(SharedString),
}

pub struct OsSelect {
    id: SharedString,
    /// (id, label) in display order.
    items: Vec<(SharedString, SharedString)>,
    current: SharedString,
    /// Field height (with its underline).
    height: f32,
    /// The search input while open.
    search: Option<Entity<InputState>>,
    query: String,
    highlight: usize,
    /// Where keyboard focus was before opening, to give it back on close.
    return_focus: Option<FocusHandle>,
    /// The field's and the open list's painted bounds (the list matches the
    /// field's width; presses inside the list don't close it).
    bounds: Rc<Cell<Bounds<Pixels>>>,
    list_bounds: Rc<Cell<Bounds<Pixels>>>,
    /// Scroll of the open list (at most [`LIST_MAX_H`] tall).
    scroll: ScrollHandle,
    _subs: Vec<Subscription>,
}

impl EventEmitter<OsSelectEvent> for OsSelect {}

impl OsSelect {
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self { id: id.into(), items: Vec::new(), current: SharedString::default(), height: FIELD_H, search: None, query: String::new(), highlight: 0, return_focus: None, bounds: Default::default(), list_bounds: Default::default(), scroll: ScrollHandle::new(), _subs: Vec::new() }
    }

    /// A shorter field (Hole's lookup rows are 22 px).
    pub fn set_height(&mut self, height: f32) {
        self.height = height;
    }

    /// The items and the current one (call on every host render).
    pub fn set_items(&mut self, items: Vec<(SharedString, SharedString)>, current: impl Into<SharedString>) {
        self.items = items;
        self.current = current.into();
    }

    pub fn is_open(&self) -> bool {
        self.search.is_some()
    }

    /// Items matching the query, each with the byte range of the match.
    fn matches(&self) -> Vec<(usize, Option<std::ops::Range<usize>>)> {
        let q = self.query.trim().to_lowercase();
        self.items
            .iter()
            .enumerate()
            .filter_map(|(i, (_, label))| {
                if q.is_empty() {
                    return Some((i, None));
                }
                let l = label.to_lowercase();
                // Lowercasing keeps byte offsets for the ASCII labels used here.
                l.find(&q).filter(|_| l.len() == label.len()).map(|at| (i, Some(at..at + q.len())))
            })
            .collect()
    }

    fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        bind_keys(cx);
        let st = cx.new(|cx| InputState::new(window, cx));
        let sub = cx.subscribe_in(&st, window, |this: &mut Self, st, ev: &InputEvent, window, cx| match ev {
            InputEvent::Change => {
                this.query = st.read(cx).value().to_string();
                this.highlight = 0;
                cx.notify();
            }
            InputEvent::PressEnter { .. } => this.pick_highlighted(window, cx),
            _ => {}
        });
        self.return_focus = window.focused(cx);
        st.update(cx, |s, cx| s.focus(window, cx));
        self.search = Some(st);
        self._subs = vec![sub];
        self.query.clear();
        self.highlight = self.items.iter().position(|(id, _)| *id == self.current).unwrap_or(0);
        self.scroll.scroll_to_item(self.highlight);
        cx.notify();
    }

    pub fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.search.take().is_none() {
            return;
        }
        self._subs.clear();
        self.query.clear();
        if let Some(f) = self.return_focus.take() {
            window.focus(&f, cx);
        }
        cx.notify();
    }

    fn pick(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some((id, _)) = self.items.get(index).cloned() else { return };
        self.close(window, cx);
        self.current = id.clone();
        cx.emit(OsSelectEvent::Picked(id));
    }

    fn pick_highlighted(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let m = self.matches();
        if let Some(&(i, _)) = m.get(self.highlight.min(m.len().saturating_sub(1))) {
            self.pick(i, window, cx);
        }
    }

    fn render_closed(&self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let label = self.items.iter().find(|(id, _)| *id == self.current).map(|(_, l)| l.clone()).unwrap_or_default();
        h_flex()
            .id(self.id.clone())
            .w_full()
            .h(px(self.height))
            // Live (dialogs/chamfer-01): the value and caret sit 1 px below
            // the field's centre, the caret 5 px from the end.
            .pt(px(2.))
            .pl(px(6.))
            .pr(px(5.))
            .border_b_1()
            .border_color(pal.border)
            // `#def1fb` hover (§4.5).
            .hover(|d| d.bg(pal.row_hover))
            .cursor_pointer()
            .child(div().flex_1().min_w_0().text_size(px(14.)).text_color(pal.text).truncate().child(label))
            .child(crate::ui::glyph("ui-caret-down", 10., pal.text))
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.open(window, cx)))
            .into_any_element()
    }

    fn render_list(&self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let accent = gpui_kit::rgb(0x1651b0);
        let m = self.matches();
        let hl = self.highlight.min(m.len().saturating_sub(1));
        let width = self.bounds.get().size.width.max(px(120.));
        let probe = self.list_bounds.clone();
        let rows = m.into_iter().enumerate().map(|(row, (i, range))| {
            let label = self.items[i].1.clone();
            let on = row == hl;
            let text: AnyElement = match range {
                Some(r) => StyledText::new(label).with_highlights([(r, HighlightStyle { font_weight: Some(FontWeight::BOLD), ..Default::default() })]).into_any_element(),
                None => label.into_any_element(),
            };
            div()
                .id(row)
                .h(px(ROW_H))
                .flex()
                .items_center()
                .pl(px(TEXT_INSET))
                .text_size(px(14.))
                .when(on, |d| d.bg(accent).text_color(gpui_kit::rgb(0xffffff)))
                .when(!on, |d| d.text_color(gpui_kit::rgb(0x666666)))
                .child(text)
                .on_hover(cx.listener(move |this, h: &bool, _, cx| {
                    if *h && this.highlight != row {
                        this.highlight = row;
                        cx.notify();
                    }
                }))
                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.pick(i, window, cx)))
        });
        deferred(
            anchored().snap_to_window_with_margin(px(4.)).child(
                v_flex()
                    .id("os-select-list")
                    .occlude()
                    .w(width)
                    .bg(gpui_kit::rgb(0xffffff))
                    .border_1()
                    .border_color(pal.border)
                    .pt(px(LIST_PAD_TOP))
                    .pb(px(LIST_PAD_BOTTOM))
                    .font_family(cx.theme().font_family.clone())
                    .relative()
                    // Live (dialogs/hole-33): long lists (tolerance type, fit
                    // classes) scroll in a 200 px list.
                    .max_h(px(LIST_MAX_H))
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .children(rows)
                    .child(canvas(move |b, _, _| probe.set(b), |_, _, _, _| {}).absolute().top_0().left_0().size_full()),
            ),
        )
        .with_priority(3)
        .into_any_element()
    }
}

impl Render for OsSelect {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let probe = self.bounds.clone();
        let accent = gpui_kit::rgb(0x1651b0);
        let field = match &self.search {
            None => self.render_closed(cx),
            Some(st) => v_flex()
                .w_full()
                .child(
                    // Live: the text 4 px in, its baseline 6 px above the underline;
                    // the 9 px × centered 11 px from the right edge.
                    h_flex()
                        .w_full()
                        .h(px(self.height + 1.))
                        .items_end()
                        .border_b_2()
                        .border_color(accent)
                        .child(div().flex_1().min_w_0().ml(px(-6.)).relative().top(px(4.)).child(Input::new(st).appearance(false).text_size(px(14.))))
                        .when(!self.query.is_empty(), |d| {
                            d.child(
                                div()
                                    .id("os-select-clear")
                                    .size(px(20.))
                                    .mr(px(1.))
                                    .mb(px(1.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_pointer()
                                    .child(crate::ui::glyph("ui-clear", 11., accent))
                                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                        if let Some(st) = &this.search {
                                            st.update(cx, |s, cx| s.set_value("", window, cx));
                                        }
                                        this.query.clear();
                                        this.highlight = 0;
                                        cx.notify();
                                    })),
                            )
                        }),
                )
                // The list hangs from here, over the rows below.
                .child(div().w_full().h_0().child(self.render_list(cx)))
                .into_any_element(),
        };
        div()
            .relative()
            .w_full()
            .key_context("OsSelect")
            .on_action(cx.listener(|this, _: &SelectDismiss, window, cx| this.close(window, cx)))
            .on_action(cx.listener(|this, _: &SelectDown, _, cx| {
                let n = this.matches().len();
                if n > 0 {
                    this.highlight = (this.highlight.min(n - 1) + 1).min(n - 1);
                    this.scroll.scroll_to_item(this.highlight);
                    cx.notify();
                }
            }))
            .on_action(cx.listener(|this, _: &SelectUp, _, cx| {
                let n = this.matches().len();
                this.highlight = this.highlight.min(n.saturating_sub(1)).saturating_sub(1);
                this.scroll.scroll_to_item(this.highlight);
                cx.notify();
            }))
            // Tab leaves the select (live: the dropdowns aren't tab stops).
            .capture_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                if this.is_open() && ev.keystroke.key == "tab" {
                    this.close(window, cx);
                }
            }))
            .on_mouse_down_out(cx.listener(|this, ev: &MouseDownEvent, window, cx| {
                if !this.list_bounds.get().contains(&ev.position) {
                    this.close(window, cx);
                }
            }))
            .child(field)
            .child(canvas(move |b, _, _| probe.set(b), |_, _, _, _| {}).absolute().top_0().left_0().size_full())
    }
}
